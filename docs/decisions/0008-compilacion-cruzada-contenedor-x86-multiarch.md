# 0008 — Compilar para la consola en un contenedor x86_64 con gcc cruzado y librerías arm64 de Debian trixie

- **Fecha:** 2026-11-19
- **Estado:** aceptada (2026-11-19)

## Contexto

Las apps en Rust ([ADR 0001](0001-lenguaje-rust.md)) se compilan en la Mac (x86_64) para
`aarch64-unknown-linux-gnu` y se ejecutan en la consola. Restricciones:

- **glibc:** el binario tiene que pedir una glibc **menor o igual** que la de la consola.
- **SDL2:** se enlaza **dinámicamente** contra la `libSDL2` de la consola, que dArkOS compila con parches propios
  (2.32.10, commit `5d24957` de libsdl-org/SDL). También hay que enlazar `libasound` (ALSA, vía `cpal`).
- **Sistema de la consola sin confirmar:** ArkOS o dArkOS. dArkOS actual se construye sobre **Debian trixie**
  (glibc 2.41). ArkOS original es Ubuntu 19.10 (glibc 2.30) **(sin verificar)**.
- En el host no se instalan paquetes sin confirmación: no hay `aarch64-linux-gnu-gcc`, `zig` ni `just`.
- El usuario está aprendiendo Rust: el ciclo tiene que ser rápido y con pocas piezas.

## Alternativas consideradas

| Opción | A favor | En contra |
|---|---|---|
| **A. Contenedor arm64** (`debian:trixie` arm64 bajo qemu-user, `rustc` emulado) | Lo más fiel: es literalmente un Debian trixie arm64. Sin configuración de cross | **~23 veces más lento**: 7,5 min un build limpio y 12 s uno incremental (medido) |
| **B. Contenedor x86_64 + `gcc-aarch64-linux-gnu` + libs arm64 por multiarch** (`libc6:arm64`, `libsdl2-dev:arm64`, `libasound2-dev:arm64` de trixie) | Casi tan rápido como nativo. Linkea contra la glibc, SDL2 y ALSA de trixie, la base de dArkOS. Sin sysroot a mano: lo arma apt. Nada que instalar en el host. Los binarios corren emulados dentro del mismo contenedor | ~1,7 s de arranque del contenedor en cada comando. Exige glibc ≥ 2.34 en la consola: no sirve para un ArkOS viejo |
| **C. `cargo zigbuild` en el host** con `--target aarch64-unknown-linux-gnu.2.30` y un sysroot con SDL2/ALSA | El más rápido en incremental. Permite fijar una glibc **más vieja** (probado con 2.30): un mismo binario para ArkOS y dArkOS | Necesita `zig` (pacman o tarball) y `cargo-zigbuild` en el host, y un sysroot extraído y mantenido aparte. Con libs de trixie hay que pasar `-Wl,--allow-shlib-undefined` (si no, lld falla porque `libasound.so` pide `GLIBC_2.34`/`2.38`), lo que oculta errores de linkeo reales. Más piezas para alguien que está aprendiendo |

## Mediciones

Crate de prueba con las dependencias previstas para el groovebox: `sdl2 0.39` (dinámica), `cpal 0.18` (ALSA),
`fundsp 0.20`, `rtrb 0.3`, `serde`, `ron 0.11` y `hound 3` (87 crates en total). Perfil `--release`,
crates ya descargados, MacBook Pro i7-1068NG7 (4 núcleos / 8 hilos), 2026-11-19. Dos corridas por opción
(salvo A); los valores son muy estables.

| Opción | Build limpio | Incremental (`touch src/main.rs`) | glibc máxima del binario |
|---|---|---|---|
| Nativo x86_64 (referencia, `just run`) | 16,5 s / 16,9 s | 0,23 s | — |
| **A.** Contenedor arm64 emulado | **449 s** (7 min 29 s) | 12,1 s | 2.34 |
| **B.** Contenedor x86_64 + cross + multiarch | **19,5 s / 19,6 s** | 2,3 s (1,7 s son el arranque de Docker) | 2.34 |
| **C.** `cargo zigbuild` (glibc 2.30) | 17,4 s / 17,5 s | 0,35 s | 2.30 |

Construir las imágenes la primera vez (descarga desde `snapshot.debian.org`, las dos en paralelo):
`rgb20s-cross` 9 min 22 s y `rgb20s-arm64` 12 min 43 s.

Pruebas funcionales (todas pasaron con B y C):
- `file`: ELF 64-bit ARM aarch64, intérprete `/lib/ld-linux-aarch64.so.1`.
- `readelf -d` (B): `NEEDED` = `libSDL2-2.0.so.0`, `libasound.so.2`, `libgcc_s.so.1`, `libc.so.6`.
- Ejecución emulada con `qemu-aarch64-static` dentro de `rgb20s-cross`: carga SDL2 2.32.4 (la de trixie),
  abre el host ALSA de `cpal` y crea una ventana de 640×480 con el driver `offscreen`.
- El binario de C, compilado para glibc 2.30, también corre sobre la glibc 2.41 del contenedor.

Observación: aunque B y A linkean contra glibc 2.41, Rust y estas librerías solo piden símbolos hasta
`GLIBC_2.34`. Ese número puede subir si una dependencia futura usa funciones más nuevas: `just check-arm` lo controla.

## Decisión

Se usa la **opción B**: un contenedor x86_64 (`infra/docker/Dockerfile.cross`) con `gcc-aarch64-linux-gnu`
y las librerías arm64 de Debian trixie instaladas por multiarch, invocado con `just build-arm`.
El contenedor arm64 (A) queda como referencia (`just build-arm-emu`) y `cargo zigbuild` (C) como plan B
si la consola tiene una glibc menor que 2.34.

## Motivos

- **Compatibilidad:** linkea contra la misma base que dArkOS (trixie), con la misma glibc y la misma ABI de SDL2
  y ALSA. La SDL2 de dArkOS (2.32.10) es más nueva que la de trixie (2.32.4) y compatible hacia atrás.
- **Velocidad:** 23 veces más rápida que A en un build limpio y 5 veces en uno incremental; apenas 3 s
  más lenta que nativo en un build limpio.
- **Simplicidad:** un solo `Dockerfile`, sin sysroot a mano ni flags del linker especiales. No requiere instalar
  nada en el host. Todo el entorno (linker, `target-cpu=cortex-a35`, pkg-config) va en variables del contenedor:
  el crate no necesita `.cargo/config.toml`.
- **Pruebas:** el mismo contenedor corre los binarios aarch64 con qemu-user (`just run-arm`).
- C es la única que resuelve una consola con glibc vieja, pero suma herramientas al host y un flag que esconde
  errores de linkeo. No vale la pena mientras la consola sea (o pase a ser) dArkOS sobre trixie.

## Consecuencias

- El binario necesita **glibc ≥ 2.34** (hoy). Si el inventario muestra ArkOS con glibc 2.30, hay que pasar la
  consola a dArkOS o agregar una receta con `cargo zigbuild` (ver Pendientes).
- La primera construcción de la imagen tarda ~10 min; después solo cuesta el arranque del contenedor (~1,7 s).
- Versiones fijadas en `infra/versions.env`: `debian:trixie-20261104` por digest, `snapshot.debian.org` en
  `20261104T000000Z`, Rust 1.99.0 y rustup 1.29.3 con sha256. Actualizarlas es un cambio explícito.
- Los headers de SDL2 son los de trixie (2.32.4), no los de la consola (2.32.10). Funciones que solo existan
  en 2.32.10 no estarían disponibles; no es un problema para el `sdl2` crate, que apunta a una API más vieja.
- Docker 29 con el almacén de containerd no encuentra una imagen arm64 local con `--platform linux/arm64`:
  los scripts no pasan `--platform` al ejecutar.

## Pendientes

- [ ] Confirmar con el inventario de `device-ops` la glibc y la SDL2 de la consola, y ajustar `CONSOLE_GLIBC_MAX` en `infra/versions.env`
- [ ] Si la consola tiene glibc < 2.34: decidir entre pasar a dArkOS o agregar `just build-arm-zig` (requiere `zig` en el host)
- [ ] Primer deploy real a la consola con `just deploy-sd` para confirmar que el binario arranca con la SDL2 de dArkOS
- [ ] Revisar si conviene un sysroot sacado de la imagen real de dArkOS en vez de los paquetes de trixie (solo si aparece alguna incompatibilidad)

## Fuentes

- [dArkOS: Makefile (`DEBIAN_CODE_NAME ?= trixie`)](https://github.com/christianhaitian/dArkOS/blob/master/Makefile)
- [dArkOS: build_sdl2.sh](https://github.com/christianhaitian/dArkOS/blob/master/build_sdl2.sh)
- [rk3326_core_builds: scripts/sdl2.sh](https://github.com/christianhaitian/rk3326_core_builds/blob/master/scripts/sdl2.sh) (SDL 2.32.10)
- [dArkOS releases (base Debian trixie 13.x)](https://github.com/christianhaitian/dArkOS/releases)
- [Debian wiki: CrossCompiling / Multiarch](https://wiki.debian.org/CrossCompiling)
- [cargo-zigbuild (README: sufijo de versión de glibc)](https://github.com/rust-cross/cargo-zigbuild)
- [Zig 0.17.1 (descargas)](https://ziglang.org/download/)
- Mediciones propias en esta Mac (2026-11-19).
