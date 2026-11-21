//! Sigue el archivo de estado del audio desde el bucle de una app, sin hilos: cada
//! [`PRESENT_POLL_MS`] hace un `stat` y solo si cambió (inodo, tamaño o mtime; el servicio
//! renombra un archivo nuevo en cada escritura) lo lee y lo parsea.
//!
//! Si el archivo no existe (servicio sin instalar, simulador), mira cada [`ABSENT_POLL_MS`] y
//! no informa nada: la app simplemente no muestra el volumen.

use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::audio_state::{self, AudioState};

pub const PRESENT_POLL_MS: u64 = 100;
pub const ABSENT_POLL_MS: u64 = 1_000;

/// Qué cambió desde la última vez.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Update {
    /// Primera lectura válida (al arrancar la app o cuando aparece el servicio): no es un
    /// cambio que haya hecho el usuario, no se resalta.
    Appeared,
    /// `volume`: una pulsación de volumen (aunque esté en un extremo y el valor no cambie).
    Changed { volume: bool, mode: bool },
    /// El archivo desapareció (se detuvo el servicio).
    Gone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stamp {
    dev: u64,
    ino: u64,
    len: u64,
    mtime: i64,
    mtime_nsec: i64,
}

impl Stamp {
    fn of(meta: &fs::Metadata) -> Self {
        Self {
            dev: meta.dev(),
            ino: meta.ino(),
            len: meta.len(),
            mtime: meta.mtime(),
            mtime_nsec: meta.mtime_nsec(),
        }
    }
}

pub struct StateWatcher {
    path: PathBuf,
    stamp: Option<Stamp>,
    state: Option<AudioState>,
    next_poll: u64,
}

impl StateWatcher {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            stamp: None,
            state: None,
            next_poll: 0,
        }
    }

    /// Con la ruta de [`audio_state::state_path`] (`$RGB20S_AUDIO_STATE` o la del servicio).
    pub fn from_env() -> Self {
        Self::new(audio_state::state_path())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Último estado válido (None si no hay servicio).
    pub fn state(&self) -> Option<AudioState> {
        self.state
    }

    /// Momento (en el mismo reloj que `poll`) del próximo `stat`: para calcular la espera del bucle.
    pub fn next_poll(&self) -> u64 {
        self.next_poll
    }

    /// Llamar en cada vuelta del bucle con el reloj de la app en ms; antes de `next_poll` no hace
    /// nada (ni siquiera el `stat`).
    pub fn poll(&mut self, now_ms: u64) -> Option<Update> {
        if now_ms < self.next_poll {
            return None;
        }
        let (update, present) = self.check();
        let wait = if present {
            PRESENT_POLL_MS
        } else {
            ABSENT_POLL_MS
        };
        self.next_poll = now_ms + wait;
        update
    }

    /// Devuelve el cambio y si el archivo existe.
    fn check(&mut self) -> (Option<Update>, bool) {
        let meta = match fs::metadata(&self.path) {
            Ok(meta) => meta,
            Err(_) => {
                self.stamp = None;
                return (self.state.take().map(|_| Update::Gone), false);
            }
        };
        let stamp = Stamp::of(&meta);
        if self.stamp == Some(stamp) {
            return (None, true);
        }
        // Un estado ilegible se ignora hasta la próxima escritura (no se relee en cada vuelta).
        self.stamp = Some(stamp);
        let fresh = match read(&self.path) {
            Ok(state) => state,
            Err(_) => return (None, true),
        };
        let update = match self.state.replace(fresh) {
            None => Some(Update::Appeared),
            Some(old) => {
                let mode = old.mode != fresh.mode;
                let volume = old.volume_pct != fresh.volume_pct || (old.seq != fresh.seq && !mode);
                (volume || mode).then_some(Update::Changed { volume, mode })
            }
        };
        (update, true)
    }
}

fn read(path: &Path) -> io::Result<AudioState> {
    let text = fs::read_to_string(path)?;
    AudioState::parse(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_state::Mode;

    fn write(path: &Path, pct: u8, mode: Mode, seq: u64) {
        let state = AudioState {
            volume_pct: pct,
            volume_db: None,
            mode,
            seq,
        };
        // Como el servicio: archivo nuevo y renombrado (cambia el inodo aunque el mtime no).
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, state.render()).unwrap();
        fs::rename(&tmp, path).unwrap();
    }

    #[test]
    fn reports_appear_changes_and_gone() {
        let dir = std::env::temp_dir().join(format!("rgb20s-watch-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state");
        let mut w = StateWatcher::new(&path);

        // Sin archivo: nada, y se vuelve a mirar en 1 s.
        assert_eq!(w.poll(0), None);
        assert_eq!(w.next_poll(), ABSENT_POLL_MS);
        assert_eq!(w.state(), None);

        write(&path, 50, Mode::Mono, 1);
        assert_eq!(w.poll(500), None, "todavía no toca mirar");
        assert_eq!(w.poll(1_000), Some(Update::Appeared));
        assert_eq!(w.state().unwrap().volume_pct, 50);
        assert_eq!(w.next_poll(), 1_000 + PRESENT_POLL_MS);
        assert_eq!(w.poll(1_100), None, "sin cambios");

        write(&path, 55, Mode::Mono, 2);
        assert_eq!(
            w.poll(1_200),
            Some(Update::Changed {
                volume: true,
                mode: false
            })
        );
        // Pulsación en el extremo: mismo volumen, seq nuevo.
        write(&path, 55, Mode::Mono, 3);
        assert_eq!(
            w.poll(1_300),
            Some(Update::Changed {
                volume: true,
                mode: false
            })
        );
        write(&path, 55, Mode::Stereo, 4);
        assert_eq!(
            w.poll(1_400),
            Some(Update::Changed {
                volume: false,
                mode: true
            })
        );
        // Reescritura idéntica: no es un cambio.
        write(&path, 55, Mode::Stereo, 4);
        assert_eq!(w.poll(1_500), None);

        // Ilegible: se ignora y queda el último estado.
        fs::write(&path, "basura").unwrap();
        assert_eq!(w.poll(1_600), None);
        assert_eq!(w.state().unwrap().mode, Mode::Stereo);

        fs::remove_file(&path).unwrap();
        assert_eq!(w.poll(1_700), Some(Update::Gone));
        assert_eq!(w.state(), None);
        assert_eq!(w.poll(2_700), None);

        write(&path, 10, Mode::Mono, 1);
        assert_eq!(w.poll(3_700), Some(Update::Appeared));
        fs::remove_dir_all(&dir).unwrap();
    }
}
