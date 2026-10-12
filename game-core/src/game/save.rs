//! What is kept when a game is saved, and putting it back.
//!
//! The world itself comes from the seed, so only what is different from the generated world is
//! saved: the changed blocks, the fires, the creatures still waiting to come back, and each
//! player (by name) with their things and where they stood. Creatures themselves are not saved:
//! they are always found at home, unhurt, when someone comes near.

use serde::{Deserialize, Serialize};

use super::*;
use crate::resources::Converter;
use crate::world::BlockChange;

/// Bumped when what is saved changes in a way old files can't be read.
pub const SAVE_VERSION: u32 = 2;

/// One player's things.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedPlayer {
    pub name: String,
    /// Where their feet were.
    pub pos: Vec3,
    pub hp: i32,
    pub resources: Resources,
    pub converter: Converter,
    /// Seconds of resurrection sickness left.
    pub sickness: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveData {
    pub version: u32,
    pub seed: u32,
    pub blocks: Vec<BlockChange>,
    pub fires: Fires,
    /// Creature homes that are empty, and the seconds until each is back.
    pub defeated: Vec<(u32, f32)>,
    /// Everyone: those playing now, and those who left earlier.
    pub players: Vec<SavedPlayer>,
}

impl Game {
    /// Everything needed to carry on from here later (see `restore`).
    pub fn save_data(&self) -> SaveData {
        let kept = |p: &PlayerState| SavedPlayer {
            name: p.name.clone(),
            pos: p.body.pos,
            hp: p.combat.player_hp,
            resources: p.resources,
            converter: p.converter.clone(),
            sickness: p.sickness,
        };
        let mut players: Vec<SavedPlayer> = self.players.iter().map(kept).collect();
        for gone in &self.away {
            if !players.iter().any(|p| p.name.eq_ignore_ascii_case(&gone.name)) {
                players.push(kept(gone));
            }
        }
        SaveData {
            version: SAVE_VERSION,
            seed: self.seed,
            blocks: self.world.all_edits(),
            fires: self.fires.clone(),
            defeated: self.defeated.iter().map(|(&id, &left)| (id, left)).collect(),
            players,
        }
    }

    /// Carries on from a saved game: call on a new `Game` made with the saved seed, before
    /// anyone joins. Players come back when they join under their name (see `join`).
    pub fn restore(&mut self, data: SaveData) -> Result<(), String> {
        if data.version != SAVE_VERSION {
            return Err(format!(
                "the save is from another version of the game ({} instead of {SAVE_VERSION})",
                data.version
            ));
        }
        if data.seed != self.seed {
            return Err("the save is from a different world".to_string());
        }
        self.world.load_edits(&data.blocks);
        self.fires = data.fires;
        self.defeated = data.defeated.into_iter().collect();
        self.away = data
            .players
            .into_iter()
            .map(|saved| {
                let mut state = PlayerState::new(PlayerId(0), saved.name, saved.pos);
                state.resources = saved.resources;
                state.converter = saved.converter;
                state.combat.player_hp = saved.hp;
                state.sickness = saved.sickness;
                state
            })
            .collect();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(game: &mut Game, seconds: f32) {
        for _ in 0..(seconds * 30.0) as u32 {
            game.update(1.0 / 30.0);
        }
    }

    #[test]
    fn a_saved_game_comes_back_the_same() {
        let mut game = Game::new(3);
        let (ann, _) = game.join("Ann");
        run(&mut game, 1.0);
        let dig = PlayerInput { mining: true, pitch: -1.4, ..PlayerInput::default() };
        game.apply(ann, Command::SetInput(dig));
        run(&mut game, 2.0);
        game.apply(ann, Command::SetInput(PlayerInput::default()));
        game.apply(ann, Command::ToggleConversion(2));
        // A creature is defeated, and Bob came and went.
        run(&mut game, 0.5);
        let deer = game.enemies.iter().position(|e| e.id == 1).unwrap();
        game.enemies[deer].health = 0;
        run(&mut game, 0.2);
        let (bob, _) = game.join("Bob");
        game.remove_player(bob);

        let data = game.save_data();
        assert_eq!(data.players.len(), 2, "Ann is playing, Bob is away");
        assert!(!data.blocks.is_empty());
        assert!(data.defeated.iter().any(|(id, _)| *id == 1));

        // A new game from the same seed, with the save put back.
        let mut again = Game::new(3);
        again.restore(data).unwrap();
        // The world is the same where it was dug.
        let mut was = game.world.all_edits();
        let mut now = again.world.all_edits();
        let key = |c: &BlockChange| c.pos;
        was.sort_by_key(key);
        now.sort_by_key(key);
        assert_eq!(was, now);
        // The deer is still defeated, so it doesn't turn up next to a player.
        let (ann2, back) = again.join("ann");
        assert!(back);
        run(&mut again, 1.0);
        assert!(again.enemies.iter().all(|e| e.id != 1));
        let (before, after) = (game.player(ann).unwrap(), again.player(ann2).unwrap());
        assert_eq!(before.resources, after.resources);
        assert_eq!(after.converter.selected(), Some(2));
        let (bob2, back) = again.join("Bob");
        assert!(back);
        assert_eq!(again.player(bob2).unwrap().resources, game.away[0].resources);
    }

    #[test]
    fn a_save_from_another_world_or_version_is_refused() {
        let mut game = Game::new(3);
        let mut data = game.save_data();
        assert!(Game::new(4).restore(data.clone()).is_err());
        data.version += 1;
        assert!(game.restore(data).is_err());
    }
}
