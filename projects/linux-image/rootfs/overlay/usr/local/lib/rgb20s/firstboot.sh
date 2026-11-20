#!/bin/bash
# Tareas tempranas de cada arranque (antes de sshd). Uso: firstboot.sh
#  1. Claves SSH: si existe /boot/authorized_keys (copiada desde la Mac), agrega sus líneas
#     a ~alarm/.ssh/authorized_keys. Permite entrar sin recompilar la imagen.
#  2. GROW_ROOT=yes en /boot/rgb20s.conf: agranda la partición raíz al tamaño de la tarjeta
#     (una sola vez; deja /var/lib/rgb20s/grown).
set -u
. /usr/local/lib/rgb20s/common.sh

# --- 1. claves SSH ---
src=/boot/authorized_keys
dst_dir=/home/alarm/.ssh
if [ -r "$src" ]; then
  install -d -m 700 -o alarm -g alarm "$dst_dir"
  touch "$dst_dir/authorized_keys"
  added=0
  while IFS= read -r line; do
    line="${line%$'\r'}"
    case "$line" in ssh-*|ecdsa-*|sk-*) ;; *) continue ;; esac
    grep -qxF -- "$line" "$dst_dir/authorized_keys" || { echo "$line" >> "$dst_dir/authorized_keys"; added=$((added+1)); }
  done < "$src"
  chown alarm:alarm "$dst_dir/authorized_keys"; chmod 600 "$dst_dir/authorized_keys"
  log "claves SSH: $added nuevas desde $src"
fi

# --- 2. agrandar la raíz ---
if [ "$(conf_get GROW_ROOT no)" = yes ] && [ ! -e /var/lib/rgb20s/grown ]; then
  part="$(findmnt -no SOURCE /)"                      # p. ej. /dev/mmcblk1p2
  disk="/dev/$(lsblk -no PKNAME "$part")"
  num="$(cat "/sys/class/block/$(basename "$part")/partition")"
  log "agrandando $part (partición $num de $disk)"
  if echo ', +' | sfdisk --no-reread --no-tell-kernel -N "$num" "$disk" \
     && partx -u -n "$num" "$disk" && resize2fs "$part"; then
    mkdir -p /var/lib/rgb20s && date -Is > /var/lib/rgb20s/grown
    log "raíz agrandada: $(df -h / | tail -n1)"
  else
    log "ERROR: no se pudo agrandar la raíz (queda como estaba)"
  fi
fi
exit 0
