// Glass effect shader
// Creates a frosted glass effect with:
// - Blur effect
// - Color tint
// - Distortion (optional)
// - Edge fading

// This is a screen-space effect that should be applied to the entire canvas
// or to specific regions (like node panels)

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> glass_tint: vec4<f32>;

@group(0) @binding(1)
var<uniform> glass_intensity: f32;

@group(0) @binding(2)
var<uniform> time: f32;

// Texture bindings for screen texture
@group(1) @binding(0)
var screen_texture: texture_2d<f32>;

@group(1) @binding(1)
var screen_sampler: sampler;

@vertex
fn vs_main(
    model: mat4x4<f32>,
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    mesh: mesh_2d,
) -> VertexOutput {
    var output: VertexOutput;
    
    let vertex: VertexInput = mesh.vertex;
    
    // Apply 2D transformation
    let world_position = (model * vec4<f32>(vertex.position, 1.0)).xyz;
    let view_position = (view * vec4<f32>(world_position, 1.0)).xyz;
    output.clip_position = projection * vec4<f32>(view_position, 1.0);
    
    output.uv = vertex.uv;
    
    return output;
}

// Simple blur function using multiple samples
fn blur(uv: vec2<f32>, texture: texture_2d<f32>, sampler: sampler) -> vec4<f32> {
    let offset = 0.002; // Adjust based on resolution
    
    let samples = array<vec2<f32>, 25>(vec2<f32>(0.0));
    let weights = array<f32, 25>(0.0);
    
    // Gaussian weights
    let w0 = 0.016216; let w1 = 0.054054; let w2 = 0.121621; let w3 = 0.194594;
    let w4 = 0.227027; let w5 = 0.194594; let w6 = 0.121621; let w7 = 0.054054;
    let w8 = 0.016216;
    
    let total_weights = w0 + w1 + w2 + w3 + w4 + w5 + w6 + w7 + w8;
    
    // Create sample offsets and weights for 5x5 gaussian blur
    for (var i: i32 = 0; i < 25; i++) {
        let x = (i % 5 - 2) as f32;
        let y = (i / 5 - 2) as f32;
        
        let weight_index = abs(x) as i32 + abs(y) as i32 * 5;
        let weight = select(
            select(w4, w3, abs(x) == 1 && abs(y) == 0),
            select(w2, w1, abs(x) == 2 && abs(y) == 0),
            abs(x) == 0 && abs(y) == 0
        );
        
        samples[i] = vec2<f32>(x, y) * offset * 2.0;
        weights[i] = weight / total_weights;
    }
    
    var color = vec4<f32>(0.0);
    for (var i: i32 = 0; i < 25; i++) {
        color += textureSample(screen_texture, screen_sampler, uv + samples[i]) * weights[i];
    }
    
    return color;
}

// Distortion effect (subtle wave)
fn distort(uv: vec2<f32>, time: f32) -> vec2<f32> {
    let distortion_amount = 0.001 * glass_intensity;
    let distortion_speed = 0.5;
    
    let distortion = vec2<
        sin(uv.y * 10.0 + time * distortion_speed) * distortion_amount,
        cos(uv.x * 10.0 + time * distortion_speed) * distortion_amount
    >;
    
    return uv + distortion;
}

@fragment
fn fs_main(input: VertexOutput) -> vec4<f32> {
    // Apply distortion
    let distorted_uv = distort(input.uv, time);
    
    // Sample the screen texture with blur
    let blurred = blur(distorted_uv, screen_texture, screen_sampler);
    
    // Apply glass tint
    let tinted = mix(blurred, glass_tint, glass_intensity * 0.5);
    
    // Add some edge fading (vignette effect)
    let edge_factor = smoothstep(0.0, 0.2, min(
        min(input.uv.x, 1.0 - input.uv.x),
        min(input.uv.y, 1.0 - input.uv.y)
    )) * 2.0;
    
    let final_color = mix(tinted, vec4<f32>(0.0), 1.0 - edge_factor);
    
    return final_color;
}
