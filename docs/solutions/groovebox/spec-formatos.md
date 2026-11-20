# Groovebox — spec de formatos: proyecto, samples y MIDI

Última revisión: 2026-11-19 · Estado: borrador para revisión del usuario · Volver al [índice](README.md)

Decisión base: [ADR 0004 — formato de proyecto en RON](../../decisions/0004-formato-de-proyecto-ron.md) (propuesta).

## Carpeta de datos

Todo vive bajo una **raíz de datos** configurable:

| Entorno | Raíz propuesta |
|---|---|
| Consola con segunda microSD | `/roms2/groovebox/` **(sin verificar el punto de montaje de la TF2 en dArkOS)** |
| Consola con una sola microSD | `/roms/groovebox/` **(sin verificar)** |
| Mac (desarrollo) | `./data/` dentro de `projects/groovebox/` o `~/.local/share/groovebox/` |

```
groovebox/
  config.ron                 configuración de la app (no del proyecto)
  proyectos/
    beat_01/
      proyecto.ron           el proyecto
      proyecto.ron.bak       versión anterior (guardado atómico)
      autoguardado.ron       (v1)
      samples/
        vox_hey.wav          copias de los samples que usa el proyecto
  samples/                   biblioteca del usuario (solo lectura para la app)
    baterias/...
  instrumentos/              (v1) presets de instrumentos sueltos (.ron)
  exportados/                (v1) renders a WAV
```

## Proyecto

- **Formato:** [RON](https://github.com/ron-rs/ron) (Rusty Object Notation) con `serde`. Es legible, admite comentarios y representa bien los `enum` con datos (por ejemplo, `Instrument::Sampler(...)`), algo que en TOML es incómodo.
- **Autocontenido:** al asignar un sample a una pista, el WAV se **copia** a `proyectos/<nombre>/samples/`. La ruta en el `.ron` es relativa a la carpeta del proyecto. Así un proyecto se puede copiar o compartir como una carpeta.
- **Disperso:** solo se guardan las frases y cadenas que existen (con su número), dentro de cada frase solo los pasos no vacíos (con su índice), y en la canción solo las filas hasta la última no vacía. Los campos con valor por defecto se pueden omitir.
- **Estructura:** canción → cadenas por pista → frases ([ADR 0006](../../decisions/0006-cadenas-por-pista.md)). Las referencias entre ellas son números (`000`–`255`), igual que en memoria.
- **Modelo de archivo separado del modelo en memoria:** conviene tener structs propios del archivo (por ejemplo `ProjectFile`) y convertir de y hacia `Project`. Así el formato puede ser disperso y estable aunque cambien los structs internos.

### Ejemplo

```ron
// groovebox: archivo de proyecto
(
    format_version: 1,
    app_version: "0.1.0",
    name: "beat_01",
    tempo_bpm: 120.0,
    swing: 54,
    tracks: [
        (
            name: "BOMBO",
            instrument: DrumSynth((model: Kick, voices: 1, tune: 0, sweep: 80, sweep_time: 30,
                                   decay: 60, punch: 40, cutoff: 127, resonance: 0,
                                   level: 100, vel_to_amp: 64)),
            mix: (volume: 100, pan: 0, delay_send: 0, reverb_send: 12),
        ),
        // ... pistas 2 a 6 ...
        // ... pistas 2 a 5 ...
        (
            name: "SAMPLE",
            instrument: Sampler((sample: Some("samples/vox_hey.wav"), voices: 2,
                                 tune: 0, fine: 0, start: 0, end: 127,
                                 cutoff: 127, resonance: 0, attack: 0, decay: 80,
                                 level: 100, vel_to_amp: 64)),
            mix: (volume: 100, pan: 0, delay_send: 40, reverb_send: 60),
        ),
        // ... pista 7 (BAJO) ...
        (
            name: "SINTE",
            instrument: Subtractive((voices: 4, wave: Saw, tune: 0, fine: 0, sub: 40,
                                     noise: 0, filter: LowPass, cutoff: 64,
                                     resonance: 30, env_amount: 50, key_track: 64,
                                     amp_env: (a: 2, d: 40, s: 100, r: 20),
                                     filter_env: (a: 0, d: 30, s: 0, r: 10),
                                     level: 100, vel_to_amp: 64)),
            mix: (volume: 75, pan: 15, delay_send: 30, reverb_send: 25),
        ),
    ],
    // canción: una fila por línea, una cadena (o None) por pista
    song: [
        [Some(0),  Some(1), Some(2), Some(3), Some(4), None,    Some(6), Some(7)],
        [Some(0),  Some(1), Some(2), Some(8), Some(4), Some(5), Some(9), Some(10)],
    ],
    chains: {
        0: [(phrase: 0), (phrase: 0), (phrase: 0), (phrase: 20)],
        7: [(phrase: 7), (phrase: 7, transpose: 5), (phrase: 12), (phrase: 13)],
        // ... solo las cadenas que existen ...
    },
    phrases: {
        0: [
            (i: 0,  note: On(36), velocity: 100),
            (i: 4,  note: On(36), velocity: 100),
            (i: 8,  note: On(36), velocity: 100),
            (i: 12, note: On(36), velocity: 100),
        ],
        7: [
            (i: 0,  note: On(60), chord: (kind: Min7, inversion: 1), velocity: 90, gate_ticks: 192),
            (i: 4,  note: On(56), chord: (kind: Major), velocity: 80, gate_ticks: 96),
            (i: 8,  note: On(58), chord: (kind: Dom7), velocity: 90, gate_ticks: 192),
            (i: 12, note: Off),
        ],
        // ... solo las frases que existen; dentro de cada una, solo los pasos no vacíos ...
    },
    fx: (
        delay: (time: Sixteenth3, feedback: 40, tone: 90, width: 100, return_level: 80),
        reverb: (size: 60, damping: 50, predelay: 10, return_level: 70),
    ),
    master: (gain: 0, limiter: true),
)
```

### Versionado

- `format_version` es un entero que sube **solo con cambios incompatibles**.
- **Cambios aditivos** (un campo nuevo con valor por defecto, un `enum` con una variante nueva) **no** suben la versión: se usan `#[serde(default)]` en los campos nuevos.
- Al leer un archivo con versión **menor**, se aplican migraciones en cadena (`v1 → v2 → …`) antes de convertir a `Project`.
- Al leer un archivo con versión **mayor** que la que entiende la app, se abre en **solo lectura** con un aviso, y no se permite sobrescribirlo.
- `app_version` se guarda solo para diagnóstico.
- Los campos de v1 del modelo (locks, condiciones, micro, retrig, largo de frase variable) ya están definidos en el formato 1, así no hace falta migrar cuando se implementen.
- Al leer, se valida la integridad: una cadena que apunta a una frase inexistente o una celda de canción que apunta a una cadena inexistente se tratan como vacías y se avisa.

### Guardado atómico

1. Serializar en memoria (en el hilo de interfaz o en un hilo auxiliar; **nunca** en el de audio).
2. Escribir `proyecto.ron.tmp` y hacer `fsync`.
3. Renombrar `proyecto.ron` → `proyecto.ron.bak`.
4. Renombrar `proyecto.ron.tmp` → `proyecto.ron`.

En FAT32 o exFAT el rename no garantiza atomicidad ante un corte de energía **(sin verificar el sistema de archivos de la TF2)**;
el `.bak` cubre ese caso. Al abrir, si `proyecto.ron` falta o no se puede leer, se ofrece abrir el `.bak`.

**Autoguardado (v1):** cada 2 minutos con cambios, a `autoguardado.ron`, por el mismo mecanismo. Al abrir, si el autoguardado es más nuevo que el proyecto, se pregunta cuál usar.

### Nombres de archivo

Caracteres `a–z`, `0–9`, `_` y `-`, hasta 32. Motivos: compatibilidad con FAT, sin problemas de mayúsculas/minúsculas,
y es lo que se puede escribir cómodo con la cruz. Los samples importados con otros nombres se normalizan al copiarse
(minúsculas, espacios a `_`, se descartan los demás caracteres); si el nombre ya existe con otro contenido, se agrega `_2`, `_3`…

## Samples

| Aspecto | MVP | v1 / después |
|---|---|---|
| Contenedor | WAV (RIFF), incluido `WAVE_FORMAT_EXTENSIBLE` | FLAC (v1, por ejemplo con el crate `claxon`); MP3/OGG (después) |
| Formato de muestra | PCM entero de 8 (sin signo), 16, 24 y 32 bits; flotante de 32 bits | — |
| Canales | Mono y estéreo (si hay más, se usan los dos primeros) | — |
| Sample rate | Cualquiera: se guarda tal cual y se compensa con el paso de lectura | Remuestreo de calidad al cargar (v1, si hace falta) |
| Largo máximo | 60 s por sample (configurable) y 128 MB en total por proyecto | — |
| Metadatos | Se ignoran los chunks extra | Leer los puntos de loop del chunk `smpl` (v1) |
| En memoria | `f32` intercalado | `i16` si falta memoria |
| Lectura | Con `hound`, en el hilo de interfaz; el sample listo se pasa al motor ([spec-motor](spec-motor.md#arquitectura)) | Carga en un hilo aparte con barra de progreso |

Errores (formato no admitido, archivo corrupto, demasiado largo): se muestra un aviso y la pista queda como estaba.

### Exportar a WAV (v1)

- Render offline (más rápido que tiempo real) de la sección actual (en loop, N vueltas) o de la canción, con 2 s extra para las colas de delay y reverb.
- Salida: `exportados/<proyecto>_<AAAA-MM-DD>_<n>.wav`, 48 kHz, estéreo, 16 o 24 bits (con dither al bajar a 16).
- **Después:** stems (un WAV por pista).

## Configuración de la app

`config.ron` en la raíz de datos. No es parte del proyecto.

```ron
(
    config_version: 1,
    data_root: "/roms2/groovebox",
    last_project: Some("beat_01"),
    audio: (sample_rate: 48000, buffer_frames: 256, device: "default"),
    input: (
        // índice de botón SDL -> botón lógico; el mapeo real de la RGB20S está sin verificar
        buttons: { 0: A, 1: B, 2: X, 3: Y, 4: L1, 5: R1, 6: L2, 7: R2, 8: Select, 9: Start },
        dpad: Hat(0),
        sticks: (left: (x: 0, y: 1), right: (x: 2, y: 3), deadzone: 0.15, invert_y: false),
        repeat_delay_ms: 250, repeat_rate_ms: 33,
    ),
    ui: (font: Small8x16, audition_on_edit: true, follow_playback: false),
)
```

El hito H1 incluye una pantalla de prueba de input para descubrir los índices reales y completar este archivo.

## MIDI

### Transporte

| Vía | Estado | Requisitos |
|---|---|---|
| **USB host** (teclado o interfaz MIDI class-compliant conectada al USB-C con un adaptador OTG) | v1 | Kernel con `snd-usb-audio` y ALSA sequencer **(sin verificar en dArkOS)**. En Rust, el crate `midir` (usa ALSA) |
| **USB gadget** (la consola aparece como dispositivo MIDI ante una PC) | después | Función `f_midi` por configfs; depende de que el USB-C tenga modo periférico (UDC) **(sin verificar)**. Ver [hardware](../../hardware/rgb20s.md#pendientes) |
| DIN o TRS MIDI | — | La consola no tiene puertos MIDI; solo con una interfaz USB |

### Mensajes

| Mensaje | Dirección | Prioridad | Comportamiento |
|---|---|---|---|
| Timing Clock (`F8`) | salida | v1 | 24 PPQN mientras suena (uno cada 4 ticks internos) |
| Start (`FA`), Stop (`FC`), Continue (`FB`) | salida | v1 | Start al reproducir desde el principio; Continue al retomar |
| Timing Clock, Start, Stop | entrada | v1 | Modo "reloj externo" (ajuste en Proyecto). El tempo se deriva del promedio de los últimos 24 pulsos, suavizado |
| Song Position Pointer (`F2`) | ambas | después | Saltar a una posición de la canción |
| Note On / Note Off | entrada | v1 | Tocan la pista actual (canal configurable u omni), usando la bolsa de voces. Con grabación en vivo (S15), se escriben en la frase |
| Note On / Note Off | salida | v1 | Desde pistas de tipo MIDI out, un canal (1–16) por pista; los acordes salen como varias notas |
| Control Change | entrada | después | MIDI learn: asignar un CC a cualquier parámetro bloqueable |
| Control Change | salida | después | Los p-locks de una pista MIDI out salen como CC |
| Program Change | entrada | después | Saltar a una fila de la canción |
| Pitch Bend | entrada | después | Afinación de la pista actual |

Convención tentativa de CC (para "después"), tomada de la práctica habitual de General MIDI 2 **(sin verificar)**:
7 volumen, 10 paneo, 71 resonancia, 74 corte del filtro, 91 envío a reverb.

### Latencia y jitter

Los mensajes MIDI se leen en un hilo propio y entran al motor por la misma cola de comandos, con marca de tiempo.
El motor los aplica al inicio del siguiente subbloque (≤ 32 muestras, < 0,7 ms de jitter a 48 kHz). La salida de clock
se genera desde el hilo de audio y se envía por otra cola a un hilo de MIDI **(diseño a validar: el jitter real depende de ALSA)**.

## Preguntas abiertas

1. ¿Dónde guardar los datos en la consola: la **segunda microSD** (propuesto) o la primera? Depende de cómo monte dArkOS la TF2.
2. ¿Copiar los samples al proyecto (propuesto, proyectos portables) o referenciarlos desde la biblioteca (ahorra espacio)?
3. ¿MIDI en v1 es prioritario para vos? ¿Tenés un teclado o controlador USB para probarlo?
4. ¿Hace falta importar proyectos o samples de otros equipos (por ejemplo, kits de Elektron o de M8)? Hoy está fuera de alcance.

## Fuentes

- [RON (GitHub)](https://github.com/ron-rs/ron), [serde](https://serde.rs/), [hound (GitHub)](https://github.com/ruuda/hound), [midir (GitHub)](https://github.com/Boddlnagg/midir) (crates propuestos; versiones y API a verificar al implementar).
- [Especificación MIDI 1.0 (MIDI Association)](https://midi.org/midi-1-0) (mensajes de tiempo real, 24 PPQN).
- [Linux USB gadget configfs: f_midi (kernel.org)](https://www.kernel.org/doc/html/latest/usb/gadget-testing.html) (MIDI gadget).
- [ADR 0001 — Rust](../../decisions/0001-lenguaje-rust.md) (crates candidatos).
- [Hardware de la RGB20S](../../hardware/rgb20s.md), [sistemas operativos](../../firmware/sistemas-operativos.md).
