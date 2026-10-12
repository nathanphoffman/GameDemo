//! What one player is told about the game each tick.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use super::{Game, PlayerId, MINE_INTERVAL};
use crate::enemy;
use crate::resources::Resources;
use crate::weather;

/// Another player (or you), as drawn.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerView {
    pub id: PlayerId,
    /// Feet position.
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub hp: i32,
    /// The block they are part way through mining, once it shows cracks.
    pub cracking: Option<(i32, i32, i32)>,
}

/// A creature, with what it takes to draw it and its health bar.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnemyView {
    pub id: u32,
    pub kind: enemy::Kind,
    pub pos: Vec3,
    pub health: i32,
    pub hostile: bool,
    /// Where it is walking back to; otherwise it faces whoever is looking at it.
    pub returning_home: Option<Vec3>,
}

/// What only you are told: the numbers on your own screen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct YouView {
    pub resources: Resources,
    /// The kind the converter is working on, and how far through the wait it is.
    pub converting: Option<(usize, f32)>,
    pub hp: i32,
    /// Dice as (a, b, age), for the player's roll and for the creatures'.
    pub player_roll: Option<(u32, u32, f32)>,
    pub enemy_rolls: Vec<(u32, u32, f32)>,
    pub mine_timer: f32,
    pub conversion_boost: u32,
}

/// Everything about the game that a player's screen shows, apart from the blocks of the world
/// (those are sent as changes).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub players: Vec<PlayerView>,
    pub enemies: Vec<EnemyView>,
    pub weather: weather::Kind,
    pub you: YouView,
}

impl Game {
    /// What player `id` is told this tick, or None if there is no such player.
    pub fn snapshot_for(&self, id: PlayerId) -> Option<Snapshot> {
        let me = self.player(id)?;
        Some(Snapshot {
            players: self
                .players
                .iter()
                .enumerate()
                .map(|(i, p)| PlayerView {
                    id: p.id,
                    pos: p.body.pos,
                    yaw: p.body.yaw,
                    pitch: p.body.pitch,
                    hp: p.combat.player_hp,
                    cracking: (p.input.mining && p.mine_timer >= MINE_INTERVAL / 2.0)
                        .then(|| self.block_in_sights_of(i))
                        .flatten(),
                })
                .collect(),
            enemies: self
                .enemies
                .iter()
                .map(|e| EnemyView {
                    id: e.id,
                    kind: e.kind,
                    pos: e.pos(),
                    health: e.health,
                    hostile: e.hostile,
                    returning_home: e.returning_home(),
                })
                .collect(),
            weather: self.weather.kind,
            you: YouView {
                resources: me.resources,
                converting: me
                    .converter
                    .selected()
                    .map(|kind| (kind, me.converter.fraction())),
                hp: me.combat.player_hp,
                player_roll: me.combat.player_roll.map(|r| r.view()),
                enemy_rolls: me.combat.enemy_rolls.iter().map(|r| r.view()).collect(),
                mine_timer: me.mine_timer,
                conversion_boost: me.conversion_boost,
            },
        })
    }
}

/// `from` turned toward `to` by `t`, the short way round.
fn lerp_angle(from: f32, to: f32, t: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let diff = (to - from + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI;
    from + diff * t
}

/// Things this far apart between two updates were not walking, so are not blended.
const TELEPORT: f32 = 8.0;

impl Snapshot {
    /// The players and creatures part way (`t`, 0 to 1) from where they were in `previous` to
    /// where they are in `self`, which draws smoothly although updates come 30 times a
    /// second. Anything that wasn't in `previous` is where it is.
    pub fn blended(&self, previous: Option<&Snapshot>, t: f32) -> (Vec<PlayerView>, Vec<EnemyView>) {
        let t = t.clamp(0.0, 1.0);
        let Some(before) = previous else {
            return (self.players.clone(), self.enemies.clone());
        };
        let place = |from: Vec3, to: Vec3| {
            if from.distance(to) > TELEPORT { to } else { from.lerp(to, t) }
        };
        let players = self
            .players
            .iter()
            .map(|now| match before.players.iter().find(|p| p.id == now.id) {
                Some(was) => PlayerView {
                    pos: place(was.pos, now.pos),
                    yaw: lerp_angle(was.yaw, now.yaw, t),
                    pitch: was.pitch + (now.pitch - was.pitch) * t,
                    ..*now
                },
                None => *now,
            })
            .collect();
        let enemies = self
            .enemies
            .iter()
            .map(|now| match before.enemies.iter().find(|e| e.id == now.id) {
                Some(was) => EnemyView { pos: place(was.pos, now.pos), ..*now },
                None => *now,
            })
            .collect();
        (players, enemies)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(x: f32) -> Snapshot {
        Snapshot {
            players: vec![PlayerView { id: PlayerId(1), pos: Vec3::new(x, 0.0, 0.0), yaw: 0.0, pitch: 0.0, hp: 50, cracking: None }],
            enemies: vec![EnemyView {
                id: 7,
                kind: enemy::Kind::Deer,
                pos: Vec3::new(x * 2.0, 0.0, 0.0),
                health: 8,
                hostile: false,
                returning_home: None,
            }],
            weather: weather::Kind::Sunny,
            you: YouView {
                resources: Resources::default(),
                converting: None,
                hp: 50,
                player_roll: None,
                enemy_rolls: Vec::new(),
                mine_timer: 0.0,
                conversion_boost: 1,
            },
        }
    }

    #[test]
    fn blending_goes_part_of_the_way() {
        let (before, now) = (snap(0.0), snap(2.0));
        let (players, enemies) = now.blended(Some(&before), 0.5);
        assert!((players[0].pos.x - 1.0).abs() < 1e-5);
        assert!((enemies[0].pos.x - 2.0).abs() < 1e-5);
        let (players, _) = now.blended(Some(&before), 5.0);
        assert!((players[0].pos.x - 2.0).abs() < 1e-5, "past the end stays at the end");
        let (players, _) = now.blended(None, 0.5);
        assert!((players[0].pos.x - 2.0).abs() < 1e-5, "nothing to blend from");
    }

    #[test]
    fn jumps_and_turns_are_not_smeared() {
        let (before, now) = (snap(0.0), snap(50.0));
        let (players, _) = now.blended(Some(&before), 0.5);
        assert_eq!(players[0].pos.x, 50.0, "a teleport is not a walk");

        let mut a = snap(0.0);
        let mut b = snap(0.0);
        a.players[0].yaw = 3.0;
        b.players[0].yaw = -3.0; // a turn of 0.28 radians through the back, not 6 the long way
        let (players, _) = b.blended(Some(&a), 0.5);
        assert!(players[0].yaw.abs() > 3.0, "{}", players[0].yaw);
    }
}
