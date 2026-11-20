#!/usr/bin/env bash
# Deploy por red (a futuro: hoy la consola no tiene red con la Mac, ver infra/README.md).
# Uso: deploy-ssh.sh <host-ssh> <dir-remoto> <binario> [assets] [crate]
# Con <crate>, copia también sus licencias de terceros junto al binario (ver list-licenses.sh).
set -euo pipefail
host="$1"; remote="$2"; bin="$3"; assets="${4:-}"; crate="${5:-}"
ssh -o ConnectTimeout=5 "$host" "mkdir -p '$remote'"
licenses=()
if [ -n "$crate" ]; then
  lic_list="$("$(dirname "$0")/list-licenses.sh" "$crate")"
  [ -z "$lic_list" ] || mapfile -t licenses <<< "$lic_list"
fi
rsync -avz --progress "$bin" "${licenses[@]}" "$host:$remote/"
if [ -n "$assets" ] && [ -d "$assets" ]; then
  rsync -avz --delete --progress "$assets/" "$host:$remote/assets/"
fi
