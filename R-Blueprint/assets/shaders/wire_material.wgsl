// Wire material with glow effect
// This shader renders smooth bezier curves for wires with:
// - Color based on wire type
// - Glow effect when highlighted
// - Smooth anti-aliased edges

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> wire_color: vec4<f32>;

@group(0) @binding(1)
var<uniform> glow_color: vec4<f32>;

@group(0) @binding(2)
var<uniform> glow_intensity: f32;

@group(0) @binding(3)
var<uniform> thickness: f32;

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
    output.color = vertex.color;
    
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> vec4<f32> {
    // Get the distance from the center of the wire
    // uv.x goes from 0 to 1 across the wire
    // uv.y goes from -0.5 to 0.5 perpendicular to wire direction
    let dist_from_center = abs(input.uv.y);
    
    // Calculate alpha based on distance from center
    let half_thickness = thickness * 0.5;
    let alpha = smoothstep(half_thickness, half_thickness - 0.5, dist_from_center);
    
    // Base wire color
    let base_color = wire_color;
    
    // Add glow effect
    let glow = glow_intensity * 0.5;
    let final_color = mix(base_color, glow_color, glow);
    
    // Apply alpha
    return vec4<f32>(final_color.rgb, alpha * final_color.a);
}
