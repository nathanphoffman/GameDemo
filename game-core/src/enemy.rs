use std::time::{SystemTime, UNIX_EPOCH};

use glam::Vec3;

use crate::biome::Biome;
use crate::player::Player;
use crate::world::World;

/// Chasers stop this far from the player (horizontally).
const STOP_DISTANCE: f32 = 1.8;
/// A hostile creature notices the player, and starts chasing, inside this distance...
const AGGRO_RANGE: f32 = 12.0;
/// ...and gives up and heads home once the player is farther than this. Being larger than the
/// range above stops it from flickering between the two at the boundary.
const GIVE_UP_DISTANCE: f32 = 20.0;
/// Close enough to home to count as arrived.
const HOME_ARRIVAL: f32 = 0.4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Plains. Peaceful until attacked, then hostile.
    Deer,
    /// Forest. Fast and hostile.
    Wolf,
    /// Meadow. Peaceful until attacked, then hostile.
    Boar,
    /// Swamp. Slow, tough and hostile.
    BogZombie,
    /// Desert. Low and quick, hostile.
    Scorpion,
    /// Badlands. Very slow and very tough, hostile.
    Golem,
    /// Volcanic country. Hostile.
    Skeleton,
    /// Snow. Big and hostile.
    Yeti,
}

impl Kind {
    pub const ALL: [Kind; 8] = [
        Kind::Deer,
        Kind::Wolf,
        Kind::Boar,
        Kind::BogZombie,
        Kind::Scorpion,
        Kind::Golem,
        Kind::Skeleton,
        Kind::Yeti,
    ];

    /// The one creature that lives in each biome.
    pub fn for_biome(biome: Biome) -> Kind {
        match biome {
            Biome::Plains => Kind::Deer,
            Biome::Forest => Kind::Wolf,
            Biome::Meadow => Kind::Boar,
            Biome::Swamp => Kind::BogZombie,
            Biome::Desert => Kind::Scorpion,
            Biome::Badlands => Kind::Golem,
            Biome::Volcanic => Kind::Skeleton,
            Biome::Snow => Kind::Yeti,
        }
    }

    pub fn max_health(self) -> i32 {
        match self {
            Kind::Deer => 8,
            Kind::Wolf => 9,
            Kind::Boar => 10,
            Kind::BogZombie => 16,
            Kind::Scorpion => 7,
            Kind::Golem => 22,
            Kind::Skeleton => 12,
            Kind::Yeti => 18,
        }
    }

    /// Blocks per second when chasing or walking home. Well under the player's walk (5).
    fn chase_speed(self) -> f32 {
        match self {
            Kind::Deer => 3.0,
            Kind::Wolf => 3.6,
            Kind::Boar => 2.4,
            Kind::BogZombie => 1.4,
            Kind::Scorpion => 3.0,
            Kind::Golem => 1.0,
            Kind::Skeleton => 2.0,
            Kind::Yeti => 1.8,
        }
    }

    /// Half the width of the box used for aiming and the wireframe highlight.
    fn half_width(self) -> f32 {
        match self {
            Kind::Skeleton | Kind::BogZombie => 0.4,
            Kind::Wolf => 0.45,
            Kind::Boar => 0.5,
            Kind::Deer | Kind::Yeti => 0.55,
            Kind::Scorpion | Kind::Golem => 0.6,
        }
    }

    pub fn height(self) -> f32 {
        match self {
            Kind::Skeleton | Kind::BogZombie | Kind::Deer => 2.0,
            Kind::Wolf => 1.1,
            Kind::Boar => 1.0,
            Kind::Scorpion => 1.2,
            Kind::Golem => 2.3,
            Kind::Yeti => 2.25,
        }
    }

    pub fn starts_hostile(self) -> bool {
        !matches!(self, Kind::Deer | Kind::Boar)
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

    /// A number from 0 up to (not including) 1.
    pub fn unit(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32
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
    fn only_the_deer_and_the_boar_start_peaceful() {
        for kind in Kind::ALL {
            let peaceful = matches!(kind, Kind::Deer | Kind::Boar);
            assert_eq!(Enemy::new(1, kind, Vec3::ZERO).hostile, !peaceful, "{kind:?}");
        }
    }

    #[test]
    fn every_biome_has_its_own_creature() {
        let mut seen = Vec::new();
        for biome in Biome::ALL {
            let kind = Kind::for_biome(biome);
            assert!(!seen.contains(&kind), "{kind:?} is in two biomes");
            seen.push(kind);
        }
        assert_eq!(seen.len(), Kind::ALL.len());
        assert_eq!(Kind::for_biome(Biome::Plains), Kind::Deer, "hills are not spooky");
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
