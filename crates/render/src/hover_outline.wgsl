// A hovered whole part's outline (live behaviours "psui (2026-10-27, P2) PART-HOVER-RED look", psui/parthover
// crops): the silhouette line along its image's boundary redrawn in the hover colour, solid, no antialiasing, its two
// pixels at 1× (the part's boundary pixel and the one outside it); the faces keep their colours. A full-screen pass
// over the resolved color reading the mask of the hovered part's pixels, hidden ones too (`fs_hover_mask` in shaders.wgsl,
// resolved: partial coverage at the boundary).

struct U {
    color: vec4<f32>,        // sRGB hover colour
    instance: vec4<f32>,     // sRGB, an instance selected through an entity (mask b)
    face: vec4<f32>,         // sRGB, beside a selected face seen (mask g)
    picked: vec4<f32>,       // sRGB and alpha, a mate pick's instance (mask a)
    p: vec4<f32>,            // width, height (device px), band reach (device px)
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var mask: texture_2d<f32>;

@vertex
fn vs_hover_outline(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(p[vi], 0.0, 1.0);
}

@fragment
fn fs_hover_outline(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    let size = vec2<i32>(i32(u.p.x), i32(u.p.y));
    let inside = textureLoad(mask, p, 0).r > 0.0;
    let r = u.p.z;
    let n = i32(ceil(r));
    // In the band when a pixel on the other side of the boundary lies within reach (the window's edge doesn't count).
    // Ends in a return: D3D's DXC rejects a function whose last statement is `discard` ("control may reach end of
    // non-void function"), which Vulkan and Metal accept.
    // The band's colour: beside a selected face seen, its boundary's; an entity-selected instance's outline its own
    // (live triad-depth-1122 t1: #ffa033 round the part, #ffa90a along the selected face).
    let own = textureLoad(mask, p, 0);
    var band = false;
    var face = own.g > 0.0;
    var instance = own.b > 0.0;
    var picked = own.a > 0.0;
    for (var dy = -n; dy <= n; dy++) {
        for (var dx = -n; dx <= n; dx++) {
            if (f32(dx * dx + dy * dy) > r * r) { continue; }
            let q = p + vec2(dx, dy);
            if (any(q < vec2(0)) || any(q >= size)) { continue; }
            let m = textureLoad(mask, q, 0);
            if ((m.r > 0.0) != inside) { band = true; }
            face = face || m.g > 0.0;
            instance = instance || m.b > 0.0;
            picked = picked || m.a > 0.0;
        }
    }
    // Beside a face whose boundary is its own line (a mate pick's, live workflows-1122 08 s07 / s08), the line stays.
    if (!band || (face && !instance)) { discard; }
    if (face) { return u.face; }
    if (instance) { return u.instance; }
    return select(u.color, u.picked, picked);
}
