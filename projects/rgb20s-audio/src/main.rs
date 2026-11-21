//! rgb20s-audio: servicio de sistema para el audio de la RGB20S.
//!
//! - Botones de volumen → `Master Playback Volume` en pasos de dB, con la repetición del kernel.
//! - Los dos botones a la vez alternan mono/estéreo en el `default` de ALSA (para todas las apps).
//! - Fuerza `Playback Mux` = HP (la salida de clase D no está conectada).
//! - Guarda volumen (`alsactl store`) y modo, y opcionalmente sigue al jack de auriculares.
//! - Publica volumen y modo para las apps en `/run/rgb20s-audio/state` (`rgb20s_common::audio_state`).
//!
//! No abre ningún PCM. Los logs van a stderr (journal), uno por cambio y no por repetición.

mod alsa_conf;
mod config;
mod evdev;
mod files;
mod gesture;
mod hw;
mod mode;
mod volume;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use config::Config;
use gesture::{Action, Gesture, KeyAction, VolumeKey};
use hw::{Jack, Mixer};
use mode::Mode;
use rgb20s_common::audio_state::{self, AudioState};

const USAGE: &str = "uso: rgb20s-audio [run] [--config ARCHIVO]
       rgb20s-audio alsa-conf mono|stereo [--config ARCHIVO]

  run        servicio (por defecto): botones de volumen, modo mono/estéreo, jack
  alsa-conf  imprime la configuración de ALSA que se escribe para ese modo
  --config   por defecto /etc/rgb20s-audio.toml (si no existe, valores por defecto)";

enum Event {
    Key(VolumeKey, KeyAction),
    /// Se perdió el dispositivo de entrada: el estado de los botones ya no vale.
    InputLost,
    /// Valor crudo del control del jack.
    Jack(bool),
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut positional = Vec::new();
    let mut config_path = PathBuf::from(config::DEFAULT_PATH);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => match args.next() {
                Some(path) => config_path = path.into(),
                None => return usage_error("falta el archivo de --config"),
            },
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            _ => positional.push(arg),
        }
    }

    let config = match Config::load(&config_path) {
        Ok((config, found)) => {
            if !found && matches!(positional.first().map(String::as_str), None | Some("run")) {
                eprintln!("sin {}: uso los valores por defecto", config_path.display());
            }
            config
        }
        Err(e) => {
            eprintln!("configuración inválida: {e}");
            return ExitCode::FAILURE;
        }
    };

    match positional.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        [] | ["run"] => run(config),
        ["alsa-conf", mode] => match mode.parse::<Mode>() {
            Ok(mode) => {
                print!("{}", alsa_conf::render(mode, &config.pcm_slave()));
                ExitCode::SUCCESS
            }
            Err(e) => usage_error(&e),
        },
        _ => usage_error("argumentos inválidos"),
    }
}

fn usage_error(msg: &str) -> ExitCode {
    eprintln!("{msg}\n{USAGE}");
    ExitCode::from(2)
}

fn run(config: Config) -> ExitCode {
    let mut service = match Service::start(config) {
        Ok(service) => service,
        Err(e) => {
            eprintln!("no pude arrancar: {e}");
            return ExitCode::FAILURE;
        }
    };
    service.run();
    ExitCode::FAILURE
}

struct Service {
    config: Config,
    mixer: Mixer,
    scale: volume::VolumeScale,
    gesture: Gesture,
    mode: Mode,
    /// Momento del `alsactl store` pendiente (se corre el plazo con cada cambio).
    store_at: Option<Instant>,
    events: mpsc::Receiver<Event>,
    /// Estado para las apps y su contador de publicaciones.
    state_path: PathBuf,
    seq: u64,
}

impl Service {
    fn start(config: Config) -> Result<Service, String> {
        let mixer = open_mixer(&config)?;
        let scale = mixer.volume_scale()?;
        eprintln!(
            "inicio: tarjeta {}, {} {}..{} ({:.1}..{:.1} dB), paso {} dB, modo por defecto {}",
            config.ctl_name(),
            config.volume_control,
            scale.raw_min,
            scale.raw_max,
            scale.db_min,
            scale.db_max,
            config.volume_step_db,
            config.default_mode
        );

        let mut store_at = None;
        let mux = &config.mux_control;
        if !mux.is_empty() {
            match mixer.force_enum(mux, &config.mux_item) {
                Ok(true) => {
                    eprintln!("{mux} → {}", config.mux_item);
                    store_at = Some(Instant::now() + config.store_delay());
                }
                Ok(false) => {}
                Err(e) => eprintln!("no pude fijar {mux}: {e}"),
            }
        }

        let mode = match files::load_mode(&config.mode_file()) {
            Ok(Some(mode)) => mode,
            Ok(None) => config.default_mode,
            Err(e) => {
                eprintln!(
                    "{}: {e}; uso {}",
                    config.mode_file().display(),
                    config.default_mode
                );
                config.default_mode
            }
        };

        let (tx, events) = mpsc::channel();
        spawn_input(config.input_device.clone(), tx.clone());
        if config.jack.enabled {
            spawn_jack(config.ctl_name(), config.jack.control.clone(), tx);
        }

        let mut service = Service {
            gesture: Gesture::new(config.gesture_window()),
            config,
            mixer,
            scale,
            mode,
            store_at,
            events,
            state_path: audio_state::state_path(),
            seq: 0,
        };
        // Siempre se reescribe: deja el archivo de ALSA coherente con el estado guardado.
        service.write_mode()?;
        eprintln!("modo: {mode}");
        service.publish();
        Ok(service)
    }

    fn run(&mut self) {
        loop {
            let deadline = [self.gesture.deadline(), self.store_at]
                .into_iter()
                .flatten()
                .min();
            let event = match deadline {
                Some(at) => self
                    .events
                    .recv_timeout(at.saturating_duration_since(Instant::now())),
                None => self
                    .events
                    .recv()
                    .map_err(|_| RecvTimeoutError::Disconnected),
            };
            let now = Instant::now();
            if let Some(action) = self.gesture.poll(now) {
                self.apply(action);
            }
            match event {
                Ok(Event::Key(key, action)) => {
                    if let Some(action) = self.gesture.on_key(key, action, now) {
                        self.apply(action);
                    }
                }
                Ok(Event::InputLost) => self.gesture.reset(),
                Ok(Event::Jack(value)) => self.on_jack(value),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    eprintln!("se cerraron los hilos de entrada");
                    return;
                }
            }
            if self.store_at.is_some_and(|at| at <= Instant::now()) {
                self.store_at = None;
                self.store_volume();
            }
        }
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Step(key) => self.step_volume(key == VolumeKey::Up),
            Action::ToggleMode => {
                let mode = self.mode.toggled();
                self.set_mode(mode, "botones");
            }
        }
    }

    fn step_volume(&mut self, up: bool) {
        let result = self.mixer.volume().and_then(|raw| {
            let next = self.scale.step(raw, up, self.config.volume_step_db);
            if next != raw {
                self.mixer.set_volume(next)?;
            }
            Ok(next != raw)
        });
        match result {
            Ok(changed) => {
                if changed {
                    self.store_at = Some(Instant::now() + self.config.store_delay());
                }
                // También en los extremos: la app muestra la barra igual.
                self.publish();
            }
            Err(e) => {
                eprintln!("volumen: {e}; reabro el mixer");
                match open_mixer(&self.config) {
                    Ok(mixer) => self.mixer = mixer,
                    Err(e) => eprintln!("no pude reabrir el mixer: {e}"),
                }
            }
        }
    }

    fn store_volume(&self) {
        match self.mixer.volume() {
            Ok(raw) => eprintln!(
                "volumen: {:.1} dB ({raw}/{})",
                self.scale.db_of(raw),
                self.scale.raw_max
            ),
            Err(e) => eprintln!("volumen: {e}"),
        }
        match Command::new(&self.config.alsactl).arg("store").status() {
            Ok(status) if status.success() => {}
            Ok(status) => eprintln!("alsactl store terminó con {status}"),
            Err(e) => eprintln!("no pude correr {}: {e}", self.config.alsactl.display()),
        }
    }

    fn on_jack(&mut self, value: bool) {
        let inserted = value == self.config.jack.inserted_value;
        let reason = if inserted {
            "jack: auriculares"
        } else {
            "jack: parlante"
        };
        self.set_mode(Mode::for_headphones(inserted), reason);
    }

    fn set_mode(&mut self, mode: Mode, reason: &str) {
        if mode == self.mode {
            return;
        }
        self.mode = mode;
        if let Err(e) = self.write_mode() {
            eprintln!("{e}");
        }
        eprintln!("modo: {mode} ({reason}); las apps lo toman al reabrir el audio");
        self.publish();
    }

    /// Escribe el estado que leen las apps. Un error se loguea y no detiene el servicio.
    fn publish(&mut self) {
        let raw = match self.mixer.volume() {
            Ok(raw) => raw,
            Err(e) => {
                eprintln!("estado: {e}");
                return;
            }
        };
        self.seq += 1;
        let state = AudioState {
            volume_pct: self.scale.percent(raw),
            volume_db: Some(self.scale.db_of(raw)),
            mode: self.mode,
            seq: self.seq,
        };
        let path = &self.state_path;
        if let Err(e) = write_file(path, || files::save_state(path, &state)) {
            eprintln!("estado: {e}");
        }
    }

    fn write_mode(&self) -> Result<(), String> {
        let conf = alsa_conf::render(self.mode, &self.config.pcm_slave());
        write_file(&self.config.alsa_conf_path, || {
            files::write_atomic(&self.config.alsa_conf_path, &conf)
        })?;
        write_file(&self.config.mode_file(), || {
            files::save_mode(&self.config.mode_file(), self.mode)
        })
    }
}

fn write_file(path: &Path, write: impl FnOnce() -> std::io::Result<()>) -> Result<(), String> {
    write().map_err(|e| format!("no pude escribir {}: {e}", path.display()))
}

/// La tarjeta puede aparecer un poco después del arranque: se reintenta unos segundos.
fn open_mixer(config: &Config) -> Result<Mixer, String> {
    let mut last_error = String::new();
    for attempt in 0..10 {
        if attempt > 0 {
            thread::sleep(Duration::from_secs(1));
        }
        match Mixer::open(&config.ctl_name(), &config.volume_control) {
            Ok(mixer) => return Ok(mixer),
            Err(e) => {
                if attempt == 0 {
                    eprintln!("mixer: {e}; reintento durante 10 s");
                }
                last_error = e;
            }
        }
    }
    Err(last_error)
}

fn spawn_input(name: String, tx: Sender<Event>) {
    thread::spawn(move || {
        let sys = Path::new("/sys/class/input");
        let dev = Path::new("/dev/input");
        let mut warned = false;
        loop {
            let opened = evdev::find_device(sys, dev, &name)
                .ok_or_else(|| format!("no encuentro el dispositivo de entrada {name:?}"))
                .and_then(|path| {
                    evdev::Device::open(&path)
                        .map(|device| (path.clone(), device))
                        .map_err(|e| format!("{}: {e}", path.display()))
                });
            let (path, mut device) = match opened {
                Ok(opened) => opened,
                Err(e) => {
                    if !warned {
                        eprintln!("{e}; reintento cada 5 s");
                        warned = true;
                    }
                    thread::sleep(Duration::from_secs(5));
                    continue;
                }
            };
            eprintln!("botones de volumen: {} ({name})", path.display());
            warned = false;
            let error = loop {
                match device.next_event() {
                    Ok(event) => {
                        if let Some((key, action)) = event.volume_key()
                            && tx.send(Event::Key(key, action)).is_err()
                        {
                            return;
                        }
                    }
                    Err(e) => break e,
                }
            };
            eprintln!("se perdió {}: {error}", path.display());
            if tx.send(Event::InputLost).is_err() {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
}

fn spawn_jack(ctl_name: String, control: String, tx: Sender<Event>) {
    thread::spawn(move || {
        let mut last_error = None;
        loop {
            let error = match Jack::open(&ctl_name, &control) {
                Ok(jack) => {
                    eprintln!("jack: siguiendo {control:?} en {ctl_name}");
                    let mut value = jack.read();
                    loop {
                        match value {
                            Ok(v) => {
                                if tx.send(Event::Jack(v)).is_err() {
                                    return;
                                }
                            }
                            Err(e) => break e,
                        }
                        value = jack.wait_change();
                    }
                }
                Err(e) => e,
            };
            if last_error.as_ref() != Some(&error) {
                eprintln!("jack: {error}; reintento cada 10 s");
                last_error = Some(error);
            }
            thread::sleep(Duration::from_secs(10));
        }
    });
}
