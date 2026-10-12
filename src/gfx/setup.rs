//! Builds the GPU state: device, surface, bind groups and buffers.

use std::sync::Arc;

use winit::window::Window;

use super::pipelines::create_pipelines;
use super::texture::upload_block_textures;
use crate::world::{CHUNKS_X, CHUNKS_Z};
use super::{dynamic_vertex_buffer, init_buffer, make_depth_view, Gfx, Globals};
use crate::hud::{HudVertex, MAX_HUD_VERTICES};
use crate::mesh::{self, Vertex};

type DeviceParts = (
    wgpu::Surface<'static>,
    wgpu::Device,
    wgpu::Queue,
    wgpu::SurfaceConfiguration,
);

fn create_device(window: &Arc<Window>) -> DeviceParts {
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
    (surface, device, queue, config)
}

/// The layout (globals, block textures, sampler) and the group that fills it.
fn create_bind_group(
    device: &wgpu::Device,
    globals_buf: &wgpu::Buffer,
    texture_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> (wgpu::BindGroupLayout, wgpu::BindGroup) {
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
                resource: wgpu::BindingResource::TextureView(texture_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    (bgl, bind_group)
}

impl Gfx {
    pub(crate) fn new(window: Arc<Window>) -> Self {
        let (surface, device, queue, config) = create_device(&window);
        let depth_view = make_depth_view(&device, config.width, config.height);
        let (texture_view, sampler) = upload_block_textures(&device, &queue);

        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (bgl, bind_group) = create_bind_group(&device, &globals_buf, &texture_view, &sampler);
        let pipelines = create_pipelines(&device, config.format, &bgl);

        const VERTEX_COPY: wgpu::BufferUsages =
            wgpu::BufferUsages::VERTEX.union(wgpu::BufferUsages::COPY_DST);
        let index = wgpu::BufferUsages::INDEX;
        let (overlay_vertices, overlay_indices) = mesh::terrain::block_overlay(0, 0, 0, 10.0);
        let crack_vertices = init_buffer(&device, "crack vertices", &overlay_vertices, VERTEX_COPY);
        let crack_indices = init_buffer(&device, "crack indices", &overlay_indices, index);
        let (outline_v, outline_i) = mesh::terrain::block_outline(0, 0, 0, 12.0);
        let outline_vertices = init_buffer(&device, "outline vertices", &outline_v, VERTEX_COPY);
        let outline_indices = init_buffer(&device, "outline indices", &outline_i, index);
        let enemy_capacity = mesh::creature::MAX_ENEMIES * mesh::creature::MAX_VERTICES_PER_ENEMY;
        let enemy_vertices = dynamic_vertex_buffer::<Vertex>(&device, "enemy vertices", enemy_capacity);
        let enemy_indices = init_buffer(&device, "enemy indices", &mesh::creature::indices(enemy_capacity), index);
        let weather_vertices =
            dynamic_vertex_buffer::<Vertex>(&device, "weather streaks", mesh::streaks::MAX_STREAK_VERTICES);
        let weather_indices = init_buffer(
            &device,
            "weather indices",
            &mesh::creature::indices(mesh::streaks::MAX_STREAK_VERTICES),
            index,
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
            sky_pipeline: pipelines.sky,
            block_pipeline: pipelines.block,
            hud_pipeline: pipelines.hud,
            weather_vertices,
            weather_indices,
            weather_index_count: 0,
            crack_pipeline: pipelines.crack,
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
            chunks: (0..CHUNKS_X * CHUNKS_Z).map(|_| None).collect(),
            hud_buf,
        }
    }
}
