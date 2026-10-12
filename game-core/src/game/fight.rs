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
                bonus: e.kind.damage_bonus(),
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
        self.announce_hits(p);
        self.sync_enemies(p);

        if let Some(enemy) = self.enemies.iter_mut().find(|e| e.id == sight.id) {
            enemy.provoke();
        }
        let (attackers, targets) = self.fight_lists(p);
        self.players[p]
            .combat
            .begin_round(Side::Player, &attackers, &targets, &mut self.dice);
    }

    /// Turns the hits that just landed into events, so every screen can write them up.
    pub(super) fn announce_hits(&mut self, p: usize) {
        let player = self.players[p].id;
        for hit in std::mem::take(&mut self.players[p].combat.landed) {
            if let Some(enemy) = self.enemies.iter().find(|e| e.id == hit.enemy) {
                self.events.push(Event::Hit {
                    player,
                    enemy: enemy.kind,
                    by_player: hit.by_player,
                    dice: (hit.a, hit.b),
                    bonus: hit.bonus,
                    killed: hit.killed,
                });
            }
        }
    }

    /// Copies the fight's result onto the creature it hit, and removes the dead.
    pub(super) fn sync_enemies(&mut self, p: usize) {
        if let Some(target) = self.players[p].combat.target
            && let Some(enemy) = self.enemies.iter_mut().find(|e| e.id == target.id)
        {
            enemy.health = enemy.health.min(target.hp);
        }
        // A defeated creature's home is empty until the wait is over.
        for dead in self.enemies.iter().filter(|e| e.health <= 0) {
            self.defeated.insert(dead.id, spawns::RESPAWN_SECONDS);
        }
        self.enemies.retain(|e| e.health > 0);
    }

    /// One frame of enemy behavior: hostile ones chase the closest living player, and once
    /// any of them is close enough to a player (and the pause after that player's last round
    /// is over) a round begins.
    pub(super) fn update_enemies(&mut self, dt: f32) {
        if self.players.is_empty() {
            return;
        }
        self.update_creature_homes(dt);
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
