# infra/ — entorno de desarrollo

Última revisión: 2026-11-19

Todo se maneja con el [`justfile`](../justfile) de la raíz del repo. La decisión de cómo se
compila para la consola está en el [ADR 0008](../docs/decisions/0008-compilacion-cruzada-contenedor-x86-multiarch.md);
los niveles de emulación, en [emulación](../docs/hardware/emulacion.md).

## Requisitos en la Mac

| Herramienta | Estado | Para qué |
|---|---|---|
| Docker (con buildx) | instalado | contenedores de build |
| `qemu-aarch64-static` + binfmt (flag `F`) | instalado | correr binarios aarch64 (nivel 2) y el contenedor arm64 |
| Rust (`rustup`, `cargo` 1.99) | instalado | `just run` nativo |
| SDL2 (`sdl2-compat`) y `alsa-lib` | instalados | `just run` nativo |
| `rsync` | instalado | deploy |
| **`just`** | **falta** | comandos de entrada: `sudo pacman -S just` |
| `qemu-system-aarch64` | falta (opcional) | nivel 3, `just qemu-boot`: `sudo pacman -S qemu-system-aarch64` |

No hace falta `aarch64-linux-gnu-gcc` en el host: el compilador cruzado vive en el contenedor.

## Primer uso

```sh
sudo pacman -S just          # una vez (requiere confirmación del usuario)
just images                  # construye rgb20s-cross y rgb20s-arm64 (medido: ~9 y ~13 min la primera vez)
just APP=groovebox build-arm # o simplemente `just build-arm`: APP=groovebox es el valor por defecto
```

`just` (sin argumentos) lista todas las recetas. Se puede correr desde cualquier subcarpeta del repo.

## Variables

Se pasan como `just VAR=valor receta` o por entorno.

| Variable | Defecto | Uso |
|---|---|---|
| `APP` | `groovebox` | crate a compilar: `projects/<APP>/` |
| `BIN` | igual a `APP` | nombre del binario |
| `RGB20S_SCALE` | `2` | escala de la ventana del simulador |
| `SD_MOUNT` | (vacío) | montaje de la partición con `ports/` de la microSD |
| `RGB20S_HOST` | `rgb20s` | host SSH para `just deploy` (a futuro) |
| `RGB20S_DIR` | `/roms/ports/<APP>` | carpeta remota para `just deploy` **(sin verificar)** |
| `RUN_ARM_OUT` | `infra/build/run-arm/` | directorio de trabajo de la app en `just run-arm`: ahí caen los archivos que genere (capturas del guion) |

## Recetas

| Receta | Qué hace |
|---|---|
| `just run [args]` | Nivel 1: corre la app nativa en la Mac, en el simulador de 640×480 |
| `just test [args]` | `cargo test` nativo |
| `just build-arm [args]` | Compila release para `aarch64-unknown-linux-gnu` en el contenedor `rgb20s-cross` |
| `just check-arm` | Muestra arquitectura, glibc máxima requerida y librerías dinámicas, y compara con la glibc de la consola |
| `just run-arm [args]` | Nivel 2: corre el binario aarch64 con `qemu-aarch64-static` dentro del contenedor (SDL `offscreen`; sin placa ALSA, así que la app sigue sin audio). Reenvía `GROOVEBOX_*` y `RGB20S_*`; la salida va a `RUN_ARM_OUT` |
| `just shell-cross` | Shell dentro del contenedor de build |
| `just build-arm-emu` | Igual que `build-arm` pero en el contenedor arm64 emulado (lento, solo para comparar) |
| `just deploy-sd` | Copia binario, licencias de terceros, assets y lanzador a la microSD montada (**vía principal**) |
| `just deploy` | rsync por SSH (**a futuro**, requiere red) |
| `just qemu-boot <kernel> <disco> [initramfs]` | Nivel 3: arranca un Linux aarch64 en `qemu-system-aarch64 -M virt -cpu cortex-a35` |
| `just images` / `image-cross` / `image-arm64` | (Re)construye las imágenes Docker |

Los artefactos van a `projects/<APP>/target/cross/` (o `target/arm64/`), separados del `target/` nativo.
El binario para la consola queda en `projects/<APP>/target/cross/aarch64-unknown-linux-gnu/release/<BIN>`.

## Cómo tiene que ser el crate para que `just build-arm` funcione

- Un crate binario en `projects/<APP>/` con `Cargo.toml` (edition 2024 funciona).
- `sdl2` **sin** la feature `bundled` ni `static-link`: se enlaza dinámicamente contra la `libSDL2-2.0.so.0`
  de la consola, que trae los parches de dArkOS para su pantalla y controles.
- `cpal` (o `alsa`) encuentra `libasound` por `pkg-config`; el contenedor ya trae `aarch64-linux-gnu-pkg-config`.
- No hace falta `.cargo/config.toml`: el contenedor define target, linker, `target-cpu=cortex-a35` y pkg-config
  por variables de entorno. Si el crate trae su propio `.cargo/config.toml`, no debe pisar el linker del target aarch64.
- Validado con un crate temporal con `sdl2 0.39`, `cpal 0.18`, `fundsp 0.20`, `rtrb 0.3`, `serde`, `ron 0.11` y `hound 3` (87 crates).

## Simulador en la Mac (nivel 1)

`just run` compila y corre la app **nativa** (x86_64) con estas variables:

| Variable | Valor | Significado |
|---|---|---|
| `RGB20S_SIM` | `1` | la app sabe que corre en la Mac (por ejemplo, para leer el teclado) |
| `RGB20S_SCALE` | `2` | factor de escala entero |
| `RGB20S_KEYMAP` | `infra/sim/keymap.toml` | mapeo teclado → botones |
| `SDL_RENDER_SCALE_QUALITY` | `nearest` | escalado sin suavizado (píxeles nítidos) |
| `SDL_APP_ID` | `rgb20s-sim` | clase (app_id) de la ventana en Wayland, para reglas de Hyprland |

### Contrato con la app

Para que la ventana se vea igual que la pantalla de la consola, la app tiene que:

1. Crear la ventana de `640·S × 480·S` (con `S = RGB20S_SCALE`, 1 si no está) y **no redimensionable**.
2. Dibujar siempre en 640×480 lógicos: `canvas.set_logical_size(640, 480)` y `canvas.set_integer_scale(true)`.
3. En la consola, usar pantalla completa (o 640×480 sin escala).

### Comportamiento medido en esta Mac (Hyprland 0.56.2, Wayland, monitor 2560×1600 a escala 2, `sdl2-compat` 2.32.74)

| `RGB20S_SCALE` | Ventana pedida | Tamaño en Hyprland (lógico) | Píxeles de render | Flotante |
|---|---|---|---|---|
| 1 | 640×480 | 320×240 | 640×480 | sí |
| 2 | 1280×960 | 640×480 | 1280×960 | sí |

- Con `sdl2-compat` el tamaño de ventana se interpreta en **píxeles físicos**: con `S=1` cada píxel de la
  consola es un píxel de la pantalla (se ve chica, 320×240 lógicos). Con `S=2` (el defecto) la ventana ocupa
  640×480 lógicos y cada píxel de la consola es un bloque de 2×2, sin suavizado.
- Hyprland pone **flotante** sola a la ventana no redimensionable: no hace falta una regla de ventana.
- La ventana corre en Wayland nativo (no XWayland).
- Si alguna vez hiciera falta una regla (por ejemplo para centrarla), la clase de la ventana se fija con
  `SDL_APP_ID=rgb20s-sim` (probado: Hyprland la ve como `rgb20s-sim`). En la config Lua de Omarchy sería algo como
  `hl.window_rule({ match = { class = "^rgb20s-sim$" }, float = true, center = true })` **(sin verificar)**.
  No se tocó la configuración de Hyprland.

### Mapeo de teclado

Definido en [`sim/keymap.toml`](sim/keymap.toml) (nombres de tecla de SDL). En la consola los mismos botones
llegan como eventos de gamepad de SDL: la app tiene que traducir las dos fuentes a un único `enum` de botones.

| Consola | Teclado |
|---|---|
| D-pad | flechas |
| A / B / X / Y | `X` / `Z` / `S` / `A` |
| L1 / R1 | `Q` / `W` |
| L2 / R2 | `1` / `2` |
| Select / Start | `Shift` derecho / `Enter` |
| Función (hotkey) **(sin verificar que exista)** | `Esc` |
| Stick izquierdo | `I` `J` `K` `L` |
| Stick derecho | `8` `4` `2` `6` del teclado numérico |

La disposición A/B/X/Y sigue la de la consola (estilo Nintendo: A a la derecha, B abajo) **(sin verificar en la RGB20S)**.

#### Mapeo estilo Vim (`just run-vim`)

[`sim/keymap-vim.toml`](sim/keymap-vim.toml): la mano derecha en `hjkl` y la izquierda en la fila central.
También se elige con `just KEYMAP=infra/sim/keymap-vim.toml run` o con `RGB20S_KEYMAP`.

| Consola | Teclado |
|---|---|
| D-pad | `h` `j` `k` `l` |
| A / B / X / Y | `F` / `D` / `S` / `A` |
| L2 / L1 / R1 / R2 | `W` / `E` / `R` / `T` |
| Select / Start | `Espacio` / `Enter` |
| Función | `Esc` |
| Stick izquierdo | flechas |
| Stick derecho | `8` `4` `2` `6` del teclado numérico |

## Compilar para la consola (nivel 2)

`just build-arm` corre `cargo build --release` en el contenedor **`rgb20s-cross`**
([`docker/Dockerfile.cross`](docker/Dockerfile.cross)):

- Base `debian:trixie` (la misma que dArkOS), fijada por digest; paquetes apt fijados con `snapshot.debian.org`
  en la fecha de [`versions.env`](versions.env).
- `gcc-aarch64-linux-gnu` como linker y las librerías **arm64** de trixie (`libc6`, `libsdl2-dev`,
  `libasound2-dev`) instaladas por multiarch: el linkeo usa exactamente la glibc 2.41, SDL2 y ALSA de trixie.
- `rustc` corre nativo en x86_64, así que compila casi tan rápido como en la Mac.

`just check-arm` imprime algo así (salida real del crate de prueba):

```
ELF 64-bit LSB pie executable, ARM aarch64, ... interpreter /lib/ld-linux-aarch64.so.1 ...
glibc máxima requerida: 2.34
librerías dinámicas:
  libSDL2-2.0.so.0
  libasound.so.2
  libgcc_s.so.1
  libc.so.6
OK: 2.34 <= 2.41 (glibc de la consola)
```

La glibc de la consola está en `CONSOLE_GLIBC_MAX` de `versions.env` (2.41 = dArkOS sobre trixie, **sin verificar**
hasta el inventario).

### Si la consola resulta tener ArkOS viejo (glibc 2.30)

El binario de `build-arm` pide glibc 2.34 y no arrancaría. Opciones, en orden:
1. Pasar la consola a dArkOS (recomendado en [sistemas operativos](../docs/firmware/sistemas-operativos.md)).
2. Compilar con `cargo zigbuild --target aarch64-unknown-linux-gnu.2.30`, que se probó y funciona
   (ver el ADR 0008); habría que agregarlo como receta.

### Tiempos medidos

Ver la tabla del [ADR 0008](../docs/decisions/0008-compilacion-cruzada-contenedor-x86-multiarch.md#mediciones).

## Correr binarios aarch64 sin la consola (nivel 2)

`just run-arm` ejecuta el binario dentro de `rgb20s-cross`: como el contenedor tiene `libc6:arm64` y las
librerías arm64, el loader `/lib/ld-linux-aarch64.so.1` existe y el binfmt del host (`qemu-aarch64-static`
con flag `F`) lo ejecuta directo. No hay pantalla ni audio: SDL usa `offscreen` y, como cpal habla directo con ALSA y no hay placa, la app arranca sin audio
(se pueden cambiar con `SDL_VIDEODRIVER` y `SDL_AUDIODRIVER`). Sirve para probar que el binario carga sus
librerías, la lógica y los tests de integración. **El rendimiento no es representativo.**

Variables y archivos:

- Se reenvían al contenedor **todas** las variables `GROOVEBOX_*` y `RGB20S_*` del entorno. Si el valor es una
  ruta que existe en el host, se traduce a la del contenedor: dentro del repo → `/work/...`, dentro de
  `RUN_ARM_OUT` → `/out/...`, y cualquier otra se monta en solo lectura en `/ext/N/`. Las rutas relativas se
  resuelven desde la carpeta donde se invocó `just` (no desde la raíz del repo). Los valores numéricos
  (`GROOVEBOX_FONT=12`, `GROOVEBOX_REPEAT=250,33`) nunca se tratan como rutas. Los `[args]` pasan sin traducir.
- La app corre con directorio de trabajo `/out`, montado desde `RUN_ARM_OUT` (por defecto
  `infra/build/run-arm/`, ignorado por git; se crea si no existe). Ahí quedan las capturas del modo guion.

```sh
# Demo del H1 emulada; las 9 capturas BMP quedan en infra/build/run-arm/ (~28 s medidos)
RGB20S_SIM=1 GROOVEBOX_AUDIO=0 GROOVEBOX_SCRIPT=projects/groovebox/scripts/demo-h1.txt just run-arm

# Otra carpeta de salida (relativa a donde se invoca, o absoluta)
RUN_ARM_OUT=/tmp/capturas GROOVEBOX_SCRIPT=scripts/demo-h1.txt just run-arm   # desde projects/groovebox/
```

## Deploy

### Vía principal: microSD (`just deploy-sd`)

La consola no tiene Wi-Fi, así que por ahora el binario se copia a la microSD con el lector de la Mac.

1. Apagar la consola, sacar la microSD y ponerla en el lector.
2. Ver qué partición tiene la carpeta `ports/` y montarla (sin root, con udisks):
   ```sh
   lsblk -o NAME,SIZE,FSTYPE,LABEL,MOUNTPOINTS
   udisksctl mount -b /dev/sdX3          # la partición de ROMs, normalmente etiqueta EASYROMS (sin verificar)
   ```
3. Compilar y copiar:
   ```sh
   SD_MOUNT=/run/media/$USER/EASYROMS just deploy-sd
   ```
   El script verifica que `SD_MOUNT` sea un punto de montaje real con `ports/` adentro, que el binario sea
   ELF aarch64, muestra el dispositivo y **pide confirmación** (`YES=1` la saltea). Solo escribe dentro de
   `<SD_MOUNT>/ports/`.
4. Desmontar: `udisksctl unmount -b /dev/sdX3`, y volver a poner la SD en la consola.

Lo que queda en la SD (convención de ports de ArkOS/dArkOS y PortMaster, **sin verificar en la RGB20S**):

```
<SD_MOUNT>/ports/Groovebox.sh          lanzador: aparece en el menú "Ports" de EmulationStation
<SD_MOUNT>/ports/groovebox/groovebox   binario aarch64
<SD_MOUNT>/ports/groovebox/LICENSE-spleen  licencias de terceros del crate (ver abajo)
<SD_MOUNT>/ports/groovebox/assets/     assets (si existe projects/groovebox/assets/)
<SD_MOUNT>/ports/groovebox/last-run.log  lo escribe la consola en cada ejecución
```

- **Licencias de terceros**: `deploy-sd` (y `deploy`) copian junto al binario todo archivo `LICENSE*`,
  `LICENCE*`, `COPYING*` o `NOTICE*` que haya en el crate hasta 3 niveles de profundidad (sin `target/` ni
  `.git/`); los lista [`scripts/list-licenses.sh`](scripts/list-licenses.sh). Así se cumple, por ejemplo, la
  BSD-2-Clause de la fuente Spleen embebida (`projects/groovebox/fonts/LICENSE-spleen`), que pide reproducir
  el aviso en las distribuciones binarias. Para declarar una licencia nueva alcanza con dejar el archivo en el
  crate con uno de esos nombres; como se copian planos, los nombres no pueden repetirse (usar
  `LICENSE-<origen>`) y el deploy falla si se repiten. **Pendiente**: las licencias de los crates de Rust
  enlazados (MIT/Apache-2.0 de `sdl2`, `cpal`, etc.) no se juntan todavía; habría que generarlas con una
  herramienta como `cargo-about`.
- El lanzador sale de [`launcher/port-launcher.sh.in`](launcher/port-launcher.sh.in). Como FAT/exFAT no guarda
  el bit de ejecución, copia el binario a `/tmp` y lo ejecuta desde ahí.
- **Sin red no hay SSH**: la salida de la app (stdout y stderr) queda en `last-run.log`, que se lee al volver a
  montar la SD en la Mac.
- En consolas con dos microSD, si se activó "Switch to SD2 for Roms" en ArkOS/dArkOS, la carpeta `ports/`
  que usa EmulationStation está en la **segunda** SD (`/roms2`), no en la de arranque **(sin verificar)**.
- Después de copiar, puede hacer falta refrescar la lista de juegos en EmulationStation para que aparezca el port.

### A futuro: deploy por red (`just deploy`)

`just deploy` hace `rsync` por SSH al host `RGB20S_HOST` (por defecto `rgb20s`, a definir en `~/.ssh/config`).
Hoy no funciona porque la consola no tiene red con la Mac. Se sabe que el puerto USB-C funciona como **host**
(la consola ya tuvo internet con el tethering USB de un Android, así que el kernel tiene un driver de red USB).
Vías posibles, todas **(sin verificar)**:

1. **USB gadget ECM/NCM**: la consola aparece como placa de red USB conectada a la Mac. Requiere que el RK3326
   tenga el UDC (controlador de dispositivo USB) habilitado en ese kernel y que el puerto soporte modo periférico.
2. **Adaptador USB-Ethernet** en la consola (OTG en modo host) y cable a la red de la Mac.
3. **Tethering USB de Android** en la consola y la Mac conectada al hotspot del mismo teléfono. Falta comprobar
   si el teléfono deja que los dos clientes se vean entre sí.

Pendiente: lo investiga `device-ops` en el inventario. Cuando haya red, definir `rgb20s` en `~/.ssh/config`
y confirmar la carpeta remota (`RGB20S_DIR`).

## Emulación del sistema completo (nivel 3)

`just qemu-boot <Image> <disco.img> [initramfs]` arranca `qemu-system-aarch64 -M virt -cpu cortex-a35 -smp 4 -m 1024`
con consola serie en la terminal (salir con `Ctrl-a x`), el disco en modo `snapshot=on` (no se modifica) y SSH
redirigido al puerto 2222 del host. Sirve para la rootfs y el kernel genéricos que genere `alarm-image-builder`;
no emula la placa RK3326 (ni pantalla, ni audio, ni botones). **No probado**: falta `qemu-system-aarch64`
(`sudo pacman -S qemu-system-aarch64`) y todavía no hay imágenes.

## Imagen de Arch Linux ARM (`infra/image/`)

Construye `infra/build/out/rgb20s-arch.img` (U-Boot + kernel + rootfs de Arch Linux ARM) sin `sudo` ni loop devices.
Qué lleva la imagen y cómo usarla: [`projects/linux-image/README.md`](../projects/linux-image/README.md); investigación y
riesgos: [`docs/firmware/arch-linux-arm.md`](../docs/firmware/arch-linux-arm.md).

| Receta | Qué hace | Dónde corre |
|---|---|---|
| `just image-builder` | Contenedor `rgb20s-image-builder:dev` (trixie x86_64 fijado como `rgb20s-cross`: gcc cruzado, dtc, mtools, e2fsprogs, sfdisk) | Docker |
| `just img-fetch [componente]` | Descarga y verifica fuentes a `infra/downloads/` (sha256, hash git, md5 + firma GPG de ALARM) | host |
| `just img-uboot` | U-Boot v2025.10 + rkbin → `infra/build/out/uboot/uboot.bin` | `rgb20s-image-builder` |
| `just img-kernel` | Kernel 7.1.2 + DTB + joypad → `infra/build/out/kernel/` | `rgb20s-image-builder` |
| `just img-rootfs` | `docker import` del tarball de ALARM, personalización emulada (arm64), `docker export` | contenedor arm64 |
| `just img-assemble` | Particiona un archivo (MBR), FAT con `mtools`, ext4 con `mke2fs -d`, `dd` del bootloader | `rgb20s-image-builder` (root del contenedor, sin `--privileged`) |
| `just img` | Todo lo anterior | |

Versiones fijadas en [`image/sources.lock`](image/sources.lock). Claves SSH locales en `infra/image/local/` (ignorada por git).

## Archivos

```
justfile                      (raíz) comandos de entrada
infra/versions.env            versiones fijadas: imagen Debian, snapshot apt, Rust, glibc de la consola
infra/docker/Dockerfile.cross contenedor de build elegido (x86_64 + gcc cruzado + libs arm64)
infra/docker/Dockerfile.arm64 contenedor arm64 emulado (referencia)
infra/docker/apt-snapshot.sh  apunta apt a snapshot.debian.org
infra/scripts/docker-cargo.sh corre cargo en un contenedor
infra/scripts/check-binary.sh revisa glibc y librerías de un binario
infra/scripts/run-arm.sh      corre un binario aarch64 emulado
infra/scripts/deploy-sd.sh    copia a la microSD montada
infra/scripts/list-licenses.sh  licencias de terceros que declara un crate
infra/scripts/deploy-ssh.sh   rsync por SSH (a futuro)
infra/scripts/qemu-boot.sh    nivel 3
infra/launcher/port-launcher.sh.in  plantilla del lanzador de ports
infra/sim/keymap.toml         mapeo de teclado del simulador
infra/device/inventory.sh     inventario de la consola (lo mantiene device-ops, ver docs/hardware/inventario.md)
infra/image/Dockerfile        contenedor de build de la imagen de Arch Linux ARM
infra/image/sources.lock      versiones y hashes de kernel, U-Boot, rkbin, joypad y ALARM
infra/image/scripts/          fetch, in-builder, build-uboot, build-kernel, build-rootfs, build-image
```

Las descargas (imágenes, tarballs, sysroots) van a `infra/downloads/` y `infra/sysroot/`, ignoradas por git.

## Fuentes

- [dArkOS: Makefile (`DEBIAN_CODE_NAME ?= trixie`)](https://github.com/christianhaitian/dArkOS/blob/master/Makefile)
- [dArkOS: build_sdl2.sh](https://github.com/christianhaitian/dArkOS/blob/master/build_sdl2.sh) y [rk3326_core_builds: scripts/sdl2.sh](https://github.com/christianhaitian/rk3326_core_builds/blob/master/scripts/sdl2.sh) (SDL 2.32.10)
- [ArkOS wiki: Emulators and Ports information](https://github.com/christianhaitian/arkos/wiki/ArkOS-Emulators-and-Ports-information) (carpeta `ports`, `roms2` en la segunda SD)
- [dArkOS R36 (fork): Firmware Installation](https://github.com/southoz/dArkOSRE-R36/wiki/Firmware-Installation) (etiquetas de la segunda SD)
- [Debian: CrossCompiling / Multiarch](https://wiki.debian.org/CrossCompiling)
- [snapshot.debian.org](https://snapshot.debian.org/)
- Mediciones propias en esta Mac (2026-11-19).
