#!/bin/sh
# Apunta apt a snapshot.debian.org en una fecha fija, para que las versiones de los
# paquetes sean reproducibles. Uso: apt-snapshot.sh <AAAAMMDDTHHMMSSZ>
set -eu
snap="$1"
rm -f /etc/apt/sources.list.d/debian.sources
cat > /etc/apt/sources.list.d/snapshot.sources <<SRC
Types: deb
URIs: http://snapshot.debian.org/archive/debian/${snap}
Suites: trixie trixie-updates
Components: main
Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg

Types: deb
URIs: http://snapshot.debian.org/archive/debian-security/${snap}
Suites: trixie-security
Components: main
Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg
SRC
# snapshot.debian.org sirve archivos Release viejos: no rechazarlos por fecha.
echo 'Acquire::Check-Valid-Until "false";' > /etc/apt/apt.conf.d/99snapshot
echo 'Acquire::Retries "5";' >> /etc/apt/apt.conf.d/99snapshot
