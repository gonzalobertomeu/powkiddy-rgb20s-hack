#!/usr/bin/env bash
# Corre cargo dentro de un contenedor de build, montando el repo en /work.
# Uso: docker-cargo.sh <cross|arm64> <directorio-del-crate> <args de cargo...>
#   cross -> imagen rgb20s-cross (x86_64 + gcc cruzado, rápida; opción elegida en el ADR 0008)
#   arm64 -> imagen rgb20s-arm64 (todo emulado con qemu-user, lenta; referencia)
set -euo pipefail

kind="$1"; crate_dir="$2"; shift 2
repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
crate_dir="$(cd "$crate_dir" && pwd)"

case "$kind" in
  cross) image="rgb20s-cross:dev" ;;
  arm64) image="rgb20s-arm64:dev" ;;
  *) echo "tipo de contenedor desconocido: $kind (cross|arm64)" >&2; exit 2 ;;
esac

case "$crate_dir" in
  "$repo_root"/*) rel="${crate_dir#"$repo_root"/}" ;;
  *) echo "el crate tiene que estar dentro del repo ($repo_root)" >&2; exit 2 ;;
esac

if [ ! -f "$crate_dir/Cargo.toml" ]; then
  echo "no hay Cargo.toml en $crate_dir" >&2; exit 2
fi

if ! docker image inspect "$image" >/dev/null 2>&1; then
  echo "falta la imagen $image: correr 'just images'" >&2; exit 2
fi

# Cache de crates compartida con el host (mismo formato de cargo): evita re-descargar.
host_cargo="${CARGO_HOME:-$HOME/.cargo}"
mkdir -p "$host_cargo/registry" "$host_cargo/git"

# No se pasa --platform: con el almacén de containerd de Docker 29, una imagen local
# arm64 no se encuentra con --platform linux/arm64 (busca la variante v8). La imagen
# ya es de una sola arquitectura; Docker solo muestra un aviso de plataforma.
# target/ separado por tipo de contenedor, para no mezclar artefactos con el build nativo.
tty_flag=()
[ -t 0 ] && [ -t 1 ] && tty_flag=(-t)

exec docker run --rm -i "${tty_flag[@]}" \
  --user "$(id -u):$(id -g)" \
  -e HOME=/tmp \
  -e CARGO_TARGET_DIR="/work/$rel/target/$kind" \
  -e CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-auto}" \
  -v "$repo_root:/work" \
  -v "$host_cargo/registry:/opt/cargo/registry" \
  -v "$host_cargo/git:/opt/cargo/git" \
  -w "/work/$rel" \
  "$image" "$@"
