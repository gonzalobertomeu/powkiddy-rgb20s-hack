#!/usr/bin/env bash
# Descarga y verifica todas las fuentes de la imagen (corre en el host, sin sudo).
# Uso: fetch.sh [linux|uboot|rkbin|joypad|alarm|all]   (por defecto: all)
# Todo va a infra/downloads/<componente>/ (ignorado por git). Las descargas son datos
# no confiables: nada se ejecuta acá, solo se verifica.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
. "$here/../sources.lock"
dl="$repo/infra/downloads"
what="${1:-all}"

log() { printf '\033[1;34m[fetch]\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m[fetch] ERROR:\033[0m %s\n' "$*" >&2; exit 1; }

# get <url> <destino>: descarga a un .part y renombra al terminar (reanuda si se cortó)
get() {
  local url="$1" out="$2"
  [ -f "$out" ] && return 0
  mkdir -p "$(dirname "$out")"
  curl -fL --retry 3 -C - -o "$out.part" "$url"
  mv "$out.part" "$out"
}

check_sha256() {
  local file="$1" want="$2"
  echo "$want  $file" | sha256sum -c --quiet - || die "sha256 no coincide: $file"
}

git_blob_sha1() {  # sha1 de objeto git, igual a `git hash-object <archivo>`
  { printf 'blob %d\0' "$(stat -c %s "$1")"; cat "$1"; } | sha1sum | cut -d' ' -f1
}

fetch_linux() {
  local f="$dl/linux/linux-$LINUX_VERSION.tar.xz"
  log "kernel $LINUX_VERSION"
  get "$LINUX_URL" "$f"
  check_sha256 "$f" "$LINUX_SHA256"
}

fetch_uboot() {
  local f="$dl/uboot/u-boot-$UBOOT_VERSION.tar.gz"
  log "U-Boot $UBOOT_VERSION"
  get "$UBOOT_URL" "$f"
  check_sha256 "$f" "$UBOOT_SHA256"
}

fetch_rkbin() {
  log "rkbin $RKBIN_COMMIT"
  local path sha out
  while read -r path sha; do
    [ -n "$path" ] || continue
    out="$dl/rkbin/$RKBIN_COMMIT/$path"
    get "$RKBIN_BASE/$RKBIN_COMMIT/$path" "$out"
    [ "$(git_blob_sha1 "$out")" = "$sha" ] || { rm -f "$out"; die "hash git no coincide: $path"; }
  done <<< "$RKBIN_FILES"
  ( cd "$dl/rkbin/$RKBIN_COMMIT" && find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS )
}

fetch_joypad() {
  local f="$dl/joypad/rocknix-joypad-$JOYPAD_COMMIT.tar.gz"
  log "rocknix-joypad $JOYPAD_COMMIT"
  get "$JOYPAD_URL" "$f"
  check_sha256 "$f" "$JOYPAD_SHA256"
}

fetch_alarm() {
  local d="$dl/alarm" f="$dl/alarm/ArchLinuxARM-aarch64-latest.tar.gz"
  log "Arch Linux ARM aarch64 (latest)"
  get "$ALARM_URL" "$f"
  get "$ALARM_URL.md5" "$f.md5"
  get "$ALARM_URL.sig" "$f.sig"
  ( cd "$d" && md5sum -c --quiet ArchLinuxARM-aarch64-latest.tar.gz.md5 ) || die "md5 no coincide (¿cambió el latest a mitad de la descarga? borrar $d y repetir)"
  # Llavero propio, aislado del del usuario.
  export GNUPGHOME="$dl/gnupg"
  mkdir -p "$GNUPGHOME"; chmod 700 "$GNUPGHOME"
  if ! gpg --batch --list-keys "$ALARM_KEY_FPR" >/dev/null 2>&1; then
    gpg --batch --keyserver hkps://keyserver.ubuntu.com --recv-keys "$ALARM_KEY_FPR"
  fi
  gpg --batch --status-fd 1 --verify "$f.sig" "$f" 2>/dev/null | grep -q "VALIDSIG $ALARM_KEY_FPR" \
    || die "firma GPG inválida o de otra clave: $f"
  sha256sum "$f" | sed "s|$d/||" > "$d/USED.sha256"
  log "ALARM verificado (md5 + firma de $ALARM_KEY_FPR); sha256 en $d/USED.sha256"
}

case "$what" in
  linux|uboot|rkbin|joypad|alarm) "fetch_$what" ;;
  all) fetch_linux; fetch_uboot; fetch_rkbin; fetch_joypad; fetch_alarm ;;
  *) die "componente desconocido: $what" ;;
esac
log "listo: $what"
