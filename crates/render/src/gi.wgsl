// Voxel flood-fill global illumination on a player-centred grid, in two parts:
//
// 1. `skylight`: direct sky light. For every air cell and each of the six axis directions, rays in a cone march
//    through the voxels; those that escape the grid see the sky. Light pouring down through a window lands on
//    the floor where it should.
// 2. `propagate` (run many times): bounce light. Every cell carries light travelling along each axis (the six
//    directions are stacked along z: direction d of cell p lives at (p.x, p.y, p.z + d * nz)). It flies straight
//    through air, spreading into the side neighbours (a cone) and turning a little; solid cells (walls, floors,
//    beams) gather what arrives (bounce + direct sky) and send albedo x that out of their open faces, plus what
//    they reflect of the sun and lamps (injected by the CPU).
//
// The shaders read direct sky + bounce.
//
// vox:  rgb albedo, a = 1 solid
// emit: rgb sun/lamp light a solid cell reflects
@group(0) @binding(0) var vox: texture_3d<f32>;
@group(0) @binding(1) var emit: texture_3d<f32>;
@group(0) @binding(2) var src: texture_3d<f32>;
@group(0) @binding(3) var dst: texture_storage_3d<rgba16float, write>;
@group(0) @binding(4) var sky: texture_3d<f32>;

const DIRS = array<vec3i, 6>(vec3i(1, 0, 0), vec3i(-1, 0, 0), vec3i(0, 1, 0), vec3i(0, -1, 0), vec3i(0, 0, 1), vec3i(0, 0, -1));
// the four axes perpendicular to each direction
const SIDES = array<vec4i, 6>(vec4i(2, 3, 4, 5), vec4i(2, 3, 4, 5), vec4i(0, 1, 4, 5), vec4i(0, 1, 4, 5), vec4i(0, 1, 2, 3), vec4i(0, 1, 2, 3));
const STRAIGHT: f32 = 0.44;
const SPREAD: f32 = 0.14;
// share of the light arriving along the perpendicular axes that turns into each direction (stands in for the
// diagonal paths six axes can't carry)
const TURN: f32 = 0.05;
// just off a surface: half of the reflected light spreads sideways (Lambertian hemisphere)
const NEAR_TURN: f32 = 0.5;

// sky as in common.wgsl (zenith/horizon, ambient strength 1.15); below the horizon: sunlit street and facades
const ZENITH: vec3f = vec3f(0.16, 0.34, 0.78);
const HORIZON: vec3f = vec3f(0.62, 0.74, 0.90);
const STREET: vec3f = vec3f(0.75, 0.70, 0.62);

var<private> dim: vec3i;

fn inside(p: vec3i) -> bool {
    return all(p >= vec3i(0)) && all(p < dim);
}

fn solid(p: vec3i) -> bool {
    return textureLoad(vox, clamp(p, vec3i(0), dim - 1), 0).a > 0.5;
}

fn stack(p: vec3i, d: i32) -> vec3i {
    let c = clamp(p, vec3i(0), dim - 1);
    return vec3i(c.x, c.y, c.z + d * dim.z);
}

// Bounce light leaving cell q along direction d, times whether that slot transmits (alpha: 1 for air and for a
// solid cell's open faces, 0 for a closed face's slot, which only holds the light arriving there for the shaders).
// Outside the grid: the nearest edge cell's.
fn light(q: vec3i, d: i32) -> vec3f {
    let l = textureLoad(src, stack(q, d), 0);
    return l.rgb * l.a;
}

// Bounce light arriving at p travelling along d: mostly from straight behind, some from the cells beside that.
fn arriving(p: vec3i, d: i32) -> vec3f {
    var dirs = DIRS;
    var sides = SIDES;
    let back = p - dirs[d];
    let s = sides[d];
    return light(back, d) * STRAIGHT + (light(back + dirs[s.x], d) + light(back + dirs[s.y], d) + light(back + dirs[s.z], d) + light(back + dirs[s.w], d)) * SPREAD;
}

// Share of the light travelling along e at air cell p that turns into the four perpendicular directions.
fn turn(p: vec3i, e: i32) -> f32 {
    var dirs = DIRS;
    return select(TURN, NEAR_TURN, solid(p - dirs[e]));
}

fn split(id: vec3u) -> vec4i {
    let full = vec3i(textureDimensions(dst));
    dim = vec3i(full.x, full.y, full.z / 6);
    if (any(vec3i(id) >= full)) {
        return vec4i(-1);
    }
    return vec4i(vec3i(id).xy, i32(id.z) % dim.z, i32(id.z) / dim.z);
}

@compute @workgroup_size(4, 4, 4)
fn propagate(@builtin(global_invocation_id) id: vec3u) {
    let s4 = split(id);
    if (s4.x < 0) {
        return;
    }
    let p = s4.xyz;
    let d = s4.w;
    var dirs = DIRS;
    let v = textureLoad(vox, p, 0);
    if (v.a < 0.5) {
        // air: carried along, with some turning between this cell's directions (last step's light). Light that
        // has just left a surface (the cell behind it along that direction is solid) turns much more: a matte
        // surface scatters into its whole hemisphere, so a sunlit floor also lights the columns beside it.
        var sides = SIDES;
        let s = sides[d];
        var side = vec3f(0.0);
        for (var k = 0; k < 4; k++) {
            let e = s[k];
            side += textureLoad(src, stack(p, e), 0).rgb * turn(p, e) * 0.25;
        }
        textureStore(dst, vec3i(id), vec4f(arriving(p, d) * (1.0 - turn(p, d)) + side, 1.0));
        return;
    }
    // solid: light leaves through the open faces only. A closed face's slot holds the light arriving at the
    // cell along that direction instead, marked as not transmitting, so the shaders read what reaches a surface
    // even where it sits inside its voxel.
    if (!inside(p + dirs[d]) || solid(p + dirs[d])) {
        textureStore(dst, vec3i(id), vec4f(arriving(p, d), 0.0));
        return;
    }
    var gathered = vec3f(0.0);
    var open = 0.0;
    for (var k = 0; k < 6; k++) {
        gathered += arriving(p, k);
        let back = p - dirs[k];
        if (inside(back) && !solid(back)) {
            // direct sky reaching the air cell in front of this face carries on onto it
            gathered += textureLoad(sky, stack(back, k), 0).rgb;
        }
        if (!solid(p + dirs[k])) {
            open += 1.0;
        }
    }
    let e = textureLoad(emit, p, 0).rgb;
    textureStore(dst, vec3i(id), vec4f(e + v.rgb * gathered / max(open, 1.0), 1.0));
}

// Bounce light starts dark.
@compute @workgroup_size(4, 4, 4)
fn init(@builtin(global_invocation_id) id: vec3u) {
    if (split(id).x < 0) {
        return;
    }
    textureStore(dst, vec3i(id), vec4f(0.0));
}

fn sky_radiance(r: vec3f) -> vec3f {
    if (r.y < 0.0) {
        return STREET;
    }
    let c = mix(ZENITH, HORIZON, pow(1.0 - r.y, 5.0)) * 1.15;
    // hazy coastal sky: what windows see is much whiter than the zenith blue
    return mix(c, vec3f(dot(c, vec3f(0.2126, 0.7152, 0.0722))), 0.4);
}

const RAYS: i32 = 16;

// Direct sky light at each air cell travelling along each axis: the mean over a cone of directions around the
// opposite axis (where that light comes from) of the sky seen along it, or nothing where the voxels block it.
@compute @workgroup_size(4, 4, 4)
fn skylight(@builtin(global_invocation_id) id: vec3u) {
    let s4 = split(id);
    if (s4.x < 0) {
        return;
    }
    let p = s4.xyz;
    let d = s4.w;
    var dirs = DIRS;
    // a solid cell takes the sky arriving at its face from the air cell in front of it
    var start = vec3f(p) + 0.5;
    if (solid(p)) {
        if (solid(p - dirs[d])) {
            textureStore(dst, vec3i(id), vec4f(0.0));
            return;
        }
        start -= vec3f(dirs[d]);
    }
    let axis = -vec3f(dirs[d]);
    let t1 = normalize(cross(axis, select(vec3f(0.0, 1.0, 0.0), vec3f(1.0, 0.0, 0.0), abs(axis.y) > 0.5)));
    let t2 = cross(axis, t1);
    var sum = vec3f(0.0);
    for (var k = 0; k < RAYS; k++) {
        // golden-angle spiral over the cap within 60 degrees of the axis
        let ct = 1.0 - (f32(k) + 0.5) / f32(RAYS) * 0.5;
        let st = sqrt(1.0 - ct * ct);
        let ph = f32(k) * 2.39996;
        let r = axis * ct + (t1 * cos(ph) + t2 * sin(ph)) * st;
        var q = start;
        var hit = false;
        for (var i = 0; i < 128; i++) {
            q += r * 0.5;
            let c = vec3i(floor(q));
            if (!inside(c)) {
                break;
            }
            if (textureLoad(vox, c, 0).a > 0.5) {
                hit = true;
                break;
            }
        }
        if (!hit) {
            sum += sky_radiance(r);
        }
    }
    textureStore(dst, vec3i(id), vec4f(sum / f32(RAYS), 1.0));
}
