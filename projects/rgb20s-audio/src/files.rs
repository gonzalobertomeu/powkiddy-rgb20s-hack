use std::fs;
use std::io::{self, Write};
use std::path::Path;

use rgb20s_common::audio_state::AudioState;

use crate::mode::Mode;

/// Escribe en un temporal del mismo directorio y renombra: los lectores (alsa-lib en cada app)
/// ven el archivo viejo o el nuevo, nunca uno a medias.
pub fn write_atomic(path: &Path, contents: &str) -> io::Result<()> {
    if fs::read_to_string(path).is_ok_and(|old| old == contents) {
        return Ok(());
    }
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let tmp = dir.join(format!(".{name}.tmp"));
    let mut file = fs::File::create(&tmp)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, path)
}

pub fn load_mode(path: &Path) -> io::Result<Option<Mode>> {
    match fs::read_to_string(path) {
        Ok(text) => text
            .parse()
            .map(Some)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn save_mode(path: &Path, mode: Mode) -> io::Result<()> {
    write_atomic(path, &format!("{mode}\n"))
}

/// Estado para las apps (`/run/rgb20s-audio/state`). Siempre cambia (`seq` sube).
pub fn save_state(path: &Path, state: &AudioState) -> io::Result<()> {
    write_atomic(path, &state.render())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_round_trip() {
        let dir = std::env::temp_dir().join(format!("rgb20s-audio-state-{}", std::process::id()));
        let path = dir.join("sub/mode");
        assert_eq!(load_mode(&path).unwrap(), None);
        save_mode(&path, Mode::Stereo).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "stereo\n");
        assert_eq!(load_mode(&path).unwrap(), Some(Mode::Stereo));
        save_mode(&path, Mode::Mono).unwrap();
        assert_eq!(load_mode(&path).unwrap(), Some(Mode::Mono));
        fs::write(&path, "x").unwrap();
        assert!(load_mode(&path).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn state_is_readable_by_the_apps() {
        let dir = std::env::temp_dir().join(format!("rgb20s-audio-run-{}", std::process::id()));
        let path = dir.join("state");
        let state = AudioState {
            volume_pct: 66,
            volume_db: Some(-20.1),
            mode: Mode::Stereo,
            seq: 7,
        };
        save_state(&path, &state).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(AudioState::parse(&text).unwrap(), state);
        assert!(!dir.join(".state.tmp").exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
