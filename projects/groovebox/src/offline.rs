//! Uso del motor sin audio ni ventana: render de demos a WAV y benchmark.
//!
//! - `groovebox render [carpeta]`: escribe `h5-cancion.wav` (la canción del proyecto de
//!   demostración en modo canción) y `h3-beat.wav` (sus secciones de la fila 0 en loop), los dos
//!   tocados por el secuenciador real en bloques de 256 frames como el callback, `h2-kit.wav` y
//!   `h2-robo.wav`.
//! - `groovebox bench [segundos]`: renderiza en bloques de 256 frames y mide cada bloque contra
//!   su presupuesto (256 / 48 kHz = 5,33 ms). Con `just run-arm bench` corre emulado: sirve para
//!   comparar, no como dato de la consola.

use std::error::Error;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::engine::sequencer::samples_per_step;
use crate::engine::{Command, Engine, MAX_NOTES, PlayMode, dsp};
use crate::instrument::{Instrument, Patch, SynthParams};
use crate::model::{
    Chord, ChordKind, GATE_INF, NoteCell, NoteSet, PHRASE_STEPS, Project, Section, Step,
    TICKS_PER_STEP, TRACKS,
};
use crate::sync::EngineMirror;

const SR: u32 = 48_000;
const BENCH_FRAMES: usize = 256;

/// Una nota (o acorde) en un paso de semicorchea.
struct Hit {
    step: u32,
    track: u8,
    notes: NoteSet,
    velocity: u8,
    gate_steps: u16,
}

fn hit(step: u32, track: u8, notes: NoteSet, gate_steps: u16) -> Hit {
    Hit {
        step,
        track,
        notes,
        velocity: 110,
        gate_steps,
    }
}

fn audition(track: u8, notes: &[u8], velocity: u8, gate_ticks: u16) -> Command {
    let mut arr = [0u8; MAX_NOTES];
    let len = notes.len().min(MAX_NOTES);
    arr[..len].copy_from_slice(&notes[..len]);
    Command::Audition {
        track,
        notes: arr,
        len: len as u8,
        velocity,
        gate_ticks,
    }
}

/// Renderiza estéreo, aplicando cada comando en su muestra exacta.
fn play(engine: &mut Engine, mut events: Vec<(usize, Command)>, frames: usize) -> Vec<f32> {
    events.sort_by_key(|(at, _)| *at);
    let mut out = vec![0.0f32; frames * 2];
    let mut pos = 0;
    let mut events = events.into_iter().peekable();
    while pos < frames {
        while let Some((_, cmd)) = events.next_if(|(at, _)| *at <= pos) {
            engine.handle(cmd);
        }
        let next = events.peek().map_or(frames, |(at, _)| (*at).min(frames));
        engine.render(&mut out[pos * 2..next * 2], 2);
        pos = next;
    }
    out
}

fn step_samples(bpm: f32) -> usize {
    (SR as f32 * 60.0 / (bpm * 4.0)).round() as usize
}

fn hits_to_events(hits: &[Hit], bpm: f32) -> Vec<(usize, Command)> {
    let step = step_samples(bpm);
    hits.iter()
        .map(|h| {
            let gate = if h.gate_steps == 0 {
                GATE_INF
            } else {
                h.gate_steps * TICKS_PER_STEP
            };
            (
                h.step as usize * step,
                audition(h.track, h.notes.as_slice(), h.velocity, gate),
            )
        })
        .collect()
}

fn write_wav(path: &Path, samples: &[f32]) -> Result<(), Box<dyn Error>> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    for &s in samples {
        w.write_sample((s.clamp(-1.0, 1.0) * 32_767.0).round() as i16)?;
    }
    w.finalize()?;
    let peak = samples.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    let rms = (samples.iter().map(|x| f64::from(x * x)).sum::<f64>() / samples.len() as f64).sqrt();
    println!(
        "render: {} ({:.1} s, pico {:.1} dBFS, RMS {:.1} dBFS)",
        path.display(),
        samples.len() as f32 / 2.0 / SR as f32,
        20.0 * peak.log10(),
        20.0 * rms.log10()
    );
    Ok(())
}

fn chord(kind: ChordKind, root: u8) -> NoteSet {
    Chord { kind, inversion: 0 }.notes(root)
}

fn one(note: u8) -> NoteSet {
    NoteSet::single(note)
}

/// Cada sonido del kit por separado, el bajo y acordes.
fn kit_events() -> (Vec<Hit>, u32) {
    let mut hits = Vec::new();
    // Percusión: cada pista dos golpes (velocidad alta y media), 4 pasos entre golpes.
    for t in 0..5u8 {
        let base = u32::from(t) * 8;
        hits.push(hit(base, t, one(60), 1));
        let mut soft = hit(base + 4, t, one(60), 1);
        soft.velocity = 70;
        hits.push(soft);
    }
    // Bajo (pista 7): C2 Eb2 G2 C3, dos pasos cada una, después legato con largo INF.
    for (i, n) in [36u8, 39, 43, 48].iter().enumerate() {
        hits.push(hit(44 + 2 * i as u32, 6, one(*n), 1));
    }
    hits.push(hit(54, 6, one(36), 0));
    hits.push(hit(55, 6, one(43), 4));
    // Sinte (pista 8): acordes de 4 notas, 6 pasos cada uno.
    let progression = [
        (ChordKind::Min7, 57u8),
        (ChordKind::Maj7, 53),
        (ChordKind::Dom7, 55),
        (ChordKind::Add9, 48),
    ];
    for (i, (kind, root)) in progression.iter().enumerate() {
        hits.push(hit(64 + 8 * i as u32, 7, chord(*kind, *root), 6));
    }
    (hits, 64 + 32 + 12)
}

/// Bolsa llena: las 8 pistas como sinte poli de 4 voces, acordes de 4 notas con largo INF
/// en cada paso. Pide 32 voces por paso a una bolsa de 16: robo constante.
fn stress_setup(engine: &mut Engine) {
    for t in 0..TRACKS as u8 {
        let mut instrument = Instrument::new(Patch::Synth(SynthParams::poly()), 4);
        instrument.volume = 70;
        engine.handle(Command::SetInstrument {
            track: t,
            instrument,
        });
    }
}

fn stress_events(steps: u32) -> Vec<Hit> {
    let kinds = [
        ChordKind::Min7,
        ChordKind::Maj7,
        ChordKind::Dom7,
        ChordKind::Add9,
    ];
    let mut hits = Vec::new();
    for s in 0..steps {
        for t in 0..TRACKS as u8 {
            let root = 40 + (u32::from(t) * 5 + s * 7) % 24;
            hits.push(hit(
                s,
                t,
                chord(kinds[(s as usize + usize::from(t)) % 4], root as u8),
                0,
            ));
        }
    }
    hits
}

/// Motor con el proyecto cargado como lo carga la interfaz (los mismos comandos).
pub fn engine_with(project: &Project) -> Engine {
    let mut engine = Engine::new(SR as f32);
    EngineMirror::new().sync(project, None, &mut |cmd| {
        engine.handle(cmd);
        true
    });
    engine
}

/// Muestras de `loops` vueltas de una frase al tempo del proyecto.
fn loop_frames(project: &Project, loops: usize) -> usize {
    let sps = samples_per_step(SR as f32, project.tempo_bpm);
    (sps * (PHRASE_STEPS * loops) as f64).round() as usize
}

/// Toca cada sección de la fila 0 `loops` veces (Select + Start en cada una, en el primer paso)
/// y después para y deja sonar las colas. Renderiza en bloques como el callback.
fn play_sections(project: &Project, loops: usize, block: usize) -> Vec<f32> {
    let mut engine = engine_with(project);
    let sections = project.section_count(0);
    let tail = SR as usize * 3 / 2;
    let mut out = Vec::new();
    let mut buf = vec![0.0f32; block * 2];
    for entry in 0..sections {
        engine.handle(Command::Play(PlayMode::Section {
            row: 0,
            entry: entry as u8,
        }));
        render_frames(&mut engine, &mut out, &mut buf, loop_frames(project, loops));
    }
    engine.handle(Command::Stop);
    render_frames(&mut engine, &mut out, &mut buf, tail);
    out
}

/// Toca la canción desde la fila 0 hasta que la pista más larga termina su bloque (las más
/// cortas ya volvieron al principio: se oye la desalineación), para y deja sonar las colas.
fn play_song(project: &Project, block: usize) -> Vec<f32> {
    let mut engine = engine_with(project);
    let phrases = song_phrases(project);
    let mut out = Vec::new();
    let mut buf = vec![0.0f32; block * 2];
    engine.handle(Command::Play(PlayMode::Song { from_row: 0 }));
    render_frames(
        &mut engine,
        &mut out,
        &mut buf,
        loop_frames(project, phrases),
    );
    engine.handle(Command::Stop);
    render_frames(&mut engine, &mut out, &mut buf, SR as usize * 3 / 2);
    out
}

/// Frases que dura una pasada de la pista más larga por el bloque de la fila 0.
fn song_phrases(project: &Project) -> usize {
    (0..TRACKS)
        .filter(|&t| project.chain_id_at(t, 0).is_some())
        .map(|t| project.block_phrases(t, 0))
        .max()
        .unwrap_or(1)
}

fn render_frames(engine: &mut Engine, out: &mut Vec<f32>, buf: &mut [f32], frames: usize) {
    let block = buf.len() / 2;
    let mut left = frames;
    while left > 0 {
        let n = left.min(block);
        engine.render(&mut buf[..n * 2], 2);
        out.extend_from_slice(&buf[..n * 2]);
        left -= n;
    }
}

pub fn render(dir: Option<&str>) -> Result<(), Box<dyn Error>> {
    dsp::enable_flush_to_zero();
    let dir = Path::new(dir.unwrap_or("."));
    std::fs::create_dir_all(dir)?;
    let bpm = 120.0;
    let step = step_samples(bpm);

    let (hits, steps) = kit_events();
    let mut engine = Engine::new(SR as f32);
    let out = play(
        &mut engine,
        hits_to_events(&hits, bpm),
        steps as usize * step,
    );
    write_wav(&dir.join("h2-kit.wav"), &out)?;

    let demo = crate::demo::project();
    let out = play_song(&demo, BENCH_FRAMES);
    write_wav(&dir.join("h5-cancion.wav"), &out)?;
    let lens: Vec<String> = (0..demo.song.rows.len())
        .map(|r| {
            let lens: String = demo
                .chain_lengths(r)
                .iter()
                .map(|l| l.map_or("-".to_string(), |l| l.to_string()))
                .collect::<Vec<_>>()
                .join(" ");
            let mark = if demo.row_misaligned(r) { " !" } else { "" };
            format!("fila {r}: {lens}{mark}")
        })
        .collect();
    println!(
        "render: h5-cancion = canción demo desde la fila 0, {} frases a {} BPM (largos: {})",
        song_phrases(&demo),
        demo.tempo_bpm,
        lens.join("; ")
    );

    let out = play_sections(&demo, 2, BENCH_FRAMES);
    write_wav(&dir.join("h3-beat.wav"), &out)?;
    println!(
        "render: h3-beat = proyecto demo con el secuenciador: {} secciones × 2 vueltas a {} BPM",
        demo.section_count(0),
        demo.tempo_bpm
    );

    let mut engine = Engine::new(SR as f32);
    stress_setup(&mut engine);
    let hits = stress_events(32);
    let out = play(&mut engine, hits_to_events(&hits, bpm), 40 * step);
    write_wav(&dir.join("h2-robo.wav"), &out)?;
    println!(
        "render: robo con la bolsa llena: {} fundidos cortados en seco",
        engine.pool().hard_cuts
    );
    Ok(())
}

/// Mide `seconds` de audio en bloques de 256 frames; `before` corre antes de cada bloque (dentro
/// de la medición, como los comandos en el callback) con la posición en frames.
fn measure(
    name: &str,
    seconds: f32,
    mut engine: Engine,
    mut before: impl FnMut(&mut Engine, usize),
) {
    let total = (seconds * SR as f32) as usize;
    let mut buf = vec![0.0f32; BENCH_FRAMES * 2];
    let mut times = Vec::with_capacity(total / BENCH_FRAMES + 1);
    let mut voice_frames = 0u64;
    let mut pos = 0;
    while pos + BENCH_FRAMES <= total {
        let start = Instant::now();
        before(&mut engine, pos);
        engine.render(&mut buf, 2);
        times.push(start.elapsed());
        std::hint::black_box(&buf);
        voice_frames += (engine.voices_in_use() * BENCH_FRAMES) as u64;
        pos += BENCH_FRAMES;
    }
    let budget = Duration::from_secs_f64(BENCH_FRAMES as f64 / f64::from(SR));
    let total_time: Duration = times.iter().sum();
    let pct = |d: Duration| d.as_secs_f64() / budget.as_secs_f64() * 100.0;
    let avg = pct(total_time / times.len() as u32);
    times.sort();
    let p99 = pct(times[times.len() * 99 / 100]);
    let max = pct(*times.last().unwrap());
    let blocks = times.len() as f64;
    let avg_voices = voice_frames as f64 / (blocks * BENCH_FRAMES as f64);
    let per_voice = if voice_frames > 0 {
        format!(
            "{:6.1} ns/voz/muestra",
            total_time.as_nanos() as f64 / voice_frames as f64
        )
    } else {
        format!("{:6.2} µs/bloque", total_time.as_secs_f64() * 1e6 / blocks)
    };
    println!(
        "bench: {name:<44} CPU prom. {avg:6.2} %  p99 {p99:6.2} %  máx. {max:6.2} %  voces prom. {avg_voices:5.1}  {per_voice}"
    );
}

/// Aplica los comandos de `events` (en un loop de `loop_len` frames) que caen en el bloque.
fn replay(events: Vec<(usize, Command)>, loop_len: usize) -> impl FnMut(&mut Engine, usize) {
    move |engine, pos| {
        let (from, to) = (pos % loop_len, pos % loop_len + BENCH_FRAMES);
        for (at, cmd) in &events {
            if (from..to).contains(at) {
                engine.handle(*cmd);
            }
        }
    }
}

/// El proyecto demo en loop de sección, cambiando de sección cada 2 vueltas.
fn sequenced(project: &Project) -> impl FnMut(&mut Engine, usize) {
    let section_len = loop_frames(project, 2);
    let sections = project.section_count(0).max(1);
    let mut current = None;
    move |engine, pos| {
        let entry = (pos / section_len) % sections;
        if current != Some(entry) {
            current = Some(entry);
            engine.handle(Command::Play(PlayMode::Section {
                row: 0,
                entry: entry as u8,
            }));
        }
    }
}

/// Solo el secuenciador: las 8 pistas sin sonido (sampler vacío) con una nota con acorde en
/// cada paso, a 300 BPM. Mide recorrer los cabezales y armar los disparos, sin DSP de voces.
fn sequencer_only() -> Project {
    let mut p = Project::new_default();
    p.tempo_bpm = 300.0;
    for t in 0..TRACKS {
        p.tracks[t].instrument = Instrument::new(Patch::Sampler, 2);
        for i in 0..PHRASE_STEPS {
            let step = p.step_mut(t, Section::default(), i).expect("frase 00t");
            *step = Step {
                note: NoteCell::On(48 + i as u8),
                chord: Chord {
                    kind: ChordKind::Min7,
                    inversion: 1,
                },
                ..Step::default()
            };
        }
    }
    p
}

pub fn bench(seconds: Option<f32>) -> Result<(), Box<dyn Error>> {
    dsp::enable_flush_to_zero();
    let seconds = seconds.unwrap_or(10.0).max(1.0);
    println!(
        "bench: {seconds} s de audio por escenario, bloques de {BENCH_FRAMES} frames a {SR} Hz \
         (presupuesto {:.2} ms); % de un núcleo de esta máquina",
        BENCH_FRAMES as f64 * 1000.0 / f64::from(SR)
    );
    let step = step_samples(120.0);

    let mut engine = Engine::new(SR as f32);
    stress_setup(&mut engine);
    let events = hits_to_events(&stress_events(16), 120.0);
    measure(
        "bolsa llena (8 sintes, acordes INF)",
        seconds,
        engine,
        replay(events, 16 * step),
    );

    let demo = crate::demo::project();
    measure(
        "secuenciador: proyecto demo",
        seconds,
        engine_with(&demo),
        sequenced(&demo),
    );

    measure(
        "secuenciador: canción demo (modo canción)",
        seconds,
        engine_with(&demo),
        |engine, pos| {
            if pos == 0 {
                engine.handle(Command::Play(PlayMode::Song { from_row: 0 }));
            }
        },
    );

    let silent = sequencer_only();
    measure(
        "secuenciador solo (8 pistas, acorde/paso, 300 BPM)",
        seconds,
        engine_with(&silent),
        sequenced(&silent),
    );
    measure(
        "motor vacío (parado, sin voces)",
        seconds,
        Engine::new(SR as f32),
        |_, _| {},
    );

    let drums: Vec<Hit> = (0..16)
        .flat_map(|s| (0..5u8).map(move |t| hit(s, t, one(60), 1)))
        .collect();
    measure(
        "percusión (5 pistas en cada paso)",
        seconds,
        Engine::new(SR as f32),
        replay(hits_to_events(&drums, 120.0), 16 * step),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beat_render_is_deterministic_and_independent_of_the_buffer_size() {
        let demo = crate::demo::project();
        let a = play_sections(&demo, 1, 256);
        let b = play_sections(&demo, 1, 256);
        assert_eq!(a, b, "determinismo");
        // Con buffers múltiplos de 32 los subbloques caen en las mismas muestras: mismo audio.
        for block in [64, 128, 512] {
            let c = play_sections(&demo, 1, block);
            assert_eq!(a, c, "el mismo audio con buffers de {block} frames");
        }
        let peak = a.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.1 && peak <= 1.0, "pico {peak}");
    }

    #[test]
    fn song_render_is_deterministic_and_independent_of_the_buffer_size() {
        let demo = crate::demo::project();
        assert_eq!(song_phrases(&demo), 10);
        let a = play_song(&demo, 256);
        assert_eq!(a, play_song(&demo, 256), "determinismo");
        assert_eq!(
            a,
            play_song(&demo, 96),
            "el mismo audio con buffers de 96 frames"
        );
        let expected = loop_frames(&demo, 10) + SR as usize * 3 / 2;
        assert_eq!(a.len(), expected * 2);
        // Suena en cada fila de la canción, no solo en la primera.
        let bar = loop_frames(&demo, 1);
        for phrase in [0, 4, 6, 9] {
            let chunk = &a[phrase * bar * 2..(phrase + 1) * bar * 2];
            let peak = chunk.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(peak > 0.1, "frase {phrase}: pico {peak}");
        }
    }
}
