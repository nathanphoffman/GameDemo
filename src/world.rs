use bytemuck::{Pod, Zeroable};
use glam::Vec3;

pub const SIZE_X: usize = 160;
pub const SIZE_Z: usize = 160;
pub const SIZE_Y: usize = 40;

/// Small margin so a box resting exactly on a block boundary isn't treated as inside it.
pub const EPS: f32 = 1e-4;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Block {
    Air,
    Grass,
    Dirt,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub layer: f32,
    pub shade: f32,
}

pub struct World {
    blocks: Vec<Block>,
    heights: Vec<u8>,
}

pub fn hash(x: u32, y: u32, seed: u32) -> f32 {
    let mut h = x
        .wrapping_mul(374761393)
        .wrapping_add(y.wrapping_mul(668265263))
        .wrapping_add(seed.wrapping_mul(2246822519));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^= h >> 16;
    (h & 0xFFFF) as f32 / 65535.0
}

fn value_noise(x: f32, z: f32, seed: u32) -> f32 {
    let (x0, z0) = (x.floor(), z.floor());
    let (fx, fz) = (x - x0, z - z0);
    let (sx, sz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let (xi, zi) = (x0 as i32 as u32, z0 as i32 as u32);
    let a = hash(xi, zi, seed);
    let b = hash(xi.wrapping_add(1), zi, seed);
    let c = hash(xi, zi.wrapping_add(1), seed);
    let d = hash(xi.wrapping_add(1), zi.wrapping_add(1), seed);
    let top = a + (b - a) * sx;
    let bottom = c + (d - c) * sx;
    top + (bottom - top) * sz
}

/// Layered noise, returns roughly 0..1.
fn fbm(x: f32, z: f32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for octave in 0..3 {
        sum += value_noise(x * freq, z * freq, seed.wrapping_add(octave)) * amp;
        norm += amp;
        amp *= 0.4;
        freq *= 2.0;
    }
    sum / norm
}

struct Face {
    normal: [i32; 3],
    corners: [[f32; 3]; 4],
    uv: [[f32; 2]; 4],
    shade: f32,
    /// 0 = top, 1 = side, 2 = bottom
    kind: u8,
}

const SIDE_UV: [[f32; 2]; 4] = [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]];
const FLAT_UV: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];

const FACES: [Face; 6] = [
    Face {
        normal: [0, 1, 0],
        corners: [[0., 1., 0.], [0., 1., 1.], [1., 1., 1.], [1., 1., 0.]],
        uv: FLAT_UV,
        shade: 1.0,
        kind: 0,
    },
    Face {
        normal: [0, -1, 0],
        corners: [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
        uv: FLAT_UV,
        shade: 0.5,
        kind: 2,
    },
    Face {
        normal: [1, 0, 0],
        corners: [[1., 0., 0.], [1., 1., 0.], [1., 1., 1.], [1., 0., 1.]],
        uv: SIDE_UV,
        shade: 0.65,
        kind: 1,
    },
    Face {
        normal: [-1, 0, 0],
        corners: [[0., 0., 1.], [0., 1., 1.], [0., 1., 0.], [0., 0., 0.]],
        uv: SIDE_UV,
        shade: 0.65,
        kind: 1,
    },
    Face {
        normal: [0, 0, 1],
        corners: [[1., 0., 1.], [1., 1., 1.], [0., 1., 1.], [0., 0., 1.]],
        uv: SIDE_UV,
        shade: 0.8,
        kind: 1,
    },
    Face {
        normal: [0, 0, -1],
        corners: [[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]],
        uv: SIDE_UV,
        shade: 0.8,
        kind: 1,
    },
];

impl World {
    /// Random rolling hills. The same seed always gives the same world.
    pub fn generate(seed: u32) -> World {
        let mut blocks = vec![Block::Air; SIZE_X * SIZE_Y * SIZE_Z];
        let mut heights = vec![0u8; SIZE_X * SIZE_Z];
        let (min_h, max_h) = (5.0, 26.0);
        for z in 0..SIZE_Z {
            for x in 0..SIZE_X {
                let n = fbm(x as f32 / 40.0, z as f32 / 40.0, seed);
                // Noise clusters around 0.5, so stretch it to use the full height range.
                let t = ((n - 0.25) / 0.5).clamp(0.0, 1.0);
                let h = (min_h + t * (max_h - min_h)).round() as usize;
                heights[z * SIZE_X + x] = h as u8;
                for y in 0..h {
                    blocks[Self::index(x, y, z)] = if y + 1 == h { Block::Grass } else { Block::Dirt };
                }
            }
        }
        World { blocks, heights }
    }

    fn index(x: usize, y: usize, z: usize) -> usize {
        (y * SIZE_Z + z) * SIZE_X + x
    }

    pub fn get(&self, x: i32, y: i32, z: i32) -> Block {
        if x < 0 || y < 0 || z < 0 || x >= SIZE_X as i32 || y >= SIZE_Y as i32 || z >= SIZE_Z as i32
        {
            return Block::Air;
        }
        self.blocks[Self::index(x as usize, y as usize, z as usize)]
    }

    /// Solid for collision purposes: the floor and the world's sides are solid walls.
    fn solid(&self, x: i32, y: i32, z: i32) -> bool {
        if y < 0 || x < 0 || z < 0 || x >= SIZE_X as i32 || z >= SIZE_Z as i32 {
            return true;
        }
        self.get(x, y, z) != Block::Air
    }

    /// Does the box from `min` to `max` overlap any solid block?
    pub fn collides(&self, min: Vec3, max: Vec3) -> bool {
        let (x0, x1) = ((min.x + EPS).floor() as i32, (max.x - EPS).floor() as i32);
        let (y0, y1) = ((min.y + EPS).floor() as i32, (max.y - EPS).floor() as i32);
        let (z0, z1) = ((min.z + EPS).floor() as i32, (max.z - EPS).floor() as i32);
        for y in y0..=y1 {
            for z in z0..=z1 {
                for x in x0..=x1 {
                    if self.solid(x, y, z) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Feet position standing on the surface in the middle of the world.
    pub fn spawn_point(&self) -> Vec3 {
        let (x, z) = (SIZE_X / 2, SIZE_Z / 2);
        let h = self.heights[z * SIZE_X + x] as f32;
        Vec3::new(x as f32 + 0.5, h + 0.01, z as f32 + 0.5)
    }

    /// Builds one mesh containing only the block faces that touch air.
    pub fn build_mesh(&self) -> (Vec<Vertex>, Vec<u32>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for y in 0..SIZE_Y as i32 {
            for z in 0..SIZE_Z as i32 {
                for x in 0..SIZE_X as i32 {
                    let block = self.get(x, y, z);
                    if block == Block::Air {
                        continue;
                    }
                    for face in &FACES {
                        let [nx, ny, nz] = face.normal;
                        if self.get(x + nx, y + ny, z + nz) != Block::Air {
                            continue;
                        }
                        if face.kind == 2 && y == 0 {
                            continue; // never visible
                        }
                        let layer = match (block, face.kind) {
                            (Block::Grass, 0) => 0.0,
                            (Block::Grass, 1) => 1.0,
                            _ => 2.0,
                        };
                        let base = vertices.len() as u32;
                        for i in 0..4 {
                            let c = face.corners[i];
                            vertices.push(Vertex {
                                pos: [x as f32 + c[0], y as f32 + c[1], z as f32 + c[2]],
                                uv: face.uv[i],
                                layer,
                                shade: face.shade,
                            });
                        }
                        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                    }
                }
            }
        }
        (vertices, indices)
    }
}
