//! Tests del secuenciador: tiempos exactos en muestras, loop sin deriva, gate, INF, edición en
//! vivo, mute/solo, cabezales y (H5) cadenas, canción, regla de bloques y modos de reproducción.

use super::*;
use crate::model::{Chord, ChordKind, NoteCell, PHRASE_STEPS};

const SR: f32 = 48_000.0;
const SEC0: PlayMode = PlayMode::Section { row: 0, entry: 0 };

/// Motor con las 8 pistas en la sección (0, 0) como el proyecto nuevo: cadena t → frase t.
fn engine(bpm: f32) -> Engine {
    let mut e = Engine::new(SR);
    for t in 0..TRACKS as u8 {
        e.handle(Command::SetSongCell {
            row: 0,
            track: t,
            chain: Some(t),
        });
        e.handle(Command::SetChainEntry {
            chain: t,
            pos: 0,
            value: Some(ChainEntry {
                phrase: t,
                transpose: 0,
            }),
        });
    }
    e.handle(Command::SetTempo(bpm));
    e
}

/// Pistas sin sonido: solo interesa cuándo dispara el secuenciador (y el render es barato).
fn silence_all(e: &mut Engine) {
    for t in 0..TRACKS as u8 {
        e.handle(Command::SetInstrument {
            track: t,
            instrument: Instrument::new(Patch::Sampler, 2),
        });
    }
}

fn note(n: u8, gate_ticks: u16) -> Step {
    Step {
        note: NoteCell::On(n),
        gate_ticks,
        ..Step::default()
    }
}

fn set(e: &mut Engine, phrase: u8, step: usize, value: Step) {
    e.handle(Command::SetStep {
        phrase,
        step: step as u8,
        value,
    });
}

fn render(e: &mut Engine, frames: usize, block: usize) {
    let mut buf = vec![0.0f32; block * 2];
    let mut left = frames;
    while left > 0 {
        let n = left.min(block);
        e.render(&mut buf[..n * 2], 2);
        left -= n;
    }
}

fn frames_of(e: &Engine, track: u8) -> Vec<u64> {
    e.trigger_log
        .iter()
        .filter(|(_, t, _)| *t == track)
        .map(|(f, _, _)| *f)
        .collect()
}

fn sps(bpm: f32) -> f64 {
    sequencer::samples_per_step(SR, bpm)
}

fn track_states(e: &Engine, track: u8) -> Vec<VoiceState> {
    e.pool()
        .voices()
        .filter(|v| v.track == track)
        .map(|v| v.state)
        .collect()
}

#[test]
fn steps_fire_on_the_exact_sample_for_any_tempo_and_buffer_size() {
    for bpm in [120.0, 125.0, 133.0, 97.3, 300.0, 40.0] {
        for block in [1, 7, 32, 64, 256, 1000] {
            // Con buffers de 1 frame se prueba menos tiempo (es lento en debug).
            let steps = if block == 1 { 6 } else { 33 };
            let mut e = engine(bpm);
            silence_all(&mut e);
            for i in 0..PHRASE_STEPS {
                set(&mut e, 0, i, note(60, 24));
            }
            e.handle(Command::Play(SEC0));
            let frames = (sps(bpm) * steps as f64) as usize;
            render(&mut e, frames, block);
            let expected: Vec<u64> = (0..steps)
                .map(|k| (k as f64 * sps(bpm)).round() as u64)
                .filter(|&f| f < frames as u64)
                .collect();
            assert_eq!(frames_of(&e, 0), expected, "{bpm} BPM, bloques de {block}");
        }
    }
}

#[test]
fn looping_does_not_drift() {
    let loops = 50;
    for bpm in [120.0, 133.0] {
        let mut e = engine(bpm);
        silence_all(&mut e);
        set(&mut e, 0, 0, note(60, 24));
        e.handle(Command::Play(SEC0));
        let loop_len = sps(bpm) * PHRASE_STEPS as f64;
        render(&mut e, (loop_len * loops as f64) as usize, 256);
        let got = frames_of(&e, 0);
        assert_eq!(got.len(), loops);
        for (n, &f) in got.iter().enumerate() {
            assert_eq!(
                f,
                (n as f64 * loop_len).round() as u64,
                "vuelta {n} a {bpm} BPM"
            );
        }
        if bpm == 120.0 {
            assert_eq!(got[loops - 1], (loops as u64 - 1) * 96_000);
        }
    }
}

#[test]
fn tempo_change_keeps_the_phase_of_the_current_step() {
    let mut e = engine(120.0);
    silence_all(&mut e);
    for i in 0..PHRASE_STEPS {
        set(&mut e, 0, i, note(60, 24));
    }
    e.handle(Command::Play(SEC0));
    render(&mut e, 3_000, 256); // mitad del paso 1 (6000 muestras)
    e.handle(Command::SetTempo(240.0));
    render(&mut e, 10_000, 256);
    // Faltaba medio paso: al doble de tempo son 1500 muestras; después, un paso cada 3000.
    assert_eq!(frames_of(&e, 0), [0, 4_500, 7_500, 10_500]);
}

#[test]
fn gate_releases_on_the_exact_sample() {
    let mut e = engine(120.0);
    // Paso 3 (muestra 18000), acorde de 3 notas, largo 2 pasos = 12000 muestras.
    let chord = Step {
        chord: Chord {
            kind: ChordKind::Major,
            inversion: 0,
        },
        ..note(60, 48)
    };
    set(&mut e, 7, 3, chord);
    e.handle(Command::Play(SEC0));
    render(&mut e, 18_000 + 12_000, 100);
    assert_eq!(track_states(&e, 7), [VoiceState::Held; 3]);
    render(&mut e, 1, 1);
    assert_eq!(track_states(&e, 7), [VoiceState::Released; 3]);
}

#[test]
fn inf_lasts_until_the_next_note_of_the_track() {
    let mut e = engine(120.0);
    set(&mut e, 7, 0, note(60, GATE_INF));
    set(&mut e, 7, 4, note(67, 24));
    e.handle(Command::Play(SEC0));
    render(&mut e, 24_000, 256);
    assert_eq!(track_states(&e, 7), [VoiceState::Held], "INF sigue sonando");
    render(&mut e, 1, 1);
    let mut states: Vec<_> = e
        .pool()
        .voices()
        .filter(|v| v.track == 7)
        .map(|v| (v.note, v.state))
        .collect();
    states.sort_by_key(|(n, _)| *n);
    assert_eq!(
        states,
        [(60, VoiceState::Released), (67, VoiceState::Held)],
        "la nota siguiente suelta la INF"
    );
}

#[test]
fn drums_play_the_root_and_chords_play_all_notes() {
    let mut e = engine(120.0);
    let chord = Step {
        chord: Chord {
            kind: ChordKind::Min7,
            inversion: 0,
        },
        ..note(57, 24)
    };
    set(&mut e, 0, 0, chord);
    set(&mut e, 7, 0, chord);
    e.handle(Command::Play(SEC0));
    render(&mut e, 64, 64);
    assert_eq!(e.pool().voices().filter(|v| v.track == 0).count(), 1);
    let mut notes: Vec<u8> = e
        .pool()
        .voices()
        .filter(|v| v.track == 7)
        .map(|v| v.note)
        .collect();
    notes.sort();
    assert_eq!(notes, [57, 60, 64, 67]);
}

#[test]
fn chain_transpose_is_applied() {
    let mut e = engine(120.0);
    e.handle(Command::SetChainEntry {
        chain: 6,
        pos: 0,
        value: Some(ChainEntry {
            phrase: 6,
            transpose: 5,
        }),
    });
    set(&mut e, 6, 0, note(36, 24));
    e.handle(Command::Play(SEC0));
    render(&mut e, 64, 64);
    assert_eq!(e.pool().voices().map(|v| v.note).collect::<Vec<_>>(), [41]);
}

#[test]
fn live_edits_apply_ahead_now_and_behind_on_the_next_loop() {
    let mut e = engine(120.0);
    silence_all(&mut e);
    set(&mut e, 0, 0, note(60, 24));
    e.handle(Command::Play(SEC0));
    render(&mut e, 5 * 6_000 + 10, 256); // pasó el paso 6
    set(&mut e, 0, 8, note(60, 24)); // adelante: suena en esta vuelta
    set(&mut e, 0, 2, note(60, 24)); // atrás: en la próxima
    set(&mut e, 0, 0, Step::default()); // borrado: no suena más
    render(&mut e, 2 * 96_000 - (5 * 6_000 + 10), 256);
    assert_eq!(
        frames_of(&e, 0),
        [0, 48_000, 96_000 + 12_000, 96_000 + 48_000]
    );

    // Cambiar la frase de la entrada (FRA) también se oye en el paso siguiente.
    set(&mut e, 9, 1, note(60, 24));
    e.handle(Command::SetChainEntry {
        chain: 0,
        pos: 0,
        value: Some(ChainEntry {
            phrase: 9,
            transpose: 0,
        }),
    });
    e.trigger_log.clear();
    render(&mut e, 96_000, 256);
    assert_eq!(frames_of(&e, 0), [2 * 96_000 + 6_000]);
}

#[test]
fn mute_and_solo_silence_tracks_in_playback() {
    let mut e = engine(120.0);
    silence_all(&mut e);
    for t in 0..3 {
        for i in 0..4 {
            set(&mut e, t, i * 4, note(60, 24));
        }
    }
    e.handle(Command::Play(SEC0));
    let mix = |e: &mut Engine, track, mute, solo| e.handle(Command::SetMix { track, mute, solo });

    mix(&mut e, 0, true, false);
    render(&mut e, 96_000, 256);
    assert!(frames_of(&e, 0).is_empty());
    assert_eq!(frames_of(&e, 1).len(), 4);

    e.trigger_log.clear();
    mix(&mut e, 0, false, false);
    mix(&mut e, 2, false, true);
    render(&mut e, 96_000, 256);
    assert!(frames_of(&e, 0).is_empty() && frames_of(&e, 1).is_empty());
    assert_eq!(frames_of(&e, 2).len(), 4, "solo: solo la pista 3");

    e.trigger_log.clear();
    mix(&mut e, 2, true, true);
    render(&mut e, 96_000, 256);
    assert!(
        e.trigger_log.is_empty(),
        "mute gana sobre solo; las demás siguen fuera"
    );
}

#[test]
fn muting_releases_held_notes() {
    let mut e = engine(120.0);
    set(&mut e, 7, 0, note(60, GATE_INF));
    e.handle(Command::Play(SEC0));
    render(&mut e, 256, 256);
    assert_eq!(track_states(&e, 7), [VoiceState::Held]);
    e.handle(Command::SetMix {
        track: 7,
        mute: true,
        solo: false,
    });
    assert_eq!(track_states(&e, 7), [VoiceState::Released]);
}

#[test]
fn stop_releases_everything_and_publishes_the_playhead() {
    let mut e = engine(120.0);
    set(&mut e, 7, 0, note(60, GATE_INF));
    set(&mut e, 6, 0, note(36, GATE_INF));
    assert_eq!(e.take_playhead(), None);
    e.handle(Command::Play(PlayMode::Section { row: 0, entry: 0 }));
    render(&mut e, 6_000 * 2 + 1, 256);
    let p = e.take_playhead().expect("cambió");
    assert!(p.playing);
    assert!(
        p.heads
            .iter()
            .all(|h| h.active && h.step == 2 && h.entry == 0)
    );
    assert_eq!(e.take_playhead(), None, "sin cambios no se republica");

    e.handle(Command::Stop);
    assert!(!e.is_playing());
    assert!(e.pool().voices().all(|v| v.state == VoiceState::Released));
    assert!(!e.take_playhead().unwrap().playing);
    e.trigger_log.clear();
    render(&mut e, 96_000, 256);
    assert!(e.trigger_log.is_empty());
}

#[test]
fn section_without_a_phrase_is_silent_but_keeps_time() {
    let mut e = engine(120.0);
    silence_all(&mut e);
    set(&mut e, 1, 0, note(60, 24));
    // Sección (0, 1): solo la pista 2 tiene frase.
    e.handle(Command::SetChainEntry {
        chain: 1,
        pos: 1,
        value: Some(ChainEntry {
            phrase: 1,
            transpose: 0,
        }),
    });
    e.handle(Command::Play(PlayMode::Section { row: 0, entry: 1 }));
    render(&mut e, 96_000 + 1, 256);
    assert_eq!(frames_of(&e, 1), [0, 96_000]);
    assert!(frames_of(&e, 0).is_empty());
    let p = e.take_playhead().unwrap();
    assert_eq!(p.heads[0].step, 0, "la pista sin frase también avanza");
}

#[test]
fn voices_peak_counts_voices_that_end_inside_a_callback() {
    let mut e = engine(120.0);
    e.handle(Command::Audition {
        track: 2, // HH-CER: dura menos que un callback de 4096 frames
        notes: [60, 0, 0, 0],
        len: 1,
        velocity: 100,
        gate_ticks: 24,
    });
    render(&mut e, 4096, 4096);
    assert_eq!(e.voices_in_use(), 0, "ya terminó al final del callback");
    assert_eq!(e.take_voices_peak(), 1);
    assert_eq!(e.take_notes_started(), 1);
    assert_eq!(e.take_voices_peak(), 0, "se reinicia al leerlo");
}

// --- H5: cadenas, canción y modos -------------------------------------------------------------

/// Una frase en muestras a 120 BPM.
const P: u64 = 96_000;

fn entry(phrase: u8, transpose: i8) -> Option<ChainEntry> {
    Some(ChainEntry { phrase, transpose })
}

fn chain(e: &mut Engine, id: u8, entries: &[(u8, i8)]) {
    for pos in 0..crate::model::CHAIN_LEN {
        let value = entries.get(pos).and_then(|&(p, t)| entry(p, t));
        e.handle(Command::SetChainEntry {
            chain: id,
            pos: pos as u8,
            value,
        });
    }
}

fn cell(e: &mut Engine, row: u8, track: u8, chain: Option<u8>) {
    e.handle(Command::SetSongCell { row, track, chain });
}

/// Motor vacío (sin la fila 0 de `engine`), sin sonido.
fn bare() -> Engine {
    let mut e = Engine::new(SR);
    silence_all(&mut e);
    e
}

fn notes_of(e: &Engine, track: u8) -> Vec<(u64, u8)> {
    e.trigger_log
        .iter()
        .filter(|(_, t, _)| *t == track)
        .map(|&(f, _, n)| (f, n))
        .collect()
}

#[test]
fn next_position_follows_chains_rows_and_blocks() {
    let mut d = sequencer::SongData::new();
    let song = PlayMode::Song { from_row: 0 };
    let set_chain = |d: &mut sequencer::SongData, id: u8, len: usize| {
        for pos in 0..len {
            d.set_chain_entry(id, pos, entry(id, 0));
        }
    };
    // Pista 0: filas 0-2 (cadenas de 2, 1 y 3 frases), 3 vacía, 4 sola, 6 cadena vacía, 7.
    set_chain(&mut d, 10, 2);
    set_chain(&mut d, 11, 1);
    set_chain(&mut d, 12, 3);
    set_chain(&mut d, 13, 1);
    set_chain(&mut d, 15, 1);
    for (row, id) in [(0, 10), (1, 11), (2, 12), (4, 13), (6, 14), (7, 15)] {
        d.set_song_cell(row, 0, Some(id));
    }
    let walk = |d: &sequencer::SongData, mode, from: (u8, u8), n: usize| {
        let mut pos = from;
        let mut out = vec![pos];
        for _ in 0..n {
            pos = d.next_position(mode, 0, pos.0, pos.1);
            out.push(pos);
        }
        out
    };
    assert_eq!(
        walk(&d, song, (0, 0), 7),
        [
            (0, 0),
            (0, 1),
            (1, 0),
            (2, 0),
            (2, 1),
            (2, 2),
            (0, 0),
            (0, 1)
        ],
        "entradas, fila siguiente y vuelta al inicio del bloque"
    );
    assert_eq!(walk(&d, song, (4, 0), 2), [(4, 0), (4, 0), (4, 0)]);
    assert_eq!(
        walk(&d, song, (6, 0), 3),
        [(6, 0), (7, 0), (6, 0), (7, 0)],
        "una cadena vacía dura una frase de silencio"
    );
    // Desde el medio del bloque también vuelve al inicio (fila 0), no a la fila de arranque.
    assert_eq!(walk(&d, song, (1, 0), 2), [(1, 0), (2, 0), (2, 1)]);
    assert_eq!(d.block_start(0, 2), 0);

    let chain_mode = PlayMode::Chain { track: 0, row: 2 };
    assert_eq!(
        walk(&d, chain_mode, (2, 0), 4),
        [(2, 0), (2, 1), (2, 2), (2, 0), (2, 1)]
    );
    let phrase = PlayMode::Phrase {
        track: 0,
        row: 2,
        entry: 1,
    };
    assert_eq!(walk(&d, phrase, (2, 1), 2), [(2, 1), (2, 1), (2, 1)]);
    let section = PlayMode::Section { row: 0, entry: 1 };
    assert_eq!(walk(&d, section, (0, 1), 1), [(0, 1), (0, 1)]);

    // Última fila de la canción: no hay fila siguiente, vuelve al inicio del bloque.
    d.set_song_cell(254, 0, Some(13));
    d.set_song_cell(255, 0, Some(13));
    assert_eq!(walk(&d, song, (255, 0), 1), [(255, 0), (254, 0)]);

    // Si se vació la celda que sonaba: sigue a la fila siguiente o se queda en su lugar.
    d.set_song_cell(1, 0, None);
    assert_eq!(walk(&d, song, (1, 0), 1), [(1, 0), (2, 0)]);
    d.set_song_cell(4, 0, None);
    assert_eq!(walk(&d, song, (4, 0), 1), [(4, 0), (4, 0)]);
}

#[test]
fn song_plays_chains_with_transpose_row_by_row_and_loops_the_block() {
    let mut e = bare();
    set(&mut e, 0, 0, note(60, 24));
    chain(&mut e, 0, &[(0, 0), (0, 2)]);
    chain(&mut e, 1, &[(0, 7)]);
    cell(&mut e, 0, 0, Some(0));
    cell(&mut e, 1, 0, Some(1));
    e.handle(Command::Play(PlayMode::Song { from_row: 0 }));
    render(&mut e, (4 * P + 1) as usize, 256);
    assert_eq!(
        notes_of(&e, 0),
        [(0, 60), (P, 62), (2 * P, 67), (3 * P, 60), (4 * P, 62)]
    );
    let p = e.take_playhead().unwrap();
    assert_eq!((p.heads[0].row, p.heads[0].entry), (0, 1));
}

#[test]
fn tracks_with_different_chain_lengths_drift_apart() {
    let mut e = bare();
    // Fila 0: la pista 0 con 2 frases, la pista 1 con 1. Fila 1: una frase cada una, con nota.
    chain(&mut e, 0, &[(0, 0), (0, 0)]);
    chain(&mut e, 1, &[(1, 0)]);
    chain(&mut e, 10, &[(10, 0)]);
    chain(&mut e, 11, &[(11, 0)]);
    set(&mut e, 10, 0, note(60, 24));
    set(&mut e, 11, 0, note(48, 24));
    cell(&mut e, 0, 0, Some(0));
    cell(&mut e, 0, 1, Some(1));
    cell(&mut e, 1, 0, Some(10));
    cell(&mut e, 1, 1, Some(11));
    e.handle(Command::Play(PlayMode::Song { from_row: 0 }));
    render(&mut e, (P + 6_000 + 1) as usize, 256);
    let p = e.take_playhead().unwrap();
    assert_eq!(
        (p.heads[0].row, p.heads[1].row),
        (0, 1),
        "en la segunda frase cada pista está en otra fila"
    );
    render(&mut e, (8 * P - P - 6_001) as usize, 256);
    // Ciclos de 3 y 2 frases: la fila 1 cae en frases distintas en cada pista.
    assert_eq!(frames_of(&e, 0), [2 * P, 5 * P]);
    assert_eq!(frames_of(&e, 1), [P, 3 * P, 5 * P, 7 * P]);
}

#[test]
fn a_track_whose_start_cell_is_empty_stays_silent() {
    let mut e = bare();
    for t in [0u8, 1] {
        chain(&mut e, t, &[(t, 0)]);
        set(&mut e, t, 0, note(60, 24));
    }
    cell(&mut e, 0, 0, Some(0));
    cell(&mut e, 1, 0, Some(0));
    cell(&mut e, 1, 1, Some(1));
    // La pista 1 no tiene cadena en la fila 0: no suena aunque la tenga en la fila 1.
    e.handle(Command::Play(PlayMode::Song { from_row: 0 }));
    render(&mut e, (3 * P) as usize, 256);
    assert_eq!(frames_of(&e, 0).len(), 3);
    assert!(frames_of(&e, 1).is_empty());
    assert!(!e.take_playhead().unwrap().heads[1].active);
    // Desde la fila 1 suenan las dos (la pista 0 sola en su bloque de la fila 1).
    e.trigger_log.clear();
    e.handle(Command::Play(PlayMode::Song { from_row: 1 }));
    render(&mut e, P as usize, 256);
    assert_eq!(frames_of(&e, 0).len(), 1);
    assert_eq!(frames_of(&e, 1).len(), 1);
}

#[test]
fn chain_and_phrase_modes_play_one_track() {
    let mut e = bare();
    for t in 0..2u8 {
        set(&mut e, t, 0, note(60, 24));
        chain(&mut e, t, &[(t, 0), (t, 3), (t, 5)]);
        cell(&mut e, 0, t, Some(t));
    }
    e.handle(Command::Play(PlayMode::Chain { track: 1, row: 0 }));
    render(&mut e, (4 * P + 1) as usize, 256);
    assert!(frames_of(&e, 0).is_empty(), "solo la pista de la cadena");
    assert_eq!(
        notes_of(&e, 1),
        [
            (4 * P - 4 * P, 60),
            (P, 63),
            (2 * P, 65),
            (3 * P, 60),
            (4 * P, 63)
        ],
        "loop de la cadena con su transposición"
    );

    e.trigger_log.clear();
    let start = 4 * P + 1;
    e.handle(Command::Play(PlayMode::Phrase {
        track: 0,
        row: 0,
        entry: 2,
    }));
    render(&mut e, (3 * P) as usize, 256);
    assert!(frames_of(&e, 1).is_empty());
    assert_eq!(
        notes_of(&e, 0),
        [(start, 65), (start + P, 65), (start + 2 * P, 65)],
        "loop de la frase de esa entrada"
    );
    let p = e.take_playhead().unwrap();
    assert!(p.heads[0].active && !p.heads[1].active);
    assert_eq!(p.heads[0].entry, 2);
}

#[test]
fn live_song_and_chain_edits_are_heard_when_the_head_gets_there() {
    let mut e = bare();
    set(&mut e, 0, 0, note(60, 24));
    set(&mut e, 1, 0, note(72, 24));
    chain(&mut e, 0, &[(0, 0)]);
    chain(&mut e, 1, &[(1, 0)]);
    cell(&mut e, 0, 0, Some(0));
    e.handle(Command::Play(PlayMode::Song { from_row: 0 }));
    render(&mut e, (P / 2) as usize, 256);
    // Mientras suena la fila 0: agregar una entrada a su cadena y una fila 1.
    e.handle(Command::SetChainEntry {
        chain: 0,
        pos: 1,
        value: entry(0, 12),
    });
    cell(&mut e, 1, 0, Some(1));
    render(&mut e, (3 * P - P / 2) as usize, 256);
    assert_eq!(notes_of(&e, 0), [(0, 60), (P, 72), (2 * P, 72)]);
    // Vaciar la fila 1 mientras suena: al terminarla vuelve al inicio del bloque (fila 0).
    e.trigger_log.clear();
    cell(&mut e, 1, 0, None);
    render(&mut e, (3 * P) as usize, 256);
    assert_eq!(notes_of(&e, 0), [(3 * P, 60), (4 * P, 72), (5 * P, 60)]);
}

#[test]
fn song_mode_does_not_drift_and_is_deterministic() {
    let build = || {
        let mut e = bare();
        for t in 0..TRACKS as u8 {
            set(&mut e, t, 0, note(60 + t, 24));
            set(&mut e, t, 7, note(48, 24));
            // Cadenas de 1 a 3 frases: las pistas se desalinean.
            let len = 1 + usize::from(t) % 3;
            let entries: Vec<(u8, i8)> = (0..len).map(|k| (t, k as i8)).collect();
            chain(&mut e, t, &entries);
            cell(&mut e, 0, t, Some(t));
            cell(&mut e, 1, t, Some((t + 1) % TRACKS as u8));
        }
        e.handle(Command::SetTempo(133.0));
        e.handle(Command::Play(PlayMode::Song { from_row: 0 }));
        e
    };
    let phrases = 40;
    let frames = (sps(133.0) * (PHRASE_STEPS * phrases) as f64) as usize;
    let mut a = build();
    render(&mut a, frames, 256);
    // Cada pista dispara en los pasos 0 y 7 de cada frase, en la muestra calculada.
    for t in 0..TRACKS as u8 {
        let expected: Vec<u64> = (0..phrases)
            .flat_map(|n| [n * PHRASE_STEPS, n * PHRASE_STEPS + 7])
            .map(|k| (k as f64 * sps(133.0)).round() as u64)
            .filter(|&f| f < frames as u64)
            .collect();
        assert_eq!(frames_of(&a, t), expected, "pista {t}");
    }
    let mut b = build();
    render(&mut b, frames, 100);
    assert_eq!(
        a.trigger_log, b.trigger_log,
        "mismos disparos con otro buffer"
    );
}

#[test]
fn song_mode_does_not_allocate() {
    let mut e = bare();
    for t in 0..TRACKS as u8 {
        for i in 0..PHRASE_STEPS {
            set(&mut e, t, i, note(48 + i as u8, 24));
        }
        chain(&mut e, t, &[(t, 0), (t, 5)]);
        cell(&mut e, 0, t, Some(t));
        cell(&mut e, 1, t, Some(t));
    }
    e.trigger_log.clear();
    let mut buf = vec![0.0f32; 256 * 2];
    let allocs = crate::alloc_check::count(|| {
        e.handle(Command::Play(PlayMode::Song { from_row: 1 }));
        for block in 0..2_000u32 {
            if block % 100 == 0 {
                let row = (block / 100 % 4) as u8;
                e.handle(Command::SetSongCell {
                    row,
                    track: 3,
                    chain: Some(row),
                });
                e.handle(Command::SetChainEntry {
                    chain: 3,
                    pos: 2,
                    value: entry(3, row as i8),
                });
            }
            e.render(&mut buf, 2);
            let _ = e.take_playhead();
        }
    });
    assert_eq!(allocs, 0);
    assert!(e.trigger_log.len() > 100);
}
