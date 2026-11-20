#!/usr/bin/env bash
# Verificación completa de un crate con salida corta: una línea por paso y, si falla, solo los errores.
# Uso: check.sh <crate> <binario-arm> <glibc-máxima>   (lo llama `just check`)
# Pasos: fmt, clippy (-D warnings), tests, build aarch64 y revisión del binario (glibc y arquitectura).
set -uo pipefail
crate="$1"; arm_bin="$2"; glibc_max="$3"
scripts="$(cd "$(dirname "$0")" && pwd)"
log="$(mktemp)"; trap 'rm -f "$log"' EXIT
failed=0

# step <nombre> <comando...>: corre el comando con la salida en $log; imprime OK o FALLA y los errores.
step() {
  local name="$1"; shift
  local t0=$SECONDS
  if "$@" >"$log" 2>&1; then
    printf 'OK    %-7s %4ss  %s\n' "$name" $((SECONDS - t0)) "$(info "$name")"
  else
    printf 'FALLA %-7s %4ss\n' "$name" $((SECONDS - t0))
    if grep -qE '^(error|warning|test .* FAILED|---- )' "$log"; then
      grep -E -A8 '^(error|warning|---- )' "$log" | head -80
      grep -E '^test .* FAILED|^test result' "$log" | head -20
    else
      tail -40 "$log"
    fi
    failed=1
  fi
}

# Una línea de información por paso, sacada del log.
info() {
  case "$1" in
    test) awk '/^test result/ {p+=$4; f+=$6} END {printf "%d tests pasan", p}' "$log" ;;
    binary) grep -E '^OK' "$log" | head -1 ;;
  esac
}

cd "$crate" || exit 1
step fmt    cargo fmt --check
step clippy cargo clippy --all-targets --quiet -- -D warnings
step test   cargo test --quiet
step build  "$scripts/docker-cargo.sh" cross "$crate" cargo build --release --quiet
step binary "$scripts/check-binary.sh" "$arm_bin" "$glibc_max"

if [ $failed -eq 0 ]; then echo "check: todo OK"; else echo "check: hubo fallas"; exit 1; fi
