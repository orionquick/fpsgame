// Reflection probe mip filter: a wide tent over the level above (approximates the rougher GGX lobe of each mip).
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

struct V {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> V {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    var o: V;
    o.pos = vec4f(p, 0.0, 1.0);
    o.uv = vec2f(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return o;
}

@fragment
fn fs(v: V) -> @location(0) vec4f {
    let px = 1.0 / vec2f(textureDimensions(src));
    var c = vec4f(0.0);
    var w = 0.0;
    for (var y = -2; y <= 2; y++) {
        for (var x = -2; x <= 2; x++) {
            let k = (3.0 - abs(f32(x))) * (3.0 - abs(f32(y)));
            c += textureSampleLevel(src, samp, v.uv + vec2f(f32(x), f32(y)) * px * 1.5, 0.0) * k;
            w += k;
        }
    }
    return c / w;
}

// plain bilinear copy (upscaling the internal-resolution frame to the window)
@fragment
fn fs_blit(v: V) -> @location(0) vec4f {
    return textureSampleLevel(src, samp, v.uv, 0.0);
}
