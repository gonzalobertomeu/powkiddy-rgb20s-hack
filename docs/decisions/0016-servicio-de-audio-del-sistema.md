# 0016 — Un servicio de sistema en Rust maneja el volumen y mezcla a mono en ALSA

- **Fecha:** 2026-11-20
- **Estado:** propuesta

## Contexto
Lo medido en la consola ([inventario](../hardware/inventario.md), "Audio, volumen y USB host"):

- El parlante está cableado al **canal izquierdo** de la salida de auriculares; con `Playback Mux` en
  SPK no suena nada. El jack corta el parlante físicamente al enchufar auriculares.
- La **detección del jack no funciona** (`SW_HEADPHONE_INSERT` siempre en 1, sin eventos): no se puede
  saber si suena el parlante o los auriculares.
- Los botones de volumen son un evdev aparte (`gpio-keys-vol`, `KEY_VOLUMEUP`/`KEY_VOLUMEDOWN`, con
  repetición del kernel) y nadie los atiende. El único control de salida es `Master Playback Volume`
  (0..255, -95..0 dB); `amixer sset Master` también mueve la captura.
- No hay pipewire ni pulse: cada app (groovebox con cpal, `speaker-test`) abre el `default` de ALSA.

Por el parlante se pierde el canal derecho, y el volumen no se puede cambiar sin SSH.

## Alternativas consideradas
| Opción | A favor | En contra |
|---|---|---|
| **Servicio de sistema** que atiende los botones y el mixer | Funciona igual con cualquier app, también en el launcher y sin apps abiertas | Un proceso más; un gesto global de botones que las apps no ven |
| Volumen y mono dentro de cada app | Sin piezas del sistema; mezcla en vivo, sin reabrir el audio | Repetir en cada app (y en las que no son nuestras); no anda en el launcher |
| **Mezcla a mono en ALSA** (`default` → `plug` → `route` con `ttable`) | Vale para todas las apps que usan `default`, sin tocarlas; costo despreciable | El cambio se toma al abrir el PCM; alsa-lib cachea la config por proceso |
| Mezcla a mono en la app | Cambio instantáneo | Solo en nuestras apps; cada una necesita saber el modo |
| Servidor de audio (pipewire) con un filtro | Cambio en vivo para todos | Mucho más pesado para un A35 con 1 GB; latencia extra para el groovebox |
| Script en bash con `evtest` y `amixer` | Corto | Parseo de texto frágil, un proceso por tecla, difícil de testear el gesto y la repetición |
| **Servicio en Rust** (crates `alsa`, `serde`, `toml`; evdev leído a mano) | Mismo build y estilo que el launcher; lógica (pasos en dB, gesto, config) con tests | Más código que el script |

Dónde escribir la config de ALSA: `/etc/asound.conf` con un `include` que el servicio reescribe, o un
archivo en **`/var/lib/alsa/conf.d/`**, que `alsa.conf` ya carga con `errors false` (pensado para
configuración generada en tiempo de ejecución). Se eligió el segundo: no toca `/etc`, y si el archivo
falta ALSA vuelve sola a su `default` en vez de fallar.

## Decisión
Un servicio `rgb20s-audio` en Rust (root sin capacidades y con hardening de systemd) atiende los
botones de volumen (pasos de 3 dB), alterna mono/estéreo con los dos botones a la vez (mono por
defecto), fija `Playback Mux` en HP, guarda volumen y modo, y redefine el `default` de ALSA en
`/var/lib/alsa/conf.d/60-rgb20s-audio.conf`. El jack puede elegir el modo, desactivado por defecto.

## Motivos
- Es la única opción que cubre todas las apps y el launcher sin tocarlas.
- La mezcla en ALSA se verificó en la Mac con `aplay`: mono da (L+R)/2 en ambos canales, un WAV mono
  sale igual en los dos, estéreo queda intacto; acepta S16 y float.
- Rust mantiene un solo toolchain y permite testear el gesto (ventana, repetición, soltar) sin hardware.
- El estado de los botones se lee como `input_event` crudo y la búsqueda por nombre va por sysfs: no
  hace falta el crate `evdev`. El jack se lee por su control ALSA, que además notifica cambios.

## Consecuencias
- **Las apps toman el modo al abrir el audio.** Reabrir el PCM en el mismo proceso no alcanza (alsa-lib
  cachea la config y no ve cambios en archivos de hooks), salvo que la app llame a
  `snd_config_update_free_global()` antes (probado en la Mac). El groovebox lo hace solo al ver el modo
  nuevo (reabre el stream y la reproducción vuelve a empezar).
- **Las apps muestran volumen y modo.** No hay compositor: cada app dibuja su indicador (decisión del
  usuario). El servicio publica `/run/rgb20s-audio/state` (`RuntimeDirectory=`, legible por `alarm`) con
  el volumen en % lineal en dB (0 % = 60 dB bajo el máximo, 5 % por paso), el modo y un contador; las
  apps lo leen con `rgb20s-common` (un `stat` cada 100 ms, sin hilos). Launcher: barra superpuesta 1,5 s;
  groovebox: batería y volumen en el título.
- Cada pulsación de volumen espera hasta 150 ms (la ventana del gesto); un toque corto se aplica al
  soltar.
- Depende de que el `alsa.conf` de la consola cargue `/var/lib/alsa/conf.d` (alsa-lib 1.2.5 o más
  nueva; `install.sh` lo comprueba). Un `/etc/asound.conf` con `pcm.!default` le ganaría.
- Las apps que abren `hw:`/`plughw:` directo no pasan por la mezcla.
- `alsa-restore.service` sigue restaurando y guardando el volumen; el servicio arranca después.

## Pendientes
- [ ] Probar en la consola: botones, repetición, gesto, `alsactl store` con el hardening, mezcla mono
      audible por el parlante y estéreo con auriculares (device-ops).
- [ ] Confirmar que el `alsa.conf` de la consola carga `/var/lib/alsa/conf.d` (sin verificar).
- [x] El groovebox reabre el audio solo al cambiar el modo (lee el estado publicado y llama a
      `update_free_global`). Falta medir en la consola cuánto tarda y cómo suena el corte.
- [ ] Probar en la consola la barra del launcher y el título del groovebox con el servicio real.
- [ ] Si se arregla la detección del jack en el DTS: activar `[jack]` y verificar la polaridad.

## Fuentes
- [alsa-lib: `src/conf/alsa.conf` (hooks que cargan `/var/lib/alsa/conf.d` y `/etc/asound.conf`)](https://github.com/alsa-project/alsa-lib/blob/master/src/conf/alsa.conf); verificado en la copia local de alsa-lib 1.2.16.
- [alsa-lib: plugin `route` (`ttable.IN.OUT`)](https://www.alsa-project.org/alsa-doc/alsa-lib/pcm_plugins.html)
- [alsa-lib: `snd_config_update_free_global`](https://www.alsa-project.org/alsa-doc/alsa-lib/group___config.html)
- [Linux: `input-event-codes.h` y `struct input_event`](https://www.kernel.org/doc/html/latest/input/input.html)
