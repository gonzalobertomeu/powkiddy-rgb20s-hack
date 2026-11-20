#!/usr/bin/env bash
# Lista (una por línea, rutas absolutas) las licencias de terceros que declara un crate.
# Uso: list-licenses.sh <directorio-del-crate>
#
# Convención: un crate "declara" una licencia teniendo, en cualquier subcarpeta hasta 3 niveles
# (fuera de target/ y .git/), un archivo LICENSE*, LICENCE*, COPYING* o NOTICE*. Ejemplo:
# projects/groovebox/fonts/LICENSE-spleen (BSD-2-Clause de la fuente embebida).
# Los deploys copian estos archivos planos junto al binario, así que los nombres no pueden
# repetirse: si dos archivos tienen el mismo nombre, falla (renombrar, p. ej. LICENSE-<origen>).
set -euo pipefail
crate="$(realpath -e "${1:?uso: list-licenses.sh <crate>}")"
mapfile -t files < <(find "$crate" -maxdepth 3 \( -name target -o -name .git \) -prune -o -type f \
  \( -iname 'LICENSE*' -o -iname 'LICENCE*' -o -iname 'COPYING*' -o -iname 'NOTICE*' \) -print | sort)
dups="$(for f in "${files[@]}"; do basename "$f"; done | sort | uniq -d)"
[ -z "$dups" ] || { echo "list-licenses: nombres repetidos: $dups" >&2; exit 1; }
for f in "${files[@]}"; do echo "$f"; done
