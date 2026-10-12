//! What the crosshair is on, and the fight with enemies.

use super::*;

impl App {
    /// The block under the crosshair within reach, unless an enemy is what you're looking at.
    pub(crate) fn block_in_sights(&self) -> Option<(i32, i32, i32)> {
        self.block_hit_in_sights().map(|hit| hit.block)
    }

    /// Same, with the empty cell in front of the face that is being looked at.
    pub(crate) fn block_hit_in_sights(&self) -> Option<world::RayHit> {
        if self.enemy_in_sights().is_some() {
            return None;
        }
        self.world
            .raycast_face(self.player.eye(), self.player.forward(), REACH)
    }

    /// The closest enemy that is in reach, under the crosshair, and not behind a block.
    pub(crate) fn enemy_in_sights(&self) -> Option<Sight> {
        let (eye, dir) = (self.player.eye(), self.player.forward());
        let (mut best, mut best_dist) = (None, f32::INFINITY);
        for enemy in &self.enemies {
            let (min, max) = enemy.bounds();
            let Some(dist) = enemy::ray_box(eye, dir, min, max).filter(|d| *d <= REACH) else {
                continue;
            };
            if dist < best_dist {
                let offset = enemy.pos() - self.player.pos;
                best_dist = dist;
                best = Some(Sight {
                    id: enemy.id,
                    hostile: enemy.hostile,
                    min,
                    max,
                    distance: Vec3::new(offset.x, 0.0, offset.z).length(),
                });
            }
        }
        let sight = best?;
        if let Some((x, y, z)) = self.world.raycast(eye, dir, REACH) {
            let block_min = Vec3::new(x as f32, y as f32, z as f32);
            let block_dist = enemy::ray_box(eye, dir, block_min, block_min + Vec3::ONE);
            if block_dist.is_some_and(|d| d < best_dist) {
                return None;
            }
        }
        Some(sight)
    }

    /// Hostile creatures within `range` of the player (horizontally, and not on another
    /// level), as combat candidates. Dead ones are left out.
    pub(crate) fn hostile_within(&self, range: f32) -> Vec<Candidate> {
        self.enemies
            .iter()
            .filter(|e| e.hostile && e.health > 0)
            .filter(|e| {
                let offset = e.pos() - self.player.pos;
                Vec3::new(offset.x, 0.0, offset.z).length() <= range && offset.y.abs() < 2.5
            })
            .map(|e| Candidate {
                id: e.id,
                hp: e.health,
            })
            .collect()
    }

    /// Who can hit the player right now, and who the player can hit.
    pub(crate) fn fight_lists(&self) -> (Vec<Candidate>, Vec<Candidate>) {
        (
            self.hostile_within(MELEE_RANGE),
            self.hostile_within(NEAR_RANGE),
        )
    }

    /// A click picks a fight with a passive enemy you are close to and looking at. It turns
    /// hostile and a round starts with my attack. After that it fights on its own like any
    /// other hostile one. Clicking a hostile enemy does nothing: those start rounds themselves.
    pub(crate) fn attack(&mut self) {
        if self.combat.player_hp <= 0 {
            return;
        }
        let Some(sight) = self.enemy_in_sights() else {
            return;
        };
        if sight.hostile || sight.distance > NEAR_RANGE {
            return;
        }
        // Clicking mid-round skips ahead: everything in flight lands right now, and the
        // health bar jumps to its final value instead of draining.
        let (attackers, targets) = self.fight_lists();
        self.combat.resolve_now(&mut self.dice, &attackers, &targets);
        self.hp_shown = self.combat.player_hp as f32;
        self.sync_enemies();

        if let Some(enemy) = self.enemies.iter_mut().find(|e| e.id == sight.id) {
            enemy.provoke();
        }
        let (attackers, targets) = self.fight_lists();
        self.combat
            .begin_round(Side::Player, &attackers, &targets, &mut self.dice);
    }

    /// Copies the fight's result onto the creature it hit, and removes the dead.
    pub(crate) fn sync_enemies(&mut self) {
        if let Some(target) = self.combat.target
            && let Some(enemy) = self.enemies.iter_mut().find(|e| e.id == target.id) {
                enemy.health = enemy.health.min(target.hp);
            }
        self.enemies.retain(|e| e.health > 0);
    }

    /// One frame of enemy behavior: hostile ones chase the player, and once any of them is
    /// close enough (and the pause after the last round is over) a round begins.
    pub(crate) fn update_enemies(&mut self, dt: f32) {
        let player_alive = self.combat.player_hp > 0;
        let target = self.player.pos;
        for enemy in &mut self.enemies {
            enemy.update(dt, target, player_alive, &self.world);
        }

        self.round_pause = (self.round_pause - dt).max(0.0);
        if self.round_pause > 0.0 || self.combat.busy() || !player_alive {
            return;
        }
        let (attackers, targets) = self.fight_lists();
        if !attackers.is_empty() {
            self.combat
                .begin_round(Side::Enemy, &attackers, &targets, &mut self.dice);
        }
    }
}
