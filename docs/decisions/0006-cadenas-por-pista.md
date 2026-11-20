# 0006 — Canción de cadenas por pista: canción → cadenas → frases

- **Fecha:** 2026-11-19
- **Estado:** aceptada (por el usuario, 2026-11-19). También decididos por el usuario (2026-11-19): instrumento fijo por pista, regla de bloques y creación implícita desde la Grilla. Los detalles que eran propuesta del agente también quedan aceptados (por el usuario, 2026-11-20)
- Reemplaza a [0003](0003-modelo-de-secuenciacion.md) en lo que respecta a la estructura de la canción

## Contexto

Había que elegir cómo se organiza la música. El [ADR 0003](0003-modelo-de-secuenciacion.md) proponía un patrón global
(todas las pistas avanzan juntas). El usuario prefirió el modelo de los trackers portátiles (LSDJ, M8, LGPT): **cada pista
recorre su propia lista de cadenas**, y cada cadena es una lista de frases. Este modelo también tiene que convivir con la
vista Grilla, ya aceptada en el [ADR 0002](0002-interfaz-hibrida-grilla-tracker.md).

## Alternativas consideradas

| Opción | A favor | En contra |
|---|---|---|
| Patrón global + canción como lista de patrones (ADR 0003) | Simple; una sola posición de reproducción | Para variar una pista hay que copiar el patrón entero; poca reutilización |
| **Cadenas por pista** | Máxima reutilización (la misma frase o cadena en muchos lugares, con transposición); cada pista evoluciona por su cuenta | Las pistas pueden desalinearse; más pantallas y conceptos; más difícil de implementar |

## Decisión

- **Frase:** 16 pasos (semicorcheas) de una pista. Hasta 256 frases por proyecto (000–255), compartidas por todas las pistas.
- **Cadena:** lista de hasta 16 entradas `(frase, transposición)`. Hasta 256 cadenas (000–255). La primera entrada vacía termina la cadena.
- **Canción:** hasta 256 filas; cada fila tiene, para cada una de las 8 pistas, una cadena o nada.
- **Reproducción:** cada pista tiene su propio cabezal. Al terminar una cadena, la pista pasa a la fila siguiente **de su columna**. Al encontrar una celda vacía, vuelve a la primera fila de su **bloque** (las celdas no vacías contiguas en las que estaba).
- **Grilla:** muestra, para una **sección** `(fila R, entrada k)`, la frase que cada pista tiene en la entrada `k` de su cadena de la fila `R`. Si la pista no tiene cadena o frase en ese lugar, poner una nota en la grilla las **crea de forma implícita** (cadena y frase nuevas).
- Una pista tiene **un instrumento fijo**, sin columna de instrumento por paso (decidido por el usuario, 2026-11-19).
- Se mantienen las decisiones del 0003 que no dependen de la estructura: 8 pistas, 24 ticks por paso y los campos de v1 por paso (p-locks, condición, micro, retrig) ya incluidos en el formato 1. La parte de polifonía la reemplaza el [ADR 0007](0007-polifonia-en-el-mvp.md).

## Motivos

- Es la elección del usuario, y es el modelo probado en las portátiles con cruz y botones.
- La reutilización con transposición permite canciones largas con pocos datos.
- La Grilla y la creación implícita mantienen el flujo rápido de una caja de ritmos: se puede hacer un beat sin abrir Cadena ni Canción.
- El esquema de "bloques" permite dejar secciones aisladas en filas de abajo para experimentar sin romper la canción (práctica tomada del M8).

## Consecuencias

- Pantallas: Canción (8 columnas de cadenas), **Cadena** (nueva), Grilla y Frase ([spec-ui](../solutions/groovebox/spec-ui.md)).
- El motor necesita 8 cabezales independientes ([spec-motor](../solutions/groovebox/spec-motor.md#secuenciador)).
- Riesgo de desalineación cuando las cadenas de una fila tienen largos distintos: la pantalla Canción muestra el largo de cada cadena y avisa.
- El MVP crece: Cadena y Canción entran en un hito propio (H5) y el objetivo "beat de 4 compases guardado" pasa a cumplirse en H6 ([hitos](../solutions/groovebox/spec-producto.md#hitos)).
- Hacen falta gestos nuevos: **clonar** (A + Y) y **número nuevo libre** (doble toque de A).
- Para el aprendizaje de Rust es buena ocasión para ver por qué se usan **IDs numéricos** en lugar de referencias compartidas.

## Pendientes

- [x] Regla de bloques: decidido por el usuario (2026-11-19)
- [x] Creación implícita desde la Grilla: decidido por el usuario (2026-11-19)
- [x] Instrumento fijo por pista, sin columna INST: decidido por el usuario (2026-11-19)
- [ ] El usuario confirma los detalles restantes: frases de 16 pasos fijos, rangos 000–255, transposición por entrada de cadena en el MVP
- [ ] Decidir si el largo de frase variable (1–16) entra en el MVP o en v1
