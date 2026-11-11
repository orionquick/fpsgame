// GPU-driven visibility: per instance frustum test, screen-size level of detail and Hi-Z occlusion against last
// frame's depth pyramid. Visible instance ids are appended to per-(batch, level) lists whose counts feed indirect
// draws. Plus the depth-pyramid builders (max depth: the farthest surface in each texel).

struct Inst { model: mat4x4f, tint: vec4f, params: vec4f };
struct BatchInfo {
    lmin: vec3f, n_lods: u32,
    lmax: vec3f, draw0: u32,
    flags: u32, p0: u32, p1: u32, p2: u32,
    // simplification error per level, model units
    err: vec4f,
};
struct View {
    planes: array<vec4f, 6>,
    // the frame the depth pyramid came from
    prev_vp: mat4x4f,
    // xyz eye; w px per metre at 1 m (perspective) or px per metre (ortho)
    eye: vec4f,
    // x ortho, y cull below this projected radius (px), z largest allowed level error (px)
    lod: vec4f,
    // x pyramid valid, y/z its size, w mip count
    hiz: vec4f,
    // x shadow view (skip glazing), y instances, z threads per dispatch row
    misc: vec4u,
    lod2: vec4f,
};

@group(0) @binding(0) var<storage, read> insts: array<Inst>;
@group(0) @binding(1) var<storage, read> inst_batch: array<u32>;
@group(0) @binding(2) var<storage, read> batches: array<BatchInfo>;
@group(0) @binding(3) var<storage, read> draw_base: array<u32>;
@group(0) @binding(4) var<uniform> view: View;
@group(0) @binding(5) var<storage, read_write> args: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read_write> ids: array<u32>;
@group(0) @binding(7) var hiz: texture_2d<f32>;

fn occluded(lo: vec3f, hi: vec3f) -> bool {
    var mn = vec2f(1e9);
    var mx = vec2f(-1e9);
    var zmin = 1.0;
    for (var k = 0u; k < 8u; k++) {
        let p = vec3f(select(lo.x, hi.x, (k & 1u) != 0u), select(lo.y, hi.y, (k & 2u) != 0u), select(lo.z, hi.z, (k & 4u) != 0u));
        let c = view.prev_vp * vec4f(p, 1.0);
        if (c.w <= 0.05) {
            return false; // crosses the eye plane: can't judge
        }
        let n = c.xyz / c.w;
        mn = min(mn, n.xy);
        mx = max(mx, n.xy);
        zmin = min(zmin, n.z);
    }
    if (mx.x < -1.0 || mn.x > 1.0 || mx.y < -1.0 || mn.y > 1.0) {
        return false; // off last frame's screen: no depth to test against
    }
    let uv0 = clamp(vec2f(mn.x * 0.5 + 0.5, 0.5 - mx.y * 0.5), vec2f(0.0), vec2f(1.0));
    let uv1 = clamp(vec2f(mx.x * 0.5 + 0.5, 0.5 - mn.y * 0.5), vec2f(0.0), vec2f(1.0));
    let size = (uv1 - uv0) * view.hiz.yz;
    let level = u32(clamp(ceil(log2(max(max(size.x, size.y), 1.0))), 0.0, view.hiz.w - 1.0));
    let dims = vec2f(textureDimensions(hiz, level));
    let p0 = vec2i(min(uv0 * dims, dims - 1.0));
    let p1 = vec2i(min(uv1 * dims, dims - 1.0));
    let d = max(max(textureLoad(hiz, p0, level).r, textureLoad(hiz, vec2i(p1.x, p0.y), level).r),
                max(textureLoad(hiz, vec2i(p0.x, p1.y), level).r, textureLoad(hiz, p1, level).r));
    return zmin > d;
}

@compute @workgroup_size(64)
fn cull(@builtin(global_invocation_id) gid: vec3u) {
    let i = gid.x + gid.y * view.misc.z;
    if (i >= view.misc.y) {
        return;
    }
    let b = batches[inst_batch[i]];
    if (view.misc.x == 1u && (b.flags & 1u) != 0u) {
        return;
    }
    let m = insts[i].model;
    let c = (m * vec4f((b.lmin + b.lmax) * 0.5, 1.0)).xyz;
    let e = (b.lmax - b.lmin) * 0.5;
    let we = abs(m[0].xyz) * e.x + abs(m[1].xyz) * e.y + abs(m[2].xyz) * e.z;
    for (var k = 0u; k < 6u; k++) {
        let pl = view.planes[k];
        if (dot(pl.xyz, c) + pl.w + dot(abs(pl.xyz), we) < 0.0) {
            return;
        }
    }
    let r = length(we);
    // px per metre at the instance (perspective: at its nearest point)
    var ppm = view.eye.w;
    if (view.lod.x == 0.0) {
        ppm = ppm / max(distance(c, view.eye.xyz) - r, 1e-3);
    }
    if (r * ppm < view.lod.y) {
        return;
    }
    // the coarsest level whose error projects under the allowed pixels (model error times the instance scale)
    let scale = max(length(m[0].xyz), max(length(m[1].xyz), length(m[2].xyz)));
    var lod = 0u;
    for (var k = b.n_lods - 1u; k > 0u; k--) {
        if (b.err[k] * scale * ppm <= view.lod.z) {
            lod = k;
            break;
        }
    }
    let inside = all(view.eye.xyz > c - we) && all(view.eye.xyz < c + we);
    if (view.hiz.x > 0.0 && !inside && occluded(c - we, c + we)) {
        return;
    }
    let d = b.draw0 + lod;
    let slot = atomicAdd(&args[d * 5u + 1u], 1u);
    ids[draw_base[d] + slot] = i;
}

// ---- depth pyramid ----
@group(0) @binding(0) var src_ms: texture_depth_multisampled_2d;
@group(0) @binding(1) var dst0: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8)
fn hiz_init(@builtin(global_invocation_id) gid: vec3u) {
    let dims = textureDimensions(src_ms);
    if (gid.x >= dims.x || gid.y >= dims.y) {
        return;
    }
    var d = 0.0;
    for (var s = 0; s < i32(textureNumSamples(src_ms)); s++) {
        d = max(d, textureLoad(src_ms, vec2i(gid.xy), s));
    }
    textureStore(dst0, vec2i(gid.xy), vec4f(d, 0.0, 0.0, 0.0));
}

@group(0) @binding(2) var src_mip: texture_2d<f32>;
@group(0) @binding(3) var dst_mip: texture_storage_2d<r32float, write>;

@compute @workgroup_size(8, 8)
fn hiz_down(@builtin(global_invocation_id) gid: vec3u) {
    let dd = textureDimensions(dst_mip);
    if (gid.x >= dd.x || gid.y >= dd.y) {
        return;
    }
    let sd = vec2i(textureDimensions(src_mip));
    let base = vec2i(gid.xy) * 2;
    // odd source sizes: the last texel also covers the extra row/column
    let ex = select(1, 2, gid.x == dd.x - 1u && (sd.x & 1) == 1);
    let ey = select(1, 2, gid.y == dd.y - 1u && (sd.y & 1) == 1);
    var d = 0.0;
    for (var y = 0; y <= ey; y++) {
        for (var x = 0; x <= ex; x++) {
            d = max(d, textureLoad(src_mip, min(base + vec2i(x, y), sd - 1), 0).r);
        }
    }
    textureStore(dst_mip, vec2i(gid.xy), vec4f(d, 0.0, 0.0, 0.0));
}
