use super::super::*;

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
