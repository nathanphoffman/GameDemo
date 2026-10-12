//! What a player's crosshair is on, and where things could be built. These work from plain
//! data (the world, a position, a list of creatures), so the host's `Game` and a player's own
//! copy of the world ask exactly the same questions and get the same answers.

use glam::Vec3;

use super::{REACH, STAIR_STEPS};
use crate::effects::{self, Effect};
use crate::enemy::{self, Kind, Sight};
use crate::player;
use crate::world::{Block, RayHit, World};

/// A creature, as far as aiming and building are concerned.
#[derive(Clone, Copy, Debug)]
pub struct Creature {
    pub id: u32,
    pub hostile: bool,
    /// Feet position.
    pub pos: Vec3,
    pub min: Vec3,
    pub max: Vec3,
}

impl Creature {
    pub fn new(id: u32, kind: Kind, hostile: bool, pos: Vec3) -> Creature {
        let (min, max) = kind.bounds_at(pos);
        Creature { id, hostile, pos, min, max }
    }
}

/// The box a player's body fills when standing at `pos`.
pub fn player_box(pos: Vec3) -> (Vec3, Vec3) {
    let half = Vec3::new(player::HALF_WIDTH, 0.0, player::HALF_WIDTH);
    (pos - half, pos + half + Vec3::new(0.0, player::HEIGHT, 0.0))
}

/// The closest creature in reach, under the crosshair (from `eye` looking along `dir`), and
/// not behind a block. `standing` is where the player's feet are.
pub fn enemy_in_sights(
    world: &World,
    eye: Vec3,
    dir: Vec3,
    standing: Vec3,
    creatures: &[Creature],
) -> Option<Sight> {
    let (mut best, mut best_dist) = (None, f32::INFINITY);
    for creature in creatures {
        let Some(dist) =
            enemy::ray_box(eye, dir, creature.min, creature.max).filter(|d| *d <= REACH)
        else {
            continue;
        };
        if dist < best_dist {
            let offset = creature.pos - standing;
            best_dist = dist;
            best = Some(Sight {
                id: creature.id,
                hostile: creature.hostile,
                min: creature.min,
                max: creature.max,
                distance: Vec3::new(offset.x, 0.0, offset.z).length(),
            });
        }
    }
    let sight = best?;
    if let Some((x, y, z)) = world.raycast(eye, dir, REACH) {
        let block_min = Vec3::new(x as f32, y as f32, z as f32);
        let block_dist = enemy::ray_box(eye, dir, block_min, block_min + Vec3::ONE);
        if block_dist.is_some_and(|d| d < best_dist) {
            return None;
        }
    }
    Some(sight)
}

/// The block under the crosshair within reach, with the empty cell in front of the face being
/// looked at, unless a creature is what the crosshair is on.
pub fn block_hit_in_sights(
    world: &World,
    eye: Vec3,
    dir: Vec3,
    standing: Vec3,
    creatures: &[Creature],
) -> Option<RayHit> {
    if enemy_in_sights(world, eye, dir, standing, creatures).is_some() {
        return None;
    }
    world.raycast_face(eye, dir, REACH)
}

/// The direction a player facing `yaw` is mostly facing, as a unit step along x or z.
pub fn stair_dir(yaw: f32) -> (i32, i32) {
    let (fx, fz) = (yaw.sin(), -yaw.cos());
    if fx.abs() > fz.abs() {
        (fx.signum() as i32, 0)
    } else {
        (0, fz.signum() as i32)
    }
}

/// Whether any of the boxes is (partly) inside the cell at (x, y, z).
pub fn occupied(boxes: &[(Vec3, Vec3)], x: i32, y: i32, z: i32) -> bool {
    let cell_min = Vec3::new(x as f32, y as f32, z as f32);
    let cell_max = cell_min + Vec3::ONE;
    boxes.iter().any(|(min, max)| {
        min.x < cell_max.x
            && max.x > cell_min.x
            && min.y < cell_max.y
            && max.y > cell_min.y
            && min.z < cell_max.z
            && max.z > cell_min.z
    })
}

/// What applying dense resource `kind` to the block at `pos` would do right now: the effect
/// for that type of block, if there is room for it (trees need space, nothing can be built
/// inside a player or creature, whose boxes are `occupants`). `yaw` is where the player
/// faces, which decides which way stairs go.
pub fn effect_at(
    world: &World,
    kind: usize,
    pos: (i32, i32, i32),
    yaw: f32,
    occupants: &[(Vec3, Vec3)],
) -> Option<Effect> {
    let (x, y, z) = pos;
    let effect = effects::effect_for(kind, world.get(x, y, z))?;
    let fits = match effect {
        Effect::GrowOak => world.tree_fits(x, y, z, false),
        Effect::GrowPine => world.tree_fits(x, y, z, true),
        e if e.builds_on_top() => {
            world.get(x, y + 1, z) == Block::Air && !occupied(occupants, x, y + 1, z)
        }
        Effect::BuildStairs => {
            let dir = stair_dir(yaw);
            world.stairs_fit(x, y, z, dir, STAIR_STEPS)
                && !World::stair_cells(x, y, z, dir, STAIR_STEPS)
                    .any(|(sx, sy, sz)| occupied(occupants, sx, sy, sz))
        }
        _ => true,
    };
    fits.then_some(effect)
}
