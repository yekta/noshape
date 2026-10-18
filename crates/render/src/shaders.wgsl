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
    if (g.flags.x == 0u || inst.params.y != 0u) { return false; }
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
    // Key light: slightly above and left of the viewer (headlight-style, so
    // shading is consistent as the model rotates, like most CAD tools).
    let key = normalize(v + g.camera_up.xyz * 0.45 - right * 0.25);
    let fill = normalize(v - g.camera_up.xyz * 0.3 + right * 0.6);
    let diff = max(dot(n, key), 0.0) * 0.72 + max(dot(n, fill), 0.0) * 0.18;
    // Hemispheric ambient (world Z up).
    let hemi = mix(0.16, 0.30, n.z * 0.5 + 0.5);
    let h = normalize(key + v);
    let spec = pow(max(dot(n, h), 0.0), 48.0) * 0.28;
    let rim = pow(1.0 - max(dot(n, v), 0.0), 3.0) * 0.08;
    return base * (diff + hemi) + vec3(spec + rim);
}

@fragment
fn fs_mesh(i: MeshOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let st = state_of(i.pick);
    if ((st & HIDDEN) != 0u || clipped(i.world)) { discard; }
    // Per-primitive instances color each vertex (analysis maps, face
    // appearances); vertices with alpha 0 keep the instance color.
    var own = inst.color.rgb;
    if (inst.params.x != 0u && i.color.a > 0.0) { own = srgb_to_linear(i.color.rgb); }
    var base = highlight(own, st);
    var c = shade(i.world, i.normal, front, base);
    // Back faces revealed by a section cut are tinted so cut solids read well.
    if (!front && g.flags.x != 0u) { c = mix(c, g.accent.rgb * 0.5, 0.5); }
    var a = inst.color.a;
    if ((st & DIM) != 0u) { a = a * 0.25; }
    return vec4(c, a);
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
    o.color = vec4(highlight(base, st), alpha);
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
    o.color = vec4(highlight(base, st), alpha);
    o.pick = pick;
    o.uv = uv;
    o.world = w;
    if ((st & HIDDEN) != 0u) { o.pos = vec4(0.0, 0.0, -2.0, 1.0); }
    return o;
}

@fragment
fn fs_point(i: PointOut) -> @location(0) vec4<f32> {
    if (dot(i.uv, i.uv) > 1.0 || clipped(i.world)) { discard; }
    return i.color;
}

@fragment
fn fs_point_pick(i: PointOut) -> @location(0) u32 {
    if (dot(i.uv, i.uv) > 1.0 || clipped(i.world)) { discard; }
    if (inst.params.z == 0u) { discard; }
    return i.pick;
}
