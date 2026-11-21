#!/usr/bin/env bash
# Instala el servicio rgb20s-audio en la consola.
# Correr EN la consola, desde esta carpeta: sudo ./install.sh [ruta-del-binario]
# Por defecto el binario es el de `APP=rgb20s-audio just deploy`: /home/alarm/rgb20s-audio/rgb20s-audio
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
bin="${1:-/home/alarm/rgb20s-audio/rgb20s-audio}"
alsa_conf=/usr/share/alsa/alsa.conf

[ "$(id -u)" -eq 0 ] || { echo "correr con sudo" >&2; exit 1; }
[ -x "$bin" ] || { echo "no existe $bin (APP=rgb20s-audio just deploy)" >&2; exit 1; }

# La mezcla mono depende de que alsa-lib cargue /var/lib/alsa/conf.d (alsa-lib >= 1.2.5).
if ! grep -q '/var/lib/alsa/conf.d' "$alsa_conf"; then
  echo "$alsa_conf no carga /var/lib/alsa/conf.d: el modo mono no tendría efecto" >&2
  exit 1
fi
# /etc/asound.conf se carga después y le ganaría al default del servicio.
if [ -e /etc/asound.conf ] && grep -q 'pcm\.!\?default' /etc/asound.conf; then
  echo "AVISO: /etc/asound.conf define pcm.default y pisa el del servicio"
fi

install -m 0755 "$bin" /usr/local/bin/rgb20s-audio
install -m 0644 "$here/rgb20s-audio.service" /etc/systemd/system/rgb20s-audio.service
if [ ! -e /etc/rgb20s-audio.toml ]; then
  install -m 0644 "$here/rgb20s-audio.toml" /etc/rgb20s-audio.toml
fi
install -d -m 0755 /var/lib/alsa /var/lib/alsa/conf.d

systemctl daemon-reload
systemctl enable rgb20s-audio.service
systemctl restart rgb20s-audio.service
sleep 1
systemctl --no-pager --lines=5 status rgb20s-audio.service || true
# Estado que leen las apps (lo crea el servicio al arrancar).
if [ -r /run/rgb20s-audio/state ]; then
  echo "estado en /run/rgb20s-audio/state:"; sed 's/^/  /' /run/rgb20s-audio/state
else
  echo "AVISO: no está /run/rgb20s-audio/state: las apps no van a mostrar el volumen"
fi
echo "listo (logs: journalctl -u rgb20s-audio -f). Las apps abiertas toman el modo al reabrirse."
