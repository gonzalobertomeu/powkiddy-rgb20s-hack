# 0013 — Tantas ranuras de fundido como voces en la bolsa (16 en lugar de 4)

- **Fecha:** 2026-11-20
- **Estado:** aceptada (por el usuario, 2026-11-20). Modifica un detalle del [ADR 0007](0007-polifonia-en-el-mvp.md): la bolsa pasa de 16 + 4 a 16 + 16 ranuras de fundido

## Contexto

El ADR 0007 fija una bolsa de 16 voces y **4 ranuras de fundido**: una voz robada se apaga en 5 ms en una ranura y, si no hay ranuras
libres, la que lleva más tiempo fundiéndose se corta en seco ("caso raro"). Al implementar el motor (H2) y correr la prueba de estrés de
la [spec del motor](../solutions/groovebox/spec-motor.md#cómo-medir-para-device-ops-y-rust-dev) (8 pistas con acordes de 4 notas y largo
INF en cada paso) resultó que el caso no es raro: cada paso pide 32 voces, se roban a la vez las 16 que sonaban y solo 4 tienen ranura.
Cada corte en seco es un click.

Medido con `groovebox render` (32 pasos a 120 BPM, `h2-robo.wav`):

| Variante | Cortes en seco |
|---|---|
| 4 ranuras, como el ADR 0007 | 880 |
| 4 ranuras, descartando sin fundido las voces que todavía no sonaron o ya son inaudibles (implementado) | 372 (~12 por paso) |
| 16 ranuras + el descarte anterior | **0** |

Con uso normal (un acorde por paso, percusión mono) no hay cortes con 4 ranuras: el problema aparece con varias pistas polifónicas que
cambian de acorde en el mismo paso, que es justamente lo que el groovebox promueve.

## Alternativas consideradas

| Opción | A favor | En contra |
|---|---|---|
| 4 ranuras (ADR 0007) | Peor caso de CPU de 20 voces | Clicks en cambios de acorde simultáneos en varias pistas |
| 8 ranuras | Menos cortes | Igual quedan cortes en el peor caso (hasta 16 robos por paso) |
| **16 ranuras** (= tamaño de la bolsa) | Nunca hay cortes: como mucho se roban las 16 voces a la vez | Pico instantáneo de 32 voces durante 5 ms |
| Retrasar el inicio de la nota nueva 5 ms mientras se funde la vieja | Sin ranuras extra | Agrega latencia y corre el tiempo de las notas |

## Decisión

Usar **16 ranuras de fundido** (`FADE_SLOTS = POOL_VOICES`), manteniendo el descarte sin fundido de las voces sin señal.

## Motivos

- Elimina los clicks por construcción: en un mismo instante no se pueden robar más voces de las que hay en la bolsa.
- El costo es chico porque un fundido dura 5 ms (menos de un buffer de 256): medido en la Mac, en el peor caso el promedio pasa de 0,70 % a 0,86 %
  y el máximo por bloque de 2,9 % a 3,4 % de un núcleo. La memoria extra son ~12 ranuras de ~150 bytes.
- El presupuesto de la spec sigue holgado: aunque el pico instantáneo sea de 32 voces por unos milisegundos, el promedio de la ventana
  de un buffer apenas cambia.

## Consecuencias

- Cambiar `FADE_SLOTS` en `projects/groovebox/src/engine/pool.rs` y el texto "16 + 4" en spec-motor, spec-producto y ADR 0007 (que quedaría
  reemplazado en ese punto).
- La interfaz mostraría "voces n/32" en lugar de "n/20".
- Hay que medir el pico por bloque en la consola con `groovebox bench` (escenario "bolsa llena") con 4 y con 16 ranuras.

## Pendientes

- [ ] El usuario decide (y, si acepta, se aplica el cambio)
- [ ] Medir en la consola el pico por bloque con 4 y 16 ranuras
