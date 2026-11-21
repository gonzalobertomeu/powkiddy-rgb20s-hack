# 0017 — Sacar la detección de jack de simple-audio-card en el DTB propio

- **Fecha:** 2026-11-20
- **Estado:** propuesta

## Contexto
El DTS de ROCKNIX para la RGB20S hereda de la Odroid Go Advance `simple-audio-card,hp-det-gpios = <&gpio2 RK_PC6 GPIO_ACTIVE_HIGH>`.
En la consola (2026-11-20) `SW_HEADPHONE_INSERT` queda siempre en 1 y no hubo ningún evento al enchufar ni al desenchufar
(dos capturas, 180 s y 90 s). El parlante está cableado al canal L de la salida HP y el jack lo corta mecánicamente.
Con simple-audio-card, la detección no solo informa: cuando dice "desenchufado" apaga el widget DAPM `Headphones`, es decir,
la salida HP que también alimenta el parlante.

## Alternativas consideradas
| Opción | A favor | En contra |
|---|---|---|
| Dejarlo como está | sin cambios | `Headphones Jack` e `event1` mienten (siempre "enchufado"); cualquier ajuste futuro del pin puede mutear el parlante |
| Invertir la polaridad (`GPIO_ACTIVE_LOW`) | lo propuso una investigación previa; ArkOS usa activo en bajo en la RG351MP | sin eventos sigue sin detectar nada; con el pin en 1 informaría "desenchufado" siempre y **apagaría la salida HP: parlante mudo** |
| Quitar `hp-det-gpios` | la salida HP queda siempre encendida; no hay señales falsas | se pierde el control `Headphones Jack` (que hoy no sirve) |
| Quitar `hp-det-gpios` y, si hay pin real, detectar con `gpio-keys` (`EV_SW`) | informa sin tocar DAPM | la app lee evdev en vez de un control ALSA; hace falta confirmar el pin |

## Decisión
El DTB propio borra `hp-det-gpios`. Si el diagnóstico de `projects/linux-image/jack/` encuentra el pin, la detección se agrega
con un nodo `gpio-keys` (variante `rk3326-powkiddy-rgb20s-alarm-hpdet.dts`), nunca con simple-audio-card.

## Motivos
Con el parlante colgado de la salida HP, ninguna detección acoplada a DAPM es correcta en esta placa. Quitarla no cambia lo que
se oye hoy (con "siempre enchufado" la salida ya estaba siempre encendida) y elimina un dato falso. ROCKNIX tampoco usa la
detección en la RGB20S ni en la R36S: sus quirks no definen `DEVICE_HEADPHONE_DEV` ni `DEVICE_JACK` para esos equipos.

## Consecuencias
- Desaparecen el control ALSA `Headphones Jack` y `/dev/input/event*` "rk817_int Headphones". `rgb20s-audio` (ADR 0016) los
  usa solo con `[jack] enabled = true` (desactivado por defecto); si se habilita la variante `-hpdet`, tiene que leer
  `SW_HEADPHONE_INSERT` del evdev "headphone-detect".
- El DTB de ROCKNIX (entrada `arch-rocknix`) sigue igual, con su detección.

## Pendientes
- [ ] Correr `jack-diagnose.sh` en la consola con auriculares
- [ ] Instalar el DTB nuevo y confirmar sonido por parlante y auriculares
- [ ] Si hay pin: probar `--hpdet` y adaptar `rgb20s-audio`

## Fuentes
- Medición en la consola y fuentes de ROCKNIX, ArkOS/dArkOS y Linux 7.1.2: [`docs/hardware/inventario.md`](../hardware/inventario.md#jack-de-auriculares-diagnóstico-del-pin-de-detección)
