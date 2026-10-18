// noshape viewport shaders. Reverse-Z depth (1 = near, 0 = far).

struct Globals {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,   // xyz eye, w = 1 perspective / 0 orthographic
    camera_fwd: vec4<f32>,   // xyz forward
    camera_up: vec4<f32>,
    viewport: vec4<f32>,     // width px, height px, device pixel ratio, pick widening px
    clip: vec4<f32>,         // plane normal xyz, d
    flags: vec4<u32>,        // x = number of clip planes (0..3)
    clip2: vec4<f32>,
    clip3: vec4<f32>,
    bg_top: vec4<f32>,
    bg_bottom: vec4<f32>,
    hover: vec4<f32>,
    select: vec4<f32>,
    accent: vec4<f32>,
    error: vec4<f32>,
};

struct Inst {
    model: mat4x4<f32>,
    normal_m: mat4x4<f32>,
    color: vec4<f32>,
    params: vec4<u32>,       // x = per-primitive color, y = ignore clip, z = pickable, w = pick base
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var<storage, read> states: array<u32>;
@group(1) @binding(0) var<uniform> inst: Inst;

const HOVER: u32 = 1u;
const SELECTED: u32 = 2u;
const HIDDEN: u32 = 4u;
const ACCENT: u32 = 8u;
const ERROR: u32 = 16u;
const DIM: u32 = 32u;

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3(0.055)) / 1.055, vec3(2.4));
    return select(hi, lo, c <= vec3(0.04045));
}

// Vertex pick ids are local (1-based); the instance adds its pick base.
fn global_pick(local: u32) -> u32 {
    if (local == 0u) { return 0u; }
    return local + inst.params.w;
}

fn state_of(id: u32) -> u32 {
    if (id == 0u || id >= arrayLength(&states)) {
        return 0u;
    }
    return states[id];
}

fn clipped(world: vec3<f32>) -> bool {
    if (g.flags.x == 0u || (inst.params.y & 1u) != 0u) { return false; }
    var c = dot(world, g.clip.xyz) > g.clip.w;
    if (g.flags.x > 1u) { c = c && dot(world, g.clip2.xyz) > g.clip2.w; }
    if (g.flags.x > 2u) { c = c && dot(world, g.clip3.xyz) > g.clip3.w; }
    return c;
}

fn highlight(base: vec3<f32>, st: u32) -> vec3<f32> {
    var c = base;
    if ((st & ERROR) != 0u) { c = mix(c, g.error.rgb, 0.7); }
    if ((st & ACCENT) != 0u) { c = mix(c, g.accent.rgb, 0.55); }
    if ((st & SELECTED) != 0u) { c = mix(c, g.select.rgb, 0.7); }
    if ((st & HOVER) != 0u) { c = mix(c, g.hover.rgb, 0.55); }
    return c;
}

// Edges, sketch curves and points (Onshape, live): hovered ones turn fully
// light orange, selected ones fully amber.
fn highlight_stroke(base: vec3<f32>, st: u32) -> vec3<f32> {
    var c = base;
    if ((st & ERROR) != 0u) { c = g.error.rgb; }
    if ((st & ACCENT) != 0u) { c = mix(c, g.accent.rgb, 0.55); }
    if ((st & SELECTED) != 0u) { c = g.select.rgb; }
    if ((st & HOVER) != 0u) { c = g.hover.rgb; }
    return c;
}

// A selected face or part (Onshape, live: the top face of a box reads
// `#e4b049`): amber with half of the usual light contrast.
const FACE_SELECTED: vec3<f32> = vec3<f32>(0.80, 0.444, 0.07);

// ---------------------------------------------------------------- background

struct BgOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) t: f32,
};

@vertex
fn vs_background(@builtin(vertex_index) vi: u32) -> BgOut {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    var o: BgOut;
    o.pos = vec4(p[vi], 0.0, 1.0);
    o.t = p[vi].y * 0.5 + 0.5;
    return o;
}

@fragment
fn fs_background(i: BgOut) -> @location(0) vec4<f32> {
    let t = clamp(i.t, 0.0, 1.0);
    return vec4(mix(g.bg_bottom.rgb, g.bg_top.rgb, t), 1.0);
}

// ---------------------------------------------------------------- meshes

struct MeshIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) pick: u32,
    @location(3) color: vec4<f32>,
};

struct MeshOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) @interpolate(flat) pick: u32,
    @location(3) color: vec4<f32>,
};

@vertex
fn vs_mesh(v: MeshIn) -> MeshOut {
    var o: MeshOut;
    let w = inst.model * vec4(v.pos, 1.0);
    o.world = w.xyz;
    o.pos = g.view_proj * w;
    o.normal = (inst.normal_m * vec4(v.normal, 0.0)).xyz;
    o.pick = global_pick(v.pick);
    o.color = v.color;
    return o;
}

fn shade(world: vec3<f32>, normal_in: vec3<f32>, front: bool, base: vec3<f32>) -> vec3<f32> {
    var n = normalize(normal_in);
    if (!front) { n = -n; }
    var v: vec3<f32>;
    if (g.camera_pos.w > 0.5) {
        v = normalize(g.camera_pos.xyz - world);
    } else {
        v = -g.camera_fwd.xyz;
    }
    let right = normalize(cross(g.camera_fwd.xyz, g.camera_up.xyz));
    // Onshape's look, fitted to its rendered face colors (a box in the
    // default view: top 0.98, right 0.71, front 0.28 of the base color plus
    // a broad highlight on the top face): one key light above and to the
    // right of the viewer, flat ambient, no fill light.
    let key = normalize(right * 0.52 + g.camera_up.xyz * 0.65 + v * 0.55);
    let diff = max(dot(n, key), 0.0) * 0.82;
    let ambient = 0.284;
    let h = normalize(key + v);
    let spec = pow(max(dot(n, h), 0.0), 6.0) * 0.42;
    return base * (diff + ambient) + vec3(spec);
}

fn mesh_color(i: MeshOut, front: bool) -> vec4<f32> {
    let st = state_of(i.pick);
    if ((st & HIDDEN) != 0u || clipped(i.world)) { discard; }
    // Per-primitive instances color each vertex (analysis maps, face
    // appearances); vertices with alpha 0 keep the instance color.
    var own = inst.color.rgb;
    if (inst.params.x != 0u && i.color.a > 0.0) { own = srgb_to_linear(i.color.rgb); }
    // params.y bit 1: unlit (construction planes keep their flat color and
    // tint on hover). Shaded faces don't change on hover: the app outlines
    // the hovered face instead (Onshape).
    let unlit = (inst.params.y & 2u) != 0u;
    var c = highlight(own, select(st & ~HOVER, st, unlit));
    if (!unlit) {
        if ((st & SELECTED) != 0u && (st & ERROR) == 0u) {
            c = mix(FACE_SELECTED * 0.97, shade(i.world, i.normal, front, FACE_SELECTED), 0.45);
        } else {
            c = shade(i.world, i.normal, front, c);
        }
    }
    // Back faces revealed by a section cut are tinted so cut solids read well.
    if (!front && g.flags.x != 0u) { c = mix(c, g.accent.rgb * 0.5, 0.5); }
    var a = inst.color.a;
    if ((st & DIM) != 0u) { a = a * 0.25; }
    return vec4(c, a);
}

@fragment
fn fs_mesh(i: MeshOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    return mesh_color(i, front);
}

struct SectionOut {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

// Section view (separate pipeline, used only while planes cut): a back face
// seen through the cut is replaced by the cap — the part's color, lit as
// the cut plane and hatched, at the depth where the view ray leaves the
// removed region (the intersection of the planes' removed half-spaces).
@fragment
fn fs_mesh_section(i: MeshOut, @builtin(front_facing) front: bool) -> SectionOut {
    var o: SectionOut;
    o.color = mesh_color(i, front);
    o.depth = i.pos.z;
    if (front || g.flags.x == 0u || (inst.params.y & 3u) != 0u) { return o; }
    var dir = g.camera_fwd.xyz;
    if (g.camera_pos.w > 0.5) { dir = normalize(i.world - g.camera_pos.xyz); }
    // Walk back toward the eye: p(s) = world - dir * s. Enter the removed
    // region at the largest crossing of the planes the point is outside of.
    var planes = array<vec4<f32>, 3>(g.clip, g.clip2, g.clip3);
    var s_in = 0.0;
    var normal = vec3<f32>(0.0);
    for (var k = 0u; k < g.flags.x; k = k + 1u) {
        let pl = planes[k];
        let f = dot(i.world, pl.xyz) - pl.w;
        if (f > 0.0) { continue; }
        let nd = dot(pl.xyz, dir);
        if (nd >= 0.0) { return o; }
        let s = f / nd;
        if (s >= s_in) { s_in = s; normal = pl.xyz; }
    }
    if (dot(normal, normal) == 0.0) { return o; }
    let c = i.world - dir * s_in;
    for (var k = 0u; k < g.flags.x; k = k + 1u) {
        let pl = planes[k];
        if (dot(c, pl.xyz) - pl.w < -1e-5 * max(1.0, abs(pl.w))) { return o; }
    }
    let clip = g.view_proj * vec4(c, 1.0);
    o.depth = clamp(clip.z / clip.w, 0.0, 1.0);
    var own = inst.color.rgb;
    if (inst.params.x != 0u && i.color.a > 0.0) { own = srgb_to_linear(i.color.rgb); }
    var col = shade(c, normal, true, own);
    // Hatching: thin diagonal lines (Onshape draws the cut in the part color, hatched).
    let k = (i.pos.x + i.pos.y) / (9.0 * g.viewport.z);
    if (fract(k) < 0.13) { col = col * 0.45; }
    o.color = vec4(col, 1.0);
    return o;
}

@fragment
fn fs_mesh_pick(i: MeshOut) -> @location(0) u32 {
    let st = state_of(i.pick);
    if ((st & HIDDEN) != 0u || clipped(i.world)) { discard; }
    if (inst.params.z == 0u) { return 0u; }
    return i.pick;
}

// ---------------------------------------------------------------- lines

struct LineIn {
    @location(0) a: vec3<f32>,
    @location(1) b: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) pick: u32,
    @location(4) width: f32,
    @location(5) style: u32,
};

struct LineOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) @interpolate(flat) pick: u32,
    @location(2) along: f32,
    @location(3) world: vec3<f32>,
    @location(4) @interpolate(flat) style: u32,
};

fn clip_near(p: vec4<f32>, q: vec4<f32>) -> vec4<f32> {
    // For perspective, move p toward q until it is in front of the camera.
    let eps = 1e-6;
    if (p.w >= eps) { return p; }
    let t = (eps - p.w) / (q.w - p.w);
    return mix(p, q, t);
}

@vertex
fn vs_line(@builtin(vertex_index) vi: u32, l: LineIn) -> LineOut {
    // Two triangles: corners (t, side)
    var ts = array<f32, 6>(0.0, 0.0, 1.0, 0.0, 1.0, 1.0);
    var ss = array<f32, 6>(-1.0, 1.0, 1.0, -1.0, 1.0, -1.0);
    let t = ts[vi];
    let side = ss[vi];

    let wa = (inst.model * vec4(l.a, 1.0)).xyz;
    let wb = (inst.model * vec4(l.b, 1.0)).xyz;
    var ca = g.view_proj * vec4(wa, 1.0);
    var cb = g.view_proj * vec4(wb, 1.0);
    ca = clip_near(ca, cb);
    cb = clip_near(cb, ca);

    let half_vp = g.viewport.xy * 0.5;
    let sa = ca.xy / ca.w * half_vp;
    let sb = cb.xy / cb.w * half_vp;
    var d = sb - sa;
    let len = length(d);
    if (len < 1e-5) { d = vec2(1.0, 0.0); } else { d = d / len; }
    let nrm = vec2(-d.y, d.x);

    let pick = global_pick(l.pick);
    let st = state_of(pick);
    var width = l.width + g.viewport.w;
    if ((st & (HOVER | SELECTED)) != 0u) { width = width * 1.8 + 1.0; }
    let half = max(width * g.viewport.z * 0.5, 0.5);

    var c = select(ca, cb, t > 0.5);
    let ext = select(-1.0, 1.0, t > 0.5);
    let off_px = nrm * side * half + d * ext * half * 0.5;
    c = vec4(c.xy + off_px / half_vp * c.w, c.z, c.w);

    if ((l.style & 2u) != 0u) {
        c.z = c.w; // on top: nearest depth
    } else {
        // Pull lines slightly toward the camera so edges win against faces.
        c.z = c.z * 1.0004 + 2e-5 * c.w;
    }

    var o: LineOut;
    o.pos = c;
    var base: vec3<f32>;
    var alpha: f32;
    if (inst.params.x != 0u) { base = srgb_to_linear(l.color.rgb); alpha = l.color.a; } else { base = inst.color.rgb; alpha = inst.color.a; }
    if ((st & DIM) != 0u) { alpha = alpha * 0.3; }
    o.color = vec4(highlight_stroke(base, st), alpha);
    o.pick = pick;
    o.along = t * len;
    o.world = mix(wa, wb, t);
    o.style = l.style;
    if ((st & HIDDEN) != 0u) { o.pos = vec4(0.0, 0.0, -2.0, 1.0); }
    return o;
}

@fragment
fn fs_line(i: LineOut) -> @location(0) vec4<f32> {
    if (clipped(i.world)) { discard; }
    if ((i.style & 1u) != 0u && fract(i.along / (7.0 * g.viewport.z)) > 0.55) { discard; }
    if ((i.style & 4u) != 0u) {
        let t = fract(i.along / (16.0 * g.viewport.z));
        if (t > 0.55 && (t < 0.68 || t > 0.8)) { discard; }
    }
    return i.color;
}

@fragment
fn fs_line_pick(i: LineOut) -> @location(0) u32 {
    if (clipped(i.world)) { discard; }
    if (inst.params.z == 0u) { discard; }
    return i.pick;
}

// ---------------------------------------------------------------- points

struct PointIn {
    @location(0) p: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) pick: u32,
    @location(3) size: f32,
    @location(4) style: u32,
};

struct PointOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) @interpolate(flat) pick: u32,
    @location(2) uv: vec2<f32>,
    @location(3) world: vec3<f32>,
    @location(4) @interpolate(flat) style: u32,
};

@vertex
fn vs_point(@builtin(vertex_index) vi: u32, p: PointIn) -> PointOut {
    var corners = array<vec2<f32>, 6>(
        vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0),
        vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0));
    let uv = corners[vi];
    let w = (inst.model * vec4(p.p, 1.0)).xyz;
    var c = g.view_proj * vec4(w, 1.0);
    let pick = global_pick(p.pick);
    let st = state_of(pick);
    var size = p.size + g.viewport.w;
    if ((st & (HOVER | SELECTED)) != 0u) { size = size * 1.5 + 1.0; }
    let half = size * g.viewport.z * 0.5;
    c = vec4(c.xy + uv * half / (g.viewport.xy * 0.5) * c.w, c.z, c.w);
    if ((p.style & 2u) != 0u) { c.z = c.w; } else { c.z = c.z * 1.0008 + 4e-5 * c.w; }
    var o: PointOut;
    o.pos = c;
    var base: vec3<f32>;
    var alpha: f32;
    if (inst.params.x != 0u) { base = srgb_to_linear(p.color.rgb); alpha = p.color.a; } else { base = inst.color.rgb; alpha = inst.color.a; }
    o.color = vec4(highlight_stroke(base, st), alpha);
    o.pick = pick;
    o.uv = uv;
    o.world = w;
    o.style = p.style;
    if ((st & HIDDEN) != 0u) { o.pos = vec4(0.0, 0.0, -2.0, 1.0); }
    return o;
}

@fragment
fn fs_point(i: PointOut) -> @location(0) vec4<f32> {
    let r2 = dot(i.uv, i.uv);
    if (r2 > 1.0 || clipped(i.world)) { discard; }
    // Style bit 4: a ring with a center dot (the origin).
    if ((i.style & 4u) != 0u && r2 < 0.42 && r2 > 0.1) { discard; }
    return i.color;
}

@fragment
fn fs_point_pick(i: PointOut) -> @location(0) u32 {
    if (dot(i.uv, i.uv) > 1.0 || clipped(i.world)) { discard; }
    if (inst.params.z == 0u) { discard; }
    return i.pick;
}
