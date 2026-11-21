#!/usr/bin/env bash
# Deshace install-dtb-on-device.sh: vuelve a poner los DTB de un respaldo /boot/dtb-backup-*.
#     sudo ./rollback-dtb-on-device.sh                              # el respaldo más reciente
#     sudo ./rollback-dtb-on-device.sh /boot/dtb-backup-20261120-230000
# Después: sudo reboot. Si la consola no arranca, lo mismo a mano desde la Mac (ver README.md).
set -euo pipefail
say() { printf '\033[1;32m[rgb20s]\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m[rgb20s] ERROR:\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(id -u)" -eq 0 ] || die "correr con sudo"
mountpoint -q /boot || die "/boot no está montado"
bk="${1:-$(ls -d /boot/dtb-backup-* 2>/dev/null | sort -r | head -n1)}"
[ -n "$bk" ] && ls "$bk"/*.dtb >/dev/null 2>&1 || die "no hay respaldo con DTB en /boot/dtb-backup-*"

say "volviendo a los DTB de $bk"
for f in "$bk"/*.dtb; do
  dst="/boot/dtbs/rockchip/$(basename "$f")"
  cp "$f" "$dst.new" && sync && mv -f "$dst.new" "$dst"
done
# Los DTB que no existían antes (por ejemplo el -hpdet) se dejan: no los usa ninguna entrada.
sync
say "listo: sudo reboot"
