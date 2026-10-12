//! The per-frame simulation steps that don't draw anything.

use super::*;

impl App {
    /// Walks the player along the keys held.
    pub(super) fn move_player(&mut self, dt: f32) {
        let (forward, right) = self.player.flat_axes();
        let mut dir = Vec3::ZERO;
        if self.keys.w {
            dir += forward;
        }
        if self.keys.s {
            dir -= forward;
        }
        if self.keys.d {
            dir += right;
        }
        if self.keys.a {
            dir -= right;
        }
        let speed = if self.keys.sprint { SPRINT_SPEED } else { WALK_SPEED };
        let velocity = dir.normalize_or_zero() * speed;
        self.player.update(dt, velocity, self.keys.jump, &self.world);
    }

    /// Mining progress builds while the button is held on a block.
    pub(super) fn update_mining(&mut self, dt: f32) {
        // Mining: progress builds while the button is held on a block. Looking at the
        // enemy or away from the ground, or letting go, starts over.
        if self.keys.mining && self.block_in_sights().is_some() {
            self.mine_timer += dt;
            if self.mine_timer >= MINE_INTERVAL {
                self.mine_timer -= MINE_INTERVAL;
                self.mine();
            }
        } else {
            self.mine_timer = 0.0;
        }
    }

    /// The fight: dice age and land, damage applies, replies roll; enemies move.
    pub(super) fn update_fight(&mut self, dt: f32) {
        // The fight: dice age and land, damage applies, replies roll.
        let (attackers, targets) = self.fight_lists();
        self.combat
            .update(dt, &mut self.dice, &attackers, &targets);
        self.sync_enemies();
        let busy = self.combat.busy();
        if self.was_busy && !busy {
            self.round_pause = ROUND_PAUSE;
        }
        self.was_busy = busy;
        self.update_enemies(dt);
        let hp = self.combat.player_hp as f32;
        self.hp_shown = (self.hp_shown - HP_DRAIN_PER_SECOND * dt).max(hp);
    }

    /// The conversion pipeline and the fires, which work in the background. Returns the
    /// conversion speed-up from a nearby forge or furnace.
    pub(super) fn update_background(&mut self, dt: f32) -> u32 {
        // The conversion pipeline and the fires work in the background. Standing
        // near a forge makes the conversion twice as fast.
        let eye = self.player.eye();
        let conversion_boost = if self
            .world
            .block_within(eye, FURNACE_RANGE, world::Block::Furnace)
        {
            4
        } else if self.world.block_within(eye, FORGE_RANGE, world::Block::Forge) {
            2
        } else {
            1
        };
        self.converter
            .update(dt * conversion_boost as f32, &mut self.resources);
        let burnt_out = self.fires.update(dt);
        let mut changed = false;
        for (x, y, z) in burnt_out {
            // Only if it is still the burning block (it may have been mined).
            if self.world.get(x, y, z) == world::Block::Burning {
                self.world.replace_block(x, y, z, world::Block::Coal);
                changed = true;
            }
        }
        if changed {
            self.rebuild_mesh();
        }
        conversion_boost
    }
}
