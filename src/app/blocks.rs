//! Changing blocks: applying resources, placing, mining.

use super::*;

impl App {
    /// Applies one dense resource of `kind` to the block at `pos`, if it does anything there.
    /// Returns whether it did (and so whether a resource was used up).
    pub(crate) fn apply_dense(&mut self, kind: usize, (x, y, z): (i32, i32, i32)) -> bool {
        let Some(effect) = self.effect_at(kind, (x, y, z)) else {
            return false;
        };
        // A staircase takes one per step; everything else takes one.
        if !self.resources.spend_dense(kind, effect.dense_cost()) {
            return false;
        }
        use world::Block;
        let changed = match effect {
            Effect::Ignite => {
                let lit = self.world.replace_block(x, y, z, Block::Burning);
                if lit {
                    self.fires.ignite((x, y, z));
                }
                lit
            }
            Effect::Douse => self.world.replace_block(x, y, z, Block::Water),
            Effect::Freeze => self.world.replace_block(x, y, z, Block::Snow),
            Effect::Seed => self.world.replace_block(x, y, z, Block::Grass),
            Effect::GrowOak => self.world.grow_tree(x, y, z, false),
            Effect::GrowPine => self.world.grow_tree(x, y, z, true),
            Effect::LayFloor => self.world.replace_block(x, y, z, Block::WoodFloor),
            Effect::StoneFloor => self.world.replace_block(x, y, z, Block::StoneFloor),
            Effect::BuildForge => self.world.place_block(x, y + 1, z, Block::Forge),
            Effect::BuildFurnace => self.world.place_block(x, y + 1, z, Block::Furnace),
            Effect::BuildStairs => {
                let dir = self.stair_dir();
                let mut built = false;
                for (sx, sy, sz) in world::World::stair_cells(x, y, z, dir, STAIR_STEPS) {
                    built |= self.world.place_block(sx, sy, sz, Block::WoodFloor);
                }
                built
            }
            Effect::PlantHouse => self.world.place_block(x, y + 1, z, Block::HousePlant),
        };
        if changed {
            self.rebuild_mesh();
        }
        true
    }

    /// What applying dense resource `kind` to the block at `pos` would do right now: the
    /// effect for that type of block, if there is room for it (trees need space).
    pub(crate) fn effect_at(&self, kind: usize, pos: (i32, i32, i32)) -> Option<Effect> {
        let (x, y, z) = pos;
        let effect = effects::effect_for(kind, self.world.get(x, y, z))?;
        let fits = match effect {
            Effect::GrowOak => self.world.tree_fits(x, y, z, false),
            Effect::GrowPine => self.world.tree_fits(x, y, z, true),
            e if e.builds_on_top() => {
                self.world.get(x, y + 1, z) == world::Block::Air
                    && !self.cell_has_creature(x, y + 1, z)
            }
            Effect::BuildStairs => {
                let dir = self.stair_dir();
                self.world.stairs_fit(x, y, z, dir, STAIR_STEPS)
                    && !world::World::stair_cells(x, y, z, dir, STAIR_STEPS)
                        .any(|(sx, sy, sz)| self.cell_has_creature(sx, sy, sz))
            }
            _ => true,
        };
        fits.then_some(effect)
    }

    /// The direction the player is mostly facing, as a unit step along x or z.
    pub(crate) fn stair_dir(&self) -> (i32, i32) {
        let (fx, fz) = (self.player.yaw.sin(), -self.player.yaw.cos());
        if fx.abs() > fz.abs() {
            (fx.signum() as i32, 0)
        } else {
            (0, fz.signum() as i32)
        }
    }

    /// Whether the player or a creature is (partly) inside the cell at (x, y, z).
    pub(crate) fn cell_has_creature(&self, x: i32, y: i32, z: i32) -> bool {
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
        let body = (
            self.player.pos - half,
            self.player.pos + half + Vec3::new(0.0, player::HEIGHT, 0.0),
        );
        overlaps(body.0, body.1)
            || self.enemies.iter().any(|e| {
                let (min, max) = e.bounds();
                overlaps(min, max)
            })
    }

    /// Right click: places a block of the same type as the one the crosshair is on, against
    /// the face it is on, paying what that block would give if mined. It happens instantly.
    pub(crate) fn place_block(&mut self) {
        let Some(hit) = self.block_hit_in_sights() else {
            return;
        };
        let Some((x, y, z)) = hit.place else {
            return;
        };
        let (bx, by, bz) = hit.block;
        let block = self.world.get(bx, by, bz);
        if !self.resources.can_afford(block) {
            return;
        }

        // Don't entomb the player or an enemy.
        if self.cell_has_creature(x, y, z) {
            return;
        }

        if self.world.place_block(x, y, z, block) {
            self.resources.spend(block);
            self.rebuild_mesh();
        }
    }

    /// Sends the chunks of the world that changed to the GPU.
    pub(crate) fn rebuild_mesh(&mut self) {
        for (cx, cz) in self.world.take_dirty() {
            if let Some(gfx) = &mut self.gfx {
                let (vertices, indices) = self.world.build_chunk(cx, cz);
                gfx.set_chunk(cx, cz, &vertices, &indices);
            }
        }
    }

    pub(crate) fn mine(&mut self) {
        let Some((x, y, z)) =
            self.world
                .raycast(self.player.eye(), self.player.forward(), REACH)
        else {
            return;
        };
        let ice = self.world.is_ice(x, y, z);
        if let Some(block) = self.world.remove_block(x, y, z) {
            if let Some(music) = &self.music {
                music.dig(block, ice);
            }
            self.resources.gain(block);
            self.rebuild_mesh();
        }
    }
}
