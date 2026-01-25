// Rectangle rendering shader
// Renders rotated rectangles (obstacles) with border

struct Camera {
    view_proj: mat4x4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,    // Position in local rect space
    @location(1) color: vec4<f32>,
    @location(2) border_color: vec4<f32>,
    @location(3) half_size: vec2<f32>,
    @location(4) border_width: f32,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

// Instance data for rectangles
struct RectInstance {
    @location(0) center: vec2<f32>,
    @location(1) half_size: vec2<f32>,
    @location(2) rotation: f32,          // Rotation in radians
    @location(3) color: vec4<f32>,
    @location(4) border_color: vec4<f32>,
    @location(5) border_width: f32,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_idx: u32,
    instance: RectInstance,
) -> VertexOutput {
    // Generate quad vertices
    var local_positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );

    let local_pos = local_positions[vertex_idx];

    // Add padding for border and anti-aliasing
    let padding = instance.border_width + 2.0;
    let padded_half_size = instance.half_size + vec2<f32>(padding, padding);

    // Rotation matrix
    let cos_r = cos(instance.rotation);
    let sin_r = sin(instance.rotation);

    // Scale to padded size
    let scaled_pos = local_pos * padded_half_size;

    // Rotate
    let rotated_pos = vec2<f32>(
        scaled_pos.x * cos_r - scaled_pos.y * sin_r,
        scaled_pos.x * sin_r + scaled_pos.y * cos_r
    );

    // Translate to world position
    let world_pos = instance.center + rotated_pos;

    var output: VertexOutput;
    output.clip_position = camera.view_proj * vec4<f32>(world_pos, 0.0, 1.0);
    // Store unrotated local position for SDF calculation
    output.local_pos = local_pos * padded_half_size;
    output.color = instance.color;
    output.border_color = instance.border_color;
    output.half_size = instance.half_size;
    output.border_width = instance.border_width;

    return output;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // SDF for rounded rectangle
    let d = abs(in.local_pos) - in.half_size;
    let outside_dist = length(max(d, vec2<f32>(0.0, 0.0)));
    let inside_dist = min(max(d.x, d.y), 0.0);
    let sdf = outside_dist + inside_dist;

    // Anti-aliasing
    let aa_width = 1.5;
    let alpha = 1.0 - smoothstep(-aa_width, aa_width, sdf);

    if alpha < 0.01 {
        discard;
    }

    // Border calculation
    let border_sdf = sdf + in.border_width;
    let border_mask = smoothstep(-aa_width, aa_width, border_sdf);

    // Blend fill and border colors
    let color = mix(in.color, in.border_color, border_mask);

    return vec4<f32>(color.rgb, color.a * alpha);
}
