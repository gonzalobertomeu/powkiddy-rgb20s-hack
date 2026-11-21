# jack — detección de auriculares de la RGB20S

Diagnóstico del pin de detección del jack y cambio **solo del device tree** en una consola en uso.
Hallazgos y evidencia: [`docs/hardware/inventario.md`](../../../docs/hardware/inventario.md#jack-de-auriculares-diagnóstico-del-pin-de-detección).
Decisión: [ADR 0017](../../../docs/decisions/0017-sin-deteccion-de-jack-en-simple-audio-card.md) (propuesta).

## Resumen

- El DTS de ROCKNIX (heredado de la Odroid Go Advance) declara `simple-audio-card,hp-det-gpios = <&gpio2 RK_PC6 GPIO_ACTIVE_HIGH>`.
  En la RGB20S ese pin se lee siempre en 1 y no cambia al enchufar (medido el 2026-11-20). Cambiar la polaridad
  **no** lo arregla (seguiría sin eventos) y además **mutearía el parlante**: simple-audio-card apaga el widget DAPM
  "Headphones" cuando informa "desenchufado", y el parlante cuelga del canal L de esa salida.
- El DTB propio (`rk3326-powkiddy-rgb20s-alarm.dtb`) ahora **borra `hp-det-gpios`**: la salida HP queda siempre
  encendida y desaparecen el control ALSA `Headphones Jack` y el evdev "rk817_int Headphones", que daban siempre "enchufado".
- Si el diagnóstico encuentra un pin real, la detección va por `gpio-keys` (evento `SW_HEADPHONE_INSERT`, sin tocar DAPM):
  variante `rk3326-powkiddy-rgb20s-alarm-hpdet.dtb` (GPIO2_C6 con pull-down, activo en alto; **experimental**).

## Archivos

| Archivo | Qué es |
|---|---|
| `jack-diagnose.sh` | diagnóstico interactivo en la consola (root): estado del kernel, barrido de los 128 GPIO, prueba de pull en GPIO2_C6 |
| `rgb20s-gpio-probe.c` | lee el nivel de todos los GPIO por `/dev/mem` sin pedirlos al kernel; `pull` cambia el pull de un pin hasta reiniciar |
| `install-dtb-on-device.sh` | instala solo los DTB (con respaldo en `/boot/dtb-backup-<fecha>/`) |
| `rollback-dtb-on-device.sh` | vuelve al último respaldo de DTB |
| `../kernel/dts/rk3326-powkiddy-rgb20s-alarm-hpdet.dts` | variante con detección por `gpio-keys` |

## Armar el paquete

```sh
just img-dtb-update     # infra/build/out/dtb-update/ (DTB + scripts + rgb20s-gpio-probe + SHA256SUMS)
```

Compila solo los DTB (segundos); necesita que `just img-kernel` haya corrido alguna vez (árbol del kernel preparado).
`just img` / `just img-kernel` también compilan los tres DTB, así que una imagen nueva ya trae el cambio.

## Diagnóstico (con el usuario y auriculares a mano)

```sh
scp -r infra/build/out/dtb-update rgb20s:/tmp/
ssh -t rgb20s 'cd /tmp/dtb-update && sudo ./jack-diagnose.sh'
```

`sudo` hace falta porque debugfs (`/sys/kernel/debug/gpio`, `pinctrl`) y `/dev/mem` son solo de root. El script pide,
en orden: desenchufar, enchufar, 30 s de enchufar/desenchufar tres veces (barrido) y, con pull-down y con pull-up forzados
en GPIO2_C6, desenchufar y enchufar otra vez. Deja el pull como estaba. Con `--no-pull` no toca nada.
Log: `/tmp/dtb-update/jack-diagnose-<fecha>.log` (traerlo con `scp`).

Sin el paquete, lo mínimo a mano (también como root):

```sh
grep -i headphone /sys/kernel/debug/gpio /proc/interrupts      # nivel (hi/lo) y cuenta de IRQ, con y sin auriculares
grep 'gpio2-22)' /sys/kernel/debug/pinctrl/*pinctrl*/pinmux-pins /sys/kernel/debug/pinctrl/*pinctrl*/pinconf-pins
```

| Resultado | Conclusión | Qué hacer |
|---|---|---|
| GPIO2_C6 no cambia con ningún pull y el barrido no muestra otro pin | no hay línea de detección hacia el SoC | dejar el DTB propio (sin `hp-det`); la app no puede saber si hay auriculares |
| GPIO2_C6 vale 0 sin y 1 con auriculares con pull=down | detección estilo OGA a la que le falta el pull-down | `install-dtb-on-device.sh --hpdet` |
| GPIO2_C6 cambia pero al revés, o cambia otro pin | pin o polaridad distintos | ajustar `gpios`/`pinctrl` en el `-hpdet.dts`, `just img-dtb-update` |

## Instalar y revertir

```sh
ssh -t rgb20s 'cd /tmp/dtb-update && sudo ./install-dtb-on-device.sh --dry-run'   # mirar qué cambia
ssh -t rgb20s 'cd /tmp/dtb-update && sudo ./install-dtb-on-device.sh && sudo reboot'
# variante con detección: sudo ./install-dtb-on-device.sh --hpdet
```

Solo cambia archivos en `/boot/dtbs/rockchip/` (escritura atómica con `.new` + `mv`); el kernel, los módulos y
`extlinux.conf` no se tocan. Comprobar después del reinicio: sonido por el parlante (`speaker-test`), que no exista
`Headphones Jack` en `amixer -c 0 contents` y, con `--hpdet`, eventos en `evtest` ("headphone-detect").

Volver atrás:
- Por SSH: `sudo ./rollback-dtb-on-device.sh` y `sudo reboot`.
- Si no arranca (improbable: solo cambia el nodo de sonido): en la Mac, montar la FAT `RGB20S_BOOT` y copiar
  `dtb-backup-<fecha>/rk3326-powkiddy-rgb20s-alarm.dtb` a `dtbs/rockchip/`, o poner `DEFAULT arch-rocknix` en
  `extlinux/extlinux.conf` (DTB de ROCKNIX, que no cambia; deja el USB en modo host, así que sin SSH).

## Probado

- `just img-dtb-update` en el contenedor: los tres DTB compilan; el de ROCKNIX queda idéntico byte a byte al anterior;
  decompilado, el propio solo pierde `hp-det-gpios` (más renumeración de phandles) y el `-hpdet` agrega `headphone-detect`
  con `pinctrl-0` apuntando a `hp-det` (GPIO2_C6, `bias-pull-down`) y `gpios` en el banco con IRQ 5 (gpio2).
- Instalar, reinstalar (idempotente), `--hpdet`, `--dry-run` y rollback en un contenedor con `/boot` en tmpfs
  (`RGB20S_ASSUME_RGB20S=1`): los hashes vuelven a los originales.
- **Sin probar en la consola:** `rgb20s-gpio-probe` (lectura por `/dev/mem`), el diagnóstico y el arranque con el DTB nuevo.
