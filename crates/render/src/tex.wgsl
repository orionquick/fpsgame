// Textured PBR (metallic-roughness) with per-instance transforms. Prepended with common.wgsl.
@group(1) @binding(0) var t_base: texture_2d<f32>;
@group(1) @binding(1) var t_orm: texture_2d<f32>;
@group(1) @binding(2) var t_nrm: texture_2d<f32>;
@group(1) @binding(3) var samp: sampler;

struct VIn {
    @location(0) pos: vec3f,
    @location(1) nrm: vec3f,
    @location(2) uv: vec2f,
    @location(3) tan: vec4f,
    @location(4) m0: vec4f,
    @location(5) m1: vec4f,
    @location(6) m2: vec4f,
    @location(7) m3: vec4f,
    // rgb: tint, a: uv scale (1 for models; 1/tile size for world-UV city surfaces)
    @location(8) tint: vec4f,
    // x: mode (0 plain, 1 curtain-wall windows over world-UV walls), y: floor height, z: window width, w: seed
    @location(9) params: vec4f,
};

struct VOut {
    @builtin(position) clip: vec4f,
    @location(0) world: vec3f,
    @location(1) nrm: vec3f,
    @location(2) uv: vec2f,
    @location(3) tan: vec4f,
    @location(4) tint: vec3f,
    @location(5) uvm: vec2f,
    @location(6) params: vec4f,
};

@vertex
fn vs(v: VIn) -> VOut {
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
    return o;
}

fn hash2(p: vec2f) -> f32 {
    return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453);
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

@fragment
fn fs(i: VOut, @builtin(front_facing) front: bool) -> @location(0) vec4f {
    let texel = textureSample(t_base, samp, i.uv);
    if (texel.a < 0.5) {
        discard;
    }
    var base = texel.rgb * i.tint;
    var orm = textureSample(t_orm, samp, i.uv).rgb;
    let tn = textureSample(t_nrm, samp, i.uv).xyz * 2.0 - 1.0;
    // two-sided foliage: light the back face with the flipped normal
    let nv = select(-normalize(i.nrm), normalize(i.nrm), front);
    let tv = normalize(i.tan.xyz - nv * dot(nv, i.tan.xyz));
    let bv = cross(nv, tv) * i.tan.w;
    var n = normalize(tv * tn.x + bv * tn.y + nv * tn.z);

    var glass = 0.0;
    if (i.params.x > 0.5 && abs(nv.y) < 0.5) {
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

    let ao = orm.r;
    let rough = clamp(orm.g, 0.05, 1.0);
    let metal = orm.b;
    let v = normalize(g.eye.xyz - i.world);
    let l = normalize(g.sun_dir.xyz);
    let ndl = max(dot(n, l), 0.0);
    let ndv = max(dot(n, v), 1e-4);
    let f0 = mix(vec3f(0.04), base, metal);
    let fv = f0 + (max(vec3f(1.0 - rough), f0) - f0) * pow(1.0 - ndv, 5.0);
    let kd = (1.0 - fv) * (1.0 - metal);
    let sun = g.sun_col.rgb * sun_shadow(i.world, nv) * ndl;
    var c = (kd * base + ggx(n, v, l, rough, f0) * PI) * sun;
    c += kd * base * ambient(n) * ao;
    // environment reflection: the sky, strongest on glass and smooth surfaces
    // near-horizontal reflections see the buildings across the street, not sky
    let rd = reflect(-v, n);
    let refl = sky_dir(rd, false) * mix(0.32, 1.0, smoothstep(0.02, 0.45, rd.y));
    c += fv * refl * mix((1.0 - rough) * 0.5, 0.65, glass) * ao;
    return vec4f(tonemap(haze(c, i.world)), 1.0);
}
