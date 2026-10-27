// noshape viewport shaders. Reverse-Z depth (1 = near, 0 = far).

struct Globals {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,   // xyz eye (orthographic: Onshape's, for lighting), w = 1 perspective / 0 orthographic
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
    depth_unit: vec4<f32>,   // x = our depth per unit of Onshape's depth buffer (perspective: per w)
    compare: vec4<f32>,      // compare mode: xyz Onshape's eye, w its near distance
    compare2: vec4<f32>,     // x its far distance, y on (1), z mix alpha
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
// Instances, and the order this pass draws them in: draw calls address
// instances by slot (instance index for meshes and images, vertex index / 6
// for lines and points), so copies sharing a buffer draw in one call.
@group(1) @binding(0) var<storage, read> insts: array<Inst>;
@group(1) @binding(1) var<storage, read> slots: array<u32>;
var<private> inst: Inst;

fn load_inst(slot: u32) {
    inst = insts[slots[slot]];
}

// The part edges' width in logical px (scene builder `push_polyline` 1.25).
const EDGE_WIDTH: f32 = 1.25;
const HOVER: u32 = 1u;
const SELECTED: u32 = 2u;
const BODY: u32 = 256u;
const HIDDEN: u32 = 4u;
const ACCENT: u32 = 8u;
const ERROR: u32 = 16u;
const DIM: u32 = 32u;
const QUERY: u32 = 64u;
const OUTLINE: u32 = 128u;
const TINT: u32 = 512u;
const NO_PICK: u32 = 1024u;
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

// Geometry lying on a section plane is kept (live ui-cap/04, 10-25: the
// plane on a block's top face leaves the face as it is, no cap; skb-sv
// bottom-edge-released: the plane through the bottom face keeps it and caps
// it hatched; an edge in the plane stays and lights up): 0.1 µm.
const CLIP_EPS: f32 = 1e-7;

fn clipped(world: vec3<f32>) -> bool {
    if (g.flags.x == 0u || (inst.params.y & 1u) != 0u) { return false; }
    var c = dot(world, g.clip.xyz) > g.clip.w + CLIP_EPS;
    if (g.flags.x > 1u) { c = c && dot(world, g.clip2.xyz) > g.clip2.w + CLIP_EPS; }
    if (g.flags.x > 2u) { c = c && dot(world, g.clip3.xyz) > g.clip3.w + CLIP_EPS; }
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
// to live selection/04's top face where no datum plane lies over it:
// (229,163,34) (was (227,158,0)). Under a plane live blends in sRGB and we
// in linear light, so the plane adds more blue there (ours (224,176,114)
// vs live (229,170,56)).
const FACE_SELECTED: vec3<f32> = vec3<f32>(0.887, 0.414, 0.018);
// A whole selected part / instance (state BODY): live (242,182,72) lit,
// outlined (state OUTLINE) in (255,192,76), 2 px (live ins-902); a part
// picked in the parts list reads (243,183,72) on its top face where no
// plane lies over it (selection/09).
const BODY_SELECTED: vec3<f32> = vec3<f32>(1.0, 0.536, 0.074);
const BODY_OUTLINE: vec3<f32> = vec3<f32>(1.0, 0.527, 0.073);

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
    return vec4(mix(linear_to_srgb(g.bg_bottom.rgb), linear_to_srgb(g.bg_top.rgb), t), 1.0);
}

// ---------------------------------------------------------------- meshes

// Compact vertices (renderer.rs `MeshVertex`): the position in 21-bit
// fixed point over the mesh bounds (the instance's model matrix scales it
// back), the normal octahedral encoded.
struct MeshIn {
    @location(0) pos: vec2<u32>,
    @location(1) normal: vec2<f32>,
    @location(2) pick: u32,
    @location(3) color: vec4<f32>,
};

fn mesh_pos(v: MeshIn) -> vec3<f32> {
    let a = v.pos.x;
    let b = v.pos.y;
    return vec3(f32(a & 0x1fffffu), f32((a >> 21u) | ((b & 0x3ffu) << 11u)), f32(b >> 10u));
}

fn oct_decode(e: vec2<f32>) -> vec3<f32> {
    var n = vec3(e, 1.0 - abs(e.x) - abs(e.y));
    let t = max(-n.z, 0.0);
    n.x += select(t, -t, n.x >= 0.0);
    n.y += select(t, -t, n.y >= 0.0);
    return normalize(n);
}

struct MeshOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) @interpolate(flat) pick: u32,
    @location(3) color: vec4<f32>,
    @location(4) @interpolate(flat) slot: u32,
};

@vertex
fn vs_mesh(v: MeshIn, @builtin(instance_index) slot: u32) -> MeshOut {
    return mesh_vertex(v, slot);
}

fn mesh_vertex(v: MeshIn, slot: u32) -> MeshOut {
    load_inst(slot);
    var o: MeshOut;
    o.slot = slot;
    let w = inst.model * vec4(mesh_pos(v), 1.0);
    o.world = w.xyz;
    o.pos = g.view_proj * w;
    o.normal = (inst.normal_m * vec4(oct_decode(v.normal), 0.0)).xyz;
    o.pick = global_pick(v.pick);
    o.color = v.color;
    return o;
}

// Onshape's part lighting (woolsthorpe bundle: `lightFragment` and the
// solid-face vertex shader), in sRGB values like WebGL: ambient 0.5 × the
// color (COLOR_AMBIENT_FACTOR), one light along eye-space (1, 1, 1) — to
// the right, up and toward the viewer — at 0.5 times the color
// (COLOR_DIFFUSE_FACTOR 1), and Blinn-Phong specular in the Specular color
// (#999999) with exponent Shininess × 1000 (the preference's 3 % is
// DEFAULT_SHININESS 30) toward the eye.
fn shade(world: vec3<f32>, normal_in: vec3<f32>, front: bool, base: vec3<f32>) -> vec3<f32> {
    var n = normalize(normal_in);
    if (!front) { n = -n; }
    let right = normalize(cross(g.camera_fwd.xyz, g.camera_up.xyz));
    let light = normalize(right + g.camera_up.xyz - g.camera_fwd.xyz);
    let eye = normalize(g.camera_pos.xyz - world);
    let h = normalize(light + eye);
    let c = linear_to_srgb(base);
    let spec = pow(max(dot(n, h), 0.0), max(g.specular.w * 1000.0, 1.0)) * linear_to_srgb(g.specular.rgb);
    // params.x bit 1: extra.z is an ambient shift, more ambient and less
    // light (Show dimensions; reflective meshes use extra.z otherwise).
    let s = select(0.0, inst.extra.z, (inst.params.x & 2u) != 0u);
    let lit = c * (0.5 + s) + max(dot(n, light), 0.0) * (0.5 - s) * c + spec;
    return srgb_to_linear(clamp(lit, vec3(0.0), vec3(1.0)));
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
// Fitted to live part-studio/230 (and psui-prv-01's Add boss): Onshape
// draws the model with the feature opaque into a texture of its own and
// mixes it over the model before it at the slider's 0.7 (uPreviewTexture,
// uPreviewAlpha; `Scene::preview_mix`).
const PREVIEW_AMBIENT: f32 = 0.58;
const PREVIEW_DIFFUSE: f32 = 0.95;

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

// ---------------------------------------------------------------- compare
//
// Versions and history › Compare (Onshape's compare pass, webgl bundle;
// renderer.rs `encode_compare`). Each version's depth and facing normal are
// rendered first (fs_compare_dn); a face of one version is lit in the "same"
// tint where the other version has the same surface at that pixel, else in
// its version's tint.

@group(3) @binding(0) var compare_base: texture_2d<f32>;
@group(3) @binding(1) var compare_target: texture_2d<f32>;
@group(3) @binding(2) var compare_over: texture_2d<f32>;

// COMPARE_SAME_LIT_TINT, COMPARE_BASE_TINT, COMPARE_TARGET_TINT (sRGB).
const COMPARE_SAME: vec3<f32> = vec3(1.0, 1.0, 1.0);
const COMPARE_BASE: vec3<f32> = vec3(144.0, 207.0, 241.0) / 255.0;
const COMPARE_TARGET: vec3<f32> = vec3(221.0, 82.0, 40.0) / 255.0;
// compareFunctions: COMPARE_NEAR_ZERO, COMPARE_NORMAL_FACTOR.
const COMPARE_NEAR_ZERO: f32 = 0.0004;
const COMPARE_NORMAL_FACTOR: f32 = 0.98;

// Onshape's vDepth: the eye distance between its near (0) and far (1).
fn compare_depth(world: vec3<f32>) -> f32 {
    return (dot(world - g.compare.xyz, g.camera_fwd.xyz) - g.compare.w) / (g.compare2.x - g.compare.w);
}

// The color this version's face takes at this pixel (linear).
fn compare_tint(i: MeshOut, front: bool, own: vec3<f32>) -> vec3<f32> {
    let layer = (inst.params.y >> 5u) & 3u;
    if (layer == 0u || g.compare2.y < 0.5) { return own; }
    let p = vec2<i32>(floor(i.pos.xy));
    var other: vec4<f32>;
    if (layer == 1u) { other = textureLoad(compare_target, p, 0); } else { other = textureLoad(compare_base, p, 0); }
    var n = normalize(i.normal);
    if (!front) { n = -n; }
    let same = other.w >= 0.0 && abs(compare_depth(i.world) - other.w) < COMPARE_NEAR_ZERO && dot(n, other.xyz) > COMPARE_NORMAL_FACTOR;
    var tint = COMPARE_SAME;
    if (!same) { tint = select(COMPARE_TARGET, COMPARE_BASE, layer == 1u); }
    return srgb_to_linear(tint);
}

// A version's facing normal and depth (w < 0: nothing there).
@fragment
fn fs_compare_dn(i: MeshOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    load_inst(i.slot);
    if ((state_of(i.pick) & HIDDEN) != 0u || clipped(i.world)) { discard; }
    var n = normalize(i.normal);
    if (!front) { n = -n; }
    return vec4(n, compare_depth(i.world));
}

// Both versions drawn together, mixed over the "under" version
// (Onshape's compareBlend: the over color with alpha uBlend), in gamma
// space: the target is the frame's unorm view.
@fragment
fn fs_compare_mix(i: BgOut) -> @location(0) vec4<f32> {
    let c = textureLoad(compare_over, vec2<i32>(floor(i.pos.xy)), 0);
    return vec4(c.rgb, g.compare2.z);
}

fn mesh_color(i: MeshOut, front: bool) -> vec4<f32> {
    let st = state_of(i.pick);
    if ((st & HIDDEN) != 0u || clipped(i.world)) { discard; }
    // params.x bit 2: only the selected faces (the selection drawn again
    // over the see-through datum planes, Instance::selected_only).
    if ((inst.params.x & 4u) != 0u && (st & SELECTED) == 0u) { discard; }
    return mesh_shade(i, front, st);
}

// `mesh_color` past the hidden / clipped test.
fn mesh_shade(i: MeshOut, front: bool, st: u32) -> vec4<f32> {
    // Per-primitive instances color each vertex (analysis maps, face
    // appearances); vertices with alpha 0 keep the instance color.
    var own = inst.color.rgb;
    if ((inst.params.x & 1u) != 0u && i.color.a > 0.0) { own = srgb_to_linear(i.color.rgb); }
    own = compare_tint(i, front, own);
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
            let sel = select(FACE_SELECTED, BODY_SELECTED, (st & BODY) != 0u);
            // A whole part keeps more of its shading (live "psui (10-27, interaction 5.x)": top (243,183,72), front (192,144,57), right (224,169,66)).
            let body = (st & BODY) != 0u;
            c = mix(sel * select(0.97, 1.0, body), shade(i.world, i.normal, front, sel), select(0.45, 0.6, body));
        } else if ((inst.params.y & 16u) != 0u) {
            c = shade_preview(i.normal, front, c);
        } else {
            c = shade(i.world, i.normal, front, c);
        }
    }
    // A light selection tint over the shading, mixed in sRGB (state TINT).
    if ((st & TINT) != 0u) { c = srgb_to_linear(mix(linear_to_srgb(c), linear_to_srgb(g.select.rgb), 0.2)); }
    // Back faces revealed by a section cut are tinted so cut solids read well.
    if (!front && g.flags.x != 0u) { c = mix(c, g.accent.rgb * 0.5, 0.5); }
    // params.y bit 7: Isolate ghost, mixed toward white in sRGB (extra.y).
    if ((inst.params.y & 128u) != 0u) { c = srgb_to_linear(mix(linear_to_srgb(c), vec3(1.0), inst.extra.y)); }
    var a = inst.color.a;
    if ((st & DIM) != 0u) { a = a * 0.25; }
    return vec4(c, a);
}

@fragment
fn fs_mesh(i: MeshOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    load_inst(i.slot);
    return enc(mesh_color(i, front));
}

// `fs_mesh` for meshes nothing hides or clips (renderer `DrawLists::solid`):
// no `discard`, so tile-based GPUs shade only the visible fragments.
@fragment
fn fs_mesh_solid(i: MeshOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    load_inst(i.slot);
    return enc(mesh_shade(i, front, state_of(i.pick)));
}

// Onshape's image-based silhouettes (`vs-backfacesil-faces`): back faces
// pushed out along their normals by half an edge width on screen (less
// where they face straight away), in the edge color; the part's front
// faces hide them except a thin band along its outline.
struct BackSilOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) clamped_ndv: f32,
    @location(1) world: vec3<f32>,
    @location(2) @interpolate(flat) pick: u32,
    @location(3) @interpolate(flat) slot: u32,
};

@vertex
fn vs_backsil(v: MeshIn, @builtin(instance_index) slot: u32) -> BackSilOut {
    load_inst(slot);
    let w = (inst.model * vec4(mesh_pos(v), 1.0)).xyz;
    let n = normalize((inst.normal_m * vec4(oct_decode(v.normal), 0.0)).xyz);
    let persp = g.camera_pos.w > 0.5;
    let ndv = dot(n, select(-g.camera_fwd.xyz, normalize(g.camera_pos.xyz - w), persp));
    // A small step along the normal (any length in orthographic).
    let step = select(1e-3, 1e-3 * length(g.camera_pos.xyz - w), persp);
    let p = g.view_proj * vec4(w, 1.0);
    let q = g.view_proj * vec4(w + n * step, 1.0);
    let half_vp = g.viewport.xy * 0.5;
    let screen = length((q.xy / q.w - p.xy / p.w) * half_vp);
    // The part's edge width (EDGE_WIDTH × the line width preference).
    let width = EDGE_WIDTH * inst.extra.x;
    let scale = 0.5 * g.viewport.z * width / (screen + 1e-6) * (1.0 - abs(ndv));
    var o: BackSilOut;
    o.pos = mix(p, q, scale);
    o.clamped_ndv = min(ndv, 0.0);
    o.world = w;
    o.pick = global_pick(v.pick);
    o.slot = slot;
    return o;
}

@fragment
fn fs_backsil(i: BackSilOut) -> @location(0) vec4<f32> {
    if (i.clamped_ndv == 0.0) { discard; }
    load_inst(i.slot);
    let st = state_of(i.pick);
    if ((st & HIDDEN) != 0u || clipped(i.world)) { discard; }
    return enc(vec4(inst.color.rgb, 1.0));
}

// Part outline (state OUTLINE): the faces turned away from the viewer,
// pushed out along their normals by the outline width in screen space.
// Where they stick out past the part's own front faces they show as the
// silhouette; everywhere else the front faces hide them.
@vertex
fn vs_outline(v: MeshIn, @builtin(instance_index) slot: u32) -> MeshOut {
    var o = mesh_vertex(v, slot);
    let n = (g.view_proj * vec4(o.normal, 0.0)).xy;
    let len = length(n);
    if (len > 1e-6) {
        // A whole selected part (BODY): live ins-902's band is 3 px at the
        // bottom, 2 px at the side; the boundary edge eats about one.
        let width = select(3.0, 4.0, (state_of(o.pick) & BODY) != 0u) * g.viewport.z;
        o.pos = vec4(o.pos.xy + n / len * width * 2.0 / g.viewport.xy * o.pos.w, o.pos.zw);
    }
    return o;
}

@fragment
fn fs_outline(i: MeshOut) -> @location(0) vec4<f32> {
    load_inst(i.slot);
    let st = state_of(i.pick);
    if ((st & OUTLINE) == 0u || (st & HIDDEN) != 0u || clipped(i.world)) { discard; }
    var v: vec3<f32>;
    if (g.camera_pos.w > 0.5) {
        v = normalize(g.camera_pos.xyz - i.world);
    } else {
        v = -g.camera_fwd.xyz;
    }
    if (dot(normalize(i.normal), v) > 0.0) { discard; }
    // Hovered, not selected: the light-orange hover color (live G35, a
    // hovered mesh part).
    if ((st & HOVER) != 0u && (st & SELECTED) == 0u) { return enc(vec4(g.hover.rgb, 1.0)); }
    if ((st & BODY) != 0u) { return enc(vec4(BODY_OUTLINE, 1.0)); }
    return enc(vec4(g.select.rgb, 1.0));
}

// Depth of outlined parts only (no color): see mesh_outline_depth.
@fragment
fn fs_outline_depth(i: MeshOut) -> @location(0) vec4<f32> {
    load_inst(i.slot);
    let st = state_of(i.pick);
    if ((st & OUTLINE) == 0u || (st & HIDDEN) != 0u || clipped(i.world)) { discard; }
    return vec4(0.0);
}

// ---------------------------------------------------------------- section caps
//
// Onshape's stencil-free capping (webgl bundle, kern G40): every kept surface
// adds its part's color to a float mask (+ for front faces, - for back
// faces, alpha + 16 per part), so where the plane cuts material the mask
// holds that part's color and part count. An edge pass outlines the regions
// (section_caps.wgsl), then a quad on each plane draws the hatched cap.

// The color target is plain BGRA8 holding sRGB-encoded values, like the
// WebGL canvas Onshape draws on: blending and the MSAA resolve mix the
// encoded values (as Chrome does), so a color computed linear leaves its
// fragment shader through `enc`.
fn enc(c: vec4<f32>) -> vec4<f32> {
    return vec4(linear_to_srgb(c.rgb), c.a);
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3(1.0 / 2.4)) - vec3(0.055);
    return select(hi, lo, c <= vec3(0.0031308));
}

// Alpha weight of one part in the mask (Onshape's partCountOrder).
const PART_COUNT_ORDER: f32 = 16.0;

@fragment
fn fs_section_mask(i: MeshOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    load_inst(i.slot);
    if ((inst.params.y & 3u) != 0u || (state_of(i.pick) & HIDDEN) != 0u || clipped(i.world)) { discard; }
    var own = linear_to_srgb(inst.color.rgb);
    if ((inst.params.x & 1u) != 0u && i.color.a > 0.0) { own = i.color.rgb; }
    // The part's id nudges its color, so equal colors hatch differently
    // (Onshape: occurrence id + mesh group hash; `Instance::hatch_id`).
    let hid = inst.params.x >> 8u;
    let id = select(f32(inst.params.w), f32(hid), hid != 0u);
    let offset = vec3(id % 31.0, id % 71.0, id % 97.0) * 0.001;
    let c = vec4(own + offset, inst.color.a + PART_COUNT_ORDER);
    return select(-c, c, front);
}

struct CapPlane {
    center: vec4<f32>,    // xyz plane point, w = half size of the quad
    u: vec4<f32>,         // xyz tangent, w = hatch pattern scale (world)
    v: vec4<f32>,         // xyz normal x tangent
    selector: vec4<f32>,  // this plane's selector color
    index: vec4<u32>,     // x = plane index, y = 1 when the plane is selected
};

@group(2) @binding(0) var<uniform> cap: CapPlane;
@group(2) @binding(1) var cap_mask: texture_2d<f32>;
@group(2) @binding(2) var cap_edges: texture_2d<f32>;

struct CapOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) world: vec3<f32>,
};

@vertex
fn vs_section_cap(@builtin(vertex_index) vi: u32) -> CapOut {
    var corners = array<vec2<f32>, 6>(vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0));
    let k = corners[vi] * cap.center.w;
    var o: CapOut;
    o.world = cap.center.xyz + cap.u.xyz * k.x + cap.v.xyz * k.y;
    o.pos = g.view_proj * vec4(o.world, 1.0);
    return o;
}

fn cap_sample(p: vec2<i32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(cap_mask));
    return abs(textureLoad(cap_mask, clamp(p, vec2(0), size - 1), 0));
}

// Hash of the part color the hatch direction and pacing come from.
fn color_hash(c: vec3<f32>) -> f32 {
    return (c.r * 1000.0) % 1000.0 + (c.g * 1000.0) % 1000.0 + (c.b + 1000.0) % 1000.0;
}

// Hatch coverage at `t` (plane coordinates): a smooth tent of width 0.2
// pattern scales once per period; direction and period follow the color.
fn hatch(t: vec2<f32>, c: vec3<f32>) -> f32 {
    const THICKNESS: f32 = 0.2;
    const VARIATION: f32 = 1.75;
    let rolloff = min(THICKNESS / 2.0, 0.1);
    let interior = THICKNESS - rolloff * 2.0;
    let h = color_hash(c);
    let pace = mix(1.0, VARIATION * VARIATION, fract(h)) / VARIATION;
    let period = cap.u.w * pace;
    let along = dot(vec2(cos(h), sin(h)), t);
    let cycle = (along - period * floor(along / period)) / period;
    let step = rolloff / pace;
    let up = 1.0 - step - interior / pace;
    let rise = smoothstep(up - step, up, cycle);
    let fall = 1.0 - smoothstep(1.0 - step, 1.0, cycle);
    return select(fall, rise, cycle < up);
}

@fragment
fn fs_section_cap(i: CapOut) -> @location(0) vec4<f32> {
    // Only where the other planes cut too.
    var planes = array<vec4<f32>, 3>(g.clip, g.clip2, g.clip3);
    for (var k = 0u; k < g.flags.x; k = k + 1u) {
        let pl = planes[k];
        if (k != cap.index.x && dot(i.world, pl.xyz) - pl.w < -CLIP_EPS) { discard; }
    }
    let p = vec2<i32>(floor(i.pos.xy));
    // The sample with the median alpha of three in a row (Onshape: robust
    // to one-pixel cracks).
    let s0 = cap_sample(p - vec2(1, 0));
    let s1 = cap_sample(p);
    let s2 = cap_sample(p + vec2(1, 0));
    var s = s1;
    if ((s1.a <= s0.a && s0.a <= s2.a) || (s2.a <= s0.a && s0.a <= s1.a)) {
        s = s0;
    } else if ((s0.a <= s2.a && s2.a <= s1.a) || (s1.a <= s2.a && s2.a <= s0.a)) {
        s = s2;
    }
    let edge = textureLoad(cap_edges, p, 0).r;
    if (s.a < 0.001) {
        if (edge <= 0.01) { discard; }
        // Black at `edge` over what is there, as if blended in gamma space
        // (Onshape's canvas) rather than linear (exact over white).
        let keep = pow((1.0 - edge + 0.055) / 1.055, 2.4);
        return vec4(0.0, 0.0, 0.0, 1.0 - keep);
    }
    let parts = floor(s.a / PART_COUNT_ORDER);
    let alpha = clamp(s.a - parts * PART_COUNT_ORDER, 0.0, 1.0);
    // Pattern colors are sRGB values, like the part color in the mask.
    let d = i.world - cap.center.xyz;
    let t = vec2(dot(cap.u.xyz, d), dot(cap.v.xyz, d));
    let fade = 0.5;
    let shade = hatch(t, s.rgb);
    var col: vec3<f32>;
    if (max(s.r, max(s.g, s.b)) < 0.4) {
        // Dark parts get lighter lines.
        let lift = mix(0.0, (1.0 - fade) * 2.0, shade);
        col = mix(vec3(lift), s.rgb * (1.0 + lift), max(s.r, max(s.g, s.b)) / 0.4);
    } else {
        col = s.rgb * mix(1.0, fade, shade);
    }
    // Two parts' material at one point: red (Onshape's intersecting color).
    if (parts > 1.0) { col = vec3(220.0 / 255.0, 0.0, 0.0); }
    col = mix(col, cap.selector.rgb, 0.1);
    // Onshape's endcap shader: a selected plane (the Section view dialog's)
    // mixes toward SECTION_PLANE_PRE_HIGHLIGHT_COLOR rgb(255, 198, 133) by
    // ENTITY_HIGHLIGHT_COLOR1's alpha 0.8 (#8fc4e4 parts give live's #e9c698).
    if (cap.index.y != 0u) { col = mix(col, vec3(255.0, 198.0, 133.0) / 255.0, 0.8); }
    col = mix(col, vec3(0.0), edge);
    var a = alpha;
    if (parts > 1.0) { a = clamp(alpha + 0.1, 0.0, 1.0); }
    return vec4(col, a);
}

@fragment
fn fs_mesh_pick(i: MeshOut) -> @location(0) u32 {
    load_inst(i.slot);
    let st = state_of(i.pick);
    if ((st & (HIDDEN | NO_PICK)) != 0u || clipped(i.world)) { discard; }
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
    @location(5) @interpolate(flat) slot: u32,
    // Signed distance from the centre line and the half width of a part
    // edge's dark core (0: none), in physical px.
    @location(6) across: f32,
    @location(7) @interpolate(flat) core: f32,
};

fn clip_near(p: vec4<f32>, q: vec4<f32>) -> vec4<f32> {
    // For perspective, move p toward q until it is in front of the camera.
    let eps = 1e-6;
    if (p.w >= eps) { return p; }
    let t = (eps - p.w) / (q.w - p.w);
    return mix(p, q, t);
}

// Onshape's depth offsets (webgl bundle `setZOffset`: polygonOffset(z / 29,
// z) with faces 27, edges 15, points 9). The slope part is the pipelines'
// depth bias (FACE/EDGE/POINT_DEPTH_BIAS in renderer.rs); the constant part
// is here: `c` in front of the faces by `units` of Onshape's 24-bit depth
// (renderer.rs `onshape_depth_unit`), so edges and points win only against
// the faces they lie on, never through geometry in front of them.
fn in_front_of_faces(c: vec4<f32>, units: f32) -> vec4<f32> {
    var o = c;
    o.z = c.z + units * g.depth_unit.x * select(1.0, c.w, g.camera_pos.w > 0.5);
    return o;
}

@vertex
fn vs_line(@builtin(vertex_index) index: u32, l: LineIn) -> LineOut {
    // Six vertices (two triangles) per segment and slot: corners (t, side).
    let slot = index / 6u;
    load_inst(slot);
    return line_vertex(index, l, 0xffffffffu);
}

// A tessellation edge inside a curved face with the normals of its two
// facets (Onshape's edge-based silhouettes, `vs-draw-silhouettes`): drawn
// like an edge where one facet faces the viewer and the other doesn't.
struct SilhouetteIn {
    @location(0) a: vec3<f32>,
    @location(1) b: vec3<f32>,
    @location(2) n1: vec3<f32>,
    @location(3) n2: vec3<f32>,
    @location(4) pick: u32,
    @location(5) width: f32,
};

@vertex
fn vs_silhouette(@builtin(vertex_index) index: u32, s: SilhouetteIn) -> LineOut {
    let slot = index / 6u;
    load_inst(slot);
    let n1 = (inst.normal_m * vec4(s.n1, 0.0)).xyz;
    let n2 = (inst.normal_m * vec4(s.n2, 0.0)).xyz;
    // Toward the viewer: the eye ray (perspective) or the view direction.
    let at = (inst.model * vec4(s.a, 1.0)).xyz;
    let view = select(-g.camera_fwd.xyz, g.camera_pos.xyz - at, g.camera_pos.w > 0.5);
    var l: LineIn;
    l.a = s.a;
    l.b = s.b;
    l.color = vec4(0.0);
    l.pick = s.pick;
    l.width = s.width;
    // Lifted: a hovered part's silhouette over the part edges it runs on.
    l.style = 32u;
    // A face's hover and selection show on its boundary edges, not here.
    var o = line_vertex(index, l, ~(HOVER | SELECTED));
    if (dot(n1, view) * dot(n2, view) >= 0.0) { o.pos = vec4(0.0, 0.0, -2.0, 1.0); }
    return o;
}

// `w` moved toward the eye by `px` physical pixels' worth of world length
// there (along the eye ray: it stays at the same place on screen).
fn toward_eye(w: vec3<f32>, px: f32) -> vec3<f32> {
    let persp = g.camera_pos.w > 0.5;
    let fwd = g.camera_fwd.xyz;
    let u = normalize(cross(fwd, select(vec3(0.0, 0.0, 1.0), vec3(1.0, 0.0, 0.0), abs(fwd.z) > 0.9)));
    let p0 = g.view_proj * vec4(w, 1.0);
    let p1 = g.view_proj * vec4(w + u, 1.0);
    let per_unit = length((p1.xy / p1.w - p0.xy / p0.w) * g.viewport.xy * 0.5);
    let dir = select(-fwd, normalize(g.camera_pos.xyz - w), persp);
    return w + dir * (px / max(per_unit, 1e-9));
}

fn line_vertex(index: u32, l: LineIn, state_mask: u32) -> LineOut {
    let slot = index / 6u;
    let vi = index % 6u;
    var ts = array<f32, 6>(0.0, 0.0, 1.0, 0.0, 1.0, 1.0);
    var ss = array<f32, 6>(-1.0, 1.0, 1.0, -1.0, 1.0, -1.0);
    let t = ts[vi];
    let side = ss[vi];

    let half_vp = g.viewport.xy * 0.5;
    let pick = global_pick(l.pick);
    let st = state_of(pick) & state_mask;
    var width = l.width * inst.extra.x + g.viewport.w;
    // Live (selection/03, 06b, 07b): highlighted edges and face outlines
    // put ~3.5–4.5 px of colour down (4 px here at the default width).
    let lit = (st & (HOVER | SELECTED | ERROR)) != 0u;
    // From at most a part edge's width: a 2 px sketch line highlights like
    // an edge (live sketch/21, 22: the same 5 rows as selection/06b).
    if (lit) { width = (min(l.width, 1.25) * inst.extra.x + g.viewport.w) * 1.8 + 2.2; }
    // Live (behaviours "psui (10-27, interaction 1.12)", light theme): a
    // highlighted face's boundary is a flat 3 px band; a part edge hovered
    // itself an 8 px band (dark outlines round a pale core), selected 5 px.
    let face_band = lit && (st & OUTLINE) != 0u;
    let own_edge = (l.style & 64u) != 0u && (st & (HOVER | SELECTED)) != 0u && !face_band;
    if (face_band) { width = 3.0; }
    if (own_edge) { width = select(5.0, 8.0, (st & HOVER) != 0u); }
    let half = max(width * g.viewport.z * 0.5, 0.5);

    var wa = (inst.model * vec4(l.a, 1.0)).xyz;
    var wb = (inst.model * vec4(l.b, 1.0)).xyz;
    // A highlighted line comes toward the eye by its half width (same place
    // on screen), so its own faces' depth slope doesn't eat the face side of
    // the band (live selection/06b: centred on the edge).
    // Lifted lines too (a hovered part's silhouette over its own faces).
    if (lit || (l.style & 32u) != 0u) {
        wa = toward_eye(wa, 1.5 * half);
        wb = toward_eye(wb, 1.5 * half);
    }
    var ca = g.view_proj * vec4(wa, 1.0);
    var cb = g.view_proj * vec4(wb, 1.0);
    ca = clip_near(ca, cb);
    cb = clip_near(cb, ca);

    let sa = ca.xy / ca.w * half_vp;
    let sb = cb.xy / cb.w * half_vp;
    var d = sb - sa;
    let len = length(d);
    if (len < 1e-5) { d = vec2(1.0, 0.0); } else { d = d / len; }
    let nrm = vec2(-d.y, d.x);

    var c = select(ca, cb, t > 0.5);
    let ext = select(-1.0, 1.0, t > 0.5);
    var off_px = nrm * side * half;
    if (len > 1e-3) {
        // The ends extend along the segment itself, depth included: a wider
        // line's depth then matches a narrower one's at the same pixel (a lit
        // plane border over the plane's own was behind it on far edges).
        c = mix(ca, cb, t + ext * half * 0.5 / len);
    } else {
        off_px += d * ext * half * 0.5;
    }
    c = vec4(c.xy + off_px / half_vp * c.w, c.z, c.w);

    if ((l.style & 2u) != 0u) {
        c.z = c.w; // on top: nearest depth
    } else {
        // At the edge's own depth, offset like Onshape's (27 - 15 units).
        c = in_front_of_faces(c, 12.0);
        // Style bit 5 (lifted): a little nearer still (1e-5 of the depth;
        // the units are 0 with nothing but planes shown).
        if ((l.style & 32u) != 0u) { c.z = c.z * (1.0 + 1e-5); }
    }

    var o: LineOut;
    o.slot = slot;
    o.pos = c;
    var base: vec3<f32>;
    var alpha: f32;
    if ((inst.params.x & 1u) != 0u) { base = srgb_to_linear(l.color.rgb); alpha = l.color.a; } else { base = inst.color.rgb; alpha = inst.color.a; }
    if ((st & DIM) != 0u) { alpha = alpha * 0.3; }
    // A selected part's edges let its amber through (live ins-902: the
    // top/front edge peaks at 0.73 of the faces, ours at 0.53 when opaque).
    if ((st & (BODY | HOVER | SELECTED)) == BODY) { alpha = alpha * 0.68; }
    o.color = vec4(highlight_stroke(base, st), alpha);
    o.pick = pick;
    o.along = t * len;
    o.world = mix(wa, wb, t);
    o.style = l.style;
    o.across = side * half;
    // 1: a hovered part edge, 2: a selected one (fs_line draws the band).
    o.core = select(0.0, select(2.0, 1.0, (st & HOVER) != 0u), own_edge);
    if ((st & HIDDEN) != 0u) { o.pos = vec4(0.0, 0.0, -2.0, 1.0); }
    return o;
}

@fragment
fn fs_line(i: LineOut) -> @location(0) vec4<f32> {
    load_inst(i.slot);
    if (clipped(i.world)) { discard; }
    if ((i.style & 1u) != 0u && fract(i.along / (7.0 * g.viewport.z)) > 0.55) { discard; }
    if ((i.style & 4u) != 0u) {
        let t = fract(i.along / (16.0 * g.viewport.z));
        if (t > 0.55 && (t < 0.68 || t > 0.8)) { discard; }
    }
    // Inferred direction over a rubber band: 4 px on / 4 off from 1.25 px.
    if ((i.style & 16u) != 0u && fract((i.along / g.viewport.z - 1.25) / 8.0) >= 0.5) { discard; }
    // Construction lines: dash, gap, dash, gap (Graphics preferences).
    if ((i.style & 8u) != 0u) {
        let period = max(g.dash.x + g.dash.y + g.dash.z + g.dash.w, 1.0);
        let t = fract(i.along / (period * g.viewport.z)) * period;
        if ((t > g.dash.x && t < g.dash.x + g.dash.y) || t > g.dash.x + g.dash.y + g.dash.z) { discard; }
    }
    // A hovered / selected part edge (live psui-light-05..07, sRGB, from
    // the centre out in logical px): hovered, a pale core (255,211,160),
    // the hover colour, then a dark outline (26,35,40) on both sides;
    // selected, a darker core (203,138,15) shading out to the selection
    // colour, no outline.
    if (i.core > 0.0) {
        let a = abs(i.across) / max(g.viewport.z, 1.0);
        var c: vec3<f32>;
        if (i.core < 1.5) {
            if (a > 2.75) { c = vec3(26.0, 35.0, 40.0) / 255.0; } else if (a > 1.25) { c = linear_to_srgb(i.color.rgb); } else { c = vec3(255.0, 211.0, 160.0) / 255.0; }
        } else {
            if (a > 1.5) { c = linear_to_srgb(i.color.rgb); } else if (a > 0.5) { c = vec3(217.0, 156.0, 35.0) / 255.0; } else { c = vec3(203.0, 138.0, 15.0) / 255.0; }
        }
        return vec4(c, i.color.a);
    }
    return enc(i.color);
}

@fragment
fn fs_line_pick(i: LineOut) -> @location(0) u32 {
    load_inst(i.slot);
    if (clipped(i.world) || (state_of(i.pick) & NO_PICK) != 0u) { discard; }
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
    @location(5) @interpolate(flat) slot: u32,
    // The point's radius in device pixels (its quad reaches 0.75 px further
    // for the anti-aliased rim).
    @location(6) @interpolate(flat) rpx: f32,
};

@vertex
fn vs_point(@builtin(vertex_index) index: u32, p: PointIn) -> PointOut {
    // Six vertices per point and slot.
    let slot = index / 6u;
    let vi = index % 6u;
    load_inst(slot);
    var corners = array<vec2<f32>, 6>(
        vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0),
        vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0));
    let uv = corners[vi];
    let w = (inst.model * vec4(p.p, 1.0)).xyz;
    var c = g.view_proj * vec4(w, 1.0);
    let pick = global_pick(p.pick);
    let st = state_of(pick);
    var size = p.size + g.viewport.w;
    // Live (behaviours "icons2 plane/origin highlight"): the hovered or
    // selected origin (style 4) is a 13 px filled disc; other points grow.
    if ((st & (HOVER | SELECTED)) != 0u) { size = select(size * 1.5 + 1.0, size + 2.0, (p.style & 4u) != 0u); }
    let half = size * g.viewport.z * 0.5;
    c = vec4(c.xy + uv * (half + 0.75) / (g.viewport.xy * 0.5) * c.w, c.z, c.w);
    // Onshape's offsets: points 9 units, faces 27.
    if ((p.style & 2u) != 0u) { c.z = c.w; } else { c = in_front_of_faces(c, 18.0); }
    var o: PointOut;
    o.slot = slot;
    o.pos = c;
    var base: vec3<f32>;
    var alpha: f32;
    if ((inst.params.x & 1u) != 0u) { base = srgb_to_linear(p.color.rgb); alpha = p.color.a; } else { base = inst.color.rgb; alpha = inst.color.a; }
    o.color = vec4(highlight_stroke(base, st), alpha);
    o.pick = pick;
    o.uv = uv;
    o.world = w;
    o.style = p.style;
    o.rpx = half;
    if ((st & HIDDEN) != 0u) { o.pos = vec4(0.0, 0.0, -2.0, 1.0); }
    return o;
}

/// Distance from a point sprite's centre in device pixels.
fn point_dist(uv: vec2<f32>, rpx: f32) -> f32 {
    return length(uv) * (rpx + 0.75);
}

@fragment
fn fs_point(i: PointOut) -> @location(0) vec4<f32> {
    load_inst(i.slot);
    // An anti-aliased disc (live sketch/51: a 50 % rim around the dot).
    let d = point_dist(i.uv, i.rpx);
    let coverage = clamp(i.rpx - d + 0.5, 0.0, 1.0);
    if (coverage <= 0.0 || clipped(i.world)) { discard; }
    // Inner edges anti-aliased like the rim (live part-studio/00: the
    // origin's ring and dot are soft-edged).
    var cov = coverage;
    var rgb = i.color.rgb;
    // Style bit 4: a ring with a center dot (the origin: live 11 px, the
    // ring's inside at 0.65 of the radius, the dot 0.32).
    if ((i.style & 4u) != 0u && (state_of(i.pick) & (HOVER | SELECTED)) == 0u) {
        let ring = clamp(d - 0.65 * i.rpx + 0.5, 0.0, 1.0);
        let dot = clamp(0.32 * i.rpx - d + 0.5, 0.0, 1.0);
        cov = cov * max(ring, dot);
    }
    // Style bit 8: a hollow point (white inside a ring of the point color),
    // Onshape's standalone sketch points and spline handles.
    // Bit 32 makes the ring thicker (live p1audit skip-02: pattern instance
    // points, white inside 0.45 of the radius).
    if ((i.style & 8u) != 0u) {
        let inner = select(0.55, 0.45, (i.style & 32u) != 0u);
        rgb = mix(rgb, vec3<f32>(1.0), clamp(inner * i.rpx - d + 0.5, 0.0, 1.0));
    }
    // Style bit 16: a ring only, see-through inside (live midpoint markers:
    // a 9 px ring with a 1.75 px stroke, inner radius 0.6).
    if ((i.style & 16u) != 0u) { cov = cov * clamp(d - 0.61 * i.rpx + 0.5, 0.0, 1.0); }
    if (cov <= 0.0) { discard; }
    return vec4<f32>(linear_to_srgb(rgb), i.color.a * cov);
}

@fragment
fn fs_point_pick(i: PointOut) -> @location(0) u32 {
    load_inst(i.slot);
    if (point_dist(i.uv, i.rpx) > i.rpx || clipped(i.world) || (state_of(i.pick) & NO_PICK) != 0u) { discard; }
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
    @location(2) @interpolate(flat) slot: u32,
};

@vertex
fn vs_image(v: ImageIn, @builtin(instance_index) slot: u32) -> ImageOut {
    load_inst(slot);
    var o: ImageOut;
    o.slot = slot;
    let w = inst.model * vec4(v.pos, 1.0);
    o.world = w.xyz;
    o.pos = g.view_proj * w;
    o.uv = v.uv;
    return o;
}

// The texture is sRGB (samples are linear like the instance color) with
// alpha premultiplied in linear space; the result is premultiplied in the
// encoded values (premultiplied blending in the target's sRGB values, as
// WebGL blends a canvas).
fn image_color(i: ImageOut) -> vec4<f32> {
    let t = textureSample(image_tex, image_sampler, i.uv);
    if (clipped(i.world)) { discard; }
    // A decal's face runs past its image (upload_textured).
    if (any(i.uv < vec2(0.0)) || any(i.uv > vec2(1.0))) { discard; }
    let a = inst.color.a;
    let straight = select(vec3(0.0), t.rgb / max(t.a, 1e-6), t.a > 0.0);
    var c = linear_to_srgb(straight * inst.color.rgb);
    // params.x bit 3: a decal on a selected face, 0.4 toward the selection
    // amber (255,170,10) in sRGB (live).
    if ((inst.params.x & 8u) != 0u) { c = mix(c, vec3(1.0, 170.0 / 255.0, 10.0 / 255.0), 0.4); }
    return vec4(c * t.a * a, t.a * a);
}

// Opaque instances draw twice: mostly opaque texels first, writing depth...
@fragment
fn fs_image_opaque(i: ImageOut) -> @location(0) vec4<f32> {
    load_inst(i.slot);
    let c = image_color(i);
    if (c.a < 0.5) { discard; }
    return c;
}

// ...then the soft (anti-aliased / translucent) texels, blended without
// writing depth.
@fragment
fn fs_image_soft(i: ImageOut) -> @location(0) vec4<f32> {
    load_inst(i.slot);
    let c = image_color(i);
    if (c.a >= 0.5 || c.a <= 0.0) { discard; }
    return c;
}

// Faded instances (instance alpha < 1): all texels blended, no depth write.
@fragment
fn fs_image_blend(i: ImageOut) -> @location(0) vec4<f32> {
    load_inst(i.slot);
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
    load_inst(i.slot);
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
    return enc(vec4(c, base.a));
}
