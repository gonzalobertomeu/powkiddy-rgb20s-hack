# 0018 — Limitador de picos sin anticipación delante del soft clip

- **Fecha:** 2026-11-21
- **Estado:** propuesta

## Contexto
El H9 pide un limitador en el master (spec-producto E4). Desde el H2 el master termina en un **soft clip** (Padé de
`tanh`) que acota la salida a ±1 pero satura: con varias pistas y acordes la suma pasa de 0 dBFS fácil y se oye
distorsión. La spec del motor pone el "limitador con lookahead" en v1 y en el MVP solo "ganancia + soft clip,
limitador sí/no". Restricciones: el A35 (presupuesto chico), nada de latencia extra innecesaria y, si se puede, que
los WAV de los hitos anteriores no cambien cuando no hace falta limitar.

## Alternativas consideradas
| Opción | A favor | En contra |
|---|---|---|
| Solo soft clip (como el H2) | Nada que hacer; barato | Satura con mezclas fuertes; no es un limitador |
| Limitador con anticipación (1–2 ms) en lugar del soft clip | Transparente, sin distorsión en el ataque | Retardo en toda la salida (corre los WAV y la latencia); buscar el máximo en la ventana cuesta más; es de v1 en la spec |
| **Limitador sin anticipación (ataque instantáneo, retención, relajación) delante del soft clip** | Ningún pico pasa de 0 dBFS a la entrada del soft clip; bit a bit igual por debajo; ~8 operaciones por frame; sin latencia | El ataque instantáneo cambia la ganancia de golpe en el pico que lo dispara; la salida limitada queda en −2,2 dBFS (por el soft clip) |
| Limitador sin anticipación en lugar del soft clip | Salida hasta 0 dBFS | Cambia el sonido de todo lo anterior (el soft clip colorea un poco siempre) y con el limitador apagado no queda ninguna protección |

## Decisión
Master = ganancia → **limitador de picos sin anticipación** (umbral 0 dBFS, ataque instantáneo, retención 25 ms,
relajación 120 ms; activable con "limitador SÍ/NO", activo por defecto) → **soft clip** siempre al final.

## Motivos
Es lo más barato que garantiza que la suma no entre en la zona de saturación del soft clip, no agrega latencia y no
toca la señal cuando no limita: `h2-kit` y `h4-swing-voces` salen idénticos byte a byte. La retención de 25 ms evita
que la relajación module la ganancia dentro de cada ciclo de un bajo. El soft clip queda como red de seguridad (con el
limitador apagado la mezcla satura como antes, que también puede ser un recurso).

## Consecuencias
- `h3-beat` y `h5-cancion` cambian: la demo pasaba de 0 dBFS antes del soft clip (pico −1,1 dBFS de salida); ahora el
  limitador la deja en −2,2 dBFS.
- El techo de salida con el limitador es `soft_clip(1) = 0,78` (−2,2 dBFS): ~2 dB menos de volumen máximo.
- Detalle y tests en [dsp.md](../solutions/groovebox/dsp.md#master-ganancia-limitador-y-soft-clip).

## Pendientes
- [ ] Escuchar en la consola con auriculares si el ataque instantáneo se nota en el bombo con la demo a +4 dB (`h9-mezcla.wav`).
- [ ] Si se nota, pasar al limitador con anticipación de la spec (v1) o subir el umbral.
