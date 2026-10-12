use super::*;

impl World {
    pub(super) fn set_if_air(&mut self, x: i32, y: i32, z: i32, block: Block) {
        if x < 0 || y < 0 || z < 0 || x >= SIZE_X as i32 || y >= SIZE_Y as i32 || z >= SIZE_Z as i32
        {
            return;
        }
        let i = Self::index(x as usize, y as usize, z as usize);
        if self.blocks[i] == Block::Air {
            self.blocks[i] = block;
            self.top_y = self.top_y.max(y as usize + 1);
            self.mark_dirty(x, z);
            self.record(x, y, z, block);
        }
    }

    /// Oaks on grass (forests have the most, then plains, meadows hardly any), bare dead trunks in swamps, pines
    /// on snow (rarer, and rarer still as the ground gets high) and cacti on sand. Each cell
    /// of a coarse grid gets at most one plant, which keeps them from piling up.
    pub(super) fn plant_trees(&mut self, seed: u32) {
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
}
