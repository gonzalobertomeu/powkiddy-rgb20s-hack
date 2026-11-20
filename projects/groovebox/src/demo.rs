//! Proyecto de demostración: una canción de 3 filas con batería, bajo y acordes. Lo usan
//! `groovebox render` (h3-beat.wav y h5-cancion.wav), `groovebox bench` y `GROOVEBOX_DEMO=1`.
//!
//! - Fila 0 (A, 4 compases): la batería comparte la misma frase en las secciones 1 a 3 y cambia
//!   en la 4 (relleno); el bajo y los acordes tienen una frase por sección (Am7 F G E).
//! - Fila 1 (B, 2 compases): reusa las frases de la fila 0 con la transposición de la cadena
//!   (bajo y acordes de Am7 a Dm7 y Em7).
//! - Fila 2 (C, 4 compases): la progresión de A una tercera menor arriba; los hi-hats cerrados
//!   tienen una cadena de 3 frases y las palmas de 2: la fila se marca desalineada y esas
//!   pistas vuelven a la fila 0 antes que el resto (spec-motor, "Cabezales por pista").
//!
//! SAMPLE no tiene cadenas: no suena y no cuenta para la alineación.

use crate::model::{
    Chord, ChordKind, NoteCell, PHRASE_STEPS, PhraseId, Project, Section, Step, TICKS_PER_STEP,
};

pub const SECTIONS: usize = 4;
#[cfg(test)]
pub const ROWS: usize = 3;

const KICK: usize = 0;
const SNARE: usize = 1;
const CLOSED_HAT: usize = 2;
const OPEN_HAT: usize = 3;
const CLAP: usize = 4;
const SAMPLE: usize = 5;
const BASS: usize = 6;
const CHORDS: usize = 7;

fn note(n: u8, velocity: u8, gate_steps: u16) -> Step {
    Step {
        note: NoteCell::On(n),
        velocity,
        gate_ticks: gate_steps * TICKS_PER_STEP,
        ..Step::default()
    }
}

/// Pone `steps` en la frase de `track` en la sección `(0, entry)`, creando lo que falte. Con
/// `share`, la sección usa esa frase en lugar de una nueva.
fn put(
    p: &mut Project,
    track: usize,
    entry: usize,
    share: Option<PhraseId>,
    steps: &[(usize, Step)],
) {
    let sec = Section { row: 0, entry };
    p.ensure_phrase(track, sec, share)
        .expect("el proyecto nuevo tiene lugar para la demo");
    if share.is_some() {
        return;
    }
    for &(i, step) in steps {
        if let Some(s) = p.step_mut(track, sec, i) {
            *s = step;
        }
    }
}

pub fn project() -> Project {
    let mut p = Project::new_default();
    p.name = "demo".to_string();

    let kick: Vec<_> = [0, 4, 8, 12]
        .iter()
        .map(|&i| (i, note(60, 120, 1)))
        .collect();
    let mut kick_fill = kick.clone();
    kick_fill.push((14, note(60, 90, 1)));
    kick_fill.push((15, note(60, 70, 1)));
    let snare = [(4, note(60, 110, 1)), (12, note(60, 110, 1))];
    let hats: Vec<_> = (0..PHRASE_STEPS)
        .step_by(2)
        .map(|i| (i, note(60, if i % 4 == 0 { 100 } else { 64 }, 1)))
        .collect();
    let open_hat = [(14, note(60, 90, 1))];
    let clap = [(12, note(60, 100, 1))];

    // Bajo: raíces A, F, G, E con la octava y una nota de paso.
    let roots = [33u8, 29, 31, 28];
    let chords = [
        (ChordKind::Min7, 57u8),
        (ChordKind::Maj7, 53),
        (ChordKind::Dom7, 55),
        (ChordKind::Sus4, 52),
    ];

    for entry in 0..SECTIONS {
        // En la sección 0 cada pista tiene su frase 000–007 (proyecto nuevo); las demás la comparten.
        let shared = |t: usize| (entry > 0).then_some(t as PhraseId);
        if entry == SECTIONS - 1 {
            put(&mut p, KICK, entry, None, &kick_fill);
        } else {
            put(&mut p, KICK, entry, shared(KICK), &kick);
        }
        put(&mut p, SNARE, entry, shared(SNARE), &snare);
        put(&mut p, CLOSED_HAT, entry, shared(CLOSED_HAT), &hats);
        put(&mut p, OPEN_HAT, entry, shared(OPEN_HAT), &open_hat);
        put(&mut p, CLAP, entry, shared(CLAP), &clap);

        let r = roots[entry];
        let bass = [
            (0, note(r, 120, 2)),
            (3, note(r, 90, 2)),
            (6, note(r + 12, 100, 1)),
            (10, note(r, 100, 2)),
            (14, note(r + 7, 90, 2)),
        ];
        put(&mut p, BASS, entry, None, &bass);

        let (kind, root) = chords[entry];
        let chord = Chord { kind, inversion: 0 };
        let stab = Step {
            chord,
            ..note(root, 80, 2)
        };
        let pad = Step {
            chord,
            ..note(root, 100, 10)
        };
        put(
            &mut p,
            CHORDS,
            entry,
            None,
            &[(0, pad), (11, stab), (14, stab)],
        );
    }
    p.set_song_cell(0, SAMPLE, None).expect("la fila 0 existe");

    // Filas 1 y 2: cadenas nuevas con las frases de la fila 0.
    let phrase = |track: usize, entry: usize| {
        p.phrase_id_at(track, Section { row: 0, entry })
            .expect("la fila 0 tiene las 4 secciones")
    };
    let fill = phrase(KICK, SECTIONS - 1);
    let first: [PhraseId; CHORDS + 1] =
        std::array::from_fn(|t| if t == SAMPLE { 0 } else { phrase(t, 0) });
    let harmony: [[PhraseId; SECTIONS]; 2] =
        [BASS, CHORDS].map(|t| std::array::from_fn(|e| phrase(t, e)));

    for (row, len) in [(1, 2), (2, SECTIONS)] {
        let drums = |t: usize, n: usize| vec![(first[t], 0i8); n];
        let mut kick_chain = drums(KICK, len - 1);
        kick_chain.push((fill, 0));
        chain(&mut p, KICK, row, &kick_chain);
        chain(&mut p, SNARE, row, &drums(SNARE, len));
        chain(&mut p, OPEN_HAT, row, &drums(OPEN_HAT, len));
        let hats = if row == 1 { 2 } else { 3 };
        chain(&mut p, CLOSED_HAT, row, &drums(CLOSED_HAT, hats));
        chain(&mut p, CLAP, row, &drums(CLAP, 2));
        // Fila 1: la sección A de la fila 0 en D y E; fila 2: toda la progresión +3.
        let sections: Vec<(usize, i8)> = if row == 1 {
            vec![(0, 5), (0, 7)]
        } else {
            (0..SECTIONS).map(|e| (e, 3)).collect()
        };
        for (track, phrases) in [BASS, CHORDS].into_iter().zip(&harmony) {
            let entries: Vec<(PhraseId, i8)> =
                sections.iter().map(|&(e, t)| (phrases[e], t)).collect();
            chain(&mut p, track, row, &entries);
        }
    }
    p
}

/// Arma la cadena de `track` en la fila `row` con `entries` (frase, transposición).
fn chain(p: &mut Project, track: usize, row: usize, entries: &[(PhraseId, i8)]) {
    for (entry, &(phrase, transpose)) in entries.iter().enumerate() {
        let sec = Section { row, entry };
        p.ensure_phrase(track, sec, Some(phrase))
            .expect("el proyecto nuevo tiene lugar para la demo");
        p.set_entry_transpose(track, sec, transpose)
            .expect("la entrada se acaba de crear");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_has_four_sections_with_shared_drums() {
        let p = project();
        assert_eq!(p.section_count(0), SECTIONS);
        let sec = |entry| Section { row: 0, entry };
        assert_eq!(p.phrase_id_at(SNARE, sec(2)), p.phrase_id_at(SNARE, sec(0)));
        assert_ne!(p.phrase_id_at(KICK, sec(3)), p.phrase_id_at(KICK, sec(0)));
        assert_ne!(p.phrase_id_at(BASS, sec(1)), p.phrase_id_at(BASS, sec(0)));
        assert_eq!(
            p.step(CHORDS, sec(0), 0).unwrap().chord.kind,
            ChordKind::Min7
        );
        assert!(p.step(KICK, sec(3), 15).unwrap().root().is_some());
        assert_eq!(p.chain_id_at(SAMPLE, 0), None, "SAMPLE no tiene cadenas");
    }

    #[test]
    fn demo_song_has_rows_with_transpose_and_different_lengths() {
        let p = project();
        assert_eq!(p.song.rows.len(), ROWS);
        assert!(!p.row_misaligned(0) && !p.row_misaligned(1));
        assert!(p.row_misaligned(2), "fila 2: hi-hats de 3 y palmas de 2");
        let lens = p.chain_lengths(2);
        assert_eq!(
            (lens[CLOSED_HAT], lens[CLAP], lens[KICK]),
            (Some(3), Some(2), Some(4))
        );
        let bass = p.chain_at(BASS, 1).unwrap();
        assert_eq!(bass.entry(1).unwrap().transpose, 7);
        assert_eq!(
            bass.entry(0).unwrap().phrase,
            p.phrase_id_at(BASS, Section { row: 0, entry: 0 }).unwrap(),
            "la fila 1 reusa las frases de la fila 0"
        );
        assert_eq!(p.block_phrases(KICK, 0), 10);
        assert_eq!(p.block_phrases(CLAP, 0), 8);
    }
}
