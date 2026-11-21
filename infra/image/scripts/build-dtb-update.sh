#!/usr/bin/env bash
# Paquete para cambiar SOLO el device tree de una RGB20S en uso (sin tocar kernel ni módulos),
# más las herramientas de diagnóstico del jack (projects/linux-image/jack/).
# Corre DENTRO de rgb20s-image-builder (in-builder.sh). Compila solo los DTB: necesita el árbol
# del kernel ya preparado por build-kernel.sh (`just img-kernel` al menos una vez).
# Salida: infra/build/out/dtb-update/ (se copia entera a la consola; ver projects/linux-image/jack/README.md).
set -euo pipefail
cd /work
. infra/image/sources.lock
li=projects/linux-image
src="infra/build/kernel/linux-$LINUX_VERSION"
out=infra/build/out/dtb-update
log() { printf '\033[1;32m[dtb-update]\033[0m %s\n' "$*"; }

[ -f "$src/.rgb20s-stamp" ] && [ -f "$src/.config" ] \
  || { echo "falta el árbol del kernel preparado en $src (correr just img-kernel)" >&2; exit 2; }

# Mismos pasos de DTS que build-kernel.sh (paso 2)
dts_dir="$src/arch/arm64/boot/dts/rockchip"
cp "$li"/kernel/rocknix/dts/* "$li"/kernel/dts/* "$dts_dir/"
dtbs=(rk3326-powkiddy-rgb20s rk3326-powkiddy-rgb20s-alarm rk3326-powkiddy-rgb20s-alarm-hpdet)
for d in "${dtbs[@]}"; do
  grep -q "+= $d.dtb\$" "$dts_dir/Makefile" || echo "dtb-\$(CONFIG_ARCH_ROCKCHIP) += $d.dtb" >> "$dts_dir/Makefile"
done
log "compilando DTB"
make -C "$src" -s $(printf 'rockchip/%s.dtb ' "${dtbs[@]}")

rm -rf "$out"; mkdir -p "$out/dtbs/rockchip"
for d in "${dtbs[@]}"; do cp "$dts_dir/$d.dtb" "$out/dtbs/rockchip/"; done

# Chequeos del contenido (dtc decompila; fdtget no está garantizado en el contenedor)
dts_of() { "$src/scripts/dtc/dtc" -I dtb -O dts "$out/dtbs/rockchip/$1.dtb" 2>/dev/null; }
dts_of rk3326-powkiddy-rgb20s | grep -q 'hp-det-gpios' \
  || { echo "el DTB de ROCKNIX debería conservar hp-det-gpios" >&2; exit 1; }
dts_of rk3326-powkiddy-rgb20s-alarm | grep -q 'hp-det-gpios' \
  && { echo "el DTB propio todavía tiene hp-det-gpios" >&2; exit 1; }
dts_of rk3326-powkiddy-rgb20s-alarm-hpdet | grep -q 'headphone-detect' \
  || { echo "el DTB -hpdet no tiene el nodo headphone-detect" >&2; exit 1; }

log "compilando rgb20s-gpio-probe"
aarch64-linux-gnu-gcc -std=c11 -O2 -Wall -Wextra -Werror -static -s \
  -o "$out/rgb20s-gpio-probe" "$li/jack/rgb20s-gpio-probe.c"
file "$out/rgb20s-gpio-probe" | grep -q 'ARM aarch64.*statically linked' \
  || { echo "rgb20s-gpio-probe no es un binario aarch64 estático" >&2; exit 1; }

cp "$li/jack/jack-diagnose.sh" "$li/jack/install-dtb-on-device.sh" "$li/jack/rollback-dtb-on-device.sh" "$out/"
chmod 0755 "$out"/*.sh "$out/rgb20s-gpio-probe"
( cd "$out" && find . -type f ! -name SHA256SUMS -printf '%P\n' | sort | xargs sha256sum > SHA256SUMS )
log "listo: $out"
ls -la "$out" "$out/dtbs/rockchip"
