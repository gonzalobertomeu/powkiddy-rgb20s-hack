#!/usr/bin/env bash
# Arma la rootfs: importa el tarball verificado de Arch Linux ARM como imagen Docker arm64,
# la personaliza con projects/linux-image/rootfs/customize.sh (emulada con qemu-aarch64-static
# vía binfmt del host) y exporta el resultado a un tar.
# Corre en el HOST (usa docker). Sin sudo: los dueños y permisos los maneja Docker.
# Requiere: fetch.sh alarm. Los módulos del kernel se agregan después, en build-image.sh,
# para que recompilar el kernel no obligue a repetir este paso (emulado y lento).
# Salida: infra/build/out/rootfs/rootfs.tar, packages.txt, size.txt
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
log() { printf '\033[1;32m[rootfs]\033[0m %s\n' "$*"; }

tarball="$repo/infra/downloads/alarm/ArchLinuxARM-aarch64-latest.tar.gz"
[ -f "$repo/infra/downloads/alarm/USED.sha256" ] || { echo "falta correr fetch.sh alarm" >&2; exit 2; }
sha="$(cut -c1-12 "$repo/infra/downloads/alarm/USED.sha256")"
base="rgb20s-alarm-base:$sha"
out="$repo/infra/build/out/rootfs"
keys="$repo/infra/image/local"
ctr="rgb20s-rootfs-build-$$"

if ! docker image inspect "$base" >/dev/null 2>&1; then
  log "importando $tarball como $base"
  docker import --platform linux/arm64 "$tarball" "$base"
fi

rm -rf "$out"; mkdir -p "$out" "$keys"
cleanup() { docker rm -f "$ctr" >/dev/null 2>&1 || true; }
trap cleanup EXIT

log "personalizando (emulado: tarda)"
# Sin --platform: con el almacén de containerd de Docker 29 no encuentra la imagen local
# (ver infra/scripts/docker-cargo.sh). Docker solo avisa que la plataforma no coincide.
docker run --name "$ctr" -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  -v "$repo/projects/linux-image/rootfs:/setup:ro" \
  -v "$keys:/keys:ro" \
  -v "$out:/out" \
  "$base" /bin/bash /setup/customize.sh

log "exportando"
docker export -o "$out/rootfs.tar" "$ctr"
ls -la "$out"
