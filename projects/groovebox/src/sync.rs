//! Del proyecto de la interfaz a la copia del motor.
//!
//! `EngineMirror` guarda lo último que se le mandó al motor y, en cada `sync`, manda solo lo que
//! cambió como comandos chicos (`SetStep`, `SetChainEntry`, `SetSongCell`, `SetMix`, `SetFx`...). Así
//! cualquier edición (poner o borrar notas, clonar frases, cambiar la FRA, crear cadenas) llega
//! sin que cada lugar del código que edita tenga que acordarse de avisar. Comparar todo el
//! proyecto cuesta unos 10 000 comparaciones chicas: nada en el hilo de interfaz.
//!
//! Si la cola está llena, lo que falta queda pendiente para la próxima llamada y se manda en
//! orden (los datos antes que Play).

use crate::app::Audition;
use crate::engine::{Command, MAX_NOTES, PlayMode};
use crate::instrument::Instrument;
use crate::mix::{FxSettings, MasterSettings};
use crate::model::{
    CHAIN_LEN, ChainEntry, ChainId, ChannelStrip, MAX_CHAINS, MAX_PHRASES, MAX_SONG_ROWS,
    PHRASE_STEPS, Project, Step, TRACKS,
};

pub struct EngineMirror {
    tempo: Option<f32>,
    swing: Option<u8>,
    instruments: [Option<Instrument>; TRACKS],
    mix: [ChannelStrip; TRACKS],
    fx: Option<FxSettings>,
    master: Option<MasterSettings>,
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
            swing: None,
            instruments: [None; TRACKS],
            mix: [ChannelStrip::default(); TRACKS],
            fx: None,
            master: None,
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
            if *sent != track.mix {
                let cmd = Command::SetMix {
                    track: t as u8,
                    strip: track.mix,
                };
                if !send(cmd) {
                    return false;
                }
                *sent = track.mix;
            }
        }
        if self.fx != Some(project.fx) {
            if !send(Command::SetFx(project.fx)) {
                return false;
            }
            self.fx = Some(project.fx);
        }
        if self.master != Some(project.master) {
            if !send(Command::SetMaster(project.master)) {
                return false;
            }
            self.master = Some(project.master);
        }
        if self.tempo != Some(project.tempo_bpm) {
            if !send(Command::SetTempo(project.tempo_bpm)) {
                return false;
            }
            self.tempo = Some(project.tempo_bpm);
        }
        if self.swing != Some(project.swing) {
            if !send(Command::SetSwing(project.swing)) {
                return false;
            }
            self.swing = Some(project.swing);
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

/// Sticks → motor (spec-ui: "Sticks = performance"). Mueven la pista actual; las demás quedan
/// en 0 (al cambiar de pista con el stick movido, la anterior vuelve a su valor guardado). Solo
/// manda lo que cambió, cuantizado a 1/256 para no llenar la cola con el ruido del eje.
#[derive(Default)]
pub struct PerformSender {
    sent: [[i16; 4]; TRACKS],
}

impl PerformSender {
    /// `left` y `right` son `(x, y)` de cada stick en −1..=1, con la zona muerta aplicada.
    /// Devuelve `false` si la cola se llenó (lo que falta se manda en la próxima llamada).
    pub fn update(
        &mut self,
        track: usize,
        left: (f32, f32),
        right: (f32, f32),
        send: &mut impl FnMut(Command) -> bool,
    ) -> bool {
        let q = |v: f32| (v.clamp(-1.0, 1.0) * 256.0).round() as i16;
        let current = [q(left.0), q(left.1), q(right.0), q(right.1)];
        for (t, sent) in self.sent.iter_mut().enumerate() {
            let want = if t == track { current } else { [0; 4] };
            if *sent != want {
                let cmd = Command::Perform {
                    track: t as u8,
                    offsets: want.map(|v| f32::from(v) / 256.0),
                };
                if !send(cmd) {
                    return false;
                }
                *sent = want;
            }
        }
        true
    }
}

pub fn audition_command(a: Audition) -> Command {
    if let Some(p) = a.phrase {
        return Command::PreviewPhrase {
            track: a.track as u8,
            phrase: p.phrase,
            transpose: p.transpose,
        };
    }
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
    fn tempo_and_swing_are_sent_once_per_change() {
        let mut project = Project::new_default();
        let mut mirror = EngineMirror::new();
        let mut sent = Vec::new();
        let mut collect = |p: &Project, sent: &mut Vec<Command>| {
            sent.clear();
            assert!(mirror.sync(p, None, &mut |cmd| {
                sent.push(cmd);
                true
            }));
        };
        collect(&project, &mut sent);
        assert!(sent.contains(&Command::SetSwing(50)));
        project.swing = 66;
        collect(&project, &mut sent);
        assert_eq!(sent, [Command::SetSwing(66)]);
        collect(&project, &mut sent);
        assert!(sent.is_empty());
    }

    #[test]
    fn mix_fx_and_master_are_sent_once_per_change() {
        let mut project = Project::new_default();
        let mut mirror = EngineMirror::new();
        let mut sent = Vec::new();
        let mut collect = |p: &Project, sent: &mut Vec<Command>| {
            sent.clear();
            assert!(mirror.sync(p, None, &mut |cmd| {
                sent.push(cmd);
                true
            }));
        };
        collect(&project, &mut sent);
        assert!(sent.contains(&Command::SetFx(FxSettings::default())));
        assert!(sent.contains(&Command::SetMaster(MasterSettings::default())));
        project.tracks[3].mix.pan = -20;
        project.fx.reverb.size = 90;
        project.master.limiter = false;
        collect(&project, &mut sent);
        assert_eq!(
            sent,
            [
                Command::SetMix {
                    track: 3,
                    strip: project.tracks[3].mix
                },
                Command::SetFx(project.fx),
                Command::SetMaster(project.master),
            ]
        );
        collect(&project, &mut sent);
        assert!(sent.is_empty());
    }

    #[test]
    fn sticks_move_only_the_current_track_and_return_to_zero() {
        let mut perf = PerformSender::default();
        let mut sent = Vec::new();
        let mut run = |track, left, right, sent: &mut Vec<Command>| {
            sent.clear();
            assert!(perf.update(track, left, right, &mut |c| {
                sent.push(c);
                true
            }));
        };
        run(2, (0.0, 0.0), (0.0, 0.0), &mut sent);
        assert!(sent.is_empty(), "en reposo no manda nada");
        run(2, (-0.5, 0.25), (0.0, 1.0), &mut sent);
        assert_eq!(
            sent,
            [Command::Perform {
                track: 2,
                offsets: [-0.5, 0.25, 0.0, 1.0]
            }]
        );
        run(2, (-0.5001, 0.25), (0.0, 1.0), &mut sent);
        assert!(sent.is_empty(), "cambios por debajo de 1/256 no se mandan");
        // Cambiar de pista con el stick movido: la anterior vuelve a 0.
        run(3, (-0.5, 0.25), (0.0, 1.0), &mut sent);
        assert_eq!(sent.len(), 2);
        assert!(sent.contains(&Command::Perform {
            track: 2,
            offsets: [0.0; 4]
        }));
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
