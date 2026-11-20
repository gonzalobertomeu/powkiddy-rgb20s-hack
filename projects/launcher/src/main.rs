//! Launcher de la RGB20S: lista las apps de `apps.toml` más Reiniciar y Apagar. No lanza nada:
//! al elegir, cierra SDL (libera DRM y el gamepad), escribe la elección y sale; la ejecuta el
//! wrapper de sesión (`system/rgb20s-session`). Formato de la elección: `apps::Choice::encode`.
//!
//! Uso: `launcher [--choice-file <ruta>]`. Sin `--choice-file`, imprime la elección (simulador).
//!
//! Códigos de salida: 0 = eligió (y escribió la elección), 1 = error, 2 = uso incorrecto,
//! 3 = salió sin elegir (ventana cerrada, SIGTERM, Esc en el simulador).
//!
//! Variables de entorno:
//! - `RGB20S_SIM=1`, `RGB20S_SCALE`, `RGB20S_KEYMAP`: igual que el groovebox (infra/README.md).
//! - `LAUNCHER_APPS`: ruta de `apps.toml` (si no, ver `apps::apps_candidates`).
//! - `LAUNCHER_REPEAT=delay,intervalo`: repetición de la cruceta en ms (defecto 300,90).
//! - `LAUNCHER_POWER_SUPPLY`: carpeta de baterías (defecto `/sys/class/power_supply`).
//! - `LAUNCHER_SHOT=archivo.bmp`: guarda una captura del primer cuadro y sale (código 3).
//! - `LAUNCHER_INPUT=down,down,a`: toques al arrancar (nombres de `infra/sim/keymap.toml`), para
//!   probar sin teclado; se aplican antes del primer cuadro.

mod apps;
mod battery;
mod font;
mod input;
mod nav;
mod ui;

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::WindowCanvas;
use sdl2::surface::Surface;

use apps::{AppEntry, Choice, Entry};
use input::{Button, Keymap};
use nav::{Menu, Repeat};

const WIDTH: u32 = ui::WIDTH as u32;
const HEIGHT: u32 = ui::HEIGHT as u32;
const EXIT_USAGE: u8 = 2;
const EXIT_NO_CHOICE: u8 = 3;
/// Espera máxima sin eventos: alcanza para refrescar la batería.
const IDLE_WAIT_MS: u64 = 1_000;
const BATTERY_REFRESH_MS: u64 = 15_000;
/// Cuadros extra después de cada cambio (ver `flush` en el bucle) y espera entre ellos.
const FLUSH_FRAMES: u32 = 2;
const FLUSH_WAIT_MS: u64 = 16;

struct Args {
    choice_file: Option<PathBuf>,
}

impl Args {
    fn parse(mut args: impl Iterator<Item = OsString>) -> Result<Self, String> {
        let mut choice_file = None;
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--choice-file") => {
                    let path = args.next().ok_or("--choice-file necesita una ruta")?;
                    choice_file = Some(PathBuf::from(path));
                }
                Some("-h" | "--help") => return Err(String::new()),
                _ => return Err(format!("argumento desconocido: {}", arg.to_string_lossy())),
            }
        }
        Ok(Self { choice_file })
    }
}

struct Config {
    sim: bool,
    scale: u32,
    keymap: Keymap,
    repeat_delay_ms: u64,
    repeat_interval_ms: u64,
    power_supply: PathBuf,
    shot: Option<PathBuf>,
    input: Vec<Button>,
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

impl Config {
    fn from_env() -> Self {
        let sim = std::env::var("RGB20S_SIM").is_ok_and(|v| v == "1");
        let scale = std::env::var("RGB20S_SCALE")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|s| (1..=8).contains(s))
            .unwrap_or(1);
        let keymap = match env_path("RGB20S_KEYMAP") {
            Some(path) => match Keymap::load(&path) {
                Ok(map) => {
                    println!("input: keymap {} ({} teclas)", path.display(), map.len());
                    map
                }
                Err(e) => {
                    println!("input: no pude leer el keymap ({e}); uso el incorporado");
                    Keymap::builtin()
                }
            },
            None => Keymap::builtin(),
        };
        let (mut repeat_delay_ms, mut repeat_interval_ms) = (300, 90);
        if let Ok(v) = std::env::var("LAUNCHER_REPEAT") {
            match v.split_once(',').map(|(a, b)| (a.parse(), b.parse())) {
                Some((Ok(d), Ok(i))) if i > 0 => (repeat_delay_ms, repeat_interval_ms) = (d, i),
                _ => println!("input: LAUNCHER_REPEAT inválido ({v}); uso 300,90"),
            }
        }
        Self {
            sim,
            scale,
            keymap,
            repeat_delay_ms,
            repeat_interval_ms,
            power_supply: env_path("LAUNCHER_POWER_SUPPLY")
                .unwrap_or_else(|| PathBuf::from(battery::POWER_SUPPLY_DIR)),
            shot: env_path("LAUNCHER_SHOT"),
            input: std::env::var("LAUNCHER_INPUT")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .filter_map(|n| {
                    let b = Button::from_config_name(n);
                    if b.is_none() {
                        println!("input: LAUNCHER_INPUT: botón desconocido {n:?}");
                    }
                    b
                })
                .collect(),
        }
    }
}

/// Apps de la primera candidata que exista. Si falla, la lista queda vacía (siguen Reiniciar y
/// Apagar): el error completo va al log y su primera línea, a la pantalla.
fn load_apps() -> (Vec<AppEntry>, Option<String>) {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let candidates = apps::apps_candidates(env_path("LAUNCHER_APPS"), exe_dir.as_deref());
    let Some(path) = candidates.iter().find(|p| p.is_file()) else {
        let tried: Vec<_> = candidates.iter().map(|p| p.display().to_string()).collect();
        println!("apps: no encontré apps.toml (busqué {})", tried.join(", "));
        return (Vec::new(), Some("no encontré apps.toml".into()));
    };
    match apps::load_apps(path) {
        Ok(list) => {
            println!("apps: {} ({} apps)", path.display(), list.len());
            (list, None)
        }
        Err(e) => {
            println!("apps: {}: {e}", path.display());
            let first = e.lines().next().unwrap_or_default();
            (Vec::new(), Some(format!("apps.toml inválido: {first}")))
        }
    }
}

fn last_choice_path() -> Option<PathBuf> {
    apps::last_choice_path(env_path("XDG_STATE_HOME"), env_path("HOME"))
}

fn remember(name: &str) {
    let Some(path) = last_choice_path() else {
        return;
    };
    let result = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| apps::write_atomic(&path, format!("{name}\n").as_bytes()));
    if let Err(e) = result {
        println!("estado: no pude guardar {}: {e}", path.display());
    }
}

fn save_screenshot(canvas: &WindowCanvas, path: &Path) -> Result<(), String> {
    let (w, h) = canvas.output_size()?;
    let mut pixels = canvas.read_pixels(None, PixelFormatEnum::RGB24)?;
    let surface = Surface::from_data(&mut pixels, w, h, w * 3, PixelFormatEnum::RGB24)?;
    surface.save_bmp(path)
}

fn main() -> ExitCode {
    let args = match Args::parse(std::env::args_os().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("error: {e}");
            }
            eprintln!("uso: launcher [--choice-file <ruta>]");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let config = Config::from_env();
    let (apps, load_error) = load_apps();
    let entries = apps::entries(&apps);
    let last = last_choice_path().and_then(|p| std::fs::read_to_string(p).ok());
    let selected = apps::initial_selection(&entries, &apps, last.as_deref());

    // Al volver de `run`, SDL ya está cerrado: el wrapper puede lanzar otra app con DRM libre.
    let entry = match run(&config, &apps, &entries, selected, load_error) {
        Ok(Some(entry)) => entry,
        Ok(None) => {
            println!("salida sin elegir");
            return ExitCode::from(EXIT_NO_CHOICE);
        }
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let choice = match entry {
        Entry::App(i) => Choice::Run(apps[i].launch()),
        Entry::Reboot => Choice::Reboot,
        Entry::Poweroff => Choice::Poweroff,
    };
    if let Some(name) = apps::remembered_name(entry, &apps) {
        remember(name);
    }
    println!("elección: {}", choice.describe());
    if let Some(path) = &args.choice_file
        && let Err(e) = apps::write_atomic(path, &choice.encode())
    {
        eprintln!("error: no pude escribir {}: {e}", path.display());
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Interfaz. Devuelve la entrada elegida, o `None` si se cerró sin elegir.
fn run(
    config: &Config,
    apps: &[AppEntry],
    entries: &[Entry],
    selected: usize,
    mut message: Option<String>,
) -> Result<Option<Entry>, String> {
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    println!("sdl: driver de video {}", video.current_video_driver());
    let window = if config.sim {
        video
            .window("launcher", WIDTH * config.scale, HEIGHT * config.scale)
            .position_centered()
            .build()
    } else {
        sdl.mouse().show_cursor(false);
        video
            .window("launcher", WIDTH, HEIGHT)
            .fullscreen_desktop()
            .build()
    }
    .map_err(|e| e.to_string())?;
    let mut canvas = window
        .into_canvas()
        .present_vsync()
        .build()
        .map_err(|e| e.to_string())?;
    canvas
        .set_logical_size(WIDTH, HEIGHT)
        .map_err(|e| e.to_string())?;
    canvas.set_integer_scale(true)?;
    let creator = canvas.texture_creator();
    let mut fonts = ui::Fonts::new(&creator)?;

    let pads = sdl.game_controller()?;
    let joysticks = sdl.joystick()?;
    let mut open_pads = Vec::new();
    let mut open_joys = Vec::new();
    let mut raw_joy_ids = HashSet::new();
    let mut events = sdl.event_pump()?;

    let available: Vec<bool> = entries
        .iter()
        .map(|e| match e {
            Entry::App(i) => apps[*i].is_available(),
            _ => true,
        })
        .collect();
    let mut menu = Menu::new(entries.len(), selected);
    let mut repeat = Repeat::new(config.repeat_delay_ms, config.repeat_interval_ms);
    let started = Instant::now();
    let now_ms = || started.elapsed().as_millis() as u64;
    let mut battery = battery::read(&config.power_supply);
    let mut battery_at = 0;
    let mut dirty = true;
    // Con KMSDRM, SDL muestra cada cuadro recién en el `present` siguiente: después de cada
    // cambio se dibujan FLUSH_FRAMES cuadros más para que el último estado llegue a la pantalla.
    let mut flush = 0u32;
    // Botones de `LAUNCHER_INPUT`: se procesan como toques en la primera vuelta.
    let mut presses = config.input.clone();
    let mut injected = !presses.is_empty();
    let mut releases = Vec::new();

    loop {
        let now = now_ms();
        let deadline = repeat
            .deadline()
            .unwrap_or(u64::MAX)
            .min(battery_at + BATTERY_REFRESH_MS);
        let wait = if flush > 0 {
            FLUSH_WAIT_MS
        } else {
            deadline.saturating_sub(now).clamp(1, IDLE_WAIT_MS)
        };
        let first = events.wait_event_timeout(wait as u32);

        for event in first.into_iter().chain(events.poll_iter()) {
            match event {
                Event::Quit { .. } => return Ok(None),
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } if config.sim => return Ok(None),
                Event::KeyDown {
                    keycode: Some(key),
                    repeat: false,
                    ..
                } => presses.extend(config.keymap.button(key)),
                Event::KeyUp {
                    keycode: Some(key), ..
                } => releases.extend(config.keymap.button(key)),
                Event::ControllerButtonDown { button, .. } => {
                    presses.extend(Button::from_pad(button));
                }
                Event::ControllerButtonUp { button, .. } => {
                    releases.extend(Button::from_pad(button));
                }
                Event::ControllerDeviceAdded { which, .. } => match pads.open(which) {
                    Ok(pad) => {
                        println!("input: gamepad \"{}\"", pad.name());
                        open_pads.push(pad);
                    }
                    Err(e) => println!("input: no pude abrir el gamepad {which}: {e}"),
                },
                Event::JoyDeviceAdded { which, .. } if !pads.is_game_controller(which) => {
                    match joysticks.open(which) {
                        Ok(joy) => {
                            println!("input: joystick sin mapeo \"{}\"", joy.name());
                            raw_joy_ids.insert(joy.instance_id());
                            open_joys.push(joy);
                        }
                        Err(e) => println!("input: no pude abrir el joystick {which}: {e}"),
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
        for b in releases.drain(..) {
            match b {
                Button::Up => repeat.release(-1),
                Button::Down => repeat.release(1),
                _ => {}
            }
        }
        for b in presses.drain(..) {
            let delta = match b {
                Button::Up => -1,
                Button::Down => 1,
                Button::A => {
                    let i = menu.selected();
                    if available[i] {
                        return Ok(Some(entries[i]));
                    }
                    if let Entry::App(a) = entries[i] {
                        message = Some(format!("no existe: {}", apps[a].command));
                        dirty = true;
                    }
                    continue;
                }
                _ => continue,
            };
            menu.step(delta, true);
            repeat.press(delta, now);
            dirty = true;
        }
        if injected {
            injected = false;
            repeat = Repeat::new(config.repeat_delay_ms, config.repeat_interval_ms);
        }
        if let Some(delta) = repeat.tick(now) {
            dirty |= menu.step(delta, false);
        }
        if now >= battery_at + BATTERY_REFRESH_MS {
            battery_at = now;
            let fresh = battery::read(&config.power_supply);
            dirty |= fresh != battery;
            battery = fresh;
        }

        if dirty || flush > 0 {
            if dirty {
                flush = FLUSH_FRAMES;
            } else {
                flush -= 1;
            }
            let screen = ui::Screen {
                apps,
                entries,
                available: &available,
                selected: menu.selected(),
                battery,
                message: message.as_deref(),
            };
            ui::draw(&mut canvas, &mut fonts, &screen)?;
            if let Some(path) = &config.shot {
                save_screenshot(&canvas, path)?;
                println!("captura: {}", path.display());
                return Ok(None);
            }
            canvas.present();
            dirty = false;
        }
    }
}
