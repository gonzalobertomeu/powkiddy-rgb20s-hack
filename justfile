# Comandos de entrada del proyecto. Documentación: infra/README.md
# Está en la raíz porque `just` busca el justfile hacia arriba: funciona desde
# cualquier subcarpeta (por ejemplo projects/groovebox/).
#
# Variables (se pueden pasar como `just APP=otra build-arm` o por entorno):

set positional-arguments := true

# Crate a compilar: projects/<APP>/
APP := env("APP", "groovebox")
# Nombre del binario (por defecto, igual al crate)
BIN := env("BIN", APP)
# Escala de la ventana del simulador (640x480 lógicos por SCALE)
SCALE := env("RGB20S_SCALE", "2")
# Punto de montaje de la partición de ROMs de la microSD (ej. /run/media/$USER/EASYROMS)
SD_MOUNT := env("SD_MOUNT", "")
# Deploy por red: host SSH (entrada de ~/.ssh/config) y carpeta remota en la consola
RGB20S_HOST := env("RGB20S_HOST", "rgb20s")
RGB20S_DIR := env("RGB20S_DIR", "/home/alarm/" + APP)

root := justfile_directory()
infra := root / "infra"
crate := root / "projects" / APP
target := "aarch64-unknown-linux-gnu"
arm_bin := crate / "target/cross" / target / "release" / BIN
# Mapeo de teclado del simulador (keymap.toml estándar; keymap-vim.toml con hjkl)
KEYMAP := env("RGB20S_KEYMAP", infra / "sim" / "keymap.toml")

# Lista las recetas
default:
    @just --list --unsorted

# --- Imágenes de build ------------------------------------------------------

# Construye las dos imágenes de build (cross y arm64)
images: image-cross image-arm64

# Imagen x86_64 con gcc cruzado y libs arm64 de trixie (la que usa build-arm)
image-cross:
    #!/usr/bin/env bash
    set -euo pipefail
    set -a; . "{{infra}}/versions.env"; set +a
    docker build -t rgb20s-cross:dev -f "{{infra}}/docker/Dockerfile.cross" \
      --build-arg DEBIAN_IMAGE --build-arg DEBIAN_SNAPSHOT --build-arg RUST_VERSION \
      --build-arg RUSTUP_VERSION --build-arg RUSTUP_SHA256_AMD64 "{{infra}}/docker"

# Imagen arm64 completa, emulada con QEMU (referencia, lenta)
image-arm64:
    #!/usr/bin/env bash
    set -euo pipefail
    set -a; . "{{infra}}/versions.env"; set +a
    docker build -t rgb20s-arm64:dev -f "{{infra}}/docker/Dockerfile.arm64" \
      --build-arg DEBIAN_IMAGE --build-arg DEBIAN_SNAPSHOT --build-arg RUST_VERSION \
      --build-arg RUSTUP_VERSION --build-arg RUSTUP_SHA256_ARM64 "{{infra}}/docker"

# --- Nivel 1: nativo en la Mac (simulador) -----------------------------------

# Corre la app nativa en una ventana de 640x480 escalada (simulador)
run *args:
    cd "{{crate}}" && \
      RGB20S_SIM=1 RGB20S_SCALE="{{SCALE}}" RGB20S_KEYMAP="{{absolute_path(KEYMAP)}}" \
      SDL_RENDER_SCALE_QUALITY=nearest SDL_APP_ID=rgb20s-sim \
      cargo run -- "$@"

# Simulador con mapeo estilo Vim (hjkl = cruz, F = A, Espacio = Select)
run-vim *args:
    just KEYMAP="{{infra}}/sim/keymap-vim.toml" run {{args}}

# Tests de la lógica pura, nativos
test *args:
    cd "{{crate}}" && cargo test "$@"

# Verificación completa con salida corta: fmt, clippy, tests, build aarch64 y glibc (usar antes de entregar)
check:
    #!/usr/bin/env bash
    set -euo pipefail
    . "{{infra}}/versions.env"
    "{{infra}}/scripts/check.sh" "{{crate}}" "{{arm_bin}}" "$CONSOLE_GLIBC_MAX"

# Benchmark del motor de audio, nativo en release (en aarch64: just run-arm bench)
bench *args:
    cd "{{crate}}" && cargo run --release -- bench "$@"

# --- Nivel 2: binario aarch64 -------------------------------------------------

# Compila para la consola (release, aarch64, glibc/SDL2/ALSA de Debian trixie)
build-arm *args:
    "{{infra}}/scripts/docker-cargo.sh" cross "{{crate}}" cargo build --release "$@"
    @echo "binario: {{arm_bin}}"

# Igual que build-arm pero todo emulado en el contenedor arm64 (lento, para comparar)
build-arm-emu *args:
    "{{infra}}/scripts/docker-cargo.sh" arm64 "{{crate}}" cargo build --release "$@"

# Revisa el binario: arquitectura, glibc máxima y librerías dinámicas
check-arm:
    #!/usr/bin/env bash
    set -euo pipefail
    . "{{infra}}/versions.env"
    "{{infra}}/scripts/check-binary.sh" "{{arm_bin}}" "$CONSOLE_GLIBC_MAX"

# Corre el binario aarch64 con qemu-user (sin pantalla ni audio reales)
# Reenvía GROOVEBOX_* y RGB20S_*; la app corre en RUN_ARM_OUT (defecto infra/build/run-arm/)
run-arm *args:
    RUN_ARM_CWD="{{invocation_directory()}}" "{{infra}}/scripts/run-arm.sh" "{{arm_bin}}" "$@"

# Shell dentro del contenedor de build cruzado, en la carpeta del crate
shell-cross:
    "{{infra}}/scripts/docker-cargo.sh" cross "{{crate}}" bash

# --- Deploy -------------------------------------------------------------------

# Copia el binario y assets a la microSD montada (sin red con la consola)
deploy-sd: build-arm
    "{{infra}}/scripts/deploy-sd.sh" "{{SD_MOUNT}}" "{{APP}}" "{{arm_bin}}" "{{crate}}/assets" "{{crate}}"

# Deploy por SSH al host RGB20S_HOST (rsync o, si la consola no lo tiene, scp)
deploy: build-arm
    "{{infra}}/scripts/deploy-ssh.sh" "{{RGB20S_HOST}}" "{{RGB20S_DIR}}" "{{arm_bin}}" "{{crate}}/assets" "{{crate}}"

# --- Nivel 3: sistema completo -------------------------------------------------

# Arranca kernel + disco aarch64 en qemu-system-aarch64 -M virt -cpu cortex-a35
qemu-boot kernel disk *initrd:
    "{{infra}}/scripts/qemu-boot.sh" "{{kernel}}" "{{disk}}" {{initrd}}

# --- Imagen de Arch Linux ARM (infra/image/, projects/linux-image/) ---------------
# Todo corre en contenedores, sin sudo ni loop devices. Ver projects/linux-image/README.md.

# Contenedor de build de la imagen (x86_64: gcc cruzado, dtc, mtools, e2fsprogs)
image-builder:
    #!/usr/bin/env bash
    set -euo pipefail
    set -a; . "{{infra}}/versions.env"; set +a
    docker build -t rgb20s-image-builder:dev -f "{{infra}}/image/Dockerfile" \
      --build-arg DEBIAN_IMAGE --build-arg DEBIAN_SNAPSHOT "{{infra}}/docker"

# Descarga y verifica las fuentes (linux|uboot|rkbin|joypad|alarm|all)
img-fetch what="all":
    "{{infra}}/image/scripts/fetch.sh" "{{what}}"

# Compila U-Boot y arma uboot.bin (idbloader + uboot.img + trust.img)
img-uboot:
    "{{infra}}/image/scripts/in-builder.sh" bash infra/image/scripts/build-uboot.sh

# Splash de arranque: arte (make-splash.py) y binario rgb20s-splash (projects/linux-image/splash/)
img-splash:
    "{{infra}}/image/scripts/in-builder.sh" bash infra/image/scripts/build-splash.sh

# Compila el kernel (con el logo de arranque), los DTB de la RGB20S y el módulo del joypad
img-kernel:
    "{{infra}}/image/scripts/in-builder.sh" bash infra/image/scripts/build-kernel.sh

# Arma la rootfs de Arch Linux ARM (emulada, tarda; los módulos se agregan en img-assemble)
img-rootfs:
    "{{infra}}/image/scripts/build-rootfs.sh"

# Ensambla infra/build/out/rgb20s-arch.img con lo anterior
img-assemble:
    "{{infra}}/image/scripts/in-builder.sh" --root bash infra/image/scripts/build-image.sh

# Todo: fuentes, U-Boot, splash, kernel, rootfs e imagen
img: img-fetch img-uboot img-splash img-kernel img-rootfs img-assemble

# Paquete para actualizar una consola en uso sin reflashear (kernel, módulos, extlinux, splash):
# infra/build/out/device-update/ + install-on-device.sh. Requiere img-splash e img-kernel.
img-device-update:
    "{{infra}}/image/scripts/in-builder.sh" bash infra/image/scripts/build-device-update.sh

# Revisa la imagen sin hardware: offsets del bootloader, particiones, FAT y ext4
img-check:
    "{{infra}}/image/scripts/in-builder.sh" bash infra/image/scripts/check-image.sh
