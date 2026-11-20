#!/bin/bash
# Copia el diagnóstico del arranque a la partición FAT (/boot/logs/), para leerlo desde la Mac
# sin consola serie ni red. Uso: bootlog.sh early|boot|shutdown
#   early     apenas se monta /boot: deja constancia de que el kernel llegó a userspace
#   boot      al terminar el arranque (multi-user.target): journal, dmesg, USB, pantalla, audio...
#   shutdown  al apagar: journal completo de la sesión
# Se guardan las últimas 10 sesiones: /boot/logs/NNN-<boot_id>/
set -u
. /usr/local/lib/rgb20s/common.sh

[ "$(conf_get BOOT_LOGS yes)" = yes ] || exit 0
mountpoint -q /boot || { log "bootlog: /boot no está montado"; exit 0; }

base=/boot/logs
bid="$(cut -c1-8 /proc/sys/kernel/random/boot_id)"
mkdir -p "$base" || exit 0
dir="$(ls -d "$base"/*-"$bid" 2>/dev/null | head -n1)"
if [ -z "$dir" ]; then
  n=$(( $(cat "$base/.counter" 2>/dev/null || echo 0) + 1 ))
  echo "$n" > "$base/.counter"
  dir="$base/$(printf '%03d' "$n")-$bid"
  mkdir -p "$dir"
  # rotación: quedan las 10 más nuevas
  ls -d "$base"/[0-9][0-9][0-9]-* 2>/dev/null | sort | head -n -10 | xargs -r rm -rf
fi

run() {  # run <archivo> <comando...>: guarda la salida, con el comando como encabezado
  local f="$dir/$1"; shift
  { echo "----- \$ $*"; "$@" 2>&1; echo "[salida: $?]"; echo; } >> "$f"
}

case "${1:-boot}" in
  early)
    { echo "llegó a userspace: $(date -Is) (uptime $(cut -d' ' -f1 /proc/uptime) s)"
      echo "kernel: $(uname -a)"
      echo "cmdline: $(cat /proc/cmdline)"; } > "$dir/00-early.txt"
    dmesg > "$dir/01-dmesg-early.txt" 2>&1
    ;;
  boot)
    f=10-resumen.txt
    run $f uptime
    run $f systemctl is-system-running
    run $f systemctl --failed --no-legend
    run $f systemd-analyze time
    run $f sh -c 'systemd-analyze blame | head -n 25'
    run $f cat /proc/device-tree/model
    run $f ip -br addr
    run $f networkctl list
    f=20-usb.txt
    run $f sh -c 'for r in /sys/class/usb_role/*/role; do echo "$r: $(cat $r)"; done'
    run $f ls -l /sys/class/udc
    run $f sh -c 'for u in /sys/class/udc/*; do echo "$u: $(cat $u/state)"; done'
    run $f sh -c 'ls -R /sys/kernel/config/usb_gadget 2>/dev/null | head -n 60'
    run $f lsusb -t
    run $f sh -c 'dmesg | grep -iE "dwc2|usb|udc|gadget|phy"'
    f=30-hardware.txt
    run $f ls -l /sys/class/drm
    run $f sh -c 'for c in /sys/class/drm/card*-*; do echo "$c: $(cat $c/status) $(cat $c/modes 2>/dev/null | tr "\n" " ")"; done'
    run $f ls -l /dev/dri
    run $f cat /proc/fb
    run $f cat /proc/bus/input/devices
    run $f cat /proc/asound/cards
    run $f aplay -l
    run $f sh -c 'for p in /sys/class/power_supply/*; do echo "== $p"; cat $p/uevent; done'
    run $f lsmod
    run $f lsblk -o NAME,SIZE,FSTYPE,LABEL,PARTUUID,MOUNTPOINTS
    run $f df -h
    run $f free -m
    run $f cat /proc/cpuinfo
    dmesg > "$dir/40-dmesg.txt" 2>&1
    journalctl -b -o short-monotonic --no-pager > "$dir/50-journal.txt" 2>&1
    ;;
  shutdown)
    journalctl -b -o short-monotonic --no-pager > "$dir/90-journal-final.txt" 2>&1
    ;;
  *) echo "uso: $0 early|boot|shutdown" >&2; exit 2 ;;
esac
sync -f "$dir" 2>/dev/null || sync
exit 0
