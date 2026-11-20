#!/usr/bin/env bash
# Corre un comando dentro del contenedor rgb20s-image-builder con el repo montado en /work.
# Uso: in-builder.sh [--root] <comando> [args...]
#   --root: corre como root DENTRO del contenedor (hace falta para conservar dueños y
#           permisos de la rootfs al armar el ext4). No toca el host: no es --privileged,
#           no monta /dev del host y no usa loop devices. Al terminar, los archivos
#           generados se devuelven al usuario del host (chown).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
image="rgb20s-image-builder:dev"

as_root=0
if [ "${1:-}" = "--root" ]; then as_root=1; shift; fi

docker image inspect "$image" >/dev/null 2>&1 || {
  echo "falta la imagen $image: correr 'just image-builder'" >&2; exit 2; }

tty_flag=()
[ -t 0 ] && [ -t 1 ] && tty_flag=(-t)

user_args=(--user "$(id -u):$(id -g)")
[ "$as_root" = 1 ] && user_args=(-e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)")

exec docker run --rm -i "${tty_flag[@]}" "${user_args[@]}" \
  -e HOME=/tmp -e JOBS="${JOBS:-$(nproc)}" \
  -v "$repo:/work" -w /work \
  "$image" "$@"
