//! Configuración de alsa-lib que redefine `default` según el modo.
//!
//! Se escribe en `/var/lib/alsa/conf.d/`, que `alsa.conf` carga con `errors false` antes de
//! `/etc/asound.conf`: si el archivo no existe, ALSA vuelve a su `default` de fábrica.
//!
//! Cadena de reproducción: `plug` (formato, frecuencia y 1→2 canales) → `route` (mezcla con
//! `ttable.ENTRADA.SALIDA`) → `pcm_slave` (por defecto `plughw:<tarjeta>,0`). La captura
//! va directo a `pcm_slave`, como sin este archivo.

use std::fmt::Write;

use crate::mode::Mode;

pub fn render(mode: Mode, pcm_slave: &str) -> String {
    let (description, ttable): (&str, &[(u8, u8, &str)]) = match mode {
        Mode::Mono => (
            "RGB20S, mono (L+R)/2 para el parlante",
            &[(0, 0, "0.5"), (1, 0, "0.5"), (0, 1, "0.5"), (1, 1, "0.5")],
        ),
        Mode::Stereo => ("RGB20S, estéreo", &[(0, 0, "1.0"), (1, 1, "1.0")]),
    };

    let mut out = String::new();
    out.push_str(
        "# Generado por rgb20s-audio: no editar, se reescribe al cambiar de modo.\n\
         # Las apps toman el cambio al abrir el dispositivo de audio.\n",
    );
    let _ = writeln!(out, "# Modo: {mode}");
    let _ = write!(
        out,
        r#"
pcm.!default {{
    type asym
    playback.pcm "rgb20s_playback"
    capture.pcm "{pcm_slave}"
    hint.description "{description}"
}}

pcm.rgb20s_playback {{
    type plug
    slave {{
        pcm "rgb20s_route"
        channels 2
    }}
}}

pcm.rgb20s_route {{
    type route
    slave {{
        pcm "{pcm_slave}"
        channels 2
    }}
"#
    );
    for (input, output, gain) in ttable {
        let _ = writeln!(out, "    ttable.{input}.{output} {gain}");
    }
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_mixes_both_inputs_into_both_outputs() {
        let conf = render(Mode::Mono, "plughw:0,0");
        for entry in ["0.0", "1.0", "0.1", "1.1"] {
            assert!(conf.contains(&format!("ttable.{entry} 0.5\n")), "{conf}");
        }
        assert!(conf.contains("# Modo: mono\n"));
    }

    #[test]
    fn stereo_is_identity() {
        let conf = render(Mode::Stereo, "plughw:0,0");
        assert!(conf.contains("ttable.0.0 1.0\n"));
        assert!(conf.contains("ttable.1.1 1.0\n"));
        assert!(!conf.contains("ttable.1.0"));
        assert!(!conf.contains("ttable.0.1"));
    }

    #[test]
    fn slave_is_used_for_playback_and_capture() {
        let conf = render(Mode::Mono, "plughw:rk817int,0");
        assert_eq!(conf.matches("\"plughw:rk817int,0\"").count(), 2);
        assert!(conf.contains("pcm.!default {"));
    }

    #[test]
    fn braces_are_balanced() {
        for mode in [Mode::Mono, Mode::Stereo] {
            let conf = render(mode, "plughw:0,0");
            assert_eq!(conf.matches('{').count(), conf.matches('}').count());
        }
    }
}
