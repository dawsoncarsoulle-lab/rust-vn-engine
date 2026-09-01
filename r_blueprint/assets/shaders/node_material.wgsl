// Node background material with gradient and border
// This shader creates the node background with:
// - Vertical gradient from color_top to color_bottom
// - Border with border_color
// - Rounded corners
// - Optional selection/highlight glow

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> color_top: vec4<f32>;

@group(0) @binding(1)
var<uniform> color_bottom: vec4<f32>;

@group(0) @binding(2)
var<uniform> border_color: vec4<f32>;

@group(0) @binding(3)
var<uniform> corner_radius: f32;

@group(0) @binding(4)
var<uniform> is_selected: f32;

@group(0) @binding(5)
var<uniform> is_highlighted: f32;

// Smooth rounded rectangle SDF
fn rounded_rect_sdf(uv: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
    let d = vec2<
        max(uv.x, 0.0),
        max(uv.y, 0.0)
    >;
    let d2 = vec2<
        min(uv.x, 0.0) + size.x,
        min(uv.y, 0.0) + size.y
    >;
    let d3 = vec2<
        max(d.x, d2.x),
        max(d.y, d2.y)
    >;
    
    let d4 = vec2<
        min(d.x, d2.x) + radius,
        min(d.y, d2.y) + radius
    >;
    
    let d5 = vec2(max(d3.x, d4.x), max(d3.y, d4.y));
    let d6 = vec2(min(d3.x, d4.x), min(d3.y, d4.y));
    
    let d7 = vec2(d5.x, d6.y);
    let d8 = vec2(d6.x, d5.y);
    
    let d9 = vec2(d7.x, d8.y);
    
    let d10 = d9 - vec2(radius);
    
    let d11 = min(max(d10.x, 0.0), max(d10.y, 0.0));
    
    let d12 = length(d10 - d11) - radius;
    
    return min(max(d12, d11.x), d11.y);
}

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

@fragment
fn fs_main(input: VertexOutput) -> vec4<f32> {
    // Sample the UV
    let uv = input.uv;
    
    // Create gradient from top to bottom
    let gradient = mix(color_bottom, color_top, uv.y);
    
    // Calculate distance from edge for border
    let size = vec2<f32>(1.0);
    let radius = corner_radius / min(size.x, size.y);
    
    // Calculate SDF for rounded rectangle
    let center = vec2<f32>(0.5);
    let uv_centered = uv - center;
    let uv_abs = abs(uv_centered);
    
    // Simple rounded rectangle approximation
    let d = vec2<
        max(uv_abs.x - (0.5 - radius), 0.0),
        max(uv_abs.y - (0.5 - radius), 0.0)
    >;
    let dist = length(d) - radius;
    
    // Border effect
    let border_width = 0.02; // 2% of node size
    let border = smoothstep(dist, dist + border_width, 0.0);
    
    // Combine gradient with border
    let color = mix(gradient, border_color, border);
    
    // Add selection glow
    let glow = is_selected * 0.3 + is_highlighted * 0.2;
    if (glow > 0.0) {
        // Create outer glow
        let glow_dist = dist + border_width * 2.0;
        let glow_alpha = smoothstep(glow_dist, glow_dist - 0.1, 0.0) * glow * 2.0;
        return color + vec4<f32>(0.95, 0.71, 0.27, 1.0) * vec4<f32>(glow_alpha, glow_alpha, glow_alpha, glow_alpha);
    }
    
    return color;
}
