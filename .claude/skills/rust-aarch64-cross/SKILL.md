---
name: rust-aarch64-cross
description: Compilación cruzada de Rust a aarch64-unknown-linux-gnu para la RGB20S. Cubre target, linker, glibc, sysroot, pkg-config/SDL2, cargo-zigbuild, contenedor arm64 y verificación del binario. Usar al armar o depurar builds para la consola.
---

# Compilación cruzada de Rust a la consola

Target: `aarch64-unknown-linux-gnu` (Tier 1). Rust **nunca** se compila en la consola.

## Opciones (medir y elegir con un ADR)

| Opción | Velocidad | Notas |
|---|---|---|
| **A. Contenedor arm64** (`docker run --platform linux/arm64`, debian:<versión de dArkOS>) | Lenta (QEMU emula todo `rustc`) | La más simple y fiel: las mismas librerías que la consola |
| **B. Compilación cruzada nativa con sysroot** | Rápida | `rustup target add aarch64-unknown-linux-gnu` más el linker cruzado más el sysroot extraído de la imagen de dArkOS |
| **C. `cargo zigbuild`** | Rápida | Zig hace de linker y permite fijar glibc: `cargo zigbuild --target aarch64-unknown-linux-gnu.2.36`. Para SDL2 sigue haciendo falta un sysroot |

## Config para la opción B (`.cargo/config.toml` del proyecto)

```toml
[target.aarch64-unknown-linux-gnu]
linker = "aarch64-linux-gnu-gcc"
rustflags = ["-C", "link-arg=--sysroot=/ruta/al/sysroot", "-C", "target-cpu=cortex-a35"]
```

Si crates como `sdl2` encuentran sus librerías con pkg-config:

```sh
export PKG_CONFIG_SYSROOT_DIR=/ruta/al/sysroot
export PKG_CONFIG_PATH=/ruta/al/sysroot/usr/lib/aarch64-linux-gnu/pkgconfig
export PKG_CONFIG_ALLOW_CROSS=1
```

`-C target-cpu=cortex-a35` habilita las optimizaciones del A35. NEON ya viene en aarch64 base.

## Sysroot

Se extrae `/usr/lib`, `/usr/include` y `/lib` de la rootfs de dArkOS (o de la imagen propia de Arch Linux ARM), montando la `.img` con un loop device en modo solo lectura, o desde un contenedor arm64 de la misma base con los paquetes `-dev` instalados.
No se commitea: va a `infra/` y está en `.gitignore`.

## Verificar el binario

```sh
file target/aarch64-unknown-linux-gnu/release/app      # ELF 64-bit ARM aarch64
objdump -T app | grep -o 'GLIBC_[0-9.]*' | sort -Vu | tail -1   # máxima glibc requerida
readelf -d app | grep NEEDED                             # librerías dinámicas requeridas
qemu-aarch64-static -L /ruta/al/sysroot ./app --help    # prueba rápida emulada
```

La glibc requerida tiene que ser **menor o igual** que la de la consola (`ldd --version` en la consola, dato de `device-ops`).

## Problemas comunes

- `version 'GLIBC_2.xx' not found` en la consola: el binario se compiló contra una glibc más nueva. Usar el sysroot de la consola o fijar la versión con zigbuild.
- La SDL2 de la consola trae parches para su pantalla y sus controles: enlazar dinámicamente contra **esa** librería, no linkear SDL2 estático.
- Lentitud en la opción A: cachear `~/.cargo/registry` y `target/` en volúmenes.
