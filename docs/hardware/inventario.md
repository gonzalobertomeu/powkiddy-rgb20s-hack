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

## Audio, volumen y USB host — 2026-11-20

Sistema: Arch Linux ARM propio, kernel 7.1.2-rgb20s (aarch64), acceso por SSH (`rgb20s`, 10.55.0.1). Solo lectura; único cambio temporal: `Master Playback Volume` 201 → 180 → 201 (restaurado y verificado).

### Audio (tarjeta 0 `rk817_int`, simple-card, I2S ff070000 + códec RK817)
| Dato | Valor | Comando |
|---|---|---|
| PCM | un solo dispositivo `hw:0,0` (playback y captura); sin dispositivos USB | `aplay -l` |
| Controles | `Headphones Jack` (bool, solo lectura, hoy `on`), `Master Playback Volume` (0-255, -95..0 dB, hoy 201,201), `Master Capture Volume` (0-255, hoy 255), `Mic Capture Gain` (0-15, -18..27 dB, hoy 6), `Playback Mux` (enum `HP`/`SPK`, hoy 0 = HP) | `amixer -c 0 contents` |
| Mixer simple | `Master`, `Mic Capture Gain`, `Playback Mux` | `amixer -c 0 scontrols` |
| Software | alsa-lib, alsa-utils, alsa-ucm-conf, evtest, usbutils, libusb, hidapi. Sin pipewire ni pulseaudio. Sin `/etc/asound.conf` | `pacman -Q` |
| UCM | existe `/usr/share/alsa/ucm2` (con `Rockchip/` y `USB-Audio/`) | `ls` |
| Jack | evdev `event1` "rk817_int Headphones" (SW=4, `SW_HEADPHONE_INSERT`); `evtest --query` devolvió 10 (switch activo). El kernel anuncia `Headphones Jack = on`; no se verificó si hay un conector enchufado (sin verificar) | `evtest --query /dev/input/event1 EV_SW SW_HEADPHONE_INSERT` |
| dmesg | `input: rk817_int Headphones`, `#0: rk817_int`, aviso "using zero-initialized flat cache" del i2s | `dmesg \| grep` |

- Volumen: el control es solo `Master Playback Volume` (único para parlante y jack; el ruteo lo elige `Playback Mux` HP/SPK). `alsa-restore.service` activo. Con la app `groovebox` reproduciendo, `amixer cset` funciona (escribe y relee). El efecto audible no se verificó: no hay oído en el relevamiento y `speaker-test` chocaría con la app que tiene el PCM abierto (pid 610).
- El volumen es en pasos de 1/255 (rango dB lineal de -95 a 0): para botones conviene pasos de ~8-10.

### Botones de volumen
| Dato | Valor |
|---|---|
| Dispositivo | `gpio-keys-vol` (`/dev/input/event2`, root:input 0660; `alarm` está en el grupo input o accede vía ACL (sin verificar)) |
| Keycodes | máscara `KEY=c000000000000 0` = bits 114 y 115 = `KEY_VOLUMEDOWN` (114) y `KEY_VOLUMEUP` (115) (deducido de la máscara; no se pulsaron los botones) |
| Otros evdev | event0 `rk805 pwrkey` (KEY 116, power), event1 jack, event3 `r36s_Gamepad` (joypad, js0) |
| Consumidores | ningún proceso mantiene abierto `/dev/input/event*` según `/proc/*/fd` (corrido como `alarm`: solo ve sus procesos, sin verificar para root). No hay `rgb20s-session`/udev/logind que maneje teclas de volumen; `systemd-logind` suele tomar teclas de volumen solo si las abre (no aparece) |
| Herramientas | `evtest` 1.36 instalado; `libinput` no |

### USB
| Dato | Valor | Comando |
|---|---|---|
| Rol actual | `device` (`/sys/class/usb_role/ff300000.usb-role-switch/role`) | `cat` |
| UDC | `ff300000.usb`, enlazado al gadget `rgb20s` (NCM + ACM) | `ls /sys/class/udc` |
| Controlador | `usb@ff300000` dwc2 (`rockchip,px30-usb`, okay), `USB_DWC2_DUAL_ROLE=y`, `USB_ROLE_SWITCH=y`. `usb@ff340000` (ehci) y `usb@ff350000` (ohci) `disabled` en el DT; "Requested PHY is disabled" (host-port) | DT y dmesg |
| Host | bus usb1 solo con el root hub dwc2 (`lsusb`: 1d6b:0002); sin dispositivos | `lsusb` |
| Kernel | `SND_USB_AUDIO=m`, `SND_HWDEP=m`, `HID=y`, `HID_GENERIC=y`, `USB_HID=y`, `USB_HIDDEV=y`, `USB_EHCI/OHCI_HCD=y`, `SND_USB_AUDIO_MIDI_V2` no, `USB_DWC3` no | `zcat /proc/config.gz` |
| Módulos | `/lib/modules/7.1.2-rgb20s/kernel/sound/usb/{snd-usb-audio,snd-usbmidi-lib}.ko`; `modinfo` OK; no cargado hoy (`lsmod`) | `ls`, `modinfo` |
| Cargador | `rk817-charger` online=0, batería 100 % | `power_supply/*/uevent` |
| Gestión del rol | `/usr/local/lib/rgb20s/usb.sh` escribe `device|host` en el role switch (`USB_ROLE` en `/boot/rgb20s.conf`) | |

### Prueba con el usuario presente (2026-11-20, reloj de la consola 15:27-15:31; sin RTC, la fecha de la consola dice 10-nov)
| Dato | Resultado |
|---|---|
| Botones (`event2`) | funcionan: 70 pulsaciones (`KEY_VOLUMEUP` 115 x52, `KEY_VOLUMEDOWN` 114 x18), cada una `value 1` y luego `value 0`, entre 15:27:52 y 15:29:12 |
| Duración de cada pulsación | 0,03-0,20 s; no hubo ninguna pulsación larga (la "mantenida 2 s" no se registró), así que **no se midió** si hay repetición (`value 2`); 0 eventos `value 2` |
| Jack (`event1`) | estado inicial `SW_HEADPHONE_INSERT` = 1; **0 eventos** en 180 s aunque el usuario debía enchufar/desenchufar (si lo hizo: el switch no cambia; si no tenía auriculares: sin verificar) |
| Mezclador | `Master Capture Volume` se mueve junto con `Master` al usar `amixer sset Master` (quedó en 220 y se restauró a 255) |
| Sonido (`speaker-test -D plughw:0,0 -c 2 -t wav -l 2`) | corrió sin error (rc 0) con la app sin PCM abierto: SPK 15:31:07-15:31:13; HP 15:31:18-15:31:24; 48000 Hz S16_LE, period 12000, buffer 48000, Master 60 % (raw 220, -13 dB) |
| Qué se oyó | pendiente de confirmar por el usuario |
| Mezclador final | restaurado: Master 201,201, Capture 255,255, Playback Mux HP; verificado contra `amixer -c 0 contents` previo |

### Parlante, auriculares y jack: conclusiones (2026-11-20, con el usuario escuchando)

- **El parlante integrado está cableado al canal izquierdo de la salida de auriculares.** Con `Playback Mux` en HP y
  sin auriculares, `speaker-test -t wav` suena por el parlante pero solo "Front Left"; con tono de 440 Hz también suena.
- **Con `Playback Mux` en SPK no suena nada**, ni al volumen 240/255: la salida de clase D del RK817 no está conectada
  a nada en esta placa (sin verificar en el esquemático). Hay que dejar el Mux siempre en **HP**.
- **El jack corta el parlante de forma física**: con auriculares enchufados todo sale por los auriculares, en estéreo.
- **La detección del jack no funciona**: `SW_HEADPHONE_INSERT` queda en 1 con y sin auriculares, y no hubo ningún
  evento en dos capturas (180 s y 90 s) enchufando y desenchufando. GPIO o polaridad incorrectos en el DTS, o el jack
  no tiene pin de detección (sin verificar).
- **Botones de volumen**: mantenidos, el kernel repite (`value 2`, 30 repeticiones en una pulsación de ~2 s).
- Consecuencia: por el parlante, el canal derecho se pierde. Sin detección del jack, la app no puede saber si conviene
  mezclar a mono (parlante) o dejar estéreo (auriculares).
- Ojo: `amixer sset Master` también cambia `Master Capture Volume`; para la salida usar
  `cset name='Master Playback Volume'`.

### Jack de auriculares: diagnóstico del pin de detección

Investigación del 2026-11-20 (sin consola; solo fuentes y el kernel del repo). Herramientas y procedimiento:
[`projects/linux-image/jack/`](../../projects/linux-image/jack/README.md). Decisión: [ADR 0017](../decisions/0017-sin-deteccion-de-jack-en-simple-audio-card.md) (propuesta).

| Fuente | Pin y polaridad | Pull | Qué dice de la RGB20S |
|---|---|---|---|
| ROCKNIX `rk3326-odroid-go.dtsi` (base del DTS de la RGB20S) y `rk3326-gameconsole-r3xs.dtsi` (R36S) | GPIO2_C6, `GPIO_ACTIVE_HIGH` en `simple-audio-card` | define el grupo `hp_det` (pull-down) **pero ningún nodo lo aplica** (`rk817-sound` no tiene `pinctrl-0`) | el pin queda con el pull de reset/U-Boot (sin verificar cuál) |
| ROCKNIX `headphone_sense` | trata `SW_HEADPHONE_INSERT` = 0 como "auriculares" | — | los quirks de "Powkiddy RGB20S" y "Game Console R36S" **no definen** `DEVICE_HEADPHONE_DEV` ni `DEVICE_JACK`: ROCKNIX no usa la detección en estos equipos. Además invierten los nombres: `DEVICE_PLAYBACK_PATH_SPK="HP"` (coincide con lo medido: el parlante sale por HP) |
| Mainline 7.1.2: `rk3326-gameforce-chi.dts` | GPIO2_C6, activo en alto | **aplica** `hp_det` con pull-up | otra placa |
| ArkOS (kernel BSP `rg351`, `rk3326-rg351mp-linux.dts`, que usan los clones R36S) | GPIO2_C6, `GPIO_ACTIVE_LOW` | — | en la RG351MP el pin va a 0 con auriculares (sin verificar en la RGB20S) |
| Wiki "R36 Control Centre" (dArkOS para clones R36) | GPIO 86 (= GPIO2_C6) solo en placas **SoySauce** (Y3506) | — | para "r36s" y "clone": sin pin de detección documentado |

Conclusiones (sin verificar en la consola hasta correr el diagnóstico):
- **Pin candidato: GPIO2_C6** (gpiochip2 línea 22, pin 86). Ninguna fuente propone otro pin para la familia RK3326.
- **La polaridad sola no explica lo medido.** `simple-audio-card` pide IRQ en ambos flancos (debounce 150 ms): con el pin
  cableado al jack habría eventos aunque la polaridad estuviera invertida. Cero eventos = el pin no cambia de nivel.
- Dos explicaciones posibles: (a) **el jack no tiene línea de detección hacia el SoC** (su contacto solo corta el parlante;
  coincide con que ROCKNIX y dArkOS no la usan en RGB20S/R36S), o (b) la detección lleva el pin a 1 al enchufar (estilo OGA)
  y, sin el pull-down de `hp_det`, el pin ya está en 1 por un pull-up por defecto. La prueba de pull del diagnóstico separa (a) de (b).
- **Riesgo de la corrección propuesta antes** (`GPIO_ACTIVE_LOW` en `simple-audio-card`): con el pin fijo en 1 informaría
  "desenchufado" siempre, y simple-audio-card apaga el widget DAPM `Headphones` (la salida HP, de la que cuelga el parlante):
  **el parlante quedaría mudo**. Por eso el DTB propio borra `hp-det-gpios` y una detección real iría por `gpio-keys`.

Diagnóstico sin recompilar (`sudo ./jack-diagnose.sh` en la consola): lee `/sys/kernel/debug/gpio`, `/proc/interrupts`,
el pinmux/pinconf de `gpio2-22`, los canales del SARADC y, con `rgb20s-gpio-probe`, el nivel de los 128 GPIO por `/dev/mem`
(el kernel tiene `CONFIG_DEVMEM=y` sin `IO_STRICT_DEVMEM`), con y sin auriculares; después fuerza pull-down y pull-up en GPIO2_C6
(registro GRF `0xff140078`, bits 12-13) y lo restaura. `libgpiod` (`gpioget`) no está en la rootfs y además, al pedir la línea,
cambiaría el mux a GPIO: por eso la lectura es por registros.

Resultados: pendiente (correrlo con el usuario).

### No se pudo relevar
- Detección del jack con un conector enchufado (sin eventos en la prueba) y repetición de los botones mantenidos.
- Efecto audible del volumen y de `Playback Mux`.
- Si el modo `host` en dwc2 entrega VBUS (no hay vbus-supply en el DT: dmesg "vbus-supply ... failed"): sin 5 V de salida, el dongle no se alimentaría (sin verificar; el cambio de rol quedó fuera del alcance).
- Esa consola tiene un solo puerto USB-C: en modo host se pierde el SSH por gadget.
- Salida cruda completa: no se guardó en `docs/hardware/inventario/` (las salidas están resumidas arriba).

### Resuelve marcas (sin verificar)
- Códec RK817 en la RGB20S: confirmado (`rk817_int`, `ff070000.i2s-rk817-hifi`).
- Soporte de `snd-usb-audio` y HID en el kernel propio: confirmado (módulo presente).

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
- Jack (2026-11-20), ROCKNIX `next` commit 21d9dc67: [`rk3326-gameconsole-r3xs.dtsi`](https://github.com/ROCKNIX/distribution/blob/21d9dc6759cc3016c2e2e7244574c0e6a5a6bcd8/projects/ROCKNIX/devices/RK3326/linux/dts/rockchip/rk3326-gameconsole-r3xs.dtsi), [`rk3326-odroid-go.dtsi`](https://github.com/ROCKNIX/distribution/blob/21d9dc6759cc3016c2e2e7244574c0e6a5a6bcd8/projects/ROCKNIX/devices/RK3326/linux/dts/rockchip/rk3326-odroid-go.dtsi), [`headphone_sense`](https://github.com/ROCKNIX/distribution/blob/21d9dc6759cc3016c2e2e7244574c0e6a5a6bcd8/projects/ROCKNIX/packages/sysutils/system-utils/sources/scripts/headphone_sense), [quirks "Powkiddy RGB20S"](https://github.com/ROCKNIX/distribution/blob/21d9dc6759cc3016c2e2e7244574c0e6a5a6bcd8/projects/ROCKNIX/packages/hardware/quirks/devices/Powkiddy%20RGB20S/001-device_config) y ["Game Console R36S"](https://github.com/ROCKNIX/distribution/blob/21d9dc6759cc3016c2e2e7244574c0e6a5a6bcd8/projects/ROCKNIX/packages/hardware/quirks/devices/Game%20Console%20R36S/001-device_config)
- [dArkOS: `audio/headphone-audio-switch.sh`](https://github.com/christianhaitian/dArkOS/blob/master/audio/headphone-audio-switch.sh) y [wiki "R36 Control Centre" (dArkOS-G80CA-RE)](https://github.com/southoz/dArkOS-G80CA-RE/wiki/R36-Control-Centre)
- Linux 7.1.2 (`infra/build/kernel/`): `sound/soc/generic/simple-card-utils.c` (`simple_util_init_jack`), `drivers/gpio/gpio-rockchip.c` (`gpio_regs_v1`), `drivers/pinctrl/pinctrl-rockchip.c` (`px30_calc_pull_reg_and_bit`), `arch/arm64/boot/dts/rockchip/{px30.dtsi,rk3326-gameforce-chi.dts}`, `lib/devmem_is_allowed.c`
