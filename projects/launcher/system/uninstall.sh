#!/usr/bin/env bash
# Revierte install.sh: saca la sesión del launcher y vuelve el login de texto en tty1.
# Correr EN la consola: sudo ./uninstall.sh
set -euo pipefail
[ "$(id -u)" -eq 0 ] || { echo "correr con sudo" >&2; exit 1; }

systemctl disable --now rgb20s-session.service 2>/dev/null || true
rm -f /etc/systemd/system/rgb20s-session.service /usr/local/bin/rgb20s-session \
  /etc/sudoers.d/20-rgb20s-power
systemctl daemon-reload
systemctl reset-failed rgb20s-session.service 2>/dev/null || true
systemctl enable getty@tty1.service
systemctl start getty@tty1.service
echo "listo: vuelve el login de texto en tty1"
