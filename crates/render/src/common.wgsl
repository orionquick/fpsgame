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
    misc: vec4f,         // x: ambient strength, y: local light count, w: probe-bake radiance encode (0 = display)
    probe_min: vec4f,    // w: probe enabled
    probe_max: vec4f,    // w: max mip
    probe_pos: vec4f,    // w: radiance decode scale
    gi_min: vec4f,       // GI volume origin, w: cell size
    gi_dim: vec4f,       // GI volume cells, w: enabled
    prev_vp: mat4x4f,    // previous frame (screen-space reflection history)
    prev_eye: vec4f,     // w: SSR enabled
    white: vec4f,        // camera white balance
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var shadow_far: texture_depth_2d;
@group(0) @binding(2) var shadow_samp: sampler_comparison;
@group(0) @binding(3) var shadow_near: texture_depth_2d;

struct PointLight {
    pos: vec3f,
    radius: f32,
    color: vec3f,
    pad: f32,
};
@group(0) @binding(4) var<uniform> lights: array<PointLight, 128>;
@group(0) @binding(5) var probe_tex: texture_cube<f32>;
@group(0) @binding(6) var probe_samp: sampler;
@group(0) @binding(7) var gi_tex: texture_3d<f32>;
@group(0) @binding(8) var gi_sky: texture_3d<f32>;

// Indirect diffuse light from the flood-fill GI volume: the light arriving at a surface with normal n, from the
// six travel directions (each stacked along z in the volume), sampled just off the surface so a floor reads the
// air above it. a: weight (fades out over the volume's last cells).
fn gi_sample(world: vec3f, n: vec3f) -> vec4f {
    if (g.gi_dim.w < 0.5) {
        return vec4f(0.0);
    }
    let cell = g.gi_min.w;
    let p = (world + n * cell * 0.6 - g.gi_min.xyz) / cell;
    let edge = min(p, g.gi_dim.xyz - p);
    let w = clamp((min(min(edge.x, edge.y), edge.z) - 1.0) / 3.0, 0.0, 1.0);
    if (w <= 0.0) {
        return vec4f(0.0);
    }
    // light travelling along +x hits surfaces facing -x, etc. Each direction's value is the mean over a 60-degree
    // cone, so the side directions also carry slanting light that reaches the surface: a soft lobe (0.6 for the
    // facing direction, 0.1 for each side one; a uniform field gives back its value).
    let c = array<f32, 6>(-n.x, n.x, -n.y, n.y, -n.z, n.z);
    var wts: array<f32, 6>;
    for (var d = 0; d < 6; d++) {
        wts[d] = select(0.0, 0.1 + 0.5 * c[d] * c[d], c[d] > -0.05);
    }
    let nz = g.gi_dim.z;
    // four taps a cell apart across the surface soften the voxel structure
    let t1 = normalize(select(vec3f(1.0, 0.0, 0.0), vec3f(0.0, 0.0, 1.0), abs(n.x) > 0.7) - n * dot(n, select(vec3f(1.0, 0.0, 0.0), vec3f(0.0, 0.0, 1.0), abs(n.x) > 0.7)));
    let t2 = cross(n, t1);
    let taps = array<vec3f, 4>(t1 + t2, t1 - t2, -t1 + t2, -t1 - t2);
    var e = vec3f(0.0);
    for (var d = 0; d < 6; d++) {
        if (wts[d] > 0.0) {
            for (var k = 0; k < 4; k++) {
                let q = p + taps[k] * 0.6;
                let uvw = vec3f(q.x / g.gi_dim.x, q.y / g.gi_dim.y, (clamp(q.z, 0.5, nz - 0.5) + f32(d) * nz) / (6.0 * nz));
                e += (textureSampleLevel(gi_tex, probe_samp, uvw, 0.0).rgb + textureSampleLevel(gi_sky, probe_samp, uvw, 0.0).rgb) * wts[d] * 0.25;
            }
        }
    }
    // weights of an axis-aligned normal sum to 1; diagonal normals gather from two or three directions
    return vec4f(e / (wts[0] + wts[1] + wts[2] + wts[3] + wts[4] + wts[5]), w);
}

// How much a point is inside the probe box (fades over the last half metre).
fn probe_weight(world: vec3f) -> f32 {
    if (g.probe_min.w < 0.5) {
        return 0.0;
    }
    // full weight up to and on the box faces (floors, outer walls), fading out over 0.4 m beyond them
    let d = min(world - g.probe_min.xyz, g.probe_max.xyz - world);
    return clamp((min(min(d.x, d.y), d.z) + 0.45) / 0.4, 0.0, 1.0);
}

// Box-projected lookup: where the ray from `world` along `r` meets the room box, seen from the capture point.
fn probe_sample(world: vec3f, r: vec3f, lod: f32) -> vec3f {
    let rr = select(r, vec3f(1e-4), abs(r) < vec3f(1e-4));
    let t1 = (g.probe_max.xyz - world) / rr;
    let t2 = (g.probe_min.xyz - world) / rr;
    let tf = max(t1, t2);
    let t = max(min(min(tf.x, tf.y), tf.z), 0.0);
    let dir = world + r * t - g.probe_pos.xyz;
    return textureSampleLevel(probe_tex, probe_samp, dir, lod).rgb * g.probe_pos.w;
}

// Split-sum environment BRDF, analytic fit (Karis, mobile).
fn env_brdf(f0: vec3f, rough: f32, ndv: f32) -> vec3f {
    let c0 = vec4f(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4f(1.0, 0.0425, 1.04, -0.04);
    let r = rough * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * ndv)) * r.x + r.y;
    let ab = vec2f(-1.04, 1.04) * a004 + r.zw;
    return f0 * ab.x + ab.y;
}

// Diffuse + a little specular from the local lights (no shadows; ranges are kept short).
fn local_lights(world: vec3f, n: vec3f, v: vec3f, base: vec3f, rough: f32) -> vec3f {
    var c = vec3f(0.0);
    let count = u32(g.misc.y);
    for (var i = 0u; i < count; i++) {
        let lt = lights[i];
        let d = lt.pos - world;
        let dist = length(d);
        if (dist > lt.radius) {
            continue;
        }
        let l = d / max(dist, 1e-3);
        let ndl = max(dot(n, l), 0.0);
        let x = dist / lt.radius;
        let fall = (1.0 - x * x) * (1.0 - x * x) / (1.0 + dist * dist);
        let h = normalize(l + v);
        let spec = pow(max(dot(n, h), 0.0), mix(64.0, 4.0, rough)) * (1.0 - rough) * 0.5;
        c += lt.color * fall * (base * ndl + vec3f(spec) * ndl);
    }
    return c;
}

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
    // probe bakes store linear radiance (scaled), not display colour
    if (g.misc.w > 0.0) {
        return clamp(c * g.misc.w, vec3f(0.0), vec3f(1.0));
    }
    let x = c * g.sun_dir.w * g.white.rgb;
    let a = clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3f(0.0), vec3f(1.0));
    let l = dot(a, vec3f(0.2126, 0.7152, 0.0722));
    return clamp(mix(vec3f(l), a, 1.02), vec3f(0.0), vec3f(1.0));
}
