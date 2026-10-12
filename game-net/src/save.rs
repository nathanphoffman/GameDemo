//! Saved games on disk: a short header, then the saved game as `postcard` bytes.

use std::fs;
use std::io;
use std::path::Path;

use game_core::game::SaveData;

const MAGIC: &[u8; 4] = b"GDSV";

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
    let Some(body) = bytes.strip_prefix(MAGIC) else {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not a saved game"));
    };
    postcard::from_bytes(body).map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, format!("the save is damaged ({e})"))
    })
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
}
