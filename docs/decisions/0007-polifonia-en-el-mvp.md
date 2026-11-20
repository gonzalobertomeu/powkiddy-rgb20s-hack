# 0007 — Polifonía en el MVP: bolsa de voces compartida y acordes por paso

- **Fecha:** 2026-11-19
- **Estado:** aceptada (por el usuario, 2026-11-19: polifonía en el MVP y columna ACORDE con tipos predefinidos e inversión). Los detalles que eran propuesta del agente también quedan aceptados (por el usuario, 2026-11-20)
- Reemplaza a [0003](0003-modelo-de-secuenciacion.md) en lo que respecta a pistas monofónicas

## Contexto

El ADR 0003 proponía pistas monofónicas en el MVP y polifonía en v1. El usuario quiere **polifonía desde el MVP**.
Hay que definir cómo se reparten las voces entre pistas, qué pasa cuando se acaban, y cómo se escribe un acorde con la cruz,
en una frase de tracker y en la grilla. Restricciones: CPU del A35 (objetivo < 50 % de un núcleo), interfaz sin teclado ni pantalla táctil,
y que lo pueda implementar alguien que aprende Rust.

## Alternativas consideradas

| Opción | A favor | En contra |
|---|---|---|
| Voces fijas por pista (por ejemplo, 4 cada una) | Simple; sin robo entre pistas | Desperdicia voces (la percusión no las usa); 32 voces de peor caso |
| **Bolsa global de voces + límite por pista** | Aprovecha las voces; costo máximo acotado | Hay que robar voces con reglas claras |
| Acordes con varias columnas de nota por paso (NOTA 1–4) | Cualquier voicing | Frase muy ancha en 640 px; lento de escribir con la cruz |
| **Columna ACORDE (tipo + inversión) sobre la nota raíz** | Una sola columna; rápido con la cruz; legible | Solo acordes predefinidos (los libres quedan para v1) |
| Solo polifonía por superposición (sin acordes) | Mínimo | No permite acordes en un paso |

## Decisión

- **Bolsa global de 16 voces** más **4 ranuras de fundido** para las voces robadas (máximo instantáneo: 20 voces). **Modificado por el [ADR 0013](0013-ranuras-de-fundido.md): 16 ranuras de fundido.**
- Cada pista tiene un parámetro **voces** (1–8). Con 1, la pista es monofónica: una nota nueva corta la anterior con fundido. Con más de 1, las notas se superponen hasta que termina su largo (gate).
- **Robo de voces:** primero, si la pista supera su límite, la voz más vieja de esa pista; si la bolsa está llena, la voz más vieja en relajación, luego la más vieja de la pista que más voces usa, luego la más vieja de todas. Nunca se roban notas del mismo acorde recién disparado mientras haya otra opción. La voz robada se apaga en 5 ms.
- **Acordes:** cada paso tiene una columna **ACORDE** con tipo (`5`, `8va`, `may`, `men`, `dis`, `aum`, `sus2`, `sus4`, `may7`, `men7`, `dom7`, `m7b5`, `add9`) e inversión (0–3), con un máximo de 4 notas. Se edita con **A + L1/R1** sobre la nota (en la Grilla y en Frase), o en la propia columna.
- Valores por defecto: percusión 1 voz, sampler 2, sustractivo 4 (bajo: 1).

## Motivos

- Es la elección del usuario. La bolsa compartida es la práctica habitual en sintes multitímbricos, y acota el peor caso de CPU.
- Con 20 voces sustractivas el peor caso estimado es de ~24 % de un núcleo, todavía por debajo del 50 % ([presupuesto](../solutions/groovebox/spec-motor.md#presupuesto-de-cpu)).
- La columna ACORDE es la forma más rápida de escribir acordes con cruz y botones, y ocupa una sola columna.

## Consecuencias

- El motor necesita la bolsa de voces desde el primer secuenciador (H3), aunque al principio solo la use la percusión. Los acordes y el robo completo entran con el sustractivo (H8).
- La Grilla dibuja con otro símbolo los pasos con acorde.
- Los acordes libres (intervalos editables) y el rasgueo (strum) quedan para v1.
- Hay que probar el robo de voces con tests (secuencias que agoten la bolsa) y medir el peor caso en la consola.

## Pendientes

- [x] Columna ACORDE con tipos predefinidos e inversión: decidido por el usuario (2026-11-19)
- [x] Tamaño de la bolsa: 16 voces + 16 ranuras de fundido (ADR 0013, 2026-11-20)
- [ ] Revisar con `dsp-engineer` las reglas de robo y el fundido
- [ ] Medir en la consola el peor caso (20 voces sustractivas + efectos)
