//! Formato de archivo del proyecto (spec-formatos, "Proyecto"; ADR 0004): RON con serde.
//!
//! Los structs del archivo (`ProjectFile` y compañía) son propios y distintos del modelo en
//! memoria (`model::Project`): el archivo es disperso (solo las cadenas y frases que existen y,
//! dentro de cada frase, solo los pasos no vacíos) y estable aunque cambie el modelo. Sin I/O.
//!
//! Versionado: `format_version` sube solo con cambios incompatibles. Un campo nuevo lleva
//! `#[serde(default)]` (o es `Option`: los parámetros de instrumento que faltan toman el valor
//! por defecto de su modelo) y no sube la versión. Los campos desconocidos se ignoran.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::instrument::{
    DrumModel, DrumParams, Instrument, MAX_VOICES, Patch, SynthParams, Wave, default_kit,
};
use crate::model::{
    CHAIN_LEN, Chain, ChainEntry, ChannelStrip, Chord, ChordKind, DEFAULT_GATE, DEFAULT_TEMPO,
    DEFAULT_VELOCITY, MAX_SONG_ROWS, NOTE_MAX, NOTE_MIN, NoteCell, Phrase, Project, SWING_MAX,
    SWING_MIN, Step, TEMPO_MAX, TEMPO_MIN, TRACKS, TRANSPOSE_MAX, Track, gate_nudge,
};

/// Versión del formato que escribe y entiende esta app.
pub const FORMAT_VERSION: u32 = 1;
const HEADER: &str =
    "// groovebox: archivo de proyecto (formato en docs/solutions/groovebox/spec-formatos.md)\n";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectFile {
    pub format_version: u32,
    /// Solo para diagnóstico.
    #[serde(default)]
    pub app_version: String,
    /// Informativo: al abrir manda el nombre de la carpeta.
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_tempo")]
    pub tempo_bpm: f32,
    #[serde(default = "default_swing")]
    pub swing: u8,
    #[serde(default)]
    pub tracks: Vec<TrackFile>,
    /// Una fila por línea: la cadena (o `None`) de cada pista. Solo hasta la última fila no vacía.
    #[serde(default)]
    pub song: Vec<[Option<u8>; TRACKS]>,
    #[serde(default)]
    pub chains: BTreeMap<u8, Vec<ChainEntryFile>>,
    #[serde(default)]
    pub phrases: BTreeMap<u8, Vec<StepFile>>,
}

fn default_tempo() -> f32 {
    DEFAULT_TEMPO
}

fn default_swing() -> u8 {
    50
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrackFile {
    #[serde(default)]
    pub name: String,
    /// Si falta, el instrumento del kit por defecto para esa pista.
    #[serde(default)]
    pub instrument: Option<InstrumentFile>,
    #[serde(default)]
    pub mix: MixFile,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum InstrumentFile {
    DrumSynth(DrumFile),
    Sampler(SamplerFile),
    Subtractive(SubtractiveFile),
}

/// Parámetros en 0–127. Los que faltan toman el valor por defecto del modelo.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrumFile {
    pub model: DrumModel,
    pub voices: Option<u8>,
    pub level: Option<u8>,
    pub vel_to_amp: Option<u8>,
    pub tune: Option<u8>,
    pub decay: Option<u8>,
    pub sweep: Option<u8>,
    pub sweep_time: Option<u8>,
    pub color: Option<u8>,
    pub punch: Option<u8>,
    pub spread: Option<u8>,
}

/// El sample llega en H7: por ahora solo la sección GENERAL.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SamplerFile {
    pub voices: Option<u8>,
    pub level: Option<u8>,
    pub vel_to_amp: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubtractiveFile {
    pub voices: Option<u8>,
    pub level: Option<u8>,
    pub vel_to_amp: Option<u8>,
    pub wave: Option<Wave>,
    pub amp_env: Option<EnvFile>,
    pub cutoff: Option<u8>,
    pub resonance: Option<u8>,
    /// 64 = sin envolvente de filtro.
    pub env_amount: Option<u8>,
    pub filter_decay: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvFile {
    pub a: Option<u8>,
    pub d: Option<u8>,
    pub s: Option<u8>,
    pub r: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MixFile {
    pub volume: u8,
    pub pan: i8,
    pub delay_send: u8,
    pub reverb_send: u8,
    #[serde(skip_serializing_if = "is_false")]
    pub mute: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub solo: bool,
}

impl Default for MixFile {
    fn default() -> Self {
        Self::from(&ChannelStrip::default())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChainEntryFile {
    pub phrase: u8,
    #[serde(default, skip_serializing_if = "is_zero_i8")]
    pub transpose: i8,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StepFile {
    /// Índice del paso en la frase (0–15).
    pub i: u8,
    pub note: NoteFile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chord: Option<ChordFile>,
    #[serde(
        default = "default_velocity",
        skip_serializing_if = "is_default_velocity"
    )]
    pub velocity: u8,
    #[serde(default = "default_gate", skip_serializing_if = "is_default_gate")]
    pub gate_ticks: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum NoteFile {
    On(u8),
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChordFile {
    pub kind: ChordKind,
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub inversion: u8,
}

fn is_false(v: &bool) -> bool {
    !*v
}

fn is_zero_i8(v: &i8) -> bool {
    *v == 0
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

fn default_velocity() -> u8 {
    DEFAULT_VELOCITY
}

fn is_default_velocity(v: &u8) -> bool {
    *v == DEFAULT_VELOCITY
}

fn default_gate() -> u16 {
    DEFAULT_GATE
}

fn is_default_gate(v: &u16) -> bool {
    *v == DEFAULT_GATE
}

// --- Del modelo al archivo -------------------------------------------------------------------

impl From<&ChannelStrip> for MixFile {
    fn from(m: &ChannelStrip) -> Self {
        Self {
            volume: m.volume,
            pan: m.pan,
            delay_send: m.delay_send,
            reverb_send: m.reverb_send,
            mute: m.mute,
            solo: m.solo,
        }
    }
}

impl From<&Instrument> for InstrumentFile {
    fn from(i: &Instrument) -> Self {
        let (voices, level, vel_to_amp) = (Some(i.voices), Some(i.volume), Some(i.velocity_amount));
        match i.patch {
            Patch::Drum(d) => InstrumentFile::DrumSynth(DrumFile {
                model: d.model,
                voices,
                level,
                vel_to_amp,
                tune: Some(d.tune),
                decay: Some(d.decay),
                sweep: Some(d.sweep),
                sweep_time: Some(d.sweep_time),
                color: Some(d.color),
                punch: Some(d.punch),
                spread: Some(d.spread),
            }),
            Patch::Sampler => InstrumentFile::Sampler(SamplerFile {
                voices,
                level,
                vel_to_amp,
            }),
            Patch::Synth(s) => InstrumentFile::Subtractive(SubtractiveFile {
                voices,
                level,
                vel_to_amp,
                wave: Some(s.wave),
                amp_env: Some(EnvFile {
                    a: Some(s.attack),
                    d: Some(s.decay),
                    s: Some(s.sustain),
                    r: Some(s.release),
                }),
                cutoff: Some(s.cutoff),
                resonance: Some(s.resonance),
                env_amount: Some(s.filter_env),
                filter_decay: Some(s.filter_decay),
            }),
        }
    }
}

fn step_file(i: usize, s: &Step) -> Option<StepFile> {
    let note = match s.note {
        NoteCell::Empty => return None,
        NoteCell::On(n) => NoteFile::On(n),
        NoteCell::Off => NoteFile::Off,
    };
    Some(StepFile {
        i: i as u8,
        note,
        chord: (s.chord.kind != ChordKind::None).then_some(ChordFile {
            kind: s.chord.kind,
            inversion: s.chord.inversion,
        }),
        velocity: s.velocity,
        gate_ticks: s.gate_ticks,
    })
}

impl From<&Project> for ProjectFile {
    fn from(p: &Project) -> Self {
        let used_rows = p
            .song
            .rows
            .iter()
            .rposition(|r| r.iter().any(Option::is_some))
            .map_or(0, |i| i + 1);
        let chains = p
            .chains
            .iter()
            .enumerate()
            .filter_map(|(id, c)| {
                let c = c.as_ref()?;
                let entries = c
                    .entries
                    .iter()
                    .map_while(|e| *e)
                    .map(|e| ChainEntryFile {
                        phrase: e.phrase,
                        transpose: e.transpose,
                    })
                    .collect();
                Some((id as u8, entries))
            })
            .collect();
        let phrases = p
            .phrases
            .iter()
            .enumerate()
            .filter_map(|(id, ph)| {
                let steps = ph
                    .as_ref()?
                    .steps
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| step_file(i, s))
                    .collect();
                Some((id as u8, steps))
            })
            .collect();
        Self {
            format_version: FORMAT_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            name: p.name.clone(),
            tempo_bpm: p.tempo_bpm,
            swing: p.swing,
            tracks: p
                .tracks
                .iter()
                .map(|t| TrackFile {
                    name: t.name.clone(),
                    instrument: Some(InstrumentFile::from(&t.instrument)),
                    mix: MixFile::from(&t.mix),
                })
                .collect(),
            song: p.song.rows[..used_rows].to_vec(),
            chains,
            phrases,
        }
    }
}

// --- Del archivo al modelo -------------------------------------------------------------------

/// Lo que se corrigió al leer (referencias rotas, valores fuera de rango). Se avisa en pantalla.
#[derive(Debug, Default, PartialEq, Eq)]
struct Repairs {
    missing_tracks: usize,
    extra_tracks: usize,
    broken_entries: usize,
    long_chains: usize,
    broken_cells: usize,
    extra_rows: usize,
    bad_steps: usize,
    out_of_range: usize,
}

impl Repairs {
    fn messages(&self) -> Vec<String> {
        let items = [
            (
                self.missing_tracks,
                "pistas faltantes (con el kit por defecto)",
            ),
            (self.extra_tracks, "pistas de más (ignoradas)"),
            (
                self.broken_entries,
                "entradas de cadena a frases inexistentes (quitadas)",
            ),
            (self.long_chains, "cadenas de más de 16 frases (recortadas)"),
            (
                self.broken_cells,
                "celdas de la canción a cadenas inexistentes (vaciadas)",
            ),
            (self.extra_rows, "filas de más en la canción (ignoradas)"),
            (self.bad_steps, "pasos fuera de la frase (ignorados)"),
            (self.out_of_range, "valores fuera de rango (ajustados)"),
        ];
        items
            .iter()
            .filter(|(n, _)| *n > 0)
            .map(|(n, text)| format!("{n} {text}"))
            .collect()
    }
}

fn param(v: Option<u8>, default: u8, r: &mut Repairs) -> u8 {
    match v {
        None => default,
        Some(v) if v > 127 => {
            r.out_of_range += 1;
            127
        }
        Some(v) => v,
    }
}

fn voices(v: Option<u8>, default: u8, r: &mut Repairs) -> u8 {
    let v = v.unwrap_or(default);
    if !(1..=MAX_VOICES).contains(&v) {
        r.out_of_range += 1;
    }
    v.clamp(1, MAX_VOICES)
}

fn instrument(file: &InstrumentFile, kit: &Instrument, r: &mut Repairs) -> Instrument {
    // Lo que falta sale del instrumento del kit de esa pista si es del mismo tipo.
    let generic = |patch| Instrument::new(patch, 1);
    let (base, patch) = match *file {
        InstrumentFile::DrumSynth(d) => {
            let base = DrumParams::new(d.model);
            let patch = Patch::Drum(DrumParams {
                model: d.model,
                tune: param(d.tune, base.tune, r),
                decay: param(d.decay, base.decay, r),
                sweep: param(d.sweep, base.sweep, r),
                sweep_time: param(d.sweep_time, base.sweep_time, r),
                color: param(d.color, base.color, r),
                punch: param(d.punch, base.punch, r),
                spread: param(d.spread, base.spread, r),
            });
            let general = match kit.patch {
                Patch::Drum(k) if k.model == d.model => *kit,
                _ => generic(patch),
            };
            ((general, d.voices, d.level, d.vel_to_amp), patch)
        }
        InstrumentFile::Sampler(s) => {
            let general = match kit.patch {
                Patch::Sampler => *kit,
                _ => generic(Patch::Sampler),
            };
            ((general, s.voices, s.level, s.vel_to_amp), Patch::Sampler)
        }
        InstrumentFile::Subtractive(s) => {
            let base = match kit.patch {
                Patch::Synth(k) => k,
                _ => SynthParams::poly(),
            };
            let env = s.amp_env.unwrap_or(EnvFile {
                a: None,
                d: None,
                s: None,
                r: None,
            });
            let patch = Patch::Synth(SynthParams {
                wave: s.wave.unwrap_or(base.wave),
                attack: param(env.a, base.attack, r),
                decay: param(env.d, base.decay, r),
                sustain: param(env.s, base.sustain, r),
                release: param(env.r, base.release, r),
                cutoff: param(s.cutoff, base.cutoff, r),
                resonance: param(s.resonance, base.resonance, r),
                filter_env: param(s.env_amount, base.filter_env, r),
                filter_decay: param(s.filter_decay, base.filter_decay, r),
            });
            let general = match kit.patch {
                Patch::Synth(_) => *kit,
                _ => generic(patch),
            };
            ((general, s.voices, s.level, s.vel_to_amp), patch)
        }
    };
    let (general, v, level, vel) = base;
    Instrument {
        patch,
        volume: param(level, general.volume, r),
        velocity_amount: param(vel, general.velocity_amount, r),
        voices: voices(v, general.voices, r),
    }
}

fn step(s: &StepFile, r: &mut Repairs) -> Step {
    let note = match s.note {
        NoteFile::On(n) => {
            if !(NOTE_MIN..=NOTE_MAX).contains(&n) {
                r.out_of_range += 1;
            }
            NoteCell::On(n.clamp(NOTE_MIN, NOTE_MAX))
        }
        NoteFile::Off => NoteCell::Off,
    };
    let mut chord = s.chord.map_or(Chord::default(), |c| Chord {
        kind: c.kind,
        inversion: c.inversion,
    });
    if chord.inversion > chord.max_inversion() {
        r.out_of_range += 1;
        chord.inversion = chord.max_inversion();
    }
    let velocity = param(Some(s.velocity), DEFAULT_VELOCITY, r);
    let gate_ticks = gate_nudge(s.gate_ticks.max(1), 0);
    if gate_ticks != s.gate_ticks {
        r.out_of_range += 1;
    }
    Step {
        note,
        chord,
        velocity,
        gate_ticks,
    }
}

/// Resultado de convertir un archivo: el proyecto y los avisos de lo que se corrigió.
pub struct Converted {
    pub project: Project,
    pub warnings: Vec<String>,
}

impl ProjectFile {
    /// Convierte al modelo en memoria, validando la integridad (spec-formatos, "Versionado").
    /// `name` es el nombre de la carpeta (manda sobre el del archivo).
    pub fn into_project(self, name: &str) -> Converted {
        let mut r = Repairs::default();
        let mut project = Project::new_default();
        project.name = name.to_string();

        let tempo = if self.tempo_bpm.is_finite() {
            self.tempo_bpm
        } else {
            default_tempo()
        };
        project.tempo_bpm = tempo.clamp(TEMPO_MIN, TEMPO_MAX);
        if project.tempo_bpm != self.tempo_bpm {
            r.out_of_range += 1;
        }
        project.swing = self.swing.clamp(SWING_MIN, SWING_MAX);
        if project.swing != self.swing {
            r.out_of_range += 1;
        }

        let kit = default_kit();
        r.missing_tracks = TRACKS.saturating_sub(self.tracks.len());
        r.extra_tracks = self.tracks.len().saturating_sub(TRACKS);
        project.tracks = std::array::from_fn(|t| {
            let (kit_name, kit_instrument) = kit[t];
            let Some(file) = self.tracks.get(t) else {
                return Track {
                    name: kit_name.to_string(),
                    instrument: kit_instrument,
                    mix: ChannelStrip::default(),
                };
            };
            let m = file.mix;
            Track {
                name: if file.name.is_empty() {
                    kit_name.to_string()
                } else {
                    file.name.clone()
                },
                instrument: file
                    .instrument
                    .as_ref()
                    .map_or(kit_instrument, |i| instrument(i, &kit_instrument, &mut r)),
                mix: ChannelStrip {
                    volume: m.volume.min(127),
                    pan: m.pan.clamp(-64, 63),
                    delay_send: m.delay_send.min(127),
                    reverb_send: m.reverb_send.min(127),
                    mute: m.mute,
                    solo: m.solo,
                },
            }
        });

        project.phrases.fill(None);
        for (&id, steps) in &self.phrases {
            let mut phrase = Phrase::default();
            for s in steps {
                match phrase.steps.get_mut(usize::from(s.i)) {
                    Some(slot) => *slot = step(s, &mut r),
                    None => r.bad_steps += 1,
                }
            }
            project.phrases[usize::from(id)] = Some(phrase);
        }

        project.chains.fill(None);
        for (&id, entries) in &self.chains {
            let mut chain = Chain::default();
            let mut len = 0;
            for e in entries {
                if project.phrases[usize::from(e.phrase)].is_none() {
                    r.broken_entries += 1;
                    continue;
                }
                if len == CHAIN_LEN {
                    r.long_chains += 1;
                    break;
                }
                if e.transpose.abs() > TRANSPOSE_MAX {
                    r.out_of_range += 1;
                }
                chain.entries[len] = Some(ChainEntry {
                    phrase: e.phrase,
                    transpose: e.transpose.clamp(-TRANSPOSE_MAX, TRANSPOSE_MAX),
                });
                len += 1;
            }
            project.chains[usize::from(id)] = Some(chain);
        }

        r.extra_rows = self.song.len().saturating_sub(MAX_SONG_ROWS);
        project.song.rows = self
            .song
            .iter()
            .take(MAX_SONG_ROWS)
            .map(|row| {
                row.map(|cell| {
                    cell.filter(|&id| {
                        let ok = project.chains[usize::from(id)].is_some();
                        if !ok {
                            r.broken_cells += 1;
                        }
                        ok
                    })
                })
            })
            .collect();
        while project
            .song
            .rows
            .last()
            .is_some_and(|row| row.iter().all(Option::is_none))
        {
            project.song.rows.pop();
        }

        Converted {
            project,
            warnings: r.messages(),
        }
    }
}

// --- Texto -----------------------------------------------------------------------------------

fn options() -> ron::Options {
    // Así se aceptan también los archivos escritos a mano sin el encabezado `#![enable(...)]`.
    ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
}

/// Serializa a RON legible: una pista, una fila, una entrada de cadena y un paso por línea.
pub fn to_ron(file: &ProjectFile) -> Result<String, ron::Error> {
    let config = ron::ser::PrettyConfig::new()
        .depth_limit(3)
        .extensions(ron::extensions::Extensions::IMPLICIT_SOME);
    let body = ron::ser::to_string_pretty(file, config)?;
    Ok(format!("{HEADER}{body}\n"))
}

/// Solo la versión: se lee antes que el resto para decidir cómo interpretar el archivo.
#[derive(Deserialize)]
#[serde(rename = "proyecto")]
struct VersionProbe {
    format_version: Option<u32>,
}

/// Archivo leído. `newer`: es de una versión del formato posterior a esta app (se abre en
/// solo lectura).
#[derive(Debug)]
pub struct Parsed {
    pub file: ProjectFile,
    pub newer: bool,
}

pub fn parse(text: &str) -> Result<Parsed, String> {
    let probe: VersionProbe = options()
        .from_str(text)
        .map_err(|e| format!("RON inválido ({e})"))?;
    let version = probe
        .format_version
        .ok_or("no es un proyecto: falta format_version")?;
    // Migraciones en cadena (v1 → v2 → …) irían acá; el formato 1 es el primero.
    match options().from_str::<ProjectFile>(text) {
        Ok(file) if version == 0 => Err(format!("format_version {} inválido", file.format_version)),
        Ok(file) => Ok(Parsed {
            file,
            newer: version > FORMAT_VERSION,
        }),
        Err(e) if version > FORMAT_VERSION => Err(format!(
            "formato {version}, más nuevo que el de esta app ({FORMAT_VERSION}): {e}"
        )),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Section;

    fn round_trip(p: &Project) -> Project {
        let text = to_ron(&ProjectFile::from(p)).unwrap();
        let parsed = parse(&text).unwrap();
        assert!(!parsed.newer);
        let c = parsed.file.into_project(&p.name);
        assert!(c.warnings.is_empty(), "{:?}", c.warnings);
        c.project
    }

    #[test]
    fn default_project_round_trips() {
        let p = Project::new_default();
        assert_eq!(round_trip(&p), p);
    }

    #[test]
    fn demo_round_trips() {
        let mut p = crate::demo::project();
        // Valores que la demo no tiene: OFF, mute, transposición, parámetros cambiados.
        p.step_mut(0, Section::default(), 3).unwrap().note = NoteCell::Off;
        p.tracks[2].mix.mute = true;
        p.tracks[4].mix.pan = -20;
        p.tracks[7].instrument.voices = 6;
        if let Patch::Synth(s) = &mut p.tracks[6].instrument.patch {
            s.wave = Wave::Square;
            s.cutoff = 3;
        }
        p.tempo_bpm = 97.5;
        p.swing = 62;
        assert_eq!(round_trip(&p), p);
    }

    #[test]
    fn swing_out_of_range_is_clamped() {
        for (swing, expected) in [(90, 75), (10, 50)] {
            let text = format!("(format_version: 1, swing: {swing})");
            let c = parse(&text).unwrap().file.into_project("x");
            assert_eq!(c.project.swing, expected);
            assert!(
                c.warnings
                    .contains(&"1 valores fuera de rango (ajustados)".to_string())
            );
        }
    }

    #[test]
    fn file_is_sparse_and_readable() {
        let p = crate::demo::project();
        let text = to_ron(&ProjectFile::from(&p)).unwrap();
        assert!(text.starts_with("// groovebox"));
        assert!(text.contains("format_version: 1,"));
        // Un paso por línea, sin los valores por defecto.
        assert!(
            text.lines()
                .any(|l| l.trim() == "(i: 0, note: On(60), velocity: 120),"),
            "{text}"
        );
        assert!(text.contains("instrument: DrumSynth((model: Kick,"));
        // Solo las frases que existen.
        let phrases = ProjectFile::from(&p).phrases.len();
        assert_eq!(phrases, p.phrases.iter().filter(|ph| ph.is_some()).count());
        assert!(phrases < 64);
    }

    #[test]
    fn trailing_empty_rows_are_not_written() {
        let mut p = Project::new_default();
        p.song.rows.push([None; TRACKS]);
        let file = ProjectFile::from(&p);
        assert_eq!(file.song.len(), 1);
    }

    #[test]
    fn missing_fields_take_defaults() {
        let text = "(format_version: 1)";
        let c = parse(text).unwrap().file.into_project("x");
        let mut expected = Project::new_default();
        expected.name = "x".to_string();
        expected.song.rows.clear();
        expected.chains.fill(None);
        expected.phrases.fill(None);
        assert_eq!(c.project, expected);
        assert_eq!(c.warnings, ["8 pistas faltantes (con el kit por defecto)"]);
    }

    #[test]
    fn missing_instrument_params_use_the_model_defaults() {
        // Una caja con solo la afinación, un sinte sin envolvente y campos desconocidos.
        let text = r#"(
            format_version: 1,
            futuro: (lo_que_sea: [1, 2]),
            tracks: [
                (name: "A", instrument: DrumSynth((model: Snare, tune: 70))),
                (name: "B", instrument: Some(Subtractive((cutoff: 10, wave: Sine))), mix: (pan: 5)),
                (name: "C"),
            ],
            song: [(Some(0), 1, None, None, None, None, None, None)],
            chains: {0: [(phrase: 3, transpose: -5)], 1: []},
            phrases: {3: [(i: 2, note: On(40), chord: (kind: Min7), nuevo: 1)]},
        )"#;
        let c = parse(text).unwrap().file.into_project("x");
        let p = &c.project;
        let Patch::Drum(d) = p.tracks[0].instrument.patch else {
            panic!("no es percusión");
        };
        assert_eq!(
            d,
            DrumParams {
                tune: 70,
                ..DrumParams::new(DrumModel::Snare)
            }
        );
        let Patch::Synth(s) = p.tracks[1].instrument.patch else {
            panic!("no es sinte");
        };
        assert_eq!(s.wave, Wave::Sine);
        assert_eq!(s.cutoff, 10);
        assert_eq!(s.attack, SynthParams::poly().attack);
        assert_eq!(p.tracks[1].mix.pan, 5);
        assert_eq!(p.tracks[1].mix.volume, 100);
        assert_eq!(p.tracks[2].instrument, default_kit()[2].1);
        assert_eq!(p.chain_id_at(1, 0), Some(1));
        let step = p.step(0, Section::default(), 2).unwrap();
        assert_eq!(step.note, NoteCell::On(40));
        assert_eq!(step.chord.kind, ChordKind::Min7);
        assert_eq!(step.velocity, DEFAULT_VELOCITY);
        assert_eq!(p.chain_at(0, 0).unwrap().entry(0).unwrap().transpose, -5);
    }

    #[test]
    fn broken_references_become_empty_with_warnings() {
        let text = r#"(
            format_version: 1,
            song: [(0, 9, None, None, None, None, None, None), (None, None, None, None, None, None, None, 7)],
            chains: {0: [(phrase: 1), (phrase: 50), (phrase: 1, transpose: 90)]},
            phrases: {1: [(i: 20, note: On(60)), (i: 1, note: On(200), gate_ticks: 30)]},
        )"#;
        let c = parse(text).unwrap().file.into_project("x");
        let p = &c.project;
        let chain = p.chain(0).unwrap();
        assert_eq!(chain.len(), 2, "la entrada a la frase 50 se quita");
        assert_eq!(chain.entry(1).unwrap().transpose, TRANSPOSE_MAX);
        assert_eq!(p.chain_id_at(1, 0), None);
        assert_eq!(p.song.rows.len(), 1, "la fila 1 queda vacía y se recorta");
        let s = p.step(0, Section::default(), 1).unwrap();
        assert_eq!(s.note, NoteCell::On(NOTE_MAX));
        assert_eq!(s.gate_ticks, 48);
        assert_eq!(
            c.warnings,
            [
                "8 pistas faltantes (con el kit por defecto)",
                "1 entradas de cadena a frases inexistentes (quitadas)",
                "2 celdas de la canción a cadenas inexistentes (vaciadas)",
                "1 pasos fuera de la frase (ignorados)",
                "3 valores fuera de rango (ajustados)",
            ]
        );
    }

    #[test]
    fn versions() {
        assert!(
            parse("(tempo_bpm: 120)")
                .unwrap_err()
                .contains("format_version")
        );
        assert!(parse("no es ron").unwrap_err().contains("RON inválido"));
        assert!(parse("(format_version: 0)").is_err());
        let newer = parse("(format_version: 2, cosa_nueva: 1)").unwrap();
        assert!(newer.newer);
        let err = parse("(format_version: 3, tracks: \"otra cosa\")").unwrap_err();
        assert!(err.contains("más nuevo"), "{err}");
    }
}
