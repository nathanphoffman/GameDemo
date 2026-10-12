//! What a player's crosshair is on, and the fight with creatures.

use super::*;

impl Game {
    pub(super) fn block_in_sights_of(&self, p: usize) -> Option<(i32, i32, i32)> {
        self.block_hit_in_sights_of(p).map(|hit| hit.block)
    }

    /// Same, with the empty cell in front of the face that is being looked at.
    pub(super) fn block_hit_in_sights_of(&self, p: usize) -> Option<world::RayHit> {
        let body = &self.players[p].body;
        sights::block_hit_in_sights(&self.world, body.eye(), body.forward(), body.pos, &self.creatures())
    }

    /// The closest enemy that is in reach, under the crosshair, and not behind a block.
    pub(super) fn enemy_in_sights_of(&self, p: usize) -> Option<Sight> {
        let body = &self.players[p].body;
        sights::enemy_in_sights(&self.world, body.eye(), body.forward(), body.pos, &self.creatures())
    }

    fn creatures(&self) -> Vec<sights::Creature> {
        self.enemies
            .iter()
            .map(|e| sights::Creature::new(e.id, e.kind, e.hostile, e.pos()))
            .collect()
    }

    /// Hostile creatures within `range` of the player (horizontally, and not on another
    /// level), as combat candidates. Dead ones are left out.
    fn hostile_within(&self, p: usize, range: f32) -> Vec<Candidate> {
        let pos = self.players[p].body.pos;
        self.enemies
            .iter()
            .filter(|e| e.hostile && e.health > 0)
            .filter(|e| {
                let offset = e.pos() - pos;
                Vec3::new(offset.x, 0.0, offset.z).length() <= range && offset.y.abs() < 2.5
            })
            .map(|e| Candidate {
                id: e.id,
                hp: e.health,
            })
            .collect()
    }

    /// Who can hit the player right now, and who the player can hit.
    pub(super) fn fight_lists(&self, p: usize) -> (Vec<Candidate>, Vec<Candidate>) {
        (
            self.hostile_within(p, MELEE_RANGE),
            self.hostile_within(p, NEAR_RANGE),
        )
    }

    /// A click picks a fight with a passive enemy you are close to and looking at. It turns
    /// hostile and a round starts with my attack. After that it fights on its own like any
    /// other hostile one. Clicking a hostile enemy does nothing: those start rounds themselves.
    pub(super) fn attack(&mut self, p: usize) {
        if self.players[p].combat.player_hp <= 0 {
            return;
        }
        let Some(sight) = self.enemy_in_sights_of(p) else {
            return;
        };
        if sight.hostile || sight.distance > NEAR_RANGE {
            return;
        }
        // Clicking mid-round skips ahead: everything in flight lands right now.
        let (attackers, targets) = self.fight_lists(p);
        self.players[p]
            .combat
            .resolve_now(&mut self.dice, &attackers, &targets);
        self.events.push(Event::RoundSkipped { player: self.players[p].id });
        self.sync_enemies(p);

        if let Some(enemy) = self.enemies.iter_mut().find(|e| e.id == sight.id) {
            enemy.provoke();
        }
        let (attackers, targets) = self.fight_lists(p);
        self.players[p]
            .combat
            .begin_round(Side::Player, &attackers, &targets, &mut self.dice);
    }

    /// Copies the fight's result onto the creature it hit, and removes the dead.
    pub(super) fn sync_enemies(&mut self, p: usize) {
        if let Some(target) = self.players[p].combat.target
            && let Some(enemy) = self.enemies.iter_mut().find(|e| e.id == target.id)
        {
            enemy.health = enemy.health.min(target.hp);
        }
        self.enemies.retain(|e| e.health > 0);
    }

    /// Keeps a few creatures around the players: every few seconds one may appear a little
    /// way off one of them, and be the kind that lives in that spot's biome. Ones left far
    /// behind are removed.
    fn update_spawns(&mut self, dt: f32) {
        const MAX_NEARBY: usize = 6;
        const PER_KIND: usize = 3;
        const MIN_DISTANCE: f32 = 25.0;
        const MAX_DISTANCE: f32 = 45.0;
        const DESPAWN_DISTANCE: f32 = 90.0;

        let count = self.players.len();
        if count == 0 {
            return;
        }
        let near_anyone = |players: &[PlayerState], p: Vec3, range: f32| {
            players.iter().any(|pl| {
                let (dx, dz) = (p.x - pl.body.pos.x, p.z - pl.body.pos.z);
                Vec3::new(dx, 0.0, dz).length() <= range
            })
        };
        let players = &self.players;
        self.enemies
            .retain(|e| near_anyone(players, e.pos(), DESPAWN_DISTANCE));

        self.spawn_timer -= dt;
        if self.spawn_timer > 0.0 {
            return;
        }
        self.spawn_timer = 3.0;
        // The limits are per player, so a second player doesn't share the first one's.
        if self.enemies.len() >= MAX_NEARBY * count {
            return;
        }
        let who = if count > 1 {
            (self.spawn_dice.unit() * count as f32) as usize % count
        } else {
            0
        };
        let player = self.players[who].body.pos;
        let angle = self.spawn_dice.unit() * std::f32::consts::TAU;
        let distance = MIN_DISTANCE + self.spawn_dice.unit() * (MAX_DISTANCE - MIN_DISTANCE);
        let (x, z) = (
            (player.x + angle.cos() * distance).floor() as i32,
            (player.z + angle.sin() * distance).floor() as i32,
        );
        let kind = Kind::for_biome(self.world.biome(x, z));
        if self.enemies.iter().filter(|e| e.kind == kind).count() >= PER_KIND * count {
            return;
        }
        if let Some(spot) = self.world.standing_spot(x, z) {
            self.enemies.push(Enemy::new(self.next_enemy_id, kind, spot));
            self.next_enemy_id += 1;
        }
    }

    /// One frame of enemy behavior: hostile ones chase the closest living player, and once
    /// any of them is close enough to a player (and the pause after that player's last round
    /// is over) a round begins.
    pub(super) fn update_enemies(&mut self, dt: f32) {
        if self.players.is_empty() {
            return;
        }
        self.update_spawns(dt);
        for enemy in &mut self.enemies {
            let flat = |p: &PlayerState| {
                let o = enemy.pos() - p.body.pos;
                Vec3::new(o.x, 0.0, o.z).length()
            };
            let closest_alive = self
                .players
                .iter()
                .filter(|p| p.combat.player_hp > 0)
                .min_by(|a, b| flat(a).total_cmp(&flat(b)));
            match closest_alive {
                Some(p) => enemy.update(dt, p.body.pos, true, &self.world),
                None => enemy.update(dt, self.players[0].body.pos, false, &self.world),
            }
        }

        for p in 0..self.players.len() {
            let player = &mut self.players[p];
            player.round_pause = (player.round_pause - dt).max(0.0);
            if player.round_pause > 0.0 || player.combat.busy() || player.combat.player_hp <= 0 {
                continue;
            }
            let (attackers, targets) = self.fight_lists(p);
            if !attackers.is_empty() {
                self.players[p]
                    .combat
                    .begin_round(Side::Enemy, &attackers, &targets, &mut self.dice);
            }
        }
    }
}
