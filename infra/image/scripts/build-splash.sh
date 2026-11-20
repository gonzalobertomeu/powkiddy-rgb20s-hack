#!/usr/bin/env bash
# Splash de arranque: genera el arte (make-splash.py) y compila rgb20s-splash (C, estático, aarch64).
# Corre DENTRO de rgb20s-image-builder (in-builder.sh). Ver projects/linux-image/splash/README.md.
# Salida: infra/build/out/splash/{rgb20s-splash,splash.ppm}
set -euo pipefail
cd /work
sp=projects/linux-image/splash
out=infra/build/out/splash
log() { printf '\033[1;32m[splash]\033[0m %s\n' "$*"; }

log "arte (make-splash.py)"
python3 -I "$sp/make-splash.py"

log "compilando rgb20s-splash"
mkdir -p "$out"
aarch64-linux-gnu-gcc -std=c11 -O2 -Wall -Wextra -Werror -static -s \
  -o "$out/rgb20s-splash" "$sp/rgb20s-splash.c"
file "$out/rgb20s-splash" | grep -q 'ARM aarch64.*statically linked' \
  || { echo "rgb20s-splash no es un binario aarch64 estático" >&2; file "$out/rgb20s-splash"; exit 1; }
cp "$sp/generated/splash.ppm" "$out/splash.ppm"
log "listo"
ls -la "$out"
