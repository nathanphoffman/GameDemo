mod player;
mod textures;
mod world;

use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowId};

use player::Player;
use world::{Vertex, World};

const WALK_SPEED: f32 = 5.0;
const SPRINT_SPEED: f32 = 8.0;
const MOUSE_SENSITIVITY: f32 = 0.0022;
const FOV_Y_DEGREES: f32 = 80.0;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    cam: [f32; 4],
}

#[derive(Default)]
struct Keys {
    w: bool,
    a: bool,
    s: bool,
    d: bool,
    sprint: bool,
    jump: bool,
}

const CROSS_VERTS: u32 = 24;

fn crosshair_vertices(width: u32, height: u32) -> [[f32; 2]; CROSS_VERTS as usize] {
    // Sizes in pixels, converted to NDC.
    let (thick, len, gap) = (2.0, 10.0, 4.0);
    let px = 2.0 / width as f32;
    let py = 2.0 / height as f32;
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32| {
        [
            [x0 * px, y0 * py],
            [x1 * px, y0 * py],
            [x1 * px, y1 * py],
            [x0 * px, y0 * py],
            [x1 * px, y1 * py],
            [x0 * px, y1 * py],
        ]
    };
    let h = thick / 2.0;
    let parts = [
        rect(gap, -h, gap + len, h),
        rect(-gap - len, -h, -gap, h),
        rect(-h, gap, h, gap + len),
        rect(-h, -gap - len, h, -gap),
    ];
    let mut out = [[0.0; 2]; CROSS_VERTS as usize];
    for (i, v) in parts.iter().flatten().enumerate() {
        out[i] = *v;
    }
    out
}

fn make_depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

struct Gfx {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_view: wgpu::TextureView,
    globals_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    sky_pipeline: wgpu::RenderPipeline,
    block_pipeline: wgpu::RenderPipeline,
    cross_pipeline: wgpu::RenderPipeline,
    mesh_vertices: wgpu::Buffer,
    mesh_indices: wgpu::Buffer,
    mesh_index_count: u32,
    cross_buf: wgpu::Buffer,
}

impl Gfx {
    fn new(window: Arc<Window>, vertices: &[Vertex], indices: &[u32]) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("no suitable GPU adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .expect("failed to create device");

        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("surface not supported");
        config.present_mode = wgpu::PresentMode::Fifo;
        let swap_format = config.format.add_srgb_suffix();
        config.view_formats = vec![swap_format];
        config.format = swap_format;
        surface.configure(&device, &config);
        let depth_view = make_depth_view(&device, config.width, config.height);

        // Block texture array (grass top, grass side, dirt) with CPU-built mips.
        let layers = textures::build();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("blocks"),
            size: wgpu::Extent3d {
                width: textures::TILE,
                height: textures::TILE,
                depth_or_array_layers: textures::LAYERS,
            },
            mip_level_count: textures::MIP_LEVELS,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (layer, mips) in layers.iter().enumerate() {
            for (level, data) in mips.iter().enumerate() {
                let s = (textures::TILE >> level).max(1);
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: level as u32,
                        origin: wgpu::Origin3d {
                            x: 0,
                            y: 0,
                            z: layer as u32,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(s * 4),
                        rows_per_image: Some(s),
                    },
                    wgpu::Extent3d {
                        width: s,
                        height: s,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        // Crisp pixels up close, smoothed when far away.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let empty_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        // `world_geometry` pipelines test and write depth; the sky and crosshair just draw.
        let make_pipeline = |layout: &wgpu::PipelineLayout,
                             vs: &str,
                             fs: &str,
                             buffers: &[wgpu::VertexBufferLayout],
                             world_geometry: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(vs),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(config.format.into())],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: world_geometry,
                    depth_compare: if world_geometry {
                        wgpu::CompareFunction::Less
                    } else {
                        wgpu::CompareFunction::Always
                    },
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };

        let block_attrs =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32, 3 => Float32];
        let block_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &block_attrs,
        };
        let cross_attrs = wgpu::vertex_attr_array![0 => Float32x2];
        let cross_layout = wgpu::VertexBufferLayout {
            array_stride: 8,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &cross_attrs,
        };

        let sky_pipeline = make_pipeline(&layout, "sky_vs", "sky_fs", &[], false);
        let block_pipeline =
            make_pipeline(&layout, "block_vs", "block_fs", &[block_layout], true);
        let cross_pipeline =
            make_pipeline(&empty_layout, "cross_vs", "cross_fs", &[cross_layout], false);

        let mesh_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh vertices"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let mesh_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh indices"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let cross_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("crosshair"),
            contents: bytemuck::cast_slice(&crosshair_vertices(config.width, config.height)),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        Gfx {
            window,
            surface,
            device,
            queue,
            config,
            depth_view,
            globals_buf,
            bind_group,
            sky_pipeline,
            block_pipeline,
            cross_pipeline,
            mesh_vertices,
            mesh_indices,
            mesh_index_count: indices.len() as u32,
            cross_buf,
        }
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        self.depth_view = make_depth_view(&self.device, size.width, size.height);
        self.queue.write_buffer(
            &self.cross_buf,
            0,
            bytemuck::cast_slice(&crosshair_vertices(size.width, size.height)),
        );
    }

    fn render(&mut self, eye: Vec3, forward: Vec3) {
        let aspect = self.config.width as f32 / self.config.height as f32;
        let view = Mat4::look_to_rh(eye, forward, Vec3::Y);
        let proj = Mat4::perspective_rh(FOV_Y_DEGREES.to_radians(), aspect, 0.05, 1000.0);
        let view_proj = proj * view;
        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            cam: [eye.x, eye.y, eye.z, 1.0],
        };
        self.queue
            .write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.config.format),
            ..Default::default()
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.bind_group, &[]);

            pass.set_pipeline(&self.sky_pipeline);
            pass.draw(0..3, 0..1);

            pass.set_pipeline(&self.block_pipeline);
            pass.set_vertex_buffer(0, self.mesh_vertices.slice(..));
            pass.set_index_buffer(self.mesh_indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.mesh_index_count, 0, 0..1);

            pass.set_pipeline(&self.cross_pipeline);
            pass.set_vertex_buffer(0, self.cross_buf.slice(..));
            pass.draw(0..CROSS_VERTS, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        self.window.pre_present_notify();
        frame.present();
    }
}

// ---------------------------------------------------------------- app

struct App {
    gfx: Option<Gfx>,
    world: World,
    player: Player,
    keys: Keys,
    last_frame: Instant,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("GameDemo")
                        .with_fullscreen(Some(Fullscreen::Borderless(None))),
                )
                .unwrap(),
        );
        let _ = window
            .set_cursor_grab(CursorGrabMode::Locked)
            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
        window.set_cursor_visible(false);
        let (vertices, indices) = self.world.build_mesh();
        self.gfx = Some(Gfx::new(window, &vertices, &indices));
        self.last_frame = Instant::now();
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            self.player.yaw += dx as f32 * MOUSE_SENSITIVITY;
            self.player.pitch -= dy as f32 * MOUSE_SENSITIVITY;
            let limit = 89.0_f32.to_radians();
            self.player.pitch = self.player.pitch.clamp(-limit, limit);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gfx) = &mut self.gfx {
                    gfx.resize(size);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let down = event.state == ElementState::Pressed;
                if let PhysicalKey::Code(code) = event.physical_key {
                    match code {
                        KeyCode::KeyW => self.keys.w = down,
                        KeyCode::KeyA => self.keys.a = down,
                        KeyCode::KeyS => self.keys.s = down,
                        KeyCode::KeyD => self.keys.d = down,
                        KeyCode::ShiftLeft => self.keys.sprint = down,
                        KeyCode::Space => self.keys.jump = down,
                        KeyCode::Escape if down => event_loop.exit(),
                        _ => {}
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - self.last_frame).as_secs_f32().min(0.1);
                self.last_frame = now;

                let (forward, right) = self.player.flat_axes();
                let mut dir = Vec3::ZERO;
                if self.keys.w {
                    dir += forward;
                }
                if self.keys.s {
                    dir -= forward;
                }
                if self.keys.d {
                    dir += right;
                }
                if self.keys.a {
                    dir -= right;
                }
                let speed = if self.keys.sprint { SPRINT_SPEED } else { WALK_SPEED };
                let velocity = dir.normalize_or_zero() * speed;
                self.player.update(dt, velocity, self.keys.jump, &self.world);

                if let Some(gfx) = &mut self.gfx {
                    gfx.render(self.player.eye(), self.player.forward());
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(gfx) = &self.gfx {
            gfx.window.request_redraw();
        }
    }
}

fn main() {
    // Optional: pass a number to get the same world again, e.g. `cargo run --release -- 42`.
    let seed = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.subsec_nanos() ^ d.as_secs() as u32)
                .unwrap_or(1)
        });
    println!("world seed: {seed}");

    let world = World::generate(seed);
    let player = Player::new(world.spawn_point());

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        gfx: None,
        world,
        player,
        keys: Keys::default(),
        last_frame: Instant::now(),
    };
    event_loop.run_app(&mut app).unwrap();
}
