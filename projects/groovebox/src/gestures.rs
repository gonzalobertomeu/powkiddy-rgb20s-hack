//! Traduce pulsaciones de botones a gestos según el mapa global de controles (spec-ui):
//! A = modificador de edición, Select = modificador global, B + cruz = saltos, A + B = borrar.
//!
//! No depende de SDL: recibe `press`/`release` con un tiempo en milisegundos y `tick` para la
//! repetición automática, así se prueba con tiempos simulados.

use crate::input::Button;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    fn from_button(b: Button) -> Option<Dir> {
        match b {
            Button::Up => Some(Dir::Up),
            Button::Down => Some(Dir::Down),
            Button::Left => Some(Dir::Left),
            Button::Right => Some(Dir::Right),
            _ => None,
        }
    }

    fn button(self) -> Button {
        match self {
            Dir::Up => Button::Up,
            Dir::Down => Button::Down,
            Dir::Left => Button::Left,
            Dir::Right => Button::Right,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    /// Cruz sola: mover el cursor (con repetición).
    Move(Dir),
    /// A + cruz: cambiar el valor (con repetición; `fast` tras 1 s mantenido multiplica el paso).
    Edit { dir: Dir, fast: bool },
    /// B + cruz: salto grande del cursor.
    Jump(Dir),
    /// Select + cruz: cambiar de pantalla.
    Screen(Dir),
    /// A: insertar / activar.
    Insert,
    /// Doble toque de A sin mover la cruz.
    InsertNew,
    /// A + B en cualquier orden.
    Delete,
    /// Select + A.
    Paste,
    /// B solo.
    Cancel,
    /// X.
    Listen,
    /// Select + X.
    Mute,
    /// Select + Y.
    Solo,
    /// Y solo: modo selección.
    SelectMode,
    /// A + Y.
    Clone,
    /// L1/R1.
    Track(i32),
    /// A + L1/R1.
    Chord(i32),
    /// Select + L1/R1.
    Octave(i32),
    /// L2/R2.
    Section(i32),
    /// Select + L2/R2.
    Tempo(i32),
    /// Start.
    Play,
    /// Select + Start.
    LoopSection,
    /// Select + Start mantenidos.
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GestureConfig {
    /// Espera antes de la primera repetición de la cruz.
    pub repeat_delay_ms: u64,
    /// Intervalo entre repeticiones.
    pub repeat_interval_ms: u64,
    /// Con A + cruz mantenido más que esto, el paso se multiplica.
    pub fast_after_ms: u64,
    pub double_tap_ms: u64,
    pub quit_hold_ms: u64,
}

impl Default for GestureConfig {
    /// Valores de spec-ui ("Repetición de teclas" y "Doble toque").
    fn default() -> Self {
        Self {
            repeat_delay_ms: 250,
            repeat_interval_ms: 33,
            fast_after_ms: 1000,
            double_tap_ms: 300,
            quit_hold_ms: 1000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CrossMode {
    Move,
    Edit,
    Jump,
    Screen,
}

#[derive(Clone, Copy, Debug)]
struct Repeat {
    dir: Dir,
    mode: CrossMode,
    pressed_at: u64,
    next_at: u64,
}

/// Si la app se trabó, no se descarga una ráfaga de repeticiones atrasadas.
const MAX_REPEATS_PER_TICK: usize = 4;

pub struct Gestures {
    config: GestureConfig,
    held: [Option<u64>; Button::ALL.len()],
    repeat: Option<Repeat>,
    last_a_tap: Option<u64>,
    quit_armed_at: Option<u64>,
    quit_sent: bool,
}

impl Gestures {
    pub fn new(config: GestureConfig) -> Self {
        Self {
            config,
            held: [None; Button::ALL.len()],
            repeat: None,
            last_a_tap: None,
            quit_armed_at: None,
            quit_sent: false,
        }
    }

    pub fn is_held(&self, b: Button) -> bool {
        self.held[b.index()].is_some()
    }

    pub fn held(&self) -> [bool; Button::ALL.len()] {
        self.held.map(|h| h.is_some())
    }

    fn cross_mode(&self) -> CrossMode {
        if self.is_held(Button::Select) {
            CrossMode::Screen
        } else if self.is_held(Button::A) {
            CrossMode::Edit
        } else if self.is_held(Button::B) {
            CrossMode::Jump
        } else {
            CrossMode::Move
        }
    }

    fn cross_gesture(mode: CrossMode, dir: Dir, fast: bool) -> Gesture {
        match mode {
            CrossMode::Move => Gesture::Move(dir),
            CrossMode::Edit => Gesture::Edit { dir, fast },
            CrossMode::Jump => Gesture::Jump(dir),
            CrossMode::Screen => Gesture::Screen(dir),
        }
    }

    pub fn press(&mut self, b: Button, now: u64, out: &mut Vec<Gesture>) {
        if self.is_held(b) {
            return;
        }
        self.held[b.index()] = Some(now);

        if let Some(dir) = Dir::from_button(b) {
            self.last_a_tap = None;
            let mode = self.cross_mode();
            out.push(Self::cross_gesture(mode, dir, false));
            self.repeat = matches!(mode, CrossMode::Move | CrossMode::Edit).then_some(Repeat {
                dir,
                mode,
                pressed_at: now,
                next_at: now + self.config.repeat_delay_ms,
            });
            return;
        }

        let select = self.is_held(Button::Select);
        let a = self.is_held(Button::A);
        let b_held = self.is_held(Button::B);
        let gesture = match b {
            Button::A if select => Some(Gesture::Paste),
            Button::A if b_held => Some(Gesture::Delete),
            Button::A => {
                let double = self
                    .last_a_tap
                    .is_some_and(|t| now.saturating_sub(t) <= self.config.double_tap_ms);
                if double {
                    self.last_a_tap = None;
                    Some(Gesture::InsertNew)
                } else {
                    self.last_a_tap = Some(now);
                    Some(Gesture::Insert)
                }
            }
            Button::B if a => Some(Gesture::Delete),
            // Select + B es deshacer (v1).
            Button::B if select => None,
            Button::B => Some(Gesture::Cancel),
            Button::X if select => Some(Gesture::Mute),
            Button::X if a => None,
            Button::X => Some(Gesture::Listen),
            Button::Y if select => Some(Gesture::Solo),
            Button::Y if a => Some(Gesture::Clone),
            Button::Y => Some(Gesture::SelectMode),
            Button::L1 | Button::R1 => {
                let d = if b == Button::L1 { -1 } else { 1 };
                Some(if select {
                    Gesture::Octave(d)
                } else if a {
                    Gesture::Chord(d)
                } else {
                    Gesture::Track(d)
                })
            }
            Button::L2 | Button::R2 => {
                let d = if b == Button::L2 { -1 } else { 1 };
                if select {
                    Some(Gesture::Tempo(d))
                } else if a {
                    None
                } else {
                    Some(Gesture::Section(d))
                }
            }
            Button::Start if select => Some(Gesture::LoopSection),
            Button::Start => Some(Gesture::Play),
            _ => None,
        };
        out.extend(gesture);

        if self.is_held(Button::Select) && self.is_held(Button::Start) {
            self.quit_armed_at = Some(now);
        }
    }

    pub fn release(&mut self, b: Button, _now: u64) {
        self.held[b.index()] = None;
        if self.repeat.is_some_and(|r| r.dir.button() == b) {
            self.repeat = None;
        }
        if matches!(b, Button::Select | Button::Start) {
            self.quit_armed_at = None;
            self.quit_sent = false;
        }
    }

    /// Repeticiones y gestos por tiempo. Llamar seguido (al menos cada `repeat_interval_ms`).
    pub fn tick(&mut self, now: u64, out: &mut Vec<Gesture>) {
        if let Some(t) = self.quit_armed_at
            && !self.quit_sent
            && now.saturating_sub(t) >= self.config.quit_hold_ms
        {
            self.quit_sent = true;
            out.push(Gesture::Quit);
        }

        let Some(mut r) = self.repeat else { return };
        // Si cambió el modificador (por ejemplo, se soltó A), la repetición se corta.
        if self.cross_mode() != r.mode {
            self.repeat = None;
            return;
        }
        let mut emitted = 0;
        while now >= r.next_at && emitted < MAX_REPEATS_PER_TICK {
            let fast = r.mode == CrossMode::Edit
                && now.saturating_sub(r.pressed_at) >= self.config.fast_after_ms;
            out.push(Self::cross_gesture(r.mode, r.dir, fast));
            r.next_at += self.config.repeat_interval_ms;
            emitted += 1;
        }
        if now >= r.next_at {
            r.next_at = now + self.config.repeat_interval_ms;
        }
        self.repeat = Some(r);
    }

    /// Próximo instante en que `tick` puede emitir algo (para dormir hasta entonces).
    pub fn next_deadline(&self) -> Option<u64> {
        let repeat = self.repeat.map(|r| r.next_at);
        let quit = self
            .quit_armed_at
            .filter(|_| !self.quit_sent)
            .map(|t| t + self.config.quit_hold_ms);
        match (repeat, quit) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g() -> Gestures {
        Gestures::new(GestureConfig::default())
    }

    fn press(gs: &mut Gestures, b: Button, t: u64) -> Vec<Gesture> {
        let mut out = Vec::new();
        gs.press(b, t, &mut out);
        out
    }

    fn tick(gs: &mut Gestures, t: u64) -> Vec<Gesture> {
        let mut out = Vec::new();
        gs.tick(t, &mut out);
        out
    }

    #[test]
    fn cross_alone_moves() {
        let mut gs = g();
        assert_eq!(
            press(&mut gs, Button::Right, 0),
            [Gesture::Move(Dir::Right)]
        );
    }

    #[test]
    fn modifiers_change_cross_meaning() {
        let mut gs = g();
        press(&mut gs, Button::A, 0);
        assert_eq!(
            press(&mut gs, Button::Up, 10),
            [Gesture::Edit {
                dir: Dir::Up,
                fast: false
            }]
        );
        gs.release(Button::Up, 20);
        gs.release(Button::A, 20);

        press(&mut gs, Button::B, 30);
        assert_eq!(press(&mut gs, Button::Left, 40), [Gesture::Jump(Dir::Left)]);
        gs.release(Button::Left, 50);
        gs.release(Button::B, 50);

        press(&mut gs, Button::Select, 60);
        assert_eq!(
            press(&mut gs, Button::Right, 70),
            [Gesture::Screen(Dir::Right)]
        );
    }

    #[test]
    fn select_wins_over_a() {
        let mut gs = g();
        press(&mut gs, Button::A, 0);
        press(&mut gs, Button::Select, 0);
        assert_eq!(
            press(&mut gs, Button::Down, 5),
            [Gesture::Screen(Dir::Down)]
        );
    }

    #[test]
    fn repeat_waits_delay_then_rate() {
        let mut gs = g();
        press(&mut gs, Button::Down, 0);
        assert!(tick(&mut gs, 249).is_empty());
        assert_eq!(tick(&mut gs, 250), [Gesture::Move(Dir::Down)]);
        assert!(tick(&mut gs, 282).is_empty());
        assert_eq!(tick(&mut gs, 283), [Gesture::Move(Dir::Down)]);
        assert_eq!(gs.next_deadline(), Some(316));
        gs.release(Button::Down, 290);
        assert!(tick(&mut gs, 400).is_empty());
        assert_eq!(gs.next_deadline(), None);
    }

    #[test]
    fn repeat_is_configurable_and_capped() {
        let mut gs = Gestures::new(GestureConfig {
            repeat_delay_ms: 100,
            repeat_interval_ms: 10,
            ..GestureConfig::default()
        });
        press(&mut gs, Button::Left, 0);
        assert_eq!(tick(&mut gs, 100).len(), 1);
        // 1 s sin tick: no larga 90 repeticiones juntas.
        assert_eq!(tick(&mut gs, 1000).len(), MAX_REPEATS_PER_TICK);
        assert_eq!(gs.next_deadline(), Some(1010));
    }

    #[test]
    fn screen_and_jump_do_not_repeat() {
        let mut gs = g();
        press(&mut gs, Button::Select, 0);
        press(&mut gs, Button::Left, 0);
        assert!(tick(&mut gs, 1000).is_empty());
    }

    #[test]
    fn edit_repeat_becomes_fast_after_one_second() {
        let mut gs = g();
        press(&mut gs, Button::A, 0);
        press(&mut gs, Button::Right, 0);
        let early = tick(&mut gs, 250);
        assert_eq!(
            early,
            [Gesture::Edit {
                dir: Dir::Right,
                fast: false
            }]
        );
        let mut t = 250;
        let mut last = Vec::new();
        while t < 1100 {
            t += 33;
            let out = tick(&mut gs, t);
            if !out.is_empty() {
                last = out;
            }
        }
        assert_eq!(
            last,
            [Gesture::Edit {
                dir: Dir::Right,
                fast: true
            }]
        );
    }

    #[test]
    fn releasing_a_stops_edit_repeat() {
        let mut gs = g();
        press(&mut gs, Button::A, 0);
        press(&mut gs, Button::Right, 0);
        gs.release(Button::A, 100);
        assert!(tick(&mut gs, 300).is_empty());
    }

    #[test]
    fn a_plus_b_deletes_in_any_order() {
        let mut gs = g();
        press(&mut gs, Button::A, 0);
        assert_eq!(press(&mut gs, Button::B, 10), [Gesture::Delete]);
        gs.release(Button::A, 20);
        gs.release(Button::B, 20);

        assert_eq!(press(&mut gs, Button::B, 500), [Gesture::Cancel]);
        assert_eq!(press(&mut gs, Button::A, 510), [Gesture::Delete]);
    }

    #[test]
    fn double_tap_a() {
        let mut gs = g();
        assert_eq!(press(&mut gs, Button::A, 0), [Gesture::Insert]);
        gs.release(Button::A, 50);
        assert_eq!(press(&mut gs, Button::A, 200), [Gesture::InsertNew]);
        gs.release(Button::A, 250);
        // Un tercer toque empieza de nuevo.
        assert_eq!(press(&mut gs, Button::A, 300), [Gesture::Insert]);
        gs.release(Button::A, 350);
        // Lento: no es doble.
        assert_eq!(press(&mut gs, Button::A, 1000), [Gesture::Insert]);
    }

    #[test]
    fn moving_the_cross_cancels_double_tap() {
        let mut gs = g();
        press(&mut gs, Button::A, 0);
        gs.release(Button::A, 30);
        press(&mut gs, Button::Right, 60);
        gs.release(Button::Right, 80);
        assert_eq!(press(&mut gs, Button::A, 100), [Gesture::Insert]);
    }

    #[test]
    fn shoulder_combinations() {
        let mut gs = g();
        assert_eq!(press(&mut gs, Button::R1, 0), [Gesture::Track(1)]);
        gs.release(Button::R1, 1);
        press(&mut gs, Button::A, 2);
        assert_eq!(press(&mut gs, Button::L1, 3), [Gesture::Chord(-1)]);
        gs.release(Button::L1, 4);
        gs.release(Button::A, 4);
        press(&mut gs, Button::Select, 5);
        assert_eq!(press(&mut gs, Button::L1, 6), [Gesture::Octave(-1)]);
        assert_eq!(press(&mut gs, Button::R2, 7), [Gesture::Tempo(1)]);
        gs.release(Button::Select, 8);
        assert_eq!(press(&mut gs, Button::L2, 9), [Gesture::Section(-1)]);
    }

    #[test]
    fn face_button_combinations() {
        let mut gs = g();
        assert_eq!(press(&mut gs, Button::X, 0), [Gesture::Listen]);
        assert_eq!(press(&mut gs, Button::Y, 0), [Gesture::SelectMode]);
        gs.release(Button::X, 1);
        gs.release(Button::Y, 1);
        press(&mut gs, Button::Select, 2);
        assert_eq!(press(&mut gs, Button::X, 3), [Gesture::Mute]);
        assert_eq!(press(&mut gs, Button::Y, 3), [Gesture::Solo]);
        assert_eq!(press(&mut gs, Button::A, 3), [Gesture::Paste]);
        gs.release(Button::Select, 4);
        gs.release(Button::Y, 4);
        gs.release(Button::A, 4);
        press(&mut gs, Button::A, 1000);
        assert_eq!(press(&mut gs, Button::Y, 1001), [Gesture::Clone]);
    }

    #[test]
    fn start_select_loops_then_quits_when_held() {
        let mut gs = g();
        assert_eq!(press(&mut gs, Button::Start, 0), [Gesture::Play]);
        gs.release(Button::Start, 10);
        press(&mut gs, Button::Select, 100);
        assert_eq!(press(&mut gs, Button::Start, 120), [Gesture::LoopSection]);
        assert!(tick(&mut gs, 1000).is_empty());
        assert_eq!(gs.next_deadline(), Some(1120));
        assert_eq!(tick(&mut gs, 1120), [Gesture::Quit]);
        assert!(tick(&mut gs, 2000).is_empty(), "una sola vez");
    }

    #[test]
    fn short_select_start_does_not_quit() {
        let mut gs = g();
        press(&mut gs, Button::Select, 0);
        press(&mut gs, Button::Start, 10);
        gs.release(Button::Start, 300);
        assert!(tick(&mut gs, 2000).is_empty());
    }

    #[test]
    fn duplicate_press_is_ignored() {
        let mut gs = g();
        assert_eq!(press(&mut gs, Button::A, 0).len(), 1);
        assert!(press(&mut gs, Button::A, 10).is_empty());
    }
}
