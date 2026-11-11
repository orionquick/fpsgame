// Temporal super resolution, ported from Ruinbound's TSR (crates/client/src/main.rs: TSR_PRE_FRAG, TSR_FRAG, rcas; tuning in
// docs/tsr-breakdown.md). The scene renders at a lower resolution with a Halton sub-pixel jitter; a prepass builds a YCoCg clip box
// (Reinhard-compressed colour, variance-bounded by the neighbourhood min/max, narrower under motion) and nearest-depth motion per
// render texel; the resolve reprojects the output-resolution history (5-tap Catmull-Rom), clips it, weights the current frame's 2x2
// samples by their distance to the pixel (Gaussian, narrowing as confidence builds), and stores the accumulated confidence in alpha
// (capped, shorter while moving: long history ghosts); the present sharpens with AMD RCAS. The world is static, so motion comes
// from depth and the two cameras: no velocity target.
struct U {
    inv_vp: mat4x4f,  // current jittered view-projection, inverted (render uv + depth -> world)
    cur_vp: mat4x4f,  // current, unjittered
    prev_vp: mat4x4f, // previous frame, unjittered
    rsz: vec4f,       // render size, 1 / render size
    osz: vec4f,       // output (history) size, 1 / size
    jit: vec4f,       // xy: jitter in render texels (texel i samples the unjittered point i + 0.5 - jit), z: 1 = no usable history
    k: vec4f,         // clip sigma still, clip sigma moving, render px/frame for full motion, history cap still
    k2: vec4f,        // history cap moving, converged kernel (output px), fresh kernel (render px), RCAS sharpness
};

@group(0) @binding(0) var<uniform> u: U;

struct V {
    @builtin(position) pos: vec4f,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> V {
    let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    var o: V;
    o.pos = vec4f(p, 0.0, 1.0);
    return o;
}

fn tm(c: vec3f) -> vec3f {
    return c / (1.0 + max(c.r, max(c.g, c.b)));
}
fn itm(c: vec3f) -> vec3f {
    return c / max(1.0 - max(c.r, max(c.g, c.b)), 1e-4);
}
fn rgb2ycocg(c: vec3f) -> vec3f {
    return vec3f(0.25 * c.r + 0.5 * c.g + 0.25 * c.b, 0.5 * c.r - 0.5 * c.b, -0.25 * c.r + 0.5 * c.g - 0.25 * c.b);
}
fn ycocg2rgb(c: vec3f) -> vec3f {
    let t = c.x - c.z;
    return vec3f(t + c.y, c.x + c.z, t - c.y);
}
fn to_uv(m: mat4x4f, p: vec3f) -> vec3f {
    let c = m * vec4f(p, 1.0);
    return vec3f(c.x / c.w * 0.5 + 0.5, 0.5 - c.y / c.w * 0.5, c.w);
}

// ---- prepass, one fragment per render texel ----
@group(0) @binding(1) var t_color: texture_2d<f32>;
@group(0) @binding(2) var t_depth: texture_depth_2d;

struct PreOut {
    @location(0) s0: vec4f, // clip-box low (YCoCg), motion x (uv)
    @location(1) s1: vec4f, // clip-box high, motion y
};

@fragment
fn fs_pre(v: V) -> PreOut {
    let ic = vec2i(floor(v.pos.xy));
    let rmax = vec2i(u.rsz.xy) - 1;
    var m1 = vec3f(0.0);
    var m2 = vec3f(0.0);
    var nmn = vec3f(1e9);
    var nmx = vec3f(-1e9);
    var zmin = 2.0;
    var zq = ic;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let q = clamp(ic + vec2i(x, y), vec2i(0), rmax);
            let c = max(textureLoad(t_color, q, 0).rgb, vec3f(0.0));
            let yc = rgb2ycocg(tm(c));
            m1 += yc;
            m2 += yc * yc;
            nmn = min(nmn, yc);
            nmx = max(nmx, yc);
            let z = textureLoad(t_depth, q, 0);
            if (z < zmin) {
                zmin = z;
                zq = q;
            }
        }
    }
    let mu = m1 / 9.0;
    let sig = sqrt(max(m2 / 9.0 - mu * mu, vec3f(0.0)));
    // motion of the nearest surface in the 3x3 (dilation): its world point, then where both cameras see it
    let zuv = (vec2f(zq) + 0.5) * u.rsz.zw;
    let w = u.inv_vp * vec4f(zuv.x * 2.0 - 1.0, 1.0 - zuv.y * 2.0, min(zmin, 1.0), 1.0);
    let p = w.xyz / w.w;
    let cur = to_uv(u.cur_vp, p);
    let prev = to_uv(u.prev_vp, p);
    let mv = select(vec2f(2.0), cur.xy - prev.xy, prev.z > 1e-4); // behind the old camera: history out of bounds
    // clip box narrows from k.x (still) toward k.y (moving): stale history only ghosts under motion
    let g = mix(u.k.x, u.k.y, clamp(length(mv * u.rsz.xy) / u.k.z, 0.0, 1.0));
    var o: PreOut;
    o.s0 = vec4f(max(mu - g * sig, nmn), mv.x);
    o.s1 = vec4f(min(mu + g * sig, nmx), mv.y);
    return o;
}

// ---- resolve, at output resolution ----
@group(0) @binding(3) var stats0: texture_2d<f32>;
@group(0) @binding(4) var stats1: texture_2d<f32>;
@group(0) @binding(5) var history: texture_2d<f32>;
@group(0) @binding(6) var samp: sampler;

fn clip_aabb(mn: vec3f, mx: vec3f, q: vec3f) -> vec3f {
    let ctr = 0.5 * (mx + mn);
    let ext = 0.5 * (mx - mn) + 1e-5;
    let v = q - ctr;
    let a = abs(v / ext);
    let ma = max(a.x, max(a.y, a.z));
    return select(q, ctr + v / ma, ma > 1.0);
}

// Catmull-Rom in 5 bilinear taps (corner taps dropped, the rest renormalised)
fn history_cr(q: vec2f) -> vec4f {
    let sp = q * u.osz.xy;
    let c = floor(sp - 0.5) + 0.5;
    let f = sp - c;
    let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    let w3 = f * f * (-0.5 + 0.5 * f);
    let w12 = w1 + w2;
    let t12 = (c + w2 / w12) * u.osz.zw;
    let t0 = (c - 1.0) * u.osz.zw;
    let t3 = (c + 2.0) * u.osz.zw;
    let r = textureSampleLevel(history, samp, vec2f(t12.x, t0.y), 0.0) * (w12.x * w0.y)
        + textureSampleLevel(history, samp, vec2f(t0.x, t12.y), 0.0) * (w0.x * w12.y)
        + textureSampleLevel(history, samp, t12, 0.0) * (w12.x * w12.y)
        + textureSampleLevel(history, samp, vec2f(t3.x, t12.y), 0.0) * (w3.x * w12.y)
        + textureSampleLevel(history, samp, vec2f(t12.x, t3.y), 0.0) * (w12.x * w3.y);
    return r / (w12.x * w0.y + w0.x * w12.y + w12.x * w12.y + w3.x * w12.y + w12.x * w3.y);
}

@fragment
fn fs_resolve(v: V) -> @location(0) vec4f {
    let uv = v.pos.xy * u.osz.zw;
    // this pixel in the jittered render grid, where texel i samples at i + 0.5
    let q = uv * u.rsz.xy + u.jit.xy;
    let bq = q - 0.5;
    let f = fract(bq);
    let rmax = u.rsz.xy - 1.0;
    let ci = vec2i(clamp(floor(q), vec2f(0.0), rmax));
    let s0 = textureLoad(stats0, ci, 0);
    let s1 = textureLoad(stats1, ci, 0);
    let mv = vec2f(s0.a, s1.a);
    let puv = uv - mv;
    let inside = all(puv >= vec2f(0.0)) && all(puv <= vec2f(1.0));
    let valid = select(0.0, 1.0 - u.jit.z, inside);
    let h = history_cr(puv);
    let hy = clip_aabb(s0.rgb, s1.rgb, rgb2ycocg(tm(max(h.rgb, vec3f(0.0)))));
    // history length: k.w frames still, k2.x moving
    let acc_cap = mix(u.k.w, u.k2.x, clamp(length(mv * u.rsz.xy) / u.k.z, 0.0, 1.0));
    let acc = valid * clamp(h.a, 0.0, acc_cap);
    // current frame: the 2x2 render samples around the pixel, Gaussian by distance (wide in render px with no history: a smooth
    // upsample; narrow in output px once history holds ~4 frames: a still image converges to output-res detail)
    let ratio = u.osz.x * u.rsz.z;
    let k = mix(u.k2.z, u.k2.y * ratio * ratio, clamp(acc * 0.25, 0.0, 1.0));
    var ct = vec3f(0.0);
    var sw = 0.0;
    for (var j = 0; j < 2; j++) {
        for (var i = 0; i < 2; i++) {
            let o = vec2f(f32(i), f32(j));
            let d = o - f;
            let wt = exp(-k * dot(d, d));
            ct += wt * tm(max(textureLoad(t_color, vec2i(clamp(floor(bq) + o, vec2f(0.0), rmax)), 0).rgb, vec3f(0.0)));
            sw += wt;
        }
    }
    ct /= sw;
    sw = min(sw, 1.0);
    let a = sw / max(acc + sw, 1e-4);
    let acc_out = min(acc + sw, u.k.w);
    return vec4f(itm(ycocg2rgb(mix(hy, rgb2ycocg(ct), a))), acc_out);
}

// ---- present: AMD RCAS (FSR1) in perceptual space ----
@group(0) @binding(7) var resolved: texture_2d<f32>;

fn gam(c: vec3f) -> vec3f {
    return pow(max(c, vec3f(0.0)), vec3f(1.0 / 2.2));
}

@fragment
fn fs_present(v: V) -> @location(0) vec4f {
    let p = vec2i(v.pos.xy);
    let s = vec2i(textureDimensions(resolved)) - 1;
    let b = gam(textureLoad(resolved, clamp(p - vec2i(0, 1), vec2i(0), s), 0).rgb);
    let d = gam(textureLoad(resolved, clamp(p - vec2i(1, 0), vec2i(0), s), 0).rgb);
    let e = gam(textureLoad(resolved, p, 0).rgb);
    let f = gam(textureLoad(resolved, clamp(p + vec2i(1, 0), vec2i(0), s), 0).rgb);
    let h = gam(textureLoad(resolved, clamp(p + vec2i(0, 1), vec2i(0), s), 0).rgb);
    let mn = min(min(b, d), min(f, h));
    let mx = max(max(b, d), max(f, h));
    let lobe_rgb = max(-min(mn, e) / (4.0 * mx + 1e-5), (1.0 - max(mx, e)) / (4.0 * mn - 4.0 - 1e-5));
    let lobe = max(-0.1875, min(max(lobe_rgb.r, max(lobe_rgb.g, lobe_rgb.b)), 0.0)) * u.k2.w;
    let c = (lobe * (b + d + f + h) + e) / (4.0 * lobe + 1.0);
    // back to linear for the sRGB target
    return vec4f(pow(max(c, vec3f(0.0)), vec3f(2.2)), 1.0);
}
