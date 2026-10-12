//! Changing blocks: applying resources, placing, mining.

use super::*;

impl Game {
    /// Applies one dense resource of `kind` to the block at `pos`, if it does anything there.
    /// Returns whether it did (and so whether a resource was used up).
    pub(super) fn apply_dense(&mut self, p: usize, kind: usize, (x, y, z): (i32, i32, i32)) -> bool {
        let Some(effect) = self.effect_at_of(p, kind, (x, y, z)) else {
            return false;
        };
        // A staircase takes one per step; everything else takes one.
        if !self.players[p].resources.spend_dense(kind, effect.dense_cost()) {
            return false;
        }
        match effect {
            Effect::Ignite => {
                let lit = self.world.replace_block(x, y, z, Block::Burning);
                if lit {
                    self.fires.ignite((x, y, z));
                }
            }
            Effect::Douse => {
                self.world.replace_block(x, y, z, Block::Water);
            }
            Effect::Freeze => {
                self.world.replace_block(x, y, z, Block::Snow);
            }
            Effect::Seed => {
                self.world.replace_block(x, y, z, Block::Grass);
            }
            Effect::GrowOak => {
                self.world.grow_tree(x, y, z, false);
            }
            Effect::GrowPine => {
                self.world.grow_tree(x, y, z, true);
            }
            Effect::LayFloor => {
                self.world.replace_block(x, y, z, Block::WoodFloor);
            }
            Effect::StoneFloor => {
                self.world.replace_block(x, y, z, Block::StoneFloor);
            }
            Effect::BuildForge => {
                self.world.place_block(x, y + 1, z, Block::Forge);
            }
            Effect::BuildFurnace => {
                self.world.place_block(x, y + 1, z, Block::Furnace);
            }
            Effect::BuildStairs => {
                let dir = self.stair_dir(p);
                for (sx, sy, sz) in World::stair_cells(x, y, z, dir, STAIR_STEPS) {
                    self.world.place_block(sx, sy, sz, Block::WoodFloor);
                }
            }
            Effect::PlantHouse => {
                self.world.place_block(x, y + 1, z, Block::HousePlant);
            }
        }
        true
    }

    /// What applying dense resource `kind` to the block at `pos` would do right now: the
    /// effect for that type of block, if there is room for it (trees need space).
    pub(super) fn effect_at_of(&self, p: usize, kind: usize, pos: (i32, i32, i32)) -> Option<Effect> {
        let (x, y, z) = pos;
        let effect = effects::effect_for(kind, self.world.get(x, y, z))?;
        let fits = match effect {
            Effect::GrowOak => self.world.tree_fits(x, y, z, false),
            Effect::GrowPine => self.world.tree_fits(x, y, z, true),
            e if e.builds_on_top() => {
                self.world.get(x, y + 1, z) == Block::Air && !self.cell_has_creature(x, y + 1, z)
            }
            Effect::BuildStairs => {
                let dir = self.stair_dir(p);
                self.world.stairs_fit(x, y, z, dir, STAIR_STEPS)
                    && !World::stair_cells(x, y, z, dir, STAIR_STEPS)
                        .any(|(sx, sy, sz)| self.cell_has_creature(sx, sy, sz))
            }
            _ => true,
        };
        fits.then_some(effect)
    }

    /// The direction the player is mostly facing, as a unit step along x or z.
    fn stair_dir(&self, p: usize) -> (i32, i32) {
        let yaw = self.players[p].body.yaw;
        let (fx, fz) = (yaw.sin(), -yaw.cos());
        if fx.abs() > fz.abs() {
            (fx.signum() as i32, 0)
        } else {
            (0, fz.signum() as i32)
        }
    }

    /// Whether a player or a creature is (partly) inside the cell at (x, y, z).
    fn cell_has_creature(&self, x: i32, y: i32, z: i32) -> bool {
        let cell_min = Vec3::new(x as f32, y as f32, z as f32);
        let cell_max = cell_min + Vec3::ONE;
        let overlaps = |min: Vec3, max: Vec3| {
            min.x < cell_max.x
                && max.x > cell_min.x
                && min.y < cell_max.y
                && max.y > cell_min.y
                && min.z < cell_max.z
                && max.z > cell_min.z
        };
        let half = Vec3::new(player::HALF_WIDTH, 0.0, player::HALF_WIDTH);
        self.players.iter().any(|pl| {
            let pos = pl.body.pos;
            overlaps(pos - half, pos + half + Vec3::new(0.0, player::HEIGHT, 0.0))
        }) || self.enemies.iter().any(|e| {
            let (min, max) = e.bounds();
            overlaps(min, max)
        })
    }

    /// Right click: places a block of the same type as the one the crosshair is on, against
    /// the face it is on, paying what that block would give if mined. It happens instantly.
    pub(super) fn place_block(&mut self, p: usize) {
        let Some(hit) = self.block_hit_in_sights_of(p) else {
            return;
        };
        let Some((x, y, z)) = hit.place else {
            return;
        };
        let (bx, by, bz) = hit.block;
        let block = self.world.get(bx, by, bz);
        if !self.players[p].resources.can_afford(block) {
            return;
        }

        // Don't entomb a player or an enemy.
        if self.cell_has_creature(x, y, z) {
            return;
        }

        if self.world.place_block(x, y, z, block) {
            self.players[p].resources.spend(block);
        }
    }

    pub(super) fn mine(&mut self, p: usize) {
        let body = &self.players[p].body;
        let Some((x, y, z)) = self.world.raycast(body.eye(), body.forward(), REACH) else {
            return;
        };
        let ice = self.world.is_ice(x, y, z);
        if let Some(block) = self.world.remove_block(x, y, z) {
            let player = &mut self.players[p];
            player.resources.gain(block);
            self.events.push(Event::Dug { player: player.id, block, ice });
        }
    }
}
