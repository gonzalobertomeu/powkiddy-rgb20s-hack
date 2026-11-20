#!/usr/bin/env bash
# Compila U-Boot (mainline + defconfig/DT de ROCKNIX) y arma uboot.bin con el flujo
# "miniloader" de Rockchip, igual que ROCKNIX (bootloader/rkhelper):
#   idbloader.img = init de DDR (rkbin) + miniloader (rkbin)
#   uboot.img     = u-boot-dtb.bin empaquetado con loaderimage (rkbin)
#   trust.img     = BL31 (rkbin) empaquetado con trust_merger (rkbin)
# uboot.bin se escribe en el sector 64 de la tarjeta, y adentro lleva:
#   +0                    idbloader.img   -> sector absoluto 64
#   +16320 sectores       uboot.img       -> sector absoluto 16384 (8 MiB)
#   +24512 sectores       trust.img       -> sector absoluto 24576 (12 MiB)
# (offsets de doc/board/rockchip/rockchip.rst de U-Boot, "Package the image with Rockchip miniloader")
# Corre DENTRO de rgb20s-image-builder (in-builder.sh). Salida: infra/build/out/uboot/
set -euo pipefail
cd /work
. infra/image/sources.lock
li=projects/linux-image/uboot
b=infra/build/uboot
out=infra/build/out/uboot
ver="${UBOOT_VERSION#v}"
src="$b/u-boot-$ver"
rkbin="/work/infra/downloads/rkbin/$RKBIN_COMMIT"
jobs="${JOBS:-$(nproc)}"
log() { printf '\033[1;32m[u-boot]\033[0m %s\n' "$*"; }

DDR_BIN="$rkbin/bin/rk33/rk3326_ddr_333MHz_v2.12.bin"
MINILOADER="$rkbin/bin/rk33/rk3326_miniloader_v1.40.bin"
BL31="$rkbin/bin/rk33/rk3326_bl31_v1.37.elf"

log "extrayendo u-boot $UBOOT_VERSION"
rm -rf "$src"; mkdir -p "$b"
tar -xzf "infra/downloads/uboot/u-boot-$UBOOT_VERSION.tar.gz" -C "$b"
[ -d "$src" ] || mv "$b/u-boot-${UBOOT_VERSION}" "$src" 2>/dev/null || true
cp -r "$li"/rocknix/sources/. "$src/"
for p in "$li"/rocknix/patches/*.patch; do
  log "parche $(basename "$p")"
  patch -d "$src" -p1 --forward --no-backup-if-mismatch < "$p"
done

# U-Boot usa ARCH=arm también para aarch64 (el contenedor exporta ARCH=arm64 para el kernel)
mk() { make -C "$src" ARCH=arm CROSS_COMPILE=aarch64-linux-gnu- "$@"; }
mk rk3326-handheld_defconfig
log "compilando u-boot-dtb.bin"
mk -j"$jobs" u-boot-dtb.bin

rm -rf "$out"; mkdir -p "$out"
# Las herramientas de rkbin (binarios x86_64 cerrados) se copian y se ejecutan solo acá adentro
tools="/work/$b/rkbin-tools"
rm -rf "$tools"; mkdir -p "$tools"
install -m 0755 "$rkbin/tools/loaderimage" "$rkbin/tools/trust_merger" "$tools/"
cd "$src"
log "empaquetando con las herramientas de rkbin"
./tools/mkimage -n px30 -T rksd -d "$DDR_BIN" -C bzip2 idbloader.img
cat "$MINILOADER" >> idbloader.img
"$tools/loaderimage" --pack --uboot u-boot-dtb.bin uboot.img 0x00200000
cat > trust.ini <<EOF
[BL30_OPTION]
SEC=0
[BL31_OPTION]
SEC=1
PATH=$BL31
ADDR=0x00010000
[BL32_OPTION]
SEC=0
[BL33_OPTION]
SEC=0
[OUTPUT]
PATH=trust.img
EOF
"$tools/trust_merger" --verbose trust.ini

rm -f uboot.bin
dd if=idbloader.img of=uboot.bin bs=512 seek=0 conv=notrunc status=none
dd if=uboot.img     of=uboot.bin bs=512 seek=16320 conv=notrunc status=none
dd if=trust.img     of=uboot.bin bs=512 seek=24512 conv=notrunc status=none

# El bootloader tiene que terminar antes de la primera partición (sector 32768 = 16 MiB):
# uboot.bin empieza en el sector 64, así que puede ocupar como mucho 32704 sectores.
size=$(stat -c %s uboot.bin)
[ "$size" -le $(( (32768 - 64) * 512 )) ] || { echo "uboot.bin demasiado grande: $size bytes" >&2; exit 1; }

cp idbloader.img uboot.img trust.img uboot.bin u-boot-dtb.bin .config "/work/$out/"
cp tools/mkimage "/work/$out/mkimage"     # lo usa build-image.sh para boot.scr
sha256sum idbloader.img uboot.img trust.img uboot.bin > "/work/$out/SHA256SUMS"
log "listo"; ls -la "/work/$out"
