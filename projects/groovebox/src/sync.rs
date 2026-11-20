//! Del proyecto de la interfaz a la copia del motor.
//!
//! `EngineMirror` guarda lo último que se le mandó al motor y, en cada `sync`, manda solo lo que
//! cambió como comandos chicos (`SetStep`, `SetChainEntry`, `SetSongCell`, `SetMix`...). Así
//! cualquier edición (poner o borrar notas, clonar frases, cambiar la FRA, crear cadenas) llega
//! sin que cada lugar del código que edita tenga que acordarse de avisar. Comparar todo el
//! proyecto cuesta unos 10 000 comparaciones chicas: nada en el hilo de interfaz.
//!
//! Si la cola está llena, lo que falta queda pendiente para la próxima llamada y se manda en
//! orden (los datos antes que Play).

use crate::app::Audition;
use crate::engine::{Command, MAX_NOTES, PlayMode};
use crate::instrument::Instrument;
use crate::model::{
    CHAIN_LEN, ChainEntry, ChainId, MAX_CHAINS, MAX_PHRASES, MAX_SONG_ROWS, PHRASE_STEPS, Project,
    Step, TRACKS,
};

pub struct EngineMirror {
    tempo: Option<f32>,
    instruments: [Option<Instrument>; TRACKS],
    mix: [(bool, bool); TRACKS],
    phrases: Box<[[Step; PHRASE_STEPS]]>,
    chains: Box<[[Option<ChainEntry>; CHAIN_LEN]]>,
    rows: Box<[[Option<ChainId>; TRACKS]]>,
    transport: Option<PlayMode>,
}

impl Default for EngineMirror {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineMirror {
    /// Arranca igual que un `Engine` recién creado: sin datos, sin mute y parado.
    pub fn new() -> Self {
        Self {
            tempo: None,
            instruments: [None; TRACKS],
            mix: [(false, false); TRACKS],
            phrases: vec![[Step::default(); PHRASE_STEPS]; MAX_PHRASES].into_boxed_slice(),
            chains: vec![[None; CHAIN_LEN]; MAX_CHAINS].into_boxed_slice(),
            rows: vec![[None; TRACKS]; MAX_SONG_ROWS].into_boxed_slice(),
            transport: None,
        }
    }

    /// Manda los cambios con `send` (que devuelve `false` si la cola está llena). Devuelve
    /// `true` si quedó todo mandado.
    pub fn sync(
        &mut self,
        project: &Project,
        transport: Option<PlayMode>,
        send: &mut impl FnMut(Command) -> bool,
    ) -> bool {
        let empty = [Step::default(); PHRASE_STEPS];
        for (id, sent) in self.phrases.iter_mut().enumerate() {
            let steps = project.phrases[id].as_ref().map_or(&empty, |p| &p.steps);
            for (i, (s, &value)) in sent.iter_mut().zip(steps).enumerate() {
                if *s != value {
                    let cmd = Command::SetStep {
                        phrase: id as u8,
                        step: i as u8,
                        value,
                    };
                    if !send(cmd) {
                        return false;
                    }
                    *s = value;
                }
            }
        }
        let no_entries = [None; CHAIN_LEN];
        for (id, sent) in self.chains.iter_mut().enumerate() {
            let entries = project.chains[id]
                .as_ref()
                .map_or(&no_entries, |c| &c.entries);
            for (pos, (e, &value)) in sent.iter_mut().zip(entries).enumerate() {
                if *e != value {
                    let cmd = Command::SetChainEntry {
                        chain: id as u8,
                        pos: pos as u8,
                        value,
                    };
                    if !send(cmd) {
                        return false;
                    }
                    *e = value;
                }
            }
        }
        for (row, sent) in self.rows.iter_mut().enumerate() {
            let cells = project
                .song
                .rows
                .get(row)
                .copied()
                .unwrap_or([None; TRACKS]);
            for (track, (c, value)) in sent.iter_mut().zip(cells).enumerate() {
                if *c != value {
                    let cmd = Command::SetSongCell {
                        row: row as u8,
                        track: track as u8,
                        chain: value,
                    };
                    if !send(cmd) {
                        return false;
                    }
                    *c = value;
                }
            }
        }
        for (t, (sent, track)) in self.mix.iter_mut().zip(&project.tracks).enumerate() {
            let value = (track.mix.mute, track.mix.solo);
            if *sent != value {
                let cmd = Command::SetMix {
                    track: t as u8,
                    mute: value.0,
                    solo: value.1,
                };
                if !send(cmd) {
                    return false;
                }
                *sent = value;
            }
        }
        if self.tempo != Some(project.tempo_bpm) {
            if !send(Command::SetTempo(project.tempo_bpm)) {
                return false;
            }
            self.tempo = Some(project.tempo_bpm);
        }
        for (t, (sent, track)) in self.instruments.iter_mut().zip(&project.tracks).enumerate() {
            if *sent != Some(track.instrument) {
                let instrument = track.instrument;
                let cmd = Command::SetInstrument {
                    track: t as u8,
                    instrument,
                };
                if !send(cmd) {
                    return false;
                }
                *sent = Some(instrument);
            }
        }
        if self.transport != transport {
            let cmd = match transport {
                Some(mode) => Command::Play(mode),
                None => Command::Stop,
            };
            if !send(cmd) {
                return false;
            }
            self.transport = transport;
        }
        true
    }
}

pub fn audition_command(a: Audition) -> Command {
    let mut notes = [0u8; MAX_NOTES];
    let slice = a.notes.as_slice();
    notes[..slice.len()].copy_from_slice(slice);
    Command::Audition {
        track: a.track as u8,
        notes,
        len: slice.len() as u8,
        velocity: a.velocity,
        gate_ticks: a.gate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use crate::model::{NoteCell, Section};

    /// La copia del motor coincide con el proyecto en todas las secciones de la fila 0 y 1.
    fn assert_same(engine: &Engine, project: &Project) {
        let data = engine.song_data();
        for row in 0..2 {
            for entry in 0..CHAIN_LEN {
                let sec = Section { row, entry };
                for t in 0..TRACKS {
                    let id = project.phrase_id_at(t, sec);
                    let got = data.entry_at(t, row, entry);
                    assert_eq!(got.map(|e| e.phrase), id, "pista {t} en {sec:?}");
                    if let Some(id) = id {
                        let phrase = project.phrase(id).unwrap();
                        for (i, s) in phrase.steps.iter().enumerate() {
                            assert_eq!(data.step(id, i), s, "frase {id} paso {i}");
                        }
                    }
                }
            }
        }
    }

    fn apply(engine: &mut Engine, mirror: &mut EngineMirror, project: &Project) -> usize {
        let mut sent = 0;
        let done = mirror.sync(project, None, &mut |cmd| {
            engine.handle(cmd);
            sent += 1;
            true
        });
        assert!(done);
        sent
    }

    #[test]
    fn mirror_sends_only_changes_and_keeps_the_engine_in_sync() {
        let mut project = crate::demo::project();
        let mut engine = Engine::new(48_000.0);
        let mut mirror = EngineMirror::new();
        assert!(apply(&mut engine, &mut mirror, &project) > 50);
        assert_same(&engine, &project);
        assert_eq!(
            apply(&mut engine, &mut mirror, &project),
            0,
            "sin cambios, nada"
        );

        let sec = Section { row: 0, entry: 1 };
        project.step_mut(6, sec, 5).unwrap().note = NoteCell::On(40);
        assert_eq!(apply(&mut engine, &mut mirror, &project), 1);
        let id = project.phrase_id_at(0, sec).unwrap();
        let copy = project.clone_phrase(id).unwrap();
        project.set_entry_phrase(0, sec, copy).unwrap();
        assert!(project.remove_entry(7, Section { row: 0, entry: 2 }));
        project
            .ensure_phrase(3, Section { row: 1, entry: 0 }, None)
            .unwrap();
        apply(&mut engine, &mut mirror, &project);
        assert_same(&engine, &project);
    }

    #[test]
    fn a_full_queue_leaves_the_rest_for_later_in_order() {
        let project = crate::demo::project();
        let mut engine = Engine::new(48_000.0);
        let mut mirror = EngineMirror::new();
        let transport = Some(PlayMode::Section { row: 0, entry: 0 });
        let mut rounds = 0;
        let mut saw_play_before_data_done = false;
        loop {
            rounds += 1;
            let mut room = 7;
            let mut sent = Vec::new();
            let done = mirror.sync(&project, transport, &mut |cmd| {
                if room == 0 {
                    return false;
                }
                room -= 1;
                sent.push(cmd);
                true
            });
            for cmd in sent {
                if matches!(cmd, Command::Play(_)) && !done {
                    saw_play_before_data_done = true;
                }
                engine.handle(cmd);
            }
            if done {
                break;
            }
        }
        assert!(rounds > 10);
        assert!(!saw_play_before_data_done, "Play va después de los datos");
        assert_same(&engine, &project);
    }
}
