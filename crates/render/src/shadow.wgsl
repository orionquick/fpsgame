// Shadow-map passes (one uniform per cascade). Textured casters are alpha-tested.
struct ShadowU {
    light_vp: mat4x4f,
};

@group(0) @binding(0) var<uniform> su: ShadowU;
@group(1) @binding(0) var t_base: texture_2d<f32>;
@group(1) @binding(3) var samp: sampler;

@vertex
fn vs_flat(@location(0) pos: vec3f) -> @builtin(position) vec4f {
    return su.light_vp * vec4f(pos, 1.0);
}

struct TIn {
    @location(0) pos: vec3f,
    @location(2) uv: vec2f,
    @location(4) m0: vec4f,
    @location(5) m1: vec4f,
    @location(6) m2: vec4f,
    @location(7) m3: vec4f,
    @location(8) tint: vec4f,
};

struct TOut {
    @builtin(position) clip: vec4f,
    @location(0) uv: vec2f,
};

@vertex
fn vs_tex(v: TIn) -> TOut {
    var o: TOut;
    o.clip = su.light_vp * (mat4x4f(v.m0, v.m1, v.m2, v.m3) * vec4f(v.pos, 1.0));
    o.uv = v.uv * v.tint.a;
    return o;
}

struct Inst { model: mat4x4f, tint: vec4f, params: vec4f };
@group(2) @binding(0) var<storage, read> insts: array<Inst>;

@vertex
fn vs_tex_ids(@location(0) pos: vec3f, @location(2) uv: vec2f, @location(4) iid: u32) -> TOut {
    let I = insts[iid];
    var o: TOut;
    o.clip = su.light_vp * (I.model * vec4f(pos, 1.0));
    o.uv = uv * I.tint.a;
    return o;
}

@fragment
fn fs_tex(i: TOut) {
    // a sharp mip: seen from the sun, foliage would otherwise pick a blurred level whose alpha cuts out blobs
    if (textureSampleLevel(t_base, samp, i.uv, 1.0).a < 0.5) {
        discard;
    }
}
