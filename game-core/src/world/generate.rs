use super::terrain::{near_spawn, slope_limit, survey};
use super::*;

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
            edits: HashMap::new(),
            fresh: Vec::new(),
        };
        world.plant_trees(seed);
        world.dirty.fill(false);
        // The trees are part of the generated world, not edits to it.
        world.edits.clear();
        world.fresh.clear();
        world
    }

    /// The block at height `y` in a column of this biome whose ground is `h` high.
    pub(super) fn ground_block(biome: Biome, y: usize, h: usize) -> Block {
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
    pub(super) fn frozen_columns(heights: &[u8], biomes: &[Biome], cold: &[bool]) -> Vec<bool> {
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
}
