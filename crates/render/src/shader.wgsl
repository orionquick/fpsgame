struct Globals {
    view_proj: mat4x4f,
    light_vp: mat4x4f,
    sun_dir: vec4f,    // xyz: toward the sun
    sun_col: vec4f,
    sky_col: vec4f,
    ground_col: vec4f,
    eye: vec4f,        // xyz camera position, w: fog density (0 = none)
    fog_col: vec4f,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_samp: sampler_comparison;

struct VIn {
    @location(0) pos: vec3f,
    @location(1) nrm: vec3f,
    @location(2) col: vec3f,
};

struct VOut {
    @builtin(position) clip: vec4f,
    @location(0) world: vec3f,
    @location(1) nrm: vec3f,
    @location(2) col: vec3f,
};

@vertex
fn vs(v: VIn) -> VOut {
    var o: VOut;
    o.clip = g.view_proj * vec4f(v.pos, 1.0);
    o.world = v.pos;
    o.nrm = v.nrm;
    o.col = v.col;
    return o;
}

fn shadow(world: vec3f, n: vec3f) -> f32 {
    let p = g.light_vp * vec4f(world + n * 0.9, 1.0);
    let ndc = p.xyz / p.w;
    let uv = ndc.xy * vec2f(0.5, -0.5) + 0.5;
    if (any(uv < vec2f(0.0)) || any(uv > vec2f(1.0)) || ndc.z > 1.0) {
        return 1.0;
    }
    let texel = 1.0 / vec2f(textureDimensions(shadow_map));
    var s = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            s += textureSampleCompareLevel(shadow_map, shadow_samp, uv + vec2f(f32(x), f32(y)) * texel, ndc.z - 0.0015);
        }
    }
    return s / 9.0;
}

fn aces(x: vec3f) -> vec3f {
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3f(0.0), vec3f(1.0));
}

@fragment
fn fs(i: VOut) -> @location(0) vec4f {
    var n = normalize(i.nrm);
    let to_eye = g.eye.xyz - i.world;
    let sun = normalize(g.sun_dir.xyz);
    let ndl = max(dot(n, sun), 0.0);
    let sh = shadow(i.world, n);
    let hemi = mix(g.ground_col.rgb, g.sky_col.rgb, n.y * 0.5 + 0.5);
    var c = i.col * (g.sun_col.rgb * ndl * sh + hemi);
    let fog = 1.0 - exp(-length(to_eye) * g.eye.w);
    c = mix(c, g.fog_col.rgb, fog);
    return vec4f(aces(c), 1.0);
}

@vertex
fn vs_shadow(v: VIn) -> @builtin(position) vec4f {
    return g.light_vp * vec4f(v.pos, 1.0);
}
