// Textured PBR (metallic-roughness) with per-instance transforms. Prepended with common.wgsl.
@group(1) @binding(0) var t_base: texture_2d<f32>;
@group(1) @binding(1) var t_orm: texture_2d<f32>;
@group(1) @binding(2) var t_nrm: texture_2d<f32>;
@group(1) @binding(3) var samp: sampler;
// screen-space reflection history: last frame's display colour, a = linear distance from its eye
@group(2) @binding(0) var t_hist: texture_2d<f32>;

struct VIn {
    @location(0) pos: vec3f,
    @location(1) nrm: vec3f,
    @location(2) uv: vec2f,
    @location(3) tan: vec4f,
    // edge wear: position on the flat face (u, v) and its size (w, h), metres; zero: use curvature
    @location(10) edge: vec4f,
    @location(4) m0: vec4f,
    @location(5) m1: vec4f,
    @location(6) m2: vec4f,
    @location(7) m3: vec4f,
    // rgb: tint, a: uv scale (1 for models; 1/tile size for world-UV city surfaces)
    @location(8) tint: vec4f,
    // x: mode (0 lit, 3 one-sided glazing, 4 emissive at strength y; legacy 1/2 procedural windows);
    //    lit surfaces carry the edge-wear kind in tenths (0.1 wood, 0.2 metal, 0.3 stone, 0.4 dusty masonry)
    // lit: y = indoor ambient factor (0 = outdoor), z = indoor lamp light
    @location(9) params: vec4f,
};

struct VOut {
    @builtin(position) @invariant clip: vec4f,
    @location(0) world: vec3f,
    @location(1) nrm: vec3f,
    @location(2) uv: vec2f,
    @location(3) tan: vec4f,
    @location(4) tint: vec3f,
    @location(5) uvm: vec2f,
    @location(6) params: vec4f,
    @location(7) edge: vec4f,
    // material slot (bindless path)
    @location(8) @interpolate(flat) mat: u32,
};

// the culled path: instances fetched by id from the registry (see cull.rs)
struct Inst { model: mat4x4f, tint: vec4f, params: vec4f };
@group(3) @binding(0) var<storage, read> insts: array<Inst>;
@group(3) @binding(1) var<storage, read> inst_mat: array<u32>;

struct VIdIn {
    @location(0) pos: vec3f,
    @location(1) nrm: vec3f,
    @location(2) uv: vec2f,
    @location(3) tan: vec4f,
    @location(10) edge: vec4f,
    @location(4) iid: u32,
};

@vertex
fn vs_ids(v: VIdIn) -> VOut {
    let I = insts[v.iid];
    var w: VIn;
    w.pos = v.pos;
    w.nrm = v.nrm;
    w.uv = v.uv;
    w.tan = v.tan;
    w.edge = v.edge;
    w.m0 = I.model[0];
    w.m1 = I.model[1];
    w.m2 = I.model[2];
    w.m3 = I.model[3];
    w.tint = I.tint;
    w.params = I.params;
    var o = vs_body(w);
    o.mat = inst_mat[v.iid];
    return o;
}

@vertex
fn vs(v: VIn) -> VOut {
    return vs_body(v);
}

fn vs_body(v: VIn) -> VOut {
    let m = mat4x4f(v.m0, v.m1, v.m2, v.m3);
    let w = m * vec4f(v.pos, 1.0);
    var o: VOut;
    o.clip = g.view_proj * w;
    o.world = w.xyz;
    o.nrm = normalize((m * vec4f(v.nrm, 0.0)).xyz);
    o.tan = vec4f(normalize((m * vec4f(v.tan.xyz, 0.0)).xyz), v.tan.w);
    o.uv = v.uv * v.tint.a;
    o.uvm = v.uv;
    o.tint = v.tint.rgb;
    o.params = v.params;
    o.edge = v.edge;
    return o;
}

fn hash2(p: vec2f) -> f32 {
    return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453);
}

fn vnoise(p: vec2f) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash2(i), hash2(i + vec2f(1.0, 0.0)), u.x), mix(hash2(i + vec2f(0.0, 1.0)), hash2(i + vec2f(1.0, 1.0)), u.x), u.y);
}

// Large-scale tonal variation in world space: breaks up visible texture tiling and uniform "CG" surfaces.
fn macro_var(w: vec3f, n: vec3f) -> f32 {
    let p = select(vec2f(w.x + w.z, w.y), w.xz, abs(n.y) > 0.7);
    return 0.55 * vnoise(p / 1.4) + 0.45 * vnoise(p / 7.0 + 13.1);
}

fn hash3(p: vec3f) -> f32 {
    var q = fract(p * 0.1031);
    q += dot(q, q.zyx + 31.32);
    return fract((q.x + q.y) * q.z);
}

// Value noise on a 3D lattice: the same grain on a floor, a wall or a slope.
fn grain3(p: vec3f) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(mix(hash3(i), hash3(i + vec3f(1.0, 0.0, 0.0)), u.x), mix(hash3(i + vec3f(0.0, 1.0, 0.0)), hash3(i + vec3f(1.0, 1.0, 0.0)), u.x), u.y);
    let b = mix(mix(hash3(i + vec3f(0.0, 0.0, 1.0)), hash3(i + vec3f(1.0, 0.0, 1.0)), u.x), mix(hash3(i + vec3f(0.0, 1.0, 1.0)), hash3(i + vec3f(1.0, 1.0, 1.0)), u.x), u.y);
    return mix(a, b, u.z);
}

fn noise3(p: vec3f) -> f32 {
    return (vnoise(p.xy) + vnoise(p.yz + 17.3) + vnoise(p.xz + 41.7)) / 3.0;
}

// Weathered edges (like a texturing tool's edge-wear generator): a mask from the distance to the face's edges
// (or, for meshes without face data, from curvature), broken up by noise into chips and rubbed patches.
// Returns the wear amount 0..1.
fn edge_wear(i: VOut, nv: vec3f, kind: i32) -> f32 {
    let width = select(select(0.06, 0.018, kind == 2), 0.04, kind == 3);
    var m = 0.0;
    if (i.edge.z > 0.0 && i.edge.w > 0.0) {
        let d = min(min(i.edge.x, i.edge.z - i.edge.x), min(i.edge.y, i.edge.w - i.edge.y));
        m = 1.0 - smoothstep(0.0, width, d);
    } else {
        // curvature: how fast the normal turns per metre on screen
        let k = length(fwidth(nv)) / max(length(fwidth(i.world)), 1e-4);
        m = smoothstep(4.0, 30.0, k);
    }
    let w = i.world;
    let breakup = noise3(w * 7.0) * 0.5 + noise3(w * 31.0) * 0.3 + noise3(w * 110.0) * 0.2;
    // broad patches where a corner has been handled or knocked more; elsewhere the edge is barely touched
    let patches = smoothstep(0.3, 0.75, noise3(w * 0.9 + 3.1));
    return smoothstep(0.5, 0.72, m * m * (0.3 + breakup) * (0.45 + patches * 0.9));
}

fn ggx(n: vec3f, v: vec3f, l: vec3f, rough: f32, f0: vec3f) -> vec3f {
    let h = normalize(v + l);
    let ndl = max(dot(n, l), 0.0);
    let ndv = max(dot(n, v), 1e-4);
    let ndh = max(dot(n, h), 0.0);
    let a2 = rough * rough * rough * rough;
    let d = a2 / (PI * pow(ndh * ndh * (a2 - 1.0) + 1.0, 2.0));
    let k = (rough + 1.0) * (rough + 1.0) / 8.0;
    let gs = (ndl / (ndl * (1.0 - k) + k)) * (ndv / (ndv * (1.0 - k) + k));
    let f = f0 + (1.0 - f0) * pow(1.0 - max(dot(h, v), 0.0), 5.0);
    return d * gs * f / (4.0 * ndl * ndv + 1e-4);
}

// Inverse of the display tonemap (per channel), back to scene radiance.
fn untonemap(a: vec3f) -> vec3f {
    let y = clamp(a, vec3f(0.0), vec3f(0.98));
    let qa = 2.43 * y - 2.51;
    let qb = 0.59 * y - 0.03;
    let qc = 0.14 * y;
    let x = (-qb - sqrt(max(qb * qb - 4.0 * qa * qc, vec3f(0.0)))) / (2.0 * qa);
    return x / (max(g.sun_dir.w, 1e-3) * g.white.rgb);
}

fn hist_at(p: vec3f) -> vec4f {
    // xy: history texel, z: distance of p from the history eye, w: 0 when off screen
    let clip = g.prev_vp * vec4f(p, 1.0);
    if (clip.w <= 0.01) {
        return vec4f(0.0);
    }
    let ndc = clip.xy / clip.w;
    let uv = vec2f(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if (any(uv < vec2f(0.0)) || any(uv >= vec2f(1.0))) {
        return vec4f(0.0);
    }
    return vec4f(uv, distance(p, g.prev_eye.xyz), 1.0);
}

// Screen-space reflection (after Ruinbound's ssrTrace): march the reflection ray with growing steps against the
// history depth; a sample just behind the stored surface (within a thickness) is a hit, refined by bisection.
// rgb: reflected radiance, a: confidence (0 on a miss, fading at the screen edges and with distance).
fn ssr(world: vec3f, r: vec3f, rough: f32) -> vec4f {
    if (g.prev_eye.w < 0.5) {
        return vec4f(0.0);
    }
    let dimv = vec2f(textureDimensions(t_hist));
    // with temporal accumulation: half the steps, growing faster (similar reach), one tap at the hit
    let taa = g.white.w > 0.0;
    let steps = select(40, 14, taa);
    let grow = select(1.13, 1.4, taa);
    var step = select(0.05, 0.06, taa);
    var t = 0.08;
    var lo = 0.0;
    for (var i = 0; i < steps; i++) {
        let h = hist_at(world + r * t);
        if (h.w < 0.5) {
            break;
        }
        let stored = textureLoad(t_hist, vec2i(h.xy * dimv), 0).a;
        let gap = h.z - stored;
        if (gap > 0.02 + h.z * 0.002 && gap < max(0.3, step * 2.5)) {
            var a = lo;
            var b = t;
            for (var k = 0; k < 5; k++) {
                let m = (a + b) * 0.5;
                let hm = hist_at(world + r * m);
                if (hm.z - textureLoad(t_hist, vec2i(hm.xy * dimv), 0).a > 0.0) {
                    b = m;
                } else {
                    a = m;
                }
            }
            let hb = hist_at(world + r * b);
            // rougher surfaces: average a few texels around the hit (a blurrier reflection)
            let rad = clamp(rough * 120.0 * b / max(hb.z, 1.0), 1.0, 20.0);
            var col = vec3f(0.0);
            let taps = select(9, 1, taa);
            for (var k = 0; k < taps; k++) {
                // centre + two rings
                let ring = select(1.0, 0.5, k < 5);
                let o = select(vec2f(0.0), vec2f(cos(f32(k) * 1.5708 + ring), sin(f32(k) * 1.5708 + ring)) * rad * ring, k > 0);
                col += textureLoad(t_hist, vec2i(clamp(hb.xy * dimv + o, vec2f(0.0), dimv - 1.0)), 0).rgb;
            }
            let uv = hb.xy;
            let edge = smoothstep(0.0, 0.07, min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y)));
            let far = 1.0 - smoothstep(20.0, 40.0, b);
            return vec4f(untonemap(col / f32(taps)), edge * far);
        }
        lo = t;
        step *= grow;
        t += step;
    }
    return vec4f(0.0);
}

// depth prepass: position, uv and mode only (the tiler stores far less per vertex than the shading pass's
// outputs), the same cut-outs as `fs`, no shading (each pixel is then shaded once)
struct PreOut {
    @builtin(position) @invariant clip: vec4f,
    @location(0) uv: vec2f,
    @location(1) mode: f32,
    @location(2) @interpolate(flat) mat: u32,
};

@vertex
fn vs_ids_pre(@location(0) pos: vec3f, @location(2) uv: vec2f, @location(4) iid: u32) -> PreOut {
    let I = insts[iid];
    var o: PreOut;
    o.clip = g.view_proj * (I.model * vec4f(pos, 1.0));
    o.uv = uv * I.tint.a;
    o.mode = I.params.x;
    o.mat = inst_mat[iid];
    return o;
}

@fragment
fn fs_pre(i: PreOut, @builtin(front_facing) front: bool) -> @location(0) vec4f {
    if (textureSample(t_base, samp, i.uv).a < 0.5 || (i.mode > 2.5 && !front)) {
        discard;
    }
    return vec4f(0.0);
}

@fragment
fn fs(i: VOut, @builtin(front_facing) front: bool) -> @location(0) vec4f {
    let texel = textureSample(t_base, samp, i.uv);
    // params.x == 3: one-sided glazing, only visible from outside
    if (texel.a < 0.5 || (i.params.x > 2.5 && !front)) {
        discard;
    }
    var base = texel.rgb * i.tint;
    // (taken here, where every fragment passes, for the detail lookups below)
    let dgx = dpdx(i.world.xz);
    let dgy = dpdy(i.world.xz);
    if (i.params.x > 3.5) {
        // params.w: the material carries the shared detail texture - real turf and clay, tiled over the ground,
        // changing only the brightness of what the photograph shows. Chosen by the photograph's own colour.
        var detailed = 0.0;
        if (i.params.w > 0.0) {
            let up = smoothstep(0.55, 0.85, normalize(i.nrm).y);
            let turf = clamp((base.g - max(base.r, base.b)) * 8.0, 0.0, 1.0) * up;
            let clay = clamp((base.r - max(base.g, base.b) * 1.08) * 6.0, 0.0, 1.0) * up;
            let p = i.world.xz;
            // two lookups at unrelated sizes and angles, so the tile's repeat does not show across a field
            let q = vec2f(p.x * 0.819 - p.y * 0.574, p.x * 0.574 + p.y * 0.819);
            // (the same picture three times over: blades up close, tufts at a few paces, mottling beyond)
            let t1 = textureSampleGrad(t_nrm, samp, p / 1.2, dgx / 1.2, dgy / 1.2).rg;
            let t2 = textureSampleGrad(t_nrm, samp, q / 3.1 + vec2f(0.37, 0.11), dgx / 3.1, dgy / 3.1).rg;
            let t3 = textureSampleGrad(t_nrm, samp, q.yx / 8.3 + vec2f(0.71, 0.53), dgx / 8.3, dgy / 8.3).rg;
            let d = ((t1 - 0.5) * 1.0 + (t2 - 0.5) * 0.9 + (t3 - 0.5) * 0.7) * 2.0 * i.params.w;
            base *= 1.0 + d.r * turf + d.g * clay;
            detailed = max(turf, clay);
        }
        // a photograph from the air runs out of detail a few metres from the eye: fine grain (stronger on grass)
        // keeps a surface from turning to smooth mush under the nose. params.z: its strength.
        if (i.params.z > 0.0) {
            let d = distance(i.world, g.eye.xyz);
            let green = clamp((base.g - max(base.r, base.b)) * 8.0, 0.0, 1.0);
            let w = i.world + vec3f(400.0, 50.0, 400.0);
            let a = (grain3(w * 37.0) - 0.5) * (1.0 - smoothstep(6.0, 18.0, d));
            let b = (grain3(w * 131.0) - 0.5) * (1.0 - smoothstep(2.0, 7.0, d));
            let c = (grain3(w * 9.0) - 0.5) * (1.0 - smoothstep(12.0, 40.0, d));
            base *= 1.0 + (a * 0.8 + b * 0.8 + c * 0.35) * mix(0.14, 0.34, green) * i.params.z * (1.0 - detailed * 0.8);
        }
        return vec4f(tonemap(haze(base * i.params.y, i.world)), 1.0);
    }
    var orm = textureSample(t_orm, samp, i.uv).rgb;
    // normal maps may be BC5 (RG only): rebuild Z
    let txy = textureSample(t_nrm, samp, i.uv).xy * 2.0 - 1.0;
    let tn = vec3f(txy, sqrt(max(1.0 - dot(txy, txy), 0.0)));
    // two-sided foliage: light the back face with the flipped normal
    let nv = select(-normalize(i.nrm), normalize(i.nrm), front);
    let tv = normalize(i.tan.xyz - nv * dot(nv, i.tan.xyz));
    let bv = cross(nv, tv) * i.tan.w;
    var n = normalize(tv * tn.x + bv * tn.y + nv * tn.z);

    var glass = 0.0;
    base *= mix(0.84, 1.1, macro_var(i.world, nv));
    if (i.params.x > 0.5 && i.params.x < 2.5 && abs(nv.y) < 0.5) {
        // window grid in world metres (u along the wall, v = -height); the material shows on frames and spandrels.
        // mode 1: curtain wall (wide glazing); mode 2: punched windows in masonry
        let cell = vec2f(i.uvm.x / i.params.z, -i.uvm.y / i.params.y);
        let f = fract(cell);
        let id = floor(cell);
        let curtain = i.params.x < 1.5;
        let x0 = select(0.22, 0.05, curtain);
        let y0 = select(0.30, 0.20, curtain);
        let y1 = select(0.86, 0.95, curtain);
        let inside = f.x > x0 && f.x < 1.0 - x0 && f.y > y0 && f.y < y1 && id.y >= 1.0;
        if (inside) {
            glass = 1.0;
            let r = hash2(id + i.params.w);
            // mostly dark glass, some with blinds or interiors; recessed: shaded under the head and on one jamb
            base = mix(vec3f(0.018, 0.022, 0.026), vec3f(0.12, 0.105, 0.085), step(0.85, r));
            let head = smoothstep(y1 - 0.12, y1, f.y);
            let jamb = select(0.0, smoothstep(x0 + 0.06, x0, f.x), !curtain);
            orm = vec3f(1.0 - 0.6 * max(head, jamb) * select(1.0, 0.3, curtain), mix(0.05, 0.4, step(0.85, r)), 0.0);
            n = nv;
        } else if (!curtain && f.x > x0 - 0.035 && f.x < 1.0 - x0 + 0.035 && f.y > y0 - 0.05 && f.y < y1 + 0.02 && id.y >= 1.0) {
            // stone surround and sill, close to the wall colour
            base = mix(base, vec3f(0.42, 0.40, 0.36), 0.35);
        }
    }

    // weathered edges
    let kind = i32(round(i.params.x * 10.0));
    if (i.params.x < 0.5 && kind == 4) {
        // dusty old masonry: faded towards a warm grey, with a pale film that collects in the rough mortar
        let lum = dot(base, vec3f(0.3, 0.59, 0.11));
        base = mix(base, vec3f(lum) * vec3f(1.12, 1.0, 0.9), 0.42) + vec3f(0.012, 0.011, 0.01) * orm.g;
    } else if (i.params.x < 0.5 && kind > 0) {
        let wr = edge_wear(i, nv, kind);
        if (kind == 1) {
            // timber: rubbed raw, lighter and drier
            let lum = dot(base, vec3f(0.3, 0.59, 0.11));
            base = mix(base, mix(vec3f(lum), base, 0.55) * 2.4 + vec3f(0.06, 0.042, 0.025), wr * 0.85);
            orm.g = mix(orm.g, 0.85, wr);
        } else if (kind == 2) {
            // paint and mill scale worn through to bright steel
            base = mix(base, vec3f(0.56, 0.56, 0.55), wr);
            orm = vec3f(orm.r, mix(orm.g, 0.32, wr), mix(orm.b, 1.0, wr));
        } else {
            // chipped arrises: fresher, paler material
            base = mix(base, base * 1.35 + vec3f(0.03), wr * 0.6);
        }
    }
    let ao = orm.r;
    // params.w (lit surfaces): roughness scale, for oiled timber and polished floors
    let rough = clamp(orm.g * select(1.0, i.params.w, i.params.x < 0.5 && i.params.w > 0.0), 0.04, 1.0);
    let metal = orm.b;
    let v = normalize(g.eye.xyz - i.world);
    let l = normalize(g.sun_dir.xyz);
    let ndl = max(dot(n, l), 0.0);
    let ndv = max(dot(n, v), 1e-4);
    let f0 = mix(vec3f(0.04), base, metal);
    let fv = f0 + (max(vec3f(1.0 - rough), f0) - f0) * pow(1.0 - ndv, 5.0);
    let kd = (1.0 - fv) * (1.0 - metal);
    let sun = g.sun_col.rgb * sun_shadow(i.world, nv) * ndl;
    // very rough surfaces (grass, soil, broom-finished concrete, chalked plastic) are not smooth facets: their
    // own texture shadows the grazing sheen a microfacet lobe predicts, which otherwise whitens a whole field or
    // street seen toward the sun
    let sheen = 1.0 - 0.85 * smoothstep(0.55, 0.9, rough) * (1.0 - metal);
    var c = (kd * base + ggx(n, v, l, rough, f0) * PI * sheen) * sun;
    c += kd * local_lights(i.world, n, v, base, rough) * ao;
    // inside a baked probe: diffuse light and glossy reflections come from the probe (split-sum PBR)
    let pw = probe_weight(i.world);
    let indoor = i.params.x < 2.5 && i.params.y > 0.0;
    var amb = kd * base * ambient(n) * ao * select(1.0, i.params.y, indoor);
    if (indoor) {
        // ceiling lamps: soft light from above, some bounce onto walls and ceilings
        amb += kd * base * ao * i.params.z * vec3f(1.0, 0.94, 0.84) * (0.35 + 0.65 * max(n.y, 0.0));
    }
    var spec_env = vec3f(0.0);
    if (pw > 0.0 && g.misc.z > 0.5) {
        // probe debug: perfect mirror
        return vec4f(tonemap(probe_sample(i.world, reflect(-v, normalize(i.nrm)), 0.0)), 1.0);
    }
    if (pw > 0.0) {
        let maxlod = g.probe_max.w;
        let rdir = reflect(-v, n);
        let irr = probe_sample(i.world, n, maxlod);
        var spec = probe_sample(i.world, rdir, sqrt(rough) * maxlod);
        // screen-space reflection where the ray finds something on screen; the probe is the fallback
        if (rough < 0.65) {
            // trace along a mostly geometric normal: the normal map would scatter the rays into noise
            let sr = ssr(i.world, reflect(-v, normalize(mix(n, nv, 0.75))), rough);
            spec = mix(spec, sr.rgb, sr.a * (1.0 - smoothstep(0.3, 0.65, rough)));
        }
        amb = mix(amb, kd * base * irr * ao, pw);
        spec_env = spec * env_brdf(f0, rough, ndv) * ao * pw;
    }
    // flood-fill GI: indirect diffuse from the voxel light field replaces the flat indoor ambient
    // only interiors use the GI field (outdoor pixels skip its fetches)
    var gi = vec4f(0.0);
    if (indoor || g.gi_dim.w > 1.5) {
        gi = gi_sample(i.world, nv);
    }
    let giw = select(0.0, gi.a, indoor);
    if (giw > 0.0) {
        amb = mix(amb, kd * base * gi.rgb * ao, giw);
    }
    if (g.gi_dim.w > 1.5) {
        // GI debug: the raw light field
        return vec4f(tonemap(gi.rgb), 1.0);
    }
    c += amb + spec_env;
    // environment reflection: the sky, strongest on glass and smooth surfaces
    // near-horizontal reflections see the buildings across the street, not sky
    let rd = reflect(-v, n);
    var refl = sky_dir(rd, false) * mix(0.32, 1.0, smoothstep(0.02, 0.45, rd.y));
    if (pw < 1.0 && rough < 0.5) {
        // outdoors: buildings and street on screen reflect in glossy surfaces, the sky elsewhere
        let sr = ssr(i.world, rd, rough);
        refl = mix(refl, sr.rgb, sr.a);
    }
    c += fv * refl * mix((1.0 - rough) * 0.5, 0.65, glass) * ao * (1.0 - pw);
    // no aerial haze indoors
    return vec4f(tonemap(select(haze(c, i.world), c, indoor)), 1.0);
}

// Clear glazing: sky/sun reflection by Fresnel, slight green-grey tint, the interior shows through.
@fragment
fn fs_glass(i: VOut, @builtin(front_facing) front: bool) -> @location(0) vec4f {
    let nv = select(-normalize(i.nrm), normalize(i.nrm), front);
    let v = normalize(g.eye.xyz - i.world);
    let ndv = clamp(dot(nv, v), 0.0, 1.0);
    let f = 0.04 + 0.96 * pow(1.0 - ndv, 5.0);
    let rd = reflect(-v, nv);
    let l = normalize(g.sun_dir.xyz);
    var refl = sky_dir(rd, false) * mix(0.32, 1.0, smoothstep(0.02, 0.45, rd.y)) + g.sun_col.rgb * pow(max(dot(rd, l), 0.0), 900.0) * 6.0;
    // tint absorbs a little; dust adds a faint lit film
    var amb = ambient(nv);
    // seen from inside a room, glass mirrors the room (the probe) and is lit by its bounce light, not the open sky
    let eye_in = g.probe_min.w > 0.5 && all(g.eye.xyz >= g.probe_min.xyz) && all(g.eye.xyz <= g.probe_max.xyz);
    if (eye_in) {
        let pw = probe_weight(i.world);
        refl = mix(refl, probe_sample(i.world, rd, 1.0), pw);
        let gi = gi_sample(i.world, nv);
        amb = mix(amb, gi.rgb, gi.a);
    }
    let film = i.tint * 0.025 * (amb + g.sun_col.rgb * max(dot(nv, l), 0.0) * sun_shadow(i.world, nv));
    // base opacity: windows ~0.3; clear glassware passes a lower one in params.y
    let base_a = select(0.30, i.params.y, i.params.y > 0.0);
    let a = clamp(base_a + f * 0.7, 0.0, 1.0);
    // tinted glass reflects in its own colour, darker than the sky it mirrors (a clear tint keeps it near white)
    let tref = mix(vec3f(1.0), clamp(i.tint * 1.8, vec3f(0.0), vec3f(1.0)), 0.6);
    let c = refl * mix(0.45, 1.0, f) * tref + film;
    // lit glassware (params.z): the glass glows with its lamp's light, added on top of what shows through it
    let glow = tonemap(i.tint * i.params.z) * (1.0 - a);
    return vec4f(tonemap(haze(c, i.world)) * a + glow, a);
}
