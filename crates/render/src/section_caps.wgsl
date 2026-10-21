// Section cap outlines (see section_caps.rs): where the cap mask changes,
// a black edge, widened by a small separable blur (Onshape's edge detect
// and Gaussian filter passes, 8-bit like its buffers).

@group(0) @binding(0) var source: texture_2d<f32>;
// x, y = blur direction.
@group(0) @binding(1) var<uniform> params: vec4<f32>;

@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4(p * 2.0 - 1.0, 0.0, 1.0);
}

fn tap(p: vec2<i32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(source));
    return textureLoad(source, clamp(p, vec2(0), size - 1), 0);
}

// An edge pixel differs from at least 2 and fewer than 7 of its 8
// neighbours.
@fragment
fn fs_edges(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(floor(pos.xy));
    let c = tap(p);
    var n = 0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            if ((x != 0 || y != 0) && length(tap(p + vec2(x, y)) - c) > 0.001) { n = n + 1; }
        }
    }
    return vec4(select(0.0, 1.0, n >= 2 && n < 7));
}

@fragment
fn fs_blur(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(floor(pos.xy));
    let d = vec2<i32>(params.xy);
    let r = tap(p - d * 2).r * 0.07 + tap(p - d).r * 0.2 + tap(p).r * 0.7 + tap(p + d).r * 0.2 + tap(p + d * 2).r * 0.07;
    return vec4(r);
}
