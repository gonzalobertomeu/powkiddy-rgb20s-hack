#!/usr/bin/env bash
# Revisa rgb20s-arch.img sin montarla ni usar loop devices (corre en rgb20s-image-builder).
# Uso: check-image.sh [imagen]
set -uo pipefail
cd /work
img="${1:-infra/build/out/rgb20s-arch.img}"
o=infra/build/out
t="$(mktemp -d)"; trap 'rm -rf "$t"' EXIT
ok=0; bad=0
pass() { printf '  \033[32mOK\033[0m   %s\n' "$*"; ok=$((ok+1)); }
fail() { printf '  \033[31mFALLA\033[0m %s\n' "$*"; bad=$((bad+1)); }
check() { local d="$1"; shift; if "$@" >/dev/null 2>&1; then pass "$d"; else fail "$d"; fi; }

echo "== tabla de particiones"
sfdisk -d "$img"
dump="$(sfdisk -d "$img")"
check "MBR (dos)" grep -q '^label: dos' <<< "$dump"
check "p1 en el sector 32768, tipo c, booteable" grep -Eq 'start= *32768, size= *524288, type=c, bootable' <<< "$dump"
check "p2 tipo 83 a continuación" grep -Eq 'start= *557056, .*type=83' <<< "$dump"

echo "== bootloader"
n=$(stat -c %s "$o/uboot/uboot.bin")
dd if="$img" of="$t/bl.bin" bs=512 skip=64 count=$(( n / 512 )) status=none
check "sectores 64..32767 == uboot.bin" cmp -s "$t/bl.bin" "$o/uboot/uboot.bin"
n_idb=$(stat -c %s "$o/uboot/idbloader.img")
dd if="$img" of="$t/idb" bs=512 skip=64 count=$(( (n_idb + 511) / 512 )) status=none
check "idbloader en el sector 64" cmp -s -n "$n_idb" "$t/idb" "$o/uboot/idbloader.img"
dd if="$img" of="$t/ub" bs=512 skip=16384 count=8192 status=none
check "uboot.img en el sector 16384" cmp -s "$t/ub" "$o/uboot/uboot.img"
dd if="$img" of="$t/tr" bs=512 skip=24576 count=8192 status=none
check "trust.img en el sector 24576" cmp -s "$t/tr" "$o/uboot/trust.img"
check "sectores 32..63 (entorno de U-Boot) vacíos" sh -c "dd if='$img' bs=512 skip=32 count=32 status=none | cmp -s - /dev/zero -n 16384"

echo "== partición de arranque (FAT)"
dd if="$img" of="$t/boot.vfat" bs=512 skip=32768 count=524288 status=none
export MTOOLS_SKIP_CHECK=1
mdir -/ -b -i "$t/boot.vfat" :: | sed 's/^/    /'
for f in Image boot.scr extlinux/extlinux.conf rgb20s.conf dtbs/rockchip/rk3326-powkiddy-rgb20s-alarm.dtb dtbs/rockchip/rk3326-powkiddy-rgb20s.dtb; do
  check "/$f" mtype -i "$t/boot.vfat" "::/$f"
done
mtype -i "$t/boot.vfat" ::/extlinux/extlinux.conf > "$t/extlinux.conf"
check "extlinux: sin espacios al final de línea" sh -c "! grep -q ' \$' '$t/extlinux.conf'"
check "extlinux: root=PARTUUID=52474253-02" grep -q 'root=PARTUUID=52474253-02' "$t/extlinux.conf"
mcopy -i "$t/boot.vfat" ::/boot.scr "$t/boot.scr"
check "boot.scr es un script de U-Boot (mkimage)" sh -c "'$o/uboot/mkimage' -l '$t/boot.scr' | grep -q Script"
mcopy -i "$t/boot.vfat" ::/dtbs/rockchip/rk3326-powkiddy-rgb20s-alarm.dtb "$t/a.dtb"
dtc -I dtb -O dts -o "$t/a.dts" "$t/a.dtb" 2>/dev/null
check "DTB: modelo RGB20S" grep -q 'model = "Powkiddy RGB20S (Arch Linux ARM)"' "$t/a.dts"
check "DTB: dwc2 con role-switch-default-mode = peripheral" grep -q 'role-switch-default-mode = "peripheral"' "$t/a.dts"
check "DTB: panel rocknix,generic-dsi" grep -q 'rocknix,generic-dsi' "$t/a.dts"
check "DTB: joypad rocknix-singleadc-joypad" grep -q 'rocknix-singleadc-joypad' "$t/a.dts"

echo "== partición raíz (ext4, con debugfs)"
start=$(sfdisk -d "$img" | sed -n 's/^.*2 : start= *\([0-9]*\),.*/\1/p')
dd if="$img" of="$t/root.ext4" bs=512 skip="$start" status=none
dbg() { debugfs -R "$1" "$t/root.ext4" 2>/dev/null; }
krel="$(cat "$o/kernel/kernel.release")"
check "e2fsck sin errores" e2fsck -fn "$t/root.ext4"
check "/usr/lib/modules/$krel/modules.dep" sh -c "debugfs -R 'stat /usr/lib/modules/$krel/modules.dep' '$t/root.ext4' 2>/dev/null | grep -q Inode"
check "módulo del joypad" sh -c "debugfs -R 'cat /usr/lib/modules/$krel/modules.dep' '$t/root.ext4' 2>/dev/null | grep -q rocknix-singleadc-joypad.ko"
dbg "cat /etc/fstab" > "$t/fstab"
check "carga del joypad en modules-load.d" sh -c "debugfs -R 'cat /etc/modules-load.d/rgb20s.conf' '$t/root.ext4' 2>/dev/null | grep -qx rocknix-singleadc-joypad"
check "fstab: raíz y /boot por PARTUUID" sh -c "grep -q 'PARTUUID=52474253-02 *[/] ' '$t/fstab' && grep -q 'PARTUUID=52474253-01 */boot' '$t/fstab'"
check "/etc/machine-id vacío" sh -c "debugfs -R 'stat /etc/machine-id' '$t/root.ext4' 2>/dev/null | grep -q 'Size: 0\$'"
check "sin claves de host SSH precargadas" sh -c "! debugfs -R 'ls -l /etc/ssh' '$t/root.ext4' 2>/dev/null | grep -q ssh_host_"
check "sin llavero de pacman del build" sh -c "! debugfs -R 'ls /etc/pacman.d' '$t/root.ext4' 2>/dev/null | grep -q gnupg"
check "sin /.dockerenv" sh -c "! debugfs -R 'ls /' '$t/root.ext4' 2>/dev/null | grep -q dockerenv"
check "hostname rgb20s" sh -c "debugfs -R 'cat /etc/hostname' '$t/root.ext4' 2>/dev/null | grep -qx rgb20s"
wants=/etc/systemd/system/multi-user.target.wants
for u in sshd systemd-networkd rgb20s-usb rgb20s-firstboot rgb20s-bootlog; do
  check "habilitado: $u" sh -c "debugfs -R 'ls $wants' '$t/root.ext4' 2>/dev/null | grep -q '$u.service'"
done
check "habilitado: rgb20s-bootlog-early (sysinit)" sh -c "debugfs -R 'ls /etc/systemd/system/sysinit.target.wants' '$t/root.ext4' 2>/dev/null | grep -q rgb20s-bootlog-early"
check "enmascarado: systemd-firstboot" sh -c "debugfs -R 'stat /etc/systemd/system/systemd-firstboot.service' '$t/root.ext4' 2>/dev/null | grep -q 'Fast link dest: \"/dev/null\"'"
check "enmascarado: systemd-networkd-wait-online" sh -c "debugfs -R 'stat /etc/systemd/system/systemd-networkd-wait-online.service' '$t/root.ext4' 2>/dev/null | grep -q '/dev/null'"
check "sin linux-aarch64 (kernel genérico)" sh -c "! debugfs -R 'ls /usr/lib/modules' '$t/root.ext4' 2>/dev/null | grep -q ARCH"
check "dueño root de /usr/bin/sudo con setuid" sh -c "debugfs -R 'stat /usr/bin/sudo' '$t/root.ext4' 2>/dev/null | grep -q 'Mode:  04755' && debugfs -R 'stat /usr/bin/sudo' '$t/root.ext4' 2>/dev/null | grep -q 'User:     0'"

echo "== resultado: $ok OK, $bad fallas"
[ "$bad" = 0 ]
