//! What the land is like at each spot: which biome it is and how high the ground is.
//!
//! The world is cut into big regions, like a Voronoi map: a handful of points are scattered
//! over it and every spot belongs to the nearest point's biome. Points are never close
//! together, so every biome is a large area. Ground shapes blend into each other near a
//! border, which is also ragged and speckled for a short way.

use crate::world::{fbm_with, hash, hill_height, mountain_height, smoothstep, value_noise};
use crate::world::{SIZE_X, SIZE_Z};

/// The world is this many regions across (and it wraps, so edge regions have neighbours).
const GRID: i32 = 3;
/// How far ground shapes take to blend across a border, in blocks.
const BLEND: f32 = 36.0;
/// The speckled strip along a border is this wide on each side.
const SPECKLE: f32 = 10.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Biome {
    /// Rolling grassy hills with a few oaks. Where you start.
    Plains,
    /// Flat and open, with flowers and hardly any trees.
    Meadow,
    /// Hills packed with oaks.
    Forest,
    /// Low and flat, full of ponds, with dead trees.
    Swamp,
    /// Dunes of sand and cacti. No water.
    Desert,
    /// Terraced mesas of layered red and orange rock.
    Badlands,
    /// Black rock, tall cones and pools of magma.
    Volcanic,
    /// Mountains of snow and pines, where the water freezes.
    Snow,
}

#[cfg(test)]
impl Biome {
    pub const ALL: [Biome; 8] = [
        Biome::Plains,
        Biome::Meadow,
        Biome::Forest,
        Biome::Swamp,
        Biome::Desert,
        Biome::Badlands,
        Biome::Volcanic,
        Biome::Snow,
    ];
}

/// Everything the generator needs to know about one column of the world.
#[derive(Clone, Copy)]
pub struct Column {
    pub biome: Biome,
    /// Ground height in blocks, before water is carved out.
    pub height: f32,
    /// How much of the usual rivers and lakes are allowed here: 0 in deserts and on volcanoes.
    pub water: f32,
    /// How swampy this column is (0 to 1): extra ponds.
    pub swamp: f32,
}

/// Layered shelves with steep sides, like mesas.
fn mesa_height(x: f32, z: f32, seed: u32) -> f32 {
    let n = fbm_with(x / 70.0, z / 70.0, seed ^ 0x3E5A, 3, 0.45, false);
    let t = ((n - 0.35) / 0.3).clamp(0.0, 1.0) * 4.0;
    let step = t.floor();
    // Flat for most of each level, then a quick climb to the next.
    let riser = smoothstep((t - step - 0.85) / 0.15);
    7.0 + 4.0 * (step + riser)
}

/// A cone with pools of magma dug into the low ground.
fn volcano_height(x: f32, z: f32, seed: u32) -> f32 {
    let cone = smoothstep((fbm_with(x / 70.0, z / 70.0, seed ^ 0xCA7E, 2, 0.45, false) - 0.40) / 0.35);
    let pools = smoothstep((fbm_with(x / 30.0, z / 30.0, seed ^ 0xB00B, 2, 0.5, false) - 0.58) / 0.1);
    11.0 + 26.0 * cone.powf(1.3) - 8.0 * pools
}

/// Where the regions are and what each one is.
pub struct Layout {
    /// Offset of each region's point inside its cell, as a fraction of the cell (-0.2 to 0.2).
    jitter: [(f32, f32); (GRID * GRID) as usize],
    biomes: [Biome; (GRID * GRID) as usize],
    /// The start is its own region of plains, so there is always room around it.
    spawn: (f32, f32),
}

impl Layout {
    pub fn new(seed: u32) -> Layout {
        let cells = (GRID * GRID) as usize;
        // Every biome appears at least once: shuffle them all, then fill what is left over
        // with a seeded pick.
        let mut order: Vec<Biome> = (0..cells)
            .map(|i| {
                if i < 8 {
                    ALL_BIOMES[i]
                } else {
                    ALL_BIOMES[(hash(i as u32, 1, seed ^ 0xB10) * 8.0) as usize % 8]
                }
            })
            .collect();
        for i in (1..cells).rev() {
            let j = (hash(i as u32, 2, seed ^ 0xB10) * (i + 1) as f32) as usize % (i + 1);
            order.swap(i, j);
        }
        let mut jitter = [(0.0, 0.0); (GRID * GRID) as usize];
        let mut biomes = [Biome::Plains; (GRID * GRID) as usize];
        for i in 0..cells {
            jitter[i] = (
                (hash(i as u32, 3, seed ^ 0xB10) - 0.5) * 0.4,
                (hash(i as u32, 4, seed ^ 0xB10) - 0.5) * 0.4,
            );
            biomes[i] = order[i];
        }
        Layout {
            jitter,
            biomes,
            spawn: (SIZE_X as f32 / 2.0, SIZE_Z as f32 / 2.0),
        }
    }

    /// The two nearest regions to a spot, as (biome, distance), nearest first.
    fn nearest_two(&self, x: f32, z: f32) -> [(Biome, f32); 2] {
        let cell = (SIZE_X as f32 / GRID as f32, SIZE_Z as f32 / GRID as f32);
        let (cx, cz) = ((x / cell.0).floor() as i32, (z / cell.1).floor() as i32);
        let mut best = [(Biome::Plains, f32::MAX); 2];
        let mut consider = |biome: Biome, px: f32, pz: f32| {
            let d = ((x - px).powi(2) + (z - pz).powi(2)).sqrt();
            if d < best[0].1 {
                best[1] = best[0];
                best[0] = (biome, d);
            } else if d < best[1].1 {
                best[1] = (biome, d);
            }
        };
        consider(Biome::Plains, self.spawn.0, self.spawn.1);
        for j in cz - 1..=cz + 1 {
            for i in cx - 1..=cx + 1 {
                let id = (i.rem_euclid(GRID) + j.rem_euclid(GRID) * GRID) as usize;
                let (jx, jz) = self.jitter[id];
                consider(
                    self.biomes[id],
                    (i as f32 + 0.5 + jx) * cell.0,
                    (j as f32 + 0.5 + jz) * cell.1,
                );
            }
        }
        best
    }
}

const ALL_BIOMES: [Biome; 8] = [
    Biome::Plains,
    Biome::Meadow,
    Biome::Forest,
    Biome::Swamp,
    Biome::Desert,
    Biome::Badlands,
    Biome::Volcanic,
    Biome::Snow,
];

/// The ground height a biome wants at a spot.
fn shape(biome: Biome, fx: f32, fz: f32, seed: u32) -> f32 {
    match biome {
        Biome::Plains | Biome::Forest => hill_height(fx, fz, seed),
        Biome::Meadow => 10.0 + 2.0 * fbm_with(fx / 50.0, fz / 50.0, seed ^ 0x3EAD, 2, 0.4, false),
        Biome::Swamp => 6.0 + 2.5 * fbm_with(fx / 20.0, fz / 20.0, seed ^ 0x5A4D, 2, 0.5, false),
        Biome::Desert => 9.0 + 7.0 * fbm_with(fx / 28.0, fz / 28.0, seed ^ 0xD00E, 2, 0.4, false),
        Biome::Badlands => mesa_height(fx, fz, seed),
        Biome::Volcanic => volcano_height(fx, fz, seed),
        Biome::Snow => mountain_height(fx, fz, seed),
    }
}

/// How much of the usual rivers and lakes a biome allows: none in deserts and on volcanoes,
/// and mountains keep their peaks.
fn water_allowed(biome: Biome) -> f32 {
    match biome {
        Biome::Desert | Biome::Volcanic => 0.0,
        Biome::Badlands | Biome::Snow => 0.1,
        _ => 1.0,
    }
}

pub fn column(layout: &Layout, x: usize, z: usize, seed: u32) -> Column {
    let (fx, fz) = (x as f32, z as f32);
    // Wobble where the borders are so they wander instead of running straight.
    let warp_x = (fbm_with(fx / 60.0, fz / 60.0, seed ^ 0xBA4D, 2, 0.5, false) - 0.5) * 80.0;
    let warp_z = (fbm_with(fx / 60.0 + 31.0, fz / 60.0 - 7.0, seed ^ 0xBA4E, 2, 0.5, false) - 0.5) * 80.0;
    let [(near, d1), (far, d2)] = layout.nearest_two(fx + warp_x, fz + warp_z);
    let gap = d2 - d1;

    // Close to a border, grains of the other biome are mixed in.
    let grain = 0.6 * hash(x as u32, z as u32, seed ^ 0x5EED)
        + 0.4 * value_noise(fx / 5.0, fz / 5.0, seed ^ 0x5EED);
    let biome = if gap < SPECKLE && grain < 0.5 - 0.5 * gap / SPECKLE {
        far
    } else {
        near
    };

    // The ground shapes of the two regions blend across the border.
    let w = 0.5 + 0.5 * smoothstep(gap / BLEND);
    let mix = |a: f32, b: f32| b + (a - b) * w;
    let height = if near == far {
        shape(near, fx, fz, seed)
    } else {
        mix(shape(near, fx, fz, seed), shape(far, fx, fz, seed))
    };
    let water = mix(water_allowed(near), water_allowed(far));
    let swamp = mix((near == Biome::Swamp) as u8 as f32, (far == Biome::Swamp) as u8 as f32);
    Column {
        biome,
        height,
        water,
        swamp,
    }
}
