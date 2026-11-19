---
name: alarm-rootfs
description: Armar y adelgazar una rootfs de Arch Linux ARM (aarch64) desde la máquina x86 usando qemu-aarch64-static y binfmt, empaquetarla en una .img con loop devices y dejarla lista como appliance (SSH, Wi-Fi, autoarranque de una app en KMS). Usar al construir o modificar la rootfs de la imagen propia.
---

# Rootfs de Arch Linux ARM

## Descarga y verificación

```sh
mkdir -p infra/downloads/alarm && cd infra/downloads/alarm   # en .gitignore
curl -LO http://os.archlinuxarm.org/os/ArchLinuxARM-aarch64-latest.tar.gz
curl -LO http://os.archlinuxarm.org/os/ArchLinuxARM-aarch64-latest.tar.gz.md5
curl -LO http://os.archlinuxarm.org/os/ArchLinuxARM-aarch64-latest.tar.gz.sig
md5sum -c ArchLinuxARM-aarch64-latest.tar.gz.md5
gpg --verify ArchLinuxARM-aarch64-latest.tar.gz.sig   # requiere la clave de Arch Linux ARM
```

## Extraer y entrar al chroot

Hace falta root para conservar dueños y permisos. Pedir confirmación y explicar los comandos.

```sh
sudo mkdir -p build/rootfs
sudo bsdtar -xpf ArchLinuxARM-aarch64-latest.tar.gz -C build/rootfs
# binfmt de qemu-aarch64 ya está registrado en el host (flag F), así que no hace falta copiar qemu
sudo arch-chroot build/rootfs /bin/bash      # paquete arch-install-scripts
```

Dentro del chroot:
```sh
pacman-key --init && pacman-key --populate archlinuxarm
pacman -Syu
pacman -Rns linux-aarch64      # usamos el kernel propio (ver la skill rockchip-rk3326-boot)
pacman -S openssh iwd sdl2 alsa-utils   # lo mínimo necesario
```

## Adelgazar

- Eliminar el kernel genérico, firmware que no se use (`linux-firmware` → solo los blobs del Wi-Fi real), man pages y locales que sobren (`NoExtract` en `pacman.conf`).
- Revisar el tamaño con `du -sh build/rootfs` y `pacman -Qi | grep -E '^(Name|Installed Size)'`.
- Init: systemd es el estándar de Arch. Cambiarlo por algo más liviano es un ADR aparte, porque Arch Linux ARM no lo soporta oficialmente.

## Seguridad y usuarios

- Los usuarios por defecto `alarm/alarm` y `root/root` hay que **cambiarlos o deshabilitarlos**.
- SSH solo con clave: `PasswordAuthentication no` y la clave pública del usuario en `authorized_keys`.
- Nunca commitear claves ni credenciales de Wi-Fi: se inyectan al armar la imagen desde un archivo ignorado por git.

## Appliance: arrancar directo en la app

Un servicio systemd `groovebox.service`, con `After=sound.target`, que corre como usuario sin privilegios en los grupos `audio`, `video` e `input`. Con `SDL_VIDEODRIVER=kmsdrm` dibuja directo sobre KMS, sin X ni Wayland. Agregar `Restart=on-failure`.

## Empaquetar en una .img

```sh
truncate -s 2G rgb20s.img
sgdisk -o -n1:32768:+256M -c1:boot -n2:0:0 -c2:root rgb20s.img   # p1 empieza en 16 MiB
LOOP=$(sudo losetup --show -fP rgb20s.img)
sudo mkfs.vfat -F32 ${LOOP}p1 && sudo mkfs.ext4 -L root ${LOOP}p2
# montar, copiar la rootfs y /boot, generar extlinux.conf, desmontar
sudo losetup -d $LOOP
# escribir el bootloader en el sector 64 (ver la skill rockchip-rk3326-boot)
```

Todo lo anterior va en un script reproducible en `projects/linux-image/`. **Nunca apuntar a `/dev/sdX` o `/dev/mmcblk*`** sin `lsblk` y confirmación del usuario.
