// Circle rendering shader
// Renders circles (marbles, obstacles, holes) using SDF with anti-aliasing

struct Camera {
    view_proj: mat4x4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,          // Local coordinates (-1 to 1)
    @location(1) color: vec4<f32>,
    @location(2) border_color: vec4<f32>,
    @location(3) radius: f32,
    @location(4) border_width: f32,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

// Instance data passed as vertex attributes
struct CircleInstance {
    @location(0) center: vec2<f32>,
    @location(1) radius: f32,
    @location(2) color: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) border_width: f32,
}

// Quad vertices for instanced rendering
// Each circle is drawn as a quad, with the fragment shader rendering the actual circle
@vertex
fn vs_main(
    @builtin(vertex_index) vertex_idx: u32,
    instance: CircleInstance,
) -> VertexOutput {
    // Generate quad vertices (2 triangles)
    // Vertex indices: 0, 1, 2 for first triangle; 3, 4, 5 for second
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),  // 0: bottom-left
        vec2<f32>(1.0, -1.0),   // 1: bottom-right
        vec2<f32>(-1.0, 1.0),   // 2: top-left
        vec2<f32>(1.0, -1.0),   // 3: bottom-right
        vec2<f32>(1.0, 1.0),    // 4: top-right
        vec2<f32>(-1.0, 1.0),   // 5: top-left
    );

    let local_pos = positions[vertex_idx];

    // Scale by radius (with some padding for anti-aliasing)
    let padding = 2.0; // Extra pixels for smooth edges
    let scaled_radius = instance.radius + padding;
    let world_pos = instance.center + local_pos * scaled_radius;

    var output: VertexOutput;
    output.clip_position = camera.view_proj * vec4<f32>(world_pos, 0.0, 1.0);
    output.uv = local_pos * scaled_radius / instance.radius; // Normalize to radius
    output.color = instance.color;
    output.border_color = instance.border_color;
    output.radius = instance.radius;
    output.border_width = instance.border_width;

    return output;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Distance from center in radius units
    let dist = length(in.uv);

    // SDF for circle
    let circle_sdf = dist - 1.0;

    // Anti-aliasing width (in radius units)
    let aa_width = 1.5 / in.radius;

    // Border calculation
    let border_inner = 1.0 - in.border_width / in.radius;
    let border_sdf = abs(dist - (1.0 + border_inner) * 0.5) - (1.0 - border_inner) * 0.5;

    // Alpha for anti-aliasing
    let circle_alpha = 1.0 - smoothstep(-aa_width, aa_width, circle_sdf);
    let border_alpha = 1.0 - smoothstep(-aa_width, aa_width, border_sdf);

    // Discard pixels outside the circle
    if circle_alpha < 0.01 {
        discard;
    }

    // Blend fill and border colors
    let fill_mask = 1.0 - smoothstep(border_inner - aa_width, border_inner + aa_width, dist);
    let color = mix(in.border_color, in.color, fill_mask);

    return vec4<f32>(color.rgb, color.a * circle_alpha);
}
