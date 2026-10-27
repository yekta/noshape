// A hovered whole part's outline (live behaviours "psui (2026-10-27, P2) PART-HOVER-RED look", psui/parthover
// crops): the silhouette line along its image's boundary redrawn in the hover colour, solid, no antialiasing, its two
// pixels at 1× (the part's boundary pixel and the one outside it); the faces keep their colours. A full-screen pass
// over the resolved color reading the mask of the hovered part's visible pixels (`fs_hover_mask` in shaders.wgsl,
// resolved: partial coverage at the boundary).

struct U {
    color: vec4<f32>,        // sRGB hover colour
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
    for (var dy = -n; dy <= n; dy++) {
        for (var dx = -n; dx <= n; dx++) {
            if (f32(dx * dx + dy * dy) > r * r) { continue; }
            let q = p + vec2(dx, dy);
            if (any(q < vec2(0)) || any(q >= size)) { continue; }
            if ((textureLoad(mask, q, 0).r > 0.0) != inside) { return u.color; }
        }
    }
    discard;
}
