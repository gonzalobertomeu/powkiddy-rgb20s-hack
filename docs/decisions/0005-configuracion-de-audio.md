# 0005 — Audio a 48 kHz en f32, buffer de 256 frames y secuenciador en el hilo de audio

- **Fecha:** 2026-11-19
- **Estado:** aceptada (por el usuario, 2026-11-20)

## Contexto

El motor corre en un Cortex-A35 con `cpal` sobre ALSA ([ADR 0001](0001-lenguaje-rust.md)). La sample rate y el formato interno
condicionan los tamaños de los buffers de delay y reverb, la lectura de samples y la exportación a WAV. El tamaño del buffer
fija la latencia y el riesgo de cortes (xruns). La ubicación del secuenciador define si el tiempo es exacto o depende de la interfaz.
La frecuencia nativa del códec (RK817, sin verificar) todavía no se conoce.

## Alternativas consideradas

| Opción | A favor | En contra |
|---|---|---|
| **48 kHz** | Frecuencia habitual de códecs y de ALSA; buena para video y USB | Un 9 % más de CPU que 44,1 kHz |
| 44,1 kHz | Algo menos de CPU; nativa de muchos samples | Si el códec trabaja a 48 kHz, ALSA remuestrea (más latencia y CPU) |
| **`f32` interno** | Simple, sin problemas de headroom, natural en Rust | El doble de memoria que `i16` en samples |
| `i16` / punto fijo | Menos memoria | Código más complejo y propenso a errores para un principiante |
| Buffer de 128 frames | 2,7 ms de latencia | Más riesgo de xruns en un sistema no realtime |
| **Buffer de 256 frames** | 5,3 ms; equilibrio razonable | Algo más de latencia |
| Buffer de 512 frames | Muy estable | 10,7 ms por período; se siente al tocar en vivo |
| **Secuenciador en el hilo de audio** | Tiempo exacto a la muestra; independiente de los fps | La lógica del secuenciador debe cumplir las reglas de tiempo real |
| Secuenciador en el hilo de interfaz | Más fácil de depurar | Jitter de hasta un cuadro (16 ms); inaceptable para ritmo |

## Decisión

Motor a **48 kHz**, estéreo, **`f32`** interno, **256 frames** por buffer (con 128 y 512 como alternativas configurables),
procesamiento en subbloques de hasta 32 frames, y **secuenciador dentro del hilo de audio**, comunicado con la interfaz por colas `rtrb`.

## Motivos

- 48 kHz evita remuestreo en el driver en la mayoría de los códecs; el costo extra de CPU es irrelevante con el margen estimado (~24 % de un núcleo en el peor caso, con polifonía según el [ADR 0007](0007-polifonia-en-el-mvp.md)).
- `f32` simplifica todo el DSP y el aprendizaje; la memoria sobra (1 GB).
- 256 frames es el punto de partida habitual para latencia baja sin un kernel realtime.
- El secuenciador en el hilo de audio es la práctica estándar para que el ritmo sea exacto.

## Consecuencias

- Los samples a otras frecuencias se leen con un paso ajustado (o se remuestrean al cargar en v1).
- Hay que medir en la consola la latencia real, los xruns con 128/256/512 y la frecuencia nativa del códec (`device-ops`, skill `audio-latency-test`).
- Si el códec resulta ser de 44,1 kHz nativo, se revisa esta decisión con un ADR nuevo.
- Detalle en [spec-motor](../solutions/groovebox/spec-motor.md#configuración-de-audio).

## Pendientes

- [ ] El usuario confirma la configuración
- [ ] Inventario: frecuencias y tamaños de período que acepta ALSA en la RGB20S
- [ ] Medir latencia y xruns con 128, 256 y 512 frames
- [ ] Verificar el mecanismo de flush-to-zero en Rust para aarch64
