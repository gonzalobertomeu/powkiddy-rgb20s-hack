#!/usr/bin/env bash
# Arma el paquete para actualizar una RGB20S que ya corre la imagen, sin reflashear:
# kernel (Image, DTB, módulos), extlinux.conf, rgb20s-splash y los scripts de instalación/rollback.
# Corre DENTRO de rgb20s-image-builder (in-builder.sh). Requiere build-splash.sh y build-kernel.sh.
# Salida: infra/build/out/device-update/ (se copia entera a la consola; ver
# projects/linux-image/splash/README.md).
set -euo pipefail
cd /work
o=infra/build/out
li=projects/linux-image
sp="$li/splash"
out="$o/device-update"
log() { printf '\033[1;32m[device-update]\033[0m %s\n' "$*"; }

for f in "$o/kernel/Image" "$o/kernel/kernel.release" "$o/splash/rgb20s-splash" "$o/splash/splash.ppm"; do
  [ -f "$f" ] || { echo "falta $f (correr just img-splash e img-kernel)" >&2; exit 2; }
done
krel="$(cat "$o/kernel/kernel.release")"
. "$li/boot/boot.env"

rm -rf "$out"; mkdir -p "$out/dtbs/rockchip"
cp "$o/kernel/Image" "$o/kernel/kernel.release" "$out/"
cp "$o/kernel/config" "$out/config-$krel"
cp "$o/kernel/dtbs/rockchip/"*.dtb "$out/dtbs/rockchip/"
log "módulos $krel"
tar --owner=0 --group=0 --numeric-owner --sort=name --mtime=@0 \
  -czf "$out/modules.tar.gz" -C "$o/kernel/modroot/usr/lib/modules" "$krel"
sed -e "s|@CMDLINE@|$CMDLINE|" -e "s|@CMDLINE_VERBOSE@|$CMDLINE_VERBOSE|" \
  "$li/boot/extlinux.conf.in" > "$out/extlinux.conf"
grep -q '@' "$out/extlinux.conf" && { echo "extlinux.conf: quedó un @MARCADOR@" >&2; exit 1; }
cp "$o/splash/rgb20s-splash" "$o/splash/splash.ppm" "$sp/system/rgb20s-splash.service" \
   "$sp/system/getty-tty1-splash.conf" "$sp/install-on-device.sh" "$sp/rollback-on-device.sh" "$out/"
chmod 0755 "$out/install-on-device.sh" "$out/rollback-on-device.sh" "$out/rgb20s-splash"
( cd "$out" && find . -type f ! -name SHA256SUMS -printf '%P\n' | sort | xargs sha256sum > SHA256SUMS )
log "listo: $out ($(du -sh "$out" | cut -f1))"
ls -la "$out"
