//! Saved games on disk: a short header, then the saved game as `postcard` bytes.
//!
//! The header says which layout follows, so saves from earlier versions of the game can still
//! be read (and are written back in the new layout).

use std::fs;
use std::io;
use std::path::Path;

use game_core::effects::Fires;
use game_core::game::{SaveData, SavedPlayer, SAVE_VERSION};
use game_core::glam::Vec3;
use game_core::resources::{Converter, Resources};
use game_core::world::BlockChange;
use serde::{Deserialize, Serialize};

/// The current layout.
const MAGIC: &[u8; 4] = b"GDS2";
/// The first layout, from before there was resurrection sickness.
const MAGIC_V1: &[u8; 4] = b"GDSV";

#[derive(Serialize, Deserialize)]
struct SavedPlayerV1 {
    name: String,
    pos: Vec3,
    hp: i32,
    resources: Resources,
    converter: Converter,
}

#[derive(Serialize, Deserialize)]
struct SaveDataV1 {
    version: u32,
    seed: u32,
    blocks: Vec<BlockChange>,
    fires: Fires,
    defeated: Vec<(u32, f32)>,
    players: Vec<SavedPlayerV1>,
}

impl From<SaveDataV1> for SaveData {
    fn from(old: SaveDataV1) -> SaveData {
        SaveData {
            version: SAVE_VERSION,
            seed: old.seed,
            blocks: old.blocks,
            fires: old.fires,
            defeated: old.defeated,
            players: old
                .players
                .into_iter()
                .map(|p| SavedPlayer {
                    name: p.name,
                    pos: p.pos,
                    hp: p.hp,
                    resources: p.resources,
                    converter: p.converter,
                    sickness: 0.0,
                })
                .collect(),
        }
    }
}

/// Writes the save, without ever leaving a half written file where the old one was: it goes to
/// a temporary file next to it first.
pub fn write_save(path: &Path, data: &SaveData) -> io::Result<()> {
    let body = postcard::to_stdvec(data).map_err(io::Error::other)?;
    let mut bytes = Vec::with_capacity(4 + body.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&body);
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir)?;
    }
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    fs::write(&temp, &bytes)?;
    fs::rename(&temp, path)
}

pub fn read_save(path: &Path) -> io::Result<SaveData> {
    let bytes = fs::read(path)?;
    let damaged = |e: postcard::Error| {
        io::Error::new(io::ErrorKind::InvalidData, format!("the save is damaged ({e})"))
    };
    if let Some(body) = bytes.strip_prefix(MAGIC) {
        postcard::from_bytes(body).map_err(damaged)
    } else if let Some(body) = bytes.strip_prefix(MAGIC_V1) {
        postcard::from_bytes::<SaveDataV1>(body).map(SaveData::from).map_err(damaged)
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidData, "not a saved game"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::game::Game;

    #[test]
    fn a_save_round_trips_through_a_file_and_junk_is_an_error() {
        let dir = std::env::temp_dir().join(format!("gamedemo-save-test-{}", std::process::id()));
        let path = dir.join("a.save");
        let mut game = Game::new(3);
        game.join("Ann");
        write_save(&path, &game.save_data()).unwrap();
        let back = read_save(&path).unwrap();
        assert_eq!(back.seed, 3);
        assert_eq!(back.players.len(), 1);
        assert!(!dir.join("a.save.tmp").exists(), "no temporary file left behind");

        fs::write(&path, b"not a save").unwrap();
        assert!(read_save(&path).is_err());
        fs::write(&path, b"GDSVxx").unwrap();
        assert!(read_save(&path).is_err());
        assert!(read_save(&dir.join("missing.save")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn saves_from_the_first_version_are_still_read() {
        let old = SaveDataV1 {
            version: 1,
            seed: 7,
            blocks: Vec::new(),
            fires: Fires::default(),
            defeated: vec![(1, 12.0)],
            players: vec![SavedPlayerV1 {
                name: "Ann".into(),
                pos: Vec3::new(1.0, 2.0, 3.0),
                hp: 33,
                resources: Resources::default(),
                converter: Converter::default(),
            }],
        };
        let mut bytes = MAGIC_V1.to_vec();
        bytes.extend(postcard::to_stdvec(&old).unwrap());
        let dir = std::env::temp_dir().join(format!("gamedemo-old-save-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.save");
        fs::write(&path, bytes).unwrap();
        let data = read_save(&path).unwrap();
        assert_eq!((data.seed, data.players[0].hp, data.players[0].sickness), (7, 33, 0.0));
        assert_eq!(data.defeated, vec![(1, 12.0)]);
        // And restoring it into a game works.
        let mut game = game_core::game::Game::new(7);
        game.restore(data).unwrap();
        let _ = fs::remove_dir_all(&dir);
    }
}
