# Emulación y pruebas sin la consola

Última revisión: 2026-11-19

No existe un emulador de la placa RK3326 completa (QEMU no tiene esa máquina).
Sí se puede emular la **CPU y el sistema operativo**, en tres niveles:

| Nivel | Cómo | Para qué |
|---|---|---|
| **1. Nativo x86** | Compilar con SDL2 en la Mac, en una ventana de 640×480, con los botones mapeados al teclado | Desarrollar la interfaz y la lógica. Es el ciclo diario, el más rápido |
| **2. Programas aarch64** | `qemu-aarch64-static` con binfmt, más un chroot o un contenedor Docker `--platform linux/arm64` con base Debian/dArkOS | Compilar y probar binarios ARM reales con las mismas librerías que la consola |
| **3. Sistema completo** | `qemu-system-aarch64 -M virt -cpu cortex-a35` (QEMU soporta el A35 desde 2022) | Arrancar un Linux ARM entero: servicios, arranque, empaquetado |

## Limitaciones

- No se emula la GPU Mali, el códec de audio, los botones físicos ni el panel.
- **El rendimiento no es representativo**: la latencia de audio y el uso de CPU hay que medirlos en la consola real.

## Estado en la máquina de desarrollo (2026-11-19)

| Herramienta | Estado |
|---|---|
| `qemu-aarch64-static` | ✅ instalado |
| binfmt `qemu-aarch64` | ✅ registrado |
| Docker | ✅ instalado |
| `qemu-system-aarch64` | ❌ falta (`pacman -S qemu-system-aarch64`) |
| `aarch64-linux-gnu-gcc` | ❌ falta (se puede compilar dentro del contenedor arm64) |

## Flujo propuesto

1. Desarrollar nativo en la Mac (nivel 1).
2. Compilar para aarch64 en un contenedor Docker arm64 con base Debian, igual que dArkOS (nivel 2).
3. Desplegar en la consola por Wi-Fi con `scp` o `rsync` y probar en el hardware real.

## Fuentes

- [QEMU patch: target/arm: Add cortex-a35](https://patchew.org/QEMU/20220815213458.733701-1-wuhaotsh@google.com/)
- [QEMU virt board docs (mirror)](https://git.cs.tu-dortmund.de/SYS-OSS/FRET-qemu/src/branch/fret_v2/docs/system/arm/virt.rst)
