//! Time passing: walking, mining, fights, weather, conversion and fires.

use super::*;

impl Game {
    /// Advances the game by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        for p in 0..self.players.len() {
            self.move_player(p, dt);
            self.update_mining(p, dt);
        }

        // The fight: dice age and land, damage applies, replies roll; enemies move.
        for p in 0..self.players.len() {
            let (attackers, targets) = self.fight_lists(p);
            self.players[p]
                .combat
                .update(dt, &mut self.dice, &attackers, &targets);
            self.sync_enemies(p);
            let player = &mut self.players[p];
            let busy = player.combat.busy();
            if player.was_busy && !busy {
                player.round_pause = ROUND_PAUSE;
            }
            player.was_busy = busy;
        }
        self.update_enemies(dt);

        // Only which weather it is lives here. How it looks to a player (snow in the cold,
        // dust storms in the desert, blue under water) depends on where they are, so each
        // player's own game works that out.
        self.weather.update(dt);

        self.update_background(dt);
    }

    /// Walks the player along the keys held (unless they walk on their own computer).
    fn move_player(&mut self, p: usize, dt: f32) {
        let player = &mut self.players[p];
        player.since_pose += dt;
        if player.pose_from_client {
            return;
        }
        walk(&mut player.body, &player.input, dt, &self.world);
    }

    /// Mining progress builds while the button is held on a block. Looking at an enemy or away
    /// from the ground, or letting go, starts over.
    fn update_mining(&mut self, p: usize, dt: f32) {
        let on_block = self.block_in_sights_of(p).is_some();
        let player = &mut self.players[p];
        if player.input.mining && on_block {
            player.mine_timer += dt;
            if player.mine_timer >= MINE_INTERVAL {
                player.mine_timer -= MINE_INTERVAL;
                self.mine(p);
            }
        } else {
            player.mine_timer = 0.0;
        }
    }

    /// The conversion pipeline and the fires work in the background. Standing near a forge
    /// makes the conversion twice as fast, near a furnace four times.
    fn update_background(&mut self, dt: f32) {
        for player in &mut self.players {
            let eye = player.body.eye();
            player.conversion_boost =
                if self.world.block_within(eye, FURNACE_RANGE, Block::Furnace) {
                    4
                } else if self.world.block_within(eye, FORGE_RANGE, Block::Forge) {
                    2
                } else {
                    1
                };
            player
                .converter
                .update(dt * player.conversion_boost as f32, &mut player.resources);
        }
        for (x, y, z) in self.fires.update(dt) {
            // Only if it is still the burning block (it may have been mined).
            if self.world.get(x, y, z) == Block::Burning {
                self.world.replace_block(x, y, z, Block::Coal);
            }
        }
    }
}
