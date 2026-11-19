# Groovebox

Estado: 🔬 investigando · Última revisión: 2026-11-19

Objetivo: convertir la RGB20S en un groovebox portátil para componer y hacer beats,
manejado con el d-pad y los botones (al estilo LSDJ / M8).

## Camino A — Software existente (para probar rápido)

| Opción | Notas |
|---|---|
| **LSDJ** en el emulador de Game Boy | Funciona ya. Es lo estándar en chiptune, aunque sigue siendo emulación |
| **LittleGPTracker (LGPT)** | Tracker pensado para controles de consola. Hay que compilarlo para aarch64 o buscar un port **(sin verificar)** |
| **SunVox** | Tiene build para Linux ARM64. Su interfaz pide mouse o pantalla táctil, así que con botones es incómoda **(sin verificar en el RK3326)** |
| **Pure Data** sin interfaz | Un patch propio controlado con los botones |
| MilkyTracker / Schism Tracker | Muy orientados a teclado |

## Camino B — Groovebox propio (el más interesante)

Stack tentativo:
- **C** con **SDL2** para pantalla e input, y **ALSA** o **miniaudio** para el audio.
- Latencia objetivo: unos 5–10 ms (por ejemplo, períodos de 256 frames a 44.1 kHz).

Features iniciales (MVP):
- Secuenciador de 16 pasos con 4–8 pistas.
- Patrones y encadenamiento de patrones (song mode).
- Motor de sonido: samples WAV, baterías sintetizadas tipo 808 y un sintetizador sustractivo o FM simple.
- Guardar y cargar proyectos en la SD.
- Interfaz pensada para el d-pad, en 640×480.

Ciclo de desarrollo: nativo en la Mac → compilar en un contenedor arm64 → probar en la consola (ver [emulación](../hardware/emulacion.md)).

## MIDI por USB-C

- **Host**: conectar un teclado o pad MIDI class-compliant para usar la consola como sintetizador. Requiere `snd-usb-audio` en el kernel.
- **Gadget**: que la consola aparezca como dispositivo MIDI USB ante el PC (configfs `f_midi`) y funcione como controlador o secuenciador para un DAW. Depende de que el puerto soporte modo periférico **(sin verificar)**.

## Preguntas abiertas

- ¿C o Lua/LÖVE? C da más rendimiento y control. LÖVE se itera más rápido (y PortMaster lo trae).
- ¿Cuántas voces simultáneas aguanta el A35? Hay que medirlo en el hardware.
- ¿Qué tan buena es la salida de audio (ruido, nivel) por el jack?

## Fuentes

- [PortMaster (GitHub)](https://github.com/christianhaitian/PortMaster)
- [PortMaster Starter Guide (Retro Game Corps)](https://retrogamecorps.com/2024/07/12/portmaster-starter-guide/)
