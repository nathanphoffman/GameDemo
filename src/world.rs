use bytemuck::{Pod, Zeroable};
use glam::Vec3;

pub const SIZE_X: usize = 240;
pub const SIZE_Z: usize = 240;
pub const SIZE_Y: usize = 64;

/// Small margin so a box resting exactly on a block boundary isn't treated as inside it.
pub const EPS: f32 = 1e-4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    Air,
    Grass,
    Dirt,
    Snow,
    Stone,
    Log,
    Leaves,
    PineLeaves,
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
    7.0 + t * 10.0
}

/// Tall ridged ranges with peaks, separated by deep valleys.
fn mountain_height(x: f32, z: f32, seed: u32) -> f32 {
    let m = fbm_with(x / 90.0, z / 90.0, seed ^ 0xA11, 3, 0.45, true);
    let t = ((m - 0.2) / 0.7).clamp(0.0, 1.0);
    3.0 + 38.0 * t.powf(1.5)
}

pub(crate) struct Face {
    pub(crate) normal: [i32; 3],
    pub(crate) corners: [[f32; 3]; 4],
    pub(crate) uv: [[f32; 2]; 4],
    pub(crate) shade: f32,
    /// 0 = top, 1 = side, 2 = bottom
    pub(crate) kind: u8,
}

const SIDE_UV: [[f32; 2]; 4] = [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]];
const FLAT_UV: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];

/// +Y, -Y, +X, -X, +Z, -Z
pub(crate) const FACES: [Face; 6] = [
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
        let mut world = World { blocks, heights };
        world.plant_trees(seed);
        world
    }

    fn set_if_air(&mut self, x: i32, y: i32, z: i32, block: Block) {
        if x < 0 || y < 0 || z < 0 || x >= SIZE_X as i32 || y >= SIZE_Y as i32 || z >= SIZE_Z as i32
        {
            return;
        }
        let i = Self::index(x as usize, y as usize, z as usize);
        if self.blocks[i] == Block::Air {
            self.blocks[i] = block;
        }
    }

    /// Oaks on grass (common), pines on snow (rarer, and rarer still as the ground gets high).
    /// Each cell of a coarse grid gets at most one tree, which keeps trees from piling up.
    fn plant_trees(&mut self, seed: u32) {
        let (spawn_x, spawn_z) = (SIZE_X / 2, SIZE_Z / 2);
        for (cell, is_pine) in [(5usize, false), (7usize, true)] {
            for cz in 0..SIZE_Z / cell {
                for cx in 0..SIZE_X / cell {
                    let salt = if is_pine { 0x71AE } else { 0x0A4B };
                    let (cx32, cz32) = (cx as u32, cz as u32);
                    let x = cx * cell + (hash(cx32, cz32, seed ^ salt) * cell as f32) as usize;
                    let z = cz * cell + (hash(cx32, cz32, seed ^ salt ^ 1) * cell as f32) as usize;
                    // Keep canopies inside the world and the spawn point clear.
                    if x < 3 || z < 3 || x + 3 >= SIZE_X || z + 3 >= SIZE_Z {
                        continue;
                    }
                    if x.abs_diff(spawn_x) <= 4 && z.abs_diff(spawn_z) <= 4 {
                        continue;
                    }
                    let ground = self.heights[z * SIZE_X + x] as i32;
                    let top = self.get(x as i32, ground - 1, z as i32);
                    let roll = hash(cx32, cz32, seed ^ salt ^ 2);
                    let size_roll = hash(cx32, cz32, seed ^ salt ^ 3);
                    let (x, z) = (x as i32, z as i32);
                    if is_pine {
                        if top != Block::Snow {
                            continue;
                        }
                        // Full chance on low ground, fading to none by height 30.
                        let high = ((ground as f32 - 12.0) / 18.0).clamp(0.0, 1.0);
                        if roll < 0.55 * (1.0 - high) {
                            self.plant_pine(x, ground, z, size_roll);
                        }
                    } else if top == Block::Grass && roll < 0.6 {
                        self.plant_oak(x, ground, z, size_roll);
                    }
                }
            }
        }
    }

    fn plant_oak(&mut self, x: i32, ground: i32, z: i32, size_roll: f32) {
        let trunk = 4 + (size_roll * 2.0) as i32; // 4 or 5
        for dy in 0..trunk {
            self.set_if_air(x, ground + dy, z, Block::Log);
        }
        // Rounded canopy: two wide layers, then a smaller cap.
        for (dy, radius) in [(trunk - 2, 2i32), (trunk - 1, 2), (trunk, 1), (trunk + 1, 1)] {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    let corner = dx.abs() == radius && dz.abs() == radius;
                    if corner && (radius == 2 || dy == trunk + 1) {
                        continue;
                    }
                    self.set_if_air(x + dx, ground + dy, z + dz, Block::Leaves);
                }
            }
        }
    }

    fn plant_pine(&mut self, x: i32, ground: i32, z: i32, size_roll: f32) {
        let trunk = 6 + (size_roll * 3.0) as i32; // 6 to 8
        for dy in 0..trunk {
            self.set_if_air(x, ground + dy, z, Block::Log);
        }
        // Tiered cone: widest near the bottom, with a single needle at the very top.
        let top = ground + trunk;
        for y in (ground + 2)..=top {
            let d = top - y;
            let radius: i32 = match d {
                0 => 0,
                1 | 2 => 1,
                n if n % 2 == 1 => 2,
                _ => 1,
            };
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if radius == 2 && dx.abs() == 2 && dz.abs() == 2 {
                        continue;
                    }
                    self.set_if_air(x + dx, y, z + dz, Block::PineLeaves);
                }
            }
        }
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

    /// Walks a ray through the grid and returns the first solid block within `max_dist`.
    pub fn raycast(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<(i32, i32, i32)> {
        let dir = dir.normalize_or_zero();
        if dir == Vec3::ZERO {
            return None;
        }
        let mut cell = [
            origin.x.floor() as i32,
            origin.y.floor() as i32,
            origin.z.floor() as i32,
        ];
        let o = origin.to_array();
        let d = dir.to_array();
        let mut step = [0i32; 3];
        let mut t_max = [f32::INFINITY; 3];
        let mut t_delta = [f32::INFINITY; 3];
        for i in 0..3 {
            if d[i] > 0.0 {
                step[i] = 1;
                t_delta[i] = 1.0 / d[i];
                t_max[i] = ((cell[i] + 1) as f32 - o[i]) / d[i];
            } else if d[i] < 0.0 {
                step[i] = -1;
                t_delta[i] = -1.0 / d[i];
                t_max[i] = (o[i] - cell[i] as f32) / -d[i];
            }
        }
        loop {
            if self.get(cell[0], cell[1], cell[2]) != Block::Air {
                return Some((cell[0], cell[1], cell[2]));
            }
            // Step into whichever neighbouring cell the ray reaches first.
            let axis = if t_max[0] < t_max[1] && t_max[0] < t_max[2] {
                0
            } else if t_max[1] < t_max[2] {
                1
            } else {
                2
            };
            if t_max[axis] > max_dist {
                return None;
            }
            cell[axis] += step[axis];
            t_max[axis] += t_delta[axis];
        }
    }

    /// Removes a block and returns what it was. The bottom layer is kept so there's never a
    /// hole to the void.
    pub fn remove_block(&mut self, x: i32, y: i32, z: i32) -> Option<Block> {
        let block = self.get(x, y, z);
        if y <= 0 || block == Block::Air {
            return None;
        }
        self.blocks[Self::index(x as usize, y as usize, z as usize)] = Block::Air;
        Some(block)
    }

    /// The six faces of one block, pushed out a hair so they sit on top of the real faces.
    pub fn block_overlay(x: i32, y: i32, z: i32, layer: f32) -> ([Vertex; 24], [u32; 36]) {
        const GROW: f32 = 0.003;
        let mut vertices = [Vertex {
            pos: [0.0; 3],
            uv: [0.0; 2],
            layer,
            shade: 1.0,
        }; 24];
        let mut indices = [0u32; 36];
        for (f, face) in FACES.iter().enumerate() {
            for i in 0..4 {
                let c = face.corners[i];
                let at = |offset: i32, c: f32| offset as f32 - GROW + c * (1.0 + 2.0 * GROW);
                vertices[f * 4 + i].pos = [at(x, c[0]), at(y, c[1]), at(z, c[2])];
                vertices[f * 4 + i].uv = face.uv[i];
            }
            let base = (f * 4) as u32;
            indices[f * 6..f * 6 + 6]
                .copy_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        (vertices, indices)
    }

    /// A wireframe box around one block, built from 12 thin bars so it has a visible
    /// thickness at any distance (GPU lines are always one pixel wide).
    pub fn block_outline(x: i32, y: i32, z: i32, layer: f32) -> (Vec<Vertex>, Vec<u32>) {
        let min = Vec3::new(x as f32, y as f32, z as f32);
        Self::box_outline(min, min + Vec3::ONE, layer)
    }

    /// Same wireframe, around any axis-aligned box.
    pub fn box_outline(min_corner: Vec3, max_corner: Vec3, layer: f32) -> (Vec<Vertex>, Vec<u32>) {
        const HALF: f32 = 0.012; // half the bar thickness
        // Corner of the unit cube from a 3-bit index (bit 0 = x, 1 = y, 2 = z).
        const QUADS: [[u32; 4]; 6] = [
            [0, 2, 6, 4],
            [1, 3, 7, 5],
            [0, 1, 5, 4],
            [2, 3, 7, 6],
            [0, 1, 3, 2],
            [4, 5, 7, 6],
        ];
        let (lo, hi) = (min_corner.to_array(), max_corner.to_array());
        let mut vertices = Vec::with_capacity(96);
        let mut indices = Vec::with_capacity(432);
        for axis in 0..3 {
            for corner in 0..4 {
                // Which two of the other axes this bar sits on (each at 0 or 1).
                let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
                let mut min = [0.0f32; 3];
                let mut max = [0.0f32; 3];
                min[axis] = lo[axis] - HALF;
                max[axis] = hi[axis] + HALF;
                for (k, bit) in [(a, corner & 1), (b, corner >> 1)] {
                    let edge = if bit == 1 { hi[k] } else { lo[k] };
                    min[k] = edge - HALF;
                    max[k] = edge + HALF;
                }
                let base = vertices.len() as u32;
                for i in 0..8u32 {
                    let pick = |k: usize, bit: u32| if bit == 1 { max[k] } else { min[k] };
                    vertices.push(Vertex {
                        pos: [
                            pick(0, i & 1),
                            pick(1, (i >> 1) & 1),
                            pick(2, (i >> 2) & 1),
                        ],
                        uv: [0.0, 0.0],
                        layer,
                        shade: 1.0,
                    });
                }
                for q in QUADS {
                    let [p, r, s, t] = q.map(|i| base + i);
                    indices.extend_from_slice(&[p, r, s, p, s, t]);
                }
            }
        }
        (vertices, indices)
    }

    /// A flat, open spot ahead of the spawn point (the player starts facing -Z) with room for
    /// a 3x3x3 skeleton. Returns the bottom center.
    pub fn enemy_spot(&self) -> Vec3 {
        let (cx, cz) = ((SIZE_X / 2) as i32, (SIZE_Z / 2) as i32);
        for dist in 7..60 {
            for dx in [0, 3, -3, 6, -6] {
                let (x, z) = (cx + dx, cz - dist);
                if x < 2 || z < 2 || x + 2 >= SIZE_X as i32 || z + 2 >= SIZE_Z as i32 {
                    continue;
                }
                let ground = self.heights[z as usize * SIZE_X + x as usize] as i32;
                let clear = (0..3).all(|dy| {
                    (-1..=1).all(|ox| {
                        (-1..=1).all(|oz| self.get(x + ox, ground + dy, z + oz) == Block::Air)
                    })
                });
                if clear {
                    return Vec3::new(x as f32 + 0.5, ground as f32, z as f32 + 0.5);
                }
            }
        }
        self.spawn_point() + Vec3::new(0.0, 0.0, -7.0)
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
                            (Block::Log, 1) => 6.0,
                            (Block::Log, _) => 7.0,
                            (Block::Leaves, _) => 8.0,
                            (Block::PineLeaves, _) => 9.0,
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
            assert!(hill_max <= 22, "hills too tall: {hill_max}");
            assert!(mountain_max >= 30, "mountains too short: {mountain_max}");
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

    #[test]
    fn oaks_are_common_and_pines_are_rarer_and_fade_with_height() {
        for seed in [1, 2, 3, 99] {
            let w = World::generate(seed);
            let (mut oaks, mut pines) = (0, 0);
            // [trees, snow columns] for low and high ground.
            let (mut low, mut high) = ([0u32; 2], [0u32; 2]);
            for z in 0..SIZE_Z {
                for x in 0..SIZE_X {
                    let ground = w.heights[z * SIZE_X + x] as i32;
                    let (xi, zi) = (x as i32, z as i32);
                    let below = w.get(xi, ground - 1, zi);
                    let has_tree = w.get(xi, ground, zi) == Block::Log;
                    if has_tree && below == Block::Grass {
                        oaks += 1;
                    }
                    if has_tree && below == Block::Snow {
                        pines += 1;
                    }
                    if below == Block::Snow {
                        let band = if ground <= 14 {
                            Some(&mut low)
                        } else if ground >= 26 {
                            Some(&mut high)
                        } else {
                            None
                        };
                        if let Some(b) = band {
                            b[1] += 1;
                            b[0] += has_tree as u32;
                        }
                    }
                }
            }
            assert!(oaks > pines * 2, "oaks {oaks} pines {pines}");
            assert!(pines > 20, "pines {pines}");
            let density = |b: [u32; 2]| b[0] as f32 / b[1].max(1) as f32;
            assert!(density(high) < density(low) * 0.3, "low {low:?} high {high:?}");
            // Spawn is clear.
            let s = w.spawn_point();
            assert_eq!(w.get(s.x as i32, s.y as i32, s.z as i32), Block::Air);
        }
    }

    #[test]
    fn raycast_hits_ground_and_mining_removes_it() {
        let mut w = World::generate(5);
        let s = w.spawn_point();
        // Looking straight down from the eye hits the block we're standing on.
        let hit = w
            .raycast(s + Vec3::new(0.0, 1.62, 0.0), Vec3::NEG_Y, 5.0)
            .expect("ground within reach");
        assert_eq!(hit.1, s.y.floor() as i32 - 1);
        assert!(w.remove_block(hit.0, hit.1, hit.2).is_some());
        assert_eq!(w.get(hit.0, hit.1, hit.2), Block::Air);
        // Straight up from the eye there is nothing in reach.
        assert!(w.raycast(s + Vec3::new(0.0, 1.62, 0.0), Vec3::Y, 5.0).is_none());
        // The bottom layer can't be mined.
        assert!(w.remove_block(10, 0, 10).is_none());
    }

    #[test]
    fn outline_has_twelve_bars_around_the_block() {
        let (v, i) = World::block_outline(3, 4, 5, 12.0);
        assert_eq!(v.len(), 12 * 8);
        assert_eq!(i.len(), 12 * 36);
        assert!(i.iter().all(|&n| (n as usize) < v.len()));
        for vert in &v {
            for (p, o) in vert.pos.iter().zip([3.0, 4.0, 5.0]) {
                assert!(*p >= o - 0.02 && *p <= o + 1.02);
            }
        }
    }

    #[test]
    fn enemy_spot_is_open_and_on_the_ground() {
        for seed in [1, 2, 3, 99] {
            let w = World::generate(seed);
            let p = w.enemy_spot();
            let (x, y, z) = (p.x.floor() as i32, p.y as i32, p.z.floor() as i32);
            assert_ne!(w.get(x, y - 1, z), Block::Air, "standing on something");
            for dy in 0..3 {
                assert_eq!(w.get(x, y + dy, z), Block::Air);
            }
        }
    }
}
