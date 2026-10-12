//! Draws one frame.

use glam::{Mat4, Vec3};

use super::{Gfx, Globals, DRAW_DISTANCE, FOV_Y_DEGREES};
use crate::world::{CHUNK, CHUNKS_X};
use crate::hud::{HudVertex, MAX_HUD_VERTICES};
use crate::mesh;
use crate::weather::Weather;

impl Gfx {
    pub(crate) fn render(&mut self, eye: Vec3, forward: Vec3, time: f32, weather: &Weather, hud: &[HudVertex]) {
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
            sky: [
                weather.cloud_offset[0],
                weather.cloud_offset[1],
                if weather.underwater { 1.0 } else { 0.0 },
                0.0,
            ],
        };

        // Rain and wind streaks, placed in the world around the player.
        // No rain or wind streaks under water.
        let (rain, wind) = if weather.underwater {
            (0.0, 0.0)
        } else {
            (weather.params.rain, weather.params.wind)
        };
        let streaks = mesh::streaks::streak_mesh(eye, time, rain, wind, weather.snowing);
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
            for (i, chunk) in self.chunks.iter().enumerate() {
                let Some(chunk) = chunk else { continue };
                // Straight-line distance from the camera to the chunk's square, sideways.
                let min_x = ((i % CHUNKS_X) * CHUNK) as f32;
                let min_z = ((i / CHUNKS_X) * CHUNK) as f32;
                let dx = (min_x - eye.x).max(eye.x - (min_x + CHUNK as f32)).max(0.0);
                let dz = (min_z - eye.z).max(eye.z - (min_z + CHUNK as f32)).max(0.0);
                if dx * dx + dz * dz > DRAW_DISTANCE * DRAW_DISTANCE {
                    continue;
                }
                pass.set_vertex_buffer(0, chunk.vertices.slice(..));
                pass.set_index_buffer(chunk.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..chunk.index_count, 0, 0..1);
            }

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
