use std::f32::consts::TAU;

use super::*;
use crate::instrument::{DrumModel, DrumParams, SynthParams, Wave};
use crate::mix::{FxSettings, MasterSettings};
use crate::model::{Chord, ChordKind};

const SR: f32 = 48_000.0;

fn audition(track: u8, notes: &[u8], gate_ticks: u16) -> Command {
    let mut arr = [0u8; MAX_NOTES];
    arr[..notes.len()].copy_from_slice(notes);
    Command::Audition {
        track,
        notes: arr,
        len: notes.len() as u8,
        velocity: 127,
        gate_ticks,
    }
}

/// Sinte de seno con el filtro abierto y ataque de 18 ms: la pendiente máxima de la salida es
/// la del seno (la de la envolvente es < 5 % de la de un seno de 110 Hz).
fn sine_synth(voices: u8) -> Instrument {
    let p = SynthParams {
        wave: Wave::Sine,
        attack: 40,
        decay: 60,
        sustain: 127,
        release: 60,
        cutoff: 127,
        resonance: 0,
        filter_env: 64,
        filter_decay: 60,
    };
    // Volumen 100 (−8 dB): varias voces juntas no llegan al soft clip.
    let mut i = Instrument::new(Patch::Synth(p), voices);
    i.volume = 100;
    i
}

/// Cambia el instrumento y, si no suena nada, deja que el volumen llegue a su valor.
fn set(engine: &mut Engine, track: u8, instrument: Instrument) {
    engine.handle(Command::SetInstrument { track, instrument });
    let mut one = [0.0f32];
    engine.render(&mut one, 1);
}

fn render_mono(engine: &mut Engine, frames: usize) -> Vec<f32> {
    let mut buf = vec![0.0; frames];
    engine.render(&mut buf, 1);
    buf
}

fn max_jump(buf: &[f32]) -> f32 {
    buf.windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f32::max)
}

fn assert_clean(buf: &[f32]) {
    for (i, &x) in buf.iter().enumerate() {
        assert!(x.is_finite(), "muestra {i} no finita: {x}");
        assert!(x == 0.0 || x.is_normal(), "muestra {i} subnormal: {x:e}");
        assert!(x.abs() <= 1.0 + 1e-6, "muestra {i} fuera de rango: {x}");
    }
}

fn notes_of(engine: &Engine, track: u8) -> Vec<u8> {
    let mut v: Vec<(u64, u8)> = engine
        .pool()
        .voices()
        .filter(|v| v.track == track)
        .map(|v| (v.age, v.note))
        .collect();
    v.sort();
    v.into_iter().map(|(_, n)| n).collect()
}

#[test]
fn silent_by_default() {
    let mut engine = Engine::new(SR);
    let mut buf = [1.0f32; 512];
    engine.render(&mut buf, 2);
    assert!(buf.iter().all(|&s| s == 0.0));
}

#[test]
fn every_default_drum_sounds_decays_and_stays_clean() {
    for track in 0..5u8 {
        let mut engine = Engine::new(SR);
        engine.handle(audition(track, &[60], 24));
        let buf = render_mono(&mut engine, SR as usize * 3);
        assert_clean(&buf);
        let peak = buf.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!((0.15..=0.95).contains(&peak), "pista {track}: pico {peak}");
        assert!(engine.is_silent(), "pista {track} sigue sonando a los 3 s");
    }
}

#[test]
fn sampler_track_without_sample_is_silent() {
    let mut engine = Engine::new(SR);
    engine.handle(audition(5, &[60], 24));
    assert!(engine.is_silent());
}

#[test]
fn synth_a4_frequency_and_release_to_silence() {
    let mut engine = Engine::new(SR);
    set(&mut engine, 7, sine_synth(4));
    // 1 paso a 120 BPM = 6000 muestras de gate.
    engine.handle(audition(7, &[69], 24));
    let buf = render_mono(&mut engine, 5_000);
    let rising = buf.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
    // 5000 muestras = 104,2 ms → 45,8 ciclos.
    assert!((45..=46).contains(&rising), "cruces: {rising}");
    let tail = render_mono(&mut engine, SR as usize);
    assert_clean(&tail);
    assert!(engine.is_silent(), "el release no terminó");
    assert_eq!(*tail.last().unwrap(), 0.0);
}

#[test]
fn envelopes_have_no_jumps() {
    // Seno a 220 Hz: la pendiente máxima es A·2π·f/fs; un salto de la envolvente la superaría.
    let mut engine = Engine::new(SR);
    set(&mut engine, 7, sine_synth(4));
    engine.handle(audition(7, &[57], 12));
    let buf = render_mono(&mut engine, SR as usize);
    let peak = buf.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(peak > 0.1);
    let bound = peak * TAU * 220.0 / SR * 1.05;
    assert!(max_jump(&buf) <= bound, "{} > {bound}", max_jump(&buf));
    assert!(engine.is_silent());
}

#[test]
fn bass_and_poly_synth_sound_and_finish() {
    for (track, notes) in [(6u8, &[36u8][..]), (7, &[60, 64, 67, 71][..])] {
        let mut engine = Engine::new(SR);
        engine.handle(audition(track, notes, 24));
        let buf = render_mono(&mut engine, SR as usize * 4);
        assert_clean(&buf);
        let peak = buf.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!((0.1..=0.95).contains(&peak), "pista {track}: pico {peak}");
        assert!(engine.is_silent(), "pista {track}");
    }
}

#[test]
fn track_limit_steals_the_oldest_of_the_track() {
    let mut engine = Engine::new(SR);
    for n in [60, 62, 64, 65, 67] {
        engine.handle(audition(7, &[n], GATE_INF));
        render_mono(&mut engine, 64);
    }
    assert_eq!(notes_of(&engine, 7), [62, 64, 65, 67]);
    assert_eq!(engine.pool().fading(), 1);
}

#[test]
fn chord_drops_highest_notes_when_track_has_fewer_voices() {
    let mut engine = Engine::new(SR);
    set(&mut engine, 7, sine_synth(2));
    let chord = Chord {
        kind: ChordKind::Maj7,
        inversion: 0,
    }
    .notes(60);
    engine.handle(audition(7, chord.as_slice(), 24));
    assert_eq!(notes_of(&engine, 7), [60, 64]);
}

#[test]
fn drums_ignore_chords_and_mono_drum_retrigger_fades_the_old_hit() {
    let mut engine = Engine::new(SR);
    engine.handle(audition(0, &[60, 64, 67], 24));
    assert_eq!(notes_of(&engine, 0), [60]);
    render_mono(&mut engine, 2_000);
    engine.handle(audition(0, &[60], 24));
    assert_eq!(notes_of(&engine, 0), [60]);
    assert_eq!(engine.pool().fading(), 1);
}

#[test]
fn mono_synth_is_legato() {
    let mut engine = Engine::new(SR);
    engine.handle(audition(6, &[36], 24));
    render_mono(&mut engine, 1_000);
    engine.handle(audition(6, &[43], 24));
    assert_eq!(notes_of(&engine, 6), [43]);
    assert_eq!(engine.pool().fading(), 0);
    assert_eq!(engine.voices_in_use(), 1);
}

/// 8 pistas de sinte poli de 8 voces, para llenar la bolsa a mano.
fn full_engine() -> Engine {
    let mut engine = Engine::new(SR);
    for t in 0..TRACKS as u8 {
        set(&mut engine, t, sine_synth(8));
    }
    engine
}

#[test]
fn full_pool_steals_oldest_released_first() {
    let mut engine = full_engine();
    for n in 0..8 {
        engine.handle(audition(0, &[48 + n], GATE_INF));
    }
    // Pista 1: gate de 1/4 de paso, se sueltan enseguida pero siguen en relajación.
    for n in 0..8 {
        engine.handle(audition(1, &[60 + n], 6));
    }
    render_mono(&mut engine, 2_000);
    assert_eq!(engine.pool().voices().count(), 16);
    assert!(
        engine
            .pool()
            .voices()
            .filter(|v| v.track == 1)
            .all(|v| v.state == VoiceState::Released)
    );
    engine.handle(audition(2, &[72], GATE_INF));
    assert_eq!(notes_of(&engine, 1), [61, 62, 63, 64, 65, 66, 67]);
    assert_eq!(notes_of(&engine, 0).len(), 8);
}

#[test]
fn full_pool_without_releases_steals_from_the_busiest_track() {
    let mut engine = full_engine();
    for n in 0..6 {
        engine.handle(audition(1, &[60 + n], GATE_INF));
    }
    for n in 0..8 {
        engine.handle(audition(0, &[48 + n], GATE_INF));
    }
    for n in 0..2 {
        engine.handle(audition(2, &[72 + n], GATE_INF));
    }
    // Bolsa: pista 0 = 8, pista 1 = 6, pista 2 = 2. La pista 1 es la más vieja, pero la 0 usa más.
    engine.handle(audition(3, &[80], GATE_INF));
    assert_eq!(notes_of(&engine, 0), [49, 50, 51, 52, 53, 54, 55]);
    assert_eq!(notes_of(&engine, 1).len(), 6);
}

#[test]
fn chord_never_steals_its_own_notes() {
    let mut engine = full_engine();
    for n in 0..8 {
        engine.handle(audition(0, &[36 + n], GATE_INF));
        engine.handle(audition(1, &[60 + n], GATE_INF));
    }
    engine.handle(audition(2, &[50, 53, 57, 60], GATE_INF));
    assert_eq!(notes_of(&engine, 2), [50, 53, 57, 60]);
    assert_eq!(engine.pool().voices().count(), 16);
    assert!(engine.voices_in_use() <= pool::POOL_VOICES + pool::FADE_SLOTS);
}

#[test]
fn stealing_does_not_click() {
    // Pista de 2 voces, seno a 110 Hz: la tercera nota roba a la primera justo en su pico.
    // Sin fundido, el salto sería la amplitud de una voz; con fundido, queda acotado por la
    // pendiente de las voces sonando más la de la rampa de 5 ms.
    let mut engine = Engine::new(SR);
    let inst = sine_synth(2);
    set(&mut engine, 7, inst);
    let period = SR / 110.0;
    let mut out = Vec::new();
    engine.handle(audition(7, &[45], GATE_INF));
    out.extend(render_mono(&mut engine, 1_000));
    engine.handle(audition(7, &[45], GATE_INF));
    let peak_at = (period * 3.25).round() as usize;
    out.extend(render_mono(&mut engine, peak_at - 1_000));
    engine.handle(audition(7, &[45], GATE_INF));
    assert_eq!(engine.pool().fading(), 1);
    out.extend(render_mono(&mut engine, 4_800));
    assert_clean(&out);
    let voice_amp = synth::VOICE_LEVEL * crate::instrument::volume_gain(inst.volume);
    let slope = voice_amp * TAU / period;
    let fade_slope = voice_amp / (pool::FADE_SECONDS * SR);
    let bound = (3.0 * slope + fade_slope) * 1.05;
    assert!(
        max_jump(&out) <= bound,
        "salto {} > {bound} (amplitud de voz {voice_amp})",
        max_jump(&out)
    );
    assert!(
        bound < voice_amp * 0.1,
        "la cota tiene que distinguir un corte"
    );
}

#[test]
fn stress_full_pool_stays_bounded_and_clean() {
    let mut engine = full_engine();
    for t in 0..TRACKS as u8 {
        set(&mut engine, t, sine_synth(4));
    }
    let chord = |root| {
        Chord {
            kind: ChordKind::Min7,
            inversion: 0,
        }
        .notes(root)
    };
    let mut all = Vec::new();
    for step in 0..32u8 {
        for t in 0..TRACKS as u8 {
            engine.handle(audition(
                t,
                chord(36 + t * 3 + step % 5).as_slice(),
                GATE_INF,
            ));
        }
        assert!(engine.voices_in_use() <= pool::POOL_VOICES + pool::FADE_SLOTS);
        all.extend(render_mono(&mut engine, 1_500));
    }
    assert_clean(&all);
    let peak = all.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(peak <= 1.0 && peak > 0.3);
}

#[test]
fn deterministic() {
    let run = || {
        let mut engine = Engine::new(SR);
        for (i, t) in [0u8, 1, 2, 3, 4, 6, 7].iter().enumerate() {
            engine.handle(audition(*t, &[48 + i as u8, 60, 64], 24));
            render_mono(&mut engine, 777);
        }
        render_mono(&mut engine, 20_000)
    };
    let a = run();
    let b = run();
    assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()));
}

#[test]
fn channels_are_identical_and_partial_blocks_work() {
    let mut engine = Engine::new(SR);
    engine.handle(audition(7, &[60, 64], 24));
    let mut buf = [0.0f32; 77 * 2];
    engine.render(&mut buf, 2);
    assert!(buf.as_chunks::<2>().0.iter().all(|f| f[0] == f[1]));
    assert!(buf.iter().any(|&x| x != 0.0));
}

#[test]
fn gate_follows_tempo() {
    let mut engine = Engine::new(SR);
    assert_eq!(engine.ticks_to_samples(24), 6_000);
    engine.handle(Command::SetTempo(60.0));
    assert_eq!(engine.ticks_to_samples(24), 12_000);
    set(&mut engine, 7, sine_synth(4));
    engine.handle(audition(7, &[60], 24));
    render_mono(&mut engine, 11_990);
    assert!(engine.pool().voices().all(|v| v.state == VoiceState::Held));
    render_mono(&mut engine, 20);
    assert!(
        engine
            .pool()
            .voices()
            .all(|v| v.state == VoiceState::Released)
    );
}

#[test]
fn instrument_volume_change_is_smoothed() {
    let mut engine = Engine::new(SR);
    let mut inst = sine_synth(4);
    set(&mut engine, 7, inst);
    engine.handle(audition(7, &[45], GATE_INF));
    let mut out = render_mono(&mut engine, 4_800);
    inst.volume = 30;
    engine.handle(Command::SetInstrument {
        track: 7,
        instrument: inst,
    });
    out.extend(render_mono(&mut engine, 4_800));
    let peak = out.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    // Sin suavizado el salto sería de casi toda la amplitud.
    let bound = peak * TAU * 110.0 / SR * 1.2;
    assert!(max_jump(&out) <= bound, "{} > {bound}", max_jump(&out));
}

#[test]
fn drum_kit_params_cover_all_models() {
    for model in [
        DrumModel::Kick,
        DrumModel::Snare,
        DrumModel::ClosedHat,
        DrumModel::OpenHat,
        DrumModel::Clap,
    ] {
        let mut engine = Engine::new(SR);
        let mut p = DrumParams::new(model);
        // Extremos de los parámetros: nada explota.
        for v in [0u8, 127] {
            p.tune = v;
            p.decay = v;
            p.color = v;
            p.punch = v;
            p.sweep = v;
            p.spread = v;
            set(&mut engine, 0, Instrument::new(Patch::Drum(p), 1));
            for note in [12u8, 60, 127] {
                engine.handle(audition(0, &[note], 24));
                assert_clean(&render_mono(&mut engine, 4_800));
            }
        }
    }
}

#[test]
fn render_does_not_allocate() {
    let mut engine = Engine::new(SR);
    let mut buf = vec![0.0f32; 256 * 2];
    let allocs = crate::alloc_check::count(|| {
        for step in 0..200u8 {
            engine.handle(audition(step % 8, &[48 + step % 24, 60, 63, 67], 12));
            engine.handle(Command::SetTempo(100.0 + f32::from(step)));
            engine.render(&mut buf, 2);
        }
    });
    assert_eq!(allocs, 0);
}

/// Magnitud de la componente de `freq` (Goertzel), normalizada a amplitud de pico.
fn goertzel(buf: &[f32], freq: f32) -> f32 {
    let w = TAU * freq / SR;
    let c = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    for &x in buf {
        let s = f64::from(x) + f64::from(c) * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    let power = s1 * s1 + s2 * s2 - f64::from(c) * s1 * s2;
    (power.sqrt() * 2.0 / buf.len() as f64) as f32
}

#[test]
fn saw_aliasing_is_low() {
    // Sierra de A7 (3520 Hz) con el filtro abierto. Los armónicos 13 y 14 (45,76 y 49,28 kHz) se
    // pliegan a 2240 y 1280 Hz, en plena zona audible y lejos de los armónicos reales. En una sierra
    // ingenua estarían a −22 dB del fundamental.
    let p = SynthParams {
        wave: Wave::Saw,
        attack: 0,
        decay: 60,
        sustain: 127,
        release: 60,
        cutoff: 127,
        resonance: 0,
        filter_env: 64,
        filter_decay: 60,
    };
    let mut engine = Engine::new(SR);
    set(&mut engine, 7, Instrument::new(Patch::Synth(p), 4));
    engine.handle(audition(7, &[105], GATE_INF));
    render_mono(&mut engine, 4_800);
    // 300 ciclos exactos de 3520 Hz no caen en muestras enteras; 48000 muestras = 1 s da bins de 1 Hz.
    let buf = render_mono(&mut engine, 48_000);
    // Control del método: la misma medición sobre una sierra ingenua.
    let naive: Vec<f32> = (0..48_000)
        .map(|i| 2.0 * (i as f32 * 3520.0 / SR).fract() - 1.0)
        .collect();
    let rel = |b: &[f32], f| 20.0 * (goertzel(b, f) / goertzel(b, 3520.0)).log10();
    for alias in [2240.0, 1280.0] {
        let (blep, ingenua) = (rel(&buf, alias), rel(&naive, alias));
        eprintln!("alias {alias} Hz: PolyBLEP {blep:.1} dB, ingenua {ingenua:.1} dB");
        assert!(ingenua > -30.0, "el método no ve el alias de la ingenua");
        assert!(blep < -40.0, "alias a {alias} Hz: {blep:.1} dB");
    }
}

// --- Mezcla, efectos y master (H9) ------------------------------------------------------------

fn render_stereo(engine: &mut Engine, frames: usize) -> (Vec<f32>, Vec<f32>) {
    let mut buf = vec![0.0; frames * 2];
    engine.render(&mut buf, 2);
    let l = buf.iter().step_by(2).copied().collect();
    let r = buf.iter().skip(1).step_by(2).copied().collect();
    (l, r)
}

fn energy(x: &[f32]) -> f32 {
    x.iter().map(|s| s * s).sum()
}

fn strip(volume: u8, pan: i8, delay_send: u8, reverb_send: u8) -> ChannelStrip {
    ChannelStrip {
        volume,
        pan,
        delay_send,
        reverb_send,
        ..ChannelStrip::default()
    }
}

/// Un seno sostenido en la pista 7, ya estable.
fn sine_engine(mix: ChannelStrip) -> Engine {
    let mut engine = Engine::new(SR);
    set(&mut engine, 7, sine_synth(1));
    engine.handle(Command::SetMix {
        track: 7,
        strip: mix,
    });
    // Sin voces, la mezcla salta a su valor.
    render_stereo(&mut engine, 1);
    engine.handle(audition(7, &[57], GATE_INF));
    render_stereo(&mut engine, 4_800);
    engine
}

#[test]
fn pan_is_constant_power_and_center_is_unchanged() {
    let (l, r) = render_stereo(&mut sine_engine(strip(100, 0, 0, 0)), 4_800);
    assert_eq!(l, r, "centrado: los dos canales iguales");
    let center = energy(&l) + energy(&r);
    let (l, r) = render_stereo(&mut sine_engine(strip(100, -64, 0, 0)), 4_800);
    assert!(r.iter().all(|&x| x.abs() < 1e-6), "todo a la izquierda");
    let left = energy(&l) + energy(&r);
    // Potencia constante (con el soft clip, casi: el seno está a −8 dB).
    assert!((left / center - 1.0).abs() < 0.1, "{left} vs {center}");
    let (l, r) = render_stereo(&mut sine_engine(strip(100, 30, 0, 0)), 4_800);
    assert!(energy(&r) > 2.0 * energy(&l), "a la derecha");
}

#[test]
fn volume_and_pan_changes_are_smoothed() {
    let mut engine = sine_engine(strip(100, 0, 0, 0));
    let (l0, _) = render_stereo(&mut engine, 4_800);
    let natural = max_jump(&l0);
    engine.handle(Command::SetMix {
        track: 7,
        strip: strip(20, -64, 0, 0),
    });
    let (l, r) = render_stereo(&mut engine, 4_800);
    assert!(
        max_jump(&l) < natural * 1.6,
        "L {} vs {natural}",
        max_jump(&l)
    );
    assert!(
        max_jump(&r) < natural * 1.05,
        "R {} vs {natural}",
        max_jump(&r)
    );
    // Llega al valor nuevo: −28 dB y todo a la izquierda (+3 dB).
    let ratio = energy(&l[2400..]) / energy(&l0[..2400]);
    let expected = (0.2f32 * 0.2).powi(2) * 2.0;
    assert!(
        (ratio / expected - 1.0).abs() < 0.15,
        "{ratio} vs {expected}"
    );
}

#[test]
fn delay_send_echoes_after_the_note_and_the_tail_ends() {
    let mut engine = Engine::new(SR);
    engine.handle(Command::SetMix {
        track: 2,
        strip: strip(100, 0, 100, 0),
    });
    engine.handle(audition(2, &[60], 24));
    // HH cerrado (60 ms) y eco a 3/16 de 120 BPM = 375 ms.
    let (l, _) = render_stereo(&mut engine, 24_000);
    let echo_at = 18_000;
    let before = &l[6_000..echo_at - 100];
    let echo = &l[echo_at..echo_at + 2_000];
    let peak = |x: &[f32]| x.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!(
        peak(before) < 1e-3,
        "entre medio, silencio: {}",
        peak(before)
    );
    assert!(peak(echo) > 0.05, "eco {}", peak(echo));
    assert!(!engine.is_quiet());
    // Las colas terminan y el motor vuelve a no hacer nada.
    let mut frames = 0;
    while !engine.is_quiet() {
        render_stereo(&mut engine, 4_800);
        frames += 4_800;
        assert!(frames < 48_000 * 30, "el delay no se apaga");
    }
    let (l, _) = render_stereo(&mut engine, 256);
    assert!(l.iter().all(|&x| x == 0.0));
}

#[test]
fn reverb_send_makes_a_stereo_tail() {
    let mut engine = Engine::new(SR);
    engine.handle(Command::SetMix {
        track: 1,
        strip: strip(100, 0, 0, 100),
    });
    engine.handle(audition(1, &[60], 24));
    render_stereo(&mut engine, 24_000);
    // A los 0,5 s la caja ya terminó: lo que suena es la cola, distinta en cada canal.
    let (l, r) = render_stereo(&mut engine, 4_800);
    assert!(energy(&l) > 1e-4);
    assert_ne!(l, r);
    assert_clean(&l);
}

#[test]
fn limiter_keeps_the_soft_clip_input_at_0_dbfs() {
    let loud = |limiter: bool| {
        let mut engine = Engine::new(SR);
        engine.handle(Command::SetMaster(MasterSettings { gain: 12, limiter }));
        engine.handle(audition(0, &[60], 24));
        let (l, _) = render_stereo(&mut engine, 24_000);
        l.iter().fold(0.0f32, |m, x| m.max(x.abs()))
    };
    let limit = soft_clip(fx::LIMIT);
    assert!(loud(true) <= limit + 1e-6, "con limitador {}", loud(true));
    assert!(
        loud(false) > limit + 0.1,
        "sin limitador satura: {}",
        loud(false)
    );
}

#[test]
fn stick_opens_the_synth_filter_and_returns() {
    let mut p = SynthParams::bass();
    p.filter_env = 64;
    let mut inst = Instrument::new(Patch::Synth(p), 1);
    inst.volume = 90;
    let brightness = |engine: &mut Engine| {
        let (l, _) = render_stereo(engine, 9_600);
        let d: Vec<f32> = l.windows(2).map(|w| w[1] - w[0]).collect();
        (energy(&d) / energy(&l)).sqrt()
    };
    let mut engine = Engine::new(SR);
    set(&mut engine, 6, inst);
    engine.handle(audition(6, &[36], GATE_INF));
    render_stereo(&mut engine, 4_800);
    let stored = brightness(&mut engine);
    engine.handle(Command::Perform {
        track: 6,
        offsets: [1.0, 0.0, 0.0, 0.0],
    });
    render_stereo(&mut engine, 9_600);
    let open = brightness(&mut engine);
    assert!(open > 3.0 * stored, "abierto {open} vs guardado {stored}");
    engine.handle(Command::Perform {
        track: 6,
        offsets: [0.0; 4],
    });
    render_stereo(&mut engine, 9_600);
    let back = brightness(&mut engine);
    assert!(
        (back / stored - 1.0).abs() < 0.05,
        "volvió {back} vs {stored}"
    );
}

#[test]
fn stick_on_a_drum_track_filters_its_bus() {
    let hat = |offsets| {
        let mut engine = Engine::new(SR);
        engine.handle(Command::Perform { track: 3, offsets });
        render_stereo(&mut engine, 4_800);
        engine.handle(audition(3, &[60], 24));
        let (l, _) = render_stereo(&mut engine, 9_600);
        energy(&l)
    };
    let open = hat([0.0; 4]);
    let closed = hat([-1.0, 0.0, 0.0, 0.0]);
    // El hat abierto es casi todo arriba de 5 kHz: con el pasa-bajos en 110 Hz desaparece.
    assert!(closed < 0.01 * open, "{closed} vs {open}");
}

#[test]
fn mixing_and_effects_do_not_allocate() {
    let mut engine = Engine::new(SR);
    let mut buf = vec![0.0f32; 256 * 2];
    let allocs = crate::alloc_check::count(|| {
        for step in 0..400u16 {
            let t = (step % 8) as u8;
            engine.handle(Command::SetMix {
                track: t,
                strip: strip(80 + (step % 40) as u8, (step % 64) as i8 - 32, 60, 90),
            });
            engine.handle(Command::Perform {
                track: t,
                offsets: [((step % 20) as f32 - 10.0) / 10.0, 0.5, 0.3, -0.2],
            });
            if step % 50 == 0 {
                let mut fx = FxSettings::default();
                fx.delay.time = crate::mix::DelayTime::ALL[usize::from(step / 50) % 7];
                fx.reverb.size = (step / 4) as u8;
                engine.handle(Command::SetFx(fx));
                engine.handle(Command::SetMaster(MasterSettings {
                    gain: (step / 50) as i8,
                    limiter: step % 100 == 0,
                }));
            }
            engine.handle(audition(t, &[48 + (step % 24) as u8, 60, 63, 67], 12));
            engine.render(&mut buf, 2);
            let _ = engine.take_meters();
        }
    });
    assert_eq!(allocs, 0);
    assert_clean(&buf);
}

#[test]
fn meters_report_tracks_voices_and_master() {
    let mut engine = Engine::new(SR);
    engine.handle(audition(0, &[60], 24));
    engine.handle(audition(7, &[60, 64, 67], 24));
    let mut buf = vec![0.0f32; 1600 * 2];
    assert!(engine.take_meters().is_none());
    engine.render(&mut buf, 2);
    let m = engine.take_meters().expect("un reporte cada 1600 frames");
    assert!(m.peaks[0] > 0.1 && m.peaks[7] > 0.01 && m.peaks[TRACKS] > 0.1);
    assert_eq!(m.peaks[3], 0.0);
    assert_eq!((m.voices[0], m.voices[7], m.pool_voices), (1, 3, 4));
    assert!(m.limiter_gain <= 1.0);
}
