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
}

@vertex
fn vs_fill(@location(0) position: vec2<f32>, @location(1) color: vec4<f32>) -> FillOut {
    var out: FillOut;
    out.position = to_clip(position);
    out.color = color;
    return out;
}

@fragment
fn fs_fill(input: FillOut) -> @location(0) vec4<f32> {
    return input.color;
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
