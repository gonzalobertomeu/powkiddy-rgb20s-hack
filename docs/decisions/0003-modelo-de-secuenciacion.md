# 0003 — Patrones globales de 8 pistas monofónicas y canción como lista de patrones

- **Fecha:** 2026-11-19
- **Estado:** reemplazada por [0006](0006-cadenas-por-pista.md) y [0007](0007-polifonia-en-el-mvp.md)

> El usuario no aceptó esta propuesta (2026-11-19): eligió **cadenas por pista** (ADR 0006) y **polifonía en el MVP** (ADR 0007).
> Se conserva como registro de la alternativa descartada.

## Contexto

Hay que definir cómo se organiza la música en el groovebox, porque condiciona el modelo de datos, el formato de archivo,
el motor y casi todas las pantallas. Los referentes usan dos modelos: **patrón global** (todas las pistas avanzan juntas:
Polyend Tracker, Elektron, Circuit) y **cadenas por pista** (cada pista sigue su propia lista de frases: LSDJ, M8).
También hay que decidir si los sonidos (instrumentos) pertenecen al proyecto o a cada patrón, y si las pistas son
monofónicas o polifónicas. El MVP tiene que ser alcanzable por alguien que aprende Rust.

## Alternativas consideradas

| Opción | A favor | En contra |
|---|---|---|
| **Patrón global + canción como lista de (patrón, repeticiones)** | Modelo mental simple; una sola posición de reproducción; fácil de dibujar y de implementar | Menos flexible: para variar una sola pista hay que copiar el patrón entero |
| Cadenas por pista (tracker) | Reutilización máxima; pistas de largos distintos | Las pistas pueden desalinearse; más pantallas (frase, cadena, canción); más difícil de aprender |
| Kits por patrón (sonidos cambian con el patrón) | Variación tímbrica entre partes | Duplica datos; confunde al cambiar de patrón; más complejidad en el motor |
| **Instrumentos por proyecto (uno por pista)** | Simple; las variaciones por paso van con p-locks (v1) | Para cambiar un sonido en una parte hay que usar locks |
| **Pistas monofónicas** (con ranura de fundido) | Motor simple, CPU predecible, sin robo de voces en el MVP | No hay acordes en el MVP |
| Pistas polifónicas desde el MVP | Acordes | Robo de voces y más estados en el primer motor |

## Decisión

- **8 pistas**; cada pista tiene **un instrumento** que pertenece al proyecto.
- **Patrón global** de 1 a 64 pasos (semicorcheas, 24 ticks por paso); hasta 64 patrones por proyecto.
- **Canción** = lista de hasta 128 filas `(patrón, repeticiones)`; transposición por fila en v1.
- Pistas **monofónicas** en el MVP, con una segunda ranura de voz para el fundido anti-clic; polifonía de sinte (bolsa de 16 voces) en v1.
- El modelo de datos incluye desde el formato 1 los campos de v1 por paso: 2 p-locks, condición, microtiming y retrig.

## Motivos

- Es el modelo más fácil de entender y de implementar, y alcanza para el objetivo del MVP (un beat de 4 compases y una canción simple).
- Las variaciones que el modelo de cadenas resuelve con reutilización se cubren en v1 con p-locks, condiciones de paso y largo por pista.
- Las pistas monofónicas siguen la práctica de M8 y Elektron, y dejan el presupuesto de CPU muy holgado ([spec-motor](../solutions/groovebox/spec-motor.md#presupuesto-de-cpu)).
- Reservar los campos de v1 en el formato evita migraciones de archivos.

## Consecuencias

- Las pantallas Patrón, Pista y Canción se basan en este modelo ([spec-ui](../solutions/groovebox/spec-ui.md)).
- Cadenas por pista, lanzamiento de patrones por pista y kits por patrón quedan para "después"; si se agregan, requieren un ADR nuevo y probablemente una versión nueva del formato.
- Un proyecto completo ocupa ~512 KB en memoria; el deshacer por snapshots (v1) es viable.

## Pendientes

- [ ] El usuario confirma el modelo (patrón global, instrumentos por proyecto, pistas monofónicas en el MVP)
- [ ] Revisar con `dsp-engineer` el diseño de la ranura de fundido y la bolsa de voces de v1
