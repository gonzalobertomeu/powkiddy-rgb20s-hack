//! Modales de la pantalla Proyecto (spec-ui 8, 9 y 10): navegador de proyectos, editor de
//! nombres y diálogos de confirmación. Solo estado y edición; `App` decide qué hacer al aceptar.

use crate::storage::{NAME_CHARS, NAME_MAX, ProjectEntry};

/// Filas que salta B + ↑/↓ en el navegador.
pub const BROWSER_PAGE: usize = 10;

pub enum Modal {
    Name(NameEditor),
    Browser(Browser),
    Confirm(Confirm),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NamePurpose {
    /// GUARDAR COMO: guarda una copia con otro nombre y sigue con esa.
    SaveAs,
    /// Sobre "nombre": cambia el nombre (y la carpeta, si ya estaba guardado).
    Rename,
}

/// Editor de nombres por cruz: ←/→ mueven el cursor, A + ↑/↓ cambian la letra, A + B borra.
/// El cursor puede estar en la posición siguiente a la última letra (el hueco `[ ]` del mockup)
/// para agregar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameEditor {
    pub purpose: NamePurpose,
    chars: Vec<u8>,
    pub cursor: usize,
}

impl NameEditor {
    pub fn new(purpose: NamePurpose, initial: &str) -> Self {
        let chars: Vec<u8> = initial
            .bytes()
            .filter(|b| NAME_CHARS.contains(b))
            .take(NAME_MAX)
            .collect();
        let cursor = chars.len().min(NAME_MAX - 1);
        Self {
            purpose,
            chars,
            cursor,
        }
    }

    pub fn name(&self) -> String {
        String::from_utf8_lossy(&self.chars).into_owned()
    }

    pub fn chars(&self) -> &[u8] {
        &self.chars
    }

    /// Última posición del cursor: el hueco para agregar (si todavía entra una letra).
    fn last_position(&self) -> usize {
        self.chars.len().min(NAME_MAX - 1)
    }

    pub fn move_by(&mut self, delta: i32) {
        let last = self.last_position() as i32;
        self.cursor = (self.cursor as i32 + delta).clamp(0, last) as usize;
    }

    pub fn jump(&mut self, to_end: bool) {
        self.cursor = if to_end { self.last_position() } else { 0 };
    }

    /// Letra bajo el cursor (`None` en el hueco).
    pub fn current(&self) -> Option<u8> {
        self.chars.get(self.cursor).copied()
    }

    /// Recorre `NAME_CHARS` en forma circular. En el hueco agrega una letra: `a` hacia arriba,
    /// `-` hacia abajo.
    pub fn cycle(&mut self, delta: i32) {
        let n = NAME_CHARS.len() as i32;
        match self.chars.get_mut(self.cursor) {
            Some(c) => {
                let i = NAME_CHARS.iter().position(|x| x == c).unwrap_or(0) as i32;
                *c = NAME_CHARS[(i + delta).rem_euclid(n) as usize];
            }
            None => {
                let i = if delta > 0 { delta - 1 } else { delta };
                self.chars.push(NAME_CHARS[i.rem_euclid(n) as usize]);
            }
        }
    }

    /// Borra la letra bajo el cursor; en el hueco, la anterior.
    pub fn delete(&mut self) {
        if self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
        } else if let Some(last) = self.chars.len().checked_sub(1) {
            self.chars.remove(last);
            self.cursor = last;
        }
        self.cursor = self.cursor.min(self.last_position());
    }
}

/// Lista de proyectos guardados (spec-ui 8, con el título "CARGAR PROYECTO").
pub struct Browser {
    pub entries: Vec<ProjectEntry>,
    pub cursor: usize,
}

impl Browser {
    pub fn new(entries: Vec<ProjectEntry>, current: &str) -> Self {
        let cursor = entries.iter().position(|e| e.name == current).unwrap_or(0);
        Self { entries, cursor }
    }

    pub fn move_by(&mut self, delta: i32) {
        let last = self.entries.len().saturating_sub(1) as i32;
        self.cursor = (self.cursor as i32 + delta).clamp(0, last) as usize;
    }

    pub fn selected(&self) -> Option<&ProjectEntry> {
        self.entries.get(self.cursor)
    }
}

/// Qué hacer después de resolver los cambios sin guardar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Then {
    Quit,
    New,
    Load(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Confirm {
    /// Hay cambios sin guardar. Start: guardar y seguir; A: seguir sin guardar; B: cancelar.
    Unsaved(Then),
    /// `proyecto.ron` no se puede leer y hay un `.bak`. A: abrir el `.bak`; B: cancelar.
    OpenBackup { name: String, reason: String },
    /// GUARDAR COMO sobre un proyecto que ya existe. A: reemplazar; B: cancelar.
    Overwrite { name: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_editor_edits_with_the_cross() {
        let mut e = NameEditor::new(NamePurpose::SaveAs, "beat_01");
        assert_eq!(e.cursor, 7, "arranca en el hueco del final");
        assert_eq!(e.current(), None);
        e.cycle(1);
        assert_eq!(e.name(), "beat_01a");
        e.cycle(1);
        assert_eq!(e.name(), "beat_01b");
        e.cycle(-2);
        assert_eq!(e.name(), "beat_01-", "da la vuelta");
        e.move_by(1);
        assert_eq!(e.cursor, 8);
        e.delete();
        assert_eq!(e.name(), "beat_01");
        assert_eq!(e.cursor, 7);
        e.jump(false);
        e.cycle(1);
        assert_eq!(e.name(), "ceat_01");
        e.delete();
        assert_eq!(e.name(), "eat_01");
        e.move_by(-5);
        assert_eq!(e.cursor, 0);
        e.move_by(100);
        assert_eq!(e.cursor, 6);
    }

    #[test]
    fn name_editor_filters_and_limits() {
        let mut e = NameEditor::new(NamePurpose::Rename, "Mi Beat!");
        assert_eq!(e.name(), "ieat");
        let mut empty = NameEditor::new(NamePurpose::Rename, "");
        empty.cycle(-1);
        assert_eq!(empty.name(), "-");
        empty.delete();
        empty.delete();
        assert_eq!(empty.name(), "");
        assert_eq!(empty.cursor, 0);

        for _ in 0..40 {
            e.jump(true);
            e.cycle(1);
        }
        assert_eq!(e.chars().len(), NAME_MAX);
        assert_eq!(e.cursor, NAME_MAX - 1, "lleno: no hay hueco");
    }

    #[test]
    fn browser_starts_on_the_current_project() {
        let entry = |name: &str| ProjectEntry {
            name: name.to_string(),
            has_main: true,
            has_backup: false,
            modified: None,
        };
        let mut b = Browser::new(vec![entry("a"), entry("b"), entry("c")], "b");
        assert_eq!(b.cursor, 1);
        b.move_by(BROWSER_PAGE as i32);
        assert_eq!(b.selected().unwrap().name, "c");
        b.move_by(-9);
        assert_eq!(b.cursor, 0);
        let empty = Browser::new(Vec::new(), "x");
        assert!(empty.selected().is_none());
    }
}
