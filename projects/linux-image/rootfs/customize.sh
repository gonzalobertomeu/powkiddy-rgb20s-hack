#!/bin/bash
# Personaliza la rootfs de Arch Linux ARM para la RGB20S.
# Corre como root DENTRO de un contenedor arm64 creado desde el tarball de ALARM
# (infra/image/scripts/build-rootfs.sh), emulado con qemu-aarch64-static (binfmt del host).
# Montajes:
#   /setup    projects/linux-image/rootfs (solo lectura)
#   /keys     infra/image/local (opcional): authorized_keys
#   /out      salida: lista de paquetes instalados
set -euo pipefail
log() { printf '\033[1;35m[rootfs]\033[0m %s\n' "$*"; }

# --- pacman: sin documentación ni traducciones, sin chequeo de espacio (falla en contenedores) ---
sed -i 's/^CheckSpace/#CheckSpace/' /etc/pacman.conf
sed -i '/^#NoExtract/a NoExtract = usr/share/man/* usr/share/doc/* usr/share/info/* usr/share/gtk-doc/*\nNoExtract = usr/share/locale/* !usr/share/locale/locale.alias usr/share/i18n/locales/* !usr/share/i18n/locales/C !usr/share/i18n/locales/POSIX' /etc/pacman.conf
# El sandbox del descargador (Landlock/seccomp) no es confiable bajo qemu-user.
PAC=(pacman --noconfirm --disable-sandbox)

log "llavero de pacman"
pacman-key --init
pacman-key --populate archlinuxarm

# --- sacar lo que no hace falta (antes del -Syu, para no bajar actualizaciones que se borran) ---
# Kernel genérico y firmware (usamos el kernel propio; la RGB20S no necesita firmware),
# mkinitcpio (sin initramfs), netctl y dhcpcd (la red la maneja systemd-networkd), editores grandes.
remove=(linux-aarch64 mkinitcpio mkinitcpio-busybox netctl dhcpcd net-tools vim vim-runtime ex-vi-compat)
mapfile -t fw < <(pacman -Qq | grep '^linux-firmware' || true)
remove+=("${fw[@]}")
installed=()
for p in "${remove[@]}"; do pacman -Qq "$p" >/dev/null 2>&1 && installed+=("$p"); done
log "quitando: ${installed[*]}"
"${PAC[@]}" -Rns "${installed[@]}"

log "actualizando el sistema (pacman -Syu)"
"${PAC[@]}" -Syu

log "instalando paquetes de projects/linux-image/rootfs/packages.txt"
mapfile -t pkgs < <(grep -v '^\s*#' /setup/packages.txt | tr -s ' \t' '\n' | grep -v '^$')
"${PAC[@]}" -S --needed "${pkgs[@]}"

# Huérfanos y caché
orph="$(pacman -Qdtq || true)"
[ -z "$orph" ] || "${PAC[@]}" -Rns $orph
"${PAC[@]}" -Scc >/dev/null
rm -rf /var/cache/pacman/pkg/* /usr/share/man/* /usr/share/doc/* /usr/share/info/*
find /usr/share/locale -mindepth 1 -maxdepth 1 ! -name locale.alias -exec rm -rf {} +

# --- archivos propios ---
log "copiando overlay"
cp -rT --no-preserve=ownership /setup/overlay /
chmod 0755 /usr/local/lib/rgb20s/*.sh

# Quitar las configs de red de ALARM (en*/eth* por DHCP): usamos las propias, que matchean por driver
rm -f /etc/systemd/network/en.network /etc/systemd/network/eth.network

# --- usuarios ---
# alarm/alarm (documentado en projects/linux-image/README.md); root bloqueado.
usermod -aG wheel,audio,video,input,uucp alarm
echo 'alarm:alarm' | chpasswd
passwd -l root
install -m 0440 /dev/stdin /etc/sudoers.d/10-wheel <<< '%wheel ALL=(ALL:ALL) ALL'

if [ -s /keys/authorized_keys ]; then
  log "authorized_keys de infra/image/local/ -> /home/alarm/.ssh/"
  install -d -m 700 -o alarm -g alarm /home/alarm/.ssh
  install -m 600 -o alarm -g alarm /keys/authorized_keys /home/alarm/.ssh/authorized_keys
  rm -f /etc/ssh/sshd_config.d/20-rgb20s-password.conf
else
  log "AVISO: sin authorized_keys: se habilita SSH con contraseña (alarm/alarm)"
  printf '# Sin clave pública al armar la imagen: se permite contraseña.\nPasswordAuthentication yes\n' \
    > /etc/ssh/sshd_config.d/20-rgb20s-password.conf
fi

# --- sistema ---
echo 'LANG=C.UTF-8' > /etc/locale.conf
echo 'KEYMAP=us' > /etc/vconsole.conf
ln -sf /usr/share/zoneinfo/UTC /etc/localtime

log "servicios"
systemctl enable systemd-networkd.service systemd-resolved.service sshd.service \
  rgb20s-usb.service rgb20s-firstboot.service rgb20s-bootlog.service rgb20s-bootlog-early.service
# Sin red "real" esperar la red solo demora el arranque (hasta 2 min)
systemctl disable systemd-networkd-wait-online.service
systemctl mask systemd-networkd-wait-online.service
# Sin internet no hay NTP
systemctl disable systemd-timesyncd.service
# La imagen sale con /etc/machine-id vacío (primer arranque): systemd-firstboot podría
# quedarse esperando respuestas en tty1, y la consola no tiene teclado.
systemctl mask systemd-firstboot.service

pacman -Q > /out/packages.txt
du -sh --one-file-system --exclude=/proc --exclude=/sys --exclude=/dev --exclude=/setup --exclude=/keys --exclude=/out / 2>/dev/null | tee /out/size.txt
chown "${HOST_UID:-0}:${HOST_GID:-0}" /out/packages.txt /out/size.txt
log "listo"
