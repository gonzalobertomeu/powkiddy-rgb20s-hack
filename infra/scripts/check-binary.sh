#!/usr/bin/env bash
# Revisa un binario aarch64: arquitectura, glibc máxima requerida y librerías dinámicas.
# Uso: check-binary.sh <binario> [glibc-máxima-de-la-consola]
# Corre las herramientas dentro de rgb20s-cross (el host no tiene binutils para aarch64).
set -euo pipefail
bin="$1"; max="${2:-}"
repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
bin_abs="$(cd "$(dirname "$bin")" && pwd)/$(basename "$bin")"
rel="${bin_abs#"$repo_root"/}"

docker run --rm --platform linux/amd64 -v "$repo_root:/work:ro" -w /work rgb20s-cross:dev bash -c '
  set -euo pipefail
  b="$1"; max="$2"
  file -b "$b"
  need=$(aarch64-linux-gnu-objdump -T "$b" | grep -o "GLIBC_[0-9.]*" | sed "s/GLIBC_//" | sort -Vu | tail -1)
  echo "glibc máxima requerida: $need"
  echo "librerías dinámicas:"; aarch64-linux-gnu-readelf -d "$b" | grep NEEDED | sed "s/.*\[/  /; s/\]//"
  if [ -n "$max" ]; then
    if [ "$(printf "%s\n%s\n" "$need" "$max" | sort -V | tail -1)" = "$max" ]; then
      echo "OK: $need <= $max (glibc de la consola)"
    else
      echo "ERROR: el binario pide glibc $need y la consola tiene $max" >&2; exit 1
    fi
  fi
' _ "$rel" "$max"
