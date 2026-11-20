# Groovebox — referentes

Última revisión: 2026-11-19 · Volver al [índice](README.md)

Análisis de trackers, grooveboxes y secuenciadores, por su **flujo de trabajo** y sus **decisiones de diseño**.
El objetivo es extraer principios para un instrumento propio en la RGB20S (d-pad, 4 botones frontales,
4 gatillos, Start/Select, 2 sticks, pantalla de 640×480 sin táctil). No se copian interfaces, nombres ni arte.

## Resumen comparativo

| Referente | Paradigma | Unidad de composición | Input principal | Lo más útil para nosotros |
|---|---|---|---|---|
| LSDJ | tracker vertical | fila → frase → cadena → canción | d-pad + A/B + Select/Start | Navegación por pantallas con Select + cruz; "botón + cruz" para editar |
| Dirtywave M8 | tracker vertical | paso → frase (16) → cadena → canción, por pista | cruz + 4 teclas (play, shift, option, edit) | Esquema de 4 teclas consistente; mapa 2D de vistas; clonar en vez de copiar |
| Polyend Tracker | tracker vertical con pads | paso → patrón (8 pistas, hasta 128 pasos) → canción | grilla de pads, encoder, botones de función | Patrón global de 8 pistas: modelo mental simple; 2 columnas de efectos por paso |
| LittleGPTracker / picoTracker | tracker vertical | igual que LSDJ, con samples | botones de consola portátil | Prueba de que un tracker de samples cabe en hardware muy chico |
| Renoise | tracker vertical (PC) | línea → patrón → secuencia de patrones | teclado | Columnas de efectos con comandos de 2 letras; matriz de patrones |
| Elektron Digitakt / Model:Samples / Model:Cycles | step sequencer | paso → patrón (hasta 64) → cadena/canción | 16 teclas de paso + encoders | P-locks, condiciones de paso, microtiming; pistas monofónicas |
| TE Pocket Operators | step sequencer mínimo | paso (16) → patrón (16) → cadena | 16 teclas + 2 perillas | Cadena de patrones directa; efectos "punch-in" en vivo; sonido inmediato |
| TE OP-Z | step sequencer multipista | paso → patrón (16 pistas) → proyecto | 16 teclas + 4 encoders | Largo por pista; "componentes de paso" (modificadores por paso) |
| TE OP-1 | sinte + cinta de 4 pistas | grabación lineal en cinta | teclado + 4 encoders | Pantalla con gráficos que explican el sonido, no solo números |
| Roland MC-101 / TR | step sequencer + clips | paso → clip (hasta 128) por pista | 16 pads + perillas | Clips por pista que guardan movimientos de perillas; efectos de performance |
| Korg Volca | step sequencer | paso (16) → patrón | 16 teclas táctiles + perillas | Motion sequence; "active step" (saltear pasos) |
| Novation Circuit | grilla de pads | paso (16/32) → patrón → cadena → escenas | grilla 4×8 + 8 encoders | Kit por defecto que suena al encender; escenas para armar canciones |
| Synthstrom Deluge | grilla piano-roll | clip → canción/arreglo | grilla 16×8 + OLED chico | Advertencia: la información crítica oculta detrás de botones confunde |

## Fichas

### LSDJ (Little Sound DJ, Game Boy)

| Campo | Contenido |
|---|---|
| Paradigma | Tracker vertical |
| Unidad de composición | Fila de frase → **frase** (16 filas) → **cadena** (lista de frases con transposición) → **canción** (cadenas por canal). Instrumentos y **tablas** (secuencias de comandos que modulan el sonido en el tiempo) |
| Input | Cruz, A, B, Select, Start. Las pantallas se ordenan de lo general (canción) a lo particular (tabla) y se navegan con **Select + cruz**. Los valores se editan con **A + cruz** |
| Ideas a tomar | 1) El mapa de pantallas ordenado por nivel de detalle. 2) "Botón + cruz" como gesto universal de edición. 3) Reutilizar bloques (cadenas) en vez de copiar |
| Ideas a evitar | Notación hexadecimal y comandos crípticos: curva de aprendizaje alta. Las limitaciones del chip de Game Boy (4 canales fijos) no aplican acá |
| Fuentes | [learnlsdj](https://learnlsdj.github.io/index.html), [tutorial TM8C (U. Waterloo)](https://cs.uwaterloo.ca/~plragde/flaneries/TM8C/Prelude.html) |

### Dirtywave M8

| Campo | Contenido |
|---|---|
| Paradigma | Tracker vertical (descendiente directo de LSDJ) |
| Unidad de composición | 8 pistas monofónicas. Frase de 16 pasos (nota, volumen, instrumento y **3 columnas de comandos**) → cadena (hasta 16 frases, con transposición) → canción (hasta 256 filas de cadenas por pista). Hasta 128 instrumentos y tablas por canción |
| Input | Cruz + 4 teclas (reproducir, shift, opción, edición). Shift + cruz navega entre vistas, que forman un mapa 2D con un minimapa en pantalla. Edición + izquierda/derecha cambia el valor de a poco; edición + arriba/abajo, de a mucho. Edición + opción borra. Shift + opción entra en modo selección |
| Ideas a tomar | 1) **Cuatro gestos que alcanzan para todo**, iguales en todas las vistas. 2) Paso fino y paso grueso según el eje de la cruz. 3) Insertar con el último valor usado (acelera la carga). 4) Minimapa de vistas siempre visible |
| Ideas a evitar | La densidad de información (hexadecimal, 3 columnas de comandos por paso) es intimidante para un principiante. Las cadenas por pista desalinean las pistas si tienen largos distintos (el propio manual lo advierte) |
| Contexto técnico | El M8 hace 8 voces con FM de 4 operadores, síntesis tipo "macro", samples, reverb, delay y chorus en un microcontrolador **(sin verificar: Teensy 4.1, Cortex-M7 a 600 MHz)**. Sugiere que el A35 tiene margen de sobra para un motor parecido |
| Fuentes | [Manual de operación M8 v3.2.0](https://cdn.shopify.com/s/files/1/0455/0485/6229/files/m8_operation_manual_v20230630.pdf) |

### Polyend Tracker

| Campo | Contenido |
|---|---|
| Paradigma | Tracker vertical con grilla de pads y encoder |
| Unidad de composición | Paso (nota, instrumento, **2 efectos**) → **patrón** de 8 pistas, de 1 a 128 pasos → canción de hasta 255 patrones |
| Input | Grilla de pads (sirve de teclado y de atajo de posición), un encoder grande y botones de función con nombre |
| Ideas a tomar | 1) **Patrón global** (todas las pistas avanzan juntas): más fácil de entender que cadenas por pista. 2) Dos columnas de efectos por paso es un buen equilibrio entre poder y legibilidad. 3) Pantallas dedicadas para el sample (forma de onda con marcadores) |
| Ideas a evitar | Depende de los pads para tocar y elegir posición; no tenemos pads |
| Fuentes | [Manual Polyend Tracker v1.6](https://cdn.shopify.com/s/files/1/0527/2557/9960/files/Polyend_Tracker_v.1.6-2.pdf), [Sound On Sound](https://www.soundonsound.com/reviews/polyend-tracker), [MusicTech](https://www.musictech.net/reviews/hardware-instruments/the-big-review-polyend-tracker/) |

### LittleGPTracker (Little Piggy Tracker) y picoTracker

| Campo | Contenido |
|---|---|
| Paradigma | Tracker vertical, imita la interfaz de LSDJ con samples |
| Unidad de composición | Igual que LSDJ: frase → cadena → canción, más tablas |
| Input | Botones de consolas portátiles (cruz, A, B, L, R, Start, Select). Corre en GP2X, PSP, Miyoo Mini, Raspberry Pi y otras |
| Ideas a tomar | 1) Es la prueba de que un tracker de samples cabe en consolas mucho más débiles que la RGB20S. 2) El comando de "salto" dentro de tablas permite secuencias programables por instrumento (idea para "después") |
| Ideas a evitar | Hereda la curva de aprendizaje de LSDJ. Podría probarse como "camino A" (ver el [índice](README.md#camino-a--software-existente)) |
| Fuentes | [LittleGPTracker (GitHub, fork de djdiskmachine)](https://github.com/djdiskmachine/LittleGPTracker), [picoTracker (GitHub)](https://github.com/democloid/picoTracker), [comandos de efectos (Battle of the Bits)](https://battleofthebits.com/lyceum/View/LittleGPTracker%20Effects%20Commands) |

### Renoise

| Campo | Contenido |
|---|---|
| Paradigma | Tracker vertical para PC |
| Unidad de composición | Línea → patrón (todas las pistas) → secuencia de patrones con matriz para mutear bloques |
| Input | Teclado y mouse |
| Ideas a tomar | 1) Comandos de efecto de 2 letras + valor, legibles. 2) Matriz de patrones: mutear una pista solo en un tramo de la canción (idea para "después") |
| Ideas a evitar | Pensado para teclado completo; casi nada se traduce a una cruz |
| Fuentes | Conocimiento general del autor **(sin verificar contra el manual)** |

### Elektron: Digitakt, Model:Samples y Model:Cycles

| Campo | Contenido |
|---|---|
| Paradigma | Step sequencer con 16 teclas de paso |
| Unidad de composición | Paso (trig) → patrón de hasta 64 pasos, con largo y escala por pista → cadena o canción. Pistas **monofónicas** |
| Input | 16 teclas de paso + encoders. Mantener un paso y girar un encoder crea un **p-lock** (valor de parámetro propio de ese paso) |
| Conceptos clave | **P-locks** (cualquier parámetro distinto por paso). **Condiciones de paso**: probabilidad, "A de cada B vueltas", FILL (solo con el modo fill activo), PRE (según la condición anterior de la pista), primera vez. **Microtiming** (correr un paso fuera de la grilla) y **retrig**. **Trigless locks**: cambiar parámetros sin disparar nota. Model:Samples simplifica: 6 pistas, "chance" por pista. Model:Cycles usa 6 "máquinas" FM con pocos parámetros (bombo, caja, metal, percusión, tono y acordes) |
| Ideas a tomar | 1) Los p-locks son la herramienta más poderosa por unidad de interfaz: con 2 por paso se consigue mucha variación. 2) Las condiciones de paso dan variación sin escribir más patrones. 3) Pistas monofónicas: simplifica el motor y el presupuesto de CPU. 4) "Máquinas" de síntesis con 4 a 6 parámetros macro, en vez de exponer todo (Model:Cycles) |
| Ideas a evitar | Las combinaciones "mantener paso + girar encoder" no existen sin encoders. Varios reviews critican a Model:Cycles por no tener ADSR completa: no esconder parámetros básicos |
| Fuentes | [Digitakt: notas de manual (mcgrp)](https://mcgrp.ru/files/viewer/1007940/33), [davetron5000 synth-docs](https://davetron5000.github.io/synth-docs/synths/elektron-digitakt.html), [MusicRadar: 10 consejos Digitakt](https://www.musicradar.com/how-to/10-ways-to-get-more-out-of-the-elektron-digitakt), [Model:Samples (ModularGrid)](https://modulargrid.net/p/elektron-model-samples), [Elektronauts: sample locks](https://www.elektronauts.com/t/no-sample-locks-on-model-samples-discuss-implemented-version-1-02/78902), [Attack: Model:Cycles](https://www.attackmagazine.com/reviews/gear-software/elektron-modelcycles/), [MusicRadar: Model:Cycles](https://musicradar.com/reviews/elektron-modelcycles), [Engadget: Model:Cycles](https://www.engadget.com/2020/02/26/elektron-model-cycles-review-affordable-fm-groovebox/) |

### Teenage Engineering: Pocket Operators

| Campo | Contenido |
|---|---|
| Paradigma | Step sequencer mínimo |
| Unidad de composición | 16 pasos → 16 patrones → cadena de patrones (se arma manteniendo el botón de patrón y tocando números) |
| Input | 16 teclas, unas pocas teclas de función y 2 perillas |
| Ideas a tomar | 1) **Sonido inmediato**: se prende y ya hay un kit que suena. 2) Cadena de patrones como gesto rápido, sin pantalla de canción. 3) **Efectos "punch-in"** en vivo que no destruyen el patrón |
| Ideas a evitar | Todo vive en la memoria muscular, sin pantalla; nosotros tenemos 640×480 y conviene mostrar el estado |
| Fuentes | [Guía PO-12](https://teenage.engineering/guides/po-12), [Guía PO-16](https://teenage.engineering/guides/po-16), [Wikipedia: Pocket Operators](https://en.wikipedia.org/wiki/Pocket_Operators), [Sound On Sound](https://www.soundonsound.com/reviews/teenage-engineering-pocket-operators) |

### Teenage Engineering: OP-Z y OP-1

| Campo | Contenido |
|---|---|
| Paradigma | OP-Z: step sequencer de 16 pistas (8 de audio y 8 de control). OP-1: sintetizador con grabador de "cinta" de 4 pistas |
| Unidad de composición | OP-Z: paso → patrón, con **largo y velocidad por pista**. OP-1: grabación lineal |
| Input | OP-Z: 16 teclas + 4 encoders; casi sin pantalla (se apoya en una app). OP-1: teclado, 4 encoders de color y pantalla con gráficos |
| Ideas a tomar | 1) **Componentes de paso** del OP-Z: modificadores por paso (repetir, saltar, cambiar pitch, probabilidad) elegidos de una lista corta. Es otra forma de pensar las condiciones de paso, más fácil de mostrar en pantalla. 2) Largo por pista (polimetría). 3) Del OP-1: **gráficos que explican el sonido** (una envolvente dibujada se entiende mejor que cuatro números) |
| Ideas a evitar | El OP-Z depende de una pantalla externa. La cinta del OP-1 es lineal y pesada en memoria; no encaja con el flujo de frases y cadenas del MVP |
| Fuentes | [OP-Z: pistas](https://teenage.engineering/guides/op-z/tracks), [OP-Z: componentes de paso](https://teenage.engineering/guides/op-z/step-components), [Sound On Sound: OP-Z](https://www.soundonsound.com/reviews/teenage-engineering-op-z), [Guía OP-1 (soundtech)](https://www.soundtech.co.uk/musicians-blog/our-guide-to-the-teenage-engineering-op-1) |

### Roland MC-101 y familia TR

| Campo | Contenido |
|---|---|
| Paradigma | Step sequencer (programación estilo TR, paso por paso) con clips |
| Unidad de composición | 4 pistas, cada una con clips de hasta 128 pasos; los clips guardan los movimientos de las perillas |
| Input | 16 pads + perillas + efectos de performance asignados a pads |
| Ideas a tomar | 1) Guardar movimientos de controles junto a las notas (para los sticks, "después"). 2) Efectos de performance pensados para transiciones |
| Ideas a evitar | Pocas pistas (4) para un groovebox completo |
| Fuentes | [Attack: MC-101](https://www.attackmagazine.com/reviews/gear-software/roland-mc-101/), [Juno: MC-101](https://www.juno.co.uk/junodaily/2019/10/10/roland-mc-101-groovebox-review/), [Sound On Sound: MC-101/707](https://www.soundonsound.com/reviews/roland-mc-101-mc-707) |

### Korg Volca

| Campo | Contenido |
|---|---|
| Paradigma | Step sequencer de 16 pasos |
| Unidad de composición | Paso → patrón (16) |
| Input | 16 teclas táctiles y perillas |
| Ideas a tomar | 1) **Motion sequence**: grabar el movimiento de una perilla en el patrón ("después", con los sticks). 2) **Active step**: apagar pasos para que se salteen, sin borrar notas (patrones de largo impar al vuelo) |
| Ideas a evitar | Sin pantalla; poca memoria de patrones |
| Fuentes | [Korg: volca beats](https://www.korg.com/us/products/dj/volca_beats/page_1.php), [Manual volca drum (active step)](https://manualshive.com/en/manual/korg/volca-drum/1998639?page=3) |

### Novation Circuit (Tracks / Rhythm)

| Campo | Contenido |
|---|---|
| Paradigma | Grilla de pads con secuenciador por pista |
| Unidad de composición | Paso (16 o 32) → patrón (8 por pista) → cadena de patrones → **escenas** que agrupan cadenas |
| Input | Grilla 4×8, 8 encoders y botones de vista |
| Ideas a tomar | 1) Proyecto nuevo con kit y sonidos que suenan bien de entrada. 2) Las escenas muestran que "canción" puede ser solo una lista de combinaciones de patrones |
| Ideas a evitar | Cadenas solo de patrones consecutivos: una restricción que confunde |
| Fuentes | [Circuit Tracks: guía](https://userguides.novationmusic.com/hc/en-gb/articles/25494476280850-Circuit-Tracks-hardware-overview), [Sound On Sound: Circuit Tracks](https://www.soundonsound.com/reviews/novation-circuit-tracks?amp) |

### Synthstrom Deluge

| Campo | Contenido |
|---|---|
| Paradigma | Grilla 16×8 tipo piano-roll, con clips y arreglo |
| Unidad de composición | Clip → vista canción (una fila por pista) → arreglo |
| Input | 128 pads RGB, encoders y una pantalla chica |
| Ideas a tomar | 1) Una vista "canción" donde cada pista ocupa una fila es fácil de leer. 2) Firmware abierto con comunidad activa |
| Ideas a evitar | Reviews de usuarios: con la pantalla chica, la información clave solo aparece tras apretar un botón, y uno termina editando el compás o el tono equivocado. **Lección: siempre mostrar dónde está el cursor y qué se está editando** |
| Fuentes | [Waveform Magazine](https://waveformmagazine.com/waveform-reviews/synthstrom-audible-deluge/), [Synthtopia](https://www.synthtopia.com/content/2017/05/19/synthstrom-deluge-review/), [Thomann: reviews de usuarios](https://www.thomannmusic.com/synthstrom_audible_deluge_reviews.htm) |

## Conceptos transversales

| Concepto | Qué es | Cómo lo adoptamos | Prioridad |
|---|---|---|---|
| P-locks | Valor de parámetro propio de un paso | 2 columnas de "lock" por paso en la vista Frase | v1 |
| Condiciones de paso | Disparar el paso solo si se cumple algo (probabilidad, A de B, fill, primera vez, anterior) | 1 columna "condición" por paso | v1 |
| Microtiming / retrig | Correr el paso fuera de la grilla / repetirlo dentro del paso | Columnas "micro" y "rep" | v1 |
| Active step | Saltear pasos sin borrarlos | Cubierto por condiciones (después, si hace falta) | después |
| Cadenas por pista (tracker) | Cada pista sigue su propia lista de cadenas y frases | Modelo adoptado: canción → cadenas → frases ([ADR 0006](../../decisions/0006-cadenas-por-pista.md)), con la Grilla mostrando la sección actual | MVP |
| Canción como lista de patrones | Filas de (patrón, repeticiones) | Descartado a favor de las cadenas por pista ([ADR 0003](../../decisions/0003-modelo-de-secuenciacion.md), reemplazada) | — |
| Clonar en vez de copiar | Duplicar una frase o cadena a un número nuevo para variarla | A + Y | MVP |
| Polifonía con bolsa de voces | Voces compartidas entre pistas con robo | Bolsa de 16 + 16 de fundido, acordes por paso ([ADR 0007](../../decisions/0007-polifonia-en-el-mvp.md), [0013](../../decisions/0013-ranuras-de-fundido.md)) | MVP |
| Motion sequence | Grabar movimientos de controles | Grabar los sticks como locks | después |
| Efectos punch-in | Efecto temporal en vivo | Sticks sobre filtro y envíos | MVP (sin grabar) |
| Máquinas de síntesis macro | Pocos parámetros con mucho efecto | Percusión sintetizada con 4–6 parámetros | MVP |

### Motores de síntesis livianos (comparados)

| Motor | Costo de CPU | Variedad sonora | Comentario |
|---|---|---|---|
| Reproducción de samples (interpolación lineal) | muy bajo | depende del material | Lo más barato y lo más versátil; requiere archivos |
| Percusión sintetizada (seno con barrido + ruido filtrado) | bajo | media (estilo caja de ritmos analógica) | Suena sin archivos: ideal para el primer arranque |
| Sustractiva (1 oscilador PolyBLEP + sub + SVF) | medio | alta para bajos y leads | El "caballo de batalla" de un groovebox |
| FM de 2 operadores (seno por tabla) | bajo | media (campanas, bajos FM, metales) | Muy barato; poco intuitivo de programar |
| FM de 4 operadores | medio | alta | Interfaz compleja con cruz y botones |
| Wavetable | bajo a medio | alta | Necesita tablas y anti-aliasing por mipmaps |

### Efectos baratos

Delay con buffer circular y pasa-bajos en la realimentación (muy barato); reverb de pocas líneas (FDN de 4 líneas o
Schroeder chico, barato a medio); bitcrusher y overdrive por pista (muy baratos); limitador o soft clip en el master (barato).
El chorus y la compresión con sidechain quedan para después. Detalle en [spec-motor.md](spec-motor.md#efectos).

## Síntesis: qué tomamos

1. **De los trackers (LSDJ, M8, LGPT):** la estructura canción → cadenas por pista → frases, el clonado, el mapa de pantallas
   navegado con "modificador + cruz", la edición "botón + cruz" con paso fino y grueso, insertar con el último valor usado y el minimapa.
2. **De Polyend:** 2 columnas de efectos por paso. (Su patrón global de 8 pistas se evaluó y se descartó a favor de las cadenas por pista.)
3. **De Elektron y OP-Z:** p-locks, condiciones de paso y microtiming (v1). Se aparta de su modelo de pistas monofónicas: el MVP es polifónico, con una bolsa de voces compartida.
4. **De Pocket Operators y Circuit:** proyecto nuevo que suena al instante con un kit sintetizado; efectos de performance.
5. **De OP-1 y Deluge:** mostrar el sonido con gráficos y nunca ocultar dónde está el cursor.
6. **Lo propio:** una **Grilla** que muestra las frases de las 8 pistas en la sección actual y crea cadenas y frases al vuelo,
   una **vista tracker** (Frase) para el detalle, **acordes por paso** editables con A + L1/R1, y los **sticks** como controles
   expresivos (ver [ADR 0002](../../decisions/0002-interfaz-hibrida-grilla-tracker.md)).

## Preguntas abiertas

- ¿Probar LGPT o LSDJ en la consola antes de implementar, para calibrar la ergonomía real de la cruz? Es barato y daría información útil.

## Fuentes

Además de las citadas en cada ficha:
- [LSDJ (manual en Scribd)](https://www.scribd.com/document/3519167/LSDJ)
- [Manual Polyend Tracker (B&H)](https://www.bhphotovideo.com/lit_files/944263.pdf)
- [Bonedo: M8 (alemán)](https://www.bonedo.de/?p=564638)
- [Elektronauts: Digitakt Unofficial Guide](https://www.elektronauts.com/t/digitakt-unofficial-guide/40705)
- [Novation: patrones en Circuit Rhythm](https://userguides.novationmusic.com/hc/en-gb/articles/25494352385810-Using-Circuit-Rhythm-s-patterns)
- [Waveform: Deluge](https://waveformmagazine.com/waveform-reviews/synthstrom-audible-deluge/)
