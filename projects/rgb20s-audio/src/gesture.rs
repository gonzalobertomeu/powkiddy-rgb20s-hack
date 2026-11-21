//! Botones de volumen: pasos sueltos, repetición al mantener y el gesto de los dos a la vez.
//!
//! Cada pulsación espera hasta `window` antes de cambiar el volumen: si en ese tiempo se aprieta
//! el otro botón, es el gesto (alterna mono/estéreo) y el volumen no se toca. Un toque que se
//! suelta antes de la ventana aplica el paso al soltar, así que en la práctica no suma demora.
//! Después del gesto se ignora todo hasta soltar los dos botones.

use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolumeKey {
    Up,
    Down,
}

impl VolumeKey {
    fn index(self) -> usize {
        match self {
            VolumeKey::Up => 0,
            VolumeKey::Down => 1,
        }
    }
}

/// Valor de un evento `EV_KEY` de evdev.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAction {
    Release,
    Press,
    /// Repetición del kernel al mantener (`value 2`).
    Repeat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Step(VolumeKey),
    ToggleMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Pending {
        key: VolumeKey,
        deadline: Instant,
    },
    /// Gesto disparado: se espera a que se suelten los dos botones.
    Chord,
}

#[derive(Debug)]
pub struct Gesture {
    window: Duration,
    held: [bool; 2],
    phase: Phase,
}

impl Gesture {
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            held: [false; 2],
            phase: Phase::Idle,
        }
    }

    /// Olvida el estado (por ejemplo, al reabrir el dispositivo de entrada).
    pub fn reset(&mut self) {
        self.held = [false; 2];
        self.phase = Phase::Idle;
    }

    /// Momento en que hay que llamar a [`Gesture::poll`].
    pub fn deadline(&self) -> Option<Instant> {
        match self.phase {
            Phase::Pending { deadline, .. } => Some(deadline),
            _ => None,
        }
    }

    /// Resuelve una pulsación pendiente cuya ventana ya pasó. Llamarla antes de `on_key`.
    pub fn poll(&mut self, now: Instant) -> Option<Action> {
        match self.phase {
            Phase::Pending { key, deadline } if now >= deadline => {
                self.phase = Phase::Idle;
                Some(Action::Step(key))
            }
            _ => None,
        }
    }

    pub fn on_key(&mut self, key: VolumeKey, action: KeyAction, now: Instant) -> Option<Action> {
        match action {
            KeyAction::Press => {
                self.held[key.index()] = true;
                match self.phase {
                    Phase::Idle => {
                        self.phase = Phase::Pending {
                            key,
                            deadline: now + self.window,
                        };
                        None
                    }
                    Phase::Pending { key: first, .. } if first != key => {
                        self.phase = Phase::Chord;
                        Some(Action::ToggleMode)
                    }
                    _ => None,
                }
            }
            KeyAction::Repeat => match self.phase {
                Phase::Idle if self.held[key.index()] => Some(Action::Step(key)),
                Phase::Pending { key: first, .. } if first == key => {
                    self.phase = Phase::Idle;
                    Some(Action::Step(key))
                }
                _ => None,
            },
            KeyAction::Release => {
                self.held[key.index()] = false;
                match self.phase {
                    Phase::Pending { key: first, .. } if first == key => {
                        self.phase = Phase::Idle;
                        Some(Action::Step(key))
                    }
                    Phase::Chord if !self.held.iter().any(|&h| h) => {
                        self.phase = Phase::Idle;
                        None
                    }
                    _ => None,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use KeyAction::*;
    use VolumeKey::*;

    const WINDOW: Duration = Duration::from_millis(150);

    struct Clock(Instant);

    impl Clock {
        fn at(&self, ms: u64) -> Instant {
            self.0 + Duration::from_millis(ms)
        }
    }

    /// Simula el bucle del servicio: `poll` antes de cada evento y al vencer el plazo.
    fn run(events: &[(u64, VolumeKey, KeyAction)], end_ms: u64) -> Vec<Action> {
        let clock = Clock(Instant::now());
        let mut g = Gesture::new(WINDOW);
        let mut out = Vec::new();
        for &(ms, key, action) in events {
            let now = clock.at(ms);
            if let Some(deadline) = g.deadline().filter(|d| *d <= now) {
                out.extend(g.poll(deadline));
            }
            out.extend(g.on_key(key, action, now));
        }
        out.extend(g.poll(clock.at(end_ms)));
        out
    }

    #[test]
    fn quick_tap_steps_on_release() {
        assert_eq!(
            run(&[(0, Up, Press), (40, Up, Release)], 1000),
            [Action::Step(Up)]
        );
    }

    #[test]
    fn slow_press_steps_after_window_once() {
        assert_eq!(
            run(&[(0, Down, Press), (200, Down, Release)], 1000),
            [Action::Step(Down)]
        );
    }

    #[test]
    fn hold_repeats() {
        let events = [
            (0, Up, Press),
            (250, Up, Repeat),
            (283, Up, Repeat),
            (316, Up, Repeat),
            (330, Up, Release),
        ];
        assert_eq!(run(&events, 1000), [Action::Step(Up); 4]);
    }

    #[test]
    fn both_buttons_toggle_without_volume_change() {
        let events = [
            (0, Up, Press),
            (60, Down, Press),
            (300, Up, Repeat),
            (310, Down, Repeat),
            (400, Up, Release),
            (420, Down, Repeat),
            (450, Down, Release),
        ];
        assert_eq!(run(&events, 1000), [Action::ToggleMode]);
    }

    #[test]
    fn after_chord_buttons_work_again() {
        let events = [
            (0, Down, Press),
            (30, Up, Press),
            (100, Down, Release),
            (120, Up, Release),
            (500, Up, Press),
            (550, Up, Release),
        ];
        assert_eq!(run(&events, 1000), [Action::ToggleMode, Action::Step(Up)]);
    }

    #[test]
    fn second_button_after_window_is_a_normal_press() {
        let events = [
            (0, Up, Press),
            (200, Down, Press),
            (230, Down, Release),
            (260, Up, Release),
        ];
        assert_eq!(run(&events, 1000), [Action::Step(Up), Action::Step(Down)]);
    }

    #[test]
    fn deadline_only_while_pending() {
        let now = Instant::now();
        let mut g = Gesture::new(WINDOW);
        assert_eq!(g.deadline(), None);
        g.on_key(Up, Press, now);
        assert_eq!(g.deadline(), Some(now + WINDOW));
        assert_eq!(g.poll(now + Duration::from_millis(10)), None);
        assert_eq!(g.poll(now + WINDOW), Some(Action::Step(Up)));
        assert_eq!(g.deadline(), None);
    }

    #[test]
    fn reset_forgets_held_keys() {
        let now = Instant::now();
        let mut g = Gesture::new(WINDOW);
        g.on_key(Up, Press, now);
        g.reset();
        assert_eq!(g.on_key(Up, Repeat, now + WINDOW * 2), None);
    }
}
