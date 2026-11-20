# Funciones comunes de los servicios rgb20s-* (se incluye con `.`).

RGB20S_CONF=/boot/rgb20s.conf

# conf_get <VARIABLE> <valor por defecto>
# Lee /boot/rgb20s.conf sin ejecutarlo (la FAT la puede editar cualquiera desde la Mac):
# solo acepta líneas VARIABLE=valor con caracteres seguros.
conf_get() {
  local key="$1" def="$2" val=""
  if [ -r "$RGB20S_CONF" ]; then
    val="$(tr -d '\r' < "$RGB20S_CONF" | sed -n "s/^${key}=\([A-Za-z0-9._:/-]*\)[[:space:]]*\$/\1/p" | tail -n1)"
  fi
  printf '%s' "${val:-$def}"
}

log() { echo "rgb20s: $*"; }
