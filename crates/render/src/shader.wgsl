// Flat vertex-colour geometry (placeholder boxes, grass, pitch) and the sky.
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

@fragment
fn fs(i: VOut, @builtin(front_facing) front: bool) -> @location(0) vec4f {
    let n = select(-normalize(i.nrm), normalize(i.nrm), front);
    let l = normalize(g.sun_dir.xyz);
    let ndl = max(dot(n, l), 0.0);
    var c = i.col * (g.sun_col.rgb * ndl * sun_shadow(i.world, n) + ambient(n));
    return vec4f(tonemap(haze(c, i.world)), 1.0);
}

struct SkyOut {
    @builtin(position) clip: vec4f,
    @location(0) ndc: vec2f,
};

@vertex
fn vs_sky(@builtin(vertex_index) i: u32) -> SkyOut {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    var o: SkyOut;
    o.clip = vec4f(p, 1.0, 1.0);
    o.ndc = p;
    return o;
}

@fragment
fn fs_sky(i: SkyOut) -> @location(0) vec4f {
    let far = g.inv_view_proj * vec4f(i.ndc, 1.0, 1.0);
    let near = g.inv_view_proj * vec4f(i.ndc, 0.0, 1.0);
    let d = normalize(far.xyz / far.w - near.xyz / near.w);
    return vec4f(tonemap(sky_dir(d, true)), 1.0);
}
