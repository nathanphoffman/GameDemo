use crate::world::hash;

pub const TILE: u32 = 16;
pub const LAYERS: u32 = 29; // 0 grass top, 1 grass side, 2 dirt, 3 snow top, 4 snow side, 5 stone,
                            // 6 log side, 7 log top, 8 leaves, 9 pine needles,
                            // 10 light cracks, 11 heavy cracks (transparent overlays),
                            // 12 solid near-black (target outline),
                            // 13 bone, 14 skull face, 15 ribs, 16 red, 17 green (health bar),
                            // 18 deer fur, 19 deer face, 20 pine bark, 21 pine log top,
                            // 22 fire, 23 coal, 24 water,
                            // 25 wood floor, 26 stone floor, 27 forge, 28 house plant
pub const MIP_LEVELS: u32 = 5; // 16, 8, 4, 2, 1

const GRASS: [f32; 3] = [88.0, 150.0, 50.0];
const DIRT: [f32; 3] = [134.0, 96.0, 67.0];
const STONE: [f32; 3] = [122.0, 124.0, 130.0];
const PINE_BARK: [f32; 3] = [78.0, 56.0, 42.0];
const PINE_WOOD: [f32; 3] = [150.0, 110.0, 76.0];
const BARK: [f32; 3] = [104.0, 82.0, 52.0];
const WOOD: [f32; 3] = [170.0, 138.0, 86.0];
const LEAVES: [f32; 3] = [58.0, 128.0, 38.0];
const NEEDLES: [f32; 3] = [28.0, 82.0, 52.0];
const FUR: [f32; 3] = [160.0, 108.0, 64.0];
const BONE: [f32; 3] = [226.0, 222.0, 205.0];
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

/// Transparent tile with dark crack lines grown outward from near the center.
fn cracks(branches: u32, length: u32, seed: u32) -> Vec<u8> {
    let size = TILE as i32;
    let mut on = vec![false; (TILE * TILE) as usize];
    let mut starts: Vec<(f32, f32)> = vec![(7.5, 7.5)];
    for b in 0..branches {
        // Later branches sprout from points already cracked, so it spreads like real cracks.
        let pick = (hash(b, 1, seed) * starts.len() as f32) as usize;
        let (mut x, mut y) = starts[pick.min(starts.len() - 1)];
        let mut angle = hash(b, 2, seed) * std::f32::consts::TAU;
        for step in 0..length {
            angle += (hash(b, 10 + step, seed) - 0.5) * 1.2;
            x += angle.cos();
            y += angle.sin();
            let (xi, yi) = (x.round() as i32, y.round() as i32);
            if xi < 0 || yi < 0 || xi >= size || yi >= size {
                break;
            }
            on[(yi * size + xi) as usize] = true;
            if step % 3 == 1 {
                starts.push((x, y));
            }
        }
    }
    let mut data = Vec::with_capacity((TILE * TILE * 4) as usize);
    for &lit in &on {
        data.extend_from_slice(if lit { &[18, 16, 16, 255] } else { &[0, 0, 0, 0] });
    }
    data
}

fn tile(layer: u32) -> Vec<u8> {
    if layer == 10 {
        return cracks(4, 7, 70);
    }
    if layer == 11 {
        return cracks(9, 9, 41);
    }
    if layer == 16 {
        return [205, 45, 45, 255].repeat((TILE * TILE) as usize);
    }
    if layer == 17 {
        return [70, 205, 70, 255].repeat((TILE * TILE) as usize);
    }
    if layer == 12 {
        return [12, 12, 12, 255].repeat((TILE * TILE) as usize);
    }
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
                13 => shaded(BONE, x, y, 50),
                18 => shaded(FUR, x, y, 53),
                19 => {
                    let eye = ((3..6).contains(&x) || (10..13).contains(&x)) && (5..8).contains(&y);
                    let nose = (6..10).contains(&x) && (11..14).contains(&y);
                    if eye || nose {
                        [24, 18, 16, 255]
                    } else {
                        shaded(FUR, x, y, 54)
                    }
                }
                14 => {
                    let eye = (3..6).contains(&x) || (10..13).contains(&x);
                    let socket = eye && (5..8).contains(&y);
                    let nose = (7..9).contains(&x) && (8..10).contains(&y);
                    let teeth = (11..13).contains(&y) && (4..12).contains(&x) && x % 2 == 0;
                    if socket || nose || teeth {
                        [30, 28, 28, 255]
                    } else {
                        shaded(BONE, x, y, 51)
                    }
                }
                15 => {
                    let rib = y % 3 == 1 && (2..14).contains(&x) && x != 7 && x != 8;
                    if rib {
                        [96, 92, 82, 255]
                    } else {
                        shaded(BONE, x, y, 52)
                    }
                }
                // Pine bark: darker and redder, with the same vertical streaks.
                20 => {
                    let f = 0.75 + 0.5 * hash(x, 0, 35);
                    let g = 0.9 + 0.2 * hash(x, y, 36);
                    let k = f * g;
                    [
                        (PINE_BARK[0] * k).min(255.0) as u8,
                        (PINE_BARK[1] * k).min(255.0) as u8,
                        (PINE_BARK[2] * k).min(255.0) as u8,
                        255,
                    ]
                }
                21 => {
                    let ring = (x as f32 - 7.5).abs().max((y as f32 - 7.5).abs()) as u32;
                    let base = if ring % 2 == 0 {
                        PINE_WOOD
                    } else {
                        [PINE_BARK[0] * 1.3, PINE_BARK[1] * 1.3, PINE_BARK[2] * 1.3]
                    };
                    shaded(base, x, y, 37)
                }
                // Flames: reds and oranges with bright yellow patches.
                22 => {
                    let t = hash(x, y, 60) * 0.6 + hash(x / 2, y / 2, 61) * 0.4;
                    if t > 0.7 {
                        [255, 225, 90, 255]
                    } else if t > 0.4 {
                        [240, 130, 30, 255]
                    } else {
                        [190, 50, 20, 255]
                    }
                }
                // Water: blue with lighter ripples.
                24 => {
                    let ripple = hash(x, y, 64) * 0.5 + hash(x / 2, y, 65) * 0.5;
                    let k = 0.8 + 0.4 * ripple;
                    [(40.0 * k) as u8, (110.0 * k) as u8, (215.0 * k).min(255.0) as u8, 255]
                }
                // Wooden floor: horizontal planks with dark seams.
                25 => {
                    let plank = y / 4;
                    let seam = y % 4 == 0 || (x + plank * 5) % 16 == 0;
                    let tone = 0.85 + 0.3 * hash(plank, 0, 70);
                    if seam {
                        [88, 58, 30, 255]
                    } else {
                        let g = 0.9 + 0.2 * hash(x, y, 71);
                        let k = tone * g;
                        [(176.0 * k).min(255.0) as u8, (128.0 * k) as u8, (76.0 * k) as u8, 255]
                    }
                }
                // Stone floor: pale grey tiles with grout lines.
                26 => {
                    let grout = x % 8 == 0 || y % 8 == 0;
                    if grout {
                        [88, 90, 96, 255]
                    } else {
                        shaded([164.0, 166.0, 172.0], x, y, 72)
                    }
                }
                // Forge: dark stone with a glowing opening.
                27 => {
                    let opening = (4..12).contains(&x) && (6..13).contains(&y);
                    let glow = (6..10).contains(&x) && (8..12).contains(&y);
                    if glow {
                        [255, 190, 60, 255]
                    } else if opening {
                        [200, 70, 20, 255]
                    } else {
                        shaded([86.0, 86.0, 92.0], x, y, 73)
                    }
                }
                // House plant: green leaves in a brown pot, on a light wall.
                28 => {
                    let pot = y >= 10 && (4..12).contains(&x);
                    let stem = (7..9).contains(&x) && (6..10).contains(&y);
                    let leaves = {
                        let (dx, dy) = (x as f32 - 7.5, y as f32 - 5.0);
                        dx * dx + dy * dy * 1.4 < 22.0 && y < 10
                    };
                    if pot {
                        shaded([150.0, 82.0, 52.0], x, y, 74)
                    } else if stem {
                        [60, 110, 40, 255]
                    } else if leaves {
                        shaded([58.0, 150.0, 60.0], x, y, 75)
                    } else {
                        shaded([226.0, 218.0, 196.0], x, y, 76)
                    }
                }
                // Coal: nearly black with a few lighter flecks.
                23 => {
                    let fleck = hash(x, y, 62) > 0.88;
                    let v = if fleck { 70.0 } else { 18.0 + 14.0 * hash(x, y, 63) };
                    [v as u8, v as u8, (v * 1.05) as u8, 255]
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

#[cfg(test)]
mod tests {
    #[test]
    fn cracks_grow_between_stages() {
        let layers = super::build();
        let count = |l: usize| layers[l][0].chunks(4).filter(|p| p[3] == 255).count();
        for l in [10, 11] {
            for row in layers[l][0].chunks((super::TILE * 4) as usize) {
                let line: String = row.chunks(4).map(|p| if p[3] == 255 { '#' } else { '.' }).collect();
                println!("{line}");
            }
            println!();
        }
        assert!(count(10) >= 8, "light cracks visible: {}", count(10));
        assert!(count(11) > count(10) * 3 / 2, "heavy {} vs light {}", count(11), count(10));
    }
}
