# Emulación y pruebas sin la consola

Última revisión: 2026-11-19

No existe un emulador de la placa RK3326 completa (QEMU no tiene esa máquina).
Sí se puede emular la **CPU y el sistema operativo**, en tres niveles. Los comandos están en
[`infra/README.md`](../../infra/README.md).

| Nivel | Cómo | Para qué | Comando |
|---|---|---|---|
| **1. Nativo x86** | Compilar con SDL2 en la Mac, ventana de 640×480 con escala entera y los botones mapeados al teclado | Desarrollar la interfaz y la lógica. Es el ciclo diario, el más rápido | `just run` |
| **2. Programas aarch64** | `qemu-aarch64-static` con binfmt, dentro del contenedor `rgb20s-cross` (glibc, SDL2 y ALSA arm64 de Debian trixie) | Probar que el binario ARM real carga sus librerías y corre su lógica | `just build-arm`, `just run-arm` |
| **3. Sistema completo** | `qemu-system-aarch64 -M virt -cpu cortex-a35` (QEMU soporta el A35 desde 2022) | Arrancar un Linux ARM entero: servicios, arranque, empaquetado | `just qemu-boot` |

## Limitaciones

- No se emula la GPU Mali, el códec de audio, los botones físicos ni el panel.
- En el nivel 2 no hay pantalla ni audio: SDL usa el driver `offscreen` y el audio `dummy`.
- **El rendimiento no es representativo**: la latencia de audio y el uso de CPU hay que medirlos en la consola real.

## Estado en la máquina de desarrollo (2026-11-19)

| Herramienta | Estado |
|---|---|
| `qemu-aarch64-static` | ✅ instalado |
| binfmt `qemu-aarch64` | ✅ registrado, flags `PF` (la `F` hace que funcione dentro de contenedores) |
| Docker 29 (buildx) | ✅ instalado |
| Imagen `rgb20s-cross:dev` | ✅ construida y probada (compila, linkea SDL2 y ALSA, corre el binario emulado) |
| Imagen `rgb20s-arm64:dev` | ✅ construida y probada (referencia, lenta) |
| SDL2 nativa (`sdl2-compat` 2.32.74) y `alsa-lib` | ✅ instaladas: el nivel 1 funciona |
| `just` | ❌ falta (`sudo pacman -S just`) |
| `qemu-system-aarch64` | ❌ falta (`sudo pacman -S qemu-system-aarch64`), solo para el nivel 3 |
| `aarch64-linux-gnu-gcc` | No hace falta en el host: está dentro de `rgb20s-cross` |

## Flujo

1. Desarrollar nativo en la Mac (nivel 1): `just run`.
2. Compilar para aarch64 en el contenedor x86_64 con gcc cruzado y las librerías arm64 de Debian trixie,
   la misma base que dArkOS (nivel 2): `just build-arm` y `just check-arm`.
   La elección y los tiempos medidos están en el [ADR 0008](../decisions/0008-compilacion-cruzada-contenedor-x86-multiarch.md).
3. Desplegar copiando a la microSD con el lector de la Mac (`just deploy-sd`): la consola no tiene Wi-Fi.
   El deploy por red (`just deploy`) queda para cuando haya red por USB.

## Fuentes

- [QEMU patch: target/arm: Add cortex-a35](https://patchew.org/QEMU/20220815213458.733701-1-wuhaotsh@google.com/)
- [QEMU virt board docs (mirror)](https://git.cs.tu-dortmund.de/SYS-OSS/FRET-qemu/src/branch/fret_v2/docs/system/arm/virt.rst)
- [ADR 0008: compilación cruzada](../decisions/0008-compilacion-cruzada-contenedor-x86-multiarch.md) (mediciones propias, 2026-11-19)
