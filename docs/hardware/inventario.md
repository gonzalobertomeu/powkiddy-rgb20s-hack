# Inventario de la RGB20S

Última revisión: 2026-11-19 (todavía sin datos de la consola)

Mientras no haya red ni SSH, el inventario se hace con un script que se ejecuta desde el menú
**Ports** de ArkOS/dArkOS y deja el resultado en un `.txt` en la misma microSD.

- Script: [`infra/device/inventory.sh`](../../infra/device/inventory.sh)
- Resultados crudos: `docs/hardware/inventario/AAAA-MM-DD/` (uno por escenario)
- Resumen: este documento (plantilla al final)

## Qué hace el script

Junta todo el checklist de la skill `rgb20s-device-inventory`: sistema, `os-release`, kernel,
`/proc/cpuinfo`, frecuencias, temperatura, memoria, glibc, SDL2, ALSA (`aplay -l/-L`, `amixer`,
`/proc/asound`), input, pantalla, batería, almacenamiento, `lsmod`, `lsusb`, red, Wi-Fi y SSH.
Agrega una sección **USB GADGET** (UDC, DWC2, PHY, extcon, configfs, config del kernel, módulos,
`dr_mode` del device tree y restos de `adbd`/`usbdevice` de Rockchip), un **RESUMEN** con las
señales clave y dos anexos: `dmesg` completo y el config completo del kernel.

Es **solo de lectura**: no carga módulos, no monta configfs ni debugfs, no activa gadgets y no
toca servicios. Lo único que escribe es el `.txt` y unas líneas de progreso en la pantalla. Si
`sudo` funciona sin contraseña (en ArkOS/dArkOS lo usa el propio menú), lo usa solo para leer
`dmesg` y debugfs. Cada comando sale como encabezado (`----- $ comando`). Si un comando no existe,
se anota "comando no disponible"; si falla, se anota el código de salida. Las líneas
`[código de salida: 1]` que aparecen después de un `ls` o un `grep` vacíos solo indican "no hay nada", no un error del script.

Se probó en un contenedor Debian trixie mínimo, como root y como usuario normal (faltaban `lsusb`, `aplay`, `ps`, `free`, etc.): el script terminó en unos 9 s (8 de ellos son la pausa final) y generó el archivo completo.

## Paso a paso

1. **Montar la microSD en la Mac** con el lector. La partición que importa se llama
   `EASYROMS` (FAT32 o exFAT) y tiene una carpeta `ports/`. En Omarchy suele montarse en
   `/run/media/$USER/EASYROMS`. Si la consola usa **dos SD** y los juegos están en la segunda
   (TF2, montada como `/roms2`), el menú Ports podría leer `ports/` de esa tarjeta **(sin verificar)**:
   si después no aparece la entrada, copiar el script en la otra SD.
2. **Copiar el script** (sin espacios en el nombre, para no depender de cómo EmulationStation
   escapa la ruta):
   ```sh
   cp infra/device/inventory.sh /run/media/$USER/EASYROMS/ports/RGB20S-Inventario.sh
   sync && udisksctl unmount -b /dev/sdX1    # ajustar el dispositivo (lsblk)
   ```
   No hace falta `chmod`: en dArkOS la partición se monta con `umask=000`, así que todo es
   ejecutable. El script tiene finales de línea LF: no editarlo con un editor que los cambie a CRLF.
3. **Encender la consola** y entrar a **Ports**. Si la entrada `RGB20S-Inventario` no aparece,
   reiniciar EmulationStation (Start > Quit > Restart EmulationStation) o la consola **(sin verificar
   los nombres del menú)**. Si no existe el sistema Ports (no aparece porque no había ningún
   port), debería aparecer al haber al menos un `.sh` en `ports/` **(sin verificar)**.
4. **Ejecutarlo.** Tarda unos segundos (en la consola, quizás 20 a 60). Si la pantalla
   de texto está disponible, se ve el progreso por secciones y al final el nombre del archivo, con una
   pausa de 8 s. Si la pantalla queda negra, esperar un minuto: vuelve solo al menú.
5. **Apagar la consola desde el menú** (no cortar la energía, por la caché de escritura de FAT; el
   script ya hace `sync` al terminar), sacar la SD y montarla otra vez en la Mac.
6. **Buscar el resultado** en `EASYROMS/ports/rgb20s-inventory-AAAAMMDD-HHMMSS.txt`. Si la carpeta no
   era escribible, el script lo deja en el `$HOME` del usuario (`/home/ark`), y para leerlo hace falta
   montar la partición del sistema (ext4). La fecha del nombre puede estar mal, porque sin red el reloj
   no se sincroniza.
7. Copiar los `.txt` a `docs/hardware/inventario/AAAA-MM-DD/`. **Antes de commitear**, revisar que no
   haya datos que no quieras publicar (direcciones MAC en `ip link`, números de serie en `lsusb` o
   `/proc/cpuinfo`). El script no lee contraseñas, claves ni la configuración de redes Wi-Fi.

### Escenarios (correr el script una vez en cada uno)

Cada ejecución genera un archivo nuevo. Para saber si el gadget es viable sirven tres:

| Escenario | Qué conectar antes de ejecutar | Qué responde |
|---|---|---|
| **A** | Nada en el USB-C | Línea de base: UDC, config y device tree |
| **B** | El Android con *USB tethering* activo (o un pendrive) | **Qué controlador atiende el USB-C** cuando la consola es host |
| **C** | Cable USB-C a la Mac. En la Mac, `journalctl -k -f` abierto | Si el DWC2 detecta la conexión (VBUS, `bvalid`, extcon). En la Mac no debería aparecer nada, porque no hay gadget activo |

## Cómo leer el resultado: ¿es viable el gadget USB?

La sección **RESUMEN** del `.txt` junta las señales. Las direcciones son las del RK3326/PX30
(`px30.dtsi`): `ff300000.usb` es el controlador **DWC2 OTG**, el único que puede funcionar como gadget;
`ff340000.usb` y `ff350000.usb` son EHCI y OHCI, solo host.

| Señal | Favorable | Desfavorable |
|---|---|---|
| `/sys/class/udc/` | Contiene `ff300000.usb` | Vacío |
| Driver de `ff300000.usb` | `dwc2` | Vacío. El nodo no existe o el probe falló: buscar `dwc2` en `dmesg` |
| Escenario B: bus del teléfono | Cuelga de `ff300000.usb` (el USB-C está cableado al OTG) | Cuelga de `ff340000/ff350000` (el USB-C está cableado al puerto **host**; el gadget es imposible sin tocar el hardware) |
| Config del kernel | `USB_DWC2_DUAL_ROLE=y` o `_PERIPHERAL`, `USB_LIBCOMPOSITE`, `USB_CONFIGFS_ECM/NCM`, `USB_ETH`/`USB_G_NCM` en `y` o `m` | `USB_GADGET` no está, o DWC2 es `HOST` |
| Módulos en disco | Hay `libcomposite.ko`, `usb_f_ecm.ko`/`usb_f_ncm.ko`, `g_ether.ko` | No hay ninguno y tampoco están en `modules.builtin` |
| Device tree | `dr_mode = otg` o `peripheral` en `usb@ff300000`, con `otg-port` del PHY en `okay` | `dr_mode = host` u `otg-port` en `disabled` |
| `otg_mode` (PHY de Rockchip) | Existe y dice `otg` o `peripheral` | No existe o dice `host` |
| Escenario C | `dmesg` muestra eventos de `dwc2` o de extcon al enchufar | Nada |

Lo que ya se sabe **antes de correrlo**, según el código fuente de dArkOS (no medido en la consola):

- El kernel de dArkOS para RK3326 (`christianhaitian/linux`, rama `rg351`, `rg351p_tweaked_defconfig`)
  trae `USB_DWC2=m` con `USB_DWC2_DUAL_ROLE=y`, `USB_GADGET=y`, `USB_LIBCOMPOSITE=m`,
  `USB_CONFIGFS=m` con ECM, ECM_SUBSET, RNDIS y EEM (pero **sin `USB_CONFIGFS_NCM`**), `CONFIGFS_FS=y`
  y los gadgets heredados `USB_ETH=m` (`g_ether`, con RNDIS y EEM) y `USB_G_NCM=m`. **No** tiene
  `IKCONFIG`, así que no habrá `/proc/config.gz`; el build copia el config a `/boot/config-<versión>`.
  En software, el gadget **parece posible** con ECM (configfs) o NCM (`g_ncm`).
- El riesgo está en el device tree: en `rk3326-rg351mp-linux.dts` (el DTB que usa la imagen RG351MP), el
  `otg-port` del PHY está en `disabled` y solo el `host-port` está en `okay`. Si la RGB20S arranca con
  ese DTB (o uno parecido), el DWC2 no tiene PHY y el USB-C podría estar cableado al puerto host. El
  escenario B lo resuelve **(sin verificar)**.
- Qué imagen corresponde a la RGB20S: la wiki de ArkOS no la lista (aclara que la imagen de RGB10/RGB20
  "no funciona en la RGB20s"). La versión más probable es la imagen del RG351MP o una de la comunidad
  **(sin verificar)**. El script lo responde con `~/.config/.DEVICE`, `/proc/device-tree/model` y `ls /boot`.

## Convención de ports en ArkOS/dArkOS

| Punto | Estado |
|---|---|
| EmulationStation lista los `.sh`/`.SH` de `/roms/ports/` como sistema "Ports" | Confirmado en `es_systems.cfg.rk3326` de dArkOS y en `es_systems.cfg` de ArkOS (2020) |
| `/roms` es la partición `EASYROMS` (vfat, `umask=000`, uid 1000) | Confirmado en `bootstrap_rootfs.sh` de dArkOS. En ArkOS original, **(sin verificar)** el sistema de archivos (exFAT o FAT) |
| Comando de lanzamiento: `sudo perfmax %GOVERNOR% %ROM%; nice -n -19 %ROM%; sudo perfnorm` (dArkOS), `sudo perfmax; %ROM%; sudo perfnorm` (ArkOS) | Confirmado en el código. Consecuencia: durante el script el gobernador de CPU suele estar en `performance`, así que la frecuencia leída no es la de reposo |
| Corre como el usuario `ark`, no como root | Confirmado: `emulationstation.service` tiene `User=ark` (dArkOS) |
| `ark` tiene `sudo` sin contraseña | **(sin verificar)**: el menú usa `sudo perfmax` sin pedir clave, lo que sugiere que sí, al menos para eso. El script lo detecta con `sudo -n true` |
| Para mostrar texto se escribe en `/dev/tty0` o `/dev/tty1` | PortMaster hace `sudo chmod 666 /dev/tty0`. El script **no cambia permisos**: escribe directo si puede, o con `sudo tee`. Que el texto se vea mientras EmulationStation tiene la pantalla está **(sin verificar)** |
| Hay que reiniciar EmulationStation para que aparezca un port nuevo | **(sin verificar)** |
| Con dos SD, `ports/` puede estar en la segunda (`/roms2`) | **(sin verificar)**: dArkOS monta `/roms2`, pero no confirmé cuándo `es_systems` apunta ahí |

## Plantilla del resumen

Copiar este bloque arriba de "Fuentes" con los datos de cada inventario:

```markdown
## AAAA-MM-DD — <sistema y versión>
| Dato | Valor | Comando |
|---|---|---|
| Sistema | | `cat /etc/os-release`, `~/.config/.OS` |
| Equipo según el sistema | | `~/.config/.DEVICE`, `/proc/device-tree/model` |
| Kernel | | `uname -r` |
| glibc | | `ldd --version` |
| SDL2 | | `ls -l /usr/lib/*/libSDL2*`, `dpkg-query` |
| Códec de audio | | `aplay -l` |
| Chip Wi-Fi | | `dmesg` / `lsmod` / `/sys/bus/sdio/devices` |
| Driver de red USB (tethering) | | `lsmod`, `lsusb -t` (escenario B) |
| UDC (gadget USB) | sí / no | `ls /sys/class/udc` |
| Controlador del USB-C | `ff300000.usb` (OTG) / `ff340000.usb` (host) | RESUMEN, escenario B |
| `dr_mode` / `otg-port` | | `/proc/device-tree` |
| Config de gadget en el kernel | | `/boot/config-*` |
| SSH instalado y habilitado | | `systemctl is-enabled ssh` |
| Device tree compatible | | `/proc/device-tree/compatible` |

### Resuelve marcas (sin verificar)
- docs/hardware/rgb20s.md: "códec RK817" → confirmado / refutado (evidencia)
- docs/hardware/rgb20s.md: "driver de red USB `rndis_host` o `cdc_ether`" → ...
- docs/hardware/rgb20s.md: "modo gadget depende del kernel y del cableado" → ...
- blueprint.md: "Gadget (ECM/NCM) requiere UDC habilitado" → ...
```

## Fuentes

- [dArkOS: `Emulationstation/es_systems.cfg.rk3326` (sistema ports)](https://github.com/christianhaitian/dArkOS/blob/master/Emulationstation/es_systems.cfg.rk3326)
- [dArkOS: `Emulationstation/emulationstation.service` (`User=ark`)](https://github.com/christianhaitian/dArkOS/blob/master/Emulationstation/emulationstation.service)
- [dArkOS: `bootstrap_rootfs.sh` (fstab de EASYROMS)](https://github.com/christianhaitian/dArkOS/blob/master/bootstrap_rootfs.sh)
- [dArkOS: `build_kernel.sh` (kernel `rg351`, config en `/boot/config-*`)](https://github.com/christianhaitian/dArkOS/blob/master/build_kernel.sh)
- [dArkOS: herramientas (`dArkOS_Tools/`, ejemplo de scripts del menú)](https://github.com/christianhaitian/dArkOS/tree/master/dArkOS_Tools)
- [dArkOS releases (imágenes por equipo)](https://github.com/christianhaitian/dArkOS/releases)
- [Kernel `christianhaitian/linux` rama `rg351`: `rg351p_tweaked_defconfig`](https://github.com/christianhaitian/linux/blob/rg351/arch/arm64/configs/rg351p_tweaked_defconfig)
- [Kernel rama `rg351`: `rk3326-rg351mp-linux.dts`](https://github.com/christianhaitian/linux/blob/rg351/arch/arm64/boot/dts/rockchip/rk3326-rg351mp-linux.dts) y [`px30.dtsi`](https://github.com/christianhaitian/linux/blob/rg351/arch/arm64/boot/dts/rockchip/px30.dtsi)
- [ArkOS (archivado): `es_systems.cfg` de 2020](https://github.com/christianhaitian/arkos/blob/main/12162020/es_systems.cfg) y [wiki (imágenes por equipo)](https://github.com/christianhaitian/arkos/wiki)
- [PortMaster: `control.txt` (`ESUDO`, `/dev/tty0`)](https://github.com/PortsMaster/PortMaster-GUI/blob/main/PortMaster/control.txt)
