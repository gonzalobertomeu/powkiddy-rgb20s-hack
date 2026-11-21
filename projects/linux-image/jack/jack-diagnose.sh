#!/usr/bin/env bash
# Diagnóstico de la detección del jack de auriculares de la RGB20S, SIN recompilar ni reiniciar.
# Se corre EN LA CONSOLA, como root, con el usuario al lado enchufando y desenchufando:
#     sudo ./jack-diagnose.sh            # todo: estado del kernel + barrido de los 128 GPIO + prueba de pull
#     sudo ./jack-diagnose.sh --no-pull  # sin cambiar el pull de GPIO2_C6
# Usa rgb20s-gpio-probe (en la misma carpeta) si está; sin él, solo hace la parte de debugfs.
# Salida: en pantalla y en ./jack-diagnose-<fecha>.log. No toca el DTB, el mezclador ni el arranque.
# Lo único que modifica (salvo --no-pull) es el pull de GPIO2_C6, y lo deja como estaba al terminar
# (de todos modos vuelve solo al reiniciar).
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
probe="$here/rgb20s-gpio-probe"
PIN=gpio2-c6        # el que declara el DTS de ROCKNIX (hp-det-gpios)
DO_PULL=1
[ "${1:-}" = "--no-pull" ] && DO_PULL=0

[ "$(id -u)" -eq 0 ] || { echo "correr con sudo (debugfs y /dev/mem son solo de root)" >&2; exit 1; }
log="$here/jack-diagnose-$(date +%Y%m%d-%H%M%S).log"
exec > >(tee -a "$log") 2>&1

say() { printf '\n\033[1;32m== %s\033[0m\n' "$*"; }
ask() { printf '\n\033[1;33m>>> %s  [Enter]\033[0m ' "$*"; read -r _ < /dev/tty; }

mountpoint -q /sys/kernel/debug || mount -t debugfs debugfs /sys/kernel/debug
pctl="$(ls -d /sys/kernel/debug/pinctrl/*pinctrl* 2>/dev/null | grep -v -- '-pinctrl-maps' | head -n1)"

snapshot() {   # $1 = etiqueta
  echo "-- [$1] gpio del kernel (línea pedida por simple-audio-card):"
  grep -i -B0 'Headphone' /sys/kernel/debug/gpio || echo "   (no hay línea 'Headphone detection')"
  echo "-- [$1] interrupción del jack (cuenta por CPU):"
  grep -i 'Headphone' /proc/interrupts || echo "   (no aparece en /proc/interrupts)"
  echo "-- [$1] control ALSA:"
  amixer -c 0 cget iface=CARD,name='Headphones Jack' 2>/dev/null | grep ': values' || echo "   (sin control)"
  if [ -n "$pctl" ]; then
    echo "-- [$1] pinmux / pinconf de GPIO2_C6 (pin 86, gpio2-22):"
    grep -E 'gpio2-22\)' "$pctl/pinmux-pins" "$pctl/pinconf-pins" 2>/dev/null | sed 's|.*/||'
  fi
  echo "-- [$1] SARADC (por si la detección fuera analógica):"
  for f in /sys/bus/iio/devices/iio:device*/in_voltage*_raw; do
    [ -r "$f" ] && printf '   %s = %s\n' "${f#/sys/bus/iio/devices/}" "$(cat "$f")"
  done
  if [ -x "$probe" ]; then
    echo "-- [$1] $("$probe" get "$PIN")"
    "$probe" dump | grep '^gpio' | sed "s/^/   [$1] /"
  fi
}

say "Sistema"
uname -r; tr '\0' ' ' < /proc/device-tree/model; echo
ls /proc/device-tree/rk817-sound/ 2>/dev/null | grep -E 'hp-det|pinctrl' | sed 's/^/   rk817-sound: /'
[ -x "$probe" ] || echo "AVISO: no está $probe; solo se hace la parte de debugfs (sin barrido ni pull)"

ask "1/3: DESENCHUFÁ los auriculares (y no toques ningún botón)"
snapshot sin-auriculares
ask "2/3: ENCHUFÁ los auriculares a fondo"
snapshot con-auriculares

if [ -x "$probe" ]; then
  say "Barrido: 30 s mirando TODOS los pines de entrada"
  echo "Durante estos 30 s: enchufá y desenchufá 3 veces, despacio (~3 s cada estado). Sin tocar botones."
  ask "listo para empezar"
  "$probe" watch 30
  echo "(Los botones del joypad están en GPIO1_A2/A5/A6/A7, GPIO1_B4-B7, GPIO2_A0-A4/A6/A7 y GPIO3_B1/B2/B4/B7;"
  echo " si aparecen sin tocar nada, es ruido. Un pin que cambie con cada enchufe es el candidato.)"

  if [ "$DO_PULL" = 1 ]; then
    orig="$("$probe" get "$PIN" | sed -E 's/.*pull=([a-z-]+).*/\1/')"
    case "$orig" in pull-up) orig=up ;; pull-down) orig=down ;; *) orig=none ;; esac
    say "Prueba de pull en GPIO2_C6 (pull original: $orig)"
    for mode in down up; do
      "$probe" pull "$PIN" "$mode"
      ask "pull=$mode: DESENCHUFÁ los auriculares"
      "$probe" get "$PIN"; grep -i 'Headphone' /proc/interrupts
      ask "pull=$mode: ENCHUFÁ los auriculares"
      "$probe" get "$PIN"; grep -i 'Headphone' /proc/interrupts
    done
    "$probe" pull "$PIN" "$orig"
    amixer -c 0 cget iface=CARD,name='Headphones Jack' 2>/dev/null | grep ': values'
    echo "pull restaurado a '$orig'. Si 'Headphones Jack' quedó en off y no se oye nada, reiniciar"
    echo "(la detección del kernel apaga la salida HP, que también alimenta el parlante)."
  fi
fi

say "Listo. Log: $log"
cat <<'EOF'
Cómo leerlo:
- GPIO2_C6 cambia entre sin/con auriculares (con el pull original o con alguno de la prueba):
  el pin es ese; anotar con qué pull y qué nivel corresponde a "enchufado".
- GPIO2_C6 no cambia con ningún pull y el barrido no muestra otro pin: el jack no tiene línea
  de detección hacia el SoC (el contacto del jack solo corta el parlante).
- Otro pin cambia en el barrido: ese es el candidato (anotar nombre y niveles).
- Un canal de SARADC que cambia mucho: detección analógica (no la usa ningún DTS conocido).
EOF
