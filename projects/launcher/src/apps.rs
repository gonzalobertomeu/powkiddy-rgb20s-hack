//! Entradas del launcher (`apps.toml`), la elección que se le pasa al wrapper de sesión y la
//! última app elegida. Sin SDL: todo se prueba con `cargo test`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Una app de `apps.toml` (`[[app]]`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppEntry {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AppsFile {
    #[serde(default)]
    app: Vec<AppEntry>,
}

fn valid_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn parse_apps(text: &str) -> Result<Vec<AppEntry>, String> {
    let file: AppsFile = toml::from_str(text).map_err(|e| e.to_string())?;
    for (i, app) in file.app.iter().enumerate() {
        let label = || format!("app {} ({:?})", i + 1, app.name);
        if app.name.trim().is_empty() {
            return Err(format!("app {}: falta el nombre", i + 1));
        }
        if app.command.is_empty() {
            return Err(format!("{}: falta command", label()));
        }
        // El choice file separa campos con NUL: no pueden aparecer en los valores.
        let texts = [&app.name, &app.command]
            .into_iter()
            .chain(&app.args)
            .chain(&app.cwd)
            .chain(app.env.values());
        if texts.into_iter().any(|s| s.contains('\0')) {
            return Err(format!("{}: hay un carácter NUL", label()));
        }
        if let Some(bad) = app.env.keys().find(|k| !valid_env_name(k)) {
            return Err(format!("{}: nombre de variable inválido: {bad:?}", label()));
        }
    }
    Ok(file.app)
}

pub fn load_apps(path: &Path) -> Result<Vec<AppEntry>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse_apps(&text)
}

/// Dónde buscar `apps.toml`, en orden: `LAUNCHER_APPS`, junto al binario (override local que el
/// deploy no pisa), `assets/` junto al binario (lo que copia `just deploy`) y `assets/` de la
/// carpeta actual (`just run`).
pub fn apps_candidates(env_path: Option<PathBuf>, exe_dir: Option<&Path>) -> Vec<PathBuf> {
    if let Some(path) = env_path {
        return vec![path];
    }
    let mut out = Vec::new();
    if let Some(dir) = exe_dir {
        out.push(dir.join("apps.toml"));
        out.push(dir.join("assets/apps.toml"));
    }
    out.push(PathBuf::from("assets/apps.toml"));
    out
}

impl AppEntry {
    /// Las rutas se verifican; los nombres sueltos (`htop`) se buscan en el PATH al lanzarlos.
    pub fn is_available(&self) -> bool {
        !self.command.contains('/') || Path::new(&self.command).is_file()
    }

    pub fn launch(&self) -> Launch {
        let cwd = self.cwd.clone().unwrap_or_else(|| {
            Path::new(&self.command)
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
        Launch {
            name: self.name.clone(),
            cwd,
            env: self.env.clone().into_iter().collect(),
            argv: std::iter::once(self.command.clone())
                .chain(self.args.iter().cloned())
                .collect(),
        }
    }
}

/// Fila de la lista: las apps y, al final, las dos entradas fijas del sistema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    App(usize),
    Reboot,
    Poweroff,
}

impl Entry {
    pub fn label<'a>(&self, apps: &'a [AppEntry]) -> &'a str {
        match self {
            Entry::App(i) => &apps[*i].name,
            Entry::Reboot => "Reiniciar",
            Entry::Poweroff => "Apagar",
        }
    }
}

pub fn entries(apps: &[AppEntry]) -> Vec<Entry> {
    (0..apps.len())
        .map(Entry::App)
        .chain([Entry::Reboot, Entry::Poweroff])
        .collect()
}

/// Lo que el wrapper tiene que ejecutar: carpeta (vacía = `$HOME`), variables extra y argv.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launch {
    pub name: String,
    pub cwd: String,
    pub env: Vec<(String, String)>,
    pub argv: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Choice {
    Run(Launch),
    Reboot,
    Poweroff,
}

impl Choice {
    /// Formato del choice file: campos terminados en NUL (sin escapes ni `eval` en bash; se lee
    /// con `mapfile -d ''`):
    ///
    /// - `reboot\0` o `poweroff\0`
    /// - `app\0<nombre>\0<cwd>\0<n>\0<K=V>\0…(n veces)<argv0>\0<arg1>\0…`
    pub fn encode(&self) -> Vec<u8> {
        let fields: Vec<String> = match self {
            Choice::Reboot => vec!["reboot".into()],
            Choice::Poweroff => vec!["poweroff".into()],
            Choice::Run(l) => ["app".into(), l.name.clone(), l.cwd.clone()]
                .into_iter()
                .chain([l.env.len().to_string()])
                .chain(l.env.iter().map(|(k, v)| format!("{k}={v}")))
                .chain(l.argv.iter().cloned())
                .collect(),
        };
        let mut out = Vec::new();
        for f in fields {
            out.extend_from_slice(f.as_bytes());
            out.push(0);
        }
        out
    }

    /// Inverso de `encode` (lo usan los tests; el consumidor real es `system/rgb20s-session`).
    #[cfg(test)]
    pub fn decode(bytes: &[u8]) -> Result<Choice, String> {
        let body = bytes.strip_suffix(b"\0").ok_or("falta el NUL final")?;
        let fields: Vec<String> = body
            .split(|&b| b == 0)
            .map(|f| String::from_utf8(f.to_vec()).map_err(|e| e.to_string()))
            .collect::<Result<_, _>>()?;
        match fields.as_slice() {
            [k] if k == "reboot" => Ok(Choice::Reboot),
            [k] if k == "poweroff" => Ok(Choice::Poweroff),
            [k, name, cwd, n, rest @ ..] if k == "app" => {
                let n: usize = n.parse().map_err(|_| format!("cantidad inválida: {n}"))?;
                if rest.len() <= n {
                    return Err("falta el comando".into());
                }
                let env = rest[..n]
                    .iter()
                    .map(|kv| {
                        kv.split_once('=')
                            .map(|(k, v)| (k.to_string(), v.to_string()))
                            .ok_or_else(|| format!("variable sin '=': {kv}"))
                    })
                    .collect::<Result<_, _>>()?;
                Ok(Choice::Run(Launch {
                    name: name.clone(),
                    cwd: cwd.clone(),
                    env,
                    argv: rest[n..].to_vec(),
                }))
            }
            _ => Err("elección desconocida".into()),
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Choice::Reboot => "reiniciar".into(),
            Choice::Poweroff => "apagar".into(),
            Choice::Run(l) => format!("{} ({})", l.name, l.argv.join(" ")),
        }
    }
}

/// Escribe el archivo completo o nada: primero a `.tmp` y después se renombra.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// `$XDG_STATE_HOME/launcher/last` o `~/.local/state/launcher/last`.
pub fn last_choice_path(xdg_state: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    let base = xdg_state
        .filter(|p| p.is_absolute())
        .or_else(|| home.map(|h| h.join(".local/state")))?;
    Some(base.join("launcher/last"))
}

/// Se recuerda el nombre (no el índice) para sobrevivir a cambios de orden en `apps.toml`.
/// Reiniciar y Apagar no se recuerdan: arrancar con Apagar seleccionado invita a un accidente.
pub fn remembered_name(entry: Entry, apps: &[AppEntry]) -> Option<&str> {
    match entry {
        Entry::App(i) => Some(&apps[i].name),
        _ => None,
    }
}

pub fn initial_selection(entries: &[Entry], apps: &[AppEntry], last: Option<&str>) -> usize {
    let last = last.map(str::trim);
    entries
        .iter()
        .position(|&e| last.is_some() && remembered_name(e, apps) == last)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = include_str!("../assets/apps.toml");

    fn app(name: &str, command: &str) -> AppEntry {
        AppEntry {
            name: name.into(),
            command: command.into(),
            args: Vec::new(),
            cwd: None,
            env: BTreeMap::new(),
        }
    }

    #[test]
    fn example_file_parses() {
        let apps = parse_apps(EXAMPLE).unwrap();
        assert_eq!(apps.len(), 1);
        let g = &apps[0];
        assert_eq!(g.name, "Groovebox");
        assert_eq!(g.command, "/home/alarm/groovebox/groovebox");
        assert_eq!(g.cwd.as_deref(), Some("/home/alarm/groovebox"));
        assert_eq!(
            g.env.get("SDL_VIDEODRIVER").map(String::as_str),
            Some("kmsdrm")
        );
        assert!(g.args.is_empty());
    }

    #[test]
    fn full_entry_and_env_table() {
        let text = r#"
            [[app]]
            name = "Uno"
            command = "/opt/uno"
            args = ["--modo", "con espacio", "it's"]

            [[app]]
            name = "Dos"
            command = "dos"
            cwd = "/tmp"
            [app.env]
            A = "1"
            B_2 = "x=y"
        "#;
        let apps = parse_apps(text).unwrap();
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].args, ["--modo", "con espacio", "it's"]);
        assert_eq!(apps[1].env.len(), 2);
        assert_eq!(apps[1].env["B_2"], "x=y");
    }

    #[test]
    fn empty_file_has_no_apps() {
        assert_eq!(parse_apps("").unwrap(), Vec::new());
    }

    #[test]
    fn invalid_files_are_errors() {
        for bad in [
            "[[app]]\nname = \"x\"\n",                   // sin command
            "[[app]]\nname = \" \"\ncommand = \"/a\"\n", // nombre vacío
            "[[app]]\nname = \"x\"\ncommand = \"/a\"\nicon = \"i.png\"\n", // campo desconocido
            "[[app]]\nname = \"x\"\ncommand = \"/a\"\nenv = { \"1A\" = \"v\" }\n",
            "[[app]]\nname = \"x\"\ncommand = \"/a\"\nargs = [\"a\\u0000b\"]\n",
            "[[app]]\nname = \"x\"\ncommand = \"/a\"\nargs = \"no-es-lista\"\n",
        ] {
            assert!(parse_apps(bad).is_err(), "debería fallar: {bad}");
        }
    }

    #[test]
    fn entries_end_with_system_actions() {
        let apps = vec![app("a", "/a"), app("b", "/b")];
        assert_eq!(
            entries(&apps),
            [Entry::App(0), Entry::App(1), Entry::Reboot, Entry::Poweroff]
        );
        assert_eq!(entries(&[]), [Entry::Reboot, Entry::Poweroff]);
        assert_eq!(Entry::Poweroff.label(&apps), "Apagar");
        assert_eq!(Entry::App(1).label(&apps), "b");
    }

    #[test]
    fn launch_defaults_cwd_to_command_dir() {
        assert_eq!(app("a", "/opt/x/bin/a").launch().cwd, "/opt/x/bin");
        assert_eq!(app("a", "htop").launch().cwd, "");
        let mut with_cwd = app("a", "/opt/a");
        with_cwd.cwd = Some("/srv".into());
        with_cwd.args = vec!["-v".into()];
        let l = with_cwd.launch();
        assert_eq!(l.cwd, "/srv");
        assert_eq!(l.argv, ["/opt/a", "-v"]);
    }

    #[test]
    fn choice_format_is_stable() {
        assert_eq!(Choice::Reboot.encode(), b"reboot\0");
        assert_eq!(Choice::Poweroff.encode(), b"poweroff\0");
        let apps = parse_apps(EXAMPLE).unwrap();
        assert_eq!(
            Choice::Run(apps[0].launch()).encode(),
            b"app\0Groovebox\0/home/alarm/groovebox\x001\0SDL_VIDEODRIVER=kmsdrm\0\
              /home/alarm/groovebox/groovebox\0"
        );
    }

    #[test]
    fn choice_round_trip_with_awkward_values() {
        let l = Launch {
            name: "Sí, 'raro' $HOME".into(),
            cwd: String::new(),
            env: vec![("A".into(), "x=y z".into()), ("B".into(), String::new())],
            argv: vec!["/bin/x y".into(), String::new(), "-n\nlínea".into()],
        };
        for c in [Choice::Run(l), Choice::Reboot, Choice::Poweroff] {
            assert_eq!(Choice::decode(&c.encode()).unwrap(), c);
        }
        assert!(Choice::decode(b"reboot").is_err());
        assert!(Choice::decode(b"app\0x\0/\x002\0A=1\0/bin/x\0").is_err());
        assert!(Choice::decode(b"halt\0").is_err());
    }

    #[test]
    fn last_choice_location() {
        assert_eq!(
            last_choice_path(Some("/st".into()), Some("/home/a".into())),
            Some(PathBuf::from("/st/launcher/last"))
        );
        // XDG_STATE_HOME relativo no vale (spec de XDG): se usa el de HOME.
        assert_eq!(
            last_choice_path(Some("rel".into()), Some("/home/a".into())),
            Some(PathBuf::from("/home/a/.local/state/launcher/last"))
        );
        assert_eq!(last_choice_path(None, None), None);
    }

    #[test]
    fn initial_selection_uses_remembered_app() {
        let apps = vec![app("a", "/a"), app("b", "/b")];
        let e = entries(&apps);
        assert_eq!(initial_selection(&e, &apps, Some("b\n")), 1);
        assert_eq!(initial_selection(&e, &apps, Some("borrada")), 0);
        assert_eq!(initial_selection(&e, &apps, None), 0);
        // Sin apps, Reiniciar (nunca Apagar) queda primero.
        assert_eq!(initial_selection(&entries(&[]), &[], Some("Apagar")), 0);
        assert_eq!(remembered_name(Entry::Poweroff, &apps), None);
    }

    #[test]
    fn candidates_order() {
        let exe = Path::new("/home/alarm/launcher");
        assert_eq!(
            apps_candidates(None, Some(exe)),
            [
                PathBuf::from("/home/alarm/launcher/apps.toml"),
                PathBuf::from("/home/alarm/launcher/assets/apps.toml"),
                PathBuf::from("assets/apps.toml"),
            ]
        );
        assert_eq!(
            apps_candidates(Some("/x.toml".into()), Some(exe)),
            [PathBuf::from("/x.toml")]
        );
    }

    #[test]
    fn write_atomic_replaces_file() {
        let dir = std::env::temp_dir().join(format!("launcher-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("choice");
        write_atomic(&path, b"uno").unwrap();
        write_atomic(&path, b"dos").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"dos");
        assert!(!dir.join("choice.tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
