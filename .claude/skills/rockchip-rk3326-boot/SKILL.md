---
name: rockchip-rk3326-boot
description: Cadena de arranque del RK3326 y cómo armar U-Boot, kernel mainline y device tree para la RGB20S. Cubre rkbin (TPL/DDR y BL31), offsets en la SD, extlinux y opciones de kernel para pantalla, audio, controles, USB y Wi-Fi. Usar al trabajar en el arranque o en el kernel de la imagen propia.
---

# Arranque del RK3326 (RGB20S)

> Correcciones verificadas por `alarm-image-builder` (2026-11-19): ver la tabla en `docs/firmware/arch-linux-arm.md#correcciones-a-las-skills-del-proyecto`. Con el defconfig de ROCKNIX hace falta un `boot.scr`.

> Verificá cada dato contra la documentación oficial de U-Boot (`doc/board/rockchip/rockchip.rst`) y del kernel antes de escribir una imagen. Lo marcado **(sin verificar)** no se probó en esta consola.

## Cadena de arranque

```
BootROM (en el SoC) ─► busca un idbloader en la SD, en el sector 64
  └─ TPL / init de DDR (blob de rkbin: rk3326_ddr_*.bin)
     └─ SPL de U-Boot
        └─ BL31 (TF-A, plataforma px30, o blob de rkbin) + U-Boot propiamente dicho (u-boot.itb)
           └─ U-Boot standard boot / distro boot ─► /boot/extlinux/extlinux.conf
              └─ kernel Image + device tree + initramfs (opcional)
```

## U-Boot mainline

- Defconfig de referencia: `odroid-go2_defconfig`; la imagen del proyecto usa `rk3326-handheld_defconfig` de ROCKNIX con el flujo miniloader de rkbin (ver `docs/firmware/arch-linux-arm.md` y ADR 0009) (Odroid Go Advance, mismo SoC). Para la RGB20S hace falta su device tree o adaptar el de la OGA **(sin verificar)**.
- Insumos: `ROCKCHIP_TPL=<rkbin>/bin/rk33/rk3326_ddr_*.bin` y `BL31=<rkbin o TF-A>/bl31.elf`.
- El build genera `u-boot-rockchip.bin`, que junta idbloader y u-boot.itb. Se escribe **sobre la `.img`**, nunca sobre una SD sin confirmación:
  ```sh
  dd if=u-boot-rockchip.bin of=rgb20s.img seek=64 conv=notrunc
  ```
- Las particiones tienen que empezar después de la zona del bootloader: primera partición en **16 MiB (sector 32768)** o más.

## Disposición de la imagen

| Zona | Contenido |
|---|---|
| sector 0 | Tabla de particiones (**solo MBR** con este U-Boot: el entorno en `0x4000` choca con la GPT) |
| sector 64 a ~16 MiB | `u-boot-rockchip.bin` (flujo TPL/SPL) o, en el flujo miniloader que usa la imagen: `idbloader` en 64, `uboot.img` en 16384, `trust.img` en 24576 |
| p1 (FAT32 o ext4, unos 256 MiB) | `/boot`: Image, dtbs, extlinux |
| p2 (ext4) | rootfs |

`extlinux.conf` de ejemplo:
```
LABEL rgb20s
  KERNEL /Image
  FDT /dtbs/rockchip/rk3326-powkiddy-rgb20s.dtb
  APPEND root=PARTUUID=<uuid-p2> rw rootwait console=ttyS2,1500000 quiet
```
La consola serial (`ttyS2`) es habitual en el RK3326 **(sin verificar en la RGB20S; puede no tener pads accesibles)**.

## Device tree

- Fuente: `rk3326-powkiddy-rgb20s.dts` del árbol de **ROCKNIX** (no está en mainline). Comparar con `rk3326-odroid-go2.dts` y `rk3326-anbernic-rg351m.dtsi` de mainline.
- Revisar en el DTS qué usa: panel (driver `panel-*`), joypad (en la RGB20S: `rocknix-singleadc-joypad`, fuera de mainline; `gpio-keys` solo para volumen), códec, regulador de backlight, Wi-Fi (SDIO o USB, qué chip).

## Kernel mainline: opciones clave

| Área | Opciones |
|---|---|
| Pantalla | `DRM_ROCKCHIP`, `ROCKCHIP_VOP`, `ROCKCHIP_DW_MIPI_DSI`, el `DRM_PANEL_*` del panel, `BACKLIGHT_PWM` |
| GPU | `DRM_PANFROST` |
| PMIC y audio | `MFD_RK8XX_I2C`, `SND_SOC_ROCKCHIP_I2S`, `SND_SOC_RK817`, `SND_SIMPLE_CARD` |
| Controles | `JOYSTICK_ADC`, `KEYBOARD_GPIO`, `INPUT_EVDEV`, `ROCKCHIP_SARADC` |
| USB OTG y gadget | `USB_DWC2` (modo dual), `USB_CONFIGFS`, `USB_CONFIGFS_F_MIDI`, `USB_CONFIGFS_F_HID`, `USB_CONFIGFS_ECM` |
| Almacenamiento | `MMC_DW_ROCKCHIP` |
| Wi-Fi | el driver del chip real **(sin verificar: identificarlo con `device-ops`)** |

Compilar fuera del árbol del kernel con `ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu-`, o dentro del contenedor de `dev-infra`.

## Si no arranca

Sin consola serial no hay logs. Opciones: probar primero el kernel y el DTB con el U-Boot de dArkOS/ROCKNIX (cambiando solo `/boot`), mantener siempre una SD que arranque, y agregar un initramfs con un shell de rescate.
