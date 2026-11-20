#!/usr/bin/env bash
# Instala la sesión del launcher en la consola: reemplaza el login de tty1.
# Correr EN la consola, desde esta carpeta: sudo ./install.sh
# Para volver al login de texto: sudo ./uninstall.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
launcher=/home/alarm/launcher/launcher
sudoers=/etc/sudoers.d/20-rgb20s-power

[ "$(id -u)" -eq 0 ] || { echo "correr con sudo" >&2; exit 1; }
id alarm >/dev/null 2>&1 || { echo "no existe el usuario alarm" >&2; exit 1; }
if [ ! -x "$launcher" ]; then
  echo "AVISO: no existe $launcher (APP=launcher just deploy). La sesión va a reintentar cada 15 s."
fi

install -m 0755 "$here/rgb20s-session" /usr/local/bin/rgb20s-session
install -m 0644 "$here/rgb20s-session.service" /etc/systemd/system/rgb20s-session.service

# Sin polkit, logind no deja que alarm apague o reinicie: se permite solo eso por sudo.
rule="$(mktemp)"
trap 'rm -f "$rule"' EXIT
echo 'alarm ALL=(root) NOPASSWD: /usr/bin/systemctl poweroff, /usr/bin/systemctl reboot' >"$rule"
visudo -cqf "$rule"
install -m 0440 "$rule" "$sudoers"

systemctl daemon-reload
systemctl disable getty@tty1.service
systemctl enable --now rgb20s-session.service
echo "listo: rgb20s-session activo en tty1 (logs: journalctl -u rgb20s-session -f)"
