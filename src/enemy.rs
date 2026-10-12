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
/// A hostile creature notices the player, and starts chasing, inside this distance...
const AGGRO_RANGE: f32 = 12.0;
/// ...and gives up and heads home once the player is farther than this. Being larger than the
/// range above stops it from flickering between the two at the boundary.
const GIVE_UP_DISTANCE: f32 = 20.0;
/// Close enough to home to count as arrived.
const HOME_ARRIVAL: f32 = 0.4;

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

    /// Blocks per second when chasing or walking home. Well under the player's walk (5).
    fn chase_speed(self) -> f32 {
        match self {
            Kind::Skeleton => 2.0,
            Kind::Deer => 3.0,
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

/// What an enemy is doing with itself right now.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Behavior {
    /// Standing at home.
    Idle,
    /// Going after the player.
    Chase,
    /// Gave up; walking back home.
    Return,
}

pub struct Enemy {
    pub id: u32,
    pub kind: Kind,
    /// Walks, falls and collides with the world like the player does.
    body: Player,
    pub health: i32,
    /// Hostile enemies chase the player and attack when close. Passive ones just stand.
    pub hostile: bool,
    /// Where it was placed; it goes back here after giving up.
    home: Vec3,
    pub behavior: Behavior,
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
            home: pos,
            behavior: Behavior::Idle,
            jump_next: false,
        }
    }

    /// Bottom center.
    pub fn pos(&self) -> Vec3 {
        self.body.pos
    }

    /// Makes the enemy hostile and sends it after the player (a click on a passive animal).
    pub fn provoke(&mut self) {
        self.hostile = true;
        self.behavior = Behavior::Chase;
    }

    /// What the body should face: the player, except while walking home.
    pub fn facing(&self, player: Vec3) -> Vec3 {
        if self.behavior == Behavior::Return {
            self.home
        } else {
            player
        }
    }

    /// One frame of behavior and movement.
    ///
    /// Hostile creatures start chasing when the player is within `AGGRO_RANGE` and give up
    /// past `GIVE_UP_DISTANCE`, then walk back home. A passive creature that had been
    /// provoked also calms down when it gives up.
    pub fn update(&mut self, dt: f32, player: Vec3, player_alive: bool, world: &World) {
        let flat = |a: Vec3, b: Vec3| Vec3::new(a.x - b.x, 0.0, a.z - b.z).length();
        let distance = flat(player, self.body.pos);
        let wants_fight = self.hostile && player_alive && distance <= AGGRO_RANGE;

        match self.behavior {
            Behavior::Idle => {
                if wants_fight {
                    self.behavior = Behavior::Chase;
                }
            }
            Behavior::Chase => {
                if !self.hostile || !player_alive || distance > GIVE_UP_DISTANCE {
                    self.behavior = Behavior::Return;
                    if !self.kind.starts_hostile() {
                        self.hostile = false;
                    }
                }
            }
            Behavior::Return => {
                if wants_fight {
                    self.behavior = Behavior::Chase;
                } else if flat(self.home, self.body.pos) <= HOME_ARRIVAL {
                    self.behavior = Behavior::Idle;
                }
            }
        }

        let goal = match self.behavior {
            Behavior::Idle => None,
            Behavior::Chase => Some((player, STOP_DISTANCE)),
            Behavior::Return => Some((self.home, HOME_ARRIVAL)),
        };
        self.walk(dt, goal, world);
    }

    /// Walks toward `goal` (a point and how close to stop), or just stands and falls.
    fn walk(&mut self, dt: f32, goal: Option<(Vec3, f32)>, world: &World) {
        let speed = self.kind.chase_speed();
        let pos = self.body.pos;
        let want = match goal {
            Some((target, stop)) => {
                let flat = Vec3::new(target.x - pos.x, 0.0, target.z - pos.z);
                let distance = flat.length();
                if distance > stop {
                    flat / distance * speed
                } else {
                    Vec3::ZERO
                }
            }
            None => Vec3::ZERO,
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

    /// Runs `seconds` of game time with the player standing at `player`.
    fn run(e: &mut Enemy, world: &World, player: Vec3, seconds: f32) {
        for _ in 0..(seconds * 60.0) as u32 {
            e.update(1.0 / 60.0, player, true, world);
        }
    }

    fn flat(a: Vec3, b: Vec3) -> f32 {
        Vec3::new(a.x - b.x, 0.0, a.z - b.z).length()
    }

    #[test]
    fn hostile_chases_gives_up_returns_home_and_notices_again() {
        let world = World::generate(4);
        let mut e = Enemy::new(1, Kind::Skeleton, world.open_spot(120, 113));
        let home = e.pos();
        run(&mut e, &world, home + Vec3::new(0.0, 0.0, 60.0), 1.0); // settle; player far away
        assert_eq!(e.behavior, Behavior::Idle, "too far to notice");
        let settled = e.pos();

        // The player appears nearby: he comes over (a tree or step can still slow him down).
        let near = settled + Vec3::new(0.0, 0.0, -8.0);
        run(&mut e, &world, near, 0.5);
        assert_eq!(e.behavior, Behavior::Chase);
        run(&mut e, &world, near, 6.0);
        assert!(flat(e.pos(), near) < 8.0 - 4.0, "closed in: {}", flat(e.pos(), near));

        // The player runs far away: he gives up and walks back to where he started.
        let far = e.pos() + Vec3::new(0.0, 0.0, -(GIVE_UP_DISTANCE + 5.0));
        run(&mut e, &world, far, 0.2);
        assert_eq!(e.behavior, Behavior::Return);
        assert!(e.hostile, "a skeleton stays hostile");
        run(&mut e, &world, far, 12.0);
        assert!(flat(e.pos(), settled) < 1.0, "back home: {}", flat(e.pos(), settled));
        assert_eq!(e.behavior, Behavior::Idle);

        // And he notices the player again.
        run(&mut e, &world, near, 0.5);
        assert_eq!(e.behavior, Behavior::Chase);
    }

    #[test]
    fn provoked_deer_calms_down_and_goes_home() {
        let world = World::generate(4);
        let mut e = Enemy::new(2, Kind::Deer, world.open_spot(112, 117));
        let spot = e.pos();
        run(&mut e, &world, spot + Vec3::new(0.0, 0.0, 60.0), 1.0); // settle
        let home = e.pos();
        run(&mut e, &world, home + Vec3::new(0.0, 0.0, -30.0), 0.5);
        assert_eq!(e.behavior, Behavior::Idle, "passive deer ignore the player");
        assert!(!e.hostile);

        let player = home + Vec3::new(0.0, 0.0, -3.0);
        e.provoke();
        assert!(e.hostile);
        run(&mut e, &world, player, 1.0);
        assert_eq!(e.behavior, Behavior::Chase);

        let far = e.pos() + Vec3::new(0.0, 0.0, -(GIVE_UP_DISTANCE + 5.0));
        run(&mut e, &world, far, 0.2);
        assert_eq!(e.behavior, Behavior::Return);
        assert!(!e.hostile, "passive again");
        run(&mut e, &world, far, 15.0);
        assert!(flat(e.pos(), home) < 1.0, "home: {}", flat(e.pos(), home));
        assert_eq!(e.behavior, Behavior::Idle);

        // Standing next to a calm deer doesn't start anything.
        run(&mut e, &world, home + Vec3::new(0.0, 0.0, -3.0), 3.0);
        assert_eq!(e.behavior, Behavior::Idle);
        assert!(flat(e.pos(), home) < 0.5);
    }

    #[test]
    fn passive_enemies_stay_put() {
        let world = World::generate(4);
        let mut e = Enemy::new(1, Kind::Deer, world.open_spot(112, 117));
        let spot = e.pos();
        run(&mut e, &world, spot, 1.0);
        let start = e.pos();
        run(&mut e, &world, start + Vec3::new(0.0, 0.0, -6.0), 2.0);
        assert!((e.pos() - start).length() < 0.01);
    }
}
