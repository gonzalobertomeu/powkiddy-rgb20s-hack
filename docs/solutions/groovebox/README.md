# Groovebox

Estado: 🔬 investigando (spec del MVP en borrador, con las primeras decisiones del usuario) · Última revisión: 2026-11-19

Objetivo: convertir la RGB20S en un groovebox portátil para componer y hacer beats, manejado con la cruz,
los botones y los sticks. Combina una grilla de pasos (para ritmos) con la estructura de un tracker portátil
(canción → cadenas por pista → frases) y es polifónico.

Este documento reemplaza al antiguo `docs/solutions/groovebox.md`, que ahora solo redirige acá.

## Documentos

| Documento | Contenido |
|---|---|
| [Referentes](referentes.md) | Trackers, grooveboxes y secuenciadores: fichas, comparación y qué tomamos de cada uno |
| [Spec de producto](spec-producto.md) | Visión, decisiones del usuario, flujos, features priorizadas (MVP, v1, después), **hitos** y riesgos |
| [Spec de interfaz](spec-ui.md) | Mockups de 640×480, mapa de pantallas y **mapa completo de controles** por pantalla y modo |
| [Spec del motor](spec-motor.md) | Arquitectura, **voces y polifonía**, instrumentos, efectos, **secuenciador**, modelo de datos y **presupuesto de CPU** |
| [Spec de formatos](spec-formatos.md) | Formato de proyecto (RON), samples, configuración y MIDI |
| [DSP del motor](dsp.md) | Lo implementado en H2: bolsa de voces, percusión, sinte, master, denormales, costos medidos y fuentes |

## Decisiones

| ADR | Tema | Estado |
|---|---|---|
| [0001](../../decisions/0001-lenguaje-rust.md) | Rust como lenguaje | aceptada |
| [0002](../../decisions/0002-interfaz-hibrida-grilla-tracker.md) | Interfaz híbrida grilla + tracker y esquema de botones | aceptada |
| [0003](../../decisions/0003-modelo-de-secuenciacion.md) | Patrón global y pistas monofónicas | reemplazada por 0006 y 0007 |
| [0004](../../decisions/0004-formato-de-proyecto-ron.md) | Proyectos en RON, autocontenidos y versionados | aceptada |
| [0005](../../decisions/0005-configuracion-de-audio.md) | 48 kHz, `f32`, buffer de 256, secuenciador en el hilo de audio | aceptada |
| [0006](../../decisions/0006-cadenas-por-pista.md) | Canción de cadenas por pista: canción → cadenas → frases | aceptada (detalles propuestos) |
| [0007](../../decisions/0007-polifonia-en-el-mvp.md) | Polifonía en el MVP: bolsa de voces y acordes por paso | aceptada (detalles propuestos) |
| [0013](../../decisions/0013-ranuras-de-fundido.md) | 16 ranuras de fundido en lugar de 4 (sin clicks al robar muchas voces a la vez) | aceptada |

## El MVP en una línea

8 pistas (percusión sintetizada, sampler y sintetizador sustractivo) con **polifonía** (bolsa de 16 voces y acordes por paso);
frases de 16 pasos con nota, acorde, velocidad y largo; **cadenas** de frases con transposición; **canción** de cadenas por pista;
una **Grilla** que muestra la sección actual de las 8 pistas y crea lo que falta; swing; mezcla con delay y reverb por envío; sticks
para el filtro y los envíos en vivo; y proyectos guardados en RON. P-locks en v1.
Hitos H0 a H11 en la [spec de producto](spec-producto.md#hitos); el objetivo mínimo ("un beat de 4 compases guardado") se cumple en H6.

## Camino A — Software existente

Para probar rápido, o para calibrar la ergonomía antes de implementar:

| Opción | Notas |
|---|---|
| **LSDJ** en el emulador de Game Boy | Funciona ya. Es lo estándar en chiptune, aunque sigue siendo emulación. Usa el mismo modelo de cadenas que elegimos |
| **LittleGPTracker (Little Piggy Tracker)** | Tracker pensado para controles de consola, con el modelo de cadenas de LSDJ. Hay builds para varias portátiles y para PortMaster **(sin verificar para la RGB20S)** |
| **SunVox** | Tiene build para Linux ARM64. Su interfaz pide mouse o pantalla táctil, así que con botones es incómoda **(sin verificar en el RK3326)** |
| **Pure Data** sin interfaz | Un patch propio controlado con los botones |
| MilkyTracker / Schism Tracker | Muy orientados a teclado |

## Camino B — Groovebox propio

Stack (lenguaje decidido en [ADR 0001](../../decisions/0001-lenguaje-rust.md)):
- **Rust**, con `sdl2` para pantalla e input y `cpal`/ALSA para el audio.
- Comunicación entre la interfaz y el audio con colas lock-free (`rtrb`).
- Latencia objetivo: ~5–10 ms por período (256 frames a 48 kHz = 5,3 ms; ver [ADR 0005](../../decisions/0005-configuracion-de-audio.md)).

Ciclo de desarrollo: nativo en la Mac → compilar en un contenedor arm64 → probar en la consola (ver [emulación](../../hardware/emulacion.md)).
El código irá en `projects/groovebox/`.

## MIDI por USB-C

- **Host**: conectar un teclado o pad MIDI class-compliant para usar la consola como sintetizador. Requiere `snd-usb-audio` en el kernel. Prioridad v1.
- **Gadget**: que la consola aparezca como dispositivo MIDI USB ante el PC (configfs `f_midi`) y funcione como controlador o secuenciador para un DAW. Depende de que el puerto soporte modo periférico **(sin verificar)**. Prioridad: después.

Detalle de mensajes y prioridades en [spec-formatos](spec-formatos.md#midi).

## Preguntas abiertas (resumen)

Las preguntas completas están al final de cada documento.

Respondidas el 2026-11-19:
- ~~¿C o Lua/LÖVE?~~ → Rust ([ADR 0001](../../decisions/0001-lenguaje-rust.md)).
- ~~¿Interfaz híbrida o un solo paradigma?~~ → híbrida ([ADR 0002](../../decisions/0002-interfaz-hibrida-grilla-tracker.md)).
- ~~¿Patrón global o cadenas por pista?~~ → cadenas por pista ([ADR 0006](../../decisions/0006-cadenas-por-pista.md)).
- ~~¿Polifonía en el MVP?~~ → sí ([ADR 0007](../../decisions/0007-polifonia-en-el-mvp.md)).
- ~~¿P-locks en el MVP o en v1?~~ → v1.
- ~~¿Instrumento fijo por pista o columna INST?~~ → fijo por pista ([ADR 0006](../../decisions/0006-cadenas-por-pista.md)).
- ~~¿Qué pasa al llegar a una celda vacía de la canción?~~ → vuelve al inicio del bloque ([ADR 0006](../../decisions/0006-cadenas-por-pista.md)).
- ~~¿Creación implícita desde la Grilla?~~ → sí ([ADR 0006](../../decisions/0006-cadenas-por-pista.md)).
- ~~¿Cómo se escriben los acordes?~~ → columna ACORDE con tipos predefinidos e inversión ([ADR 0007](../../decisions/0007-polifonia-en-el-mvp.md)).

Pendientes (las que más condicionan el trabajo):
1. ¿Frases de 16 pasos fijos en el MVP y transposición por entrada de cadena (propuesto)?
4. ~~Tamaño de la bolsa de voces~~: 16 voces + 16 ranuras de fundido (ADR 0013, aceptado 2026-11-20).
5. Start contextual por pantalla (propuesto).
6. ¿Sticks sobre la pista actual (filtro y envíos, propuesto) o sobre el master?
7. ¿Borrar con A + B (propuesto) o con un toque de A sobre un paso con nota?
8. Tamaño de fuente (8×16 o 12×24): se decide probando en la consola (hito H1).
9. ¿Valores 0–127 o 0–99? ¿Dónde guardar los datos (segunda microSD, propuesto)? ¿Copiar los samples al proyecto (propuesto)?
10. 48 kHz o 44,1 kHz; límite de 128 MB para samples; prioridad de MIDI en v1.
11. Nombre del instrumento y estilo del kit por defecto.

Pendientes de medición (no son decisiones del usuario):
- ¿Cuántas voces aguanta el A35 en la práctica? La estimación da ~24 % de un núcleo en el peor caso del MVP (bolsa llena) ([spec-motor](spec-motor.md#presupuesto-de-cpu)).
- ¿Qué tan buena es la salida de audio (ruido, nivel) por el jack y el parlante?

## Fuentes

- [PortMaster (GitHub)](https://github.com/christianhaitian/PortMaster)
- [PortMaster Starter Guide (Retro Game Corps)](https://retrogamecorps.com/2024/07/12/portmaster-starter-guide/)
- [LittleGPTracker (GitHub)](https://github.com/djdiskmachine/LittleGPTracker)
- Las fuentes de cada tema están al final de cada documento.
