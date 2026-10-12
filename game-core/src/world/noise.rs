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
