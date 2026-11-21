#!/usr/bin/env bash
# Instala SOLO los device trees nuevos en una RGB20S que ya corre la imagen (no toca kernel,
# módulos ni extlinux.conf). Se corre EN LA CONSOLA, como root, desde la carpeta del paquete que
# arma `just img-dtb-update` (infra/build/out/dtb-update/):
#     sudo ./install-dtb-on-device.sh             # DTB propio sin detección de jack (recomendado)
#     sudo ./install-dtb-on-device.sh --hpdet     # variante experimental: jack por gpio-keys en GPIO2_C6
#     sudo ./install-dtb-on-device.sh --dry-run   # solo muestra qué haría (combinable con --hpdet)
#
# Respaldo: /boot/dtb-backup-<fecha>/ (los DTB actuales). Volver atrás: ./rollback-dtb-on-device.sh
# El cambio aplica al reiniciar. Si la consola no arrancara, la entrada "arch-rocknix" de
# extlinux.conf usa el DTB de ROCKNIX (también incluido, sin cambios de jack); ver README.md.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
DRY=0
HPDET=0
for a in "$@"; do
  case "$a" in
    --dry-run) DRY=1 ;;
    --hpdet) HPDET=1 ;;
    *) echo "opción desconocida: $a" >&2; exit 2 ;;
  esac
done

say() { printf '\033[1;32m[rgb20s]\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m[rgb20s] ERROR:\033[0m %s\n' "$*" >&2; exit 1; }
same_file() { [ -f "$1" ] && [ -f "$2" ] && [ "$(sha256sum < "$1")" = "$(sha256sum < "$2")" ]; }
run() { if [ "$DRY" = 1 ]; then echo "  (dry-run) $*"; else "$@"; fi; }

[ "$(id -u)" -eq 0 ] || die "correr con sudo"
cd "$here"
say "verificando SHA256SUMS"
sha256sum -c --quiet SHA256SUMS || die "el paquete está corrupto o incompleto"
[ "${RGB20S_ASSUME_RGB20S:-0}" = 1 ] || tr '\0' '\n' < /proc/device-tree/compatible 2>/dev/null | grep -qx 'powkiddy,rgb20s' \
  || die "esto no parece una RGB20S (/proc/device-tree/compatible)"
mountpoint -q /boot || die "/boot no está montado (es la FAT RGB20S_BOOT)"
[ -d /boot/dtbs/rockchip ] || die "/boot/dtbs/rockchip no existe"

main=rk3326-powkiddy-rgb20s-alarm.dtb
src_main="dtbs/rockchip/$main"
[ "$HPDET" = 1 ] && src_main="dtbs/rockchip/rk3326-powkiddy-rgb20s-alarm-hpdet.dtb"
[ -f "$src_main" ] || die "falta $src_main"

# Qué va a cambiar: el DTB que usa "arch" (alarm) y los demás si difieren
declare -A plan
plan["/boot/dtbs/rockchip/$main"]="$src_main"
for d in dtbs/rockchip/*.dtb; do
  [ "$(basename "$d")" = "$main" ] && continue
  plan["/boot/$d"]="$d"
done
changes=()
for dst in "${!plan[@]}"; do
  same_file "${plan[$dst]}" "$dst" || changes+=("$dst")
done
if [ "${#changes[@]}" -eq 0 ]; then
  say "los DTB ya son los del paquete; nada que hacer"
  exit 0
fi

ts="$(date +%Y%m%d-%H%M%S)"
bk="/boot/dtb-backup-$ts"
say "respaldando /boot/dtbs/rockchip en $bk"
run mkdir -p "$bk"
run cp /boot/dtbs/rockchip/*.dtb "$bk/"
for dst in "${changes[@]}"; do
  say "instalando $(basename "${plan[$dst]}") -> $dst"
  run cp "${plan[$dst]}" "$dst.new"
  run sync
  run mv -f "$dst.new" "$dst"
done
run sync

say "listo. Variante: $([ "$HPDET" = 1 ] && echo 'hpdet (gpio-keys en GPIO2_C6, experimental)' || echo 'sin detección de jack')"
cat <<EOF

Siguiente paso: sudo reboot. Después, comprobar:
  - Sonido por el parlante:    speaker-test -D plughw:0,0 -c 2 -t wav -l 1   (con el launcher/app cerrado)
  - El jack ya no está en ALSA: amixer -c 0 contents | grep -c 'Headphones Jack'   (esperado: 0)
EOF
[ "$HPDET" = 1 ] && cat <<'EOF'
  - Detección (variante hpdet): evtest   -> elegir "headphone-detect", enchufar y desenchufar;
    tiene que aparecer EV_SW SW_HEADPHONE_INSERT 1/0.
EOF
echo "Volver atrás: sudo ./rollback-dtb-on-device.sh $bk"
