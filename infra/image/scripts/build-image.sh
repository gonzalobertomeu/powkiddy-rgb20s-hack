#!/usr/bin/env bash
# Arma infra/build/out/rgb20s-arch.img a partir de los artefactos de build-uboot.sh,
# build-kernel.sh y build-rootfs.sh. Corre DENTRO de rgb20s-image-builder como root del
# contenedor (in-builder.sh --root), para conservar dueños y permisos de la rootfs.
# No usa loop devices ni toca dispositivos: todo son archivos.
#
# Disposición (MBR, sectores de 512 bytes):
#   0            MBR (tabla de particiones)
#   32..63       (entorno de U-Boot, CONFIG_ENV_OFFSET=0x4000; no se usa) -> por eso MBR y no GPT
#   64           idbloader (DDR + miniloader)       \
#   16384        uboot.img (U-Boot)                  > uboot.bin, escrito en el sector 64
#   24576        trust.img (BL31)                   /
#   32768 (16M)  p1 FAT32 "RGB20S_BOOT" 256 MiB, booteable: Image, DTB, extlinux, boot.scr, logs
#   ...          p2 ext4  "RGB20S_ROOT": rootfs de Arch Linux ARM
set -euo pipefail
cd /work
log() { printf '\033[1;32m[imagen]\033[0m %s\n' "$*"; }

o=infra/build/out
b=infra/build/image
li=projects/linux-image
img="$o/rgb20s-arch.img"
krel="$(cat "$o/kernel/kernel.release")"

# Identificadores de disco y líneas de comandos del kernel (CMDLINE, CMDLINE_VERBOSE)
. "$li/boot/boot.env"
BOOT_MIB=256

for f in "$o/uboot/uboot.bin" "$o/uboot/mkimage" "$o/kernel/Image" "$o/rootfs/rootfs.tar" \
         "$o/splash/rgb20s-splash" "$o/splash/splash.ppm"; do
  [ -f "$f" ] || { echo "falta $f (correr los pasos anteriores)" >&2; exit 2; }
done

rm -rf "$b"; mkdir -p "$b/boot" "$b/rootfs"

# --- rootfs ---
log "extrayendo rootfs.tar (conserva dueños y permisos)"
bsdtar -xpf "$o/rootfs/rootfs.tar" --numeric-owner -C "$b/rootfs"
r="$b/rootfs"
rm -f "$r/.dockerenv"
# Docker monta estos archivos durante el build: en el export quedan los de la capa base
echo rgb20s > "$r/etc/hostname"
printf '127.0.0.1\tlocalhost\n::1\t\tlocalhost\n127.0.1.1\trgb20s\n' > "$r/etc/hosts"
ln -sfn /run/systemd/resolve/stub-resolv.conf "$r/etc/resolv.conf"
: > "$r/etc/machine-id"                       # se genera en el primer arranque
# El llavero de pacman del build tiene una clave privada local: no repartirla en la imagen.
# En la consola: sudo pacman-key --init && sudo pacman-key --populate archlinuxarm
rm -rf "$r/etc/pacman.d/gnupg"
rm -f "$r"/etc/ssh/ssh_host_*                 # sshdgenkeys.service las genera al arrancar
cat > "$r/etc/fstab" <<EOF
# <dispositivo>                <punto>  <tipo>  <opciones>                                          <dump> <pass>
PARTUUID=$ROOT_PARTUUID  /        ext4    rw,noatime                                          0 1
PARTUUID=$BOOT_PARTUUID  /boot    vfat    rw,noatime,fmask=0022,dmask=0022,nofail,x-systemd.device-timeout=10s  0 2
EOF
mkdir -p "$r/boot"
# El overlay se vuelve a copiar acá (solo archivos; los servicios se habilitan en customize.sh),
# así un cambio en el overlay no obliga a repetir build-rootfs.sh
cp -rT --no-preserve=ownership "$li/rootfs/overlay" "$r"
chmod 0755 "$r"/usr/local/lib/rgb20s/*.sh
log "splash de arranque (rgb20s-splash)"
sp="$li/splash/system"
install -D -m 0755 "$o/splash/rgb20s-splash" "$r/usr/local/bin/rgb20s-splash"
install -D -m 0644 "$o/splash/splash.ppm" "$r/usr/local/share/rgb20s/splash.ppm"
install -D -m 0644 "$sp/rgb20s-splash.service" "$r/etc/systemd/system/rgb20s-splash.service"
install -D -m 0644 "$sp/getty-tty1-splash.conf" "$r/etc/systemd/system/getty@tty1.service.d/10-rgb20s-splash.conf"
# = systemctl enable (WantedBy=sysinit.target), sin systemd en este contenedor
mkdir -p "$r/etc/systemd/system/sysinit.target.wants"
ln -sfn /etc/systemd/system/rgb20s-splash.service "$r/etc/systemd/system/sysinit.target.wants/rgb20s-splash.service"
log "módulos del kernel $krel"
rm -rf "$r/usr/lib/modules/$krel"
cp -rT --no-preserve=ownership "$o/kernel/modroot/usr/lib/modules" "$r/usr/lib/modules"
[ -f "$r/usr/lib/modules/$krel/modules.dep" ] || { echo "faltan los módulos de $krel" >&2; exit 1; }

# --- partición de arranque ---
log "armando /boot"
bt="$b/boot"
mkdir -p "$bt/dtbs/rockchip" "$bt/extlinux" "$bt/logs"
cp "$o/kernel/Image" "$bt/Image"
cp "$o/kernel/dtbs/rockchip/"*.dtb "$bt/dtbs/rockchip/"
cp "$o/kernel/config" "$bt/config-$krel"
sed -e "s|@CMDLINE@|$CMDLINE|" -e "s|@CMDLINE_VERBOSE@|$CMDLINE_VERBOSE|" \
  "$li/boot/extlinux.conf.in" > "$bt/extlinux/extlinux.conf"
grep -q '@' "$bt/extlinux/extlinux.conf" && { echo "extlinux.conf: quedó un @MARCADOR@ sin reemplazar" >&2; exit 1; }
grep -n ' $' "$bt/extlinux/extlinux.conf" && { echo "extlinux.conf tiene espacios al final de línea" >&2; exit 1; }
"$o/uboot/mkimage" -A arm64 -O linux -T script -C none -n "rgb20s boot" -d "$li/uboot/boot.cmd" "$bt/boot.scr" >/dev/null
cp "$li/boot/rgb20s.conf" "$bt/rgb20s.conf"
cat > "$bt/LEEME.txt" <<EOF
RGB20S - Arch Linux ARM ($(date -u +%Y-%m-%d), kernel $krel)
- rgb20s.conf      rol del USB-C (device/host), IP del gadget, logs, agrandar la raíz
- authorized_keys  (opcional) claves SSH públicas: se agregan al usuario alarm al arrancar
- logs/            diagnóstico de cada arranque (las últimas 10 sesiones)
- extlinux/        entradas de arranque: arch (splash), arch-verbose (todos los mensajes), arch-rocknix
Documentación: projects/linux-image/README.md y docs/firmware/arch-linux-arm.md del repo.
EOF

boot_img="$b/boot.vfat"
rm -f "$boot_img"
mkfs.vfat -F 32 -n RGB20S_BOOT -i "$BOOT_VOLID" -C "$boot_img" $((BOOT_MIB * 1024)) >/dev/null
mcopy -s -i "$boot_img" "$bt"/* ::/

# --- partición raíz ---
used_mib=$(( $(du -sm --apparent-size "$r" | cut -f1) ))
root_mib=$(( used_mib * 13 / 10 + 256 ))
log "ext4: ${used_mib} MiB usados -> partición de ${root_mib} MiB"
root_img="$b/root.ext4"
rm -f "$root_img"
mke2fs -q -t ext4 -L RGB20S_ROOT -U "$ROOT_FSUUID" -E hash_seed="$ROOT_FSUUID" \
  -d "$r" "$root_img" "${root_mib}M"

# --- disco ---
boot_start=32768
boot_sectors=$(( BOOT_MIB * 2048 ))
root_start=$(( boot_start + boot_sectors ))
root_sectors=$(( root_mib * 2048 ))
total_sectors=$(( root_start + root_sectors ))
log "disco: $(( total_sectors / 2048 )) MiB"
rm -f "$img"
truncate -s $(( total_sectors * 512 )) "$img"
sfdisk -q "$img" <<EOF
label: dos
label-id: $DISK_ID
unit: sectors

start=$boot_start, size=$boot_sectors, type=c, bootable
start=$root_start, size=$root_sectors, type=83
EOF
dd if="$boot_img" of="$img" bs=512 seek=$boot_start conv=notrunc,sparse status=none
dd if="$root_img" of="$img" bs=512 seek=$root_start conv=notrunc,sparse status=none
dd if="$o/uboot/uboot.bin" of="$img" bs=512 seek=64 conv=notrunc status=none

sfdisk -d "$img"
sha256sum "$img" | sed 's|  .*/|  |' > "$img.sha256"
chown "${HOST_UID:-0}:${HOST_GID:-0}" "$img" "$img.sha256"
rm -rf "$r"    # la rootfs extraída tiene archivos de root: no dejarla en el árbol del usuario
chown -R "${HOST_UID:-0}:${HOST_GID:-0}" "$b"
log "listo: $img"
ls -la "$img"
