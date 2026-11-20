#!/usr/bin/env bash
# Deshace install-on-device.sh en la consola (por SSH): vuelve al Image, DTB, extlinux.conf y módulos
# del último respaldo y deshabilita rgb20s-splash. Correr con sudo; después, sudo reboot.
#     sudo ./rollback-on-device.sh                 # último /boot/backup-*
#     sudo ./rollback-on-device.sh /boot/backup-20261120-180000
# Si la consola no arranca, se hace lo mismo a mano desde la Mac (ver README.md, "Volver atrás").
set -euo pipefail
say() { printf '\033[1;32m[rgb20s]\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m[rgb20s] ERROR:\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(id -u)" -eq 0 ] || die "correr con sudo"
mountpoint -q /boot || die "/boot no está montado"

bk="${1:-}"
if [ -z "$bk" ]; then
  for d in $(ls -d /boot/backup-* 2>/dev/null | sort -r); do
    [ -f "$d/Image" ] && { bk="$d"; break; }
  done
fi
[ -n "$bk" ] && [ -f "$bk/Image" ] || die "no hay respaldo con Image en /boot/backup-*"
say "volviendo a $bk"

cp "$bk/Image" /boot/Image.new && sync && mv -f /boot/Image.new /boot/Image
[ -d "$bk/dtbs" ] && cp -r "$bk/dtbs/." /boot/dtbs/
if [ -f "$bk/extlinux/extlinux.conf" ]; then
  cp "$bk/extlinux/extlinux.conf" /boot/extlinux/extlinux.conf.new && sync
  mv -f /boot/extlinux/extlinux.conf.new /boot/extlinux/extlinux.conf
fi

# Módulos: el respaldo de la misma instalación (modules-<versión>-<fecha del backup de /boot>)
ts="${bk##*/backup-}"
mb="$(ls -d /var/backups/rgb20s/modules-*-"$ts" 2>/dev/null | head -n1 || true)"
krel=""
if [ -n "$mb" ]; then
  krel="$(basename "$mb" | sed -E 's/^modules-(.*)-[0-9]{8}-[0-9]{6}$/\1/')"
fi
if [ -n "$mb" ] && [ -n "$krel" ]; then
  say "módulos: vuelvo a $mb"
  rm -rf "/usr/lib/modules/$krel.rgb20s-old"
  mv "/usr/lib/modules/$krel" "/usr/lib/modules/$krel.rgb20s-old"
  mv "$mb" "/usr/lib/modules/$krel"
  rm -rf "/usr/lib/modules/$krel.rgb20s-old"
else
  say "módulos: no hay respaldo (no habían cambiado), no se tocan"
fi

systemctl disable rgb20s-splash.service 2>/dev/null || true
sync
say "listo: sudo reboot"
