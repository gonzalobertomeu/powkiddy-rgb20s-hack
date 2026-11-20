#!/usr/bin/env bash
# Instala el kernel con logo propio, la línea de comandos silenciosa y el servicio rgb20s-splash en
# una RGB20S que YA corre la imagen de Arch Linux ARM, sin reflashear.
#
# Se corre EN LA CONSOLA, como root, desde la carpeta del paquete que arma
# `just img-device-update` (infra/build/out/device-update/):
#     sudo ./install-on-device.sh            # instala (no reinicia)
#     sudo ./install-on-device.sh --dry-run  # solo muestra qué haría
#
# Qué hace (idempotente: correrlo dos veces no rompe nada):
#   1. Verifica SHA256SUMS del paquete y que /boot sea la FAT de la imagen.
#   2. Si el Image cambia: respalda en /boot/backup-<fecha>/ el Image, los DTB, extlinux.conf y la
#      línea de comandos actual (/proc/cmdline), e instala el Image nuevo (y los DTB si cambian).
#   3. Módulos: si difieren de los instalados, mueve los actuales a /var/backups/rgb20s/ y pone los nuevos.
#   4. extlinux.conf: el del paquete (arch, arch-verbose, arch-rocknix) + "arch-prev", que arranca el
#      Image respaldado con la línea de comandos anterior. DEFAULT queda en "arch".
#   5. Instala y habilita rgb20s-splash.service (no lo arranca ahora: el launcher tiene la pantalla).
#
# Volver atrás: ./rollback-on-device.sh, o desde la Mac con la SD montada (ver README.md).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
DRY=0
[ "${1:-}" = "--dry-run" ] && DRY=1

say() { printf '\033[1;32m[rgb20s]\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[rgb20s] AVISO:\033[0m %s\n' "$*" >&2; }
die() { printf '\033[1;31m[rgb20s] ERROR:\033[0m %s\n' "$*" >&2; exit 1; }
# La imagen no trae diffutils (cmp/diff): se compara por sha256.
same_file() { [ -f "$1" ] && [ -f "$2" ] && [ "$(sha256sum < "$1")" = "$(sha256sum < "$2")" ]; }
run() {
  if [ "$DRY" = 1 ]; then echo "  (dry-run) $*"; else "$@"; fi
}

[ "$(id -u)" -eq 0 ] || die "correr con sudo"
cd "$here"

# --- 1. chequeos ---------------------------------------------------------------------------------
for f in Image kernel.release extlinux.conf modules.tar.gz rgb20s-splash splash.ppm \
         rgb20s-splash.service getty-tty1-splash.conf SHA256SUMS; do
  [ -e "$f" ] || die "falta $f en $here (¿se copió el paquete completo?)"
done
say "verificando SHA256SUMS"
sha256sum -c --quiet SHA256SUMS || die "el paquete está corrupto o incompleto"

# RGB20S_ASSUME_RGB20S=1 solo para probar el script fuera de la consola (en un contenedor)
[ "${RGB20S_ASSUME_RGB20S:-0}" = 1 ] || tr '\0' '\n' < /proc/device-tree/compatible 2>/dev/null | grep -qx 'powkiddy,rgb20s' \
  || die "esto no parece una RGB20S (/proc/device-tree/compatible)"
mountpoint -q /boot || die "/boot no está montado (es la FAT RGB20S_BOOT)"
[ -f /boot/extlinux/extlinux.conf ] && [ -f /boot/Image ] || die "/boot no tiene Image o extlinux/extlinux.conf"
grep -q 'APPEND ' /boot/extlinux/extlinux.conf || die "/boot/extlinux/extlinux.conf no tiene entradas APPEND"

krel="$(cat kernel.release)"
[ "$(uname -r)" = "$krel" ] || warn "el kernel en uso es $(uname -r) y el nuevo es $krel"

new_img_sha="$(sha256sum Image | cut -d' ' -f1)"
cur_img_sha="$(sha256sum /boot/Image | cut -d' ' -f1)"
need_kb=$(( $(stat -c %s Image) * 2 / 1024 + 4096 ))
free_kb="$(df -Pk /boot | awk 'NR==2 {print $4}')"
if [ "$new_img_sha" != "$cur_img_sha" ] && [ "$free_kb" -lt "$need_kb" ]; then
  die "/boot tiene ${free_kb} KiB libres; hacen falta ~${need_kb} KiB (borrar backups viejos /boot/backup-*)"
fi

ts="$(date +%Y%m%d-%H%M%S)"
changed=0

# --- 2. kernel y DTB -----------------------------------------------------------------------------
if [ "$new_img_sha" = "$cur_img_sha" ]; then
  say "Image: ya es el nuevo, no se toca"
else
  bk="/boot/backup-$ts"
  say "respaldando el arranque actual en $bk"
  run mkdir -p "$bk/extlinux"
  run cp /boot/Image "$bk/Image"
  run cp -r /boot/dtbs "$bk/dtbs"
  run cp /boot/extlinux/extlinux.conf "$bk/extlinux/extlinux.conf"
  if [ "$DRY" = 0 ]; then
    tr -d '\n' < /proc/cmdline > "$bk/cmdline.txt"
    echo "$cur_img_sha" > "$bk/Image.sha256"
  fi
  say "instalando Image nuevo ($krel)"
  run cp Image /boot/Image.new
  run sync
  run mv -f /boot/Image.new /boot/Image
  changed=1
fi
if [ -d dtbs ]; then
  for d in dtbs/rockchip/*.dtb; do
    dst="/boot/$d"
    if ! same_file "$d" "$dst"; then
      say "DTB cambia: $d"
      run install -D -m 0644 "$d" "$dst"
      changed=1
    fi
  done
fi
[ -f "config-$krel" ] && run cp "config-$krel" "/boot/config-$krel"

# --- 3. módulos ----------------------------------------------------------------------------------
moddir="/usr/lib/modules/$krel"
tmpmod="/usr/lib/modules/.rgb20s-new-$ts"
run mkdir -p "$tmpmod"
if [ "$DRY" = 0 ]; then
  tar --no-same-owner -xzf modules.tar.gz -C "$tmpmod"
  [ -f "$tmpmod/$krel/modules.dep" ] || { rm -rf "$tmpmod"; die "modules.tar.gz no trae $krel/modules.dep"; }
  sums() { (cd "$1" && find . -type f -print0 | sort -z | xargs -0 sha256sum); }
  if [ -d "$moddir" ] && [ "$(sums "$moddir")" = "$(sums "$tmpmod/$krel")" ]; then
    say "módulos: iguales a los instalados, no se tocan"
    rm -rf "$tmpmod"
  else
    mkdir -p /var/backups/rgb20s
    if [ -d "$moddir" ]; then
      say "módulos: respaldo de los actuales en /var/backups/rgb20s/modules-$krel-$ts"
      mv "$moddir" "/var/backups/rgb20s/modules-$krel-$ts"
    fi
    mv "$tmpmod/$krel" "$moddir"
    rm -rf "$tmpmod"
    say "módulos: instalados los nuevos ($krel)"
    changed=1
  fi
else
  echo "  (dry-run) extraer modules.tar.gz, comparar con $moddir y reemplazar si difiere"
  rmdir "$tmpmod" 2>/dev/null || true
fi

# --- 4. extlinux.conf ----------------------------------------------------------------------------
# "arch-prev": el último respaldo cuyo Image NO es el nuevo (al reinstalar, sigue apuntando al viejo).
prev=""
for d in $(ls -d /boot/backup-* 2>/dev/null | sort -r); do
  [ -f "$d/Image" ] && [ -f "$d/cmdline.txt" ] || continue
  [ "$(cat "$d/Image.sha256" 2>/dev/null)" = "$new_img_sha" ] && continue
  prev="$d"
  break
done
[ "$DRY" = 1 ] && [ -z "$prev" ] && [ "$new_img_sha" != "$cur_img_sha" ] && prev="/boot/backup-$ts"

newconf="$(mktemp)"
trap 'rm -f "$newconf"' EXIT
cat extlinux.conf > "$newconf"
if [ -n "$prev" ]; then
  rel="/${prev#/boot/}"
  prev_cmdline="$(cat "$prev/cmdline.txt" 2>/dev/null || tr -d '\n' < /proc/cmdline)"
  cat >> "$newconf" <<EOF

# Kernel anterior (respaldo de install-on-device.sh, $(basename "$prev")).
# Para volver: DEFAULT arch-prev (o ./rollback-on-device.sh).
LABEL arch-prev
  LINUX $rel/Image
  FDT $rel/dtbs/rockchip/rk3326-powkiddy-rgb20s-alarm.dtb
  APPEND $prev_cmdline
EOF
fi
grep -n ' $' "$newconf" && die "el extlinux.conf generado tiene espacios al final de línea"
if same_file "$newconf" /boot/extlinux/extlinux.conf; then
  say "extlinux.conf: sin cambios"
else
  [ -d "/boot/backup-$ts" ] || run mkdir -p "/boot/backup-$ts/extlinux"
  [ -f "/boot/backup-$ts/extlinux/extlinux.conf" ] || run cp /boot/extlinux/extlinux.conf "/boot/backup-$ts/extlinux/extlinux.conf"
  say "extlinux.conf nuevo (respaldo del anterior en /boot/backup-$ts/extlinux/)"
  if [ "$DRY" = 1 ]; then
    echo "--- extlinux.conf nuevo:"; cat "$newconf"
  else
    cp "$newconf" /boot/extlinux/extlinux.conf.new
    sync
    mv -f /boot/extlinux/extlinux.conf.new /boot/extlinux/extlinux.conf
  fi
  changed=1
fi

# --- 5. splash de userspace ----------------------------------------------------------------------
say "instalando rgb20s-splash"
run install -D -m 0755 rgb20s-splash /usr/local/bin/rgb20s-splash
run install -D -m 0644 splash.ppm /usr/local/share/rgb20s/splash.ppm
run install -D -m 0644 rgb20s-splash.service /etc/systemd/system/rgb20s-splash.service
run install -D -m 0644 getty-tty1-splash.conf /etc/systemd/system/getty@tty1.service.d/10-rgb20s-splash.conf
run systemctl daemon-reload
run systemctl enable rgb20s-splash.service
run sync

say "listo. Cambios en el arranque: $([ "$changed" = 1 ] && echo sí || echo no)."
grep -E '^(DEFAULT|LABEL)' /boot/extlinux/extlinux.conf | sed 's/^/    /'
cat <<EOF

Siguiente paso: sudo reboot
- Si arranca bien: logo propio al iniciar el kernel, el splash hasta el launcher.
  Revisar: journalctl -b -u rgb20s-splash ; dmesg | grep -iE 'fbcon|logo|Console: switching'
- Si no arranca: ver "Volver atrás" en el README (o DEFAULT arch-prev en extlinux.conf desde la Mac).
EOF
