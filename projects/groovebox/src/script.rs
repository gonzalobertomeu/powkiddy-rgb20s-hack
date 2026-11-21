//! Guion de input (`GROOVEBOX_SCRIPT`): aprieta botones con tiempos y guarda capturas, para
//! probar la interfaz sin escritorio (SDL `offscreen`, `just run-arm`) o en la consola sin teclado.
//!
//! Una orden por línea; `#` comenta:
//! - `tap <botón> [veces]`: aprieta y suelta (con 60 ms entre toques).
//! - `press <botón>` / `release <botón>`: mantener y soltar (combinaciones).
//! - `wait <ms>`
//! - `stick <left|right> <x> <y>`: mueve un stick a `x`, `y` en % (−100 a 100; arriba es
//!   positivo). `stick left 0 0` lo suelta.
//! - `shot <archivo.bmp>`: captura de la pantalla tal como se ve.
//! - `quit`
//!
//! Los botones se nombran como en `infra/sim/keymap.toml` (`a`, `select`, `left`...).

use std::collections::VecDeque;
use std::path::PathBuf;

use crate::input::{Button, Stick};

/// Pausa después de cada toque: deja que se dibuje y separa toques seguidos.
const TAP_GAP_MS: u64 = 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Press(Button),
    Release(Button),
    /// Stick y posición en % (−100..=100).
    Stick(Stick, i8, i8),
    Wait(u64),
    Shot(PathBuf),
    Quit,
}

pub fn parse(text: &str) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let err = |msg: &str| format!("guion, línea {}: {msg}: {raw}", n + 1);
        let mut words = line.split_whitespace();
        let cmd = words.next().unwrap_or("");
        let arg = words.next();
        let button = || {
            arg.and_then(Button::from_config_name)
                .ok_or_else(|| err("botón desconocido"))
        };
        match cmd {
            "tap" => {
                let b = button()?;
                let times = match words.next() {
                    Some(t) => t.parse::<u32>().map_err(|_| err("cantidad inválida"))?,
                    None => 1,
                };
                for _ in 0..times {
                    steps.extend([Step::Press(b), Step::Release(b), Step::Wait(TAP_GAP_MS)]);
                }
            }
            "press" => steps.push(Step::Press(button()?)),
            "release" => steps.push(Step::Release(button()?)),
            "wait" => {
                let ms = arg
                    .and_then(|a| a.parse().ok())
                    .ok_or_else(|| err("milisegundos inválidos"))?;
                steps.push(Step::Wait(ms));
            }
            "shot" => steps.push(Step::Shot(
                arg.ok_or_else(|| err("falta el archivo"))?.into(),
            )),
            "stick" => {
                let stick = match arg {
                    Some("left") => Stick::Left,
                    Some("right") => Stick::Right,
                    _ => return Err(err("stick desconocido (left o right)")),
                };
                let mut pct = || {
                    words
                        .next()
                        .and_then(|w| w.parse::<i8>().ok())
                        .filter(|v| (-100..=100).contains(v))
                        .ok_or_else(|| err("posición inválida (−100 a 100)"))
                };
                let (x, y) = (pct()?, pct()?);
                steps.push(Step::Stick(stick, x, y));
            }
            "quit" => steps.push(Step::Quit),
            _ => return Err(err("orden desconocida")),
        }
    }
    Ok(steps)
}

/// Ejecuta el guion contra el reloj de la app.
pub struct Runner {
    steps: VecDeque<Step>,
    resume_at: u64,
}

impl Runner {
    pub fn new(steps: Vec<Step>) -> Self {
        Self {
            steps: steps.into(),
            resume_at: 0,
        }
    }

    /// Devuelve los pasos que tocan ahora. Corta después de una captura, para que se dibuje
    /// el estado de ese momento antes de seguir.
    pub fn due(&mut self, now: u64) -> Vec<Step> {
        let mut out = Vec::new();
        while now >= self.resume_at {
            let Some(step) = self.steps.pop_front() else {
                break;
            };
            match step {
                Step::Wait(ms) => self.resume_at = now + ms,
                Step::Shot(_) => {
                    out.push(step);
                    break;
                }
                other => out.push(other),
            }
        }
        out
    }

    pub fn next_deadline(&self) -> Option<u64> {
        (!self.steps.is_empty()).then_some(self.resume_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_commands_and_expands_taps() {
        let steps = parse("# prueba\ntap right 2\npress a\ntap up # sube\nrelease a\nwait 500\nshot /tmp/x.bmp\nquit\n").unwrap();
        let tap = |b| [Step::Press(b), Step::Release(b), Step::Wait(TAP_GAP_MS)];
        let mut expected = Vec::new();
        expected.extend(tap(Button::Right));
        expected.extend(tap(Button::Right));
        expected.push(Step::Press(Button::A));
        expected.extend(tap(Button::Up));
        expected.extend([
            Step::Release(Button::A),
            Step::Wait(500),
            Step::Shot("/tmp/x.bmp".into()),
            Step::Quit,
        ]);
        assert_eq!(steps, expected);
    }

    #[test]
    fn errors_name_the_line() {
        let e = parse("tap a\ntap turbo\n").unwrap_err();
        assert!(e.contains("línea 2"), "{e}");
        assert!(parse("wait x").is_err());
        assert!(parse("jump").is_err());
        assert!(parse("stick middle 0 0").is_err());
        assert!(parse("stick left 120 0").is_err());
        assert_eq!(
            parse("stick right -50 100").unwrap(),
            [Step::Stick(Stick::Right, -50, 100)]
        );
    }

    #[test]
    fn runner_waits_and_stops_after_shot() {
        let mut r = Runner::new(parse("tap a\nwait 100\nshot s.bmp\npress b\nquit").unwrap());
        assert_eq!(r.due(0), [Step::Press(Button::A), Step::Release(Button::A)]);
        assert!(r.due(59).is_empty());
        // A los 60 ms termina la pausa del toque y empieza la de 100 ms.
        assert!(r.due(60).is_empty());
        assert_eq!(r.next_deadline(), Some(160));
        assert_eq!(r.due(160), [Step::Shot("s.bmp".into())]);
        assert_eq!(r.due(160), [Step::Press(Button::B), Step::Quit]);
        assert_eq!(r.next_deadline(), None);
    }
}
