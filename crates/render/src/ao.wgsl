// Screen-space ambient occlusion (Onshape's Graphics preferences "Ambient
// occlusion"): a full-screen pass after the color pass that darkens the
// resolved image where nearby geometry shades a surface (corners, creases,
// contacts). The result is multiplied into the color target.
// DEPTH_TYPE is replaced with the depth texture type (multisampled or not).

struct U {
    inv_vp: mat4x4<f32>,
    p: vec4<f32>,            // width, height (device px), device pixel ratio, strength
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var depth_tex: DEPTH_TYPE;

fn depth_at(px: vec2<i32>) -> f32 {
    let c = clamp(px, vec2<i32>(0, 0), vec2<i32>(i32(u.p.x) - 1, i32(u.p.y) - 1));
    return textureLoad(depth_tex, c, 0);
}

fn world_at(px: vec2<i32>, d: f32) -> vec3<f32> {
    let ndc = vec2((f32(px.x) + 0.5) / u.p.x * 2.0 - 1.0, 1.0 - (f32(px.y) + 0.5) / u.p.y * 2.0);
    let w = u.inv_vp * vec4(ndc, d, 1.0);
    return w.xyz / w.w;
}

@vertex
fn vs_ao(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(p[vi], 0.0, 1.0);
}

@fragment
fn fs_ao(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    let d = depth_at(p);
    // Reverse-Z: 0 is the cleared background.
    if (d <= 0.0) { return vec4(1.0); }
    let c = world_at(p, d);
    let cx = world_at(p + vec2(1, 0), depth_at(p + vec2(1, 0)));
    let cy = world_at(p + vec2(0, 1), depth_at(p + vec2(0, 1)));
    var n = cross(cx - c, cy - c);
    if (dot(n, n) < 1e-20) { return vec4(1.0); }
    n = normalize(n);
    // Face the camera: towards the near end of this pixel's ray.
    let toward = world_at(p, 1.0) - world_at(p, 0.0);
    if (dot(n, toward) < 0.0) { n = -n; }
    let px_world = max(length(cx - c), 1e-9);
    let radius_px = 16.0 * u.p.z;
    let r = radius_px * px_world;
    // A spiral of samples, rotated per pixel to hide banding.
    let noise = fract(sin(dot(pos.xy, vec2(12.9898, 78.233))) * 43758.5453) * 6.2831853;
    var occ = 0.0;
    let count = 12;
    for (var i = 0; i < count; i = i + 1) {
        let fi = f32(i);
        let a = fi * 2.3999632 + noise;
        let rad = radius_px * (fi + 0.5) / f32(count);
        let q = p + vec2<i32>(vec2(cos(a), sin(a)) * rad);
        let dq = depth_at(q);
        if (dq <= 0.0) { continue; }
        let v = world_at(q, dq) - c;
        let dist = length(v);
        if (dist < 1e-9) { continue; }
        let h = dot(v, n) / dist;
        let falloff = 1.0 - smoothstep(0.5 * r, r, dist);
        occ = occ + max(h - 0.1, 0.0) * falloff;
    }
    let ao = clamp(1.0 - u.p.w * occ / f32(count) * 2.2, 0.0, 1.0);
    return vec4(ao, ao, ao, 1.0);
}
