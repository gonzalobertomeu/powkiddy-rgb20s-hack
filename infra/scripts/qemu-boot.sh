#!/usr/bin/env bash
# Nivel 3 de emulación: arranca un Linux aarch64 genérico con la CPU de la consola.
# No emula la placa RK3326 (ni pantalla, ni audio, ni botones): sirve para probar
# arranque, servicios y empaquetado de la rootfs que genere alarm-image-builder.
# Uso: qemu-boot.sh <kernel Image> <disco .img|.qcow2> [initramfs]
# Consola serie en la terminal; salir con Ctrl-a x.
set -euo pipefail
kernel="${1:-}"; disk="${2:-}"; initrd="${3:-}"
command -v qemu-system-aarch64 >/dev/null || {
  echo "falta qemu-system-aarch64. Instalar (requiere confirmación): sudo pacman -S qemu-system-aarch64" >&2; exit 2; }
[ -f "$kernel" ] && [ -f "$disk" ] || {
  echo "uso: qemu-boot.sh <kernel Image> <disco> [initramfs] (los genera alarm-image-builder)" >&2; exit 2; }

fmt=raw; case "$disk" in *.qcow2) fmt=qcow2 ;; esac
args=(
  -M virt -cpu cortex-a35 -smp 4 -m 1024        # 4x A35 y 1 GB, como la RGB20S
  -nographic
  -kernel "$kernel"
  -append "root=/dev/vda2 rw console=ttyAMA0"
  # snapshot=on: los cambios se descartan al salir, el .img no se modifica
  -drive "if=virtio,format=$fmt,file=$disk,snapshot=on"
  -netdev user,id=net0,hostfwd=tcp::2222-:22 -device virtio-net-device,netdev=net0
)
[ -n "$initrd" ] && args+=(-initrd "$initrd")
exec qemu-system-aarch64 "${args[@]}"
