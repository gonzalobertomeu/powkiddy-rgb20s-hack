//! Groovebox para la Powkiddy RGB20S. Hitos H1 (input, fuente, Grilla y Frase), H2 (motor),
//! H3 (secuenciador: loop de la sección), H5 (cadenas y canción: modos de reproducción,
//! pantallas Canción y Cadena, seguir reproducción) y H6 (guardar y abrir proyectos en RON).
//!
//! Subcomandos sin ventana ni audio (ver `offline.rs`):
//! - `groovebox render [carpeta]`: renderiza demos a WAV (la canción y el beat salen del secuenciador).
//! - `groovebox bench [segundos]`: mide el costo del motor (bolsa llena y otros escenarios).
//!
//! Variables de entorno (contrato en infra/README.md):
//! - `RGB20S_SIM=1`: corre en la Mac (ventana); si no, pantalla completa (consola).
//! - `RGB20S_SCALE`: escala entera de la ventana del simulador (defecto 1).
//! - `RGB20S_KEYMAP`: ruta del mapeo de teclado (si falta, se usa el incorporado).
//! - `GROOVEBOX_FRAMES`: período de audio pedido en frames (defecto 256).
//! - `GROOVEBOX_FONT=12`: arrancar con la fuente de 12×24.
//! - `GROOVEBOX_REPEAT=delay,intervalo`: repetición de la cruz en ms (defecto 250,33).
//! - `GROOVEBOX_AUDIO=0`: sin audio; `=null`: motor al ritmo real pero sin dispositivo.
//! - `GROOVEBOX_DEMO=1`: arrancar con el proyecto de demostración (`demo.rs`).
//! - `GROOVEBOX_DATA`: carpeta de datos (defecto `$XDG_DATA_HOME/groovebox` o
//!   `~/.local/share/groovebox`): `config.ron` y `proyectos/<nombre>/proyecto.ron`.
//! - `GROOVEBOX_SCRIPT=archivo`: guion de input y capturas (ver `script.rs`).
//! - `GROOVEBOX_POWER_SUPPLY`: carpeta de baterías (defecto `/sys/class/power_supply`).
//! - `RGB20S_AUDIO_STATE`: estado del servicio de audio (defecto `/run/rgb20s-audio/state`).

#[cfg(test)]
mod alloc_check;
mod app;
mod audio;
mod demo;
mod engine;
mod gestures;
mod input;
mod instrument;
mod modal;
mod model;
mod offline;
mod project_file;
mod script;
mod selection;
mod storage;
mod sync;
mod ui;

use std::collections::HashSet;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::WindowCanvas;
use sdl2::surface::Surface;

use app::{App, FontSize};
use engine::Event as EngineEvent;
use gestures::{GestureConfig, Gestures};
use input::{Button, Keymap, Sticks, Triggers};
use model::Project;
use rgb20s_common::audio_state::Mode;
use rgb20s_common::battery::{self, Battery};
use rgb20s_common::watch::{StateWatcher, Update};

const WIDTH: u32 = ui::WIDTH as u32;
const HEIGHT: u32 = ui::HEIGHT as u32;
/// Zona muerta de los sticks (spec-ui: 15 %, configurable a futuro en config.ron).
const STICK_DEAD_ZONE: f32 = 0.15;
/// Espera máxima sin eventos: los mensajes vencen y los sticks se redibujan a tiempo.
const IDLE_WAIT_MS: u64 = 100;
/// Con el secuenciador sonando: el cabezal se dibuja a lo sumo 8 ms tarde.
const PLAYING_WAIT_MS: u64 = 8;
/// Cada cuánto se escribe la carga del motor en el log.
const LOAD_LOG_MS: u64 = 10_000;
const BATTERY_REFRESH_MS: u64 = 15_000;
/// Cuánto se resalta el volumen del título después de una pulsación.
const VOLUME_HIGHLIGHT_MS: u64 = 1_500;

#[derive(Clone, Copy, PartialEq, Eq)]
enum AudioMode {
    Device,
    Null,
    Off,
}

struct Config {
    sim: bool,
    scale: u32,
    keymap: Keymap,
    audio_frames: u32,
    large_font: bool,
    gestures: GestureConfig,
    audio: AudioMode,
    demo: bool,
    script: Option<Vec<script::Step>>,
    power_supply: PathBuf,
}

fn env_parse<T: std::str::FromStr>(name: &str) -> Option<T> {
    std::env::var(name).ok()?.parse().ok()
}

impl Config {
    fn from_env() -> Self {
        let sim = std::env::var("RGB20S_SIM").is_ok_and(|v| v == "1");
        let scale = env_parse::<u32>("RGB20S_SCALE")
            .filter(|s| (1..=8).contains(s))
            .unwrap_or(1);
        let keymap = match std::env::var("RGB20S_KEYMAP") {
            Ok(path) => match Keymap::load(Path::new(&path)) {
                Ok(map) => {
                    println!("input: keymap {path} ({} teclas)", map.len());
                    map
                }
                Err(e) => {
                    println!("input: no pude leer el keymap ({e}); uso el incorporado");
                    Keymap::builtin()
                }
            },
            Err(_) => Keymap::builtin(),
        };
        let mut gestures = GestureConfig::default();
        if let Ok(v) = std::env::var("GROOVEBOX_REPEAT") {
            match v.split_once(',').map(|(a, b)| (a.parse(), b.parse())) {
                Some((Ok(delay), Ok(interval))) if interval > 0 => {
                    gestures.repeat_delay_ms = delay;
                    gestures.repeat_interval_ms = interval;
                }
                _ => println!("input: GROOVEBOX_REPEAT inválido ({v}); uso 250,33"),
            }
        }
        let script =
            std::env::var("GROOVEBOX_SCRIPT")
                .ok()
                .and_then(|path| {
                    match std::fs::read_to_string(&path)
                        .map_err(|e| e.to_string())
                        .and_then(|text| script::parse(&text))
                    {
                        Ok(steps) => {
                            println!("guion: {path} ({} pasos)", steps.len());
                            Some(steps)
                        }
                        Err(e) => {
                            println!("guion: no pude usar {path}: {e}");
                            None
                        }
                    }
                });
        Self {
            sim,
            scale,
            keymap,
            audio_frames: env_parse("GROOVEBOX_FRAMES").unwrap_or(audio::PREFERRED_FRAMES),
            large_font: env_parse::<u32>("GROOVEBOX_FONT") == Some(12),
            gestures,
            audio: match std::env::var("GROOVEBOX_AUDIO").as_deref() {
                Ok("0") => AudioMode::Off,
                Ok("null") => AudioMode::Null,
                _ => AudioMode::Device,
            },
            demo: std::env::var("GROOVEBOX_DEMO").is_ok_and(|v| v == "1"),
            script,
            power_supply: std::env::var_os("GROOVEBOX_POWER_SUPPLY")
                .filter(|v| !v.is_empty())
                .map_or_else(|| PathBuf::from(battery::POWER_SUPPLY_DIR), PathBuf::from),
        }
    }
}

/// Guarda lo que hay en el buffer del canvas (antes de `present`) como BMP.
fn save_screenshot(canvas: &WindowCanvas, path: &Path) -> Result<(), String> {
    let (w, h) = canvas.output_size()?;
    let mut pixels = canvas.read_pixels(None, PixelFormatEnum::RGB24)?;
    let surface = Surface::from_data(&mut pixels, w, h, w * 3, PixelFormatEnum::RGB24)?;
    surface.save_bmp(path)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("render") => offline::render(args.get(1).map(String::as_str)),
        Some("bench") => offline::bench(args.get(1).and_then(|s| s.parse().ok())),
        Some(other) => Err(format!("subcomando desconocido: {other} (render | bench)").into()),
        None => run(),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

/// Fuente de botones de esta vuelta del bucle: se juntan y después se pasan a `Gestures`.
enum ButtonChange {
    Press(Button),
    Release(Button),
}

/// Manda al motor los cambios del proyecto y del transporte. Lo que no entra en la cola se
/// reintenta en la vuelta siguiente del bucle.
fn sync_engine(out: &mut audio::AudioOutput, mirror: &mut sync::EngineMirror, app: &App) {
    mirror.sync(&app.project, app.transport, &mut |cmd| {
        out.commands.push(cmd).is_ok()
    });
}

/// Cierra el audio y lo vuelve a abrir releyendo la configuración de ALSA, para tomar el modo
/// mono/estéreo nuevo. El motor arranca de cero: quien llama tiene que reiniciar el `EngineMirror`
/// para que le mande el proyecto entero (si estaba sonando, vuelve a empezar la reproducción).
fn reopen_audio(audio: &mut Option<audio::AudioOutput>, frames: u32) -> Result<(), Box<dyn Error>> {
    // Primero se cierra el PCM viejo (se detiene y se une el hilo de cpal).
    drop(audio.take());
    audio::reload_config()?;
    *audio = Some(audio::start(frames)?);
    Ok(())
}

fn mode_message(mode: Mode) -> &'static str {
    match mode {
        Mode::Mono => "audio en mono (parlante)",
        Mode::Stereo => "audio en estéreo (auriculares)",
    }
}

/// Texto de la carga que se muestra: redibujar solo si cambia.
fn load_label(r: &engine::LoadReport) -> (u32, u32, u8) {
    (
        r.cpu_avg.round() as u32,
        r.cpu_peak.round() as u32,
        r.voices_max,
    )
}

fn run() -> Result<(), Box<dyn Error>> {
    let config = Config::from_env();

    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let v = sdl2::version::version();
    println!(
        "sdl: versión linkeada {}.{}.{} ({}), driver de video {}",
        v.major,
        v.minor,
        v.patch,
        sdl2::version::revision(),
        video.current_video_driver()
    );

    let window = if config.sim {
        video
            .window("groovebox", WIDTH * config.scale, HEIGHT * config.scale)
            .position_centered()
            .build()?
    } else {
        // En la consola (KMSDRM) se usa el modo actual del panel; set_logical_size escala si no es 640x480.
        sdl.mouse().show_cursor(false);
        video
            .window("groovebox", WIDTH, HEIGHT)
            .fullscreen_desktop()
            .build()?
    };
    let mut canvas = window.into_canvas().present_vsync().build()?;
    canvas.set_logical_size(WIDTH, HEIGHT)?;
    canvas.set_integer_scale(true)?;
    let (ow, oh) = canvas.output_size()?;
    println!(
        "sdl: ventana {}x{} (render {ow}x{oh}), lógico {WIDTH}x{HEIGHT}, sim={}",
        canvas.window().size().0,
        canvas.window().size().1,
        config.sim
    );
    let creator = canvas.texture_creator();
    let mut fonts = ui::Fonts::new(&creator)?;

    let pads = sdl.game_controller()?;
    let joysticks = sdl.joystick()?;
    let mut open_pads = Vec::new();
    let mut open_joys = Vec::new();
    // Joysticks sin mapeo de GameController: se loguean sus botones crudos para armar el mapeo.
    let mut raw_joy_ids = HashSet::new();

    // Sin audio la app sigue (sirve para `just run-arm`, que no tiene placa de sonido).
    let started_audio = match config.audio {
        AudioMode::Off => {
            println!("audio: deshabilitado (GROOVEBOX_AUDIO=0)");
            None
        }
        AudioMode::Null => Some(audio::start_null(config.audio_frames)),
        AudioMode::Device => Some(audio::start(config.audio_frames)),
    };
    let mut audio = match started_audio {
        Some(Ok(out)) => Some(out),
        Some(Err(e)) => {
            println!("audio: deshabilitado ({e})");
            None
        }
        None => None,
    };

    let data = storage::DataDir::from_env();
    println!("datos: {}", data.root().display());
    let app_config = data.load_config().unwrap_or_else(|e| {
        println!("datos: config.ron ilegible ({e}); uso la configuración por defecto");
        storage::AppConfig::default()
    });
    let mut app = App::new(Project::new_default());
    app.attach_data(data, app_config);
    if config.demo {
        println!("proyecto: demo (GROOVEBOX_DEMO=1)");
        app.project = demo::project();
    } else {
        app.open_last_or_new();
        println!("proyecto: {}", app.project.name);
    }
    if config.large_font {
        app.font = FontSize::Large;
    }
    let mut gestures = Gestures::new(config.gestures);
    let mut sticks = Sticks::new(STICK_DEAD_ZONE);
    let mut triggers = Triggers::default();
    let mut events = sdl.event_pump()?;
    let mut battery: Option<Battery> = battery::read(&config.power_supply);
    let mut battery_at = 0;
    let mut audio_state = StateWatcher::from_env();
    let mut highlight_until: Option<u64> = None;
    let started = Instant::now();
    // Para el log del período real: se reinicia al reabrir el audio.
    let mut audio_started = Instant::now();
    let now_ms = || started.elapsed().as_millis() as u64;
    let mut logged_period = false;
    let mut reported_errors = 0;
    let mut pending = Vec::new();
    let mut changes = Vec::new();
    let mut dirty = true;
    let mut drawn_message: Option<u64> = None;
    let mut script = config.script.map(script::Runner::new);
    let mut shot: Option<std::path::PathBuf> = None;
    let mut mirror = sync::EngineMirror::new();
    // Peor reporte desde la última línea de log.
    let mut worst_load: Option<engine::LoadReport> = None;
    let mut last_load_log = 0;

    while !app.quit {
        let now = now_ms();
        let deadline = [
            gestures.next_deadline(),
            script.as_ref().and_then(script::Runner::next_deadline),
            highlight_until,
            Some(audio_state.next_poll()),
        ]
        .into_iter()
        .flatten()
        .min();
        let idle = if app.transport.is_some() || app.playhead.playing {
            PLAYING_WAIT_MS
        } else {
            IDLE_WAIT_MS
        };
        let wait = deadline
            .map_or(idle, |d| d.saturating_sub(now).min(idle))
            .max(1);
        let first = events.wait_event_timeout(wait as u32);

        for event in first.into_iter().chain(events.poll_iter()) {
            match event {
                Event::Quit { .. } => app.quit = true,
                // Atajo del simulador: Esc sale (en la consola es FN, que la app no usa). Con
                // cambios sin guardar pregunta; un segundo Esc sale igual.
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    repeat: false,
                    ..
                } if config.sim => {
                    if app.confirming_quit() {
                        app.quit = true;
                    } else {
                        app.request_quit();
                    }
                    dirty = true;
                }
                Event::KeyDown {
                    keycode: Some(key),
                    repeat: false,
                    ..
                } => {
                    if let Some(b) = config.keymap.button(key) {
                        changes.push(ButtonChange::Press(b));
                    } else if let Some((stick, dir)) = config.keymap.stick(key) {
                        sticks.set_key(stick, dir, true);
                        dirty = true;
                    }
                }
                Event::KeyUp {
                    keycode: Some(key), ..
                } => {
                    if let Some(b) = config.keymap.button(key) {
                        changes.push(ButtonChange::Release(b));
                    } else if let Some((stick, dir)) = config.keymap.stick(key) {
                        sticks.set_key(stick, dir, false);
                        dirty = true;
                    }
                }
                Event::ControllerDeviceAdded { which, .. } => match pads.open(which) {
                    Ok(pad) => {
                        println!("input: gamepad \"{}\" (índice {which})", pad.name());
                        open_pads.push(pad);
                    }
                    Err(e) => println!("input: no pude abrir el gamepad {which}: {e}"),
                },
                Event::JoyDeviceAdded { which, .. } if !pads.is_game_controller(which) => {
                    match joysticks.open(which) {
                        Ok(joy) => {
                            println!(
                                "input: joystick sin mapeo \"{}\" ({} botones, {} ejes)",
                                joy.name(),
                                joy.num_buttons(),
                                joy.num_axes()
                            );
                            raw_joy_ids.insert(joy.instance_id());
                            open_joys.push(joy);
                        }
                        Err(e) => println!("input: no pude abrir el joystick {which}: {e}"),
                    }
                }
                Event::ControllerButtonDown { button, .. } => {
                    println!("input: pad {button:?}");
                    if let Some(b) = Button::from_pad(button) {
                        changes.push(ButtonChange::Press(b));
                    }
                }
                Event::ControllerButtonUp { button, .. } => {
                    if let Some(b) = Button::from_pad(button) {
                        changes.push(ButtonChange::Release(b));
                    }
                }
                Event::ControllerAxisMotion { axis, value, .. } => {
                    if let Some((b, down)) = triggers.update(axis, value) {
                        changes.push(if down {
                            ButtonChange::Press(b)
                        } else {
                            ButtonChange::Release(b)
                        });
                    } else if let Some((stick, vertical)) = Sticks::axis_of(axis) {
                        sticks.set_axis(stick, vertical, value);
                        dirty = true;
                    }
                }
                Event::JoyButtonDown {
                    which, button_idx, ..
                } if raw_joy_ids.contains(&which) => {
                    println!("input: joystick sin mapeo, botón {button_idx}");
                }
                Event::Window { .. } => dirty = true,
                _ => {}
            }
        }

        let now = now_ms();
        if let Some(runner) = &mut script {
            for step in runner.due(now) {
                match step {
                    script::Step::Press(b) => changes.push(ButtonChange::Press(b)),
                    script::Step::Release(b) => changes.push(ButtonChange::Release(b)),
                    script::Step::Shot(path) => shot = Some(path),
                    script::Step::Quit => app.quit = true,
                    script::Step::Wait(_) => {}
                }
            }
        }
        for change in changes.drain(..) {
            match change {
                ButtonChange::Press(b) => gestures.press(b, now, &mut pending),
                ButtonChange::Release(b) => gestures.release(b, now),
            }
            dirty = true;
        }
        gestures.tick(now, &mut pending);
        for g in pending.drain(..) {
            let audition = app.handle(g, now);
            if let Some(out) = audio.as_mut() {
                // Primero los cambios del proyecto (pasos, tempo, instrumentos), después la audición.
                sync_engine(out, &mut mirror, &app);
                if let Some(a) = audition
                    && out.commands.push(sync::audition_command(a)).is_err()
                {
                    println!("audio: cola de comandos llena");
                }
            }
            dirty = true;
        }

        if now >= battery_at + BATTERY_REFRESH_MS {
            battery_at = now;
            let fresh = battery::read(&config.power_supply);
            dirty |= fresh != battery;
            battery = fresh;
        }
        match audio_state.poll(now) {
            Some(Update::Changed { mode, .. }) => {
                highlight_until = Some(now + VOLUME_HIGHLIGHT_MS);
                dirty = true;
                if mode && let Some(state) = audio_state.state() {
                    let mut text = mode_message(state.mode).to_string();
                    let mut warn = false;
                    if config.audio == AudioMode::Device {
                        let t0 = Instant::now();
                        match reopen_audio(&mut audio, config.audio_frames) {
                            Ok(()) => {
                                println!(
                                    "audio: reabierto en {} ({} ms)",
                                    state.mode,
                                    t0.elapsed().as_millis()
                                );
                                mirror = sync::EngineMirror::new();
                                reported_errors = 0;
                                logged_period = false;
                                audio_started = Instant::now();
                            }
                            Err(e) => {
                                println!("audio: no pude reabrirlo en {} ({e})", state.mode);
                                text = format!("no pude reabrir el audio: {e}");
                                warn = true;
                            }
                        }
                    }
                    app.notify(text, warn, now);
                }
            }
            Some(Update::Appeared | Update::Gone) => dirty = true,
            None => {}
        }
        if highlight_until.is_some_and(|until| now >= until) {
            highlight_until = None;
            dirty = true;
        }

        if let Some(out) = audio.as_mut() {
            sync_engine(out, &mut mirror, &app);
            while let Ok(event) = out.events.pop() {
                match event {
                    EngineEvent::Load(r) => {
                        if app.audio_load.as_ref().map(load_label) != Some(load_label(&r)) {
                            dirty = true;
                        }
                        app.audio_load = Some(r);
                        let worst = worst_load.get_or_insert(engine::LoadReport::default());
                        worst.cpu_avg = worst.cpu_avg.max(r.cpu_avg);
                        worst.cpu_peak = worst.cpu_peak.max(r.cpu_peak);
                        worst.voices_max = worst.voices_max.max(r.voices_max);
                        worst.notes += r.notes;
                    }
                    EngineEvent::Playhead(p) => {
                        if app.playhead != p {
                            app.set_playhead(p);
                            dirty = true;
                        }
                    }
                }
            }
            if now >= last_load_log + LOAD_LOG_MS
                && let Some(w) = worst_load.take()
            {
                last_load_log = now;
                println!(
                    "audio: carga en los últimos {} s: CPU prom. máx. {:.1} %, pico {:.1} %, voces {}, notas {}",
                    LOAD_LOG_MS / 1000,
                    w.cpu_avg,
                    w.cpu_peak,
                    w.voices_max,
                    w.notes
                );
            }
            if !logged_period && audio_started.elapsed() > Duration::from_millis(500) {
                logged_period = true;
                let frames = out.stats.last_frames.load(Ordering::Relaxed);
                let calls = out.stats.callbacks.load(Ordering::Relaxed);
                println!("audio: período real {frames} frames ({calls} callbacks en 0,5 s)");
            }
            let errors = out.stats.stream_errors.load(Ordering::Relaxed);
            if errors != reported_errors {
                println!("audio: {errors} errores del stream (xruns u otros)");
                reported_errors = errors;
            }
        }

        // Redibujar solo si cambió algo o si venció el mensaje que está en pantalla.
        let message = app.message_at(now).map(|m| m.at_ms);
        if message != drawn_message {
            dirty = true;
        }
        if (dirty || shot.is_some()) && !app.quit {
            let view = ui::View {
                held: gestures.held(),
                sticks: [
                    sticks.value(input::Stick::Left),
                    sticks.value(input::Stick::Right),
                ],
                now,
                system: ui::SystemStatus {
                    battery,
                    audio: audio_state.state(),
                    highlight: highlight_until.is_some(),
                },
            };
            ui::draw(&mut canvas, &mut fonts, &app, &view)?;
            if let Some(path) = shot.take() {
                match save_screenshot(&canvas, &path) {
                    Ok(()) => println!("guion: captura {}", path.display()),
                    Err(e) => println!("guion: no pude guardar {}: {e}", path.display()),
                }
            }
            canvas.present();
            drawn_message = message;
            dirty = false;
        }
    }

    println!("salida limpia");
    Ok(())
}
