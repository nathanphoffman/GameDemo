use bytemuck::{Pod, Zeroable};
use glam::Vec3;

pub const SIZE_X: usize = 240;
pub const SIZE_Z: usize = 240;
pub const SIZE_Y: usize = 64;

/// Small margin so a box resting exactly on a block boundary isn't treated as inside it.
pub const EPS: f32 = 1e-4;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Block {
    Air,
    Grass,
    Dirt,
    Snow,
    Stone,
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
fn fbm_with(x: f32, z: f32, seed: u32, octaves: u32, persistence: f32, ridged: bool) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for octave in 0..octaves {
        let mut n = value_noise(x * freq, z * freq, seed.wrapping_add(octave));
        if ridged {
            // Sharp crests along the n = 0.5 contour, which form mountain ranges.
            n = 1.0 - (2.0 * n - 1.0).abs();
        }
        sum += n * amp;
        norm += amp;
        amp *= persistence;
        freq *= 2.0;
    }
    sum / norm
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 0 = pure grass country, 1 = pure snow country. Snow is on the east side, grass on the
/// west, with a fairly narrow border in the middle. Noise only wobbles the border a little.
fn snow_amount(x: usize, z: usize, seed: u32) -> f32 {
    let (fx, fz) = (x as f32, z as f32);
    let wobble = (fbm_with(fx / 70.0, fz / 70.0, seed ^ 0xB10E, 3, 0.4, false) - 0.5) * 0.5;
    // Goes from about -0.25 on the west edge to 1.25 on the east edge.
    let coldness = 0.5 + wobble + (fx / SIZE_X as f32 - 0.5) * 1.5;
    ((coldness - 0.25) / 0.5).clamp(0.0, 1.0)
}

/// Low, long, gentle rolling hills.
fn hill_height(x: f32, z: f32, seed: u32) -> f32 {
    let n = fbm_with(x / 75.0, z / 75.0, seed, 2, 0.35, false);
    let t = ((n - 0.3) / 0.4).clamp(0.0, 1.0);
    7.0 + t * 7.0
}

/// Tall ridged ranges with peaks, separated by deep valleys.
fn mountain_height(x: f32, z: f32, seed: u32) -> f32 {
    let m = fbm_with(x / 90.0, z / 90.0, seed ^ 0xA11, 3, 0.45, true);
    let t = ((m - 0.2) / 0.7).clamp(0.0, 1.0);
    2.0 + 50.0 * t.powf(1.7)
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
    /// Random terrain: gentle hills in the grass zone, tall ranges in the snow zone. The same
    /// seed always gives the same world.
    pub fn generate(seed: u32) -> World {
        let mut blocks = vec![Block::Air; SIZE_X * SIZE_Y * SIZE_Z];
        let mut heights = vec![0u8; SIZE_X * SIZE_Z];
        for z in 0..SIZE_Z {
            for x in 0..SIZE_X {
                let snow = snow_amount(x, z, seed);
                // Terrain shape follows the zone smoothly, so hills grow into mountains.
                let w = smoothstep(snow);
                let (fx, fz) = (x as f32, z as f32);
                let hills = hill_height(fx, fz, seed);
                let mountains = mountain_height(fx, fz, seed);
                let h = (hills + (mountains - hills) * w).round() as usize;
                let h = h.clamp(1, SIZE_Y - 1);
                heights[z * SIZE_X + x] = h as u8;

                let top = Self::surface_block(x, z, snow, seed);
                // Snowy country has rock under the snow; grassy country has dirt.
                let under = if top == Block::Snow { Block::Stone } else { Block::Dirt };
                for y in 0..h {
                    blocks[Self::index(x, y, z)] = if y + 1 == h { top } else { under };
                }
            }
        }
        World { blocks, heights }
    }

    /// Picks grass or snow for a column. The jitter makes the border a speckled mix.
    fn surface_block(x: usize, z: usize, snow: f32, seed: u32) -> Block {
        let (fx, fz) = (x as f32, z as f32);
        let jitter = 0.6 * hash(x as u32, z as u32, seed ^ 0x5EED)
            + 0.4 * value_noise(fx / 5.0, fz / 5.0, seed ^ 0x5EED)
            - 0.5;
        if snow + jitter * 0.5 > 0.5 {
            Block::Snow
        } else {
            Block::Grass
        }
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
                            (Block::Snow, 0) => 3.0,
                            (Block::Snow, 1) => 4.0,
                            (Block::Stone, _) => 5.0,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hills_are_low_and_gentle_and_mountains_are_tall() {
        for seed in [1, 2, 3, 99] {
            let w = World::generate(seed);
            let column = |x: usize, z: usize| w.heights[z * SIZE_X + x] as i32;
            let (mut hill_max, mut mountain_max, mut mountain_min) = (0, 0, i32::MAX);
            for z in 0..SIZE_Z - 1 {
                for x in 0..SIZE_X - 1 {
                    if x < 80 {
                        hill_max = hill_max.max(column(x, z));
                        // No cliffs in the hills: every step can be hopped up.
                        assert!((column(x, z) - column(x + 1, z)).abs() <= 1);
                        assert!((column(x, z) - column(x, z + 1)).abs() <= 1);
                    } else if x >= 160 {
                        mountain_max = mountain_max.max(column(x, z));
                        mountain_min = mountain_min.min(column(x, z));
                    }
                }
            }
            assert!(hill_max <= 20, "hills too tall: {hill_max}");
            assert!(mountain_max >= 35, "mountains too short: {mountain_max}");
            assert!(mountain_min <= 6, "valleys too shallow: {mountain_min}");
        }
    }

    #[test]
    fn has_grass_snow_and_a_mixed_border() {
        for seed in [1, 2, 3, 99] {
            let w = World::generate(seed);
            let mut snow_per_third = [0; 3];
            for z in 0..SIZE_Z {
                for x in 0..SIZE_X {
                    let top = w.heights[z * SIZE_X + x] as i32 - 1;
                    if w.get(x as i32, top, z as i32) == Block::Snow {
                        snow_per_third[x * 3 / SIZE_X] += 1;
                    }
                }
            }
            assert!(snow_per_third[0] < snow_per_third[2], "{snow_per_third:?}");
            assert!(snow_per_third[2] > 1000, "{snow_per_third:?}");
            assert!(snow_per_third[0] < SIZE_X * SIZE_Z / 3 - 1000, "{snow_per_third:?}");
        }
    }
}
