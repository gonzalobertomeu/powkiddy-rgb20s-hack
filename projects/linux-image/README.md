# linux-image — Arch Linux ARM para la RGB20S

Imagen mínima de Arch Linux ARM (aarch64) para la Powkiddy RGB20S: U-Boot, kernel con el device tree de la
RGB20S, rootfs adelgazada y acceso de desarrollo por el USB-C (red + serie) hacia la Mac.

- Investigación, cadena de arranque, riesgos y diagnóstico: [`docs/firmware/arch-linux-arm.md`](../../docs/firmware/arch-linux-arm.md)
- Decisiones: ADR [0009](../../docs/decisions/0009-arranque-uboot-miniloader-rkbin.md),
  [0010](../../docs/decisions/0010-kernel-mainline-con-dts-y-drivers-de-rocknix.md),
  [0011](../../docs/decisions/0011-rootfs-alarm-con-systemd.md),
  [0012](../../docs/decisions/0012-acceso-por-usb-gadget-ncm-acm.md),
  [0015](../../docs/decisions/0015-splash-de-arranque-logo-del-kernel-y-drm.md) (splash, propuesta),
  [0017](../../docs/decisions/0017-sin-deteccion-de-jack-en-simple-audio-card.md) (jack, propuesta)
- Estado: la imagen arranca en la consola (2026-11-20). El splash de arranque ([`splash/`](splash/README.md)) está
  armado y probado en QEMU, **sin probar en la consola**.

Acá está **qué lleva la imagen**; **cómo se construye** está en [`infra/image/`](../../infra/image/).

## Construir

Todo corre en contenedores: sin `sudo`, sin loop devices y sin tocar ninguna SD.

```sh
just image-builder        # contenedor rgb20s-image-builder (una vez, ~3 min)
# (opcional) clave pública para entrar por SSH sin contraseña; no se commitea:
mkdir -p infra/image/local && cp ~/.ssh/id_ed25519.pub infra/image/local/authorized_keys
just img                  # fetch + U-Boot + splash + kernel + rootfs + imagen
```

O por pasos: `just img-fetch`, `just img-uboot`, `just img-splash`, `just img-kernel`, `just img-rootfs`, `just img-assemble`, `just img-check`.
Para actualizar una consola en uso sin reflashear (kernel, módulos, `extlinux.conf`, splash): `just img-device-update` y
[`splash/README.md`](splash/README.md#instalar-en-la-consola-en-uso-sin-reflashear).
Para cambiar solo el device tree (y diagnosticar el jack de auriculares): `just img-dtb-update` y [`jack/README.md`](jack/README.md).
El overlay y los módulos se copian en `img-assemble`: cambiarlos no obliga a repetir `img-rootfs`.
Resultado: `infra/build/out/rgb20s-arch.img` y su `.sha256`.

| Paso | Dónde corre | Tiempo medido (2026-11-19) |
|---|---|---|
| `img-fetch` | host (curl, gpg) | ~4 min (≈1 GB) |
| `img-uboot` | `rgb20s-image-builder` | ~1,5 min |
| `img-splash` | `rgb20s-image-builder` | segundos |
| `img-kernel` | `rgb20s-image-builder` | ~23 min limpio (6 hilos, en paralelo con la rootfs); incremental, segundos |
| `img-rootfs` | contenedor arm64 de ALARM, emulado | ~13 min (incluye `pacman -Syu`) |
| `img-assemble` | `rgb20s-image-builder` como root del contenedor | ~10 s |
| `img-check` | `rgb20s-image-builder` | ~10 s: 41 chequeos (offsets, particiones, FAT, DTB, ext4) |

**Grabar en la SD no es parte del build.** Lo hace el usuario, a mano, después de respaldar la tarjeta y confirmar el dispositivo
con `lsblk` (ver "Grabar" más abajo).

## Contenido

```
kernel/
  rocknix/            copiado de ROCKNIX (commit 61690fa8, ver PROCEDENCIA.md)
    dts/              rk3326-powkiddy-rgb20s.dts, rk3326-odroid-go.dtsi, rk3326.dtsi
    patches/          7 parches (dwc2, SD, símbolo que necesita el joypad)
    drivers/          panel-generic-dsi.c (driver de panel, built-in)
    linux.aarch64.conf  config base de RK3326
  dts/rk3326-powkiddy-rgb20s-alarm.dts   DTB propio: USB en modo periférico por defecto, sin hp-det (ADR 0017)
  dts/rk3326-powkiddy-rgb20s-alarm-hpdet.dts  variante experimental: jack por gpio-keys en GPIO2_C6
  config.fragment     ajustes sobre la config de ROCKNIX
uboot/
  rocknix/            defconfig, DT y parche de U-Boot de ROCKNIX; b_boot.ini de referencia
  boot.cmd            -> boot.scr: llama a sysboot sobre extlinux.conf
boot/
  boot.env            identificadores de disco y línea de comandos del kernel (normal y verbose)
  extlinux.conf.in    entradas "arch" (splash), "arch-verbose" (todos los mensajes) y "arch-rocknix" (DTB de ROCKNIX)
  rgb20s.conf         configuración editable desde la Mac
rootfs/
  customize.sh        personalización de ALARM (corre en el contenedor arm64)
  packages.txt        paquetes que se agregan
  overlay/            archivos que se copian a la rootfs (servicios rgb20s-*, red, SSH, journald)
splash/               splash de arranque: arte (make-splash.py), logo del kernel, rgb20s-splash (DRM),
                      instalación y rollback en una consola en uso
jack/                 diagnóstico del pin del jack (rgb20s-gpio-probe, jack-diagnose.sh) e instalación
                      solo del DTB, con rollback
```

## Usar la imagen

### Credenciales

| Usuario | Clave | Notas |
|---|---|---|
| `alarm` | `alarm` | grupos `wheel` (sudo), `audio`, `video`, `input`, `uucp`. **Cambiarla** con `passwd` |
| `root` | bloqueado | `sudo -i` desde `alarm`. El shell de emergencia de systemd entra sin clave (requiere teclado USB) |

SSH: solo `alarm`, solo con clave, **si** al armar la imagen existía `infra/image/local/authorized_keys`. Si no existía,
SSH acepta la contraseña. En cualquier caso se pueden agregar claves copiando un archivo `authorized_keys` a la raíz de la
partición FAT desde la Mac: se incorpora en el siguiente arranque.

### Conectar desde la Mac (gadget USB, sin verificar en la consola)

1. Cable USB-C de la consola a la Mac. En la Mac, `journalctl -k -f` debería mostrar `cdc_ncm` y `cdc_acm`.
2. La Mac recibe una IP 10.55.0.1x por DHCP. Entrar con `ssh alarm@10.55.0.1`.
3. Si la red no levanta, login serie: `screen /dev/ttyACM0 115200` (o `picocom`), usuario `alarm`.

### `/boot/rgb20s.conf` (se edita desde la Mac)

| Variable | Valores | Por defecto |
|---|---|---|
| `USB_ROLE` | `device` (gadget hacia la Mac) / `host` (tethering, adaptador USB-Ethernet, pendrive) | `device` |
| `USB_NET` | `ncm` / `ecm` / `rndis` | `ncm` |
| `USB_ADDRESS` | IP/prefijo de la consola en el gadget | `10.55.0.1/24` |
| `BOOT_LOGS` | `yes` / `no` | `yes` |
| `GROW_ROOT` | `yes` agranda la raíz al tamaño de la tarjeta (una vez) | `no` |

### Antes de usar pacman

La imagen no trae el llavero de pacman (tendría una clave privada compartida por todas las copias). Una vez, en la consola:
`sudo pacman-key --init && sudo pacman-key --populate archlinuxarm` (tarda). Para instalar paquetes hace falta internet:
`USB_ROLE=host` con tethering, o compartir la conexión de la Mac por la red del gadget (sin configurar todavía).

### Logs sin red

Cada arranque deja una carpeta en `/boot/logs/NNN-<boot_id>/` (FAT, se lee desde la Mac): `00-early.txt` (llegó a
userspace), `10-resumen.txt` (servicios, tiempos), `20-usb.txt` (rol y UDC), `30-hardware.txt` (pantalla, entrada, audio,
batería), `40-dmesg.txt`, `50-journal.txt` y `90-journal-final.txt` (al apagar). Además, el journal persistente está en la
ext4: `journalctl -D /run/media/$USER/RGB20S_ROOT/var/log/journal -b -1`.

## Grabar (lo hace el usuario, con confirmación)

```sh
lsblk -o NAME,SIZE,MODEL,TRAN,MOUNTPOINTS      # identificar la microSD (ej. /dev/sdX) y confirmar
sha256sum -c infra/build/out/rgb20s-arch.img.sha256
sudo dd if=infra/build/out/rgb20s-arch.img of=/dev/sdX bs=4M conv=fsync status=progress
```

`dd` pisa la tarjeta entera (sistema actual incluido): respaldarla antes.

## Fuentes

- Ver [`docs/firmware/arch-linux-arm.md`](../../docs/firmware/arch-linux-arm.md#fuentes) y [`kernel/rocknix/PROCEDENCIA.md`](kernel/rocknix/PROCEDENCIA.md).
