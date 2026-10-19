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
    specular: vec4<f32>,     // rgb specular color, w shininess 0..1 (Graphics preferences)
    dash: vec4<f32>,         // construction lines: dash, gap, dash, gap (px)
    room: vec4<f32>,         // zebra stripes room: center xyz, radius
};

struct Inst {
    model: mat4x4<f32>,
    normal_m: mat4x4<f32>,
    color: vec4<f32>,
    params: vec4<u32>,       // x = per-primitive color, y = ignore clip, z = pickable, w = pick base
    extra: vec4<f32>,        // x = line width scale
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
const QUERY: u32 = 64u;
const OUTLINE: u32 = 128u;
// Entity of a hovered dialog selection row (Onshape, live: #f2ff80).
const QUERY_COLOR: vec3<f32> = vec3<f32>(0.888, 1.0, 0.216);

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

// Geometry lying on a section plane counts as cut (live part-studio/74: a
// face picked as the section plane shows the hatched cap): 0.1 µm.
const CLIP_EPS: f32 = 1e-7;

fn clipped(world: vec3<f32>) -> bool {
    if (g.flags.x == 0u || (inst.params.y & 1u) != 0u) { return false; }
    var c = dot(world, g.clip.xyz) > g.clip.w - CLIP_EPS;
    if (g.flags.x > 1u) { c = c && dot(world, g.clip2.xyz) > g.clip2.w - CLIP_EPS; }
    if (g.flags.x > 2u) { c = c && dot(world, g.clip3.xyz) > g.clip3.w - CLIP_EPS; }
    return c;
}

fn highlight(base: vec3<f32>, st: u32) -> vec3<f32> {
    var c = base;
    if ((st & ERROR) != 0u) { c = mix(c, g.error.rgb, 0.7); }
    if ((st & ACCENT) != 0u) { c = mix(c, g.accent.rgb, 0.55); }
    if ((st & SELECTED) != 0u) { c = mix(c, g.select.rgb, 0.7); }
    if ((st & HOVER) != 0u) { c = mix(c, g.hover.rgb, 0.55); }
    if ((st & QUERY) != 0u) { c = mix(c, QUERY_COLOR, 0.7); }
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
    if ((st & QUERY) != 0u) { c = QUERY_COLOR; }
    return c;
}

// A selected face or part, with half of the usual light contrast. Fitted
// so the top face under a default plane reads `#e4b049` like live
// (selection/04); alone it is about `#e9a100`.
const FACE_SELECTED: vec3<f32> = vec3<f32>(0.87, 0.385, 0.0);

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
    return mesh_vertex(v);
}

fn mesh_vertex(v: MeshIn) -> MeshOut {
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
    // (Scaled by 0.745 when the default part color became the live
    // `#9dcfed`; fitted against `#8ab5cf` before.)
    let diff = max(dot(n, key), 0.0) * 0.611;
    let ambient = 0.212;
    let h = normalize(key + v);
    // Shininess 3% (Onshape's default) gives a broad highlight (exponent 6);
    // the default specular color #999 gives 0.42 of white.
    let spec = pow(max(dot(n, h), 0.0), max(g.specular.w * 200.0, 1.0)) * g.specular.rgb * 1.32;
    return base * (diff + ambient) + spec;
}

// Feature previews (instance params.y bit 4): no part lighting. Onshape's
// preview of a box in the default view has the top and front faces alike and
// the right side lighter (live part-studio/230): flat ambient plus one light
// from the viewer's right and slightly below, no highlight.
fn shade_preview(normal_in: vec3<f32>, front: bool, base: vec3<f32>) -> vec3<f32> {
    var n = normalize(normal_in);
    if (!front) { n = -n; }
    let right = normalize(cross(g.camera_fwd.xyz, g.camera_up.xyz));
    let light = normalize(right * 0.866 - g.camera_up.xyz * 0.5);
    return base * (PREVIEW_AMBIENT + PREVIEW_DIFFUSE * max(dot(n, light), 0.0));
}
const PREVIEW_AMBIENT: f32 = 0.5;
const PREVIEW_DIFFUSE: f32 = 0.74;

// Zebra stripes (Onshape's analysis tool): the reflection of a room with
// black and white stripes around the viewer, horizontal unless flipped
// (instance params.y bit 3); the count is in bits 8 and up.
fn zebra(world: vec3<f32>, normal: vec3<f32>, front: bool) -> vec3<f32> {
    var n = normalize(normal);
    if (!front) { n = -n; }
    var v: vec3<f32>;
    if (g.camera_pos.w > 0.5) { v = normalize(g.camera_pos.xyz - world); } else { v = -g.camera_fwd.xyz; }
    let r = reflect(-v, n);
    // The striped room is sampled a fixed distance along the reflected ray,
    // seen from the model's center: on a flat face the stripes ring a point
    // and radiate from it (Onshape). Stripes are latitudes about the view's
    // up axis; flipped, longitudes.
    let dir = normalize(world - g.room.xyz + r * g.room.w);
    let right = normalize(cross(g.camera_fwd.xyz, g.camera_up.xyz));
    var s = acos(clamp(dot(dir, g.camera_up.xyz), -1.0, 1.0)) / 3.14159265;
    if ((inst.params.y & 8u) != 0u) { s = atan2(dot(dir, right), dot(dir, -g.camera_fwd.xyz)) / 6.2831853 + 0.5; }
    let count = f32(max(inst.params.y >> 8u, 1u));
    if ((u32(floor(s * count)) & 1u) == 0u) { return vec3(0.0); }
    return vec3(1.0);
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
    if (!unlit && (inst.params.y & 4u) != 0u) {
        c = zebra(i.world, i.normal, front);
        if ((st & SELECTED) != 0u) { c = mix(c, FACE_SELECTED, 0.5); }
    } else if (!unlit) {
        if ((st & SELECTED) != 0u && (st & ERROR) == 0u) {
            c = mix(FACE_SELECTED * 0.97, shade(i.world, i.normal, front, FACE_SELECTED), 0.45);
        } else if ((inst.params.y & 16u) != 0u) {
            c = shade_preview(i.normal, front, c);
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

// Part outline (state OUTLINE): the faces turned away from the viewer,
// pushed out along their normals by the outline width in screen space.
// Where they stick out past the part's own front faces they show as the
// silhouette; everywhere else the front faces hide them.
@vertex
fn vs_outline(v: MeshIn) -> MeshOut {
    var o = mesh_vertex(v);
    let n = (g.view_proj * vec4(o.normal, 0.0)).xy;
    let len = length(n);
    if (len > 1e-6) {
        let width = 3.0 * g.viewport.z;
        o.pos = vec4(o.pos.xy + n / len * width * 2.0 / g.viewport.xy * o.pos.w, o.pos.zw);
    }
    return o;
}

@fragment
fn fs_outline(i: MeshOut) -> @location(0) vec4<f32> {
    let st = state_of(i.pick);
    if ((st & OUTLINE) == 0u || (st & HIDDEN) != 0u || clipped(i.world)) { discard; }
    var v: vec3<f32>;
    if (g.camera_pos.w > 0.5) {
        v = normalize(g.camera_pos.xyz - i.world);
    } else {
        v = -g.camera_fwd.xyz;
    }
    if (dot(normalize(i.normal), v) > 0.0) { discard; }
    return vec4(g.select.rgb, 1.0);
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
    // Back faces only (front faces are culled); anything but a cap is
    // already drawn by the opaque pass.
    if (front || g.flags.x == 0u || (inst.params.y & 3u) != 0u || (state_of(i.pick) & HIDDEN) != 0u) { discard; }
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
        if (nd >= 0.0) { discard; }
        let s = f / nd;
        if (s >= s_in) { s_in = s; normal = pl.xyz; }
    }
    if (dot(normal, normal) == 0.0) { discard; }
    let c = i.world - dir * s_in;
    for (var k = 0u; k < g.flags.x; k = k + 1u) {
        let pl = planes[k];
        if (dot(c, pl.xyz) - pl.w < -1e-5 * max(1.0, abs(pl.w))) { discard; }
    }
    let clip = g.view_proj * vec4(c, 1.0);
    o.depth = clamp(clip.z / clip.w, 0.0, 1.0);
    var own = inst.color.rgb;
    if (inst.params.x != 0u && i.color.a > 0.0) { own = srgb_to_linear(i.color.rgb); }
    var col = shade(c, normal, true, own);
    // Hatching (live part-studio/74): 1 px lines at 45° in the cut plane's
    // own axes (X+Y on Top), 11 px apart vertically on screen (live), half the cap's color.
    var u = vec3<f32>(1.0, 0.0, 0.0);
    if (abs(normal.z) < 0.9) { u = vec3<f32>(0.0, 0.0, 1.0); }
    u = normalize(u - normal * dot(u, normal));
    let v = cross(normal, u);
    let across = normalize(u - v);
    let p0 = g.view_proj * vec4(c, 1.0);
    let p1 = g.view_proj * vec4(c + across * 0.001, 1.0);
    let px_per_unit = length((p1.xy / p1.w - p0.xy / p0.w) * g.viewport.xy * 0.5) * 1000.0;
    let spacing = 11.6 * g.viewport.z / max(px_per_unit, 1e-6);
    let line_w = 1.0 * g.viewport.z / max(px_per_unit, 1e-6);
    if (fract(dot(c, across) / spacing) < line_w / spacing) { col = col * 0.5; }
    o.color = vec4(col, 1.0);
    return o;
}

// Section caps, first pass: kept surfaces only (the stencil counts them).
@fragment
fn fs_section_count(i: MeshOut) -> @location(0) vec4<f32> {
    if ((inst.params.y & 3u) != 0u || (state_of(i.pick) & HIDDEN) != 0u || clipped(i.world)) { discard; }
    return vec4(0.0);
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
    var width = l.width * inst.extra.x + g.viewport.w;
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
    // Construction lines: dash, gap, dash, gap (Graphics preferences).
    if ((i.style & 8u) != 0u) {
        let period = max(g.dash.x + g.dash.y + g.dash.z + g.dash.w, 1.0);
        let t = fract(i.along / (period * g.viewport.z)) * period;
        if ((t > g.dash.x && t < g.dash.x + g.dash.y) || t > g.dash.x + g.dash.y + g.dash.z) { discard; }
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
    // Style bit 8: a hollow point (white inside a ring of the point color),
    // Onshape's standalone sketch points and spline handles.
    if ((i.style & 8u) != 0u && r2 < 0.3) { return vec4<f32>(1.0, 1.0, 1.0, i.color.a); }
    return i.color;
}

@fragment
fn fs_point_pick(i: PointOut) -> @location(0) u32 {
    if (dot(i.uv, i.uv) > 1.0 || clipped(i.world)) { discard; }
    if (inst.params.z == 0u) { discard; }
    return i.pick;
}

// ---------------------------------------------------------------- images

// Textured quads (sketch images). Group 2 is bound only by the image
// pipelines. Never drawn into the pick buffer.
@group(2) @binding(0) var image_tex: texture_2d<f32>;
@group(2) @binding(1) var image_sampler: sampler;

struct ImageIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct ImageOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_image(v: ImageIn) -> ImageOut {
    var o: ImageOut;
    let w = inst.model * vec4(v.pos, 1.0);
    o.world = w.xyz;
    o.pos = g.view_proj * w;
    o.uv = v.uv;
    return o;
}

// The texture is sRGB (samples are linear like the instance color) with
// premultiplied alpha; so is the result (premultiplied blending).
fn image_color(i: ImageOut) -> vec4<f32> {
    let t = textureSample(image_tex, image_sampler, i.uv);
    if (clipped(i.world)) { discard; }
    let a = inst.color.a;
    return vec4(t.rgb * inst.color.rgb * a, t.a * a);
}

// Opaque instances draw twice: mostly opaque texels first, writing depth...
@fragment
fn fs_image_opaque(i: ImageOut) -> @location(0) vec4<f32> {
    let c = image_color(i);
    if (c.a < 0.5) { discard; }
    return c;
}

// ...then the soft (anti-aliased / translucent) texels, blended without
// writing depth.
@fragment
fn fs_image_soft(i: ImageOut) -> @location(0) vec4<f32> {
    let c = image_color(i);
    if (c.a >= 0.5 || c.a <= 0.0) { discard; }
    return c;
}

// Faded instances (instance alpha < 1): all texels blended, no depth write.
@fragment
fn fs_image_blend(i: ImageOut) -> @location(0) vec4<f32> {
    let c = image_color(i);
    if (c.a <= 0.0) { discard; }
    return c;
}

// Reflection analysis: a mirror of an equirectangular environment map
// (bound like an image). inst.extra: y = rotation (rad) about the up axis,
// z = blur (mip level), w = 1 when locked to the model (world, Z up; else
// it follows the camera).
@fragment
fn fs_mesh_reflect(i: MeshOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let base = mesh_color(i, front);
    var n = normalize(i.normal);
    if (!front) { n = -n; }
    var v: vec3<f32>;
    if (g.camera_pos.w > 0.5) { v = normalize(g.camera_pos.xyz - i.world); } else { v = -g.camera_fwd.xyz; }
    var r = reflect(-v, n);
    if (inst.extra.w < 0.5) {
        // Not locked: the environment turns with the camera (Onshape).
        // Camera frame: x right, y toward the viewer, z up.
        let right = normalize(cross(g.camera_fwd.xyz, g.camera_up.xyz));
        r = vec3(dot(r, right), -dot(r, g.camera_fwd.xyz), dot(r, g.camera_up.xyz));
    }
    let u = fract(atan2(r.y, r.x) / 6.2831853 + 0.5 + inst.extra.y / 6.2831853);
    let w = acos(clamp(r.z, -1.0, 1.0)) / 3.14159265;
    let env = textureSampleLevel(image_tex, image_sampler, vec2(u, w), inst.extra.z).rgb;
    // Selection and hover still tint the mirror.
    let st = state_of(i.pick);
    var c = env;
    if ((st & SELECTED) != 0u) { c = mix(c, FACE_SELECTED, 0.5); }
    return vec4(c, base.a);
}
