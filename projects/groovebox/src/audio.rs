//! Salida de audio con cpal. El callback corre en el hilo que crea el backend
//! (en ALSA, un hilo propio de cpal), no en el de la interfaz.
//!
//! El trabajo del callback está en `Processor` (sin cpal, testeable): vaciar la cola de
//! comandos, renderizar, convertir al formato del dispositivo, medir el tiempo y publicar la
//! carga y los cabezales por la cola de eventos.
//!
//! `start_null` corre el mismo `Processor` en un hilo propio al ritmo del audio pero sin
//! dispositivo (`GROOVEBOX_AUDIO=null`): sirve para probar la app sin placa de sonido.

use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, FromSample, SampleFormat, SizedSample, StreamConfig};

use crate::engine::{Command, Engine, Event, LoadMeter, Playhead, dsp};

pub const PREFERRED_RATE: u32 = 48_000;
pub const PREFERRED_FRAMES: u32 = 256;
/// Tope de frames por callback para el buffer intermedio preasignado.
/// Si el backend entrega más, se procesa en trozos.
const SCRATCH_FRAMES: usize = 4096;
const MAX_CHANNELS: usize = 8;
/// Tamaño de la spec: entra una carga completa de ediciones sin bloquear a la interfaz.
const COMMAND_QUEUE: usize = 1024;
/// Cabe más de un segundo de pasos a 300 BPM aunque la interfaz se trabe.
const EVENT_QUEUE: usize = 256;

/// Contadores que el callback escribe y la interfaz lee (sin locks).
#[derive(Default)]
pub struct AudioStats {
    /// Frames del último callback: el tamaño de período real que eligió el backend.
    pub last_frames: AtomicU32,
    pub callbacks: AtomicU32,
    pub stream_errors: AtomicU32,
}

/// Hilo del backend nulo; al soltarlo se detiene.
struct NullThread {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for NullThread {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

enum Backend {
    // Se guarda para que el stream siga vivo; al soltarlo se detiene el audio.
    Cpal(#[expect(dead_code, reason = "solo se guarda para mantener vivo el stream")] cpal::Stream),
    Null(
        #[expect(dead_code, reason = "solo se guarda para detener el hilo al soltarlo")] NullThread,
    ),
}

pub struct AudioOutput {
    _backend: Backend,
    pub commands: rtrb::Producer<Command>,
    pub events: rtrb::Consumer<Event>,
    pub stats: Arc<AudioStats>,
}

/// Abre el dispositivo por defecto. Intenta 48 kHz f32 estéreo con el período pedido;
/// si no se puede, cae a la configuración por defecto del dispositivo.
pub fn start(frames: u32) -> Result<AudioOutput, Box<dyn Error>> {
    let host = cpal::default_host();
    println!("audio: host {}", host.id().name());
    let device = host
        .default_output_device()
        .ok_or("no hay dispositivo de salida de audio")?;
    match device.description() {
        Ok(desc) => println!("audio: dispositivo \"{desc}\""),
        Err(e) => println!("audio: dispositivo (sin descripción: {e})"),
    }

    let stats = Arc::new(AudioStats::default());

    let preferred = find_preferred(&device);
    let (config, format) = match preferred {
        Some(range) => {
            let buffer_size = match range.buffer_size() {
                cpal::SupportedBufferSize::Range { min, max } => {
                    BufferSize::Fixed(frames.clamp(*min, *max))
                }
                cpal::SupportedBufferSize::Unknown => BufferSize::Fixed(frames),
            };
            let config = StreamConfig {
                channels: range.channels(),
                sample_rate: PREFERRED_RATE,
                buffer_size,
            };
            (config, SampleFormat::F32)
        }
        None => {
            let def = device.default_output_config()?;
            println!(
                "audio: el dispositivo no ofrece {PREFERRED_RATE} Hz f32; uso la config por defecto"
            );
            (def.config(), def.sample_format())
        }
    };

    // Primer intento con lo elegido; si falla con período fijo, reintento con el del backend.
    let (stream, commands, events) = match build(&device, config, format, stats.clone()) {
        Ok(built) => {
            log_config(&config, format);
            built
        }
        Err(e) if config.buffer_size != BufferSize::Default => {
            println!(
                "audio: falló con {:?} ({e}); reintento con período por defecto",
                config.buffer_size
            );
            let config = StreamConfig {
                buffer_size: BufferSize::Default,
                ..config
            };
            let built = build(&device, config, format, stats.clone())?;
            log_config(&config, format);
            built
        }
        Err(e) => return Err(e),
    };
    stream.play()?;

    Ok(AudioOutput {
        _backend: Backend::Cpal(stream),
        commands,
        events,
        stats,
    })
}

/// Descarta la configuración de ALSA que alsa-lib guarda en caché en el proceso, para que el
/// próximo `start` relea los archivos (el modo mono/estéreo de `rgb20s-audio` vive en
/// `/var/lib/alsa/conf.d`, que alsa-lib no vuelve a mirar por su cuenta). Llamar con el stream
/// anterior ya cerrado.
pub fn reload_config() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    alsa::config::update_free_global().map_err(|e| e.to_string())?;
    Ok(())
}

/// Backend sin dispositivo: llama al `Processor` cada `frames` muestras de tiempo real.
pub fn start_null(frames: u32) -> Result<AudioOutput, Box<dyn Error>> {
    let frames = frames.clamp(16, SCRATCH_FRAMES as u32) as usize;
    let stats = Arc::new(AudioStats::default());
    let (cmd_tx, cmd_rx) = rtrb::RingBuffer::new(COMMAND_QUEUE);
    let (ev_tx, ev_rx) = rtrb::RingBuffer::new(EVENT_QUEUE);
    let channels = 2;
    let mut processor = Processor::new(
        PREFERRED_RATE as f32,
        channels,
        cmd_rx,
        ev_tx,
        stats.clone(),
    );
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    let period = Duration::from_secs_f64(frames as f64 / f64::from(PREFERRED_RATE));
    let handle = std::thread::Builder::new()
        .name("audio-null".into())
        .spawn(move || {
            let mut buf = vec![0.0f32; frames * channels];
            let mut next = Instant::now();
            while !thread_stop.load(Ordering::Relaxed) {
                processor.process(&mut buf);
                next += period;
                if let Some(wait) = next.checked_duration_since(Instant::now()) {
                    std::thread::sleep(wait);
                }
            }
        })?;
    println!("audio: backend nulo, {PREFERRED_RATE} Hz, {frames} frames por período, sin salida");
    Ok(AudioOutput {
        _backend: Backend::Null(NullThread {
            stop,
            handle: Some(handle),
        }),
        commands: cmd_tx,
        events: ev_rx,
        stats,
    })
}

fn find_preferred(device: &cpal::Device) -> Option<cpal::SupportedStreamConfigRange> {
    let ranges = device.supported_output_configs().ok()?;
    let mut candidates: Vec<_> = ranges
        .filter(|r| {
            r.sample_format() == SampleFormat::F32
                && (r.min_sample_rate()..=r.max_sample_rate()).contains(&PREFERRED_RATE)
                && (r.channels() as usize) <= MAX_CHANNELS
        })
        .collect();
    // Estéreo primero; si no hay, el que tenga menos canales.
    candidates.sort_by_key(|r| (r.channels() != 2, r.channels()));
    candidates.into_iter().next()
}

fn log_config(config: &StreamConfig, format: SampleFormat) {
    let buffer = match config.buffer_size {
        BufferSize::Fixed(n) => format!(
            "{n} frames pedidos ({:.2} ms)",
            n as f32 * 1000.0 / config.sample_rate as f32
        ),
        BufferSize::Default => "por defecto del backend".to_string(),
    };
    println!(
        "audio: {} Hz, {} canales, formato {format}, buffer {buffer}",
        config.sample_rate, config.channels
    );
}

type Built = (cpal::Stream, rtrb::Producer<Command>, rtrb::Consumer<Event>);

/// Cada intento crea sus propias colas: si `build_output_stream` falla, los extremos que se
/// movieron al closure se pierden con él, así que no se pueden reutilizar.
fn build(
    device: &cpal::Device,
    config: StreamConfig,
    format: SampleFormat,
    stats: Arc<AudioStats>,
) -> Result<Built, Box<dyn Error>> {
    let (cmd_tx, cmd_rx) = rtrb::RingBuffer::new(COMMAND_QUEUE);
    let (ev_tx, ev_rx) = rtrb::RingBuffer::new(EVENT_QUEUE);
    let processor = Processor::new(
        config.sample_rate as f32,
        config.channels as usize,
        cmd_rx,
        ev_tx,
        stats,
    );
    let stream = match format {
        SampleFormat::F32 => build_typed::<f32>(device, config, processor)?,
        SampleFormat::I16 => build_typed::<i16>(device, config, processor)?,
        SampleFormat::I32 => build_typed::<i32>(device, config, processor)?,
        SampleFormat::U16 => build_typed::<u16>(device, config, processor)?,
        other => return Err(format!("formato de muestra no soportado: {other}").into()),
    };
    Ok((stream, cmd_tx, ev_rx))
}

fn build_typed<T>(
    device: &cpal::Device,
    config: StreamConfig,
    mut processor: Processor,
) -> Result<cpal::Stream, Box<dyn Error>>
where
    T: SizedSample + FromSample<f32>,
{
    let err_stats = processor.stats.clone();
    let stream = device.build_output_stream::<T, _, _>(
        config,
        move |out: &mut [T], _info| processor.process(out),
        move |_err| {
            err_stats.stream_errors.fetch_add(1, Ordering::Relaxed);
        },
        None,
    )?;
    Ok(stream)
}

/// Todo lo que vive en el hilo de audio. Se construye (y asigna) antes de arrancar el stream.
pub struct Processor {
    engine: Engine,
    commands: rtrb::Consumer<Command>,
    events: rtrb::Producer<Event>,
    meter: LoadMeter,
    scratch: Vec<f32>,
    channels: usize,
    stats: Arc<AudioStats>,
    fpu_ready: bool,
    /// Posición que no entró en la cola llena: se reintenta en el callback siguiente.
    pending_playhead: Option<Playhead>,
}

impl Processor {
    pub fn new(
        sample_rate: f32,
        channels: usize,
        commands: rtrb::Consumer<Command>,
        events: rtrb::Producer<Event>,
        stats: Arc<AudioStats>,
    ) -> Self {
        let channels = channels.max(1);
        Self {
            engine: Engine::new(sample_rate),
            commands,
            events,
            meter: LoadMeter::new(sample_rate),
            scratch: vec![0.0; SCRATCH_FRAMES * channels],
            channels,
            stats,
            fpu_ready: false,
            pending_playhead: None,
        }
    }

    pub fn process<T: SizedSample + FromSample<f32>>(&mut self, out: &mut [T]) {
        if !self.fpu_ready {
            // El hilo lo crea el backend: el modo del FPU se fija en el primer callback.
            dsp::enable_flush_to_zero();
            self.fpu_ready = true;
        }
        let start = Instant::now();
        while let Ok(cmd) = self.commands.pop() {
            self.engine.handle(cmd);
        }
        let channels = self.channels;
        for chunk in out.chunks_mut(self.scratch.len()) {
            let buf = &mut self.scratch[..chunk.len()];
            self.engine.render(buf, channels);
            for (dst, &src) in chunk.iter_mut().zip(buf.iter()) {
                *dst = T::from_sample(src);
            }
        }
        let frames = out.len() / channels;
        if let Some(p) = self.engine.take_playhead() {
            self.pending_playhead = Some(p);
        }
        if let Some(p) = self.pending_playhead
            && self.events.push(Event::Playhead(p)).is_ok()
        {
            self.pending_playhead = None;
        }
        let voices = self.engine.take_voices_peak();
        let notes = self.engine.take_notes_started();
        if let Some(report) = self.meter.record(start.elapsed(), frames, voices, notes) {
            // Si la interfaz no lee, se pierde el reporte: no hay que bloquear acá.
            let _ = self.events.push(Event::Load(report));
        }
        self.stats
            .last_frames
            .store(frames as u32, Ordering::Relaxed);
        self.stats.callbacks.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_path_does_not_allocate_and_reports_load() {
        let (mut cmd_tx, cmd_rx) = rtrb::RingBuffer::new(COMMAND_QUEUE);
        let (ev_tx, mut ev_rx) = rtrb::RingBuffer::new(EVENT_QUEUE);
        let stats = Arc::new(AudioStats::default());
        let mut p = Processor::new(48_000.0, 2, cmd_rx, ev_tx, stats.clone());
        let mut out_f32 = vec![0.0f32; 256 * 2];
        let mut out_i16 = vec![0i16; 256 * 2];
        let allocs = crate::alloc_check::count(|| {
            for i in 0..200u32 {
                let cmd = Command::Audition {
                    track: (i % 8) as u8,
                    notes: [48, 55, 60, 63],
                    len: 4,
                    velocity: 100,
                    gate_ticks: 12,
                };
                cmd_tx.push(cmd).unwrap();
                if i % 2 == 0 {
                    p.process(&mut out_f32);
                } else {
                    p.process(&mut out_i16);
                }
            }
        });
        assert_eq!(allocs, 0);
        assert_eq!(stats.callbacks.load(Ordering::Relaxed), 200);
        // 200 × 256 frames = 1,07 s: dos reportes.
        let mut reports = 0;
        while let Ok(Event::Load(r)) = ev_rx.pop() {
            assert!(r.cpu_avg > 0.0 && r.voices_max > 0 && r.frames == 256);
            reports += 1;
        }
        assert_eq!(reports, 2);
    }

    /// El camino completo de la audición: gesto X → `Audition` → comando → callback → reporte.
    #[test]
    fn listening_reaches_the_engine_and_is_reported() {
        use crate::app::App;
        use crate::gestures::Gesture;
        use crate::model::Project;

        let (mut cmd_tx, cmd_rx) = rtrb::RingBuffer::new(COMMAND_QUEUE);
        let (ev_tx, mut ev_rx) = rtrb::RingBuffer::new(EVENT_QUEUE);
        let mut p = Processor::new(48_000.0, 2, cmd_rx, ev_tx, Arc::new(AudioStats::default()));
        let mut app = App::new(Project::new_default());
        let mut mirror = crate::sync::EngineMirror::new();
        let mut out = vec![0.0f32; 4096 * 2];
        mirror.sync(&app.project, app.transport, &mut |c| cmd_tx.push(c).is_ok());
        app.memory[7].chord = crate::model::Chord {
            kind: crate::model::ChordKind::Major,
            inversion: 0,
        };
        for track in [0, 2, 7] {
            app.track = track;
            let a = app.handle(Gesture::Listen, 0).expect("X siempre audiciona");
            cmd_tx.push(crate::sync::audition_command(a)).unwrap();
            // Callbacks largos (4096 frames): el hi-hat termina adentro de uno.
            for _ in 0..3 {
                p.process(&mut out);
            }
        }
        // Cerrar la ventana de medición (0,5 s) del último sonido.
        for _ in 0..6 {
            p.process(&mut out);
        }
        let mut voices = 0;
        let mut notes = 0;
        while let Ok(ev) = ev_rx.pop() {
            if let Event::Load(r) = ev {
                voices = voices.max(r.voices_max);
                notes += r.notes;
            }
        }
        assert_eq!(
            notes,
            1 + 1 + 3,
            "bombo, hi-hat y acorde de 3 notas del sinte"
        );
        assert!(voices >= 3, "voces {voices}");
    }

    #[test]
    fn callback_with_the_sequencer_playing_and_live_edits_does_not_allocate() {
        use crate::engine::PlayMode;
        use crate::model::{NoteCell, Section};

        let (mut cmd_tx, cmd_rx) = rtrb::RingBuffer::new(COMMAND_QUEUE);
        let (ev_tx, mut ev_rx) = rtrb::RingBuffer::new(EVENT_QUEUE);
        let mut p = Processor::new(48_000.0, 2, cmd_rx, ev_tx, Arc::new(AudioStats::default()));
        let mut project = crate::demo::project();
        let mut mirror = crate::sync::EngineMirror::new();
        let play = Some(PlayMode::Song { from_row: 0 });
        assert!(mirror.sync(&project, play, &mut |c| cmd_tx.push(c).is_ok()));
        // Las pistas con frase en la fila 0 (SAMPLE no tiene).
        let tracks = [0, 1, 2, 3, 4, 6, 7];

        // Las ediciones se arman fuera de la medición; adentro solo corre el callback.
        let mut edits: Vec<Vec<Command>> = Vec::new();
        for i in 0..400usize {
            let sec = Section {
                row: i % 3,
                entry: i % 2,
            };
            let step = project.step_mut(tracks[i % 7], sec, i % 16).unwrap();
            step.note = if step.is_empty() {
                NoteCell::On(48 + (i % 24) as u8)
            } else {
                NoteCell::Empty
            };
            project.tempo_bpm = 100.0 + (i % 50) as f32;
            project.tracks[i % 8].mix.mute = i % 7 == 0;
            if i % 50 == 0 {
                // Edición de la canción y de una cadena mientras suena.
                let chain = project.chain_id_at(6, 1);
                project.set_song_cell(1, 6, chain.map(|c| c ^ 1)).unwrap();
                project
                    .set_entry_transpose(7, Section { row: 1, entry: 0 }, (i / 50) as i8)
                    .unwrap();
            }
            let mut batch = Vec::new();
            mirror.sync(&project, play, &mut |c| {
                batch.push(c);
                true
            });
            edits.push(batch);
        }
        let mut out = vec![0.0f32; 256 * 2];
        let mut playheads = 0;
        let allocs = crate::alloc_check::count(|| {
            for batch in &edits {
                for &c in batch {
                    cmd_tx.push(c).unwrap();
                }
                p.process(&mut out);
                while let Ok(ev) = ev_rx.pop() {
                    if matches!(ev, Event::Playhead(_)) {
                        playheads += 1;
                    }
                }
            }
        });
        assert_eq!(allocs, 0);
        // 400 × 256 frames = 2,1 s a 100–150 BPM: unos 16 pasos.
        assert!(playheads > 10, "{playheads} posiciones publicadas");
    }
    /// Regresión (reporte en la consola): Start en la Canción con una fila desalineada. Cada
    /// pista pasa a la fila siguiente al terminar su cadena, sin esperar a la más larga.
    #[test]
    fn start_in_song_moves_each_track_to_the_next_row() {
        use crate::app::{App, Screen};
        use crate::engine::PlayMode;
        use crate::gestures::Gesture;
        use crate::model::{Project, Section};

        let (mut cmd_tx, cmd_rx) = rtrb::RingBuffer::new(COMMAND_QUEUE);
        let (ev_tx, mut ev_rx) = rtrb::RingBuffer::new(EVENT_QUEUE);
        let mut p = Processor::new(48_000.0, 2, cmd_rx, ev_tx, Arc::new(AudioStats::default()));
        let mut project = Project::new_default();
        // Fila 0: la pista 0 con 3 frases, el resto con 1. Fila 1: una frase en cada pista.
        for entry in 1..3 {
            project
                .ensure_phrase(0, Section { row: 0, entry }, None)
                .unwrap();
        }
        for t in 0..crate::model::TRACKS {
            project
                .ensure_phrase(t, Section { row: 1, entry: 0 }, None)
                .unwrap();
        }
        assert!(project.row_misaligned(0));
        let mut app = App::new(project);
        app.screen = Screen::Song;
        app.handle(Gesture::Play, 0);
        assert_eq!(app.transport, Some(PlayMode::Song { from_row: 0 }));

        let mut mirror = crate::sync::EngineMirror::new();
        let mut out = vec![0.0f32; 256 * 2];
        // A 120 BPM una frase son 96 000 frames: se miran los cabezales a mitad de cada frase.
        let phrase = 96_000usize;
        let mut rows = Vec::new();
        let mut frame = 0usize;
        for k in 0..4 {
            while frame < k * phrase + phrase / 2 {
                mirror.sync(&app.project, app.transport, &mut |c| cmd_tx.push(c).is_ok());
                p.process(&mut out);
                frame += 256;
                while let Ok(ev) = ev_rx.pop() {
                    if let Event::Playhead(h) = ev {
                        app.set_playhead(h);
                    }
                }
            }
            let h = app.playhead.heads;
            rows.push(((h[0].row, h[0].entry), (h[1].row, h[1].entry)));
        }
        assert_eq!(
            rows,
            [
                ((0, 0), (0, 0)),
                ((0, 1), (1, 0)),
                ((0, 2), (0, 0)),
                ((1, 0), (1, 0)),
            ],
            "la pista 1 pasa a la fila 1 sin esperar a la 0; el bloque de 2 filas vuelve a empezar"
        );
    }
}
