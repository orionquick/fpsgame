// Screen-space ambient occlusion from the main pass depth, then a composite that darkens creases and contacts.
// Prepended with common.wgsl (globals at group 0).
@group(1) @binding(0) var t_depth: texture_depth_multisampled_2d;
@group(1) @binding(1) var t_scene: texture_2d<f32>;
@group(1) @binding(2) var t_ao: texture_2d<f32>;

struct FsIn {
    @builtin(position) pos: vec4f,
};

@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> FsIn {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    var o: FsIn;
    o.pos = vec4f(p, 0.0, 1.0);
    return o;
}

fn dims() -> vec2f {
    return vec2f(textureDimensions(t_depth));
}

fn world_at(px: vec2i) -> vec4f {
    let s = dims();
    let q = clamp(px, vec2i(0), vec2i(s) - 1);
    let d = textureLoad(t_depth, q, 0);
    let uv = (vec2f(q) + 0.5) / s;
    let ndc = vec2f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let w = g.inv_view_proj * vec4f(ndc, d, 1.0);
    return vec4f(w.xyz / w.w, d);
}

fn ign(p: vec2f) -> f32 {
    return fract(52.9829189 * fract(dot(p, vec2f(0.06711056, 0.00583715))));
}

@fragment
fn fs_ao(i: FsIn) -> @location(0) vec4f {
    // with temporal accumulation the AO target is half size
    let px = vec2i(i.pos.xy) * select(1, 2, g.white.w > 0.0);
    let c = world_at(px);
    if (c.w >= 1.0) {
        return vec4f(1.0);
    }
    let p = c.xyz;
    // normal from the flatter of the two neighbours on each axis (avoids smearing across edges)
    let xr = world_at(px + vec2i(1, 0)).xyz - p;
    let xl = p - world_at(px - vec2i(1, 0)).xyz;
    let yd = world_at(px + vec2i(0, 1)).xyz - p;
    let yu = p - world_at(px - vec2i(0, 1)).xyz;
    let dx = select(xl, xr, dot(xr, xr) < dot(xl, xl));
    let dy = select(yu, yd, dot(yd, yd) < dot(yu, yu));
    var n = normalize(cross(dy, dx));
    let to_eye = g.eye.xyz - p;
    if (dot(n, to_eye) < 0.0) {
        n = -n;
    }
    let dist = length(to_eye);
    let radius = mix(0.55, 2.5, smoothstep(15.0, 120.0, dist));
    // tangent frame rotated per pixel
    // with temporal super resolution (g.white.w > 0): fewer samples (fixed per pixel)
    let taa = g.white.w > 0.0;
    let a = ign(i.pos.xy) * 6.2831853;
    let up = select(vec3f(0.0, 1.0, 0.0), vec3f(1.0, 0.0, 0.0), abs(n.y) > 0.9);
    let t0 = normalize(cross(up, n));
    let b0 = cross(n, t0);
    let t = t0 * cos(a) + b0 * sin(a);
    let b = cross(n, t);
    let s = dims();
    var occ = 0.0;
    let count = select(14, 6, taa);
    for (var k = 0; k < count; k++) {
        let fk = f32(k);
        // spiral of directions over the hemisphere, samples denser near the centre
        let ang = fk * 2.39996;
        let h = (fk + 0.5) / f32(count);
        let r = sqrt(1.0 - h * h);
        let dir = t * (cos(ang) * r) + b * (sin(ang) * r) + n * h;
        let scale = mix(0.15, 1.0, fract(fk * 0.618 + ign(i.pos.yx)));
        let sp = p + dir * radius * scale + n * 0.02;
        let clip = g.view_proj * vec4f(sp, 1.0);
        if (clip.w <= 0.0) {
            continue;
        }
        let ndc = clip.xy / clip.w;
        let uv = vec2f(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
        if (any(uv < vec2f(0.0)) || any(uv > vec2f(1.0))) {
            continue;
        }
        let q = world_at(vec2i(uv * s));
        if (q.w >= 1.0) {
            continue;
        }
        let ds = length(sp - g.eye.xyz);
        let dq = length(q.xyz - g.eye.xyz);
        let gap = ds - dq;
        if (gap > 0.02) {
            occ += 1.0 - smoothstep(radius * 0.5, radius * 2.0, gap);
        }
    }
    var ao = 1.0 - occ / f32(count);
    ao = mix(ao, 1.0, smoothstep(150.0, 400.0, dist));
    return vec4f(ao, ao, ao, 1.0);
}

struct CompositeOut {
    @location(0) color: vec4f,
    // reflection history for the next frame: display colour, linear distance from the eye (sky: far)
    @location(1) hist: vec4f,
};

@fragment
fn fs_composite(i: FsIn) -> CompositeOut {
    let px = vec2i(i.pos.xy);
    let col = textureLoad(t_scene, px, 0);
    let s = vec2i(dims());
    let d0 = world_at(px).w;
    let ao_div = max(i32(round(dims().x / f32(textureDimensions(t_ao).x))), 1);
    // depth-aware 5x5 blur of the noisy AO
    var sum = 0.0;
    var wsum = 0.0;
    let rb = select(2, 1, g.white.w > 0.0);
    for (var y = -rb; y <= rb; y++) {
        for (var x = -rb; x <= rb; x++) {
            let q = clamp(px + vec2i(x, y) * 1, vec2i(0), s - 1);
            let dq = textureLoad(t_depth, q, 0);
            let w = 1.0 / (1.0 + abs(dq - d0) * 4000.0);
            sum += textureLoad(t_ao, q / ao_div, 0).r * w;
            wsum += w;
        }
    }
    let ao = sum / max(wsum, 1e-4);
    // multiply in display space; keep a floor so lit faces don't go muddy
    let k = mix(0.25, 1.0, ao);
    let w = world_at(px);
    var o: CompositeOut;
    o.color = vec4f(col.rgb * k, col.a);
    o.hist = vec4f(col.rgb * k, select(length(w.xyz - g.eye.xyz), 1e5, w.w >= 1.0));
    return o;
}
