# 0010 — Kernel mainline 7.1.2 con el DTS de la RGB20S y lo mínimo de ROCKNIX

- **Fecha:** 2026-11-19
- **Estado:** aceptada (por el usuario, 2026-11-20)

## Contexto

La RGB20S no tiene device tree en mainline. ROCKNIX tiene `rk3326-powkiddy-rgb20s.dts`, pero depende de:

- su propio `rk3326.dtsi` y `rk3326-odroid-go.dtsi` (distintos de los de mainline: role switch USB, OPP, nodo de la GPU),
- un driver de panel fuera de mainline, `rocknix,generic-dsi` (`panel-generic-dsi.c`), que lee la secuencia de init y
  los modos del propio DT. ROCKNIX probó los paneles NV3051D de mainline y los dejó comentados en el DTS,
- un driver de joypad fuera de mainline, `rocknix-singleadc-joypad` (repo `ROCKNIX/rocknix-joypad`): sticks por un
  multiplexor analógico sobre un canal del SARADC más botones GPIO.

ROCKNIX compila RK3326 con Linux 7.1.2 más varios directorios de parches. La versión estable de kernel.org es 7.3.5
(7.1 ya no recibe actualizaciones).

## Alternativas consideradas

| Opción | A favor | En contra |
|---|---|---|
| **A. 7.1.2 + DTS de ROCKNIX sin cambios + `panel-generic-dsi` built-in + `rocknix-joypad` como módulo externo + 7 parches puntuales (dwc2, SD, símbolo del joypad) + config de ROCKNIX** | La misma combinación que ROCKNIX usa en la RGB20S: lo más probable es que pantalla, controles y audio anden de entrada. Pocos cambios propios, todos visibles | 7.1 sin soporte. Dos drivers fuera de mainline |
| B. Kernel de ROCKNIX completo (todos sus parches) | Idéntico a ROCKNIX | Cientos de parches que no aplican (DVB de Amlogic, HDMI, PX30S, Wi-Fi ESP8089). Más lento y opaco |
| C. 7.3.5 (o 6.18 LTS) + DTS portado a drivers de mainline (`adc-joystick` + `io-channel-mux`, panel NV3051D) | Sin código fuera de mainline. Kernel con soporte | Portar el joypad y adivinar el panel sin la consola. Si la pantalla no enciende, no hay forma de diagnosticar |

## Decisión

Opción **A**. Las fuentes de ROCKNIX se copian al repo (`projects/linux-image/kernel/rocknix/`) desde el commit
`61690fa8`, con el hash de cada archivo. La config de ROCKNIX se usa como base con un fragmento propio
(`config.fragment`: sin initramfs embebido, `PANIC_TIMEOUT=0`, gadget ACM/MIDI/HID, identificación `-rgb20s`).
Se compila además un DTB propio (`rk3326-powkiddy-rgb20s-alarm.dtb`) que solo cambia el modo USB por defecto a periférico
(ver [ADR 0012](0012-acceso-por-usb-gadget-ncm-acm.md)).

## Motivos

- Primer arranque a ciegas: lo más importante es que la pantalla encienda para ver los mensajes. Eso depende del panel,
  y el único driver probado en la RGB20S es `generic-dsi` con la secuencia del DTS de ROCKNIX.
- Pocos parches propios: lo que difiere de mainline está en archivos concretos y fáciles de revisar.
- Los dos drivers externos compilan contra APIs estándar del kernel (GPIO por descriptores, IIO, PWM): no necesitan
  los parches de "API legacy" de ROCKNIX.

## Consecuencias

- Kernel 7.1.2 sin parches de seguridad: aceptable mientras la única red sea el cable USB a la Mac. Pasar a 7.3 o a
  6.18 LTS cuando ROCKNIX lo haga (los parches y el DTS se mantienen ahí).
- Panfrost queda como módulo (`DRM_PANFROST=m`); el DTS de ROCKNIX trae propiedades del blob Mali en el nodo de la GPU
  que Panfrost debería ignorar **(sin verificar)**.
- La config de ROCKNIX trae mucho que no hace falta (btrfs, ntfs, nftables...). Adelgazarla es un trabajo aparte, medible
  con el tiempo de arranque.

## Pendientes

- [ ] Confirmar en la consola: pantalla (`/sys/class/drm`), joypad (`evtest`), audio (`aplay -l`), Panfrost (`/dev/dri/renderD128`)
- [ ] Adelgazar la config y medir el arranque con `systemd-analyze`
- [ ] Evaluar portar el joypad a `adc-joystick` + `io-channel-mux` de mainline (opción C) cuando haya hardware para probar

## Fuentes

- [ROCKNIX: `packages/linux/package.mk`](https://github.com/ROCKNIX/distribution/blob/61690fa8042acf631922deb2c80e8574e96958d2/projects/ROCKNIX/packages/linux/package.mk) (versión 7.1.2 para RK3326, integración de `panel-generic-dsi.c`)
- [ROCKNIX: `devices/RK3326/linux/dts/rockchip/`](https://github.com/ROCKNIX/distribution/tree/61690fa8042acf631922deb2c80e8574e96958d2/projects/ROCKNIX/devices/RK3326/linux/dts/rockchip)
- [ROCKNIX: `devices/RK3326/patches/linux/`](https://github.com/ROCKNIX/distribution/tree/61690fa8042acf631922deb2c80e8574e96958d2/projects/ROCKNIX/devices/RK3326/patches/linux)
- [ROCKNIX: `devices/RK3326/linux/linux.aarch64.conf`](https://github.com/ROCKNIX/distribution/blob/61690fa8042acf631922deb2c80e8574e96958d2/projects/ROCKNIX/devices/RK3326/linux/linux.aarch64.conf)
- [ROCKNIX/rocknix-joypad, commit ac1be0fa](https://github.com/ROCKNIX/rocknix-joypad/tree/ac1be0fa0c4e1f7feb9a900fc428d7ac6014029c)
- [kernel.org: releases](https://www.kernel.org/releases.json) (7.3.5 estable, 2026-11-19)
