#!/usr/bin/env bash
# Copia una app a la microSD de la consola montada en la Mac (deploy sin red).
# Uso: deploy-sd.sh <punto-de-montaje> <app> <binario> [directorio-de-assets] [directorio-del-crate]
#
# Estructura que deja en la SD (convención de ports de ArkOS/dArkOS, sin verificar en la RGB20S):
#   <montaje>/ports/<App>.sh        lanzador que ve EmulationStation
#   <montaje>/ports/<app>/<bin>     binario aarch64
#   <montaje>/ports/<app>/assets/   assets (opcional)
#   <montaje>/ports/<app>/LICENSE-*  licencias de terceros que declara el crate (list-licenses.sh)
#   <montaje>/ports/<app>/last-run.log   lo escribe la consola al correr la app
#
# No formatea ni escribe fuera de <montaje>/ports. Pide confirmación salvo con YES=1.
set -euo pipefail

mount="${1:-}"; app="${2:-}"; bin="${3:-}"; assets="${4:-}"; crate="${5:-}"
infra="$(cd "$(dirname "$0")/.." && pwd)"

die() { echo "deploy-sd: $*" >&2; exit 1; }

[ -n "$mount" ] || die "falta SD_MOUNT (ej.: SD_MOUNT=/run/media/$USER/EASYROMS just deploy-sd)"
[ -n "$app" ] && [ -n "$bin" ] || die "uso: deploy-sd.sh <montaje> <app> <binario> [assets]"
mount="$(realpath -e "$mount")" || die "no existe $mount"
case "$mount" in
  /|/home|/home/*/|"$HOME"|/boot|/boot/*|/usr|/usr/*|/etc|/etc/*) die "$mount no parece una SD, me niego" ;;
esac
findmnt -rn --target "$mount" -o TARGET | grep -qx "$mount" || die "$mount no es un punto de montaje"
fstype="$(findmnt -rn --target "$mount" -o FSTYPE)"
src_dev="$(findmnt -rn --target "$mount" -o SOURCE)"
[ -d "$mount/ports" ] || die "no hay carpeta ports/ en $mount: ¿es la partición de ROMs (EASYROMS)?"
[ -f "$bin" ] || die "no existe el binario $bin (correr 'just build-arm' antes)"
file -b "$bin" 2>/dev/null | grep -q 'ARM aarch64' || die "$bin no es un ELF aarch64"

licenses=()
if [ -n "$crate" ]; then
  # Se captura antes para que un error de list-licenses (p. ej. nombres repetidos) corte el deploy.
  lic_list="$("$infra/scripts/list-licenses.sh" "$crate")" || die "no pude listar las licencias de $crate"
  [ -z "$lic_list" ] || mapfile -t licenses <<< "$lic_list"
fi

name="$(echo "${app:0:1}" | tr '[:lower:]' '[:upper:]')${app:1}"
dest="$mount/ports/$app"
echo "Dispositivo: $src_dev ($fstype) montado en $mount"
echo "Se va a escribir: $mount/ports/$name.sh y $dest/"
for f in "${licenses[@]}"; do echo "  licencia: ${f#"$crate"/}"; done
if [ "${YES:-0}" != "1" ]; then
  read -r -p "¿Continuar? [s/N] " ans
  [ "$ans" = "s" ] || [ "$ans" = "S" ] || die "cancelado"
fi

mkdir -p "$dest"
# FAT/exFAT: sin permisos ni dueños; --modify-window por la resolución de 2 s de FAT.
rsync_opts=(-rt --no-perms --no-owner --no-group --modify-window=2 --info=progress2)
rsync "${rsync_opts[@]}" "$bin" "$dest/$(basename "$bin")"
# Licencias de terceros junto al binario (p. ej. la BSD-2-Clause de Spleen pide su aviso en binarios).
if [ "${#licenses[@]}" -gt 0 ]; then
  rsync "${rsync_opts[@]}" "${licenses[@]}" "$dest/"
fi
if [ -n "$assets" ] && [ -d "$assets" ]; then
  rsync "${rsync_opts[@]}" --delete "$assets/" "$dest/assets/"
fi
sed -e "s/@APP@/$app/g" -e "s/@BIN@/$(basename "$bin")/g" \
  "$infra/launcher/port-launcher.sh.in" > "$mount/ports/$name.sh"
chmod +x "$mount/ports/$name.sh" 2>/dev/null || true
sync
echo "Listo. Desmontar antes de sacar la SD: udisksctl unmount -b $src_dev"
