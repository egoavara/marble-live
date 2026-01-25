// Line rendering shader
// Renders thick lines (walls) with rounded caps

struct Camera {
    view_proj: mat4x4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,    // Position along/perpendicular to line
    @location(1) color: vec4<f32>,
    @location(2) half_length: f32,
    @location(3) half_width: f32,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

// Instance data for line segments
struct LineInstance {
    @location(0) start: vec2<f32>,
    @location(1) end: vec2<f32>,
    @location(2) width: f32,
    @location(3) color: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_idx: u32,
    instance: LineInstance,
) -> VertexOutput {
    // Calculate line direction and perpendicular
    let dir = instance.end - instance.start;
    let line_len = length(dir);
    let normalized_dir = dir / line_len;
    let perpendicular = vec2<f32>(-normalized_dir.y, normalized_dir.x);

    let half_length = line_len * 0.5;
    let half_width = instance.width * 0.5;
    let center = (instance.start + instance.end) * 0.5;

    // Generate quad vertices with extra padding for rounded caps
    var local_positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),  // 0: start, bottom
        vec2<f32>(1.0, -1.0),   // 1: end, bottom
        vec2<f32>(-1.0, 1.0),   // 2: start, top
        vec2<f32>(1.0, -1.0),   // 3: end, bottom
        vec2<f32>(1.0, 1.0),    // 4: end, top
        vec2<f32>(-1.0, 1.0),   // 5: start, top
    );

    let local_pos = local_positions[vertex_idx];

    // Scale to include rounded caps (extend by half_width at each end)
    let extended_half_length = half_length + half_width;

    // World position
    let world_pos = center
        + normalized_dir * local_pos.x * extended_half_length
        + perpendicular * local_pos.y * half_width;

    var output: VertexOutput;
    output.clip_position = camera.view_proj * vec4<f32>(world_pos, 0.0, 1.0);
    // Store position in line-local coordinates
    output.local_pos = vec2<f32>(local_pos.x * extended_half_length, local_pos.y * half_width);
    output.color = instance.color;
    output.half_length = half_length;
    output.half_width = half_width;

    return output;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Distance calculation for rounded rectangle (capsule shape)
    // The line is a capsule: rectangle with semicircle caps

    let x = in.local_pos.x;
    let y = in.local_pos.y;

    // Clamp x to the line segment (not including caps)
    let clamped_x = clamp(x, -in.half_length, in.half_length);

    // Distance from the line's central axis
    let dx = x - clamped_x;
    let dist_from_center = sqrt(dx * dx + y * y);

    // SDF for capsule
    let sdf = dist_from_center - in.half_width;

    // Anti-aliasing
    let aa_width = 1.5;
    let alpha = 1.0 - smoothstep(-aa_width, aa_width, sdf);

    if alpha < 0.01 {
        discard;
    }

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
