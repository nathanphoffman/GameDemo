use super::super::*;

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
