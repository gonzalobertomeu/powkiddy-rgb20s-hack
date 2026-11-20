#!/usr/bin/env bash
# Corre un binario aarch64 emulado (nivel 2 de emulación) dentro de rgb20s-cross:
# el contenedor tiene la glibc, SDL2 y ALSA arm64 de trixie, y el binfmt del host
# (qemu-aarch64-static con flag F) ejecuta el binario sin pasos extra.
# Uso: run-arm.sh <binario> [args...]
# Sin pantalla ni audio: SDL usa el driver "offscreen" y ALSA no tiene placas.
#
# Entorno:
#   GROOVEBOX_*, RGB20S_*  se reenvían todas al contenedor. Si el valor es una ruta que existe
#                          en el host, se traduce a la ruta equivalente dentro del contenedor
#                          (repo -> /work, RUN_ARM_OUT -> /out; fuera de esos, se monta en
#                          modo solo lectura en /ext/N/). Las relativas se resuelven desde RUN_ARM_CWD.
#   RUN_ARM_OUT            directorio de trabajo de la app dentro del contenedor (/out): ahí caen
#                          los archivos que genere (p. ej. las capturas del modo guion).
#                          Defecto: <repo>/infra/build/run-arm (en .gitignore). Se crea si no existe.
#   RUN_ARM_CWD            desde dónde resolver rutas relativas (just pasa invocation_directory()).
#   SDL_VIDEODRIVER, SDL_AUDIODRIVER  defecto offscreen y dummy.
# Los argumentos [args...] se pasan tal cual, sin traducir rutas.
set -euo pipefail
bin="$1"; shift
repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
cwd="${RUN_ARM_CWD:-$PWD}"

abs() { case "$1" in /*) echo "$1" ;; *) echo "$cwd/$1" ;; esac; }

bin_abs="$(realpath -e "$(abs "$bin")")"
case "$bin_abs" in "$repo_root"/*) ;; *) echo "run-arm: $bin_abs está fuera del repo" >&2; exit 1 ;; esac
rel="${bin_abs#"$repo_root"/}"

out="$(abs "${RUN_ARM_OUT:-$repo_root/infra/build/run-arm}")"
mkdir -p "$out"
out="$(realpath -e "$out")"

args=(-v "$repo_root:/work" -v "$out:/out" -w /out)
n=0
# Traduce una ruta del host a la del contenedor y deja el resultado en $REPLY (el valor sin
# cambios si no es una ruta existente). No usa $(...) para poder agregar montajes a args.
to_container() {
  local v="$1" p
  REPLY="$v"
  # Vacíos y valores numéricos (GROOVEBOX_FONT=12, GROOVEBOX_REPEAT=250,33) nunca son rutas.
  [[ -n "$v" && ! "$v" =~ ^[0-9.,]+$ ]] || return 0
  p="$(abs "$v")"
  [ -e "$p" ] || return 0
  p="$(realpath -e "$p")"
  case "$p" in
    "$out") REPLY=/out ;;
    "$out"/*) REPLY="/out/${p#"$out"/}" ;;
    "$repo_root") REPLY=/work ;;
    "$repo_root"/*) REPLY="/work/${p#"$repo_root"/}" ;;
    *) n=$((n + 1)); REPLY="/ext/$n/$(basename "$p")"; args+=(-v "$p:$REPLY:ro") ;;
  esac
}

while IFS='=' read -r name _; do
  case "$name" in
    GROOVEBOX_*|RGB20S_*)
      to_container "${!name}"
      args+=(-e "$name=$REPLY")
      ;;
  esac
done < <(env | grep -E '^(GROOVEBOX|RGB20S)_[A-Za-z0-9_]*=' || true)

echo "run-arm: salida en $out" >&2
exec docker run --rm -i --platform linux/amd64 --user "$(id -u):$(id -g)" \
  -e SDL_VIDEODRIVER="${SDL_VIDEODRIVER:-offscreen}" -e HOME=/tmp -e SDL_AUDIODRIVER="${SDL_AUDIODRIVER:-dummy}" \
  "${args[@]}" rgb20s-cross:dev "/work/$rel" "$@"
