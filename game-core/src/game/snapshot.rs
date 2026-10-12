//! What one player is told about the game each tick.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use super::{Game, PlayerId};
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
                .map(|p| PlayerView {
                    id: p.id,
                    pos: p.body.pos,
                    yaw: p.body.yaw,
                    pitch: p.body.pitch,
                    hp: p.combat.player_hp,
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
