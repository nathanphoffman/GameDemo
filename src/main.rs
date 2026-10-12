mod hud;
mod music;
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
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowId};

use hud::{HudVertex, MAX_HUD_VERTICES};
use player::Player;
use world::{Vertex, World};

const WALK_SPEED: f32 = 5.0;
const SPRINT_SPEED: f32 = 8.0;
const MOUSE_SENSITIVITY: f32 = 0.0022;
const FOV_Y_DEGREES: f32 = 80.0;
const MINE_INTERVAL: f32 = 3.0; // seconds between automatic clicks
const REACH: f32 = 5.0; // blocks
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
    /// Left mouse button held.
    mining: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Mine,
    Attack,
    Off,
}

impl Mode {
    const ALL: [Mode; 3] = [Mode::Mine, Mode::Attack, Mode::Off];

    fn step(self, forward: bool) -> Mode {
        let i = Mode::ALL.iter().position(|m| *m == self).unwrap();
        let n = Mode::ALL.len();
        Mode::ALL[if forward { (i + 1) % n } else { (i + n - 1) % n }]
    }

    fn label(self) -> &'static str {
        match self {
            Mode::Mine => "MINE",
            Mode::Attack => "ATTACK",
            Mode::Off => "OFF",
        }
    }

    fn color(self) -> [f32; 3] {
        match self {
            Mode::Mine => [0.85, 0.87, 0.92],
            Mode::Attack => [0.95, 0.35, 0.30],
            Mode::Off => [0.65, 0.65, 0.65],
        }
    }
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
    hud_pipeline: wgpu::RenderPipeline,
    crack_pipeline: wgpu::RenderPipeline,
    crack_vertices: wgpu::Buffer,
    crack_indices: wgpu::Buffer,
    crack_visible: bool,
    outline_vertices: wgpu::Buffer,
    outline_indices: wgpu::Buffer,
    outline_index_count: u32,
    outline_visible: bool,
    mesh_vertices: wgpu::Buffer,
    mesh_indices: wgpu::Buffer,
    mesh_index_count: u32,
    hud_buf: wgpu::Buffer,
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

        // World blocks test and write depth; the crack overlay only tests; sky and HUD just draw.
        let make_pipeline = |layout: &wgpu::PipelineLayout,
                             vs: &str,
                             fs: &str,
                             buffers: &[wgpu::VertexBufferLayout],
                             depth_write: bool,
                             depth_test: bool| {
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
                    depth_write_enabled: depth_write,
                    depth_compare: if depth_test {
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
        let hud_attrs = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x3];
        let hud_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<HudVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &hud_attrs,
        };

        let sky_pipeline = make_pipeline(&layout, "sky_vs", "sky_fs", &[], false, false);
        let block_pipeline = make_pipeline(
            &layout,
            "block_vs",
            "block_fs",
            &[block_layout.clone()],
            true,
            true,
        );
        let crack_pipeline =
            make_pipeline(&layout, "block_vs", "crack_fs", &[block_layout], false, true);
        let hud_pipeline =
            make_pipeline(&empty_layout, "hud_vs", "hud_fs", &[hud_layout], false, false);

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
        let (overlay_vertices, overlay_indices) = World::block_overlay(0, 0, 0, 10.0);
        let crack_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("crack vertices"),
            contents: bytemuck::cast_slice(&overlay_vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let crack_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("crack indices"),
            contents: bytemuck::cast_slice(&overlay_indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let (outline_v, outline_i) = World::block_outline(0, 0, 0, 12.0);
        let outline_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("outline vertices"),
            contents: bytemuck::cast_slice(&outline_v),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let outline_indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("outline indices"),
            contents: bytemuck::cast_slice(&outline_i),
            usage: wgpu::BufferUsages::INDEX,
        });
        let hud_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hud"),
            size: (MAX_HUD_VERTICES * std::mem::size_of::<HudVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
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
            hud_pipeline,
            crack_pipeline,
            crack_vertices,
            crack_indices,
            crack_visible: false,
            outline_vertices,
            outline_indices,
            outline_index_count: outline_i.len() as u32,
            outline_visible: false,
            mesh_vertices,
            mesh_indices,
            mesh_index_count: indices.len() as u32,
            hud_buf,
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
    }

    /// Draws a wireframe box around the targeted block, or hides it with None.
    fn set_outline(&mut self, target: Option<(i32, i32, i32)>) {
        self.outline_visible = target.is_some();
        if let Some((x, y, z)) = target {
            let (vertices, _) = World::block_outline(x, y, z, 12.0);
            self.queue
                .write_buffer(&self.outline_vertices, 0, bytemuck::cast_slice(&vertices));
        }
    }

    /// Shows crack stage 1 or 2 on a block, or hides the overlay with None.
    fn set_crack(&mut self, target: Option<((i32, i32, i32), u32)>) {
        self.crack_visible = target.is_some();
        if let Some(((x, y, z), stage)) = target {
            let layer = 9.0 + stage as f32; // texture layers 10 and 11
            let (vertices, _) = World::block_overlay(x, y, z, layer);
            self.queue
                .write_buffer(&self.crack_vertices, 0, bytemuck::cast_slice(&vertices));
        }
    }

    /// Replaces the world mesh, e.g. after a block was mined.
    fn update_mesh(&mut self, vertices: &[Vertex], indices: &[u32]) {
        self.mesh_vertices = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh vertices"),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        self.mesh_indices = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh indices"),
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        self.mesh_index_count = indices.len() as u32;
    }

    fn render(&mut self, eye: Vec3, forward: Vec3, hud: &[HudVertex]) {
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
        let hud = &hud[..hud.len().min(MAX_HUD_VERTICES)];
        self.queue
            .write_buffer(&self.hud_buf, 0, bytemuck::cast_slice(hud));

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

            if self.outline_visible {
                pass.set_pipeline(&self.crack_pipeline);
                pass.set_vertex_buffer(0, self.outline_vertices.slice(..));
                pass.set_index_buffer(self.outline_indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.outline_index_count, 0, 0..1);
            }

            if self.crack_visible {
                pass.set_pipeline(&self.crack_pipeline);
                pass.set_vertex_buffer(0, self.crack_vertices.slice(..));
                pass.set_index_buffer(self.crack_indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..36, 0, 0..1);
            }

            pass.set_pipeline(&self.hud_pipeline);
            pass.set_vertex_buffer(0, self.hud_buf.slice(..));
            pass.draw(0..hud.len() as u32, 0..1);
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
    mode: Mode,
    rocks: u32,
    mine_timer: f32,
    _music: Option<music::Music>,
}

impl App {
    fn mine(&mut self) {
        let Some((x, y, z)) =
            self.world
                .raycast(self.player.eye(), self.player.forward(), REACH)
        else {
            return;
        };
        if self.world.remove_block(x, y, z) {
            self.rocks += 1;
            let (vertices, indices) = self.world.build_mesh();
            if let Some(gfx) = &mut self.gfx {
                gfx.update_mesh(&vertices, &indices);
            }
        }
    }
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
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                self.keys.mining = state == ElementState::Pressed;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                if amount != 0.0 {
                    self.mode = self.mode.step(amount > 0.0);
                    self.mine_timer = 0.0;
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

                // Auto-click: every few seconds, mine whatever the crosshair is on.
                // Progress only builds while the left button is held; letting go starts over.
                if self.mode == Mode::Mine && self.keys.mining {
                    self.mine_timer += dt;
                    if self.mine_timer >= MINE_INTERVAL {
                        self.mine_timer -= MINE_INTERVAL;
                        self.mine();
                    }
                } else {
                    self.mine_timer = 0.0;
                }

                // Cracks grow with the timer: light after 1 second, heavy after 2.
                let stage = (self.mine_timer / (MINE_INTERVAL / 3.0)) as u32;
                let target = if self.mode == Mode::Mine {
                    self.world
                        .raycast(self.player.eye(), self.player.forward(), REACH)
                } else {
                    None
                };
                let crack = target
                    .filter(|_| stage >= 1)
                    .map(|block| (block, stage.min(2)));

                if let Some(gfx) = &mut self.gfx {
                    gfx.set_outline(target);
                    gfx.set_crack(crack);
                    let hud = hud::build(
                        gfx.config.width,
                        gfx.config.height,
                        self.rocks,
                        self.mode.label(),
                        self.mode.color(),
                    );
                    gfx.render(self.player.eye(), self.player.forward(), &hud);
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
        mode: Mode::Mine,
        rocks: 0,
        mine_timer: 0.0,
        _music: music::Music::start(),
    };
    event_loop.run_app(&mut app).unwrap();
}
