//! The render pipelines: sky, blocks, crack overlay and HUD.

use super::DEPTH_FORMAT;
use crate::hud::HudVertex;
use crate::world::Vertex;

pub(super) struct Pipelines {
    pub sky: wgpu::RenderPipeline,
    pub block: wgpu::RenderPipeline,
    pub crack: wgpu::RenderPipeline,
    pub hud: wgpu::RenderPipeline,
}

/// What differs between the pipelines; everything else is shared.
struct Spec<'a> {
    layout: &'a wgpu::PipelineLayout,
    vs: &'a str,
    fs: &'a str,
    buffers: &'a [wgpu::VertexBufferLayout<'a>],
    depth_write: bool,
    depth_test: bool,
}

fn make_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    spec: Spec,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(spec.vs),
        layout: Some(spec.layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(spec.vs),
            compilation_options: Default::default(),
            buffers: spec.buffers,
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(spec.fs),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: spec.depth_write,
            depth_compare: if spec.depth_test {
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
}

/// World blocks test and write depth; the crack overlay only tests; sky and HUD just draw.
pub(super) fn create_pipelines(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    bgl: &wgpu::BindGroupLayout,
) -> Pipelines {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shader.wgsl").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[bgl],
        push_constant_ranges: &[],
    });
    let empty_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

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

    let build = |spec: Spec| make_pipeline(device, &shader, format, spec);
    Pipelines {
        sky: build(Spec {
            layout: &layout,
            vs: "sky_vs",
            fs: "sky_fs",
            buffers: &[],
            depth_write: false,
            depth_test: false,
        }),
        block: build(Spec {
            layout: &layout,
            vs: "block_vs",
            fs: "block_fs",
            buffers: std::slice::from_ref(&block_layout),
            depth_write: true,
            depth_test: true,
        }),
        crack: build(Spec {
            layout: &layout,
            vs: "block_vs",
            fs: "crack_fs",
            buffers: std::slice::from_ref(&block_layout),
            depth_write: false,
            depth_test: true,
        }),
        hud: build(Spec {
            layout: &empty_layout,
            vs: "hud_vs",
            fs: "hud_fs",
            buffers: std::slice::from_ref(&hud_layout),
            depth_write: false,
            depth_test: false,
        }),
    }
}
