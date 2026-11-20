//! Carpeta de datos, proyectos en disco y `config.ron` (spec-formatos, "Carpeta de datos",
//! "Guardado atómico" y "Configuración de la app").
//!
//! Todo corre en el hilo de interfaz: nunca en el de audio. Los errores vuelven como texto para
//! mostrarlos en pantalla.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::model::Project;
use crate::project_file::{self, ProjectFile};

pub const PROJECTS_DIR: &str = "proyectos";
pub const PROJECT_FILE: &str = "proyecto.ron";
pub const BACKUP_FILE: &str = "proyecto.ron.bak";
pub const CONFIG_FILE: &str = "config.ron";
pub const NAME_MAX: usize = 32;
/// Caracteres de los nombres de proyecto, en el orden del editor de nombres.
pub const NAME_CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_-";
const CONFIG_VERSION: u32 = 1;

/// Nombre válido para una carpeta de proyecto (spec-formatos, "Nombres de archivo").
pub fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= NAME_MAX && name.bytes().all(|b| NAME_CHARS.contains(&b))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FontChoice {
    #[default]
    Small8x16,
    Large12x24,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub font: FontChoice,
    pub audition_on_edit: bool,
    pub follow_playback: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            font: FontChoice::Small8x16,
            audition_on_edit: true,
            follow_playback: true,
        }
    }
}

/// `config.ron`: lo que no es del proyecto. Por ahora solo el último proyecto y los ajustes de
/// Proyecto; el resto de la spec (audio, input) se agrega con `#[serde(default)]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub config_version: u32,
    pub last_project: Option<String>,
    pub ui: UiConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            config_version: CONFIG_VERSION,
            last_project: None,
            ui: UiConfig::default(),
        }
    }
}

/// Un proyecto en la carpeta `proyectos/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectEntry {
    pub name: String,
    pub has_main: bool,
    pub has_backup: bool,
    pub modified: Option<SystemTime>,
}

#[derive(Debug)]
pub struct Loaded {
    pub project: Project,
    /// Lo que se corrigió al leer.
    pub warnings: Vec<String>,
    /// Formato más nuevo que esta app: solo lectura.
    pub newer: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LoadError {
    /// No hay `proyecto.ron` ni `.bak`.
    Missing,
    /// El archivo existe pero no se pudo leer o interpretar. `backup`: hay un `.bak` para ofrecer.
    Broken { reason: String, backup: bool },
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Missing => write!(f, "el proyecto no existe"),
            LoadError::Broken { reason, .. } => write!(f, "{reason}"),
        }
    }
}

pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `GROOVEBOX_DATA`; si no, `$XDG_DATA_HOME/groovebox`; si no, `~/.local/share/groovebox`.
    pub fn from_env() -> Self {
        let var = |name| std::env::var_os(name).filter(|v| !v.is_empty());
        if let Some(dir) = var("GROOVEBOX_DATA") {
            return Self::new(dir);
        }
        if let Some(xdg) = var("XDG_DATA_HOME").map(PathBuf::from)
            && xdg.is_absolute()
        {
            return Self::new(xdg.join("groovebox"));
        }
        match var("HOME") {
            Some(home) => Self::new(PathBuf::from(home).join(".local/share/groovebox")),
            None => Self::new("groovebox-data"),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn project_dir(&self, name: &str) -> PathBuf {
        self.root.join(PROJECTS_DIR).join(name)
    }

    fn project_files(&self, name: &str) -> (PathBuf, PathBuf) {
        let dir = self.project_dir(name);
        (dir.join(PROJECT_FILE), dir.join(BACKUP_FILE))
    }

    /// Hay una carpeta con ese nombre (aunque no tenga proyecto adentro).
    pub fn exists(&self, name: &str) -> bool {
        self.project_dir(name).exists()
    }

    /// Proyectos guardados (carpetas con `proyecto.ron` o `.bak`), por nombre.
    pub fn list(&self) -> Result<Vec<ProjectEntry>, String> {
        let dir = self.root.join(PROJECTS_DIR);
        let read = match fs::read_dir(&dir) {
            Ok(read) => read,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("no pude leer {}: {e}", dir.display())),
        };
        let mut entries: Vec<ProjectEntry> = read
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter_map(|e| {
                let name = e.file_name().to_str()?.to_string();
                let (main, backup) = self.project_files(&name);
                let modified = |p: &Path| fs::metadata(p).and_then(|m| m.modified()).ok();
                let entry = ProjectEntry {
                    has_main: main.is_file(),
                    has_backup: backup.is_file(),
                    modified: modified(&main).or_else(|| modified(&backup)),
                    name,
                };
                (entry.has_main || entry.has_backup).then_some(entry)
            })
            .collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    /// Primer `proyecto-NNN` sin carpeta.
    pub fn free_name(&self) -> String {
        (1..)
            .map(|n| format!("proyecto-{n:03}"))
            .find(|name| !self.exists(name))
            .unwrap_or_default()
    }

    /// Guarda en `proyectos/<nombre>/proyecto.ron` de forma atómica, dejando el anterior en `.bak`.
    pub fn save(&self, project: &Project) -> Result<PathBuf, String> {
        let text = project_file::to_ron(&ProjectFile::from(project))
            .map_err(|e| format!("no pude serializar el proyecto: {e}"))?;
        // Nunca se escribe algo que después no se pueda abrir.
        project_file::parse(&text)
            .map_err(|e| format!("el archivo generado no se puede leer ({e}); no se guardó"))?;
        let (main, backup) = self.project_files(&project.name);
        // Si el proyecto.ron actual está roto, no pasa a ser el .bak: el .bak bueno se conserva.
        let rotate = fs::read_to_string(&main).is_ok_and(|t| project_file::parse(&t).is_ok());
        write_atomic(&main, text.as_bytes(), rotate.then_some(backup.as_path()))
            .map_err(|e| format!("no pude guardar en {}: {e}", main.display()))?;
        Ok(main)
    }

    /// Abre `proyecto.ron` o, con `from_backup`, `proyecto.ron.bak`.
    pub fn load(&self, name: &str, from_backup: bool) -> Result<Loaded, LoadError> {
        let (main, backup) = self.project_files(name);
        let path = if from_backup { &backup } else { &main };
        let backup_available = !from_backup && backup.is_file();
        let broken = |reason: String| LoadError::Broken {
            reason,
            backup: backup_available,
        };
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound && !backup_available => {
                return Err(LoadError::Missing);
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(broken(format!("falta {PROJECT_FILE}")));
            }
            Err(e) => return Err(broken(format!("no pude leer {}: {e}", path.display()))),
        };
        let parsed = project_file::parse(&text).map_err(broken)?;
        let converted = parsed.file.into_project(name);
        Ok(Loaded {
            project: converted.project,
            warnings: converted.warnings,
            newer: parsed.newer,
        })
    }

    /// Renombra la carpeta del proyecto (si existe en disco).
    pub fn rename(&self, old: &str, new: &str) -> Result<(), String> {
        let (from, to) = (self.project_dir(old), self.project_dir(new));
        if to.exists() {
            return Err(format!("ya existe un proyecto \"{new}\""));
        }
        if !from.exists() {
            return Ok(());
        }
        fs::rename(&from, &to).map_err(|e| format!("no pude renombrar {}: {e}", from.display()))
    }

    /// `config.ron`; si no existe, la configuración por defecto.
    pub fn load_config(&self) -> Result<AppConfig, String> {
        let path = self.root.join(CONFIG_FILE);
        match fs::read_to_string(&path) {
            Ok(text) => ron::Options::default()
                .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
                .from_str(&text)
                .map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(AppConfig::default()),
            Err(e) => Err(format!("no pude leer {}: {e}", path.display())),
        }
    }

    pub fn save_config(&self, config: &AppConfig) -> Result<(), String> {
        let path = self.root.join(CONFIG_FILE);
        let pretty =
            ron::ser::PrettyConfig::new().extensions(ron::extensions::Extensions::IMPLICIT_SOME);
        let text = ron::ser::to_string_pretty(config, pretty)
            .map_err(|e| format!("no pude serializar la configuración: {e}"))?;
        let text = format!("// groovebox: configuración de la app (no del proyecto)\n{text}\n");
        write_atomic(&path, text.as_bytes(), None)
            .map_err(|e| format!("no pude guardar {}: {e}", path.display()))
    }
}

/// Escribe `path.tmp`, hace fsync y lo renombra sobre `path`; con `backup`, antes mueve el
/// archivo anterior ahí. En FAT el rename no es atómico ante un corte: el `.bak` cubre ese caso.
fn write_atomic(path: &Path, data: &[u8], backup: Option<&Path>) -> io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);
    let written = File::create(&tmp).and_then(|mut f| {
        f.write_all(data)?;
        f.sync_all()
    });
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    if let Some(backup) = backup
        && path.exists()
    {
        fs::rename(path, backup)?;
    }
    fs::rename(&tmp, path)?;
    // Que los renombres lleguen al disco. Algunos sistemas de archivos no dejan sincronizar una
    // carpeta: no es un error para el usuario.
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

#[cfg(test)]
pub mod test_dir {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Carpeta temporal única que se borra al salir del test.
    pub struct TempDir(PathBuf);

    impl TempDir {
        pub fn new() -> Self {
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("groovebox-test-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_dir::TempDir;
    use super::*;
    use crate::model::{NoteCell, Section};

    fn demo(name: &str) -> Project {
        let mut p = crate::demo::project();
        p.name = name.to_string();
        p
    }

    #[test]
    fn names() {
        assert!(valid_name("beat_01"));
        assert!(valid_name("proyecto-001"));
        assert!(!valid_name(""));
        assert!(!valid_name("Beat"));
        assert!(!valid_name("a b"));
        assert!(!valid_name("../x"));
        assert!(!valid_name(&"a".repeat(33)));
    }

    #[test]
    fn save_load_and_backup() {
        let tmp = TempDir::new();
        let data = DataDir::new(tmp.path());
        assert_eq!(data.list().unwrap(), []);
        assert_eq!(data.free_name(), "proyecto-001");

        let mut p = demo("beat");
        let path = data.save(&p).unwrap();
        assert_eq!(path, tmp.path().join("proyectos/beat/proyecto.ron"));
        assert!(!path.with_file_name(BACKUP_FILE).exists());
        let loaded = data.load("beat", false).unwrap();
        assert_eq!(loaded.project, p);
        assert!(loaded.warnings.is_empty());
        assert!(!loaded.newer);

        // El segundo guardado deja el primero en .bak y no deja .tmp.
        p.step_mut(0, Section::default(), 1).unwrap().note = NoteCell::On(72);
        data.save(&p).unwrap();
        assert_eq!(data.load("beat", false).unwrap().project, p);
        let old = data.load("beat", true).unwrap().project;
        assert_eq!(old, demo("beat"));
        assert!(!path.with_file_name("proyecto.ron.tmp").exists());

        let list = data.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "beat");
        assert!(list[0].has_main && list[0].has_backup);
        assert!(list[0].modified.is_some());
    }

    #[test]
    fn broken_file_offers_the_backup() {
        let tmp = TempDir::new();
        let data = DataDir::new(tmp.path());
        assert_eq!(data.load("nada", false).unwrap_err(), LoadError::Missing);

        let p = demo("roto");
        data.save(&p).unwrap();
        let main = data.project_dir("roto").join(PROJECT_FILE);
        // Sin .bak: error sin oferta.
        fs::write(&main, "(format_version: 1, tracks: [").unwrap();
        let Err(LoadError::Broken { backup: false, .. }) = data.load("roto", false) else {
            panic!("tendría que fallar sin .bak");
        };

        data.save(&p).unwrap(); // el roto no pasa a .bak
        assert!(!main.with_file_name(BACKUP_FILE).exists());
        data.save(&p).unwrap(); // ahora hay un .bak bueno
        fs::write(&main, "basura").unwrap();
        let err = data.load("roto", false).unwrap_err();
        let LoadError::Broken { reason, backup } = &err else {
            panic!("{err:?}");
        };
        assert!(*backup);
        assert!(reason.contains("RON inválido"), "{reason}");
        assert_eq!(data.load("roto", true).unwrap().project, p);
        // Guardar lo recuperado repara proyecto.ron y deja el .bak bueno como estaba.
        data.save(&p).unwrap();
        assert_eq!(data.load("roto", false).unwrap().project, p);
        assert_eq!(data.load("roto", true).unwrap().project, p);
        fs::write(&main, "basura").unwrap();

        // Corte entre los dos renombres: falta proyecto.ron pero está el .bak.
        fs::remove_file(&main).unwrap();
        assert!(matches!(
            data.load("roto", false),
            Err(LoadError::Broken { backup: true, .. })
        ));
        assert!(!data.list().unwrap()[0].has_main);
    }

    #[test]
    fn rename_and_free_names() {
        let tmp = TempDir::new();
        let data = DataDir::new(tmp.path());
        data.save(&demo("proyecto-001")).unwrap();
        assert_eq!(data.free_name(), "proyecto-002");
        data.save(&demo("otro")).unwrap();
        assert!(data.rename("proyecto-001", "otro").is_err());
        data.rename("proyecto-001", "nuevo").unwrap();
        assert!(data.load("nuevo", false).is_ok());
        assert!(!data.exists("proyecto-001"));
        // Sin carpeta en disco no hay nada que mover.
        data.rename("sin-guardar", "x").unwrap();
        let names: Vec<_> = data.list().unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["nuevo", "otro"]);
    }

    #[test]
    fn newer_format_loads_read_only() {
        let tmp = TempDir::new();
        let data = DataDir::new(tmp.path());
        let dir = data.project_dir("futuro");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(PROJECT_FILE), "(format_version: 9, tempo_bpm: 90)").unwrap();
        let loaded = data.load("futuro", false).unwrap();
        assert!(loaded.newer);
        assert_eq!(loaded.project.tempo_bpm, 90.0);
        assert_eq!(loaded.project.name, "futuro");
    }

    #[test]
    fn config_round_trip_and_defaults() {
        let tmp = TempDir::new();
        let data = DataDir::new(tmp.path().join("sub"));
        assert_eq!(data.load_config().unwrap(), AppConfig::default());
        let config = AppConfig {
            last_project: Some("beat".to_string()),
            ui: UiConfig {
                font: FontChoice::Large12x24,
                audition_on_edit: false,
                follow_playback: true,
            },
            ..AppConfig::default()
        };
        data.save_config(&config).unwrap();
        assert_eq!(data.load_config().unwrap(), config);
        // El ejemplo de la spec (con campos que la app todavía no usa) se lee.
        fs::write(
            tmp.path().join("sub").join(CONFIG_FILE),
            r#"(config_version: 1, data_root: "/roms2/groovebox", last_project: Some("beat_01"),
                audio: (sample_rate: 48000), ui: (font: Small8x16, follow_playback: false))"#,
        )
        .unwrap();
        let c = data.load_config().unwrap();
        assert_eq!(c.last_project.as_deref(), Some("beat_01"));
        assert!(!c.ui.follow_playback);
        assert!(c.ui.audition_on_edit);
        fs::write(tmp.path().join("sub").join(CONFIG_FILE), "(ui: 3)").unwrap();
        assert!(data.load_config().is_err());
    }
}
