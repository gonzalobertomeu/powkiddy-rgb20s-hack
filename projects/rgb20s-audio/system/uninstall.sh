#!/usr/bin/env bash
# Revierte install.sh: saca el servicio y la configuración de ALSA que genera.
# Correr EN la consola: sudo ./uninstall.sh   (--purge borra también /etc/rgb20s-audio.toml)
# El volumen guardado (/var/lib/alsa/asound.state) queda: es de alsa-restore.service.
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "correr con sudo" >&2; exit 1; }

systemctl disable --now rgb20s-audio.service 2>/dev/null || true
rm -f /etc/systemd/system/rgb20s-audio.service /usr/local/bin/rgb20s-audio \
  /var/lib/alsa/conf.d/60-rgb20s-audio.conf
rm -rf /var/lib/rgb20s-audio
[ "${1:-}" != "--purge" ] || rm -f /etc/rgb20s-audio.toml
systemctl daemon-reload
systemctl reset-failed rgb20s-audio.service 2>/dev/null || true
echo "listo: ALSA vuelve a su default de fábrica (estéreo; el parlante solo da el canal izquierdo)"
