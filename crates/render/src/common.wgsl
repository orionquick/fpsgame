// Shared lighting: globals, sky, two-cascade sun shadows, aerial haze, tonemapping.
struct Globals {
    view_proj: mat4x4f,
    light_far: mat4x4f,
    light_near: mat4x4f,
    inv_view_proj: mat4x4f,
    sun_dir: vec4f,      // xyz toward the sun, w: exposure
    sun_col: vec4f,
    sky_zenith: vec4f,
    sky_horizon: vec4f,
    ground_col: vec4f,
    eye: vec4f,          // xyz camera, w: haze density per metre
    misc: vec4f,         // x: ambient strength
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var shadow_far: texture_depth_2d;
@group(0) @binding(2) var shadow_samp: sampler_comparison;
@group(0) @binding(3) var shadow_near: texture_depth_2d;

const PI: f32 = 3.14159265;

// Sky radiance along a direction; `disc` adds the sun disc.
fn sky_dir(d: vec3f, disc: bool) -> vec3f {
    let up = d.y;
    let t = pow(1.0 - clamp(up, 0.0, 1.0), 5.0);
    var c = mix(g.sky_zenith.rgb, g.sky_horizon.rgb, t);
    if (up < 0.0) {
        c = mix(g.sky_horizon.rgb, g.ground_col.rgb * 1.2, clamp(-up * 3.0, 0.0, 1.0));
    }
    let s = max(dot(d, normalize(g.sun_dir.xyz)), 0.0);
    c += g.sun_col.rgb * (pow(s, 10.0) * 0.08 + pow(s, 200.0) * 0.25);
    if (disc) {
        c += g.sun_col.rgb * smoothstep(0.9997, 0.99985, s) * 40.0;
    }
    return c;
}

fn pcf(map: texture_depth_2d, uv: vec2f, z: f32, bias: f32) -> f32 {
    let texel = 1.0 / vec2f(textureDimensions(map));
    var s = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            s += textureSampleCompareLevel(map, shadow_samp, uv + vec2f(f32(x), f32(y)) * texel * 1.5, z - bias);
        }
    }
    return s / 9.0;
}

fn sun_shadow(world: vec3f, n: vec3f) -> f32 {
    let pn = g.light_near * vec4f(world + n * 0.06, 1.0);
    let un = pn.xy * vec2f(0.5, -0.5) + 0.5;
    if (all(un > vec2f(0.03)) && all(un < vec2f(0.97)) && pn.z < 1.0) {
        return pcf(shadow_near, un, pn.z, 0.0004);
    }
    let pf = g.light_far * vec4f(world + n * 0.9, 1.0);
    let uf = pf.xy * vec2f(0.5, -0.5) + 0.5;
    if (any(uf < vec2f(0.0)) || any(uf > vec2f(1.0)) || pf.z > 1.0) {
        return 1.0;
    }
    return pcf(shadow_far, uf, pf.z, 0.0015);
}

fn ambient(n: vec3f) -> vec3f {
    let sky_amb = mix(g.sky_horizon.rgb, g.sky_zenith.rgb, 0.5);
    return mix(g.ground_col.rgb, sky_amb, n.y * 0.5 + 0.5) * g.misc.x;
}

fn haze(c: vec3f, world: vec3f) -> vec3f {
    let v = world - g.eye.xyz;
    let dist = length(v);
    if (dist < 1e-3 || g.eye.w <= 0.0) {
        return c;
    }
    let dir = v / dist;
    // denser near the ground
    let f = 1.0 - exp(-dist * g.eye.w * exp(-max(world.y, 0.0) / 400.0));
    let hc = sky_dir(vec3f(dir.x, max(dir.y, 0.0) * 0.5, dir.z), false);
    return mix(c, hc, f);
}

fn tonemap(c: vec3f) -> vec3f {
    let x = c * g.sun_dir.w;
    let a = clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3f(0.0), vec3f(1.0));
    let l = dot(a, vec3f(0.2126, 0.7152, 0.0722));
    return clamp(mix(vec3f(l), a, 1.02), vec3f(0.0), vec3f(1.0));
}
