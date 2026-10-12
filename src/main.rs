mod combat;
mod effects;
mod enemy;
mod hud;
mod music;
mod player;
mod resources;
mod textures;
mod weather;
mod world;

use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowId};

use hud::{HudInfo, HudVertex, MAX_HUD_VERTICES};
use combat::{Candidate, Combat, Side, MAX_PLAYER_HP};
use effects::{Effect, Fires, STAIR_STEPS};
use enemy::{Dice, Enemy, Kind, Sight};
use player::Player;
use resources::{Converter, Resources};
use weather::Weather;
use world::{Vertex, World};

const WALK_SPEED: f32 = 5.0;
const SPRINT_SPEED: f32 = 8.0;
const MOUSE_SENSITIVITY: f32 = 0.0022;
const FOV_Y_DEGREES: f32 = 80.0;
const MINE_INTERVAL: f32 = 0.55; // seconds of holding to mine one block
const REACH: f32 = 5.0; // blocks
// Hostile creatures can attack you from this close (horizontally)...
const MELEE_RANGE: f32 = 2.3;
// ...you can attack them from this close (and click a passive one to start a fight).
const NEAR_RANGE: f32 = 3.0;
// A forge this close (in blocks) doubles the conversion speed; a furnace quadruples it from
// further away.
const FORGE_RANGE: f32 = 6.0;
const FURNACE_RANGE: f32 = 10.0;
// After a round ends, the next one can't start for this long.
const ROUND_PAUSE: f32 = 2.0;
// How fast the health bar's draining chunk catches up, in hp per second.
const HP_DRAIN_PER_SECOND: f32 = 30.0;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    /// Camera position and the time in seconds.
    cam: [f32; 4],
    /// Fog start, fog end, overcast, dimming.
    fog: [f32; 4],
    /// Cloud drift (x, z); the rest is unused.
    sky: [f32; 4],
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
    /// Ctrl held: the mouse is free to click the resource list.
    ctrl: bool,
}

fn init_buffer<T: bytemuck::Pod>(
    device: &wgpu::Device,
    label: &str,
    data: &[T],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(data),
        usage,
    })
}

/// An empty vertex buffer with room for `count` vertices of `T`, refilled with `write_buffer`.
fn dynamic_vertex_buffer<T>(device: &wgpu::Device, label: &str, count: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (count * std::mem::size_of::<T>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
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
    weather_vertices: wgpu::Buffer,
    weather_indices: wgpu::Buffer,
    weather_index_count: u32,
    crack_pipeline: wgpu::RenderPipeline,
    crack_vertices: wgpu::Buffer,
    crack_indices: wgpu::Buffer,
    crack_visible: bool,
    outline_vertices: wgpu::Buffer,
    outline_indices: wgpu::Buffer,
    outline_index_count: u32,
    outline_visible: bool,
    enemy_vertices: wgpu::Buffer,
    enemy_indices: wgpu::Buffer,
    enemy_index_count: u32,
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
                             depth_test: bool,
                             blend: Option<wgpu::BlendState>| {
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
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
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

        let sky_pipeline = make_pipeline(&layout, "sky_vs", "sky_fs", &[], false, false, None);

        let block_pipeline = make_pipeline(
            &layout,
            "block_vs",
            "block_fs",
            std::slice::from_ref(&block_layout),
            true,
            true,
            None,
        );
        let crack_pipeline =
            make_pipeline(&layout, "block_vs", "crack_fs", &[block_layout], false, true, None);
        let hud_pipeline =
            make_pipeline(&empty_layout, "hud_vs", "hud_fs", &[hud_layout], false, false, None);

        const VERTEX_COPY: wgpu::BufferUsages =
            wgpu::BufferUsages::VERTEX.union(wgpu::BufferUsages::COPY_DST);
        let mesh_vertices = init_buffer(&device, "mesh vertices", vertices, wgpu::BufferUsages::VERTEX);
        let mesh_indices = init_buffer(&device, "mesh indices", indices, wgpu::BufferUsages::INDEX);
        let (overlay_vertices, overlay_indices) = World::block_overlay(0, 0, 0, 10.0);
        let crack_vertices = init_buffer(&device, "crack vertices", &overlay_vertices, VERTEX_COPY);
        let crack_indices =
            init_buffer(&device, "crack indices", &overlay_indices, wgpu::BufferUsages::INDEX);
        let (outline_v, outline_i) = World::block_outline(0, 0, 0, 12.0);
        let outline_vertices = init_buffer(&device, "outline vertices", &outline_v, VERTEX_COPY);
        let outline_indices =
            init_buffer(&device, "outline indices", &outline_i, wgpu::BufferUsages::INDEX);
        let enemy_capacity = enemy::MAX_ENEMIES * enemy::MAX_VERTICES_PER_ENEMY;
        let enemy_vertices = dynamic_vertex_buffer::<Vertex>(&device, "enemy vertices", enemy_capacity);
        let enemy_indices = init_buffer(
            &device,
            "enemy indices",
            &enemy::indices(enemy_capacity),
            wgpu::BufferUsages::INDEX,
        );
        let weather_vertices = dynamic_vertex_buffer::<Vertex>(
            &device,
            "weather streaks",
            weather::MAX_STREAK_VERTICES,
        );
        let weather_indices = init_buffer(
            &device,
            "weather indices",
            &enemy::indices(weather::MAX_STREAK_VERTICES),
            wgpu::BufferUsages::INDEX,
        );
        let hud_buf = dynamic_vertex_buffer::<HudVertex>(&device, "hud", MAX_HUD_VERTICES);

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
            weather_vertices,
            weather_indices,
            weather_index_count: 0,
            crack_pipeline,
            crack_vertices,
            crack_indices,
            crack_visible: false,
            outline_vertices,
            outline_indices,
            outline_index_count: outline_i.len() as u32,
            outline_visible: false,
            enemy_vertices,
            enemy_indices,
            enemy_index_count: 0,
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

    /// Draws a wireframe around the targeted box (a block or the enemy), or hides it.
    fn set_outline(&mut self, target: Option<(Vec3, Vec3)>) {
        self.outline_visible = target.is_some();
        if let Some((min, max)) = target {
            let (vertices, _) = World::box_outline(min, max, 12.0);
            self.queue
                .write_buffer(&self.outline_vertices, 0, bytemuck::cast_slice(&vertices));
        }
    }

    /// Uploads every enemy's body and health bar (4-vertex quads, back to back).
    fn set_enemies(&mut self, vertices: &[Vertex]) {
        let capacity = enemy::MAX_ENEMIES * enemy::MAX_VERTICES_PER_ENEMY;
        let vertices = &vertices[..vertices.len().min(capacity)];
        self.queue
            .write_buffer(&self.enemy_vertices, 0, bytemuck::cast_slice(vertices));
        self.enemy_index_count = (vertices.len() / 4 * 6) as u32;
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
        self.mesh_vertices =
            init_buffer(&self.device, "mesh vertices", vertices, wgpu::BufferUsages::VERTEX);
        self.mesh_indices =
            init_buffer(&self.device, "mesh indices", indices, wgpu::BufferUsages::INDEX);
        self.mesh_index_count = indices.len() as u32;
    }

    fn render(&mut self, eye: Vec3, forward: Vec3, time: f32, weather: &Weather, hud: &[HudVertex]) {
        let aspect = self.config.width as f32 / self.config.height as f32;
        let view = Mat4::look_to_rh(eye, forward, Vec3::Y);
        let proj = Mat4::perspective_rh(FOV_Y_DEGREES.to_radians(), aspect, 0.05, 1000.0);
        let view_proj = proj * view;
        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            // The fourth number is the time, which makes the clouds drift.
            cam: [eye.x, eye.y, eye.z, time],
            fog: [
                weather.params.fog_start,
                weather.params.fog_end,
                weather.params.overcast,
                weather.params.dim,
            ],
            sky: [weather.cloud_offset[0], weather.cloud_offset[1], 0.0, 0.0],
        };

        // Rain and wind streaks, placed in the world around the player.
        let streaks = weather::streak_mesh(eye, time, weather.params.rain, weather.params.wind);
        self.queue
            .write_buffer(&self.weather_vertices, 0, bytemuck::cast_slice(&streaks));
        self.weather_index_count = (streaks.len() / 4 * 6) as u32;
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

            if self.enemy_index_count > 0 {
                pass.set_pipeline(&self.block_pipeline);
                pass.set_vertex_buffer(0, self.enemy_vertices.slice(..));
                pass.set_index_buffer(self.enemy_indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.enemy_index_count, 0, 0..1);
            }

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

            // Rain and wind streaks: plain textured quads in the world, hidden by anything in
            // front of them.
            if self.weather_index_count > 0 {
                pass.set_pipeline(&self.crack_pipeline);
                pass.set_vertex_buffer(0, self.weather_vertices.slice(..));
                pass.set_index_buffer(self.weather_indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.weather_index_count, 0, 0..1);
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
    /// When the game started, for things that move with time (the clouds).
    started: Instant,
    weather: Weather,
    resources: Resources,
    converter: Converter,
    fires: Fires,
    /// The apply menu, open on this block, or None.
    menu: Option<(i32, i32, i32)>,
    /// What right click does: `None` clones the block it is aimed at (normal block placing),
    /// `Some(kind)` applies that dense resource to it. Changed with the scroll wheel.
    selected_dense: Option<usize>,
    /// Last known mouse position in window pixels (only meaningful while Ctrl frees it).
    cursor_pos: (f32, f32),
    mine_timer: f32,
    enemies: Vec<Enemy>,
    dice: Dice,
    /// Last roll and how long ago it happened.
    combat: Combat,
    /// Health shown on the bar; trails the real value after a hit so the loss is visible.
    hp_shown: f32,
    round_pause: f32,
    /// The window has keyboard focus (and so should own the mouse).
    focused: bool,
    /// The OS really locked the pointer. If not, it is only confined, and the game keeps
    /// pulling it back to the middle of the window so it can't wander onto another monitor.
    cursor_locked: bool,
    was_busy: bool,
    _music: Option<music::Music>,
}

impl App {
    /// True while the mouse is pointing at the HUD (Ctrl held, or the apply menu open).
    fn ui_open(&self) -> bool {
        self.keys.ctrl || self.menu.is_some()
    }

    /// Opens the apply menu on the block under the crosshair, freeing the mouse.
    fn open_menu(&mut self) {
        if self.ui_open() {
            return;
        }
        if let Some(target) = self.block_in_sights() {
            self.menu = Some(target);
            self.keys.mining = false;
            self.release_cursor();
        }
    }

    fn close_menu(&mut self) {
        self.menu = None;
        if self.focused && !self.keys.ctrl {
            self.grab_cursor();
        }
    }

    /// A left click while the menu is open: apply a resource, or close it.
    fn menu_click(&mut self) {
        let (width, height) = self
            .gfx
            .as_ref()
            .map_or((0, 0), |g| (g.config.width, g.config.height));
        let (x, y) = self.cursor_pos;
        match hud::menu_click_at(width, height, x, y) {
            None => self.close_menu(),
            Some(None) => {}
            Some(Some(kind)) => self.apply_resource(kind),
        }
    }

    /// Applies one dense resource of `kind` to the block the menu is open on. If it does
    /// nothing to that block, nothing is used up and the menu stays open.
    fn apply_resource(&mut self, kind: usize) {
        let Some(pos) = self.menu else {
            return;
        };
        if self.apply_dense(kind, pos) {
            self.close_menu();
        }
    }

    /// Right click with a dense resource selected: applies it to the block under the
    /// crosshair, just like picking it from the menu.
    fn apply_selected(&mut self, kind: usize) {
        if let Some(pos) = self.block_in_sights() {
            self.apply_dense(kind, pos);
        }
    }

    /// Applies one dense resource of `kind` to the block at `pos`, if it does anything there.
    /// Returns whether it did (and so whether a resource was used up).
    fn apply_dense(&mut self, kind: usize, (x, y, z): (i32, i32, i32)) -> bool {
        let Some(effect) = self.effect_at(kind, (x, y, z)) else {
            return false;
        };
        // A staircase takes one per step; everything else takes one.
        if !self.resources.spend_dense(kind, effect.dense_cost()) {
            return false;
        }
        use world::Block;
        let changed = match effect {
            Effect::Ignite => {
                let lit = self.world.replace_block(x, y, z, Block::Burning);
                if lit {
                    self.fires.ignite((x, y, z));
                }
                lit
            }
            Effect::Douse => self.world.replace_block(x, y, z, Block::Water),
            Effect::Freeze => self.world.replace_block(x, y, z, Block::Snow),
            Effect::Seed => self.world.replace_block(x, y, z, Block::Grass),
            Effect::GrowOak => self.world.grow_tree(x, y, z, false),
            Effect::GrowPine => self.world.grow_tree(x, y, z, true),
            Effect::LayFloor => self.world.replace_block(x, y, z, Block::WoodFloor),
            Effect::StoneFloor => self.world.replace_block(x, y, z, Block::StoneFloor),
            Effect::BuildForge => self.world.place_block(x, y + 1, z, Block::Forge),
            Effect::BuildFurnace => self.world.place_block(x, y + 1, z, Block::Furnace),
            Effect::BuildStairs => {
                let dir = self.stair_dir();
                let mut built = false;
                for (sx, sy, sz) in world::World::stair_cells(x, y, z, dir, STAIR_STEPS) {
                    built |= self.world.place_block(sx, sy, sz, Block::WoodFloor);
                }
                built
            }
            Effect::PlantHouse => self.world.place_block(x, y + 1, z, Block::HousePlant),
        };
        if changed {
            self.rebuild_mesh();
        }
        true
    }

    /// What applying dense resource `kind` to the block at `pos` would do right now: the
    /// effect for that type of block, if there is room for it (trees need space).
    fn effect_at(&self, kind: usize, pos: (i32, i32, i32)) -> Option<Effect> {
        let (x, y, z) = pos;
        let effect = effects::effect_for(kind, self.world.get(x, y, z))?;
        let fits = match effect {
            Effect::GrowOak => self.world.tree_fits(x, y, z, false),
            Effect::GrowPine => self.world.tree_fits(x, y, z, true),
            e if e.builds_on_top() => {
                self.world.get(x, y + 1, z) == world::Block::Air
                    && !self.cell_has_creature(x, y + 1, z)
            }
            Effect::BuildStairs => {
                let dir = self.stair_dir();
                self.world.stairs_fit(x, y, z, dir, STAIR_STEPS)
                    && !world::World::stair_cells(x, y, z, dir, STAIR_STEPS)
                        .any(|(sx, sy, sz)| self.cell_has_creature(sx, sy, sz))
            }
            _ => true,
        };
        fits.then_some(effect)
    }

    /// Takes the mouse: hidden, and locked (or, where locking isn't available, confined to
    /// the window). Safe to call again; it is repeated on focus and click in case the first
    /// attempt didn't take or the grab was lost.
    fn grab_cursor(&mut self) {
        let Some(gfx) = &self.gfx else {
            return;
        };
        let window = &gfx.window;
        let locked = window.set_cursor_grab(CursorGrabMode::Locked).is_ok();
        if !locked {
            let _ = window.set_cursor_grab(CursorGrabMode::Confined);
        }
        window.set_cursor_visible(false);
        self.cursor_locked = locked;
    }

    /// Gives the mouse back, for when the window isn't the one being played in.
    fn release_cursor(&self) {
        if let Some(gfx) = &self.gfx {
            let _ = gfx.window.set_cursor_grab(CursorGrabMode::None);
            gfx.window.set_cursor_visible(true);
        }
    }

    /// The block under the crosshair within reach, unless an enemy is what you're looking at.
    fn block_in_sights(&self) -> Option<(i32, i32, i32)> {
        self.block_hit_in_sights().map(|hit| hit.block)
    }

    /// Same, with the empty cell in front of the face that is being looked at.
    fn block_hit_in_sights(&self) -> Option<world::RayHit> {
        if self.enemy_in_sights().is_some() {
            return None;
        }
        self.world
            .raycast_face(self.player.eye(), self.player.forward(), REACH)
    }

    /// The closest enemy that is in reach, under the crosshair, and not behind a block.
    fn enemy_in_sights(&self) -> Option<Sight> {
        let (eye, dir) = (self.player.eye(), self.player.forward());
        let (mut best, mut best_dist) = (None, f32::INFINITY);
        for enemy in &self.enemies {
            let (min, max) = enemy.bounds();
            let Some(dist) = enemy::ray_box(eye, dir, min, max).filter(|d| *d <= REACH) else {
                continue;
            };
            if dist < best_dist {
                let offset = enemy.pos() - self.player.pos;
                best_dist = dist;
                best = Some(Sight {
                    id: enemy.id,
                    hostile: enemy.hostile,
                    min,
                    max,
                    distance: Vec3::new(offset.x, 0.0, offset.z).length(),
                });
            }
        }
        let sight = best?;
        if let Some((x, y, z)) = self.world.raycast(eye, dir, REACH) {
            let block_min = Vec3::new(x as f32, y as f32, z as f32);
            let block_dist = enemy::ray_box(eye, dir, block_min, block_min + Vec3::ONE);
            if block_dist.is_some_and(|d| d < best_dist) {
                return None;
            }
        }
        Some(sight)
    }

    /// Hostile creatures within `range` of the player (horizontally, and not on another
    /// level), as combat candidates. Dead ones are left out.
    fn hostile_within(&self, range: f32) -> Vec<Candidate> {
        self.enemies
            .iter()
            .filter(|e| e.hostile && e.health > 0)
            .filter(|e| {
                let offset = e.pos() - self.player.pos;
                Vec3::new(offset.x, 0.0, offset.z).length() <= range && offset.y.abs() < 2.5
            })
            .map(|e| Candidate {
                id: e.id,
                hp: e.health,
            })
            .collect()
    }

    /// Who can hit the player right now, and who the player can hit.
    fn fight_lists(&self) -> (Vec<Candidate>, Vec<Candidate>) {
        (
            self.hostile_within(MELEE_RANGE),
            self.hostile_within(NEAR_RANGE),
        )
    }

    /// A click picks a fight with a passive enemy you are close to and looking at. It turns
    /// hostile and a round starts with my attack. After that it fights on its own like any
    /// other hostile one. Clicking a hostile enemy does nothing: those start rounds themselves.
    fn attack(&mut self) {
        if self.combat.player_hp <= 0 {
            return;
        }
        let Some(sight) = self.enemy_in_sights() else {
            return;
        };
        if sight.hostile || sight.distance > NEAR_RANGE {
            return;
        }
        // Clicking mid-round skips ahead: everything in flight lands right now, and the
        // health bar jumps to its final value instead of draining.
        let (attackers, targets) = self.fight_lists();
        self.combat.resolve_now(&mut self.dice, &attackers, &targets);
        self.hp_shown = self.combat.player_hp as f32;
        self.sync_enemies();

        if let Some(enemy) = self.enemies.iter_mut().find(|e| e.id == sight.id) {
            enemy.provoke();
        }
        let (attackers, targets) = self.fight_lists();
        self.combat
            .begin_round(Side::Player, &attackers, &targets, &mut self.dice);
    }

    /// Copies the fight's result onto the creature it hit, and removes the dead.
    fn sync_enemies(&mut self) {
        if let Some(target) = self.combat.target
            && let Some(enemy) = self.enemies.iter_mut().find(|e| e.id == target.id) {
                enemy.health = enemy.health.min(target.hp);
            }
        self.enemies.retain(|e| e.health > 0);
    }

    /// One frame of enemy behavior: hostile ones chase the player, and once any of them is
    /// close enough (and the pause after the last round is over) a round begins.
    fn update_enemies(&mut self, dt: f32) {
        let player_alive = self.combat.player_hp > 0;
        let target = self.player.pos;
        for enemy in &mut self.enemies {
            enemy.update(dt, target, player_alive, &self.world);
        }

        self.round_pause = (self.round_pause - dt).max(0.0);
        if self.round_pause > 0.0 || self.combat.busy() || !player_alive {
            return;
        }
        let (attackers, targets) = self.fight_lists();
        if !attackers.is_empty() {
            self.combat
                .begin_round(Side::Enemy, &attackers, &targets, &mut self.dice);
        }
    }

    /// Sends the world's current blocks to the GPU.
    fn rebuild_mesh(&mut self) {
        let (vertices, indices) = self.world.build_mesh();
        if let Some(gfx) = &mut self.gfx {
            gfx.update_mesh(&vertices, &indices);
        }
    }

    fn mine(&mut self) {
        let Some((x, y, z)) =
            self.world
                .raycast(self.player.eye(), self.player.forward(), REACH)
        else {
            return;
        };
        if let Some(block) = self.world.remove_block(x, y, z) {
            self.resources.gain(block);
            self.rebuild_mesh();
        }
    }

    /// The direction the player is mostly facing, as a unit step along x or z.
    fn stair_dir(&self) -> (i32, i32) {
        let (fx, fz) = (self.player.yaw.sin(), -self.player.yaw.cos());
        if fx.abs() > fz.abs() {
            (fx.signum() as i32, 0)
        } else {
            (0, fz.signum() as i32)
        }
    }

    /// Whether the player or a creature is (partly) inside the cell at (x, y, z).
    fn cell_has_creature(&self, x: i32, y: i32, z: i32) -> bool {
        let cell_min = Vec3::new(x as f32, y as f32, z as f32);
        let cell_max = cell_min + Vec3::ONE;
        let overlaps = |min: Vec3, max: Vec3| {
            min.x < cell_max.x
                && max.x > cell_min.x
                && min.y < cell_max.y
                && max.y > cell_min.y
                && min.z < cell_max.z
                && max.z > cell_min.z
        };
        let half = Vec3::new(player::HALF_WIDTH, 0.0, player::HALF_WIDTH);
        let body = (
            self.player.pos - half,
            self.player.pos + half + Vec3::new(0.0, player::HEIGHT, 0.0),
        );
        overlaps(body.0, body.1)
            || self.enemies.iter().any(|e| {
                let (min, max) = e.bounds();
                overlaps(min, max)
            })
    }

    /// Right click: places a block of the same type as the one the crosshair is on, against
    /// the face it is on, paying what that block would give if mined. It happens instantly.
    fn place_block(&mut self) {
        let Some(hit) = self.block_hit_in_sights() else {
            return;
        };
        let Some((x, y, z)) = hit.place else {
            return;
        };
        let (bx, by, bz) = hit.block;
        let block = self.world.get(bx, by, bz);
        if !self.resources.can_afford(block) {
            return;
        }

        // Don't entomb the player or an enemy.
        if self.cell_has_creature(x, y, z) {
            return;
        }

        if self.world.place_block(x, y, z, block) {
            self.resources.spend(block);
            self.rebuild_mesh();
        }
    }

    /// Left button pressed or released: picks from the menu, picks the converter's
    /// resource (with Ctrl), or starts and stops mining and attacking.
    fn left_click(&mut self, pressed: bool) {
        if self.menu.is_some() {
            self.keys.mining = false;
            if pressed {
                self.menu_click();
            }
        } else if self.keys.ctrl {
            // Ctrl + click on a resource row picks what the converter works on.
            self.keys.mining = false;
            if pressed {
                let height = self.gfx.as_ref().map_or(0, |g| g.config.height);
                let (x, y) = self.cursor_pos;
                if let Some(row) = hud::resource_row_at(height, x, y) {
                    self.converter.toggle(row);
                }
            }
        } else {
            self.keys.mining = pressed;
            if pressed {
                if self.focused {
                    // A click is a good moment to retake the mouse if we somehow lost it.
                    self.grab_cursor();
                }
                // Every click is an attack, if the enemy is what the crosshair is on.
                self.attack();
            }
        }
    }

    /// A key went down or up. Esc backs out of the menu first, and only then quits.
    fn key_changed(&mut self, event_loop: &ActiveEventLoop, code: KeyCode, down: bool) {
        match code {
            KeyCode::KeyW => self.keys.w = down,
            KeyCode::KeyA => self.keys.a = down,
            KeyCode::KeyS => self.keys.s = down,
            KeyCode::KeyD => self.keys.d = down,
            KeyCode::ShiftLeft => self.keys.sprint = down,
            KeyCode::Space => self.keys.jump = down,
            KeyCode::ControlLeft | KeyCode::ControlRight => {
                if down != self.keys.ctrl {
                    self.keys.ctrl = down;
                    self.keys.mining = false;
                    // Free the pointer so it can be moved to the list; take it
                    // back (and recenter) when Ctrl is let go.
                    if down {
                        self.release_cursor();
                    } else if self.focused && self.menu.is_none() {
                        self.grab_cursor();
                    }
                }
            }
            KeyCode::KeyE if down => self.open_menu(),
            KeyCode::Escape if down => {
                // Esc backs out of the menu first, and only then quits.
                if self.menu.is_some() {
                    self.close_menu();
                } else {
                    event_loop.exit();
                }
            }
            _ => {}
        }

    }

    /// Runs one frame: moves the player, mining, the fight, weather and background
    /// conversion, then draws everything.
    fn redraw(&mut self) {
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

        // Mining: progress builds while the button is held on a block. Looking at the
        // enemy or away from the ground, or letting go, starts over.
        if self.keys.mining && self.block_in_sights().is_some() {
            self.mine_timer += dt;
            if self.mine_timer >= MINE_INTERVAL {
                self.mine_timer -= MINE_INTERVAL;
                self.mine();
            }
        } else {
            self.mine_timer = 0.0;
        }

        // The fight: dice age and land, damage applies, replies roll.
        let (attackers, targets) = self.fight_lists();
        self.combat
            .update(dt, &mut self.dice, &attackers, &targets);
        self.sync_enemies();
        let busy = self.combat.busy();
        if self.was_busy && !busy {
            self.round_pause = ROUND_PAUSE;
        }
        self.was_busy = busy;
        self.update_enemies(dt);
        let hp = self.combat.player_hp as f32;
        self.hp_shown = (self.hp_shown - HP_DRAIN_PER_SECOND * dt).max(hp);

        // One crack stage, halfway through the mining time.
        let stage = if self.mine_timer >= MINE_INTERVAL / 2.0 { 1 } else { 0 };
        self.weather.update(dt);

        // The conversion pipeline and the fires work in the background. Standing
        // near a forge makes the conversion twice as fast.
        let eye = self.player.eye();
        let conversion_boost = if self
            .world
            .block_within(eye, FURNACE_RANGE, world::Block::Furnace)
        {
            4
        } else if self.world.block_within(eye, FORGE_RANGE, world::Block::Forge) {
            2
        } else {
            1
        };
        self.converter
            .update(dt * conversion_boost as f32, &mut self.resources);
        let burnt_out = self.fires.update(dt);
        let mut changed = false;
        for (x, y, z) in burnt_out {
            // Only if it is still the burning block (it may have been mined).
            if self.world.get(x, y, z) == world::Block::Burning {
                self.world.replace_block(x, y, z, world::Block::Coal);
                changed = true;
            }
        }
        if changed {
            self.rebuild_mesh();
        }

        // Whatever the crosshair is on: an enemy, or else a block.
        let enemy_target = self.enemy_in_sights().map(|s| (s.min, s.max));
        let target = self.block_in_sights();
        let crack = target
            .filter(|_| stage >= 1)
            .map(|block| (block, stage));

        // One wireframe, around whichever it is.
        let outline = enemy_target.or_else(|| {
            target.map(|(x, y, z)| {
                let min = Vec3::new(x as f32, y as f32, z as f32);
                (min, min + Vec3::ONE)
            })
        });

        let look_at = self.player.pos;
        let enemy_rolls: Vec<(u32, u32, f32)> = self
            .combat
            .enemy_rolls
            .iter()
            .map(|r| r.view())
            .collect();
        let enemy_mesh: Vec<Vertex> = self
            .enemies
            .iter()
            .flat_map(|e| e.mesh(e.facing(look_at)))
            .collect();

        // Without a real pointer lock, keep the pointer parked in the middle of the
        // window. Looking around uses raw mouse motion, which this doesn't disturb.
        if self.focused && !self.cursor_locked && !self.ui_open()
            && let Some(gfx) = &self.gfx {
                let size = gfx.window.inner_size();
                let _ = gfx.window.set_cursor_position(PhysicalPosition::new(
                    size.width / 2,
                    size.height / 2,
                ));
            }

        // With a dense resource selected: what right click would build on the block
        // the crosshair is on, written over that block.
        let target_label = match (self.selected_dense, self.block_in_sights()) {
            (Some(kind), Some(pos)) if !self.ui_open() => {
                // Only if there is enough of it (a staircase needs one per step).
                let enough = |effect: &Effect| {
                    self.resources.dense[kind] >= effect.dense_cost()
                };
                self.effect_at(kind, pos).filter(enough).and_then(|effect| {
                    let (width, height) = self
                        .gfx
                        .as_ref()
                        .map_or((1.0, 1.0), |g| (g.config.width as f32, g.config.height as f32));
                    let view = Mat4::look_to_rh(
                        self.player.eye(),
                        self.player.forward(),
                        Vec3::Y,
                    );
                    let proj = Mat4::perspective_rh(
                        FOV_Y_DEGREES.to_radians(),
                        width / height,
                        0.05,
                        1000.0,
                    );
                    let center =
                        Vec3::new(pos.0 as f32 + 0.5, pos.1 as f32 + 0.5, pos.2 as f32 + 0.5);
                    hud::project_to_screen(proj * view, center, width, height)
                        .map(|(x, y)| (effect.label(), x, y))
                })
            }
            _ => None,
        };

        // What each dense resource would do to the block the menu is open on.
        let menu_labels = self.menu.map(|pos| {
            let mut labels = [None; 5];
            for (kind, label) in labels.iter_mut().enumerate() {
                *label = self
                    .effect_at(kind, pos)
                    .map(|e| (e.label(), e.dense_cost()));
            }
            labels
        });

        if let Some(gfx) = &mut self.gfx {
            gfx.set_outline(outline);
            gfx.set_crack(crack);
            gfx.set_enemies(&enemy_mesh);
            let hud = hud::build(
                gfx.config.width,
                gfx.config.height,
                &HudInfo {
                    resources: self.resources.as_array(),
                    dense: self.resources.dense,
                    menu: menu_labels,
                    converting: self
                        .converter
                        .selected()
                        .map(|kind| (kind, self.converter.fraction())),
                    ctrl_held: self.keys.ctrl,
                    weather_label: self.weather.kind.label(),
                    selection: self
                        .selected_dense
                        .map(|kind| (kind, self.resources.dense[kind])),
                    target_label,
                    conversion_boost,
                    player_roll: self.combat.player_roll.map(|r| r.view()),
                    enemy_rolls: &enemy_rolls,
                    hp: (self.hp_shown, self.combat.player_hp, MAX_PLAYER_HP),
                },
            );
            let time = self.started.elapsed().as_secs_f32();
            gfx.render(self.player.eye(), self.player.forward(), time, &self.weather, &hud);
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
        let (vertices, indices) = self.world.build_mesh();
        self.gfx = Some(Gfx::new(window, &vertices, &indices));
        self.grab_cursor();
        self.last_frame = Instant::now();
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if self.ui_open() {
            return; // the mouse is pointing at the HUD, not turning the camera
        }
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
            WindowEvent::Focused(focused) => {
                self.focused = focused;
                if focused {
                    if self.menu.is_none() {
                        self.grab_cursor();
                    }
                } else {
                    // Let go of everything so nothing stays "held" while we're away.
                    self.keys = Keys::default();
                    self.release_cursor();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos = (position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput {
                button: MouseButton::Middle,
                state: ElementState::Pressed,
                ..
            } => {
                self.open_menu();
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => self.left_click(state == ElementState::Pressed),
            WindowEvent::MouseInput {
                button: MouseButton::Right,
                state: ElementState::Pressed,
                ..
            } => {
                if !self.ui_open() {
                    match self.selected_dense {
                        None => self.place_block(),
                        Some(kind) => self.apply_selected(kind),
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                if amount != 0.0 && !self.ui_open() {
                    self.selected_dense =
                        effects::cycle_selection(self.selected_dense, amount > 0.0);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let down = event.state == ElementState::Pressed;
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.key_changed(event_loop, code, down);
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
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
    // A skeleton straight ahead of the spawn point, and a deer off to the west (the grass).
    let (cx, cz) = (world::SIZE_X as i32 / 2, world::SIZE_Z as i32 / 2);
    let enemies = vec![
        Enemy::new(1, Kind::Skeleton, world.open_spot(cx, cz - 7)),
        Enemy::new(2, Kind::Deer, world.open_spot(cx - 9, cz - 3)),
    ];

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        gfx: None,
        world,
        player,
        keys: Keys::default(),
        last_frame: Instant::now(),
        started: Instant::now(),
        weather: Weather::new(seed ^ 0x5EA7),
        // A big pile of everything to try things out with. Use `Resources::default()` for a
        // normal start.
        resources: Resources::test_kit(),
        converter: Converter::default(),
        fires: Fires::default(),
        menu: None,
        selected_dense: None,
        cursor_pos: (0.0, 0.0),
        mine_timer: 0.0,
        enemies,
        dice: Dice::new(),
        combat: Combat::new(),
        hp_shown: MAX_PLAYER_HP as f32,
        round_pause: 0.0,
        focused: true,
        cursor_locked: false,
        was_busy: false,
        _music: music::Music::start(),
    };
    event_loop.run_app(&mut app).unwrap();
}
