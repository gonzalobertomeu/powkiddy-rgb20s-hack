# Groovebox — spec de producto

Última revisión: 2026-11-19 · Estado: borrador, con decisiones del usuario incorporadas · Volver al [índice](README.md)

## Visión

Un **groovebox de bolsillo** que corre en la RGB20S: se prende, ya hay un kit que suena, y en pocos minutos
se arma un beat de varios compases con la cruz y los botones, sin pantalla táctil ni computadora.
Combina la **rapidez de una caja de ritmos** (grilla de pasos) con la **estructura de un tracker portátil**
(canción → cadenas por pista → frases, al estilo de LSDJ y M8), es **polifónico** (acordes y notas que se superponen)
y usa los **dos sticks** como controles expresivos para tocar en vivo.

Nombre de trabajo: **"groovebox"** (carpeta `projects/groovebox/`). El nombre definitivo es una pregunta abierta.

## Decisiones del usuario (2026-11-19)

| Tema | Decisión | Registro |
|---|---|---|
| Interfaz | Híbrida: grilla + tracker sobre los mismos datos | [ADR 0002](../../decisions/0002-interfaz-hibrida-grilla-tracker.md) (aceptada) |
| Estructura de la música | **Cadenas por pista**: canción → cadenas → frases | [ADR 0006](../../decisions/0006-cadenas-por-pista.md) (aceptada; reemplaza a 0003) |
| Polifonía | **En el MVP** | [ADR 0007](../../decisions/0007-polifonia-en-el-mvp.md) (aceptada; reemplaza a 0003) |
| P-locks | En **v1** | Este documento (S9) |

## Usuarios

| Perfil | Qué necesita | Implicancia |
|---|---|---|
| **El autor del proyecto** (usuario principal): músico amateur que aprende Rust | Un instrumento que pueda usar y, a la vez, construir por etapas chicas | El MVP tiene que dividirse en hitos que suenen desde el principio |
| Beatmaker portátil (fan de Pocket Operators o Elektron) | Armar ritmos rápido, sin manual | Grilla con creación implícita, kit por defecto, controles visibles en pantalla |
| Usuario de trackers (LSDJ, M8, LGPT) | Canción, cadenas y frases; detalle por paso; edición sin soltar la cruz | Pantallas Canción, Cadena y Frase con "botón + cruz" y atajos consistentes |

## Principios de diseño

1. **Suena al instante.** Un proyecto nuevo trae 8 pistas con sonidos sintetizados listos (no hacen falta samples), y cada pista ya tiene una cadena y una frase vacías.
2. **La estructura no estorba.** Se puede hacer un beat entero en la Grilla sin abrir Canción ni Cadena: lo que falta se crea solo.
3. **Dos botones como máximo.** Nada importante necesita más de dos botones a la vez. El modificador de edición (A) y el global (Select) son los mismos en todas las pantallas.
4. **Una pantalla, una tarea.** Canción para el arreglo, Grilla para el ritmo, Cadena para el orden de frases de una pista, Frase para el detalle, Instrumento para el sonido, Mezcla para el balance.
5. **Siempre se ve dónde estás.** Cursor, pista, fila, sección, frase y valor en edición siempre visibles (lección del Deluge). Las desalineaciones entre pistas se señalan.
6. **Nunca perder trabajo.** Guardado atómico con respaldo; autoguardado en v1.
7. **Margen de CPU.** El motor no pasa del 50 % de un núcleo del A35 en el peor caso configurado.
8. **Propio, no clon.** Se toman principios de los [referentes](referentes.md), no interfaces ni nombres.

## Alcance

- **MVP:** lo mínimo para **hacer un beat de 4 compases (una cadena de 4 frases por pista) con batería, bajo, acordes y un sample, mezclarlo y guardarlo**, y armar una canción simple con varias filas.
- **v1:** lo que lo convierte en un instrumento expresivo: p-locks, condiciones de paso, FM, MIDI, exportar a WAV, deshacer, autoguardado.
- **Después:** ideas valiosas pero caras o que dependen de hardware sin verificar.

## Flujos principales

### F1 — Primer beat (objetivo: menos de 2 minutos)
1. Abrir la app → se carga el último proyecto o uno nuevo con el kit por defecto.
2. Pantalla **Grilla**: mover el cursor con la cruz y apretar **A** en los pasos 1, 5, 9 y 13 de la pista BOMBO.
3. **Start** → suena en loop (la canción tiene una sola fila con cadenas de una frase). Agregar caja y hi-hats mientras suena.

### F2 — De 1 compás a 4
1. En la Grilla, **R2** pasa a la sección 2. Las pistas no tienen frase ahí: poner una nota crea la frase y la agrega a la cadena de la pista.
2. Para repetir el compás 1 en la sección 2 sin copiar: en la columna **FRA**, **A** inserta la última frase usada (la misma, compartida).
3. Para variarla: **A + Y** en FRA clona la frase y se edita la copia (por ejemplo, un fill).
4. **Select + Start** reproduce en loop solo la sección actual; **Start** reproduce la fila completa (4 compases).

### F3 — Bajo y acordes
1. **L1/R1** hasta la pista BAJO; **Select + →** dos veces abre **Frase** (pasando por Cadena).
2. **A** en una fila vacía inserta la última nota usada; **A + ←/→** cambia de a un semitono, **A + ↑/↓** de a una octava. **X** escucha la nota.
3. En la pista SINTE (4 voces), sin soltar A, **L1/R1** recorren los tipos de acorde: `men7`, `may`, `dom7`… Con un LARGO de 8 pasos, los acordes se superponen y suenan sostenidos.

### F4 — Ajustar el sonido
1. **Select + →** desde Frase abre **Instrumento**.
2. Cruz para elegir el parámetro, **A + cruz** para cambiarlo. El cambio se oye en tiempo real.
3. **Stick izquierdo** para barrer el filtro en vivo sin cambiar el valor guardado.

### F5 — De un beat a una canción
1. **Select + ←** hasta **Canción**. La fila 00 tiene una cadena por pista.
2. En la fila 01: **A** en cada celda inserta la última cadena usada (repite la parte); **A + Y** clona una cadena para variarla (por ejemplo, otra cadena de bajo con transposición +5 en **Cadena**).
3. Dejar una celda vacía en una pista la "apaga" en esa fila solo si su bloque termina ahí; para silenciar manteniendo la alineación, usar una cadena con frases vacías. La columna "largo de cadenas" avisa las filas desparejas.
4. **Start** en Canción reproduce desde la fila del cursor.

### F6 — Usar un sample
1. En **Instrumento**, cambiar el tipo de la pista a SAMPLE.
2. **A** sobre "archivo" abre el **Navegador**. **X** escucha antes de elegir; **A** elige.
3. El WAV se copia a la carpeta del proyecto (ver [spec-formatos](spec-formatos.md#proyecto)).

### F7 — Guardar y retomar
1. **Select + ↑** abre **Proyecto**. "Guardar" (o "Guardar como", con el editor de nombres por cruz).
2. Al reabrir la app, se carga el último proyecto abierto.

### F8 — Tocar en vivo
Con la canción o una sección sonando: **stick izquierdo** = filtro de la pista actual; **stick derecho** = envíos a delay y reverb;
**Select + X / Select + Y** = mute y solo; **L2/R2** en la Grilla = moverse de sección (con "seguir reproducción", la Grilla acompaña a los cabezales).

## Features priorizadas

Costo: **impl.** = esfuerzo de implementación; **CPU** = carga en el hilo de audio. Bajo, medio o alto.

### Secuenciador

| ID | Feature | Prioridad | Justificación | Impl. | CPU | Depende de |
|---|---|---|---|---|---|---|
| S1 | 8 pistas; **frases** de 16 pasos de semicorchea (hasta 256 por proyecto) | MVP | Unidad básica del modelo de cadenas ([ADR 0006](../../decisions/0006-cadenas-por-pista.md)) | medio | bajo | — |
| S2 | Play/stop, tempo de 40 a 300 BPM; Start contextual y loop de sección (Select + Start) | MVP | Escuchar lo que se edita, en su contexto | medio | bajo | S1 |
| S3 | Swing global (50–75 %) | MVP | Muy barato y cambia mucho el groove | bajo | bajo | S2 |
| S4 | Nota, velocidad y largo (gate) por paso | MVP | Sin velocidad no hay dinámica; sin largo no hay notas sostenidas ni acordes que se superpongan | medio | bajo | S1 |
| S5 | **Cadenas**: hasta 256, de hasta 16 frases, con **transposición por entrada** | MVP | Reutilizar frases y armar partes de varios compases (decisión del usuario) | medio | bajo | S1 |
| S6 | **Canción**: hasta 256 filas × 8 pistas de cadenas; un cabezal por pista; regla de bloques | MVP | Arreglo con pistas independientes (decisión del usuario) | alto | bajo | S5 |
| S7 | **Grilla con secciones** y creación implícita de cadenas y frases | MVP | Mantiene el flujo rápido de caja de ritmos sobre el modelo de cadenas | medio | — | S5, S6 |
| S8 | **Clonar** (A + Y) y número nuevo (doble toque de A); marca `*` de reutilización | MVP | Sin clonar, editar una frase compartida cambia todos sus usos | bajo | — | S5 |
| S9 | **P-locks**: 2 por paso, cualquier parámetro del instrumento o de la mezcla | v1 | Confirmado por el usuario; el modelo de datos ya los contempla | medio | bajo | S4, M1–M3 |
| S10 | **Condiciones de paso**: probabilidad, A de B, fill, primera vez, anterior (y negaciones) | v1 | Variación sin escribir más frases | medio | bajo | S4 |
| S11 | Microtiming (±1/2 paso en 24 ticks) y retrig (2 a 8 repeticiones con caída de velocidad) | v1 | Grooves humanos, redobles y hi-hats rápidos | medio | bajo | S4 |
| S12 | Largo de frase variable (1–16 pasos) | v1 | Compases impares y polimetría | bajo | bajo | S1 |
| S13 | Mute y solo por pista | MVP | Esencial para escuchar y tocar en vivo | bajo | bajo | — |
| S14 | Selección, copiar, cortar y pegar (pasos, entradas de cadena, celdas de canción) | MVP | Sin esto, armar variaciones es tedioso | medio | — | S1 |
| S15 | Modo fill (mantener un botón) | v1 | Complementa las condiciones de paso | bajo | bajo | S10 |
| S16 | Deshacer y rehacer (Select + B), al menos 32 niveles | v1 | Seguridad al editar; snapshots del proyecto (~0,1 MB c/u) | medio | — | — |
| S17 | Grabación en vivo cuantizada (tocar notas con los botones mientras suena) | v1 | Más rápido que paso a paso para melodías | medio | bajo | S4 |
| S18 | Modo "live": lanzar cadenas por pista en vivo, con cuantización | después | Muy útil en vivo; necesita diseño de gestos | alto | bajo | S6 |
| S19 | Arpegiador por pista | después | Aprovecha la polifonía; no es esencial | medio | bajo | M5 |
| S20 | Grabar movimientos de los sticks como locks (motion) | después | Muy expresivo; necesita locks continuos | alto | bajo | S9, U3 |

### Sonido

| ID | Feature | Prioridad | Justificación | Impl. | CPU | Depende de |
|---|---|---|---|---|---|---|
| M1 | **Percusión sintetizada**: bombo, caja, hi-hat (cerrado/abierto), palmas, tom | MVP | Suena sin archivos; es el primer sonido del primer hito | bajo | bajo | — |
| M2 | **Sampler** one-shot: afinación, inicio, fin, filtro, envolvente AD | MVP | Lo más versátil y lo más barato en CPU | medio | bajo | A2 |
| M3 | **Sustractivo**: oscilador (sierra, cuadrada, pulso, triángulo) + sub + ruido, SVF LP/BP/HP por voz, 2 envolventes | MVP | El motor de bajos, acordes y leads | medio | medio | M5 |
| M4 | **FM de 2 operadores** | v1 | Muy barato; suma campanas, bajos FM y metales | bajo | bajo | M5 |
| M5 | **Polifonía**: bolsa de 16 voces + 4 de fundido, parámetro "voces" (1–8) por pista, robo de voces | MVP | Decisión del usuario ([ADR 0007](../../decisions/0007-polifonia-en-el-mvp.md)); acota el peor caso de CPU | medio | medio | — |
| M6 | **Acordes** por paso: columna ACORDE (tipo + inversión), edición con A + L1/R1 | MVP | Escribir acordes con la cruz sin ensanchar la frase | bajo | bajo | M5 |
| M7 | Acorde libre (intervalos editables) y rasgueo | v1 | Voicings propios y acordes "tocados" | bajo | bajo | M6 |
| M8 | Sampler: loop, reversa, rebanadas (slices) | v1 | Loops de batería y chops | medio | bajo | M2 |
| M9 | LFO por instrumento (destino seleccionable) | v1 | Movimiento sin p-locks | bajo | bajo | M1–M4 |
| M10 | Glide (portamento) en el sustractivo con 1 voz | v1 | Bajos tipo acid | bajo | bajo | M3 |
| M11 | FM de 4 operadores y wavetable | después | Interfaz compleja; hay alternativas más simples | alto | medio | — |
| M12 | Grabar samples desde una entrada | después | La RGB20S no tendría entrada de línea ni micrófono **(sin verificar)**; quizás por interfaz de audio USB | alto | bajo | hardware |

### Efectos y mezcla

| ID | Feature | Prioridad | Justificación | Impl. | CPU | Depende de |
|---|---|---|---|---|---|---|
| E1 | Mezcla por pista: volumen, paneo, envío a delay y a reverb; medidor de voces | MVP | Balance básico; los envíos ahorran CPU frente a efectos por pista | bajo | bajo | — |
| E2 | Delay estéreo sincronizado al tempo (envío) | MVP | Efecto más usado en grooveboxes; muy barato | bajo | bajo | E1 |
| E3 | Reverb liviana (envío) | MVP | Da espacio; versión barata (FDN chico) | medio | medio | E1 |
| E4 | Limitador / soft clip en el master | MVP | Con polifonía, la suma de voces satura más fácil; protege los oídos con auriculares | bajo | bajo | — |
| E5 | Overdrive y bitcrusher por pista | v1 | Carácter con muy poco costo | bajo | bajo | — |
| E6 | Compresor master con sidechain desde una pista | después | Típico en house/techno; más caro y complejo de ajustar | medio | medio | — |
| E7 | Efectos de performance (stutter, beat-repeat) | después | Divertido en vivo; necesita buffers y diseño de gestos | medio | bajo | — |
| E8 | Chorus | después | Menos prioritario que delay y reverb | bajo | bajo | — |

### Interfaz

| ID | Feature | Prioridad | Justificación | Impl. | CPU | Depende de |
|---|---|---|---|---|---|---|
| U1 | Pantallas: Canción, Grilla, Cadena, Frase, Instrumento, Mezcla, Proyecto, Navegador | MVP | El conjunto mínimo para los flujos F1–F8 ([spec-ui](spec-ui.md)) | alto | — | — |
| U2 | Audición con X (escuchar nota, acorde, frase o instrumento, aun sin reproducir) | MVP | Feedback inmediato al editar | bajo | bajo | — |
| U3 | Sticks: filtro de la pista actual y envíos (performance, no se graba) | MVP | Diferencial de la consola; barato | bajo | bajo | E1 |
| U4 | Línea de ayuda contextual y minimapa de pantallas | MVP | Reduce la curva de aprendizaje, más necesaria con cadenas | bajo | — | — |
| U5 | Medidor de CPU, voces en uso, xruns y batería | MVP | Hace falta para medir en el hardware | bajo | — | — |
| U6 | Archivo de configuración con el mapeo de botones y la calibración de los sticks | MVP | El mapeo de SDL en la RGB20S está sin verificar | bajo | — | — |
| U7 | Aviso de filas desparejas en Canción ("largo de cadenas") | MVP | Mitiga el principal problema del modelo de cadenas | bajo | — | S6 |
| U8 | Pantalla de ayuda completa (mantener Select) | v1 | Referencia rápida de atajos | bajo | — | — |
| U9 | Visualización de envolventes y forma de onda del filtro | v1 | "Mostrar el sonido" (idea del OP-1) | medio | — | — |
| U10 | Temas de color y tamaño de fuente | después | Comodidad; la legibilidad base se resuelve en el MVP | bajo | — | — |

### Archivos, MIDI y sistema

| ID | Feature | Prioridad | Justificación | Impl. | CPU | Depende de |
|---|---|---|---|---|---|---|
| A1 | Guardar y cargar proyectos (RON) | MVP | Objetivo del MVP ([ADR 0004](../../decisions/0004-formato-de-proyecto-ron.md)) | medio | — | — |
| A2 | Cargar WAV y copiarlo a la carpeta del proyecto | MVP | Proyectos autocontenidos y portables | medio | — | — |
| A3 | Guardado atómico con `.bak` | MVP | La batería se puede agotar en cualquier momento | bajo | — | A1 |
| A4 | Autoguardado periódico | v1 | Protección extra; requiere guardar sin cortar el audio | bajo | — | A1 |
| A5 | Exportar la canción o la sección a WAV (render offline) | v1 | Compartir lo hecho sin cables | medio | — | — |
| A6 | Exportar pistas separadas (stems) | después | Para terminar en un DAW | bajo | — | A5 |
| A7 | Presets de instrumentos (guardar y cargar un instrumento suelto) | v1 | Reutilizar sonidos entre proyectos | bajo | — | A1 |
| I1 | MIDI clock de salida y entrada (USB host) | v1 | Sincronizar con otros equipos | medio | bajo | hardware |
| I2 | Notas MIDI de entrada → pista actual (polifónicas) | v1 | Tocar con un teclado USB | medio | bajo | I1, M5 |
| I3 | Pista de tipo "MIDI out" | v1 | Secuenciar sintes externos | medio | bajo | I1 |
| I4 | Mapeo de CC de entrada y salida | después | Controladores externos | medio | bajo | I2 |
| I5 | Consola como dispositivo MIDI USB (gadget) | después | Depende de que el USB-C soporte modo periférico **(sin verificar)** | alto | bajo | hardware |
| Y1 | Lanzarla desde el menú de dArkOS (sección Ports) | MVP | Usarla sin SSH | bajo | — | infra |
| Y2 | Sistema "appliance" que arranca directo en la app | después | Depende de la imagen propia (blueprint, paso 8) | alto | — | imagen |

## Hitos

Pensados para que el usuario aprenda Rust de a poco y que **cada hito termine con algo que suena o se ve**.
Los hitos H0 a H6 cumplen el objetivo del MVP con percusión sintetizada; H7 a H11 lo completan.
Respecto de la primera versión de la spec, las cadenas suman un hito (H5) y la polifonía se reparte entre H3 (la bolsa) y H8 (acordes y robo completo).

| Hito | Contenido | Resultado visible | Conceptos de Rust que aparecen | Agentes |
|---|---|---|---|---|
| **H0** | "Hola mundo": ventana SDL2 de 640×480 y un tono con `cpal` (blueprint, paso 3) | Un tono en la Mac y en la consola | `cargo`, crates, `Result` y `?`, closures | rust-dev, dev-infra |
| **H1** | Input y pantalla: leer cruz, botones, gatillos y sticks; fuente bitmap; dibujar la grilla y mover el cursor. **Prueba de legibilidad y de ergonomía en la consola** | La grilla vacía se mueve con la cruz | `struct`, `enum`, `match`, `impl` | rust-dev, device-ops |
| **H2** | Motor mínimo: hilo de audio con cola `rtrb`; un bombo sintetizado que suena con X | Apretar X y escuchar un bombo | hilos, `move`, ownership, `Send` | rust-dev, dsp-engineer |
| **H3** | Secuenciador: una frase de 16 pasos por pista, 4 pistas de percusión; Start, tempo, poner y borrar pasos. **Bolsa de voces simple** (voz libre o robar la más vieja) | **Primer beat** | `Vec`, arrays, `Option`, tiempos en muestras | rust-dev |
| **H4** | 8 pistas; nota, velocidad y largo; vista Frase; swing; parámetro "voces" por pista (polifonía por superposición) | Notas largas que se superponen | módulos, traits simples, tests con `cargo test` | rust-dev |
| **H5** | **Cadenas y Canción**: cabezales por pista, regla de bloques, transposición, secciones en la Grilla (L2/R2), creación implícita, clonar, loop de sección, pantallas Cadena y Canción | Beat de 4 compases (cadenas de 4 frases) y una canción de 2 filas | **IDs en lugar de referencias** (el borrow checker), `Box<[T; N]>`, máquinas de estado | rust-dev |
| **H6** | Guardar y cargar el proyecto en RON | **Objetivo del MVP cumplido** (con percusión) | `serde`, derive, manejo de errores, archivos | rust-dev |
| **H7** | Sampler + Navegador de archivos + copia de WAV al proyecto | Un sample en la pista 6 | `hound`, `Box`, pasar datos grandes al hilo de audio | rust-dev, dsp-engineer |
| **H8** | Sintetizador sustractivo polifónico + **acordes** (columna ACORDE, A + L1/R1) + **robo de voces completo** + pantalla Instrumento | Bajo y progresión de acordes | traits para instrumentos, `enum` con datos, tests de asignación de voces | dsp-engineer, rust-dev |
| **H9** | Mezcla: volumen, paneo, mute/solo, envíos a delay y reverb, limitador; sticks | Beat mezclado, filtro en vivo | buffers preasignados, atómicos | dsp-engineer, rust-dev |
| **H10** | Selección, copiar, cortar y pegar en todas las pantallas | Variaciones rápidas | iteradores, slices, `Clone` | rust-dev |
| **H11** | Cierre del MVP: medición de CPU (prueba de estrés de la bolsa), latencia, lanzador en Ports, config de botones | MVP completo y medido | perfilado, `--release`, features de Cargo | device-ops, rust-dev, dev-infra |

**H5 es el hito más pesado.** Si se hace largo, se puede partir en H5a (cadenas y cabezales, sin pantalla Canción: una sola fila implícita)
y H5b (pantalla Canción y regla de bloques).

## Riesgos

| Riesgo | Impacto | Mitigación |
|---|---|---|
| **Legibilidad**: 640×480 en 3,5" son ~229 ppp; una fuente de 8×16 px mide ~1,8 mm de alto | Fatiga visual, errores al editar | Validarlo en H1 con la consola real; si no alcanza, fuente de 12×24 o 16×16 en las vistas principales y layout de menos columnas ([spec-ui](spec-ui.md#tipografía-y-legibilidad)) |
| **Complejidad del modelo de cadenas** para un principiante (como usuario y como programador) | Curva de aprendizaje; H5 se alarga | Grilla con creación implícita; kit que ya trae cadenas y frases; H5 divisible en dos |
| **Desalineación de pistas** cuando las cadenas de una fila tienen largos distintos | Canciones que "se corren" sin que se entienda por qué | Columna "largo de cadenas" con aviso en Canción; recomendación de cadenas de silencio |
| **Ediciones con efecto a distancia** (editar una frase compartida cambia todos sus usos) | Cambios inesperados | Marca `*` y contador de usos visibles; clonar con A + Y |
| Peor caso de CPU con la bolsa llena y robo constante | Xruns | Bolsa acotada (16 + 16 de fundido, ADR 0013); prueba de estrés en H11 |
| Latencia de ALSA mayor a la esperada | Tocar en vivo se siente lento | Medir en H2/H11 (skill `audio-latency-test`); probar buffers de 128/256/512 |
| Mapeo de botones distinto entre SDL y la consola | Controles cruzados | Archivo de configuración (U6) y pantalla de prueba de input en H1 |
| Drift de los sticks | Filtro que se mueve solo | Zona muerta y calibración en la configuración |
| El SO intercepta combinaciones (por ejemplo, Select + Start o el botón FN) **(sin verificar)** | Acciones que no llegan a la app | Verificar en H1; FN no se usa; cambiar el atajo si hace falta |
| Gobernador de CPU que baja la frecuencia | Xruns intermitentes | Medir; fijar el gobernador `performance` mientras corre la app **(sin verificar en dArkOS)** |

## Preguntas abiertas

Respondidas el 2026-11-19: ~~interfaz híbrida~~ (sí), ~~modelo de secuenciación~~ (cadenas por pista), ~~polifonía~~ (en el MVP), ~~p-locks~~ (v1),
~~instrumento por pista~~ (fijo, sin columna INST), ~~acordes~~ (columna ACORDE con tipos predefinidos e inversión),
~~celda vacía en la canción~~ (vuelve al inicio del bloque), ~~creación implícita desde la Grilla~~ (sí).

Pendientes:
1. **Nombre** del instrumento (hoy: "groovebox").
3. **Valores en decimal** (0–127) en lugar de hexadecimal. ¿OK?
4. ¿Los **sticks** controlan filtro y envíos de la pista actual (propuesto) o el master, u otra asignación (por ejemplo, pitch bend)?
5. ¿Qué estilo musical querés priorizar para el kit por defecto (techno/house, hip-hop, chiptune)? Define los presets de fábrica.

Nuevas, generadas por las decisiones del 2026-11-19:

7. ¿Frases de **16 pasos fijos** en el MVP (propuesto) y largo variable en v1?
8. ¿**Transposición por entrada de cadena** en el MVP (propuesto: es barata y es parte de la gracia de las cadenas)?
9. ~~**Tamaño de la bolsa de voces**~~: 16 + 16 de fundido (ADR 0013, decidido por el usuario 2026-11-20).
11. **Start contextual** (canción, cadena o frase según la pantalla) o siempre la canción completa.

## Fuentes

- [Referentes](referentes.md) (con sus fuentes).
- [Hardware de la RGB20S](../../hardware/rgb20s.md).
- [Manual M8 v3.2.0](https://cdn.shopify.com/s/files/1/0455/0485/6229/files/m8_operation_manual_v20230630.pdf) (canción, cadenas, frases, clonado, filas aisladas, desalineación de pistas).
- [Retro Dodo: RGB20S (disposición de botones, FN)](https://retrododo.com/powkiddy-rgb20s/), [Time Extension: RGB20S](https://www.timeextension.com/reviews/powkiddy-rgb20s-cheap-design-impressive-performance), [retrocatalog: RGB20S (L3/R3, volumen)](https://retrocatalog.com/retro-handhelds/powkiddy-rgb20s).
- Cálculo de densidad de píxeles: diagonal 3,5" en 4:3 → 2,8" × 2,1" → 640 / 2,8 ≈ 229 ppp (cálculo propio).
