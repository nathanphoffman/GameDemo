use crate::world::hash;

pub const TILE: u32 = 16;
pub const LAYERS: u32 = 10; // 0 grass top, 1 grass side, 2 dirt, 3 snow top, 4 snow side, 5 stone,
                            // 6 log side, 7 log top, 8 leaves, 9 pine needles
pub const MIP_LEVELS: u32 = 5; // 16, 8, 4, 2, 1

const GRASS: [f32; 3] = [88.0, 150.0, 50.0];
const DIRT: [f32; 3] = [134.0, 96.0, 67.0];
const STONE: [f32; 3] = [122.0, 124.0, 130.0];
const BARK: [f32; 3] = [104.0, 82.0, 52.0];
const WOOD: [f32; 3] = [170.0, 138.0, 86.0];
const LEAVES: [f32; 3] = [58.0, 128.0, 38.0];
const NEEDLES: [f32; 3] = [28.0, 82.0, 52.0];
const SNOW: [f32; 3] = [238.0, 243.0, 250.0];

fn shaded(base: [f32; 3], x: u32, y: u32, seed: u32) -> [u8; 4] {
    let spread = if base[0] > 200.0 { 0.12 } else { 0.4 };
    let f = 1.0 - spread / 2.0 + spread * hash(x, y, seed);
    [
        (base[0] * f).min(255.0) as u8,
        (base[1] * f).min(255.0) as u8,
        (base[2] * f).min(255.0) as u8,
        255,
    ]
}

fn tile(layer: u32) -> Vec<u8> {
    let mut data = Vec::with_capacity((TILE * TILE * 4) as usize);
    for y in 0..TILE {
        for x in 0..TILE {
            let px = match layer {
                0 => shaded(GRASS, x, y, 10),
                1 => {
                    // Green fringe along the top, ragged per column.
                    let fringe = 3 + (hash(x, 0, 20) * 3.0) as u32;
                    if y < fringe {
                        shaded(GRASS, x, y, 11)
                    } else {
                        shaded(DIRT, x, y, 12)
                    }
                }
                3 => shaded(SNOW, x, y, 14),
                4 => {
                    let fringe = 3 + (hash(x, 0, 21) * 3.0) as u32;
                    if y < fringe {
                        shaded(SNOW, x, y, 15)
                    } else {
                        shaded(STONE, x, y, 16)
                    }
                }
                5 => shaded(STONE, x, y, 16),
                // Bark: vertical streaks, so shade by column.
                6 => {
                    let f = 0.75 + 0.5 * hash(x, 0, 30);
                    let g = 0.9 + 0.2 * hash(x, y, 31);
                    let k = f * g;
                    [
                        (BARK[0] * k).min(255.0) as u8,
                        (BARK[1] * k).min(255.0) as u8,
                        (BARK[2] * k).min(255.0) as u8,
                        255,
                    ]
                }
                // Cut end: light and dark rings.
                7 => {
                    let ring = (x as f32 - 7.5).abs().max((y as f32 - 7.5).abs()) as u32;
                    let base = if ring % 2 == 0 { WOOD } else { [BARK[0] * 1.3, BARK[1] * 1.3, BARK[2] * 1.3] };
                    shaded(base, x, y, 32)
                }
                8 => shaded(LEAVES, x, y, 33),
                9 => shaded(NEEDLES, x, y, 34),
                _ => shaded(DIRT, x, y, 13),
            };
            data.extend_from_slice(&px);
        }
    }
    data
}

/// Returns `[layer][mip]` RGBA8 pixel data, largest mip first.
pub fn build() -> Vec<Vec<Vec<u8>>> {
    (0..LAYERS)
        .map(|layer| {
            let mut mips = vec![tile(layer)];
            let mut s = TILE;
            while s > 1 {
                let prev = mips.last().unwrap();
                let ns = s / 2;
                let mut next = vec![0u8; (ns * ns * 4) as usize];
                for y in 0..ns {
                    for x in 0..ns {
                        for c in 0..4 {
                            let at = |dx: u32, dy: u32| {
                                prev[(((y * 2 + dy) * s + (x * 2 + dx)) * 4) as usize + c] as u32
                            };
                            let avg = (at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1)) / 4;
                            next[((y * ns + x) * 4) as usize + c] = avg as u8;
                        }
                    }
                }
                mips.push(next);
                s = ns;
            }
            mips
        })
        .collect()
}
