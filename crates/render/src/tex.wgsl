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
    // x: mode (0 lit, 3 one-sided glazing, 4 emissive at strength y; legacy 1/2 procedural windows)
    // lit: y = indoor ambient factor (0 = outdoor), z = indoor lamp light
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
    // params.x == 3: one-sided glazing, only visible from outside
    if (texel.a < 0.5 || (i.params.x > 2.5 && !front)) {
        discard;
    }
    var base = texel.rgb * i.tint;
    if (i.params.x > 3.5) {
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
    var c = (kd * base + ggx(n, v, l, rough, f0) * PI) * sun;
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
        let spec = probe_sample(i.world, rdir, sqrt(rough) * maxlod);
        amb = mix(amb, kd * base * irr * ao, pw);
        spec_env = spec * env_brdf(f0, rough, ndv) * ao * pw;
    }
    c += amb + spec_env;
    // environment reflection: the sky, strongest on glass and smooth surfaces
    // near-horizontal reflections see the buildings across the street, not sky
    let rd = reflect(-v, n);
    let refl = sky_dir(rd, false) * mix(0.32, 1.0, smoothstep(0.02, 0.45, rd.y));
    c += fv * refl * mix((1.0 - rough) * 0.5, 0.65, glass) * ao * (1.0 - pw);
    return vec4f(tonemap(haze(c, i.world)), 1.0);
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
    let refl = sky_dir(rd, false) * mix(0.32, 1.0, smoothstep(0.02, 0.45, rd.y)) + g.sun_col.rgb * pow(max(dot(rd, l), 0.0), 900.0) * 6.0;
    // tint absorbs a little; dust adds a faint lit film
    let film = i.tint * 0.025 * (ambient(nv) + g.sun_col.rgb * max(dot(nv, l), 0.0) * sun_shadow(i.world, nv));
    let a = clamp(0.30 + f * 0.7, 0.0, 1.0);
    let c = refl * mix(0.45, 1.0, f) + film;
    return vec4f(tonemap(haze(c, i.world)) * a, a);
}
