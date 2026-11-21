# rgb20s-common

Biblioteca chica que comparten el [launcher](../launcher/README.md), el [groovebox](../groovebox/README.md)
y el servicio [rgb20s-audio](../rgb20s-audio/README.md). Sin hilos ni SDL: todo se llama desde el bucle de la
interfaz. Única dependencia: `serde` (derive), para que el servicio lea el modo de su config.

| Módulo | Contenido |
|---|---|
| `audio_state` | `Mode` (mono/estéreo), `AudioState` y su formato, `volume_percent`, ruta del estado (`state_path`) |
| `watch` | `StateWatcher`: detecta cambios del estado con un `stat` cada 100 ms (1 s si no existe) |
| `battery` | Carga de la batería desde `/sys/class/power_supply` (antes estaba en el launcher) |

## Estado del audio

El servicio escribe `/run/rgb20s-audio/state` (renombrado atómico, `0644`) al arrancar, en cada pulsación de
volumen (también en los extremos) y en cada cambio de modo; el directorio lo crea systemd
(`RuntimeDirectory=`) y desaparece al detener el servicio.

```text
volume_pct=66
volume_db=-20.1
mode=mono
seq=3
```

- **`volume_pct`** es lineal en dB: 100 % = máximo del control (0 dB en el RK817) y 0 % = 60 dB por debajo.
  Cada paso de 3 dB mueve un 5 % (con la cuantización del control, a veces 4 o 6). Por debajo de -60 dB se
  muestra 0 % aunque todavía no esté en el mínimo (-95 dB).
- `seq` sube con cada publicación; sirve para distinguir una pulsación en un extremo (mismo volumen) de una
  reescritura idéntica. Vuelve a 1 si se reinicia el servicio.
- El lector ignora claves desconocidas y comentarios: se pueden agregar campos sin romper apps viejas.

`StateWatcher::poll(now_ms)` devuelve `Appeared` (primera lectura: no se resalta), `Changed { volume, mode }`
o `Gone`. Si el archivo no existe (servicio sin instalar, simulador), no informa nada.
`RGB20S_AUDIO_STATE=/ruta` reemplaza la ruta (simulador y pruebas).

## Uso

Dependencia por ruta en cada crate: `rgb20s-common = { path = "../rgb20s-common" }`. Funciona igual en el
build cruzado: `docker-cargo.sh` monta todo el repo en `/work`.

```sh
cd projects/rgb20s-common && cargo test     # no tiene binario: `just check` no aplica
```

Los crates que la usan la compilan y testean con su `just check`, pero sus tests propios solo corren acá.
