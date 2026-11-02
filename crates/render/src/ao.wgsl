// Screen-space ambient occlusion as Onshape draws it (webgl bundle: the "occlusion" pass into a half-size buffer,
// "occlusionFilter", then the faces' `rgb *= pow(1 - occlusion, uOcclusionFactor)`): here after the color pass, its
// result multiplied into the resolved image. Normals come from the depth (Onshape draws a normal-depth pass).
// DEPTH_TYPE is replaced with the depth texture type (multisampled or not).

struct U {
    inv_vp: mat4x4<f32>,
    // full width, full height (device px), occlusion buffer width, height
    p: vec4<f32>,
    // ring radius (occlusion buffer px), the scene's depth extent, uOcclusionFactor
    q: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var depth_tex: DEPTH_TYPE;
@group(0) @binding(2) var occ_tex: texture_2d<f32>;
@group(0) @binding(3) var occ_samp: sampler;

// AMBIENT_OCCLUSION_INTENSITY, _BIAS, _SCALE and the shader's minResolution.
const INTENSITY: f32 = 1.0;
const BIAS: f32 = 0.1;
const SCALE: f32 = 24.0;
const MIN_RESOLUTION: f32 = 2e-4;

fn depth_at(px: vec2<i32>) -> f32 {
    let c = clamp(px, vec2<i32>(0, 0), vec2<i32>(i32(u.p.x) - 1, i32(u.p.y) - 1));
    return textureLoad(depth_tex, c, 0);
}

fn world_at(px: vec2<i32>, d: f32) -> vec3<f32> {
    let ndc = vec2((f32(px.x) + 0.5) / u.p.x * 2.0 - 1.0, 1.0 - (f32(px.y) + 0.5) / u.p.y * 2.0);
    let w = u.inv_vp * vec4(ndc, d, 1.0);
    return w.xyz / w.w;
}

fn point_at(px: vec2<i32>) -> vec3<f32> {
    return world_at(px, depth_at(px));
}

@vertex
fn vs_ao(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(p[vi], 0.0, 1.0);
}

// Onshape's getOcclusion.
fn occlusion_from(px: vec2<i32>, c: vec3<f32>, n: vec3<f32>) -> f32 {
    let d = depth_at(px);
    // Reverse-Z: 0 is the cleared background.
    if (d <= 0.0) { return 0.0; }
    let diff = world_at(px, d) - c;
    let len = length(diff);
    if (len < 1e-12) { return 0.0; }
    let dist = SCALE * len / u.q.y;
    return INTENSITY * max(0.0, (dot(n, diff) - MIN_RESOLUTION * u.q.y) / len - BIAS) / (1.0 + dist * dist);
}

// The occlusion buffer: five rings of five samples, each ring turned half a step on, all turned by a random angle.
@fragment
fn fs_occlusion(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let scale = u.p.xy / u.p.zw;
    let p = vec2<i32>(floor(pos.xy * scale));
    let d = depth_at(p);
    if (d <= 0.0) { return vec4(0.0); }
    let c = world_at(p, d);
    // The normal from the nearer neighbour each way (no step across an edge).
    let l = point_at(p - vec2(1, 0));
    let r = point_at(p + vec2(1, 0));
    let t = point_at(p - vec2(0, 1));
    let b = point_at(p + vec2(0, 1));
    let toward = world_at(p, 1.0) - world_at(p, 0.0);
    let ax = select(r - c, c - l, abs(dot(c - l, toward)) < abs(dot(r - c, toward)));
    let ay = select(b - c, c - t, abs(dot(c - t, toward)) < abs(dot(b - c, toward)));
    var n = cross(ax, ay);
    if (dot(n, n) < 1e-30) { return vec4(0.0); }
    n = normalize(n);
    if (dot(n, toward) < 0.0) { n = -n; }
    let random = fract(sin(dot(floor(pos.xy), vec2(12.9898, 78.233))) * 43758.5453);
    let radius_max = u.q.x;
    let radius_min = radius_max / 5.0;
    let step = 6.2831853 / 5.0;
    var angle = 6.2831853 * random;
    var occ = 0.0;
    for (var i = 0; i < 5; i = i + 1) {
        let radius = radius_min * f32(i + 1);
        for (var j = 0; j < 5; j = j + 1) {
            let o = radius * vec2(cos(angle), sin(angle)) * scale;
            occ = occ + occlusion_from(p + vec2<i32>(round(o)), c, n);
            angle = angle + step;
        }
        angle = angle - step / 2.0;
    }
    return vec4(occ / 25.0, 0.0, 0.0, 1.0);
}

// "occlusionFilter": a 5 x 5 blur weighted 1 / (1 + |offset|^2).
@fragment
fn fs_filter(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    let size = vec2<i32>(u.p.zw);
    var occ = 0.0;
    var count = 0.0;
    for (var x = -2; x <= 2; x = x + 1) {
        for (var y = -2; y <= 2; y = y + 1) {
            let q = clamp(p + vec2(x, y), vec2(0, 0), size - 1);
            let w = 1.0 / (1.0 + f32(x * x + y * y));
            occ = occ + w * textureLoad(occ_tex, q, 0).r;
            count = count + w;
        }
    }
    return vec4(occ / count, 0.0, 0.0, 1.0);
}

// The faces' darkening, multiplied into the image.
@fragment
fn fs_apply(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let occ = textureSampleLevel(occ_tex, occ_samp, pos.xy / u.p.xy, 0.0).r;
    let k = pow(max(1.0 - occ, 0.0), u.q.z);
    return vec4(k, k, k, 1.0);
}
