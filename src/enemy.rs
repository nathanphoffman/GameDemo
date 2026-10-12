use std::time::{SystemTime, UNIX_EPOCH};

use glam::Vec3;

use crate::world::{Vertex, FACES};

pub const MAX_HEALTH: i32 = 12;
pub const HEIGHT: f32 = 2.0;
const HALF_WIDTH: f32 = 0.4;

// Texture layers (see textures.rs).
const BONE: f32 = 13.0;
const SKULL: f32 = 14.0;
const RIBS: f32 = 15.0;
const CRACKS: f32 = 11.0;

/// How far the crack overlay sits off the body.
const GROW: f32 = 0.004;

pub struct Enemy {
    /// Bottom center of the skeleton.
    pub pos: Vec3,
    pub health: i32,
}

/// One box of the body. `layers` is indexed like `FACES`: +Y, -Y, +X, -X, +Z (front), -Z.
struct Part {
    min: [f32; 3],
    max: [f32; 3],
    layers: [f32; 6],
}

const PARTS: usize = 6;
pub const VERTEX_COUNT: usize = PARTS * 6 * 4;

fn parts() -> [Part; PARTS] {
    [
        // Head, with the face on the front.
        Part {
            min: [-0.25, 1.5, -0.25],
            max: [0.25, 2.0, 0.25],
            layers: [BONE, BONE, BONE, BONE, SKULL, BONE],
        },
        // Torso, with ribs on the front.
        Part {
            min: [-0.25, 0.75, -0.125],
            max: [0.25, 1.5, 0.125],
            layers: [BONE, BONE, BONE, BONE, RIBS, BONE],
        },
        // Arms.
        Part {
            min: [0.25, 0.75, -0.0625],
            max: [0.375, 1.5, 0.0625],
            layers: [BONE; 6],
        },
        Part {
            min: [-0.375, 0.75, -0.0625],
            max: [-0.25, 1.5, 0.0625],
            layers: [BONE; 6],
        },
        // Legs.
        Part {
            min: [0.03, 0.0, -0.0625],
            max: [0.16, 0.75, 0.0625],
            layers: [BONE; 6],
        },
        Part {
            min: [-0.16, 0.0, -0.0625],
            max: [-0.03, 0.75, 0.0625],
            layers: [BONE; 6],
        },
    ]
}

impl Enemy {
    pub fn new(pos: Vec3) -> Self {
        Enemy {
            pos,
            health: MAX_HEALTH,
        }
    }

    /// Box used for aiming, the wireframe highlight, and hit tests.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        (
            self.pos - Vec3::new(HALF_WIDTH, 0.0, HALF_WIDTH),
            self.pos + Vec3::new(HALF_WIDTH, HEIGHT, HALF_WIDTH),
        )
    }

    /// Shows cracks once he is at half health or lower.
    pub fn cracked(&self) -> bool {
        self.health * 2 <= MAX_HEALTH
    }

    /// Body vertices, with the front turned toward `look_at`. With `cracks` set, returns the
    /// same shape slightly inflated and using the crack texture, to draw on top.
    pub fn mesh(&self, look_at: Vec3, cracks: bool) -> Vec<Vertex> {
        let front = Vec3::new(look_at.x - self.pos.x, 0.0, look_at.z - self.pos.z)
            .try_normalize()
            .unwrap_or(Vec3::Z);
        let right = Vec3::new(front.z, 0.0, -front.x);
        let grow = if cracks { GROW } else { 0.0 };

        let mut out = Vec::with_capacity(VERTEX_COUNT);
        for part in parts() {
            for (f, face) in FACES.iter().enumerate() {
                for i in 0..4 {
                    let c = face.corners[i];
                    let mut local = [0.0f32; 3];
                    for k in 0..3 {
                        let size = part.max[k] - part.min[k];
                        local[k] = part.min[k] - grow + c[k] * (size + 2.0 * grow);
                    }
                    let p = self.pos + right * local[0] + Vec3::Y * local[1] + front * local[2];
                    out.push(Vertex {
                        pos: p.to_array(),
                        uv: face.uv[i],
                        layer: if cracks { CRACKS } else { part.layers[f] },
                        shade: if cracks { 1.0 } else { face.shade },
                    });
                }
            }
        }
        out
    }
}

/// Index list for `Enemy::mesh` (the same for every skeleton).
pub fn indices() -> Vec<u32> {
    let mut out = Vec::with_capacity(VERTEX_COUNT / 4 * 6);
    for quad in 0..(VERTEX_COUNT / 4) as u32 {
        let b = quad * 4;
        out.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    out
}

/// Distance along the ray at which it enters the box, if it does.
pub fn ray_box(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let (o, d, lo, hi) = (origin.to_array(), dir.to_array(), min.to_array(), max.to_array());
    let (mut t0, mut t1) = (0.0f32, f32::INFINITY);
    for i in 0..3 {
        if d[i].abs() < 1e-8 {
            if o[i] < lo[i] || o[i] > hi[i] {
                return None;
            }
        } else {
            let (a, b) = ((lo[i] - o[i]) / d[i], (hi[i] - o[i]) / d[i]);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
            if t0 > t1 {
                return None;
            }
        }
    }
    Some(t0)
}

/// Tiny random number generator, plenty for dice.
pub struct Dice {
    state: u32,
}

impl Dice {
    pub fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos() ^ d.as_secs() as u32)
            .unwrap_or(12345);
        Dice::with_seed(seed)
    }

    pub fn with_seed(seed: u32) -> Self {
        Dice {
            state: seed.max(1),
        }
    }

    fn next(&mut self) -> u32 {
        // xorshift32
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    pub fn d6(&mut self) -> u32 {
        // Use the high bits; they are better mixed than the low ones.
        (self.next() >> 8) % 6 + 1
    }

    pub fn roll_2d6(&mut self) -> (u32, u32) {
        (self.d6(), self.d6())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dice_are_fair_enough() {
        let mut dice = Dice::with_seed(42);
        let mut counts = [0u32; 6];
        for _ in 0..6000 {
            let v = dice.d6();
            assert!((1..=6).contains(&v));
            counts[v as usize - 1] += 1;
        }
        for c in counts {
            assert!((800..1200).contains(&c), "{counts:?}");
        }
    }

    #[test]
    fn ray_hits_and_misses() {
        let (min, max) = (Vec3::new(-1.0, 0.0, -1.0), Vec3::new(1.0, 2.0, 1.0));
        let t = ray_box(Vec3::new(0.0, 1.0, 5.0), Vec3::NEG_Z, min, max).unwrap();
        assert!((t - 4.0).abs() < 1e-4);
        assert!(ray_box(Vec3::new(3.0, 1.0, 5.0), Vec3::NEG_Z, min, max).is_none());
        assert!(ray_box(Vec3::new(0.0, 1.0, 5.0), Vec3::Z, min, max).is_none());
    }

    #[test]
    fn mesh_matches_index_list_and_cracks_at_half() {
        let mut e = Enemy::new(Vec3::new(10.0, 5.0, 10.0));
        let v = e.mesh(Vec3::new(0.0, 5.0, 0.0), false);
        assert_eq!(v.len(), VERTEX_COUNT);
        assert!(indices().iter().all(|&i| (i as usize) < v.len()));
        assert!(!e.cracked());
        e.health = 7;
        assert!(!e.cracked());
        e.health = 6;
        assert!(e.cracked());
        assert_eq!(e.mesh(Vec3::ZERO, true).len(), VERTEX_COUNT);
    }
}
