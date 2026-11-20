#!/usr/bin/env bash
# Deploy por red (SSH sobre el gadget USB de la consola, ver infra/README.md).
# Uso: deploy-ssh.sh <host-ssh> <dir-remoto> <binario> [assets] [crate]
# Con <crate>, copia también sus licencias de terceros junto al binario (ver list-licenses.sh).
# Usa rsync si la consola lo tiene; si no (la imagen de Arch no lo trae), scp.
set -euo pipefail
host="$1"; remote="$2"; bin="$3"; assets="${4:-}"; crate="${5:-}"
ssh -o ConnectTimeout=5 "$host" "mkdir -p '$remote'"
licenses=()
if [ -n "$crate" ]; then
  lic_list="$("$(dirname "$0")/list-licenses.sh" "$crate")"
  [ -z "$lic_list" ] || mapfile -t licenses <<< "$lic_list"
fi
if ssh "$host" 'command -v rsync >/dev/null'; then
  rsync -avz --progress "$bin" "${licenses[@]}" "$host:$remote/"
  if [ -n "$assets" ] && [ -d "$assets" ]; then
    rsync -avz --delete --progress "$assets/" "$host:$remote/assets/"
  fi
else
  echo "deploy: la consola no tiene rsync, copio con scp"
  scp "$bin" "${licenses[@]}" "$host:$remote/"
  if [ -n "$assets" ] && [ -d "$assets" ]; then
    # Equivalente a --delete: se reemplaza la carpeta entera
    ssh "$host" "rm -rf '$remote/assets'"
    scp -r "$assets" "$host:$remote/assets"
  fi
fi
echo "deploy: listo en $host:$remote"
