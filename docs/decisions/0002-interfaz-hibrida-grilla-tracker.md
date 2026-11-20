# 0002 — Interfaz híbrida: grilla de pasos y vista tracker sobre los mismos datos

- **Fecha:** 2026-11-19
- **Estado:** aceptada (por el usuario, 2026-11-19)

> Nota: con el modelo de cadenas por pista ([ADR 0006](0006-cadenas-por-pista.md)), la pantalla "Patrón" pasa a llamarse
> **Grilla** (muestra las frases de las 8 pistas en la sección actual) y la pantalla "Pista" pasa a llamarse **Frase**.
> L2/R2 pasan a moverse por el nivel superior (sección, cadena o canción). El esquema de botones de esta decisión no cambia.

## Contexto

El groovebox se maneja con cruz, 4 botones frontales, 4 gatillos, Start/Select y 2 sticks, en una pantalla de 640×480 sin táctil
(ver [hardware](../hardware/rgb20s.md)). Hay dos familias de interfaz probadas en hardware chico:
los **trackers verticales** (LSDJ, M8, LGPT), que muestran todo el detalle de una pista por filas, y los **step sequencers**
(Elektron, Pocket Operators), que muestran el ritmo de todas las pistas de un vistazo. El usuario principal está aprendiendo
y quiere hacer beats rápido, pero también controlar el detalle. Análisis en [referentes](../solutions/groovebox/referentes.md).

## Alternativas consideradas

| Opción | A favor | En contra |
|---|---|---|
| Tracker puro (estilo LSDJ/M8) | Muy potente, probado con pocos botones, denso | Curva de aprendizaje alta; el ritmo de todas las pistas no se ve junto |
| Grilla pura (estilo Elektron) | Intuitiva para ritmos; se ve todo el patrón | Sin teclas de paso ni encoders, editar el detalle por paso es lento y oculto |
| **Híbrida: grilla + tracker sobre los mismos datos** | Lo rápido de la grilla para el ritmo y lo preciso del tracker para el detalle; las dos se aprenden por separado | Dos vistas para mantener sincronizadas; más trabajo de interfaz |

## Decisión

Interfaz con una pantalla **Patrón** en grilla (8 pistas × 16 pasos por página) y una pantalla **Pista** en formato tracker (una fila por paso,
con columnas de nota, velocidad, largo y, en v1, locks, condición, micro y retrig), que editan el mismo patrón.
El esquema de control es único en toda la app: **A** modificador de edición (A + cruz: ←/→ fino, ↑/↓ grueso), **B** cancelar y, con A, borrar,
**Select** modificador global (Select + cruz cambia de pantalla), **X** escuchar, **Y** seleccionar, **L1/R1** pista, **L2/R2** patrón, **Start** reproducir.
Los valores se muestran en **decimal** (0–127), no en hexadecimal.

## Motivos

- Cubre los dos perfiles de usuario (beatmaker y tracker) sin obligar a ninguno a aprender el paradigma del otro para empezar.
- El gesto "botón + cruz" con paso fino y grueso según el eje es lo que hace usables a LSDJ y M8 con pocos botones.
- Un solo modificador de edición y uno global hacen que lo aprendido en una pantalla sirva en todas.
- Decimal es más fácil de leer para un principiante; 0–127 coincide con MIDI, lo que simplifica p-locks, sticks y CC.

## Consecuencias

- La implementación necesita un modelo de cursor y de selección compartido por las dos vistas.
- Hay que validar en la consola real la ergonomía de los botones (la cruz está debajo del stick izquierdo) y la legibilidad de la fuente (hito H1).
- Los valores decimales ocupan 3 caracteres en lugar de 2: la vista Pista tiene menos columnas que un tracker hexadecimal.
- Detalle completo en [spec-ui](../solutions/groovebox/spec-ui.md).

## Pendientes

- [x] El usuario confirma la interfaz híbrida y el esquema de botones (2026-11-19)
- [ ] Probar el esquema en la consola (hito H1) y ajustar
- [ ] Decidir entre borrar con A + B o con un toque de A sobre un paso con nota (pregunta abierta en spec-ui)
