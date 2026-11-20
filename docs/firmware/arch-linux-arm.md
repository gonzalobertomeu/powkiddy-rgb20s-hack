# Arch Linux ARM en la RGB20S: cadena de arranque, kernel, rootfs e imagen

Última revisión: 2026-11-19 (fase 1: imagen armada, **nunca arrancada en la consola**)

Todo lo de este documento sale de fuentes (ROCKNIX, U-Boot, kernel, rkbin, Arch Linux ARM) y de builds en la Mac.
Nada se probó en la consola: los datos de hardware son **(sin verificar)** salvo que se diga otra cosa.
Cómo se construye: [`projects/linux-image/README.md`](../../projects/linux-image/README.md).
Decisiones: ADR [0009](../decisions/0009-arranque-uboot-miniloader-rkbin.md) (arranque),
[0010](../decisions/0010-kernel-mainline-con-dts-y-drivers-de-rocknix.md) (kernel),
[0011](../decisions/0011-rootfs-alarm-con-systemd.md) (rootfs),
[0012](../decisions/0012-acceso-por-usb-gadget-ncm-acm.md) (acceso por USB).

## Resumen

| Pieza | Qué se usa | De dónde | Estado |
|---|---|---|---|
| Init de DDR | `rk3326_ddr_333MHz_v2.12.bin` | rkbin `3e288fe8` (blob) | Empaquetado |
| Primer cargador | `rk3326_miniloader_v1.40.bin` | rkbin (blob) | Empaquetado |
| BL31 | `rk3326_bl31_v1.37.elf` → `trust.img` | rkbin (blob + `trust_merger`) | Empaquetado |
| U-Boot | v2025.10 + `rk3326-handheld_defconfig` de ROCKNIX | u-boot.org + ROCKNIX `61690fa8` | **Compila** |
| Script de arranque | `boot.scr` → `sysboot` → `extlinux/extlinux.conf` | propio | Generado |
| Kernel | Linux 7.1.2 + 7 parches de ROCKNIX + `panel-generic-dsi` | kernel.org + ROCKNIX | **Compila** (`7.1.2-rgb20s`, Image de 21 MB) |
| Device tree | `rk3326-powkiddy-rgb20s.dts` de ROCKNIX + variante propia | ROCKNIX + propio | **Compila** (2 DTB) |
| Joypad | `rocknix-singleadc-joypad.ko` (módulo externo, cargado por `modules-load.d`) | ROCKNIX/rocknix-joypad `ac1be0fa` | **Compila** |
| Rootfs | `ArchLinuxARM-aarch64-latest.tar.gz` (2026-08-05) actualizada y adelgazada | archlinuxarm.org (md5 + firma GPG verificados) | **Armada** (544 MiB) |
| Imagen | `infra/build/out/rgb20s-arch.img` (MBR, 1235 MiB) | `infra/image/scripts/build-image.sh` | **Armada**; `check-image.sh`: 41/41 |

## Cadena de arranque

```
BootROM del RK3326 ─ lee la SD (TF1) en el sector 64
 └─ idbloader.img: blob DDR (inicializa la DDR3L) + miniloader de Rockchip         sector 64
    └─ miniloader carga trust.img (BL31, TF-A de Rockchip)                         sector 24576 (12 MiB)
       y uboot.img (U-Boot mainline v2025.10, como BL33)                           sector 16384 (8 MiB)
       └─ U-Boot: bootmeth order script; bootflow scan -b
          └─ /boot.scr de la partición 1 (FAT32, marcada booteable)
             └─ sysboot → /extlinux/extlinux.conf
                └─ /Image + /dtbs/rockchip/rk3326-powkiddy-rgb20s-alarm.dtb, sin initramfs
                   └─ systemd en la partición 2 (ext4, root=PARTUUID=52474253-02)
```

### Offsets (verificados en documentación, no en la consola)

| Zona | Sector (512 B) | Bytes | Fuente |
|---|---|---|---|
| MBR | 0 | 0 | — |
| Entorno de U-Boot (no se usa) | 32 a 63 | 16 KiB | `CONFIG_ENV_OFFSET=0x4000`, `CONFIG_ENV_SIZE=0x4000` del defconfig |
| `idbloader.img` | 64 | 32 KiB | `rockchip.rst` ("sudo dd if=idbloader.img of=/dev/sda seek=64") y `update.sh` de ROCKNIX (escribe en el byte 32 KiB) |
| `uboot.img` | 16384 | 8 MiB | `rockchip.rst` ("seek=16384") y `rkhelper` (16320 dentro de `uboot.bin`) |
| `trust.img` | 24576 | 12 MiB | `rockchip.rst` ("seek=24576") y `rkhelper` (24512 dentro de `uboot.bin`) |
| Partición 1 | 32768 | 16 MiB | `uboot.bin` termina exactamente ahí (medido: 16 744 448 bytes desde el sector 64) |

Por qué **MBR** y no GPT: la tabla GPT ocupa los sectores 2 a 33 y se pisaría con el entorno de U-Boot (32 a 63).
ROCKNIX también usa MBR.

### U-Boot

- **Mainline** tiene la Odroid Go Advance: `configs/odroid-go2_defconfig` (no `odroid-go2-rk3326_defconfig`) con TPL/SPL de
  U-Boot, flash SPI y `OF_LIST` para OGA, OGA v1.1 y OGS. La RGB20S no tiene flash SPI ni está en esa lista.
- **ROCKNIX** (imagen "b", donde está la RGB20S) usa U-Boot v2025.10 con `TARGET_ODROID_GO2`, su propio defconfig
  `rk3326-handheld_defconfig` y un DT genérico `rk3326-common-handheld.dts`, pero **no** usa el TPL/SPL de U-Boot: el
  primer paso es el miniloader de rkbin ("mkimage does not support packing second level bootloader on rk3326", comentario de `rkhelper`).
- La imagen replica ROCKNIX ([ADR 0009](../decisions/0009-arranque-uboot-miniloader-rkbin.md)). Consola de U-Boot en UART2 a
  1 500 000 baudios; los clones K36 usan UART5 (ROCKNIX tiene un `uboot.bin.uart5` aparte; no aplica a la RGB20S).
- El `bootcmd` solo ejecuta scripts: sin `boot.scr`, U-Boot no lee `extlinux.conf`. El `boot.scr` propio es una versión
  mínima del `b_boot.ini` de ROCKNIX, sin la detección del modelo por ADC (en ROCKNIX la RGB20S cae en el caso por defecto
  y hay que renombrar `extlinux.conf.rgb20s` a mano).
- U-Boot **no enciende la pantalla** (no hay driver DSI en este U-Boot): hasta que arranca el kernel, la pantalla queda negra.

## Kernel

- **Versión:** 7.1.2, la misma que ROCKNIX usa para RK3326 (sha256 verificado contra ROCKNIX y contra `sha256sums.asc` de
  kernel.org; la firma de ese archivo no se verificó).
- **Config:** la de ROCKNIX para RK3326 (`linux.aarch64.conf`) más `config.fragment`. Ya trae dwc2 dual-role, configfs con
  NCM/ECM/RNDIS, role switch, DRM Rockchip (VOP + DSI), Panfrost (`=m`), RK817 (PMIC, códec, cargador), SARADC, `gpio-keys`,
  `IKCONFIG_PROC`, `PSTORE_RAM`. El fragmento agrega ACM, MIDI y HID al gadget, `PANIC_TIMEOUT=0` y `LOCALVERSION=-rgb20s`.
- **Parches de ROCKNIX aplicados (7):** cinco de dwc2 (`003`, `004`, `010`, `035`, `038`: NULL deref al restaurar
  registros, PM tras un reset fallido, estado de `vbus-supply`, liberar VBUS al cambiar de rol, sin pull-up al cambiar ID
  sin gadget), `0008` (reintento de resume de la SD) y `mainline/0004` de `packages/linux/patches` (exporta `joypad_input_g`
  desde `adc-keys`: sin él `rocknix-singleadc-joypad.ko` no linkea; se comprobó).
- **No aplicados:** `030-pwm-vibra` (para la GKD Pixel 2; necesita `linux/rockchip/cpu.h`, que agrega un parche de PX30S:
  se probó y no compila sin él), `004-input-drivers` (joypads de OGA/Chi/XU10; la RGB20S usa otro driver), `008-esp-8089-wifi`
  (la RGB20S usa el controlador SDIO para la segunda SD), los de PX30S (otro silicio), `001-panel-updates` (paneles de otras
  consolas), `036/037` (devfreq de DMC, deshabilitado en el DTS) y el directorio `default` (DVB de Amlogic, i915).
- **Drivers fuera de mainline:** `panel-generic-dsi.c` se compila built-in (`obj-y`, igual que ROCKNIX); el joypad
  `rocknix-singleadc-joypad` se compila como módulo externo contra el árbol del kernel (con `DEVICE=RK3326`: con la variable
  vacía su Makefile solo compila `rocknix-joypad.ko`). No declara tabla de compatibles, así que udev no lo carga solo:
  lo carga `/etc/modules-load.d/rgb20s.conf`. Panfrost sí se carga solo (alias `arm,mali-bifrost`).

### Lo que dice el DTS de ROCKNIX sobre el hardware (sin verificar)

| Bloque | Según `rk3326-powkiddy-rgb20s.dts` + `rk3326-odroid-go.dtsi` de ROCKNIX |
|---|---|
| Base | Hereda de la Odroid Go Advance (`rk3326-odroid-go.dtsi`); compatible `powkiddy,rgb20s` |
| Pantalla | DSI de 4 carriles, 640×480, RGB888, panel con comandos tipo NV3051D (`ff30 52 0x`), driver `rocknix,generic-dsi`; 10 modos (el por defecto, 26,4 MHz). Alimentación `vcc_lcd` (LDO8 del RK817), backlight PWM1 (`vcc_bl`, LDO7) |
| Controles | `rocknix-singleadc-joypad`: 4 ejes por un multiplexor analógico sobre el canal 1 del SARADC (selección GPIO3 PB3/PB0, habilitación PB5), 17 botones GPIO, ejes X/Y invertidos. Volumen +/− con `gpio-keys` (GPIO2 PA0/PA1). Vibración por PWM0 |
| Audio | Códec del RK817 por I2S1 (`simple-audio-card` "rk817_int"): parlante (SPKO), auriculares (HPOL/HPOR) con detección en GPIO2 PC6, micrófono |
| Energía | PMIC RK817 en I2C0 (0x20); batería 3500 mAh; cargador sin resistencia de sensado (`resistor-sense-micro-ohms = <0>`) |
| microSD | TF1 en `sdmmc` (arranque); **TF2 en el controlador `sdio`** (detección GPIO3 PB6). Alias `mmc0 = sdio`, `mmc1 = sdmmc`: **la tarjeta de arranque es `mmcblk1`** y la de datos `mmcblk0`. Por eso la imagen monta por PARTUUID |
| USB | `u2phy`: `host-port` okay, `otg-port` **disabled**; `usb20_otg` (dwc2) okay con `usb-role-switch` (por defecto host). Se borra `vcc_host` (en la OGA alimenta el puerto host) |
| Otros | Un solo LED (se borran los de la OGA), sin flash SPI (`sfc` deshabilitado), sin Wi-Fi en el DTS |

El DTB propio (`rk3326-powkiddy-rgb20s-alarm.dtb`) incluye el de ROCKNIX y solo cambia `role-switch-default-mode` a
`"peripheral"` ([ADR 0012](../decisions/0012-acceso-por-usb-gadget-ncm-acm.md)). El DTB de ROCKNIX sin cambios también va en la
imagen, como segunda entrada de `extlinux.conf`.

### USB gadget: ¿es posible?

En software sí: dwc2 dual-role, configfs y el role switch que fuerza la sesión B por registros (ver ADR 0012). Lo que no se
sabe es el **cableado**: si el USB-C va al puerto OTG del PHY (dwc2, `ff300000`) el gadget debería andar; si va al puerto host
(EHCI/OHCI, `ff340000`/`ff350000`), no hay gadget posible. Indicios a favor del OTG: ROCKNIX usa dwc2 como host en el USB-C de
clones de la misma familia (RGB10X, R36S) con el `otg-port` deshabilitado igual que la RGB20S. Indicio en contra: en la OGA el
puerto host del PHY sí está cableado a un conector, y la RGB20S hereda ese dtsi. **(sin verificar)**

## Rootfs

- Tarball genérico `ArchLinuxARM-aarch64-latest.tar.gz` (Last-Modified 2026-08-05, sha256 `42a4eeaa…b319`), verificado con el md5
  publicado y la firma GPG de "Arch Linux ARM Build System" (`68B3537F39A313B3E574D06777193F152BDBE6A6`).
- Trae 165 paquetes, entre ellos `linux-aarch64`, 11 paquetes de `linux-firmware`, `mkinitcpio`, `netctl`, `dhcpcd` y `vim`, y
  habilita `systemd-networkd-wait-online`. La personalización ([`customize.sh`](../../projects/linux-image/rootfs/customize.sh))
  los saca o deshabilita, actualiza con `pacman -Syu` y agrega `alsa-utils`, `usbutils`, `evtest`, `dosfstools` y `sudo`.
- Usuario `alarm` / clave `alarm` (grupos `wheel`, `audio`, `video`, `input`, `uucp`); root bloqueado; shell de emergencia sin
  contraseña (`SYSTEMD_SULOGIN_FORCE=1`). SSH solo con clave si al armar había `infra/image/local/authorized_keys`; si no, con contraseña.
- Servicios propios: `rgb20s-usb` (rol y gadget), `rgb20s-firstboot` (claves desde `/boot/authorized_keys`, agrandar la raíz),
  `rgb20s-bootlog-early` y `rgb20s-bootlog` (diagnóstico a `/boot/logs/`).

## Imagen

```
sector 0       MBR, id 0x52474253
sector 64      uboot.bin (idbloader + uboot.img + trust.img), hasta el sector 32767
p1  32768      FAT32 "RGB20S_BOOT", 256 MiB, booteable    PARTUUID 52474253-01
               Image, dtbs/rockchip/*.dtb, extlinux/extlinux.conf, boot.scr, rgb20s.conf, logs/, LEEME.txt
p2  ...        ext4 "RGB20S_ROOT" (tamaño justo + 30 %)    PARTUUID 52474253-02
```

Línea de comandos del kernel:
`root=PARTUUID=52474253-02 rootfstype=ext4 rootwait rw console=ttyS2,1500000n8 console=tty0 loglevel=7 consoleblank=0 panic=0 systemd.show_status=1`
(todos los mensajes en pantalla, sin apagarla, y un panic queda visible en vez de reiniciar).

## Diagnóstico sin consola serie

La Mac corre Linux: puede leer las dos particiones de la SD (FAT y ext4) con el lector, sin `sudo` (`udisksctl mount`).

| Qué se ve | Dónde | Qué indica |
|---|---|---|
| Nada en pantalla | — | Falla antes del kernel (U-Boot o anterior) **o** el panel no inicializa. Ver riesgos 1 a 4 |
| Texto del kernel en pantalla | pantalla | U-Boot, kernel, DTB y panel funcionan |
| Prompt de login en tty1 con la IP de `usb0` | pantalla (`/etc/issue`) | Arranque completo |
| `/boot/logs/NNN-*/00-early.txt` | FAT | El kernel montó la raíz y systemd llegó a `sysinit` |
| `/boot/logs/NNN-*/10-resumen.txt`, `20-usb.txt`, `30-hardware.txt`, `40-dmesg.txt`, `50-journal.txt` | FAT | Arranque completo: tiempos, servicios fallidos, UDC y rol USB, pantalla, entrada, audio, batería |
| `/boot/logs/NNN-*/90-journal-final.txt` | FAT | Apagado ordenado |
| `/var/log/journal/` | ext4 | Journal persistente de todos los arranques: `journalctl -D <montaje>/var/log/journal -b -1` en la Mac |
| `/dev/ttyACM0` en la Mac | USB | Gadget funcionando: login serie aunque falle la red |
| Interfaz nueva en la Mac (`ip link`), IP 10.55.0.1x por DHCP | USB | Red del gadget funcionando: `ssh alarm@10.55.0.1` |

En la Mac, antes de enchufar: `journalctl -k -f` muestra si aparece un dispositivo USB (`cdc_ncm`, `cdc_acm`).

## Riesgos del primer arranque

| # | Riesgo | Síntoma | Cómo diagnosticarlo sin serie ni red | Qué probar |
|---|---|---|---|---|
| 1 | El BootROM no acepta el `idbloader` (offset, formato) | Pantalla negra, no hay `logs/` en la FAT | No hay forma directa. Comparar los primeros 16 MiB de la imagen con los de la SD respaldada (ArkOS/dArkOS) y con una imagen de ROCKNIX | Grabar una imagen de ROCKNIX para la RGB20S: si arranca, el problema está en nuestro `uboot.bin`. Alternativa: copiar los sectores 64 a 32767 de ROCKNIX |
| 2 | DDR mal inicializada (la RGB20S tiene DDR3L, el blob detecta el tipo) | Pantalla negra o cuelgues al azar | Igual que 1 | Probar el `uboot.bin` de una imagen de ROCKNIX "b" |
| 3 | U-Boot no ejecuta `boot.scr` (partición no booteable, nombre, `bootmeth`) o `sysboot` no encuentra el `extlinux.conf` | Pantalla negra, sin `logs/` | Con serie sería obvio. Sin serie: mirar que `boot.scr` y `extlinux/extlinux.conf` estén en la raíz de la FAT y sin espacios al final de las líneas | Probar con el `extlinux.conf` mínimo de ROCKNIX (`FDTDIR /`) o con un `boot.scr` que haga `load`/`booti` directo |
| 4 | El panel no inicializa (driver `generic-dsi`, secuencia o modo) | Pantalla negra o backlight encendido sin imagen, **pero** aparecen `logs/` en la FAT | `40-dmesg.txt`: buscar `panel`, `dsi`, `vop`, `drm`. `30-hardware.txt`: estado de `/sys/class/drm/card*-DSI-1` | Cambiar `default=1` a otro modo en el DTS; probar el DTB de ROCKNIX (entrada `arch-rocknix`) |
| 5 | El kernel no monta la raíz (PARTUUID, MMC, ext4) | Pantalla con `VFS: Unable to mount root fs` y se queda quieta (`panic=0`) | Lo dice la pantalla. Revisar `root=` en `extlinux.conf` y que la SD de datos (TF2) no tenga otra partición `52474253-02` | Probar `root=/dev/mmcblk1p2` (la TF1 es `mmcblk1` según los alias del DTS) |
| 6 | systemd se cuelga o cae en modo emergencia | Pantalla con mensajes de systemd o "emergency mode" | `00-early.txt` existe pero no `10-resumen.txt`; journal en la ext4 desde la Mac | Leer el journal con `journalctl -D`; `fstab` (la FAT es `nofail`) |
| 7 | El USB-C está cableado al puerto host del PHY | No aparece nada en la Mac (`journalctl -k -f`) | `20-usb.txt`: `ff300000.usb` en `/sys/class/udc` con estado `not attached` aunque haya cable; rol `device` | `USB_ROLE=host` en `rgb20s.conf` y tethering/adaptador. Si con host el teléfono aparece bajo `ff340000`/`ff350000`, el gadget es imposible |
| 8 | dwc2 en modo periférico no detecta la conexión (PHY OTG apagado al estar `disabled`) | Igual que 7 | `20-usb.txt` y `dmesg \| grep dwc2` | DTS con `&u2phy_otg { status = "okay"; }` (como la GameForce Chi) |
| 9 | La Mac no configura la red | Aparece la interfaz pero sin IP | En la Mac: `ip addr`, `journalctl -u NetworkManager` | Poner `10.55.0.2/24` a mano; probar `USB_NET=ecm`; entrar por `/dev/ttyACM0` (`screen /dev/ttyACM0 115200`, usuario `alarm`) |
| 10 | SSH rechaza la conexión | `Permission denied (publickey)` | `50-journal.txt` (sshd) | Copiar la clave pública a `/boot/authorized_keys` desde la Mac y reiniciar |
| 11 | El joypad no carga (módulo externo) | Botones sin respuesta | `30-hardware.txt` (`/proc/bus/input/devices`), `lsmod` | `modprobe rocknix-singleadc-joypad`, `evtest` |
| 12 | Sin audio | `aplay -l` vacío | `30-hardware.txt` | `dmesg \| grep -i rk817`; `amixer` (el parlante puede estar muteado por defecto) |
| 13 | La batería o el cargador del RK817 fallan con `resistor-sense = 0` | Sin `power_supply` o valores absurdos | `30-hardware.txt` | Comparar con los parches `mainline-rockchip` de ROCKNIX (no aplicados) |

Regla general: **siempre** tener a mano una forma de volver al sistema anterior (el respaldo de la SD que coordina la sesión
principal, o una imagen de ROCKNIX/dArkOS) para separar "falla del hardware" de "falla de la imagen".

## Correcciones a las skills del proyecto

Lo que las skills `rockchip-rk3326-boot` y `alarm-rootfs` dicen de memoria, contrastado con las fuentes:

| Skill dice | Fuente dice |
|---|---|
| Defconfig `odroid-go2-rk3326_defconfig` | Se llama `odroid-go2_defconfig` (U-Boot v2026.10) |
| `u-boot-rockchip.bin` en el sector 64 con TPL de rkbin | Válido para el flujo TPL/SPL; ROCKNIX (y esta imagen) usan el flujo miniloader: `idbloader` 64, `uboot.img` 16384, `trust.img` 24576 |
| Primera partición en 16 MiB o más | Correcto (y con el flujo miniloader el bootloader llega justo hasta 16 MiB) |
| GPT o MBR | Solo MBR con este U-Boot: el entorno en `0x4000` choca con la GPT |
| `extlinux.conf` se lee solo | Con el defconfig de ROCKNIX hace falta `boot.scr` |
| `console=ttyS2,115200` | ROCKNIX usa `console=ttyS2,1500000` |
| `root=PARTUUID=…` | Correcto, y además necesario: la SD de arranque es `mmcblk1` por los alias del DTS |
| Joypad con `adc-joystick` / `gpio-keys` | El DTS de ROCKNIX usa `rocknix-singleadc-joypad` (fuera de mainline); `gpio-keys` solo para volumen |
| Opciones del kernel `SND_SOC_RK817`, `MFD_RK8XX_I2C`, `ROCKCHIP_SARADC`, `PHY_ROCKCHIP_INNO_USB2`, `USB_CONFIGFS_*` | Existen con esos nombres (config de ROCKNIX 7.1.2) |
| `USB_CONFIGFS_ECM` | Existe; para red también están `USB_CONFIGFS_NCM` y `USB_CONFIGFS_RNDIS` |
| Extraer la rootfs con `sudo bsdtar` y `arch-chroot` | Se puede sin `sudo`: `docker import` + contenedor arm64 + `mke2fs -d` |
| `pacman -Rns linux-aarch64` y quitar firmware | Correcto; además conviene `--disable-sandbox` en pacman 7.1 bajo qemu-user y quitar `CheckSpace` en contenedores |

## Pendientes

- [ ] Respaldar la partición de arranque actual (sesión principal) y comparar su bootloader con `uboot.bin`
- [ ] Primer arranque y lectura de `/boot/logs/`
- [x] Confirmar el cableado del USB-C (riesgos 7 y 8): era el **8**. Con `&u2phy_otg { status = "okay"; }` el gadget enumera en la Mac, pero solo con cable USB-A a USB-C (con C a C la Mac no entrega VBUS) (2026-11-20)
- [ ] Confirmar pantalla, joypad, audio, batería y Panfrost
- [ ] Medir el tiempo de arranque y adelgazar la config del kernel
- [ ] Servicio del groovebox en KMS (requiere `mesa`, `sdl2` y un build contra Arch)

## Fuentes

- [U-Boot `doc/board/rockchip/rockchip.rst` (v2026.10)](https://github.com/u-boot/u-boot/blob/v2026.10/doc/board/rockchip/rockchip.rst)
- [U-Boot `configs/odroid-go2_defconfig` (v2026.10)](https://github.com/u-boot/u-boot/blob/v2026.10/configs/odroid-go2_defconfig)
- [ROCKNIX `distribution`, commit 61690fa8](https://github.com/ROCKNIX/distribution/tree/61690fa8042acf631922deb2c80e8574e96958d2): `projects/ROCKNIX/devices/RK3326/` (DTS, parches, `linux.aarch64.conf`, `packages/u-boot`, `bootloader/update.sh`, `options`), `projects/ROCKNIX/bootloader/rkhelper`, `projects/ROCKNIX/packages/linux/package.mk`, `projects/ROCKNIX/packages/linux-drivers/`, `projects/ROCKNIX/config.xml`
- [ROCKNIX/rocknix-joypad, commit ac1be0fa](https://github.com/ROCKNIX/rocknix-joypad/tree/ac1be0fa0c4e1f7feb9a900fc428d7ac6014029c)
- [rkbin, commit 3e288fe8](https://github.com/rockchip-linux/rkbin/tree/3e288fe814e059dd06833495f845cab04ac20a5c)
- [kernel.org: linux-7.1.2 y `sha256sums.asc`](https://cdn.kernel.org/pub/linux/kernel/v7.x/)
- Linux 7.1.2: `drivers/usb/dwc2/drd.c`, `drivers/usb/gadget/function/u_ether.c`, `arch/arm64/boot/dts/rockchip/px30.dtsi`, `rk3326-gameforce-chi.dts`, `rk3326-odroid-go.dtsi`, `rk3326-anbernic-rg351m.dtsi`
- [Arch Linux ARM: firma de paquetes](https://archlinuxarm.org/about/package-signing) y [tarball aarch64](http://os.archlinuxarm.org/os/)
- [Linux: USB gadget configfs](https://docs.kernel.org/usb/gadget_configfs.html)
- Builds propios en la Mac (2026-11-19): ver [`projects/linux-image/README.md`](../../projects/linux-image/README.md)
