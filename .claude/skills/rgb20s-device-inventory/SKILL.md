---
name: rgb20s-device-inventory
description: Checklist de comandos para relevar el hardware y el software de la RGB20S por SSH (sistema, CPU, glibc/SDL2, audio, input, Wi-Fi, USB OTG/gadget, almacenamiento) y plantilla para documentarlo en docs/hardware/inventario.md. Usar al conectarse por primera vez o después de cambiar el sistema de la consola.
---

# Inventario de la consola

Todo es **solo lectura**. Correr vía `ssh rgb20s '<comando>'`, guardar la salida cruda en
`docs/hardware/inventario/AAAA-MM-DD/<nombre>.txt` y el resumen en `docs/hardware/inventario.md`.

## Comandos

| Área | Comandos |
|---|---|
| Sistema | `cat /etc/os-release`, `uname -a`, `uptime`, `df -h`, `free -m` |
| CPU | `cat /proc/cpuinfo`, `ls /sys/devices/system/cpu/cpu0/cpufreq/`, `cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_{available_frequencies,governor}` |
| Temperatura | `cat /sys/class/thermal/thermal_zone*/{type,temp}` |
| Librerías | `ldd --version`, `ls -l /usr/lib/*/libSDL2*` (o `sdl2-config --version`), `ls /usr/lib/*/libasound*` |
| Device tree | `cat /proc/device-tree/model; echo`, `cat /proc/device-tree/compatible \| tr '\0' '\n'` |
| Kernel | `lsmod`, `dmesg \| head -200` (puede requerir sudo: preguntar), `zcat /proc/config.gz \| grep -E 'USB_CONFIGFS\|DWC2\|SND_SOC\|PANFROST'` si existe |
| Audio | `aplay -l`, `aplay -L`, `amixer`, `cat /proc/asound/cards` |
| Input | `ls -l /dev/input/by-path/`, `cat /proc/bus/input/devices` (nombres y capacidades de botones y sticks) |
| Pantalla y GPU | `ls /dev/dri/`, `cat /sys/class/drm/*/modes`, `cat /sys/class/backlight/*/max_brightness` |
| Wi-Fi | `ip link`, `iw dev`, `dmesg \| grep -iE 'wlan\|sdio\|rtl\|brcm\|esp'`, `lsusb` |
| USB / OTG | `ls /sys/class/udc/` (si hay UDC, el modo gadget es posible), `cat /sys/kernel/debug/usb/*/dr_mode` si es accesible, `ls /sys/kernel/config/usb_gadget/` |
| Almacenamiento | `lsblk -o NAME,SIZE,FSTYPE,MOUNTPOINT,LABEL` |
| Batería | `cat /sys/class/power_supply/*/uevent` |

`evtest` es interactivo: solo con el usuario presente y pidiéndole que toque los botones.

## Plantilla de `docs/hardware/inventario.md`

```markdown
# Inventario de la RGB20S

## AAAA-MM-DD — <sistema y versión>
| Dato | Valor | Comando |
|---|---|---|
| Sistema | | `cat /etc/os-release` |
| Kernel | | `uname -r` |
| glibc | | `ldd --version` |
| SDL2 | | |
| Códec de audio | | `aplay -l` |
| Chip Wi-Fi | | `dmesg` / `lsmod` |
| UDC (gadget USB) | sí / no | `ls /sys/class/udc` |
| Device tree compatible | | `/proc/device-tree/compatible` |

### Resuelve marcas (sin verificar)
- docs/hardware/rgb20s.md: "códec RK817" → confirmado / refutado (evidencia)
```

Después actualizar los docs donde había **(sin verificar)** con el dato y un enlace al inventario.
