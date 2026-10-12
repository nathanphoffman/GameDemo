use std::time::{SystemTime, UNIX_EPOCH};

use glam::Vec3;

use crate::player::Player;
use crate::world::{Vertex, World, FACES};

// Texture layers (see textures.rs).
const BONE: f32 = 13.0;
const SKULL: f32 = 14.0;
const RIBS: f32 = 15.0;
const FUR: f32 = 18.0;
const DEER_FACE: f32 = 19.0;
const BAR_BACK: f32 = 12.0; // near-black border
const BAR_LOST: f32 = 16.0; // red
const BAR_HEALTH: f32 = 17.0; // green

/// Health bar size and how high it floats above the head, in blocks.
const BAR_WIDTH: f32 = 0.9;
const BAR_HEIGHT: f32 = 0.12;
const BAR_BORDER: f32 = 0.025;
const BAR_LIFT: f32 = 0.35;
/// Each bar layer sits this much closer to the viewer so they don't fight in the depth buffer.
const BAR_LAYER_GAP: f32 = 0.01;

/// Chasers stop this far from the player (horizontally).
const STOP_DISTANCE: f32 = 1.8;

/// How many enemies can be drawn at once, and the most vertices one of them can need.
pub const MAX_ENEMIES: usize = 8;
const MAX_PARTS: usize = 10;
pub const MAX_VERTICES_PER_ENEMY: usize = MAX_PARTS * 6 * 4 + 3 * 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Hostile from the start.
    Skeleton,
    /// Peaceful until attacked, then hostile.
    Deer,
}

impl Kind {
    pub fn max_health(self) -> i32 {
        match self {
            Kind::Skeleton => 12,
            Kind::Deer => 8,
        }
    }

    /// Blocks per second when chasing. Both are slower than the player's walk.
    fn chase_speed(self) -> f32 {
        match self {
            Kind::Skeleton => 3.0,
            Kind::Deer => 4.0,
        }
    }

    /// Half the width of the box used for aiming and the wireframe highlight.
    fn half_width(self) -> f32 {
        match self {
            Kind::Skeleton => 0.4,
            Kind::Deer => 0.55,
        }
    }

    fn height(self) -> f32 {
        2.0
    }

    pub fn starts_hostile(self) -> bool {
        self == Kind::Skeleton
    }
}

pub struct Enemy {
    pub id: u32,
    pub kind: Kind,
    /// Walks, falls and collides with the world like the player does.
    body: Player,
    pub health: i32,
    /// Hostile enemies chase the player and attack when close. Passive ones just stand.
    pub hostile: bool,
    /// Set when walking but not getting anywhere, so he hops over one-block steps.
    jump_next: bool,
}

/// One box of the body. `layers` is indexed like `FACES`: +Y, -Y, +X, -X, +Z (front), -Z.
struct Part {
    min: [f32; 3],
    max: [f32; 3],
    layers: [f32; 6],
}

fn part(min: [f32; 3], max: [f32; 3], layers: [f32; 6]) -> Part {
    Part { min, max, layers }
}

fn parts(kind: Kind) -> Vec<Part> {
    match kind {
        Kind::Skeleton => vec![
            // Head, with the face on the front.
            part(
                [-0.25, 1.5, -0.25],
                [0.25, 2.0, 0.25],
                [BONE, BONE, BONE, BONE, SKULL, BONE],
            ),
            // Torso, with ribs on the front.
            part(
                [-0.25, 0.75, -0.125],
                [0.25, 1.5, 0.125],
                [BONE, BONE, BONE, BONE, RIBS, BONE],
            ),
            // Arms.
            part([0.25, 0.75, -0.0625], [0.375, 1.5, 0.0625], [BONE; 6]),
            part([-0.375, 0.75, -0.0625], [-0.25, 1.5, 0.0625], [BONE; 6]),
            // Legs.
            part([0.03, 0.0, -0.0625], [0.16, 0.75, 0.0625], [BONE; 6]),
            part([-0.16, 0.0, -0.0625], [-0.03, 0.75, 0.0625], [BONE; 6]),
        ],
        Kind::Deer => vec![
            // Body, longer front to back; the head end is +Z.
            part([-0.25, 0.65, -0.5], [0.25, 1.15, 0.5], [FUR; 6]),
            // Legs.
            part([0.1, 0.0, 0.3], [0.22, 0.65, 0.42], [FUR; 6]),
            part([-0.22, 0.0, 0.3], [-0.1, 0.65, 0.42], [FUR; 6]),
            part([0.1, 0.0, -0.42], [0.22, 0.65, -0.3], [FUR; 6]),
            part([-0.22, 0.0, -0.42], [-0.1, 0.65, -0.3], [FUR; 6]),
            // Neck and head, with the face on the front.
            part([-0.1, 1.0, 0.3], [0.1, 1.55, 0.5], [FUR; 6]),
            part(
                [-0.13, 1.4, 0.38],
                [0.13, 1.68, 0.74],
                [FUR, FUR, FUR, FUR, DEER_FACE, FUR],
            ),
            // Antlers.
            part([0.07, 1.68, 0.42], [0.12, 1.98, 0.5], [BONE; 6]),
            part([-0.12, 1.68, 0.42], [-0.07, 1.98, 0.5], [BONE; 6]),
        ],
    }
}

/// Everything the crosshair needs to know about an enemy it is on.
#[derive(Clone, Copy)]
pub struct Sight {
    pub id: u32,
    pub hostile: bool,
    pub min: Vec3,
    pub max: Vec3,
    /// Horizontal distance from the player to the enemy.
    pub distance: f32,
}

impl Enemy {
    pub fn new(id: u32, kind: Kind, pos: Vec3) -> Self {
        Enemy {
            id,
            kind,
            body: Player::new(pos),
            health: kind.max_health(),
            hostile: kind.starts_hostile(),
            jump_next: false,
        }
    }

    /// Bottom center.
    pub fn pos(&self) -> Vec3 {
        self.body.pos
    }

    /// Walks toward `target` if `chase`, stopping at arm's length.
    pub fn step(&mut self, dt: f32, target: Vec3, chase: bool, world: &World) {
        let speed = self.kind.chase_speed();
        let pos = self.body.pos;
        let flat = Vec3::new(target.x - pos.x, 0.0, target.z - pos.z);
        let distance = flat.length();
        let want = if chase && distance > STOP_DISTANCE {
            flat / distance * speed
        } else {
            Vec3::ZERO
        };
        let jump = self.jump_next && want != Vec3::ZERO;
        self.body.update(dt, want, jump, world);

        let after = self.body.pos;
        let moved = Vec3::new(after.x - pos.x, 0.0, after.z - pos.z).length();
        self.jump_next = want != Vec3::ZERO && moved < 0.3 * speed * dt;
    }

    /// Box used for aiming, the wireframe highlight, and hit tests.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let pos = self.body.pos;
        let half = self.kind.half_width();
        (
            pos - Vec3::new(half, 0.0, half),
            pos + Vec3::new(half, self.kind.height(), half),
        )
    }

    /// Body and health bar vertices, with the front (and the bar) turned toward `look_at`.
    pub fn mesh(&self, look_at: Vec3) -> Vec<Vertex> {
        let pos = self.body.pos;
        let front = Vec3::new(look_at.x - pos.x, 0.0, look_at.z - pos.z)
            .try_normalize()
            .unwrap_or(Vec3::Z);
        let right = Vec3::new(front.z, 0.0, -front.x);

        let mut out = Vec::with_capacity(MAX_VERTICES_PER_ENEMY);
        for part in parts(self.kind) {
            for (f, face) in FACES.iter().enumerate() {
                for i in 0..4 {
                    let c = face.corners[i];
                    let mut local = [0.0f32; 3];
                    for k in 0..3 {
                        let size = part.max[k] - part.min[k];
                        local[k] = part.min[k] + c[k] * size;
                    }
                    let p = pos + right * local[0] + Vec3::Y * local[1] + front * local[2];
                    out.push(Vertex {
                        pos: p.to_array(),
                        uv: face.uv[i],
                        layer: part.layers[f],
                        shade: face.shade,
                    });
                }
            }
        }
        self.push_health_bar(&mut out, pos, front, right);
        out
    }

    /// Three flat quads facing the viewer: border, red (missing health), green (health left).
    /// The green one is anchored to one end so it shrinks as he takes damage.
    fn push_health_bar(&self, out: &mut Vec<Vertex>, pos: Vec3, front: Vec3, right: Vec3) {
        let max = self.kind.max_health() as f32;
        let fraction = (self.health.max(0) as f32 / max).clamp(0.0, 1.0);
        let center = pos + Vec3::Y * (self.kind.height() + BAR_LIFT);
        let (half_w, half_h) = (BAR_WIDTH / 2.0, BAR_HEIGHT / 2.0);

        // (left edge, right edge, half height, layer, how far toward the viewer)
        let quads = [
            (-half_w - BAR_BORDER, half_w + BAR_BORDER, half_h + BAR_BORDER, BAR_BACK, 0.0),
            (-half_w, half_w, half_h, BAR_LOST, BAR_LAYER_GAP),
            (
                -half_w,
                -half_w + BAR_WIDTH * fraction,
                half_h,
                BAR_HEALTH,
                2.0 * BAR_LAYER_GAP,
            ),
        ];
        for (left, right_edge, height, layer, lift) in quads {
            let origin = center + front * (0.1 + lift);
            for (x, y) in [(left, -height), (left, height), (right_edge, height), (right_edge, -height)] {
                out.push(Vertex {
                    pos: (origin + right * x + Vec3::Y * y).to_array(),
                    uv: [0.0, 0.0],
                    layer,
                    shade: 1.0,
                });
            }
        }
    }
}

/// Index list for any number of enemy quads, up to `vertex_count` vertices. Every mesh is a
/// run of 4-vertex quads, so a prefix of this list fits whatever is currently uploaded.
pub fn indices(vertex_count: usize) -> Vec<u32> {
    let mut out = Vec::with_capacity(vertex_count / 4 * 6);
    for quad in 0..(vertex_count / 4) as u32 {
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
    fn every_kind_fits_in_the_buffers() {
        for kind in [Kind::Skeleton, Kind::Deer] {
            let e = Enemy::new(1, kind, Vec3::new(10.0, 5.0, 10.0));
            let v = e.mesh(Vec3::ZERO);
            assert!(v.len() <= MAX_VERTICES_PER_ENEMY, "{kind:?}");
            assert_eq!(v.len() % 4, 0);
            assert!(parts(kind).len() <= MAX_PARTS);
        }
    }

    #[test]
    fn only_skeletons_start_hostile() {
        assert!(Enemy::new(1, Kind::Skeleton, Vec3::ZERO).hostile);
        assert!(!Enemy::new(2, Kind::Deer, Vec3::ZERO).hostile);
    }

    #[test]
    fn health_bar_shrinks_with_health() {
        for kind in [Kind::Skeleton, Kind::Deer] {
            let mut e = Enemy::new(1, kind, Vec3::new(10.0, 5.0, 10.0));
            let width = |e: &Enemy| {
                // The green quad is the last four vertices.
                let v = e.mesh(Vec3::ZERO);
                let g = &v[v.len() - 4..];
                assert_eq!(g[0].layer, BAR_HEALTH);
                (Vec3::from(g[3].pos) - Vec3::from(g[0].pos)).length()
            };
            assert!((width(&e) - BAR_WIDTH).abs() < 1e-4);
            e.health = kind.max_health() / 2;
            assert!((width(&e) - BAR_WIDTH / 2.0).abs() < 1e-4);
        }
    }

    #[test]
    fn chasers_walk_up_and_stop_at_arm_s_length() {
        let world = World::generate(4);
        for kind in [Kind::Skeleton, Kind::Deer] {
            let mut e = Enemy::new(1, kind, world.open_spot(120, 113));
            // Let him land first.
            for _ in 0..60 {
                e.step(1.0 / 60.0, e.pos(), false, &world);
            }
            let target = e.pos() + Vec3::new(0.0, 0.0, -4.0);
            let start = e.pos();
            for _ in 0..60 * 4 {
                e.step(1.0 / 60.0, target, true, &world);
            }
            let flat = |a: Vec3, b: Vec3| Vec3::new(a.x - b.x, 0.0, a.z - b.z).length();
            assert!(flat(e.pos(), start) > 1.5, "{kind:?} moved toward the target");
            let gap = flat(e.pos(), target);
            assert!((gap - STOP_DISTANCE).abs() < 0.5, "{kind:?} stopped at {gap}");
        }
    }

    #[test]
    fn passive_enemies_stay_put() {
        let world = World::generate(4);
        let mut e = Enemy::new(1, Kind::Deer, world.open_spot(112, 117));
        for _ in 0..60 {
            e.step(1.0 / 60.0, e.pos(), false, &world);
        }
        let start = e.pos();
        for _ in 0..120 {
            e.step(1.0 / 60.0, start + Vec3::new(0.0, 0.0, -6.0), false, &world);
        }
        assert!((e.pos() - start).length() < 0.01);
    }
}
