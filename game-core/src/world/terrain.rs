use super::*;
use crate::biome;

/// How wet a column is: 0 = dry land, up to 1 in the deepest part of a river, lake or pond.
pub(super) fn wetness(x: f32, z: f32, seed: u32) -> f32 {
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
pub(super) fn near_spawn(x: usize, z: usize) -> f32 {
    let (dx, dz) = (x as f32 - (SIZE_X / 2) as f32, z as f32 - (SIZE_Z / 2) as f32);
    1.0 - smoothstep(((dx * dx + dz * dz).sqrt() - 20.0) / 20.0)
}

/// For every column: its biome, its ground height, and how deep a water bed is dug there
/// (`u8::MAX` for none). Worked out on every core, a band of rows each.
pub(super) fn survey(seed: u32) -> Vec<(Biome, u8, u8)> {
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

pub(super) fn survey_column(layout: &biome::Layout, x: usize, z: usize, seed: u32) -> (Biome, u8, u8) {
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
pub(super) fn slope_limit(limit: &mut [u8]) {
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
