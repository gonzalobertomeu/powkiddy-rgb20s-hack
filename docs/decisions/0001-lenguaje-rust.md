# 0001 — Rust como lenguaje para las aplicaciones

- **Fecha:** 2026-11-19
- **Estado:** aceptada

## Contexto

Hay que elegir un lenguaje para las aplicaciones propias, empezando por el [groovebox](../solutions/groovebox.md).
Las alternativas eran C (con SDL2) y Rust.

## Decisión

**Rust** para las aplicaciones. Además, es una oportunidad para aprenderlo: el proyecto
se implementa en pareja con Claude, y los conceptos de Rust se explican a medida que aparecen.

C o shell quedan solo para tareas puntuales de bajo nivel:
- Configurar USB gadget (MIDI o HID): scripts de shell sobre configfs.
- Portar software existente en C/C++ (por ejemplo LGPT) o parchear el kernel o el device tree.

## Motivos

- **Audio en tiempo real con concurrencia:** el hilo de audio no puede bloquearse ni pedir memoria. Rust no deja compartir estado entre hilos de forma insegura, y `rtrb` (colas lock-free) sirve para mandar comandos de la interfaz al motor.
- **Modelo de datos rico** (patrones, pistas, instrumentos, proyectos): `enum`, `match` y `serde` lo hacen mucho más simple que C.
- **Rendimiento equivalente a C** en `--release`, que importa en un Cortex-A35.
- `aarch64-unknown-linux-gnu` es un **target Tier 1**, el nivel de soporte más alto.

## Consecuencias

- **No se compila Rust en la consola**: el A35 con 1 GB de RAM es demasiado lento y puede quedarse sin memoria. Siempre se hace compilación cruzada desde la Mac.
- **glibc**: el binario debe enlazarse contra una glibc igual o más vieja que la de dArkOS. Hay dos opciones:
  - un contenedor Docker arm64 con la misma base Debian que dArkOS, o
  - `cargo zigbuild --target aarch64-unknown-linux-gnu.<versión glibc>`.
- **SDL2**: enlazar contra la SDL2 de la consola, que trae soporte para su pantalla y sus controles. El contenedor con la misma base lo resuelve.
- Ciclo de desarrollo: `cargo run` nativo en la Mac → compilación cruzada → `rsync` a la consola.

## Crates candidatos

| Área | Crates |
|---|---|
| Audio | `cpal` (ALSA por debajo) o `alsa` directo |
| DSP y síntesis | `fundsp`, `dasp` |
| Comunicación entre hilos | `rtrb` |
| Pantalla e input | `sdl2` |
| Samples | `hound` (WAV) |
| Guardar proyectos | `serde` + `ron` o `toml` |

## Pendientes

- [ ] Confirmar la versión de glibc y SDL2 de dArkOS
- [ ] Elegir entre el contenedor arm64 y `cargo zigbuild`
