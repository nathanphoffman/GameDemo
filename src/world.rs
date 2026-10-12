use bytemuck::{Pod, Zeroable};

use crate::biome::{self, Biome};
use glam::Vec3;

pub const SIZE_X: usize = 1024;
pub const SIZE_Z: usize = 1024;

/// The world is meshed in square chunks of this many columns, so an edit only rebuilds one.
pub const CHUNK: usize = 32;
pub const CHUNKS_X: usize = SIZE_X / CHUNK;
pub const CHUNKS_Z: usize = SIZE_Z / CHUNK;
pub const SIZE_Y: usize = 64;

/// Water fills every column lower than this, up to (not including) this height.
pub const WATER_LEVEL: usize = 6;

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
    PineLog,
    Leaves,
    PineLeaves,
    /// On fire; turns into `Coal` after a while.
    Burning,
    Coal,
    /// Made by dousing a fire with ice.
    Water,
    /// Man-made. Wooden floor is what the other crafted things are built on.
    WoodFloor,
    StoneFloor,
    /// Speeds up converting resources while the player is near.
    Forge,
    /// A stronger forge, built on a stone floor.
    Furnace,
    HousePlant,
    /// Desert ground.
    Sand,
    /// Badlands rock comes in two alternating layers.
    RedRock,
    Clay,
    /// Volcanic ground.
    Basalt,
    /// A glowing pool surface in volcanic country.
    Magma,
    Cactus,
}

#[cfg(test)]
impl Block {
    /// Every block type, for tests that check them all.
    pub const ALL: [Block; 23] = [
        Block::Air,
        Block::Grass,
        Block::Dirt,
        Block::Snow,
        Block::Stone,
        Block::Log,
        Block::PineLog,
        Block::Leaves,
        Block::PineLeaves,
        Block::Burning,
        Block::Coal,
        Block::Water,
        Block::WoodFloor,
        Block::StoneFloor,
        Block::Forge,
        Block::Furnace,
        Block::HousePlant,
        Block::Sand,
        Block::RedRock,
        Block::Clay,
        Block::Basalt,
        Block::Magma,
        Block::Cactus,
    ];
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub layer: f32,
    pub shade: f32,
}

// Texture layers used by the house plant model.
const PLANT_POT: f32 = 28.0;
const PLANT_SOIL: f32 = 30.0;
const PLANT_LEAF: f32 = 31.0;

/// The parts of the potted plant, in cell coordinates (0 to 1): pot, stem, a leafy bush, a
/// tuft on top, and two leaves spread out in a cross. Face layers are in `FACES` order: +Y,
/// -Y, +X, -X, +Z, -Z.
const PLANT_PARTS: [([f32; 3], [f32; 3], [f32; 6]); 6] = [
    (
        [0.26, 0.0, 0.26],
        [0.74, 0.38, 0.74],
        [PLANT_SOIL, PLANT_POT, PLANT_POT, PLANT_POT, PLANT_POT, PLANT_POT],
    ),
    ([0.46, 0.38, 0.46], [0.54, 0.54, 0.54], [PLANT_LEAF; 6]),
    ([0.30, 0.50, 0.30], [0.70, 0.78, 0.70], [PLANT_LEAF; 6]),
    ([0.38, 0.78, 0.38], [0.62, 0.95, 0.62], [PLANT_LEAF; 6]),
    ([0.10, 0.58, 0.42], [0.90, 0.70, 0.58], [PLANT_LEAF; 6]),
    ([0.42, 0.58, 0.10], [0.58, 0.70, 0.90], [PLANT_LEAF; 6]),
];

/// Adds the potted plant model to a cell.
fn push_plant(vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>, x: i32, y: i32, z: i32) {
    for (min, max, layers) in PLANT_PARTS {
        for (f, face) in FACES.iter().enumerate() {
            let base = vertices.len() as u32;
            for i in 0..4 {
                let c = face.corners[i];
                let mut pos = [0.0f32; 3];
                for (k, origin) in [x, y, z].into_iter().enumerate() {
                    pos[k] = origin as f32 + min[k] + c[k] * (max[k] - min[k]);
                }
                vertices.push(Vertex {
                    pos,
                    uv: face.uv[i],
                    layer: layers[f],
                    shade: face.shade,
                });
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
}

/// What a ray found: the block it hit, and the empty cell in front of the face it hit (None
/// if the ray started inside the block).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RayHit {
    pub block: (i32, i32, i32),
    pub place: Option<(i32, i32, i32)>,
}

pub struct World {
    blocks: Vec<Block>,
    heights: Vec<u8>,
    /// One above the highest non-air block, so meshing can skip the empty sky.
    top_y: usize,
    /// Per column: which biome it is in.
    biomes: Vec<Biome>,
    /// Per chunk: changed since the mesh was last built.
    dirty: Vec<bool>,
    /// Per column: water here belongs to a pond, lake or river that is entirely in the snow
    /// biome, so it is drawn as ice.
    frozen: Vec<bool>,
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

pub(crate) fn value_noise(x: f32, z: f32, seed: u32) -> f32 {
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
pub(crate) fn fbm_with(x: f32, z: f32, seed: u32, octaves: u32, persistence: f32, ridged: bool) -> f32 {
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

pub(crate) fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Low, long, gentle rolling hills.
pub(crate) fn hill_height(x: f32, z: f32, seed: u32) -> f32 {
    let n = fbm_with(x / 75.0, z / 75.0, seed, 2, 0.35, false);
    let t = ((n - 0.3) / 0.4).clamp(0.0, 1.0);
    7.0 + t * 10.0
}

/// Tall ridged ranges with peaks, separated by deep valleys.
pub(crate) fn mountain_height(x: f32, z: f32, seed: u32) -> f32 {
    let m = fbm_with(x / 90.0, z / 90.0, seed ^ 0xA11, 3, 0.45, true);
    let t = ((m - 0.2) / 0.7).clamp(0.0, 1.0);
    3.0 + 38.0 * t.powf(1.5)
}

/// How wet a column is: 0 = dry land, up to 1 in the deepest part of a river, lake or pond.
fn wetness(x: f32, z: f32, seed: u32) -> f32 {
    // Rivers wind along the middle contour of a warped noise field.
    let warp_x = (fbm_with(x / 40.0, z / 40.0, seed ^ 0x51F7, 2, 0.5, false) - 0.5) * 50.0;
    let warp_z = (fbm_with(x / 40.0 + 17.0, z / 40.0 - 9.0, seed ^ 0x51F8, 2, 0.5, false) - 0.5) * 50.0;
    let bend = fbm_with((x + warp_x) / 110.0, (z + warp_z) / 110.0, seed ^ 0xA1BE, 2, 0.4, false);
    let river = 1.0 - (bend - 0.5).abs() / 0.02;
    // Big, slow blobs make lakes; small, fast ones make ponds.
    let lake = (fbm_with(x / 55.0, z / 55.0, seed ^ 0x1A4E, 3, 0.5, false) - 0.68) / 0.1;
    let pond = (fbm_with(x / 16.0, z / 16.0, seed ^ 0x90D, 2, 0.5, false) - 0.78) / 0.08;
    river.max(lake).max(pond).clamp(0.0, 1.0)
}

/// 1 right at the spawn point, fading to 0 forty blocks away. Keeps water off it.
fn near_spawn(x: usize, z: usize) -> f32 {
    let (dx, dz) = (x as f32 - (SIZE_X / 2) as f32, z as f32 - (SIZE_Z / 2) as f32);
    1.0 - smoothstep(((dx * dx + dz * dz).sqrt() - 20.0) / 20.0)
}

/// For every column: its biome, its ground height, and how deep a water bed is dug there
/// (`u8::MAX` for none). Worked out on every core, a band of rows each.
fn survey(seed: u32) -> Vec<(Biome, u8, u8)> {
    let layout = biome::Layout::new(seed);
    let layout = &layout;
    let mut out = vec![(Biome::Plains, 0u8, u8::MAX); SIZE_X * SIZE_Z];
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let band = SIZE_Z.div_ceil(threads) * SIZE_X;
    std::thread::scope(|scope| {
        for (n, part) in out.chunks_mut(band).enumerate() {
            scope.spawn(move || {
                for (i, cell) in part.iter_mut().enumerate() {
                    let at = n * band + i;
                    *cell = survey_column(layout, at % SIZE_X, at / SIZE_X, seed);
                }
            });
        }
    });
    out
}

fn survey_column(layout: &biome::Layout, x: usize, z: usize, seed: u32) -> (Biome, u8, u8) {
    let col = biome::column(layout, x, z, seed);
    let (fx, fz) = (x as f32, z as f32);
    let mut wet = 0.0;
    if col.water > 0.0 {
        wet = wetness(fx, fz, seed) * col.water;
    }
    if col.swamp > 0.0 {
        // A third of a swamp is pond.
        let pond = ((fbm_with(fx / 14.0, fz / 14.0, seed ^ 0x5B0C, 2, 0.5, false) - 0.56) / 0.06)
            .clamp(0.0, 1.0);
        wet = f32::max(wet, pond * col.swamp);
    }
    wet *= 1.0 - near_spawn(x, z);
    // One block deep at the edge, up to four in the middle.
    let bed = if wet > 0.0 {
        (WATER_LEVEL as f32 - 1.0 - wet * 3.0).round() as u8
    } else {
        u8::MAX
    };
    let height = col.height.round().clamp(1.0, (SIZE_Y - 1) as f32) as u8;
    (col.biome, height, bed)
}

/// Spreads `limit` outward so neighbouring columns differ by at most 1: each column ends up
/// no higher than the lowest seeded column plus its distance from it.
fn slope_limit(limit: &mut [u8]) {
    let (w, d) = (SIZE_X as i32, SIZE_Z as i32);
    // Neighbours already visited by a forward sweep, and by a backward one.
    const BEFORE: [(i32, i32); 4] = [(-1, 0), (-1, -1), (0, -1), (1, -1)];
    const AFTER: [(i32, i32); 4] = [(1, 0), (1, 1), (0, 1), (-1, 1)];
    for forward in [true, false] {
        let neighbours = if forward { BEFORE } else { AFTER };
        for step in 0..w * d {
            let i = if forward { step } else { w * d - 1 - step };
            let (x, z) = (i % w, i / w);
            let mut best = limit[i as usize];
            for (dx, dz) in neighbours {
                let (nx, nz) = (x + dx, z + dz);
                if nx >= 0 && nz >= 0 && nx < w && nz < d {
                    best = best.min(limit[(nz * w + nx) as usize].saturating_add(1));
                }
            }
            limit[i as usize] = best;
        }
    }
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
    /// Random terrain, with the biomes laid out by the climate. The same seed always gives
    /// the same world.
    pub fn generate(seed: u32) -> World {
        let survey = survey(seed);
        // Land can't be steeper than a step per block next to water, so banks are walkable.
        let mut limit: Vec<u8> = survey.iter().map(|c| c.2).collect();
        slope_limit(&mut limit);

        let mut blocks = vec![Block::Air; SIZE_X * SIZE_Y * SIZE_Z];
        let mut heights = vec![0u8; SIZE_X * SIZE_Z];
        let mut biomes = Vec::with_capacity(SIZE_X * SIZE_Z);
        for z in 0..SIZE_Z {
            for x in 0..SIZE_X {
                let i = z * SIZE_X + x;
                let (biome, ground, _) = survey[i];
                // Dry ground around the spawn point.
                let floor = (WATER_LEVEL as f32 * near_spawn(x, z)).round() as usize;
                let h = (ground.min(limit[i]) as usize).max(floor).clamp(1, SIZE_Y - 1);
                heights[i] = h as u8;
                biomes.push(biome);

                for y in 0..h {
                    blocks[Self::index(x, y, z)] = Self::ground_block(biome, y, h);
                }
                if biome != Biome::Volcanic {
                    for y in h..WATER_LEVEL {
                        blocks[Self::index(x, y, z)] = Block::Water;
                    }
                }
            }
        }
        let cold: Vec<bool> = biomes.iter().map(|b| *b == Biome::Snow).collect();
        let frozen = Self::frozen_columns(&heights, &biomes, &cold);
        // Trees reach about a dozen blocks above the ground.
        let top_y = (heights.iter().copied().max().unwrap_or(0) as usize + 12).min(SIZE_Y);
        let mut world = World {
            blocks,
            heights,
            top_y,
            biomes,
            dirty: vec![false; CHUNKS_X * CHUNKS_Z],
            frozen,
        };
        world.plant_trees(seed);
        world.dirty.fill(false);
        world
    }

    /// The block at height `y` in a column of this biome whose ground is `h` high.
    fn ground_block(biome: Biome, y: usize, h: usize) -> Block {
        let top = y + 1 == h;
        match biome {
            Biome::Volcanic => {
                if top && h <= 8 {
                    Block::Magma
                } else {
                    Block::Basalt
                }
            }
            // Beaches and lake beds.
            _ if h <= WATER_LEVEL => {
                if biome == Biome::Desert {
                    Block::Sand
                } else {
                    Block::Dirt
                }
            }
            Biome::Plains | Biome::Meadow | Biome::Forest | Biome::Swamp => {
                if top {
                    Block::Grass
                } else {
                    Block::Dirt
                }
            }
            Biome::Desert => Block::Sand,
            // Alternating bands of red rock and clay.
            Biome::Badlands => {
                if (y / 2) % 3 == 1 {
                    Block::Clay
                } else {
                    Block::RedRock
                }
            }
            Biome::Snow => {
                if top {
                    Block::Snow
                } else {
                    Block::Stone
                }
            }
        }
    }

    /// Marks the columns of every body of water that lies completely inside the snow biome.
    fn frozen_columns(heights: &[u8], biomes: &[Biome], cold: &[bool]) -> Vec<bool> {
        let wet = |i: usize| (heights[i] as usize) < WATER_LEVEL && biomes[i] != Biome::Volcanic;
        let mut frozen = vec![false; heights.len()];
        let mut seen = vec![false; heights.len()];
        for start in 0..heights.len() {
            if seen[start] || !wet(start) {
                continue;
            }
            // Flood out over this body of water, noting whether any of it is outside the cold.
            let (mut body, mut stack, mut all_cold) = (Vec::new(), vec![start], true);
            seen[start] = true;
            while let Some(i) = stack.pop() {
                body.push(i);
                all_cold &= cold[i];
                let (x, z) = (i % SIZE_X, i / SIZE_X);
                let mut next = Vec::with_capacity(4);
                if x > 0 {
                    next.push(i - 1);
                }
                if x + 1 < SIZE_X {
                    next.push(i + 1);
                }
                if z > 0 {
                    next.push(i - SIZE_X);
                }
                if z + 1 < SIZE_Z {
                    next.push(i + SIZE_X);
                }
                for n in next {
                    if !seen[n] && wet(n) {
                        seen[n] = true;
                        stack.push(n);
                    }
                }
            }
            if all_cold {
                for i in body {
                    frozen[i] = true;
                }
            }
        }
        frozen
    }

    /// The biome of the column at (x, z); outside the world it is whatever is nearest.
    pub fn biome(&self, x: i32, z: i32) -> Biome {
        let x = x.clamp(0, SIZE_X as i32 - 1) as usize;
        let z = z.clamp(0, SIZE_Z as i32 - 1) as usize;
        self.biomes[z * SIZE_X + x]
    }

    /// Notes that the mesh of the chunk holding column (x, z) is out of date, and of the
    /// next chunk over if the column is on the edge (faces there depend on it).
    fn mark_dirty(&mut self, x: i32, z: i32) {
        if x < 0 || z < 0 || x >= SIZE_X as i32 || z >= SIZE_Z as i32 {
            return;
        }
        let (x, z) = (x as usize, z as usize);
        let (cx, cz) = (x / CHUNK, z / CHUNK);
        self.dirty[cz * CHUNKS_X + cx] = true;
        if x % CHUNK == 0 && cx > 0 {
            self.dirty[cz * CHUNKS_X + cx - 1] = true;
        }
        if x % CHUNK == CHUNK - 1 && cx + 1 < CHUNKS_X {
            self.dirty[cz * CHUNKS_X + cx + 1] = true;
        }
        if z % CHUNK == 0 && cz > 0 {
            self.dirty[(cz - 1) * CHUNKS_X + cx] = true;
        }
        if z % CHUNK == CHUNK - 1 && cz + 1 < CHUNKS_Z {
            self.dirty[(cz + 1) * CHUNKS_X + cx] = true;
        }
    }

    /// The chunks (x, z) whose meshes need rebuilding since the last call.
    pub fn take_dirty(&mut self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for (i, flag) in self.dirty.iter_mut().enumerate() {
            if *flag {
                *flag = false;
                out.push((i % CHUNKS_X, i / CHUNKS_X));
            }
        }
        out
    }

    fn set_if_air(&mut self, x: i32, y: i32, z: i32, block: Block) {
        if x < 0 || y < 0 || z < 0 || x >= SIZE_X as i32 || y >= SIZE_Y as i32 || z >= SIZE_Z as i32
        {
            return;
        }
        let i = Self::index(x as usize, y as usize, z as usize);
        if self.blocks[i] == Block::Air {
            self.blocks[i] = block;
            self.top_y = self.top_y.max(y as usize + 1);
            self.mark_dirty(x, z);
        }
    }

    /// Oaks on grass (forests have the most, then plains, meadows hardly any), bare dead trunks in swamps, pines
    /// on snow (rarer, and rarer still as the ground gets high) and cacti on sand. Each cell
    /// of a coarse grid gets at most one plant, which keeps them from piling up.
    fn plant_trees(&mut self, seed: u32) {
        #[derive(Clone, Copy, PartialEq)]
        enum Plant {
            Oak,
            Pine,
            Cactus,
        }
        let (spawn_x, spawn_z) = (SIZE_X / 2, SIZE_Z / 2);
        let passes = [(5usize, Plant::Oak), (7, Plant::Pine), (7, Plant::Cactus)];
        for (cell, plant) in passes {
            let salt = match plant {
                Plant::Oak => 0x0A4B,
                Plant::Pine => 0x71AE,
                Plant::Cactus => 0xCAC7,
            };
            for cz in 0..SIZE_Z / cell {
                for cx in 0..SIZE_X / cell {
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
                    let biome = self.biomes[z * SIZE_X + x];
                    let roll = hash(cx32, cz32, seed ^ salt ^ 2);
                    let size_roll = hash(cx32, cz32, seed ^ salt ^ 3);
                    let (x, z) = (x as i32, z as i32);
                    match plant {
                        Plant::Pine => {
                            if top != Block::Snow {
                                continue;
                            }
                            // Full chance on low ground, fading to none by height 30.
                            let high = ((ground as f32 - 12.0) / 18.0).clamp(0.0, 1.0);
                            if roll < 0.55 * (1.0 - high) {
                                self.plant_pine(x, ground, z, size_roll);
                            }
                        }
                        Plant::Oak if top == Block::Grass => {
                            let chance = match biome {
                                Biome::Forest => 0.5,
                                Biome::Plains => 0.35,
                                Biome::Meadow => 0.05,
                                Biome::Swamp => 0.12,
                                _ => 0.0,
                            };
                            if roll < chance {
                                if biome == Biome::Swamp {
                                    self.plant_dead_tree(x, ground, z, size_roll);
                                } else {
                                    self.plant_oak(x, ground, z, size_roll);
                                }
                            }
                        }
                        Plant::Cactus if top == Block::Sand && roll < 0.25 => {
                            for dy in 0..1 + (size_roll * 3.0) as i32 {
                                self.set_if_air(x, ground + dy, z, Block::Cactus);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// A bare trunk, 3 to 5 tall, for swamps.
    fn plant_dead_tree(&mut self, x: i32, ground: i32, z: i32, size_roll: f32) {
        for dy in 0..3 + (size_roll * 3.0) as i32 {
            self.set_if_air(x, ground + dy, z, Block::Log);
        }
    }

    /// Trunk height for a tree, from a roll in 0..1: oaks are 4 or 5, pines 6 to 8.
    fn trunk_height(pine: bool, size_roll: f32) -> i32 {
        if pine {
            6 + (size_roll * 3.0) as i32
        } else {
            4 + (size_roll * 2.0) as i32
        }
    }

    /// Whether a tree could grow on the block at (x, y, z): its trunk column has to be open
    /// air, and it has to be clear of the world's edges.
    pub fn tree_fits(&self, x: i32, y: i32, z: i32, pine: bool) -> bool {
        let trunk = Self::trunk_height(pine, Self::tree_roll(x, z));
        let inside = x >= 3 && z >= 3 && x + 3 < SIZE_X as i32 && z + 3 < SIZE_Z as i32;
        inside
            && y + 1 + trunk + 2 < SIZE_Y as i32
            && (1..=trunk).all(|dy| self.get(x, y + dy, z) == Block::Air)
    }

    /// Grows a tree on top of the block at (x, y, z), if there is room. The same spot always
    /// gives the same size of tree.
    pub fn grow_tree(&mut self, x: i32, y: i32, z: i32, pine: bool) -> bool {
        if !self.tree_fits(x, y, z, pine) {
            return false;
        }
        let roll = Self::tree_roll(x, z);
        if pine {
            self.plant_pine(x, y + 1, z, roll);
        } else {
            self.plant_oak(x, y + 1, z, roll);
        }
        true
    }

    fn tree_roll(x: i32, z: i32) -> f32 {
        hash(x as u32, z as u32, 0x7233)
    }

    fn plant_oak(&mut self, x: i32, ground: i32, z: i32, size_roll: f32) {
        let trunk = Self::trunk_height(false, size_roll);
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
        let trunk = Self::trunk_height(true, size_roll);
        for dy in 0..trunk {
            self.set_if_air(x, ground + dy, z, Block::PineLog);
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

    /// Solid for collision purposes: the floor and the world's sides are solid walls. Water
    /// isn't, you swim through it.
    fn solid(&self, x: i32, y: i32, z: i32) -> bool {
        if y < 0 || x < 0 || z < 0 || x >= SIZE_X as i32 || z >= SIZE_Z as i32 {
            return true;
        }
        self.get(x, y, z) != Block::Air && !self.is_liquid(x, y, z)
    }

    /// Is there water you can swim through in the cell containing this point? Ice doesn't
    /// count: that is solid.
    pub fn in_water(&self, p: Vec3) -> bool {
        self.is_liquid(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)
    }

    /// Is this cell a block of ice: water in a body that is frozen over?
    pub fn is_ice(&self, x: i32, y: i32, z: i32) -> bool {
        self.get(x, y, z) == Block::Water && self.frozen[z as usize * SIZE_X + x as usize]
    }

    /// Water that isn't frozen over.
    fn is_liquid(&self, x: i32, y: i32, z: i32) -> bool {
        self.get(x, y, z) == Block::Water && !self.frozen[z as usize * SIZE_X + x as usize]
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
        self.raycast_face(origin, dir, max_dist).map(|hit| hit.block)
    }

    /// Like `raycast`, but also reports the empty cell the ray was in just before it hit,
    /// which is where a new block would go against the face that was hit.
    pub fn raycast_face(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<RayHit> {
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
        let mut previous: Option<(i32, i32, i32)> = None;
        loop {
            let (x, y, z) = (cell[0], cell[1], cell[2]);
            if self.get(x, y, z) != Block::Air && !self.is_liquid(x, y, z) {
                return Some(RayHit {
                    block: (cell[0], cell[1], cell[2]),
                    place: previous,
                });
            }
            previous = Some((cell[0], cell[1], cell[2]));
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

    /// The cells of a staircase that climbs away from the block at (x, y, z): one step per
    /// cell, each one block further along (dx, dz) and one block higher.
    pub fn stair_cells(
        x: i32,
        y: i32,
        z: i32,
        (dx, dz): (i32, i32),
        steps: i32,
    ) -> impl Iterator<Item = (i32, i32, i32)> {
        (1..=steps).map(move |i| (x + dx * i, y + i, z + dz * i))
    }

    /// Whether every step of such a staircase is inside the world and empty.
    pub fn stairs_fit(&self, x: i32, y: i32, z: i32, dir: (i32, i32), steps: i32) -> bool {
        Self::stair_cells(x, y, z, dir, steps).all(|(sx, sy, sz)| {
            let inside = sx >= 0
                && sz >= 0
                && sy >= 0
                && sx < SIZE_X as i32
                && sz < SIZE_Z as i32
                && sy < SIZE_Y as i32;
            inside && self.get(sx, sy, sz) == Block::Air
        })
    }

    /// Whether there is a `block` within `radius` of `center` (measured to block centers).
    pub fn block_within(&self, center: Vec3, radius: f32, block: Block) -> bool {
        let r = radius.ceil() as i32;
        let (cx, cy, cz) = (
            center.x.floor() as i32,
            center.y.floor() as i32,
            center.z.floor() as i32,
        );
        for y in cy - r..=cy + r {
            for z in cz - r..=cz + r {
                for x in cx - r..=cx + r {
                    if self.get(x, y, z) != block {
                        continue;
                    }
                    let to = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5) - center;
                    if to.length() <= radius {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Puts a block in an empty cell inside the world. Returns whether it was placed.
    pub fn place_block(&mut self, x: i32, y: i32, z: i32, block: Block) -> bool {
        let inside = x >= 0
            && y >= 0
            && z >= 0
            && x < SIZE_X as i32
            && y < SIZE_Y as i32
            && z < SIZE_Z as i32;
        if !inside
            || block == Block::Air
            || (self.get(x, y, z) != Block::Air && !self.is_liquid(x, y, z))
        {
            return false;
        }
        self.blocks[Self::index(x as usize, y as usize, z as usize)] = block;
        self.top_y = self.top_y.max(y as usize + 1);
        self.mark_dirty(x, z);
        true
    }

    /// Swaps an existing block for another (e.g. grass for fire). Returns whether it did.
    pub fn replace_block(&mut self, x: i32, y: i32, z: i32, block: Block) -> bool {
        if block == Block::Air || self.get(x, y, z) == Block::Air {
            return false;
        }
        self.blocks[Self::index(x as usize, y as usize, z as usize)] = block;
        self.mark_dirty(x, z);
        true
    }

    /// Removes a block and returns what it was. The bottom layer is kept so there's never a
    /// hole to the void.
    pub fn remove_block(&mut self, x: i32, y: i32, z: i32) -> Option<Block> {
        let block = self.get(x, y, z);
        if y <= 0 || block == Block::Air {
            return None;
        }
        self.blocks[Self::index(x as usize, y as usize, z as usize)] = Block::Air;
        self.mark_dirty(x, z);
        self.flow_into(x, y, z);
        Some(block)
    }

    fn set_cell(&mut self, x: i32, y: i32, z: i32, block: Block) {
        self.blocks[Self::index(x as usize, y as usize, z as usize)] = block;
        self.mark_dirty(x, z);
    }

    /// A cell that was just dug out fills with water if there is water above it or beside
    /// it. The water above doesn't go down: the hole just extends the water.
    fn flow_into(&mut self, x: i32, y: i32, z: i32) {
        let wet = [(x, y + 1, z), (x + 1, y, z), (x - 1, y, z), (x, y, z + 1), (x, y, z - 1)]
            .into_iter()
            .any(|(a, b, c)| self.is_liquid(a, b, c));
        if wet {
            self.set_cell(x, y, z, Block::Water);
        }
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

    /// The closest spot to (`want_x`, `want_z`) with room for a 3x3x3 creature standing on the
    /// ground, searching outward in rings. Returns the bottom center.
    pub fn open_spot(&self, want_x: i32, want_z: i32) -> Vec3 {
        for radius in 0..40i32 {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    // Only the outline of this ring; the inside was checked already.
                    if dx.abs().max(dz.abs()) != radius {
                        continue;
                    }
                    if let Some(spot) = self.standing_spot(want_x + dx, want_z + dz) {
                        return spot;
                    }
                }
            }
        }
        self.spawn_point()
    }

    /// Standing on the ground in column (x, z), if there is room there for a 3x3x3 creature.
    /// Returns the bottom center.
    pub fn standing_spot(&self, x: i32, z: i32) -> Option<Vec3> {
        if x < 2 || z < 2 || x + 2 >= SIZE_X as i32 || z + 2 >= SIZE_Z as i32 {
            return None;
        }
        let ground = self.heights[z as usize * SIZE_X + x as usize] as i32;
        let clear = (0..3).all(|dy| {
            (-1..=1).all(|ox| {
                (-1..=1).all(|oz| self.get(x + ox, ground + dy, z + oz) == Block::Air)
            })
        });
        clear.then(|| Vec3::new(x as f32 + 0.5, ground as f32, z as f32 + 0.5))
    }

    /// Feet position standing on the surface in the middle of the world.
    pub fn spawn_point(&self) -> Vec3 {
        let (x, z) = (SIZE_X / 2, SIZE_Z / 2);
        let h = self.heights[z * SIZE_X + x] as f32;
        Vec3::new(x as f32 + 0.5, h + 0.01, z as f32 + 0.5)
    }

    /// Every chunk's mesh in one (for tests).
    #[cfg(test)]
    pub fn build_mesh(&self) -> (Vec<Vertex>, Vec<u32>) {
        let (mut vertices, mut indices) = (Vec::new(), Vec::new());
        for cz in 0..CHUNKS_Z {
            for cx in 0..CHUNKS_X {
                let (v, i) = self.build_chunk(cx, cz);
                let base = vertices.len() as u32;
                vertices.extend(v);
                indices.extend(i.into_iter().map(|n| n + base));
            }
        }
        (vertices, indices)
    }

    /// Builds the mesh of one chunk of columns, containing only the block faces that touch air.
    pub fn build_chunk(&self, cx: usize, cz: usize) -> (Vec<Vertex>, Vec<u32>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let (x0, z0) = ((cx * CHUNK) as i32, (cz * CHUNK) as i32);
        for y in 0..self.top_y.min(SIZE_Y) as i32 {
            for z in z0..z0 + CHUNK as i32 {
                for x in x0..x0 + CHUNK as i32 {
                    let block = self.get(x, y, z);
                    if block == Block::Air {
                        continue;
                    }
                    // The house plant is a little model, not a cube.
                    if block == Block::HousePlant {
                        push_plant(&mut vertices, &mut indices, x, y, z);
                        continue;
                    }
                    for face in &FACES {
                        let [nx, ny, nz] = face.normal;
                        // Cells holding the plant don't hide the faces next to them. Neither
                        // does liquid water (the lake bed has to be there to see), but water
                        // does hide its own sides, and ice hides everything under it.
                        let beside = self.get(x + nx, y + ny, z + nz);
                        let hidden = match beside {
                            Block::Air | Block::HousePlant => false,
                            Block::Water => {
                                block == Block::Water
                                    || self.frozen[(z + nz) as usize * SIZE_X + (x + nx) as usize]
                            }
                            _ => true,
                        };
                        if hidden {
                            continue;
                        }
                        if face.kind == 2 && y == 0 {
                            continue; // never visible
                        }
                        let biome = self.biomes[z as usize * SIZE_X + x as usize];
                        let layer = match (block, face.kind) {
                            (Block::Grass, 0) if biome == Biome::Meadow => 41.0,
                            (Block::Grass, 0) if biome == Biome::Swamp => 42.0,
                            (Block::Dirt, _) if biome == Biome::Swamp => 43.0,
                            (Block::Grass, 0) => 0.0,
                            (Block::Grass, 1) => 1.0,
                            (Block::Snow, 0) => 3.0,
                            (Block::Snow, 1) => 4.0,
                            (Block::Stone, _) => 5.0,
                            (Block::Log, 1) => 6.0,
                            (Block::Log, _) => 7.0,
                            (Block::PineLog, 1) => 20.0,
                            (Block::PineLog, _) => 21.0,
                            (Block::Leaves, _) => 8.0,
                            (Block::PineLeaves, _) => 9.0,
                            (Block::Burning, _) => 22.0,
                            (Block::Coal, _) => 23.0,
                            (Block::Water, _) if self.frozen[z as usize * SIZE_X + x as usize] => 34.0,
                            (Block::Water, _) => 24.0,
                            (Block::WoodFloor, _) => 25.0,
                            (Block::StoneFloor, _) => 26.0,
                            (Block::Forge, _) => 27.0,
                            (Block::Furnace, _) => 29.0,
                            (Block::Sand, _) => 35.0,
                            (Block::RedRock, _) => 36.0,
                            (Block::Clay, _) => 37.0,
                            (Block::Basalt, _) => 38.0,
                            (Block::Magma, _) => 39.0,
                            (Block::Cactus, _) => 40.0,
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
    fn plains_are_gentle_and_snow_country_has_tall_mountains_and_deep_valleys() {
        for seed in [1, 2, 3, 99] {
            let w = World::generate(seed);
            let column = |x: usize, z: usize| w.heights[z * SIZE_X + x] as i32;
            let is = |x: usize, z: usize, b: Biome| w.biomes[z * SIZE_X + x] == b;
            let (mut pairs, mut cliffs) = (0u32, 0u32);
            let (mut plains_max, mut snow_max, mut snow_min) = (0, 0, i32::MAX);
            for z in 0..SIZE_Z - 1 {
                for x in 0..SIZE_X - 1 {
                    if is(x, z, Biome::Plains) {
                        plains_max = plains_max.max(column(x, z));
                        for (nx, nz) in [(x + 1, z), (x, z + 1)] {
                            if is(nx, nz, Biome::Plains) {
                                pairs += 1;
                                cliffs += ((column(x, z) - column(nx, nz)).abs() > 1) as u32;
                            }
                        }
                    } else if is(x, z, Biome::Snow) {
                        snow_max = snow_max.max(column(x, z));
                        snow_min = snow_min.min(column(x, z));
                    }
                }
            }
            // The plains are walkable (a step can be hopped up), apart from the odd spot where
            // they meet another biome.
            assert!(cliffs * 100 <= pairs, "{cliffs} steps over 1 in {pairs} (seed {seed})");
            assert!(plains_max <= 30, "plains too tall: {plains_max}");
            assert!(snow_max >= 30, "mountains too short: {snow_max}");
            assert!(snow_min <= 8, "valleys too shallow: {snow_min}");
        }
    }

    #[test]
    fn every_biome_exists_and_the_world_starts_on_plains() {
        for seed in [1, 2, 3, 99] {
            let w = World::generate(seed);
            for biome in Biome::ALL {
                let n = w.biomes.iter().filter(|b| **b == biome).count();
                assert!(n > SIZE_X * SIZE_Z / 100, "{biome:?} is only {n} columns (seed {seed})");
            }
            let s = w.spawn_point();
            assert_eq!(w.biome(s.x as i32, s.z as i32), Biome::Plains);
        }
    }

    #[test]
    fn biomes_are_big_regions_not_strips_or_specks() {
        // Sample the world every 16 blocks and flood out each biome's regions.
        const STEP: usize = 16;
        let (nx, nz) = (SIZE_X / STEP, SIZE_Z / STEP);
        for seed in [1, 2, 3, 99] {
            let w = World::generate(seed);
            let at = |i: usize| w.biomes[(i / nx * STEP) * SIZE_X + (i % nx) * STEP];
            let mut seen = vec![false; nx * nz];
            let mut small = 0;
            for start in 0..nx * nz {
                if seen[start] {
                    continue;
                }
                let biome = at(start);
                let mut stack = vec![start];
                seen[start] = true;
                let (mut size, mut on_edge) = (0, false);
                while let Some(i) = stack.pop() {
                    size += 1;
                    let (x, z) = (i % nx, i / nx);
                    on_edge |= x == 0 || z == 0 || x + 1 == nx || z + 1 == nz;
                    let mut next = Vec::new();
                    if x > 0 { next.push(i - 1); }
                    if x + 1 < nx { next.push(i + 1); }
                    if z > 0 { next.push(i - nx); }
                    if z + 1 < nz { next.push(i + nx); }
                    for n in next {
                        if !seen[n] && at(n) == biome {
                            seen[n] = true;
                            stack.push(n);
                        }
                    }
                }
                // Under about 120 blocks across. A scrap on the map's edge is just a region
                // cut off by it, and a single sample is a speckle of a border.
                if size < 50 && size > 3 && !on_edge {
                    small += size;
                }
            }
            assert!(small * 100 < nx * nz, "{small} of {} samples in small regions (seed {seed})", nx * nz);
        }
    }

    #[test]
    fn each_biome_has_its_own_ground_and_plants() {
        let w = World::generate(1);
        let mut seen: Vec<(Biome, Block)> = Vec::new();
        for z in 0..SIZE_Z {
            for x in 0..SIZE_X {
                let h = w.heights[z * SIZE_X + x] as i32;
                if h > WATER_LEVEL as i32 {
                    let pair = (w.biomes[z * SIZE_X + x], w.get(x as i32, h - 1, z as i32));
                    if !seen.contains(&pair) {
                        seen.push(pair);
                    }
                }
            }
        }
        for pair in [
            (Biome::Desert, Block::Sand),
            (Biome::Badlands, Block::RedRock),
            (Biome::Badlands, Block::Clay),
            (Biome::Volcanic, Block::Basalt),
            (Biome::Volcanic, Block::Magma),
            (Biome::Snow, Block::Snow),
            (Biome::Forest, Block::Grass),
        ] {
            assert!(seen.contains(&pair), "{pair:?}");
        }
        // Cacti only on sand, and dead swamp trunks without leaves.
        let count = |block| w.blocks.iter().filter(|b| **b == block).count();
        assert!(count(Block::Cactus) > 100, "cacti: {}", count(Block::Cactus));
        // No water on volcanoes, and hardly any in deserts (only where a river from outside
        // laps over the border).
        let (mut desert, mut desert_wet) = (0, 0);
        for z in 0..SIZE_Z {
            for x in 0..SIZE_X {
                let wet = w.get(x as i32, WATER_LEVEL as i32 - 1, z as i32) == Block::Water;
                match w.biomes[z * SIZE_X + x] {
                    Biome::Volcanic => assert!(!wet),
                    Biome::Desert => {
                        desert += 1;
                        desert_wet += wet as u32;
                    }
                    _ => {}
                }
            }
        }
        assert!(desert_wet * 20 < desert, "{desert_wet} wet columns of {desert} desert");
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
                    let above = w.get(xi, ground, zi);
                    let has_tree = above == Block::Log || above == Block::PineLog;
                    if above == Block::Log && below == Block::Grass {
                        oaks += 1;
                    }
                    if above == Block::PineLog && below == Block::Snow {
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
            assert!(density(high) < density(low) * 0.5, "low {low:?} high {high:?}");
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
    fn digging_under_water_fills_the_hole_and_leaves_the_surface_alone() {
        let top = WATER_LEVEL as i32 - 1;
        let mut w = World::generate(1);
        // Open water at least two deep, with water on every side.
        let (x, z) = (0..SIZE_Z)
            .flat_map(|z| (0..SIZE_X).map(move |x| (x as i32, z as i32)))
            .find(|&(x, z)| {
                let h = w.heights[z as usize * SIZE_X + x as usize] as i32;
                h <= top - 1
                    && !w.frozen[z as usize * SIZE_X + x as usize]
                    && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .all(|(dx, dz)| w.is_liquid(x + dx, h, z + dz))
            })
            .expect("a deep spot");
        let bed = w.heights[z as usize * SIZE_X + x as usize] as i32 - 1;
        assert!(w.remove_block(x, bed, z).is_some());
        assert_eq!(w.get(x, bed, z), Block::Water, "the hole fills with water");
        // And you can keep digging down: each new hole is water too.
        assert!(w.remove_block(x, bed - 1, z).is_some());
        assert_eq!(w.get(x, bed - 1, z), Block::Water);
        assert_eq!(w.get(x, top, z), Block::Water, "the surface is untouched");
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
    fn open_spot_is_open_and_on_the_ground() {
        for seed in [1, 2, 3, 99] {
            let w = World::generate(seed);
            let p = w.open_spot(SIZE_X as i32 / 2, SIZE_Z as i32 / 2 - 7);
            let (x, y, z) = (p.x.floor() as i32, p.y as i32, p.z.floor() as i32);
            assert_ne!(w.get(x, y - 1, z), Block::Air, "standing on something");
            for dy in 0..3 {
                assert_eq!(w.get(x, y + dy, z), Block::Air);
            }
        }
    }

    #[test]
    fn raycast_face_gives_the_cell_in_front_and_blocks_can_be_placed_there() {
        let mut w = World::generate(5);
        let s = w.spawn_point();
        let eye = s + Vec3::new(0.0, 1.62, 0.0);
        let hit = w.raycast_face(eye, Vec3::NEG_Y, 5.0).expect("ground");
        let ground = s.y.floor() as i32 - 1;
        assert_eq!(hit.block.1, ground);
        let place = hit.place.expect("there is air in front of the top face");
        assert_eq!(place, (hit.block.0, ground + 1, hit.block.2));

        assert!(w.place_block(place.0, place.1, place.2, Block::Stone));
        assert_eq!(w.get(place.0, place.1, place.2), Block::Stone);
        assert!(!w.place_block(place.0, place.1, place.2, Block::Dirt), "occupied");
        assert!(!w.place_block(-1, 5, 5, Block::Dirt), "outside the world");
        assert!(!w.place_block(5, SIZE_Y as i32, 5, Block::Dirt), "above the world");
        assert!(!w.place_block(5, 40, 5, Block::Air), "air is not a block");

        // The new block is what the ray hits now, with its own free cell in front of it.
        let again = w.raycast_face(eye, Vec3::NEG_Y, 5.0).unwrap();
        assert_eq!(again.block, place);
        assert_eq!(again.place, Some((place.0, place.1 + 1, place.2)));
    }

    #[test]
    fn replace_block_only_swaps_existing_blocks() {
        let mut w = World::generate(5);
        let s = w.spawn_point();
        let (x, y, z) = (s.x.floor() as i32, s.y.floor() as i32 - 1, s.z.floor() as i32);
        assert_ne!(w.get(x, y, z), Block::Air);
        assert!(w.replace_block(x, y, z, Block::Burning));
        assert_eq!(w.get(x, y, z), Block::Burning);
        assert!(!w.replace_block(x, y + 5, z, Block::Coal), "nothing there to replace");
        assert!(!w.replace_block(x, y, z, Block::Air));
    }

    #[test]
    fn trees_grow_on_open_grass_and_not_where_blocked() {
        let mut w = World::generate(3);
        // Find a grass block with a lot of air above it (grass is on the west side).
        let mut spot = None;
        'search: for x in 6..70i32 {
            for z in 6..SIZE_Z as i32 - 6 {
                let top = w.heights[z as usize * SIZE_X + x as usize] as i32 - 1;
                let open = (1..=12).all(|dy| w.get(x, top + dy, z) == Block::Air);
                if w.get(x, top, z) == Block::Grass && open {
                    spot = Some((x, top, z));
                    break 'search;
                }
            }
        }
        let (x, y, z) = spot.expect("an open grass block");
        assert!(w.tree_fits(x, y, z, false) && w.tree_fits(x, y, z, true));

        assert!(w.grow_tree(x, y, z, false));
        assert_eq!(w.get(x, y + 1, z), Block::Log, "oak trunk on top of the grass");
        assert!((0..4).all(|dy| w.get(x, y + 1 + dy, z) == Block::Log));
        // Now the column is taken, so nothing else fits there.
        assert!(!w.grow_tree(x, y, z, true));
        assert!(!w.tree_fits(x, y, z, false));

        // A pine somewhere else is made of pine wood.
        let mut other = World::generate(3);
        assert!(other.grow_tree(x, y, z, true));
        assert_eq!(other.get(x, y + 1, z), Block::PineLog);
        assert!((0..6).all(|dy| other.get(x, y + 1 + dy, z) == Block::PineLog));
    }

    #[test]
    fn block_within_measures_distance() {
        let mut w = World::generate(5);
        let s = w.spawn_point();
        let (x, y, z) = (s.x.floor() as i32, s.y.floor() as i32 + 3, s.z.floor() as i32);
        assert!(w.place_block(x, y, z, Block::Forge));
        let center = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
        assert!(w.block_within(center + Vec3::new(5.0, 0.0, 0.0), 6.0, Block::Forge));
        assert!(!w.block_within(center + Vec3::new(7.0, 0.0, 0.0), 6.0, Block::Forge));
        assert!(!w.block_within(center, 6.0, Block::HousePlant));
    }

    #[test]
    fn stairs_climb_one_block_per_step_and_need_room() {
        let mut w = World::generate(5);
        let s = w.spawn_point();
        let (x, y, z) = (s.x.floor() as i32, s.y.floor() as i32 + 4, s.z.floor() as i32);
        let cells: Vec<_> = World::stair_cells(x, y, z, (1, 0), 3).collect();
        assert_eq!(cells, vec![(x + 1, y + 1, z), (x + 2, y + 2, z), (x + 3, y + 3, z)]);
        assert!(w.stairs_fit(x, y, z, (1, 0), 3));
        // Something in the way of the second step.
        assert!(w.place_block(x + 2, y + 2, z, Block::Stone));
        assert!(!w.stairs_fit(x, y, z, (1, 0), 3));
        assert!(w.stairs_fit(x, y, z, (-1, 0), 3), "the other way is still open");
        // Off the edge of the world.
        assert!(!w.stairs_fit(SIZE_X as i32 - 2, y, z, (1, 0), 3));
    }

    #[test]
    fn house_plant_is_a_small_model_that_does_not_hide_its_neighbours() {
        let mut w = World::generate(5);
        let s = w.spawn_point();
        // A cell with open air on every side, a few blocks above the ground.
        let (x, y, z) = (s.x.floor() as i32, s.y.floor() as i32 + 3, s.z.floor() as i32);
        let before = w.build_mesh().0.len();
        assert!(w.place_block(x, y, z, Block::HousePlant));
        let (vertices, _) = w.build_mesh();
        assert_eq!(vertices.len(), before + PLANT_PARTS.len() * 6 * 4);

        // The cell was empty with air all around, so every vertex in it is the plant's. They
        // are all well inside the cell sideways (the model is smaller than a cube).
        let in_cell: Vec<&Vertex> = vertices
            .iter()
            .filter(|v| {
                (x as f32..=x as f32 + 1.0).contains(&v.pos[0])
                    && (y as f32..=y as f32 + 1.0).contains(&v.pos[1])
                    && (z as f32..=z as f32 + 1.0).contains(&v.pos[2])
            })
            .collect();
        assert_eq!(in_cell.len(), PLANT_PARTS.len() * 6 * 4);
        assert!(in_cell.iter().all(|v| {
            v.pos[0] > x as f32 + 0.05
                && v.pos[0] < x as f32 + 0.95
                && v.pos[2] > z as f32 + 0.05
                && v.pos[2] < z as f32 + 0.95
        }));

        // A block next to it still gets its face on the plant's side.
        assert!(w.place_block(x + 1, y, z, Block::Stone));
        let with_stone = w.build_mesh().0.len();
        assert_eq!(with_stone, before + PLANT_PARTS.len() * 6 * 4 + 6 * 4);
    }
}
