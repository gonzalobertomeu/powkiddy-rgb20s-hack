# rgb20s-audio

Servicio de sistema para el audio de la RGB20S: botones de volumen, modo mono/estéreo para todas
las apps y `Playback Mux` fijo en HP. Decisión: [ADR 0016](../../docs/decisions/0016-servicio-de-audio-del-sistema.md) (propuesta).
Datos del hardware: [inventario, "Audio, volumen y USB host"](../../docs/hardware/inventario.md).

Estado: tests de la lógica y prueba de la configuración de ALSA con `aplay` en la Mac; compilado
para aarch64 (glibc 2.39). **Sin probar en la consola.**

## Qué hace

- **Volumen**: los botones (`gpio-keys-vol`, buscado por nombre en `/sys/class/input`) cambian
  `Master Playback Volume` en pasos de **3 dB** (≈ 8 de 0..255), con la repetición del kernel al
  mantener. Escribe solo la reproducción (no toca `Master Capture Volume`).
- **Mono/estéreo**: el parlante está cableado al canal izquierdo, así que por defecto el
  `default` de ALSA mezcla a mono, (L+R)/2 en los dos canales. Estéreo es para auriculares.
- **`Playback Mux` = HP** al arrancar (con SPK no suena nada).
- **Estado para las apps**: `/run/rgb20s-audio/state` (volumen en %, modo, contador), escrito al
  arrancar, en cada pulsación y en cada cambio de modo. El launcher muestra una barra de volumen y el
  groovebox lo muestra en el título. Formato y escala del porcentaje (lineal en dB, 5 % por paso):
  [rgb20s-common](../rgb20s-common/README.md#estado-del-audio).
- **Persistencia**: `alsactl store` 2 s después del último cambio de volumen (al arrancar lo
  restaura `alsa-restore.service`); el modo, en `/var/lib/rgb20s-audio/mode`.
- **Jack** (desactivado): con `[jack] enabled = true`, auriculares → estéreo y sin auriculares →
  mono, leyendo el control `Headphones Jack`. Hoy la detección no funciona (queda siempre en 1).
- No abre ningún PCM. Logs: `journalctl -u rgb20s-audio -f` (una línea por cambio de modo y una
  por ráfaga de volumen, al guardar).

## Gesto: los dos botones a la vez

Apretar **Vol+ y Vol−** con menos de **150 ms** de diferencia alterna mono/estéreo, sin cambiar el
volumen. Para eso cada pulsación espera hasta 150 ms antes de aplicar el paso; un toque corto se
aplica al soltar, así que casi no se nota. Tras el gesto se ignoran los botones (y sus
repeticiones) hasta soltar los dos. La ventana se cambia con `gesture_window_ms`.

## Cómo llega el modo a las apps

El servicio escribe `/var/lib/alsa/conf.d/60-rgb20s-audio.conf` (renombrado atómico).
`alsa.conf` de alsa-lib carga esa carpeta con `errors false`, así que si el archivo no está, ALSA
vuelve a su `default` de fábrica. Cadena de reproducción de `default`:
`plug` (formato, frecuencia, 1→2 canales) → `route` con `ttable` → `plughw:0,0`. La captura va a
`plughw:0,0`. Para verlo: `rgb20s-audio alsa-conf mono`.

- **Las apps toman el modo al abrir el dispositivo.** El groovebox (cpal, PCM abierto todo el tiempo)
  lo cambia solo: al ver el modo nuevo en el estado, cierra el stream, llama a
  `update_free_global()` y lo vuelve a abrir (ver su README).
- alsa-lib guarda la configuración en caché por proceso y no detecta cambios en archivos cargados
  por hooks: reabrir el PCM en el mismo proceso **no alcanza**. Sí funciona si la app llama antes a
  `snd_config_update_free_global()` (`alsa::config::update_free_global()`); probado en la Mac.
- Las apps que abren `hw:`/`plughw:` directamente (por ejemplo `speaker-test -D plughw:0,0`) no
  pasan por la mezcla.

## Configuración

`/etc/rgb20s-audio.toml`, opcional (si falta, valores por defecto). Ejemplo comentado con todos los
campos: [`system/rgb20s-audio.toml`](system/rgb20s-audio.toml). Después de editar:
`sudo systemctl restart rgb20s-audio`.

## Desarrollo

```sh
APP=rgb20s-audio just test
APP=rgb20s-audio just check                  # fmt, clippy, tests, build aarch64, glibc
cargo run -- alsa-conf mono                  # imprime la config de ALSA
```

Lógica pura con tests: `volume.rs` (pasos en dB y porcentaje), `gesture.rs` (botones y gesto), `alsa_conf.rs`,
`config.rs`, `evdev.rs` (eventos crudos y búsqueda por nombre), `files.rs` (escritura atómica, modo y
estado). `Mode` y el formato del estado están en [rgb20s-common](../rgb20s-common/README.md).
`RGB20S_AUDIO_STATE` cambia la ruta del estado (pruebas).
I/O: `hw.rs` (mixer y control del jack con el crate `alsa`) y `main.rs` (hilos y bucle).

Probar la mezcla en la Mac sin tocar la configuración del sistema: un `alsa.conf` copiado cuyos
hooks cargan una carpeta propia (`ALSA_CONFIG_PATH`), `pcm_slave` apuntando a un PCM `type file`
y `aplay` de un WAV con L ≠ R. Resultado: mono da (L+R)/2 en los dos canales y un WAV mono sale
igual en ambos; estéreo deja L y R intactos.

## Instalación en la consola

```sh
APP=rgb20s-audio just deploy                      # binario en /home/alarm/rgb20s-audio
scp -r projects/rgb20s-audio/system rgb20s:/home/alarm/rgb20s-audio/
ssh rgb20s 'sudo /home/alarm/rgb20s-audio/system/install.sh'
```

`install.sh` comprueba que `alsa.conf` cargue `/var/lib/alsa/conf.d`, copia el binario a
`/usr/local/bin`, la unidad a `/etc/systemd/system`, la config de ejemplo a `/etc/rgb20s-audio.toml`
(si no existe) y habilita y arranca el servicio. Avisa si `/etc/asound.conf` define `default`
(se carga después y le ganaría).

Desinstalar: `sudo /home/alarm/rgb20s-audio/system/uninstall.sh` (`--purge` borra también la
config). Saca el archivo de `conf.d`, así que ALSA vuelve al estéreo de fábrica.

La unidad corre como root sin capacidades, con `ProtectSystem=strict` y escritura solo en
`/var/lib/alsa`, `/var/lib/rgb20s-audio` y `/run/rgb20s-audio` (`RuntimeDirectory=`, 0755; se borra
al detener el servicio), `DevicePolicy=closed` con `char-alsa` (rw) y
`char-input` (r), sin red. Arranca después de `alsa-restore.service` (que restaura al arrancar y
guarda al apagar) y no lo reemplaza.
