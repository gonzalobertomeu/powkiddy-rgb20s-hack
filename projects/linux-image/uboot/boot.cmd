# boot.cmd -> boot.scr (lo compila build-image.sh con mkimage -T script).
# El U-Boot de ROCKNIX (rk3326-handheld_defconfig) arranca con
#   bootmeth order script; bootflow scan -b
# o sea, SOLO ejecuta scripts boot.scr: sin este archivo no lee extlinux.conf.
# Versión mínima del b_boot.ini de ROCKNIX (rocknix/b_boot.ini.reference): no detecta el
# modelo por ADC, porque la imagen es solo para la RGB20S y extlinux.conf fija el DTB.

setenv thispart "${devtype} ${devnum}:${distro_bootpart}"
echo "rgb20s: boot.scr desde ${thispart}"

# Mismas direcciones que ROCKNIX: el kernel en 144 MiB, lejos del DTB y del script.
setenv kernel_addr_r "0x09000000"
setenv fdtoverlay_addr_r "0x08000000"

sysboot ${thispart} any 0x100000 /extlinux/extlinux.conf

echo "rgb20s: sysboot falló; revisar /extlinux/extlinux.conf en la partición de arranque"
