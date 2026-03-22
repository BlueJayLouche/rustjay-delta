//! # wgpu Renderer
//!
//! Main rendering engine with motion extraction using Posy's RGB delay method.
//! Uses a frame history buffer to create time-delayed RGB channel effects.

use crate::core::{MotionParams, SharedState};
use crate::core::vertex::Vertex;
use crate::engine::blit::BlitPipeline;
use crate::engine::frame_history::FrameHistory;
use crate::engine::texture::{InputTexture, Texture};
use crate::output::OutputManager;

use anyhow::Result;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

/// GPU representation of Motion parameters
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct MotionUniforms {
    /// delays: x=red, y=green, z=blue, w=max_history
    delays: [f32; 4],
    /// settings: x=intensity, y=blend_mode, z=grayscale_input, w=unused
    settings: [f32; 4],
    /// channel_gain: x=red, y=green, z=blue, w=unused
    channel_gain: [f32; 4],
    /// mix_options: x=input_mix, y=trail_fade, z=threshold, w=smoothing
    mix_options: [f32; 4],
}

impl From<&MotionParams> for MotionUniforms {
    fn from(params: &MotionParams) -> Self {
        Self {
            delays: [
                params.red_delay as f32,
                params.green_delay as f32,
                params.blue_delay as f32,
                16.0, // max_history
            ],
            settings: [
                params.intensity,
                params.blend_mode as i32 as f32,
                if params.grayscale_input { 1.0 } else { 0.0 },
                0.0, // unused
            ],
            channel_gain: [
                params.red_gain,
                params.green_gain,
                params.blue_gain,
                0.0, // unused
            ],
            mix_options: [
                params.input_mix,
                params.trail_fade,
                params.threshold,
                params.smoothing,
            ],
        }
    }
}

/// Main wgpu rendering engine
pub struct WgpuEngine {
    #[allow(dead_code)]
    instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,

    // Window size
    window_width: u32,
    window_height: u32,

    // Shared state
    shared_state: Arc<std::sync::Mutex<SharedState>>,

    // Render pipeline
    render_pipeline: wgpu::RenderPipeline,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    uniform_bind_group_layout: wgpu::BindGroupLayout,

    // Render target (internal resolution)
    pub render_target: Texture,

    // Input texture
    pub input_texture: InputTexture,

    // Frame history for motion extraction
    frame_history: FrameHistory,

    // Vertex buffer
    vertex_buffer: wgpu::Buffer,

    // Uniform buffer for motion parameters
    motion_uniform_buffer: wgpu::Buffer,

    // Cached uniform bind group (buffer identity never changes)
    uniform_bind_group: wgpu::BindGroup,

    // Cached blit pipeline (previously recreated every frame)
    blit_pipeline: BlitPipeline,

    // Frame counter
    frame_count: u64,
    fps_last_time: std::time::Instant,
    fps_frame_count: u32,
    fps_current: f32,

    // Output manager (NDI, Syphon)
    output_manager: OutputManager,
}

impl WgpuEngine {
    /// Create a new wgpu engine
    pub async fn new(
        instance: &wgpu::Instance,
        window: Arc<Window>,
        shared_state: Arc<std::sync::Mutex<SharedState>>,
    ) -> Result<Self> {
        let size = window.inner_size();

        let surface = instance.create_surface(window)?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    label: Some("Device"),
                    memory_hints: wgpu::MemoryHints::default(),
                    trace: wgpu::Trace::Off,
                },
            )
            .await?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| *f == wgpu::TextureFormat::Bgra8UnormSrgb || *f == wgpu::TextureFormat::Bgra8Unorm)
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Get initial resolution from shared state
        let (initial_width, initial_height, history_size) = {
            let state = shared_state.lock().unwrap();
            (state.output_width, state.output_height, state.history_size)
        };

        // Create render target at specified resolution
        let render_target = Texture::create_render_target(&device, initial_width, initial_height, "Render Target");

        // Create input texture manager
        let input_texture = InputTexture::new(Arc::clone(&device), Arc::clone(&queue));

        // Create frame history
        let frame_history = FrameHistory::new(
            Arc::clone(&device),
            Arc::clone(&queue),
            history_size,
        );

        // Create motion extraction shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Motion Extraction Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/motion_extraction.wgsl").into()),
        });

        // Create texture bind group layout for 5 textures (3 history + input + placeholder)
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Texture Bind Group Layout"),
                entries: &[
                    // History texture 0 (red channel source)
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // History texture 1 (green channel source)
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // History texture 2 (blue channel source)
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Input texture (current frame)
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Shared sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // Create uniform bind group layout for motion parameters
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Uniform Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&texture_bind_group_layout, &uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
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
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create vertex buffer
        let vertices = Vertex::quad_vertices();
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Create motion uniform buffer
        let motion_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Motion Uniform Buffer"),
            size: std::mem::size_of::<MotionUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create cached blit pipeline (was previously recreated every frame)
        let blit_pipeline = BlitPipeline::new(&device, surface_format);

        // Cache uniform bind group — buffer identity never changes, only contents
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: motion_uniform_buffer.as_entire_binding(),
            }],
        });

        Ok(Self {
            instance: instance.clone(),
            adapter,
            device: Arc::clone(&device),
            queue: Arc::clone(&queue),
            surface,
            surface_config,
            window_width: size.width,
            window_height: size.height,
            shared_state,
            render_pipeline,
            texture_bind_group_layout,
            uniform_bind_group_layout,
            render_target,
            input_texture,
            frame_history,
            vertex_buffer,
            motion_uniform_buffer,
            uniform_bind_group,
            blit_pipeline,
            frame_count: 0,
            fps_last_time: std::time::Instant::now(),
            fps_frame_count: 0,
            fps_current: 0.0,
            output_manager: OutputManager::new(),
        })
    }

    /// Resize the surface
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.window_width = width;
            self.window_height = height;
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
            log::debug!("Resized to {}x{}", width, height);
        }
    }
    
    /// Resize render target and frame history to new resolution
    pub fn resize_render_target(&mut self, width: u32, height: u32) {
        log::info!("Resizing render target to {}x{}", width, height);
        self.render_target = Texture::create_render_target(&self.device, width, height, "Render Target");
        self.frame_history.resize(width, height);
    }
    
    /// Update frame history size
    pub fn set_history_size(&mut self, size: usize) {
        self.frame_history.set_history_size(size);
    }
    
    /// Get frame history memory usage
    pub fn history_memory_usage(&self) -> String {
        self.frame_history.memory_usage_string()
    }

    /// Start NDI output
    #[cfg(feature = "ndi")]
    pub fn start_ndi_output(&mut self, name: &str, include_alpha: bool) -> anyhow::Result<()> {
        self.output_manager.start_ndi(
            name,
            self.render_target.width,
            self.render_target.height,
            include_alpha,
        )?;
        Ok(())
    }

    #[cfg(not(feature = "ndi"))]
    pub fn start_ndi_output(&mut self, _name: &str, _include_alpha: bool) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("NDI support not compiled. Enable the 'ndi' feature."))
    }

    /// Stop NDI output
    #[cfg(feature = "ndi")]
    pub fn stop_ndi_output(&mut self) {
        self.output_manager.stop_ndi();
    }

    #[cfg(not(feature = "ndi"))]
    pub fn stop_ndi_output(&mut self) {}

    /// Start Syphon output (macOS only)
    #[cfg(target_os = "macos")]
    pub fn start_syphon_output(&mut self, server_name: &str) -> anyhow::Result<()> {
        self.output_manager.start_syphon(
            server_name,
            Arc::clone(&self.device),
            Arc::clone(&self.queue),
        )?;
        Ok(())
    }

    /// Stop Syphon output (macOS only)
    #[cfg(target_os = "macos")]
    pub fn stop_syphon_output(&mut self) {
        self.output_manager.stop_syphon();
    }

    /// Render a frame
    pub fn render(&mut self, _occluded: bool) {
        // Get current motion parameters from shared state
        let (motion_params, motion_enabled) = {
            let state = match self.shared_state.lock() {
                Ok(s) => s,
                Err(e) => e.into_inner(),
            };
            
            // Apply audio routing if enabled
            let mut params = if state.audio_routing.enabled {
                state.audio_routing.matrix.apply_to_params(&state.motion_params)
            } else {
                state.motion_params
            };

            // Apply LFO modulation on top of audio routing
            let (rd, gd, bd, intensity, mix, fade, thresh, smooth) = state.lfo.apply_to_motion();
            params.red_delay = ((params.red_delay as f32 + rd).round() as u32).clamp(0, 16);
            params.green_delay = ((params.green_delay as f32 + gd).round() as u32).clamp(0, 16);
            params.blue_delay = ((params.blue_delay as f32 + bd).round() as u32).clamp(0, 16);
            params.intensity = (params.intensity + intensity).clamp(0.0, 1.0);
            params.input_mix = (params.input_mix + mix).clamp(0.0, 1.0);
            params.trail_fade = (params.trail_fade + fade).clamp(0.0, 1.0);
            params.threshold = (params.threshold + thresh).clamp(0.0, 1.0);
            params.smoothing = (params.smoothing + smooth).clamp(0.0, 1.0);

            (params, state.motion_enabled)
        };

        // Get surface texture
        let surface_texture = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(_) => {
                self.surface.configure(&self.device, &self.surface_config);
                return;
            }
        };

        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Create command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Ensure we have an input texture
        if self.input_texture.texture.is_none() {
            self.input_texture.ensure_size(1280, 720);
            if let Some(ref tex) = self.input_texture.texture {
                tex.clear_to_black(&self.queue);
            }
        }

        // Push current input to frame history
        // Note: For Syphon input, the texture must be copied to the owned texture
        // in update.rs (see update_from_texture call) before we get here
        if let Some(ref input_tex) = self.input_texture.texture {
            self.frame_history.push_frame(&input_tex.texture, &mut encoder);
        }

        // Get the three history frames for RGB channels
        let red_frame = self.frame_history.get_frame(motion_params.red_delay as usize)
            .or_else(|| self.input_texture.texture.as_ref())
            .map(|t| &t.view);
        let green_frame = self.frame_history.get_frame(motion_params.green_delay as usize)
            .or_else(|| self.input_texture.texture.as_ref())
            .map(|t| &t.view);
        let blue_frame = self.frame_history.get_frame(motion_params.blue_delay as usize)
            .or_else(|| self.input_texture.texture.as_ref())
            .map(|t| &t.view);

        // Create texture bind group
        let texture_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Texture Bind Group"),
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        red_frame.expect("Red frame not available")
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        green_frame.expect("Green frame not available")
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        blue_frame.expect("Blue frame not available")
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(
                        self.input_texture.binding_view().expect("Input texture not initialized")
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(
                        self.input_texture.binding_sampler().expect("Input sampler not initialized")
                    ),
                },
            ],
        });

        // Update motion uniform buffer
        let motion_uniforms: MotionUniforms = if motion_enabled {
            (&motion_params).into()
        } else {
            // Default passthrough
            MotionUniforms {
                delays: [0.0, 0.0, 0.0, 16.0],
                settings: [0.0, 0.0, 0.0, 0.0],
                channel_gain: [1.0, 1.0, 1.0, 0.0],
                mix_options: [1.0, 0.0, 0.0, 0.0],
            }
        };
        self.queue.write_buffer(
            &self.motion_uniform_buffer,
            0,
            bytemuck::bytes_of(&motion_uniforms),
        );

        // Render to render target (uniform bind group is cached — buffer contents
        // are updated via write_buffer above, bind group references don't change)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.render_target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_bind_group(0, &texture_bind_group, &[]);
            render_pass.set_bind_group(1, &self.uniform_bind_group, &[]);
            render_pass.draw(0..6, 0..1);
        }

        // Blit render target to surface (using cached pipeline)
        let blit_bind_group = self.blit_pipeline.create_bind_group(&self.device, &self.render_target.view);
        self.blit_pipeline.blit(&mut encoder, &blit_bind_group, &surface_view, &self.vertex_buffer);

        // Submit commands
        self.queue.submit(std::iter::once(encoder.finish()));

        // Present
        surface_texture.present();

        // Submit to outputs
        self.output_manager
            .submit_frame(&self.render_target.texture, &self.device, &self.queue);

        // FPS tracking
        self.fps_frame_count += 1;
        let elapsed = self.fps_last_time.elapsed();
        if elapsed.as_secs_f32() >= 0.5 {
            self.fps_current = self.fps_frame_count as f32 / elapsed.as_secs_f32();
            self.fps_frame_count = 0;
            self.fps_last_time = std::time::Instant::now();

            if let Ok(mut state) = self.shared_state.lock() {
                state.performance.fps = self.fps_current;
                state.performance.frame_time_ms = if self.fps_current > 0.0 {
                    1000.0 / self.fps_current
                } else {
                    0.0
                };
            }
        }

        self.frame_count += 1;
    }

    
    /// Drain the async readback pool during shutdown
    pub fn drain_readback(&mut self) {
        self.output_manager.drain_readback(&self.device);
    }
}
