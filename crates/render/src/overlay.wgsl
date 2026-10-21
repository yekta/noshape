// Screen overlays (overlay.rs): triangles and text masks rendered with 4x
// MSAA into an 8-bit UNORM layer (premultiplied, blended in gamma space like
// GPUI's paths), then composited over the frame.

struct OverlayGlobals {
    // Device-pixel origin and size of the overlay layer within the frame.
    origin: vec2<f32>,
    size: vec2<f32>,
    // x: 1 = GPUI's (wgpu) text contrast and gamma correction; y: its
    // grayscale enhanced contrast.
    shading: vec4<f32>,
    gamma_ratios: vec4<f32>,
    // x: 1 = clip path pixels to the path's bounds (GPUI's wgpu path shader).
    flags: vec4<f32>,
}

@group(0) @binding(0) var<uniform> g: OverlayGlobals;

fn to_clip(p: vec2<f32>) -> vec4<f32> {
    let t = (p - g.origin) / g.size;
    return vec4<f32>(t.x * 2.0 - 1.0, 1.0 - t.y * 2.0, 0.0, 1.0);
}

// --- filled paths ---

struct FillOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) @interpolate(flat) bounds: vec4<f32>,
}

@vertex
fn vs_fill(@location(0) position: vec2<f32>, @location(1) color: vec4<f32>, @location(2) bounds: vec4<f32>) -> FillOut {
    var out: FillOut;
    out.position = to_clip(position);
    out.color = color;
    out.bounds = bounds;
    return out;
}

@fragment
fn fs_fill(input: FillOut) -> @location(0) vec4<f32> {
    // GPUI's wgpu path shader clips pixel centers to the path's bounds
    // (measured: a center on the right or bottom bound is dropped, one on
    // the left or top bound kept).
    let p = input.position.xy + g.origin;
    if (g.flags.x > 0.5 && (any(p < input.bounds.xy) || any(p >= input.bounds.zw))) {
        return vec4<f32>(0.0);
    }
    return input.color;
}

// --- quads and shadows (GPUI's `fs_quad` / `fs_shadow`, solid colors) ---
// Ported from gpui's wgpu shaders.wgsl (Apache-2.0): same signed distances,
// antialiasing threshold, border blending and 4-sample gaussian shadow, so
// the pixels match GPUI's. One quad per instance; `bounds` in frame device
// px, colors straight (output premultiplied for the layer).

struct QuadIn {
    @location(0) bounds: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) border_color: vec4<f32>,
    // x corner radius, y border width (quads) or blur radius (shadows).
    @location(3) params: vec4<f32>,
}

struct QuadOut {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) bounds: vec4<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>,
    @location(2) @interpolate(flat) border_color: vec4<f32>,
    @location(3) @interpolate(flat) params: vec4<f32>,
}

fn quad_vertex(i: u32, geometry: vec4<f32>, q: QuadIn) -> QuadOut {
    let corner = vec2<f32>(f32(i & 1u), 0.5 * f32(i & 2u));
    var out: QuadOut;
    out.position = to_clip(mix(geometry.xy, geometry.zw, corner));
    out.bounds = q.bounds;
    out.color = q.color;
    out.border_color = q.border_color;
    out.params = q.params;
    return out;
}

@vertex
fn vs_quad(@builtin(vertex_index) i: u32, q: QuadIn) -> QuadOut {
    return quad_vertex(i, q.bounds, q);
}

@vertex
fn vs_shadow(@builtin(vertex_index) i: u32, q: QuadIn) -> QuadOut {
    // Room for the gaussian tail outside the shadow rect.
    let margin = 3.0 * q.params.y;
    return quad_vertex(i, q.bounds + vec4<f32>(-margin, -margin, margin, margin), q);
}

fn premultiplied(color: vec4<f32>, alpha_factor: f32) -> vec4<f32> {
    let a = color.a * alpha_factor;
    return vec4<f32>(color.rgb * a, a);
}

fn quad_sdf_impl(corner_center_to_point: vec2<f32>, corner_radius: f32) -> f32 {
    if (corner_radius == 0.0) {
        return max(corner_center_to_point.x, corner_center_to_point.y);
    }
    let signed_distance_to_inset_quad = length(max(vec2<f32>(0.0), corner_center_to_point)) + min(0.0, max(corner_center_to_point.x, corner_center_to_point.y));
    return signed_distance_to_inset_quad - corner_radius;
}

fn over(below: vec4<f32>, above: vec4<f32>) -> vec4<f32> {
    let alpha = above.a + below.a * (1.0 - above.a);
    let color = (above.rgb * above.a + below.rgb * below.a * (1.0 - above.a)) / alpha;
    return vec4<f32>(color, alpha);
}

@fragment
fn fs_quad(input: QuadOut) -> @location(0) vec4<f32> {
    let background_color = input.color;
    let corner_radius = input.params.x;
    let border_width = input.params.y;
    if (border_width == 0.0 && corner_radius == 0.0) {
        return premultiplied(background_color, 1.0);
    }
    let origin = input.bounds.xy;
    let size = input.bounds.zw - input.bounds.xy;
    let half_size = size / 2.0;
    let point = input.position.xy + g.origin - origin;
    let center_to_point = point - half_size;
    let antialias_threshold = 0.5;
    let border = vec2<f32>(border_width);
    let reduced_border = vec2<f32>(select(border.x, -antialias_threshold, border.x == 0.0), select(border.y, -antialias_threshold, border.y == 0.0));
    let corner_to_point = abs(center_to_point) - half_size;
    let corner_center_to_point = corner_to_point + corner_radius;
    let is_near_rounded_corner = corner_center_to_point.x >= 0.0 && corner_center_to_point.y >= 0.0;
    let straight_border_inner_corner_to_point = corner_to_point + reduced_border;
    let is_beyond_inner_straight_border = straight_border_inner_corner_to_point.x > 0.0 || straight_border_inner_corner_to_point.y > 0.0;
    let is_within_inner_straight_border = straight_border_inner_corner_to_point.x < -antialias_threshold && straight_border_inner_corner_to_point.y < -antialias_threshold;
    if (is_within_inner_straight_border && !is_near_rounded_corner) {
        return premultiplied(background_color, 1.0);
    }
    let outer_sdf = quad_sdf_impl(corner_center_to_point, corner_radius);
    var inner_sdf = 0.0;
    if (corner_center_to_point.x <= 0.0 || corner_center_to_point.y <= 0.0) {
        inner_sdf = -max(straight_border_inner_corner_to_point.x, straight_border_inner_corner_to_point.y);
    } else if (is_beyond_inner_straight_border) {
        inner_sdf = -1.0;
    } else {
        // Equal borders all round: the circular inner edge.
        inner_sdf = -(outer_sdf + reduced_border.x);
    }
    let border_sdf = max(inner_sdf, outer_sdf);
    var color = background_color;
    if (border_sdf < antialias_threshold) {
        let blended_border = over(background_color, input.border_color);
        color = mix(background_color, blended_border, saturate(antialias_threshold - inner_sdf));
    }
    return premultiplied(color, saturate(antialias_threshold - outer_sdf));
}

const M_PI_F: f32 = 3.1415926;

fn gaussian(x: f32, sigma: f32) -> f32 {
    return exp(-(x * x) / (2.0 * sigma * sigma)) / (sqrt(2.0 * M_PI_F) * sigma);
}

fn erf(v: vec2<f32>) -> vec2<f32> {
    let s = sign(v);
    let a = abs(v);
    let r1 = 1.0 + (0.278393 + (0.230389 + (0.000972 + 0.078108 * a) * a) * a) * a;
    let r2 = r1 * r1;
    return s - s / (r2 * r2);
}

fn blur_along_x(x: f32, y: f32, sigma: f32, corner: f32, half_size: vec2<f32>) -> f32 {
    let delta = min(half_size.y - corner - abs(y), 0.0);
    let curved = half_size.x - corner + sqrt(max(0.0, corner * corner - delta * delta));
    let integral = 0.5 + 0.5 * erf((x + vec2<f32>(-curved, curved)) * (sqrt(0.5) / sigma));
    return integral.y - integral.x;
}

@fragment
fn fs_shadow(input: QuadOut) -> @location(0) vec4<f32> {
    let corner_radius = input.params.x;
    let blur_radius = input.params.y;
    let half_size = (input.bounds.zw - input.bounds.xy) / 2.0;
    let center = input.bounds.xy + half_size;
    let p = input.position.xy + g.origin;
    let center_to_point = p - center;
    var alpha: f32;
    if (blur_radius == 0.0) {
        let corner_to_point = abs(center_to_point) - half_size;
        let distance = quad_sdf_impl(corner_to_point + corner_radius, corner_radius);
        alpha = saturate(0.5 - distance);
    } else {
        let low = center_to_point.y - half_size.y;
        let high = center_to_point.y + half_size.y;
        let start = clamp(-3.0 * blur_radius, low, high);
        let end = clamp(3.0 * blur_radius, low, high);
        let step = (end - start) / 4.0;
        var y = start + step * 0.5;
        alpha = 0.0;
        for (var i = 0; i < 4; i += 1) {
            let blur = blur_along_x(center_to_point.x, center_to_point.y - y, blur_radius, corner_radius, half_size);
            alpha += blur * gaussian(y, blur_radius) * step;
            y += step;
        }
    }
    return premultiplied(input.color, alpha);
}

// --- text masks (GPUI's monochrome sprites) ---

@group(1) @binding(0) var t_mask: texture_2d<f32>;
@group(1) @binding(1) var s_mask: sampler;

struct MaskOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_mask(@location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>) -> MaskOut {
    var out: MaskOut;
    out.position = to_clip(position);
    out.uv = uv;
    out.color = color;
    return out;
}

// Contrast and gamma correction as in GPUI's wgpu shaders (adapted there from
// https://github.com/microsoft/terminal/blob/1283c0f5b99a2961673249fa77c6b986efb5086c/src/renderer/atlas/dwrite.hlsl,
// Copyright (c) Microsoft Corporation, MIT license).
fn color_brightness(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.30, 0.59, 0.11));
}

fn light_on_dark_contrast(enhanced_contrast: f32, color: vec3<f32>) -> f32 {
    let brightness = color_brightness(color);
    let multiplier = saturate(4.0 * (0.75 - brightness));
    return enhanced_contrast * multiplier;
}

fn enhance_contrast(alpha: f32, k: f32) -> f32 {
    return alpha * (k + 1.0) / (alpha * k + 1.0);
}

fn apply_alpha_correction(a: f32, b: f32, gr: vec4<f32>) -> f32 {
    let brightness_adjustment = gr.x * b + gr.y;
    let correction = brightness_adjustment * a + (gr.z * b + gr.w);
    return a + a * (1.0 - a) * correction;
}

fn apply_contrast_and_gamma_correction(sample: f32, color: vec3<f32>, enhanced_contrast_factor: f32, gamma_ratios: vec4<f32>) -> f32 {
    let enhanced_contrast = light_on_dark_contrast(enhanced_contrast_factor, color);
    let brightness = color_brightness(color);
    let contrasted = enhance_contrast(sample, enhanced_contrast);
    return apply_alpha_correction(contrasted, brightness, gamma_ratios);
}

@fragment
fn fs_mask(input: MaskOut) -> @location(0) vec4<f32> {
    let sample = textureSample(t_mask, s_mask, input.uv).r;
    var coverage = sample;
    if (g.shading.x > 0.5) {
        coverage = apply_contrast_and_gamma_correction(sample, input.color.rgb, g.shading.y, g.gamma_ratios);
    }
    let a = input.color.a * coverage;
    return vec4<f32>(input.color.rgb * a, a);
}

// --- compositing the layer over the frame ---

@group(1) @binding(0) var t_layer: texture_2d<f32>;

struct CompositeOut {
    @builtin(position) position: vec4<f32>,
}

// `g.size` here is the frame's size and `g.origin` the layer's origin in it;
// `g.shading.zw` the layer's size.
@vertex
fn vs_composite(@builtin(vertex_index) i: u32) -> CompositeOut {
    let corner = vec2<f32>(f32(i & 1u), f32((i >> 1u) & 1u));
    let p = g.origin + corner * g.shading.zw;
    let t = p / g.size;
    var out: CompositeOut;
    out.position = vec4<f32>(t.x * 2.0 - 1.0, 1.0 - t.y * 2.0, 0.0, 1.0);
    return out;
}

@fragment
fn fs_composite(input: CompositeOut) -> @location(0) vec4<f32> {
    let p = vec2<i32>(floor(input.position.xy - g.origin));
    return textureLoad(t_layer, p, 0);
}
