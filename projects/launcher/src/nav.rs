//! Navegación de la lista y repetición de la cruceta, con tiempos en milisegundos (sin SDL).

/// Cursor de la lista. Un toque da la vuelta en los extremos; la repetición al mantener se
/// detiene en ellos, para no pasarse de largo y caer en la otra punta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Menu {
    len: usize,
    selected: usize,
}

impl Menu {
    pub fn new(len: usize, selected: usize) -> Self {
        Self {
            len,
            selected: selected.min(len.saturating_sub(1)),
        }
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Mueve una fila hacia `delta` (±1). Devuelve si cambió la selección.
    pub fn step(&mut self, delta: i32, wrap: bool) -> bool {
        if self.len == 0 {
            return false;
        }
        let last = self.len - 1;
        let next = match (delta < 0, self.selected) {
            (true, 0) if wrap => last,
            (true, 0) => 0,
            (true, s) => s - 1,
            (false, s) if s == last && wrap => 0,
            (false, s) => (s + 1).min(last),
        };
        let changed = next != self.selected;
        self.selected = next;
        changed
    }
}

/// Repetición de una dirección mantenida: primer paso al apretar, después `delay` y luego uno
/// cada `interval`.
#[derive(Clone, Copy, Debug)]
pub struct Repeat {
    delay_ms: u64,
    interval_ms: u64,
    held: Option<(i32, u64)>,
}

impl Repeat {
    pub fn new(delay_ms: u64, interval_ms: u64) -> Self {
        Self {
            delay_ms,
            interval_ms: interval_ms.max(1),
            held: None,
        }
    }

    pub fn press(&mut self, delta: i32, now: u64) {
        self.held = Some((delta, now + self.delay_ms));
    }

    /// Soltar otra dirección (por ejemplo, la que se apretó antes) no corta la repetición actual.
    pub fn release(&mut self, delta: i32) {
        if self.held.is_some_and(|(d, _)| d == delta) {
            self.held = None;
        }
    }

    /// Un paso por llamada como mucho: si el bucle se demoró, no se acumulan saltos.
    pub fn tick(&mut self, now: u64) -> Option<i32> {
        let (delta, next) = self.held?;
        if now < next {
            return None;
        }
        self.held = Some((delta, now + self.interval_ms));
        Some(delta)
    }

    pub fn deadline(&self) -> Option<u64> {
        self.held.map(|(_, next)| next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tap_wraps_around() {
        let mut m = Menu::new(3, 0);
        assert!(m.step(-1, true));
        assert_eq!(m.selected(), 2);
        assert!(m.step(1, true));
        assert_eq!(m.selected(), 0);
        assert!(m.step(1, true));
        assert_eq!(m.selected(), 1);
    }

    #[test]
    fn repeat_stops_at_ends() {
        let mut m = Menu::new(3, 1);
        assert!(m.step(1, false));
        assert!(!m.step(1, false));
        assert_eq!(m.selected(), 2);
        m.step(-1, false);
        m.step(-1, false);
        assert!(!m.step(-1, false));
        assert_eq!(m.selected(), 0);
    }

    #[test]
    fn single_entry_and_clamping() {
        let mut m = Menu::new(1, 0);
        assert!(!m.step(1, true));
        assert!(!m.step(-1, true));
        assert_eq!(Menu::new(2, 9).selected(), 1);
        let mut empty = Menu::new(0, 0);
        assert!(!empty.step(1, true));
    }

    #[test]
    fn repeat_timing() {
        let mut r = Repeat::new(300, 100);
        assert_eq!(r.tick(0), None);
        r.press(1, 1000);
        assert_eq!(r.deadline(), Some(1300));
        assert_eq!(r.tick(1299), None);
        assert_eq!(r.tick(1300), Some(1));
        assert_eq!(r.tick(1350), None);
        assert_eq!(r.tick(1400), Some(1));
        // Bucle demorado: un solo paso, y el siguiente se cuenta desde ahora.
        assert_eq!(r.tick(2000), Some(1));
        assert_eq!(r.tick(2050), None);
        r.release(1);
        assert_eq!(r.tick(5000), None);
        assert_eq!(r.deadline(), None);
    }

    #[test]
    fn newer_direction_wins() {
        let mut r = Repeat::new(300, 100);
        r.press(-1, 0);
        r.press(1, 100);
        r.release(-1);
        assert_eq!(r.tick(400), Some(1));
        r.release(1);
        assert_eq!(r.tick(1000), None);
    }
}
