//! wgpu-based renderer for the game.
//!
//! Provides GPU-accelerated rendering with WebGL2 fallback for WASM.
//!
//! This module is WASM-only. Use `cargo check --target wasm32-unknown-unknown` to build.

#[cfg(not(target_arch = "wasm32"))]
compile_error!("marble-client only supports wasm32 target. Use: cargo check -p marble-client --target wasm32-unknown-unknown");

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use marble_core::map::{EvaluatedShape, ObjectRole};
use marble_core::{Color, GameContext, GameState};
use web_sys::HtmlCanvasElement;
use wgpu::util::DeviceExt;

use crate::camera::CameraState;

/// Circle instance data for GPU rendering.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CircleInstance {
    pub center: [f32; 2],
    pub radius: f32,
    pub _padding1: f32, // Alignment padding
    pub color: [f32; 4],
    pub border_color: [f32; 4],
    pub border_width: f32,
    pub _padding2: [f32; 3], // Alignment padding
}

impl CircleInstance {
    pub fn new(
        center: (f32, f32),
        radius: f32,
        color: Color,
        border_color: Color,
        border_width: f32,
    ) -> Self {
        Self {
            center: [center.0, center.1],
            radius,
            _padding1: 0.0,
            color: color_to_array(color),
            border_color: color_to_array(border_color),
            border_width,
            _padding2: [0.0; 3],
        }
    }
}

/// Line instance data for GPU rendering.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LineInstance {
    pub start: [f32; 2],
    pub end: [f32; 2],
    pub width: f32,
    pub _padding1: [f32; 3], // Alignment padding
    pub color: [f32; 4],
}

impl LineInstance {
    pub fn new(start: (f32, f32), end: (f32, f32), width: f32, color: Color) -> Self {
        Self {
            start: [start.0, start.1],
            end: [end.0, end.1],
            width,
            _padding1: [0.0; 3],
            color: color_to_array(color),
        }
    }
}

/// Rectangle instance data for GPU rendering.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct RectInstance {
    pub center: [f32; 2],
    pub half_size: [f32; 2],
    pub rotation: f32,        // Rotation in radians
    pub _padding1: [f32; 3],  // Alignment padding
    pub color: [f32; 4],
    pub border_color: [f32; 4],
    pub border_width: f32,
    pub _padding2: [f32; 3],  // Alignment padding
}

impl RectInstance {
    pub fn new(
        center: (f32, f32),
        half_size: (f32, f32),
        rotation_degrees: f32,
        color: Color,
        border_color: Color,
        border_width: f32,
    ) -> Self {
        Self {
            center: [center.0, center.1],
            half_size: [half_size.0, half_size.1],
            rotation: rotation_degrees.to_radians(),
            _padding1: [0.0; 3],
            color: color_to_array(color),
            border_color: color_to_array(border_color),
            border_width,
            _padding2: [0.0; 3],
        }
    }
}

/// Camera uniform data.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
}

/// wgpu-based renderer.
pub struct WgpuRenderer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,

    // Pipelines
    circle_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    rect_pipeline: wgpu::RenderPipeline,

    // Camera uniform
    camera_uniform_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,

    // Instance buffers (resizable)
    circle_instance_buffer: wgpu::Buffer,
    line_instance_buffer: wgpu::Buffer,
    rect_instance_buffer: wgpu::Buffer,
    max_circles: u32,
    max_lines: u32,
    max_rects: u32,

    // Dimensions
    width: u32,
    height: u32,
}

impl WgpuRenderer {
    /// Creates a new wgpu renderer from an HTML canvas element.
    ///
    /// This is an async operation that initializes the GPU device.
    pub async fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
        let width = canvas.width();
        let height = canvas.height();

        // Create wgpu instance
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
            ..Default::default()
        });

        // Create surface from canvas (wgpu 28: use SurfaceTarget::Canvas)
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| format!("Failed to create surface: {e}"))?;

        // Request adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("Failed to find a suitable GPU adapter: {e}"))?;

        tracing::info!(
            "Using GPU adapter: {:?} ({:?})",
            adapter.get_info().name,
            adapter.get_info().backend
        );

        // Request device (wgpu 28: single argument, no trace_dir)
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|e| format!("Failed to create device: {e}"))?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Create camera uniform buffer and bind group
        let camera_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Uniform Buffer"),
            contents: bytemuck::cast_slice(&[CameraUniform {
                view_proj: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_uniform_buffer.as_entire_binding(),
            }],
        });

        // Create pipelines
        let circle_pipeline =
            Self::create_circle_pipeline(&device, &camera_bind_group_layout, surface_format);
        let line_pipeline =
            Self::create_line_pipeline(&device, &camera_bind_group_layout, surface_format);
        let rect_pipeline =
            Self::create_rect_pipeline(&device, &camera_bind_group_layout, surface_format);

        // Create instance buffers with initial capacity
        let max_circles = 256u32;
        let max_lines = 256u32;
        let max_rects = 64u32;

        let circle_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Circle Instance Buffer"),
            size: (max_circles as usize * std::mem::size_of::<CircleInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let line_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Line Instance Buffer"),
            size: (max_lines as usize * std::mem::size_of::<LineInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let rect_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Rect Instance Buffer"),
            size: (max_rects as usize * std::mem::size_of::<RectInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            device,
            queue,
            surface,
            config,
            circle_pipeline,
            line_pipeline,
            rect_pipeline,
            camera_uniform_buffer,
            camera_bind_group,
            circle_instance_buffer,
            line_instance_buffer,
            rect_instance_buffer,
            max_circles,
            max_lines,
            max_rects,
            width,
            height,
        })
    }

    fn create_circle_pipeline(
        device: &wgpu::Device,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Circle Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/circle.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Circle Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout],
            immediate_size: 0,
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Circle Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<CircleInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        // center
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        // radius
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32,
                        },
                        // color
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        // border_color
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        // border_width
                        wgpu::VertexAttribute {
                            offset: 48,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32,
                        },
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        })
    }

    fn create_line_pipeline(
        device: &wgpu::Device,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Line Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/line.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Line Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout],
            immediate_size: 0,
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Line Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<LineInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        // start
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        // end
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        // width
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32,
                        },
                        // color
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        })
    }

    fn create_rect_pipeline(
        device: &wgpu::Device,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Rect Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/rect.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Rect Pipeline Layout"),
            bind_group_layouts: &[camera_bind_group_layout],
            immediate_size: 0,
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Rect Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<RectInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        // center
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        // half_size
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        // rotation
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32,
                        },
                        // color (offset 32 due to padding)
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        // border_color
                        wgpu::VertexAttribute {
                            offset: 48,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        // border_width
                        wgpu::VertexAttribute {
                            offset: 64,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Float32,
                        },
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        })
    }

    /// Resize the renderer to new dimensions.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.width = width;
        self.height = height;
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Get current width.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get current height.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Renders the game state with the given camera.
    pub fn render(&mut self, game_state: &GameState, camera: &CameraState) {
        // Update camera uniform
        let camera_uniform = CameraUniform {
            view_proj: camera.view_projection_matrix(),
        };
        self.queue.write_buffer(
            &self.camera_uniform_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );

        // Collect instances
        let (circles, lines, rects) = self.collect_instances(game_state);

        // Ensure buffers are large enough
        self.ensure_buffer_capacity(&circles, &lines, &rects);

        // Write instance data
        if !circles.is_empty() {
            self.queue.write_buffer(
                &self.circle_instance_buffer,
                0,
                bytemuck::cast_slice(&circles),
            );
        }
        if !lines.is_empty() {
            self.queue
                .write_buffer(&self.line_instance_buffer, 0, bytemuck::cast_slice(&lines));
        }
        if !rects.is_empty() {
            self.queue
                .write_buffer(&self.rect_instance_buffer, 0, bytemuck::cast_slice(&rects));
        }

        // Get surface texture
        let output = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("Failed to get surface texture: {:?}", e);
                return;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.102,
                            g: 0.102,
                            b: 0.180,
                            a: 1.0,
                        }), // #1a1a2e
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // Draw lines (walls) first
            if !lines.is_empty() {
                render_pass.set_pipeline(&self.line_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.line_instance_buffer.slice(..));
                render_pass.draw(0..6, 0..lines.len() as u32);
            }

            // Draw rectangles (obstacles)
            if !rects.is_empty() {
                render_pass.set_pipeline(&self.rect_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.rect_instance_buffer.slice(..));
                render_pass.draw(0..6, 0..rects.len() as u32);
            }

            // Draw circles (holes, obstacles, marbles)
            if !circles.is_empty() {
                render_pass.set_pipeline(&self.circle_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.circle_instance_buffer.slice(..));
                render_pass.draw(0..6, 0..circles.len() as u32);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    /// Renders the game state with additional overlay instances (e.g., gizmos).
    pub fn render_with_overlay(
        &mut self,
        game_state: &GameState,
        camera: &CameraState,
        overlay_circles: &[CircleInstance],
        overlay_lines: &[LineInstance],
        overlay_rects: &[RectInstance],
    ) {
        // Update camera uniform
        let camera_uniform = CameraUniform {
            view_proj: camera.view_projection_matrix(),
        };
        self.queue.write_buffer(
            &self.camera_uniform_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );

        // Collect base instances
        let (base_circles, base_lines, base_rects) = self.collect_instances(game_state);

        // Track base counts for separate draw calls
        let base_circle_count = base_circles.len();
        let base_line_count = base_lines.len();
        let base_rect_count = base_rects.len();

        // Combine base + overlay for buffer write
        let mut circles = base_circles;
        let mut lines = base_lines;
        let mut rects = base_rects;
        circles.extend_from_slice(overlay_circles);
        lines.extend_from_slice(overlay_lines);
        rects.extend_from_slice(overlay_rects);

        // Ensure buffers are large enough
        self.ensure_buffer_capacity(&circles, &lines, &rects);

        // Write instance data
        if !circles.is_empty() {
            self.queue.write_buffer(
                &self.circle_instance_buffer,
                0,
                bytemuck::cast_slice(&circles),
            );
        }
        if !lines.is_empty() {
            self.queue
                .write_buffer(&self.line_instance_buffer, 0, bytemuck::cast_slice(&lines));
        }
        if !rects.is_empty() {
            self.queue
                .write_buffer(&self.rect_instance_buffer, 0, bytemuck::cast_slice(&rects));
        }

        let overlay_circle_count = overlay_circles.len();
        let overlay_line_count = overlay_lines.len();
        let overlay_rect_count = overlay_rects.len();

        // Get surface texture
        let output = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("Failed to get surface texture: {:?}", e);
                return;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.102,
                            g: 0.102,
                            b: 0.180,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // === Draw base instances first ===
            if base_line_count > 0 {
                render_pass.set_pipeline(&self.line_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.line_instance_buffer.slice(..));
                render_pass.draw(0..6, 0..base_line_count as u32);
            }

            if base_rect_count > 0 {
                render_pass.set_pipeline(&self.rect_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.rect_instance_buffer.slice(..));
                render_pass.draw(0..6, 0..base_rect_count as u32);
            }

            if base_circle_count > 0 {
                render_pass.set_pipeline(&self.circle_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.circle_instance_buffer.slice(..));
                render_pass.draw(0..6, 0..base_circle_count as u32);
            }

            // === Draw overlay (gizmo) on top ===
            if overlay_line_count > 0 {
                render_pass.set_pipeline(&self.line_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.line_instance_buffer.slice(..));
                render_pass.draw(0..6, base_line_count as u32..(base_line_count + overlay_line_count) as u32);
            }

            if overlay_rect_count > 0 {
                render_pass.set_pipeline(&self.rect_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.rect_instance_buffer.slice(..));
                render_pass.draw(0..6, base_rect_count as u32..(base_rect_count + overlay_rect_count) as u32);
            }

            if overlay_circle_count > 0 {
                render_pass.set_pipeline(&self.circle_pipeline);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.circle_instance_buffer.slice(..));
                render_pass.draw(0..6, base_circle_count as u32..(base_circle_count + overlay_circle_count) as u32);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    fn collect_instances(&self, game_state: &GameState) -> (Vec<CircleInstance>, Vec<LineInstance>, Vec<RectInstance>) {
        let mut circles = Vec::new();
        let mut lines = Vec::new();
        let mut rects = Vec::new();

        if let Some(config) = &game_state.map_config {
            // Get current game context for CEL expression evaluation
            let time = game_state.physics_world.current_frame() as f32 / 60.0;
            let ctx = GameContext::new(time, game_state.physics_world.current_frame());

            for obj in &config.objects {
                // Check if this object is a kinematic body (animated)
                let kinematic_transform = obj.id.as_ref().and_then(|id| {
                    game_state.kinematic_bodies.get(id).and_then(|handle| {
                        game_state.physics_world.get_body_position(*handle)
                    })
                });

                // Get the shape, applying kinematic transform if available
                let shape = if let Some((pos, rot)) = kinematic_transform {
                    // Use the kinematic body's current position
                    let base_shape = obj.shape.evaluate(&ctx);
                    match base_shape {
                        EvaluatedShape::Line { start, end } => {
                            // For lines, we need to rotate and translate relative to the body center
                            let (dx1, dy1, dx2, dy2) = if let Some(&(init_pos, _)) = game_state.kinematic_initial_transforms.get(obj.id.as_ref().unwrap()) {
                                let mid_x = (start[0] + end[0]) / 2.0;
                                let mid_y = (start[1] + end[1]) / 2.0;
                                (start[0] - mid_x, start[1] - mid_y, end[0] - mid_x, end[1] - mid_y)
                            } else {
                                let mid_x = (start[0] + end[0]) / 2.0;
                                let mid_y = (start[1] + end[1]) / 2.0;
                                (start[0] - mid_x, start[1] - mid_y, end[0] - mid_x, end[1] - mid_y)
                            };
                            let cos_r = rot.cos();
                            let sin_r = rot.sin();
                            EvaluatedShape::Line {
                                start: [
                                    pos[0] + dx1 * cos_r - dy1 * sin_r,
                                    pos[1] + dx1 * sin_r + dy1 * cos_r,
                                ],
                                end: [
                                    pos[0] + dx2 * cos_r - dy2 * sin_r,
                                    pos[1] + dx2 * sin_r + dy2 * cos_r,
                                ],
                            }
                        }
                        EvaluatedShape::Circle { radius, .. } => {
                            EvaluatedShape::Circle { center: pos, radius }
                        }
                        EvaluatedShape::Rect { size, .. } => {
                            EvaluatedShape::Rect {
                                center: pos,
                                size,
                                rotation: rot.to_degrees(),
                            }
                        }
                        EvaluatedShape::Bezier { .. } => {
                            // Bezier curves are not supported for kinematic objects
                            obj.shape.evaluate(&ctx)
                        }
                    }
                } else {
                    obj.shape.evaluate(&ctx)
                };

                match obj.role {
                    ObjectRole::Trigger => {
                        // Render triggers (holes) with special styling
                        match shape {
                            EvaluatedShape::Circle { center, radius } => {
                                // Hole shadow/glow
                                circles.push(CircleInstance::new(
                                    (center[0], center[1]),
                                    radius * 1.2,
                                    Color::new(0, 0, 0, 128),
                                    Color::new(0, 0, 0, 0),
                                    0.0,
                                ));
                                // Hole interior
                                circles.push(CircleInstance::new(
                                    (center[0], center[1]),
                                    radius,
                                    Color::new(13, 13, 26, 255),
                                    Color::new(255, 68, 68, 255),
                                    0.03,
                                ));
                            }
                            EvaluatedShape::Rect { center, size, rotation } => {
                                rects.push(RectInstance::new(
                                    (center[0], center[1]),
                                    (size[0] / 2.0, size[1] / 2.0),
                                    rotation,
                                    Color::new(13, 13, 26, 255),
                                    Color::new(255, 68, 68, 255),
                                    0.03,
                                ));
                            }
                            EvaluatedShape::Line { .. } => {}
                            EvaluatedShape::Bezier { .. } => {}
                        }
                    }
                    ObjectRole::Obstacle => {
                        // Render obstacles
                        match shape {
                            EvaluatedShape::Line { start, end } => {
                                lines.push(LineInstance::new(
                                    (start[0], start[1]),
                                    (end[0], end[1]),
                                    0.04,
                                    Color::new(74, 74, 106, 255),
                                ));
                            }
                            EvaluatedShape::Circle { center, radius } => {
                                circles.push(CircleInstance::new(
                                    (center[0], center[1]),
                                    radius,
                                    Color::new(58, 58, 90, 255),
                                    Color::new(90, 90, 138, 255),
                                    0.02,
                                ));
                            }
                            EvaluatedShape::Rect { center, size, rotation } => {
                                rects.push(RectInstance::new(
                                    (center[0], center[1]),
                                    (size[0] / 2.0, size[1] / 2.0),
                                    rotation,
                                    Color::new(58, 58, 90, 255),
                                    Color::new(90, 90, 138, 255),
                                    0.02,
                                ));
                            }
                            EvaluatedShape::Bezier { .. } => {
                                // Render bezier as multiple line segments
                                if let Some(points) = shape.bezier_to_points() {
                                    for i in 0..points.len() - 1 {
                                        lines.push(LineInstance::new(
                                            (points[i][0], points[i][1]),
                                            (points[i + 1][0], points[i + 1][1]),
                                            0.04,
                                            Color::new(74, 74, 106, 255),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    ObjectRole::Spawner => {
                        // Optionally render spawner area (semi-transparent)
                        match shape {
                            EvaluatedShape::Rect { center, size, rotation } => {
                                rects.push(RectInstance::new(
                                    (center[0], center[1]),
                                    (size[0] / 2.0, size[1] / 2.0),
                                    rotation,
                                    Color::new(50, 100, 50, 40),
                                    Color::new(100, 200, 100, 80),
                                    0.01,
                                ));
                            }
                            EvaluatedShape::Circle { center, radius } => {
                                circles.push(CircleInstance::new(
                                    (center[0], center[1]),
                                    radius,
                                    Color::new(50, 100, 50, 40),
                                    Color::new(100, 200, 100, 80),
                                    0.01,
                                ));
                            }
                            EvaluatedShape::Line { .. } => {}
                            EvaluatedShape::Bezier { .. } => {}
                        }
                    }
                }
            }
        }

        // Collect marble instances
        for marble in game_state.marble_manager.marbles() {
            if marble.eliminated {
                continue;
            }

            if let Some((x, y)) = game_state
                .marble_manager
                .get_marble_position(&game_state.physics_world, marble.id)
            {
                // Marble shadow
                circles.push(CircleInstance::new(
                    (x + 0.02, y + 0.02),
                    marble.radius,
                    Color::new(0, 0, 0, 76),
                    Color::new(0, 0, 0, 0),
                    0.0,
                ));

                // Marble body
                let border_color = Color::rgb(
                    marble.color.r.saturating_sub(40),
                    marble.color.g.saturating_sub(40),
                    marble.color.b.saturating_sub(40),
                );
                circles.push(CircleInstance::new(
                    (x, y),
                    marble.radius,
                    marble.color,
                    border_color,
                    0.02,
                ));

                // Marble highlight
                circles.push(CircleInstance::new(
                    (x - marble.radius * 0.3, y - marble.radius * 0.3),
                    marble.radius * 0.3,
                    Color::new(255, 255, 255, 102),
                    Color::new(0, 0, 0, 0),
                    0.0,
                ));
            }
        }

        (circles, lines, rects)
    }

    fn ensure_buffer_capacity(&mut self, circles: &[CircleInstance], lines: &[LineInstance], rects: &[RectInstance]) {
        let needed_circles = circles.len() as u32;
        let needed_lines = lines.len() as u32;
        let needed_rects = rects.len() as u32;

        if needed_circles > self.max_circles {
            self.max_circles = needed_circles.next_power_of_two();
            self.circle_instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Circle Instance Buffer"),
                size: (self.max_circles as usize * std::mem::size_of::<CircleInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        if needed_lines > self.max_lines {
            self.max_lines = needed_lines.next_power_of_two();
            self.line_instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Line Instance Buffer"),
                size: (self.max_lines as usize * std::mem::size_of::<LineInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        if needed_rects > self.max_rects {
            self.max_rects = needed_rects.next_power_of_two();
            self.rect_instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Rect Instance Buffer"),
                size: (self.max_rects as usize * std::mem::size_of::<RectInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
    }
}

/// Converts a Color to a normalized float array.
fn color_to_array(color: Color) -> [f32; 4] {
    [
        f32::from(color.r) / 255.0,
        f32::from(color.g) / 255.0,
        f32::from(color.b) / 255.0,
        f32::from(color.a) / 255.0,
    ]
}
