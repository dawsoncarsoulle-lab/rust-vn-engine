// Original RVN canvas shader. Uniform arrays work on WebGL2 as well as desktop;
// there are no storage buffers, platform extensions or third-party UI assets.
#import bevy_ui::ui_vertex_output::UiVertexOutput

struct Clip {
    row_x: vec4<f32>,
    row_y: vec4<f32>,
    rect: vec4<f32>,
}
struct Ink {
    bounds: vec4<f32>,
    row_x: vec4<f32>,
    row_y: vec4<f32>,
    color: vec4<f32>,
    rect: vec4<f32>,
    // kind, radius, line width, number of points
    parameters: vec4<f32>,
    // number of clips, unused
    counts: vec4<f32>,
    points: array<vec4<f32>, 256>,
    clips: array<Clip, 64>,
}
@group(1) @binding(0) var<uniform> ink: Ink;
@group(1) @binding(1) var ink_texture: texture_2d<f32>;
@group(1) @binding(2) var ink_sampler: sampler;

fn mapped(point: vec2<f32>, x: vec4<f32>, y: vec4<f32>) -> vec2<f32> {
    return vec2<f32>(dot(x.xyz, vec3<f32>(point, 1.0)), dot(y.xyz, vec3<f32>(point, 1.0)));
}
fn rectangle_distance(point: vec2<f32>, rect: vec4<f32>, radius: f32) -> f32 {
    let half_size = rect.zw * 0.5;
    let r = clamp(radius, 0.0, min(half_size.x, half_size.y));
    let q = abs(point - rect.xy - half_size) - half_size + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}
fn segment_distance(point: vec2<f32>, start: vec2<f32>, end: vec2<f32>) -> f32 {
    let direction = end - start;
    let fraction = clamp(dot(point - start, direction) / max(dot(direction, direction), 0.000001), 0.0, 1.0);
    return length(point - start - fraction * direction);
}
@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let canvas_point = ink.bounds.xy + in.uv * ink.bounds.zw;
    // Clips use their own inverse affine matrices. Rotating/scaling a group
    // therefore clips the drawing and its hit regions in precisely the same space.
    for (var index = 0u; index < u32(ink.counts.x); index += 1u) {
        let clip = ink.clips[index];
        let local = mapped(canvas_point, clip.row_x, clip.row_y);
        if any(local < clip.rect.xy) || any(local > clip.rect.xy + clip.rect.zw) { discard; }
    }
    let point = mapped(canvas_point, ink.row_x, ink.row_y);
    let kind = u32(ink.parameters.x);
    var distance = 0.0;
    var result = ink.color;
    if kind == 0u {
        distance = rectangle_distance(point, ink.rect, ink.parameters.y);
    } else if kind == 1u {
        let radius = max(ink.rect.zw * 0.5, vec2<f32>(0.000001));
        let normalized = (point - ink.rect.xy - radius) / radius;
        distance = (length(normalized) - 1.0) * min(radius.x, radius.y);
    } else if kind == 2u {
        distance = 1000000.0;
        for (var index = 1u; index < u32(ink.parameters.w); index += 1u) {
            distance = min(distance, segment_distance(point, ink.points[index-1u].xy, ink.points[index].xy));
        }
        distance -= ink.parameters.z * 0.5;
    } else if kind == 3u {
        var inside = false;
        distance = 1000000.0;
        var previous = u32(ink.parameters.w) - 1u;
        for (var index = 0u; index < u32(ink.parameters.w); index += 1u) {
            let a = ink.points[index].xy;
            let b = ink.points[previous].xy;
            distance = min(distance, segment_distance(point, a, b));
            if (a.y > point.y) != (b.y > point.y) {
                let intersection = a.x + (point.y - a.y) * (b.x - a.x) / (b.y - a.y);
                if point.x < intersection { inside = !inside; }
            }
            previous = index;
        }
        if inside { distance = -distance; }
    } else {
        distance = rectangle_distance(point, ink.rect, 0.0);
        let uv = (point - ink.rect.xy) / max(ink.rect.zw, vec2<f32>(0.000001));
        result *= textureSample(ink_texture, ink_sampler, clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)));
    }
    // fwidth keeps edges smooth under both nested transforms and HiDPI scaling.
    let coverage = clamp(0.5 - distance / max(fwidth(distance), 0.0001), 0.0, 1.0);
    result.a *= coverage;
    return result;
}
