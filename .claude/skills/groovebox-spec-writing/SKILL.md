---
name: groovebox-spec-writing
description: Método y plantillas para investigar referentes y escribir las specs del groovebox (producto, interfaz con mockups ASCII de 640x480 y mapa de controles, motor con presupuesto de CPU, formatos). Usar al crear o ampliar documentos en docs/solutions/groovebox/.
---

# Escribir la spec del groovebox

## Ficha de referente (`referentes.md`)

Para cada referente (LSDJ, M8, Polyend Tracker, LGPT, Digitakt, OP-Z, Pocket Operators...):

| Campo | Contenido |
|---|---|
| Paradigma | tracker vertical / step sequencer / híbrido |
| Unidad de composición | fila, paso, frase, cadena, patrón, canción |
| Input | qué controles tiene y cómo se edita con pocos botones |
| Ideas a tomar | 2 a 4 principios concretos |
| Ideas a evitar | qué no encaja con la RGB20S y por qué |
| Fuentes | manual oficial, reviews |

Extraer **principios**, no copiar interfaces ni nombres propietarios.

## Priorización

Cada feature lleva: **prioridad** (MVP / v1 / después) · **justificación** · **costo estimado** (bajo, medio o alto, en implementación y en CPU) · **dependencias**.
El MVP es lo mínimo para **hacer un beat de 4 compases y guardarlo**.

## Mockups (`spec-ui.md`)

- Grilla de 80×30 caracteres, que aproxima 640×480 con una fuente de 8×16. Dibujar dentro de un bloque de código.
- Cada pantalla tiene: propósito, mockup, **mapa de controles** y transiciones a otras pantallas.

Mapa de controles por pantalla:

| Control | Solo | + A (o el modificador elegido) | + Select |
|---|---|---|---|
| D-pad | mover cursor | cambiar valor | ... |
| L1 / R1 | ... | | |
| L2 / R2 | ... | | |
| Stick izquierdo / derecho | ... | | |

Reglas: un modificador consistente en toda la app, edición rápida con "botón + d-pad" (estilo tracker), y que nada importante requiera más de dos botones a la vez.

## Motor (`spec-motor.md`)

- Modelo de datos con tipos y rangos (Proyecto → Canción → Cadena → Patrón → Pista → Paso → Instrumento), como pseudocódigo o structs de Rust ilustrativos.
- **Presupuesto de CPU:** tabla de bloques (voz de sample, voz sustractiva, voz FM, filtro, delay, reverb) con costo estimado por muestra, y el total para la configuración del MVP, contra un objetivo menor al 50 % de un núcleo del A35. Las estimaciones van marcadas como tales hasta que `device-ops` las mida.
- Sample rate y buffer propuestos, polifonía máxima y voice stealing.

## Formatos (`spec-formatos.md`)

Formato de proyecto (por ejemplo RON o TOML más una carpeta de samples), versionado del formato, samples admitidos (WAV 16/24 bits, mono o estéreo) y MIDI de entrada y salida (clock, notas, CC).

## Cierre

Cada documento termina con **Preguntas abiertas** (decisiones para el usuario) y **Fuentes**.
