//! Everything that talks to the GPU.

mod pipelines;
mod render;
mod setup;
mod texture;

use std::sync::Arc;

use bytemuck::{Pod, Zeroable}; 
use glam::Vec3;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::{enemy};
use crate::world::{Vertex, World, CHUNKS_X};

pub(crate) const FOV_Y_DEGREES: f32 = 80.0;
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

/// How far away chunks are still drawn, in blocks. Past the longest fog it is all haze.
const DRAW_DISTANCE: f32 = 280.0;

struct ChunkMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
}

pub(crate) struct Gfx {
    pub(crate) window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
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
    /// The world, one mesh per chunk (None while a chunk has nothing to draw).
    chunks: Vec<Option<ChunkMesh>>,
    hud_buf: wgpu::Buffer,
}

impl Gfx {
    pub(crate) fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        self.depth_view = make_depth_view(&self.device, size.width, size.height);
    }

    /// Draws a wireframe around the targeted box (a block or the enemy), or hides it.
    pub(crate) fn set_outline(&mut self, target: Option<(Vec3, Vec3)>) {
        self.outline_visible = target.is_some();
        if let Some((min, max)) = target {
            let (vertices, _) = World::box_outline(min, max, 12.0);
            self.queue
                .write_buffer(&self.outline_vertices, 0, bytemuck::cast_slice(&vertices));
        }
    }

    /// Uploads every enemy's body and health bar (4-vertex quads, back to back).
    pub(crate) fn set_enemies(&mut self, vertices: &[Vertex]) {
        let capacity = enemy::MAX_ENEMIES * enemy::MAX_VERTICES_PER_ENEMY;
        let vertices = &vertices[..vertices.len().min(capacity)];
        self.queue
            .write_buffer(&self.enemy_vertices, 0, bytemuck::cast_slice(vertices));
        self.enemy_index_count = (vertices.len() / 4 * 6) as u32;
    }

    /// Shows crack stage 1 or 2 on a block, or hides the overlay with None.
    pub(crate) fn set_crack(&mut self, target: Option<((i32, i32, i32), u32)>) {
        self.crack_visible = target.is_some();
        if let Some(((x, y, z), stage)) = target {
            let layer = 9.0 + stage as f32; // texture layers 10 and 11
            let (vertices, _) = World::block_overlay(x, y, z, layer);
            self.queue
                .write_buffer(&self.crack_vertices, 0, bytemuck::cast_slice(&vertices));
        }
    }

    /// Replaces the mesh of one chunk, e.g. after a block in it was mined.
    pub(crate) fn set_chunk(&mut self, cx: usize, cz: usize, vertices: &[Vertex], indices: &[u32]) {
        self.chunks[cz * CHUNKS_X + cx] = (!indices.is_empty()).then(|| ChunkMesh {
            vertices: init_buffer(&self.device, "chunk vertices", vertices, wgpu::BufferUsages::VERTEX),
            indices: init_buffer(&self.device, "chunk indices", indices, wgpu::BufferUsages::INDEX),
            index_count: indices.len() as u32,
        });
    }
}
