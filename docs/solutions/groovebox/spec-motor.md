# Groovebox — spec del motor de sonido y del secuenciador

Última revisión: 2026-11-20 (H5: cadenas, canción y modos de reproducción implementados) · Estado: borrador para revisión del usuario · Volver al [índice](README.md)

Decisiones relacionadas:
[ADR 0006 — cadenas por pista](../../decisions/0006-cadenas-por-pista.md) (aceptada) ·
[ADR 0007 — polifonía en el MVP](../../decisions/0007-polifonia-en-el-mvp.md) (aceptada) ·
[ADR 0005 — configuración de audio](../../decisions/0005-configuracion-de-audio.md) (propuesta).
Recetas de DSP: skill `dsp-recipes`. Reglas de tiempo real: skill `rust-realtime-audio`.
Lo implementado (H2), con fórmulas, costos medidos y pruebas: [DSP del motor](dsp.md).

## Arquitectura

```
Hilo de interfaz (SDL2: input, dibujo, archivos)          núcleo 1 (el que asigne el SO)
  - dueño del Project (la "fuente de verdad")
  - cada edición => un Command
        |  rtrb::Producer<Command>   (cola SPSC sin locks, 1024 entradas)
        v
Hilo de audio (callback de cpal / ALSA)                    otro núcleo, prioridad alta
  1. vacía la cola de comandos
  2. avanza los 8 cabezales del secuenciador (reloj en muestras)
  3. asigna voces de la bolsa, renderiza -> mezcla -> envíos -> master
        |  rtrb::Producer<Event>     (posiciones, medidores, CPU, xruns,
        v                             memoria a liberar)
Hilo de interfaz (dibuja los cabezales y los medidores; libera lo que devuelve el motor)
```

- **El secuenciador vive en el hilo de audio**: la sincronía es exacta a la muestra y no depende de los fps de la interfaz.
- El motor tiene **su propia copia** de los datos que necesita para sonar (frases, cadenas, canción, instrumentos, mezcla). La interfaz manda ediciones chicas y, al cargar un proyecto, una copia completa ya construida (`Box`).
- Los datos grandes (un sample cargado) se construyen en el hilo de interfaz y se pasan por la cola. Lo que el motor deja de usar vuelve por la cola de eventos para liberarse fuera del hilo de audio.
- El `Engine` no depende de `cpal`: se puede probar con `cargo test` y renderizar offline a WAV (base de la exportación de v1).

### Comandos y eventos (ilustrativo)

```rust
enum Command {
    SetStep { phrase: PhraseId, step: u8, value: Step },
    SetChainEntry { chain: ChainId, pos: u8, value: Option<ChainEntry> },
    SetSongCell { row: u8, track: u8, chain: Option<ChainId> },
    SetInstrument { track: u8, instrument: Box<Instrument> }, // cambio de tipo
    SetParam { track: u8, param: ParamId, value: u8 },
    SetMix { track: u8, param: MixParam, value: u8 },
    SetFx { param: FxParam, value: u8 },
    SetTempo(f32), SetSwing(u8),
    Play(PlayMode),
    Stop,
    Audition { track: u8, note: u8, chord: Chord, velocity: u8 },
    Perform { track: u8, target: PerfTarget, offset: f32 },   // sticks, -1.0..=1.0
    LoadSample { id: SampleId, data: Box<SampleData> },
    ReplaceProject(Box<EngineProject>),
}

enum PlayMode {
    Song { from_row: u8 },                       // todas las pistas
    Section { row: u8, entry: u8 },              // loop de una sección, todas las pistas
    Chain { track: u8, chain: ChainId },         // loop de una cadena, una pista
    Phrase { track: u8, phrase: PhraseId },      // loop de una frase, una pista
}
// Implementado (H5): Chain { track, row } y Phrase { track, row, entry }, por posición (ver "Modos de reproducción").

enum Event {
    Positions([TrackPosition; 8]),   // fila, entrada de cadena, frase y paso de cada pista
    Meters { peaks: [f32; 9] },      // 8 pistas + master
    Load { audio_cpu_percent: u8, voices_in_use: u8 },
    Xrun,
    Free(Garbage),                   // memoria para liberar en la interfaz
}
```

## Configuración de audio

| Parámetro | Valor propuesto | Motivo |
|---|---|---|
| Sample rate | **48 kHz** | Frecuencia habitual de los códecs y de ALSA; evita remuestrear en el driver **(sin verificar la nativa del códec RK817)** |
| Formato interno | `f32`, estéreo intercalado | Simple y sin problemas de headroom; conversión a `i16` solo si el dispositivo lo exige |
| Tamaño de buffer | **256 frames** (5,3 ms) | Equilibrio entre latencia y riesgo de xruns. Alternativas: 128 (2,7 ms) si es estable; 512 (10,7 ms) como respaldo |
| Latencia total estimada | ~11–16 ms (2 o 3 períodos + driver) **(sin verificar)** | Aceptable para tocar en vivo; se mide con `audio-latency-test` |
| Subbloque interno | hasta **32 frames** | Los eventos del secuenciador se aplican en su muestra exacta, y las modulaciones (envolvente → filtro, sticks) se actualizan cada ≤ 32 muestras |
| Denormales | flush-to-zero al iniciar el hilo de audio | Evita picos de CPU en colas de filtros y reverb (mecanismo exacto en Rust para aarch64 **sin verificar**) |
| Prioridad | `SCHED_FIFO` para el hilo de audio, si el sistema lo permite **(sin verificar en dArkOS)** | Menos xruns con la interfaz ocupada |

Detalle en [ADR 0005](../../decisions/0005-configuracion-de-audio.md).

## Voces y polifonía

Decisión: [ADR 0007](../../decisions/0007-polifonia-en-el-mvp.md).

- **Bolsa global de 16 voces**, compartida por las 8 pistas, más **16 ranuras de fundido** donde terminan las voces robadas (5 ms; [ADR 0013](../../decisions/0013-ranuras-de-fundido.md)). Máximo instantáneo: **32 voces**, aunque las que se funden duran solo 5 ms.
- Cada pista tiene el parámetro **voces** (1–8):
  - **1 (mono):** una nota nueva corta la anterior. En percusión y sampler, la anterior pasa a una ranura de fundido. En el sustractivo, la envolvente se redispara desde su nivel actual (legato) y, en v1, el glide desliza el tono.
  - **2–8 (poli):** cada nota suena hasta que termina su largo (gate) y luego entra en relajación. Las notas de pasos siguientes se superponen.
- Valores por defecto: percusión 1, sampler 2, sustractivo 4 (la pista de bajo del kit, 1).
- **Implementado (H3 y H4):** las notas de largo fijo de una pista poli se superponen hasta su límite; una voz en
  relajación sigue contando para el límite de su pista (regla 1 de abajo), así que la nota que llega justo cuando
  termina el largo de otra la roba con fundido. Cambiar las voces mientras suena no corta nada: el límite nuevo se
  aplica en la próxima nota de la pista, que roba las más viejas que hagan falta para quedar dentro del límite. Se
  edita en la pantalla Instrumento (por ahora solo ese parámetro, [spec-ui](spec-ui.md#5-instrumento)).

### Asignación y robo de voces

Al disparar un paso con `n` notas (1 sin acorde, hasta 4 con acorde), para cada nota, empezando por la raíz:

1. Si la pista ya usa todas sus voces → se roba **la voz más vieja de esa pista**.
2. Si no, y hay una voz libre en la bolsa → se usa.
3. Si la bolsa está llena → se roba, en este orden:
   1. la voz **en relajación** más vieja (de cualquier pista);
   2. la voz más vieja de la **pista que más voces usa**;
   3. la voz más vieja de todas.
4. Nunca se roba una nota del mismo acorde recién disparado si queda otra opción. Si la pista tiene menos voces que notas el acorde, se descartan las notas más agudas.
5. La voz robada pasa a una ranura de fundido. Si no hay ranuras libres, se corta en seco la que lleva más tiempo fundiéndose (caso raro).
   Al implementarlo resultó no ser raro con varias pistas polifónicas cambiando de acorde a la vez: ver [ADR 0013](../../decisions/0013-ranuras-de-fundido.md) (propuesta).

Estas reglas se prueban con tests que agotan la bolsa (por ejemplo, 8 pistas con acordes de 4 notas y largo `INF`).

### Acordes

- Cada paso tiene una columna **ACORDE**: tipo e inversión. Las notas se calculan desde la nota del paso:

| Tipo | Intervalos (semitonos) | Tipo | Intervalos |
|---|---|---|---|
| `---` | 0 (nota sola) | `sus4` | 0, 5, 7 |
| `5` | 0, 7 | `may7` | 0, 4, 7, 11 |
| `8va` | 0, 12 | `men7` | 0, 3, 7, 10 |
| `may` | 0, 4, 7 | `dom7` | 0, 4, 7, 10 |
| `men` | 0, 3, 7 | `m7b5` | 0, 3, 6, 10 |
| `dis` | 0, 3, 6 | `add9` | 0, 4, 7, 14 |
| `aum` | 0, 4, 8 | | |
| `sus2` | 0, 2, 7 | | |

- **Inversión** 0–3: sube una octava las `i` notas más graves.
- La percusión sintetizada ignora el acorde (toca la raíz).
- **v1:** acorde libre (3 intervalos editables) y rasgueo (un retraso de 0–12 ticks entre notas).

### Cadena de señal por pista

```
nota/velocidad/acorde -> voces de la bolsa (sample | percusión | osciladores | FM)
   cada voz: fuente -> filtro SVF (LP/BP/HP) <- envolvente de filtro, stick izquierdo, p-locks (v1)
                    -> amplificador          <- envolvente de amplitud, velocidad
   suma de las voces de la pista
               -> drive/bitcrusher (v1)
               -> volumen + paneo (ley de potencia constante, por tabla)
               -> suma master  +  envío a delay  +  envío a reverb
```

```
pistas 1..8 --+--> suma --------------------------------+--> ganancia master --> soft clip / limitador --> salida
              +--> bus delay  --> delay estéreo  -------+
              +--> bus reverb --> reverb liviana -------+
```

El filtro es **por voz** (cada nota de un acorde tiene su envolvente de filtro). Es lo más caro del sustractivo y está contado así en el presupuesto.

## Instrumentos

Todos los parámetros se guardan como `u8` en **0–127** (o con signo cuando corresponde) y se mapean a valores físicos al usarlos.
Así los p-locks (v1), los sticks y el MIDI CC (después) usan el mismo rango.

| Mapeo | Fórmula (v = 0–127) | Ejemplos |
|---|---|---|
| Corte del filtro | f = 20 Hz · 1000^(v/127) (exponencial, 20 Hz–20 kHz) | 0 → 20 Hz · 64 → ~650 Hz · 127 → 20 kHz |
| Tiempos de envolvente | t = 1 ms · 10000^(v/127) (1 ms–10 s) | 0 → 1 ms · 40 → ~18 ms · 127 → 10 s |
| Resonancia | k del SVF de 2 a ~0,05 (lineal) | 127 = casi autooscilación |
| Volumen | dB = −60 + 66·(v/127), con 0 = silencio | 100 → ~−8 dB · 127 → +6 dB |

Todos los instrumentos tienen, en GENERAL: volumen, veloc>amp y **voces** (1–8).

### Percusión sintetizada (MVP)

| Modelo | Síntesis (receta en `dsp-recipes`) | Parámetros |
|---|---|---|
| BOMBO | seno con barrido exponencial de pitch + click de ruido | afinación, barrido, tiempo de barrido, decaimiento, golpe |
| CAJA | seno/triángulo ~180 Hz + ruido por pasa-banda | afinación, decaimiento, color (tono/ruido), golpe |
| HAT-CERRADO / HAT-ABIERTO | ruido (o 6 cuadradas inarmónicas) por pasa-altos ~7 kHz | afinación, decaimiento, color. Corte mutuo entre pistas "agrupadas" **(v1)** |
| PALMAS | 3–4 ráfagas de ruido filtrado separadas ~10 ms + cola | decaimiento, color, separación |
| TOM | seno con barrido corto | afinación, barrido, decaimiento |

Todos comparten filtro (corte, resonancia). Voces por defecto: 1.

### Sampler (MVP, ampliado en v1)

- Lectura con **interpolación lineal** y paso `ratio = (fs_sample / 48000) · 2^(semitonos/12)`. Interpolación cúbica (Hermite) en v1 si se oye aliasing al bajar el tono.
- Parámetros: afinación (±24 st), fino (±50 ct), inicio y fin (0–127 sobre el largo), filtro SVF, envolvente AD de amplitud, volumen, veloc>amp, voces.
- Con acordes, cada nota es una voz que lee el mismo sample a otra velocidad.
- **v1:** loop (adelante, ping-pong), reversa, rebanadas (8 o 16 partes iguales, elegibles por p-lock).

### Sustractivo (MVP, polifónico)

- Por voz: 1 oscilador **PolyBLEP** (sierra, cuadrada, pulso con ancho, triángulo) + suboscilador cuadrado una octava abajo + ruido (xorshift32).
- Filtro **SVF TPT** (Zavalishin/Cytomic) LP/BP/HP por voz, con envolvente (cantidad bipolar) y seguimiento de teclado.
- 2 envolventes ADSR exponenciales por voz (amplitud y filtro).
- **v1:** glide (en modo mono), LFO por pista (triángulo, cuadrada, S&H; destinos: corte, pitch, ancho de pulso, volumen), drive.
- `tan()` del SVF: recalcular solo cuando cambia el corte, una vez por subbloque, con aproximación o tabla.

### FM de 2 operadores (v1)

`salida = sin(2π·fase_c + índice · env_índice · sin(2π·fase_m + realim · salida_m_anterior))`, con tabla de seno de 2048 puntos
e interpolación lineal. Parámetros: relación del modulador (0,5 a 16, en pasos musicales), índice, envolvente del índice (AD), realimentación, envolvente de amplitud.

### Pista MIDI (v1)

No genera audio: envía notas (incluidos los acordes), velocidades y, desde los p-locks, CC. No consume voces de la bolsa. Ver [spec-formatos](spec-formatos.md#midi).

## Efectos

| Efecto | Diseño | Parámetros | Memoria | Prioridad |
|---|---|---|---|---|
| Delay estéreo (envío) | Buffer circular de 2^17 muestras por canal (~2,7 s), interpolación lineal, pasa-bajos de un polo en la realimentación, cruce L/R para ping-pong | tiempo sincronizado (1/16, 1/8, 3/16, 1/4, 3/8, 1/2, 1 compás), realimentación (hasta 95 %), tono, ancho, retorno | ~1 MB | MVP |
| Reverb liviana (envío) | FDN de 4 líneas (matriz de Hadamard) con pasa-bajos de amortiguación y 2 allpass de difusión a la entrada; alternativa: Schroeder chico. Elección final de `dsp-engineer` | tamaño, amortiguación, predelay, retorno | < 1 MB | MVP |
| Master | Ganancia + **soft clip** (aproximación racional de `tanh`); medidor de pico | ganancia, limitador sí/no | — | MVP |
| Limitador con lookahead | Detector de pico con 1–2 ms de anticipación | umbral, relajación | < 10 KB | v1 |
| Drive y bitcrusher por pista | Saturación asimétrica; reducción de bits y de sample rate | cantidad, bits, divisor | — | v1 |
| Compresor con sidechain | Seguidor de envolvente desde otra pista | umbral, ratio, tiempos, fuente | — | después |
| Chorus | 2 líneas moduladas | velocidad, profundidad, mezcla | ~50 KB | después |

**Implementado (H9)**, con estos desvíos ([dsp.md](dsp.md#mezcla-efectos-y-master-h9)): el delay usa 2^18 muestras por
canal (5,5 s, 2 MB los dos) para que "1 compás" entre desde 44 BPM; la reverb es el FDN de 4 líneas con 2 allpass,
"tamaño" es el RT60 (0,3–8 s) y las líneas no se modulan; el master lleva un **limitador de picos sin anticipación**
delante del soft clip ([ADR 0018](../../decisions/0018-limitador-sin-anticipacion-antes-del-soft-clip.md), propuesta) y
el de anticipación sigue en v1. Volumen y envíos con ley cuadrática (100 = 0 dB) y paneo seno/coseno normalizado a 0 dB
en el centro (no por tabla: se calcula al recibir `SetMix`). Comandos: `SetMix { track, strip }` (volumen, paneo,
envíos, mute y solo), `SetFx`, `SetMaster` y `Perform { track, offsets: [f32; 4] }`; evento `Meters` (picos de las 8
pistas y el master, voces por pista y de la bolsa, reducción del limitador) cada 1600 muestras.

## Secuenciador

Decisión: [ADR 0006](../../decisions/0006-cadenas-por-pista.md).

### Estructura

```
Canción: hasta 256 filas × 8 pistas; cada celda = cadena o vacío
   └─ Cadena (000–255): hasta 16 entradas (frase, transposición); la primera vacía la termina
        └─ Frase (000–255): 16 pasos de una pista
             └─ Paso: nota, acorde, velocidad, largo  (+ v1: 2 locks, condición, micro, retrig)
```

Las frases y las cadenas son **compartidas**: la misma frase puede aparecer en varias cadenas y la misma cadena en varias filas o pistas.
Editarla cambia todos los lugares donde se usa (la interfaz lo indica con `*`). Para variar una copia, se **clona** (A + Y).

### Reloj

- Resolución: **24 ticks por paso** (paso = semicorchea), o sea **96 PPQN**. Un pulso de MIDI clock (24 PPQN) cada 4 ticks.
- Duración de un paso en muestras: `fs · 60 / (BPM · 4)`; a 120 BPM y 48 kHz son 6000 muestras (250 por tick).
- La posición se acumula en `f64` (en muestras) para que no haya deriva en sesiones largas.
- El render avanza hasta el próximo tick, a lo sumo 32 muestras por vez; en cada tick se evalúan los eventos de las 8 pistas.
- Cambiar el tempo afecta desde el tick siguiente. El reloj es **global**: todos los cabezales comparten tempo y swing.

**Implementado (H3):** como en el MVP solo hay eventos al principio de cada paso (sin micro, retrig ni swing), el reloj
corta el render en los **pasos**, no en cada tick: el paso `n` desde el último Play o cambio de tempo cae en la muestra
`round(origen + n · muestras_por_paso)`, calculada en `f64` (no se acumula: N vueltas = N × largo exacto). Los largos se
pasan de ticks a muestras al disparar. Los subbloques de 32 muestras se alinean a la muestra absoluta y se cortan en el
paso, así el disparo cae en su muestra exacta dentro del buffer y, con buffers múltiplos de 32, el audio no depende del
tamaño de buffer. Un cambio de tempo conserva la fase del paso en curso (si faltaba medio paso, falta medio paso al tempo
nuevo). Cuando lleguen micro, retrig o swing (H4), el próximo evento pasa a ser `inicio del paso + desplazamiento`.

**Implementado (H4):** el swing ya usa ese esquema: el paso `n` cae en `round(origen + n · muestras_por_paso +
atraso)`, con `atraso = muestras_por_paso · (s − 50) / 50` si `n` es impar (el segundo de cada par) y 0 si es par. La
paridad se cuenta desde Play (no se reinicia con el tempo), así que coincide con el paso de cada cabezal: las frases
tienen 16 pasos y todos los cabezales arrancan juntos. Un cambio de tempo conserva la fase respecto de la **grilla
recta**: si el paso esperaba su swing (la grilla ya pasó), la fracción ya esperada se conserva al tempo nuevo.

### Swing

Con swing `s` en 50–75 %, los pasos pares de cada par (2, 4, 6…) se atrasan `duración_paso · (s − 50) / 50`.
50 % = recto; ~67 % = tresillo; 75 % = medio paso.

**Implementado (H4):** `SetSwing(u8)` (fuera de rango se recorta; al leer el archivo, también, y se cuenta como
"valor fuera de rango"). Se aplica desde el próximo paso: si al bajar el swing el paso que esperaba ya pasó su
momento, suena en la muestra siguiente (no se pierde). El largo de las notas no cambia con el swing (sigue en ticks
rectos). Se edita en Proyecto ([spec-ui](spec-ui.md#7-proyecto)).

### Cabezales por pista

Cada pista tiene su propio cabezal: `(fila de canción, entrada de cadena, paso)`.

1. Al terminar el paso 16 de una frase, pasa a la **entrada siguiente** de la cadena.
2. Si la entrada siguiente está vacía (o era la 16), pasa a la **fila siguiente** de la canción, en su columna.
3. Si esa celda está vacía, vuelve a la **primera fila de su bloque** (las celdas no vacías contiguas de su columna donde estaba).
4. Una pista cuya celda de inicio está vacía no suena.

Consecuencia conocida: si en una fila las cadenas tienen largos distintos, las pistas se desalinean. Es intencional (permite polirritmias
largas), pero la pantalla Canción muestra el largo de cada cadena y marca las filas desparejas. Para "silenciar" una pista
manteniendo la alineación, se usa una cadena con frases vacías.

**Implementado (H5):** `SongData::next_position(modo, pista, fila, entrada)` decide adónde va el cabezal al terminar el
paso 16; el hilo de audio lo llama solo en ese momento. Casos que la regla no cubría (desvíos documentados):

- Una cadena **sin entradas** en una celda dura una frase de silencio (no hay bucles de largo cero).
- Al terminar la **fila 255** (no hay siguiente) se vuelve al inicio del bloque.
- La primera fila del bloque se busca subiendo desde la fila actual mientras la de arriba tenga cadena (a lo sumo 255
  lecturas, solo al terminar una cadena). Si en vivo se vacía la celda que suena, al terminar la frase el cabezal va a
  la fila siguiente si tiene cadena y, si no, al inicio del bloque de arriba; sin bloque arriba se queda en esa fila,
  en silencio, y la vuelve a mirar en cada frase.

### Modos de reproducción

| Modo | Qué suena | Cómo se activa ([spec-ui](spec-ui.md#mapa-global-de-controles)) |
|---|---|---|
| `Song { from_row }` | Las 8 pistas desde la fila `R`, cada una con su cabezal | Start en Canción o en Grilla (desde la fila de la sección), y en Mezcla o Proyecto |
| `Section { row, entry }` | Las 8 pistas en loop sobre la sección `(R, k)`: cada pista repite su frase en la entrada `k` | Select + Start en cualquier pantalla. Es el "loop de beat" |
| `Chain { track, chain }` | Solo la pista actual, en loop sobre su cadena | Start en Cadena |
| `Phrase { track, phrase }` | Solo la pista actual, en loop sobre su frase | Start en Frase o en Instrumento |

Start con el secuenciador sonando lo detiene. Cambiar de pantalla no cambia lo que suena.

**Implementado (H3 y H5):** los cuatro modos, con el Start contextual de la tabla. Los cabezales guardan
`(fila, entrada, paso)` y resuelven la cadena y la frase en cada paso (un cambio en una celda, una entrada de cadena o la
FRA se oye en el paso siguiente). Al terminar el paso 16: `Section` y `Phrase` vuelven a la misma entrada, `Chain` pasa a
la entrada siguiente o vuelve a la primera, y `Song` aplica las reglas de "Cabezales por pista". En `Song`, una pista con
la celda de inicio vacía queda inactiva toda la reproducción; en `Chain` y `Phrase` solo la pista elegida está activa.
Una pista sin frase en su posición no suena pero su cabezal avanza igual. Select + Start con algo sonando también para.

Desvío: `Chain` y `Phrase` se identifican por **posición** (`Chain { track, row }`, `Phrase { track, row, entry }`) y
no por número de cadena o frase: el cabezal se resuelve igual que en la canción (editar la celda o la FRA mientras suena
se oye), el loop de frase suena con la transposición de su entrada y la interfaz dibuja el cabezal sin traducir IDs.
Stop suelta todas las voces sostenidas (siguen su relajación). El motor publica `Event::Playhead` (posición de las 8
pistas) cada vez que cambia el paso, y lo reintenta si la cola de eventos está llena.

**Vista previa de una frase (H4, pendiente del H5):** X en la Canción y la Cadena manda `PreviewPhrase { track,
phrase, transpose }`: los 16 pasos de esa frase suenan **una vez** en la pista, con un reloj propio que arranca en la
muestra en que llega el comando (suena aunque el secuenciador esté parado) y comparte tempo y swing. Al terminar el
paso 16 se sueltan las notas INF de la pista. No es un modo de reproducción: no mueve los cabezales ni cambia lo que
la interfaz cree que suena; Play y Stop la cortan, y otra X la reemplaza. Ignora el mute (como la audición).

### Edición en vivo

La interfaz es dueña del `Project`; el motor tiene su copia (`SongData`: 256 frases × 16 pasos, 256 cadenas × 16
entradas, 256 filas × 8 celdas, ~50 KB asignados una sola vez al crear el motor). Cada edición llega como un comando
`Copy` chico (`SetStep`, `SetChainEntry`, `SetSongCell`, `SetMix`, `SetTempo`...) que el callback aplica escribiendo en
el array: sin locks, sin asignar ni liberar.

En el hilo de interfaz, `EngineMirror` (`src/sync.rs`) guarda lo último que mandó y después de cada gesto compara el
proyecto entero (~10 000 comparaciones, microsegundos) y manda solo las diferencias. Así cualquier edición (poner o
borrar notas, acordes, velocidad, largo, clonar, cambiar la FRA, crear cadenas y filas) llega sin que cada lugar que
edita tenga que avisar. Si la cola está llena, lo que falta queda pendiente y se manda en orden en la vuelta
siguiente (los datos siempre antes que Play).

Un paso se lee recién cuando el cabezal lo dispara: lo que se edita adelante del cabezal suena en esta vuelta; lo de
atrás, en la siguiente. Lo mismo con la canción y las cadenas (H5): una celda o una entrada de cadena se leen cuando el
cabezal llega a ellas (por ejemplo, agregar una entrada a la cadena que suena la alarga en esta vuelta; vaciar la fila
siguiente hace que el cabezal vuelva al inicio del bloque). Una nota que ya está sonando conserva el largo con el que se disparó.

Alternativa descartada: mandar una copia inmutable de la sección (`Box`) en cada edición, con doble buffer y
devolución de la vieja por la cola de eventos para liberarla en la interfaz. Es más código (dos colas de memoria) y
copia 8 frases por cada cambio de un paso; los comandos por paso ya son lo que pide la arquitectura y alcanzan para H5
(el motor tiene cadenas y canción completas). Cargar un proyecto (H6) sí va a usar `ReplaceProject(Box<...>)`.

### Mute y solo (H3)

Se aplican en el secuenciador: una pista en mute (o fuera del solo, si hay alguno) no dispara notas y lo que tenía
sostenido se suelta en el momento. Lo que ya sonaba termina su relajación. La audición con X sigue sonando en una
pista en mute. Con la Mezcla (H9) se puede pasar a una ganancia suavizada si hace falta cortar las colas.
**H9**: se dejó así (las colas y los envíos ya mandados terminan solos); la Mezcla solo agrega mute y solo editables.

### Disparo de un paso

1. Si la nota es `---`, no pasa nada (salvo locks sin nota **(v1)**: "trigless locks", que solo cambian parámetros).
2. **(v1)** Evaluar la condición; si es falsa, no se dispara.
3. **(v1)** Aplicar los p-locks: valores temporales para esas voces.
4. Calcular las notas: `nota + transposición de la entrada de cadena`, más los intervalos del acorde con su inversión.
5. Asignar voces según [las reglas](#asignación-y-robo-de-voces) y dispararlas en `tick 0 + micro` **(v1: micro)**, con la velocidad del paso.
6. Programar el note-off de esas voces a los `gate_ticks`. `INF` = hasta la próxima nota de la pista (que libera todas sus voces). En percusión y sampler one-shot, el largo se ignora.
   Implementado (H3): la nota siguiente suelta solo las voces INF de la pista; las de largo fijo siguen hasta su note-off (se superponen). En una pista de 1 voz, la nota siguiente hace legato sobre la que suena.
7. **(v1)** Retrig: `n` disparos repartidos en el largo del paso, con caída de velocidad configurable.
8. `OFF` en la columna de nota libera todas las voces de la pista.

### Condiciones de paso (v1)

| Condición | Verdadera cuando… |
|---|---|
| `Chance(p)` | un número aleatorio (xorshift, semilla fija por proyecto) es menor que p % |
| `Cycle { a, b }` | `vuelta_de_la_frase % b == a − 1` (por ejemplo, 2:4 = la segunda de cada 4 vueltas de esa frase en ese lugar) |
| `Fill` / `NotFill` | el modo fill está activo / no lo está |
| `First` / `NotFirst` | es la primera vuelta desde que empezó a sonar / no lo es |
| `Pre` / `NotPre` | la última condición evaluada en la misma pista fue verdadera / falsa |

## Modelo de datos

Structs ilustrativos (los nombres finales los decide la implementación). Rangos en comentarios.
Las referencias entre frases, cadenas y canción son **IDs numéricos** (`u8`), no referencias de Rust: así varias cadenas pueden
"apuntar" a la misma frase sin pelear con el borrow checker, y el formato de archivo es directo.

```rust
pub const TRACKS: usize = 8;
pub const PHRASE_STEPS: usize = 16;
pub const CHAIN_LEN: usize = 16;
pub const MAX_PHRASES: usize = 256;    // 000..=255
pub const MAX_CHAINS: usize = 256;     // 000..=255
pub const MAX_SONG_ROWS: usize = 256;
pub const TICKS_PER_STEP: u16 = 24;
pub const VOICE_POOL: usize = 16;
pub const FADE_SLOTS: usize = 4;

pub type PhraseId = u8;
pub type ChainId = u8;

pub struct Project {
    pub format_version: u32,                 // 1 (ver spec-formatos)
    pub name: String,                        // [a-z0-9_-], hasta 32
    pub tempo_bpm: f32,                      // 40.0..=300.0
    pub swing: u8,                           // 50..=75 (%)
    pub tracks: [Track; TRACKS],
    pub song: Song,
    pub chains: Box<[Option<Chain>; MAX_CHAINS]>,
    pub phrases: Box<[Option<Phrase>; MAX_PHRASES]>,
    pub fx: FxSettings,                      // delay y reverb
    pub master: MasterSettings,              // ganancia, limitador
}

pub struct Track {
    pub name: String,                        // hasta 6 caracteres visibles en la grilla
    pub instrument: Instrument,              // fijo por pista en el MVP
    pub mix: ChannelStrip,
}

pub struct ChannelStrip {
    pub volume: u8,                          // 0..=127
    pub pan: i8,                             // -64..=63
    pub delay_send: u8,                      // 0..=127
    pub reverb_send: u8,                     // 0..=127
    pub mute: bool,
    pub solo: bool,
}

pub enum Instrument {                        // cada variante incluye `voices: u8` (1..=8)
    DrumSynth(DrumParams),                   // MVP
    Sampler(SamplerParams),                  // MVP
    Subtractive(SubtractiveParams),          // MVP
    Fm2(Fm2Params),                          // v1
    MidiOut(MidiOutParams),                  // v1
}

pub struct Song {
    pub rows: Vec<[Option<ChainId>; TRACKS]>, // hasta 256 filas
}

pub struct Chain {
    pub entries: [Option<ChainEntry>; CHAIN_LEN], // la primera None termina la cadena
}

pub struct ChainEntry {
    pub phrase: PhraseId,
    pub transpose: i8,                       // -48..=48 semitonos
}

pub struct Phrase {
    pub steps: [Step; PHRASE_STEPS],
    pub length: Option<u8>,                  // v1: largo variable 1..=16
}

pub struct Step {
    pub note: NoteCell,                      // Empty | On(12..=127) | Off
    pub chord: Chord,                        // MVP
    pub velocity: u8,                        // 1..=127
    pub gate_ticks: u16,                     // 6..=384 (1/4 a 16 pasos); u16::MAX = INF
    pub locks: [Option<ParamLock>; 2],       // v1
    pub condition: Condition,                // v1; Always por defecto
    pub micro: i8,                           // v1; -12..=12 ticks
    pub retrig: u8,                          // v1; 1..=8
}

pub struct Chord {
    pub kind: ChordKind,                     // None, Fifth, Octave, Major, Minor, Dim, Aug,
                                             // Sus2, Sus4, Maj7, Min7, Dom7, Min7b5, Add9
    pub inversion: u8,                       // 0..=3
}

pub struct ParamLock { pub param: ParamId, pub value: u8 }

pub enum Condition {
    Always, Chance(u8), Cycle { a: u8, b: u8 },
    Fill, NotFill, First, NotFirst, Pre, NotPre,
}
```

- `ParamId` es un `enum` con todos los parámetros que se pueden lockear (v1). Un lock a un parámetro que el instrumento no tiene se ignora.
- Los campos de v1 existen desde el **formato 1** (con valores por defecto), para no migrar archivos cuando lleguen.
- Tamaño en memoria: un `Step` ocupa ~20 bytes → una frase ~320 bytes → 256 frases ≈ **80 KB**; cadenas ~8 KB; canción ~2 KB. Un proyecto completo cabe en ~100 KB.
- **Proyecto nuevo:** canción con 1 fila; en esa fila, cada pista tiene su cadena (000 a 007) con una frase vacía (000 a 007). Tempo 120, swing 50. Kit por defecto: 1 BOMBO, 2 CAJA, 3 HH-CER, 4 HH-ABI, 5 PALMAS (percusión, 1 voz); 6 SAMPLE (vacío, 2 voces); 7 BAJO (sustractivo, 1 voz); 8 SINTE (sustractivo, 4 voces). Con esto, la Grilla funciona desde el primer momento.

## Presupuesto de CPU

**Todas las cifras son estimaciones** hasta que `device-ops` las mida en la consola (hito H11). Supuestos:

- Cortex-A35 a **1,3 GHz** (conservador: la hoja dice hasta 1,5 GHz; la frecuencia real depende del SO y del gobernador **(sin verificar)**). Es un núcleo en orden con doble emisión limitada, algo más lento que un A53 a igual frecuencia.
- Código escalar `f32` en `--release`, ~1 operación por ciclo de promedio contando dependencias y accesos a memoria. NEON queda como optimización posterior.
- 48 kHz → **27 083 ciclos por frame estéreo** en un núcleo. Objetivo: < 50 % → **13 541 ciclos por frame**.

### Costo por bloque

| Bloque | Operaciones aprox. por frame | Ciclos estimados por frame |
|---|---|---|
| Voz de sample (interpolación lineal, ganancia, paneo) | ~15 | 25–40 |
| Filtro SVF (por voz, cuando se usa) | ~10 + `tan()` por subbloque | 15–25 |
| Voz de percusión sintetizada (oscilador por tabla + ruido + SVF + 2 envolventes) | ~35 | 50–80 |
| Voz sustractiva (PolyBLEP + sub + ruido + SVF + 2 ADSR + suavizado) | ~60 | 90–140 |
| Voz FM de 2 operadores (v1) | ~30 | 50–70 |
| Voz FM de 4 operadores (después) | ~60 | 100–150 |
| Canal de mezcla (suma de voces, volumen, paneo, 2 envíos, medidor) | ~10 | 15–20 |
| Delay estéreo con pasa-bajos | ~15 | 25–40 |
| Reverb liviana (FDN de 4 líneas + 2 allpass) | 60–100 | 100–200 |
| Master (suma, ganancia, soft clip, medidor) | ~10 | 20–40 |
| Secuenciador: 8 cabezales, acordes, asignación de voces (amortizado) | — | < 20 |

### Totales

Costos fijos (8 canales × 20 + delay 40 + reverb 200 + master 40 + secuenciador 20) = **460 ciclos/frame**.

| Escenario | Cálculo (cota alta) | Ciclos/frame | Con factor ×2 | % de un núcleo a 1,3 GHz |
|---|---|---|---|---|
| **MVP típico**: 4 percusiones, 1 sample, bajo, acorde de 4 notas + 4 colas en relajación, 2 fundidos | 4×80 + 1×65 + 1×140 + 8×140 + 2×100 + 460 | ~2 305 | ~4 610 | **~17 %** |
| **MVP peor caso**: bolsa llena (16) + 4 fundidos, todas voces sustractivas | 20×140 + 460 | ~3 260 | ~6 520 | **~24 %** |
| Variante: bolsa de 24 + 4 fundidos | 28×140 + 460 | ~4 380 | ~8 760 | ~32 % |
| **v1 peor caso**: bolsa de 16 + 4 (estimación previa al ADR 0013, que pasó a 16 + 16; las ranuras de fundido duran 5 ms y su costo medido es chico), + 8 drives + limitador con lookahead | 20×140 + 8×25 + 460 + 40 | ~3 500 | ~7 000 | **~26 %** |

- Comparado con la propuesta monofónica anterior (~20 % en el peor caso), la polifonía suma unos **4 puntos** en el peor caso,
  porque la bolsa acota el total. El techo teórico con el factor ×2 es de unas **45 voces sustractivas** antes de llegar al 50 %.
- El riesgo real no es el promedio sino los **picos** (xruns por el SO, el gobernador de CPU o la interfaz), y eso solo se ve midiendo.
- Es coherente con que el M8 haga un motor parecido en un microcontrolador mucho más chico **(sin verificar sus specs internas)**.

### Hilo de interfaz (otro núcleo)

Redibujar 640×480 completo por software cuesta del orden de 1–3 millones de ciclos por cuadro → a 60 fps, ~5–15 % de un núcleo **(estimación)**.
Se reduce redibujando solo lo que cambia. La GPU (Mali-G31 con Panfrost o blob) puede ayudar vía SDL2 **(sin verificar en dArkOS)**.

### Cómo medir (para `device-ops` y `rust-dev`)

- El motor mide el tiempo de cada `render` contra la duración del buffer y publica el porcentaje y las voces en uso (evento `Load`), y los xruns.
- Prueba de estrés: un proyecto con 8 pistas sustractivas de 4 voces, acordes de 4 notas en todos los pasos y largo `INF` (bolsa siempre llena, robo constante). Render offline de 60 s en la consola, cronometrado.
- Registrar la frecuencia real de la CPU (`/sys/devices/system/cpu/cpu0/cpufreq/`) durante la medición.

## Memoria

| Uso | Tamaño |
|---|---|
| Samples en `f32` (límite propuesto) | **128 MB** (~11 min mono a 48 kHz) |
| Delay + reverb | ~2 MB |
| Proyecto en memoria (interfaz + copia del motor) | ~0,2 MB |
| Bolsa de voces (estado de 20 voces) | < 50 KB |
| Deshacer, 32 niveles (v1) | ~4 MB |
| Binario, SDL2, fuentes, pilas | ~20–40 MB **(estimación)** |
| **Total** | **< 200 MB** de 1 GB (el SO usa una parte, **sin verificar** cuánto) |

Los samples se guardan en `f32` por simplicidad. Si el límite queda corto, pasar a `i16` (mitad de memoria) con conversión en la lectura.

## Preguntas abiertas

1. ¿**48 kHz** (propuesto) o 44,1 kHz? Depende de la frecuencia nativa del códec, a confirmar en el inventario.
2. ¿Límite de **128 MB** para samples, o más?
3. ¿Bolsa de **16 voces + 4 de fundido** (propuesto) o 24? Las dos entran en el presupuesto.
5. ¿Frases de **16 pasos fijos** en el MVP (propuesto) y largo variable en v1?
6. Kit por defecto: ¿4 percusiones + 1 sample + bajo + sinte poli (propuesto) u otra combinación?

## Fuentes

- Skills del proyecto: `dsp-recipes` (PolyBLEP, SVF TPT, envolventes, percusión, FM, delay) y `rust-realtime-audio` (arquitectura, `rtrb`, reglas del callback).
- Vadim Zavalishin, *The Art of VA Filter Design* (SVF TPT).
- Välimäki y Huovilainen, *Antialiasing oscillators in subtractive synthesis* (PolyBLEP).
- Jot y Chaigne (1991) y Julius O. Smith, *Physical Audio Signal Processing* (FDN para reverbs).
- [Manual M8 v3.2.0](https://cdn.shopify.com/s/files/1/0455/0485/6229/files/m8_operation_manual_v20230630.pdf): estructura canción/cadena/frase, cabezales por pista, filas aisladas en la canción, especificaciones.
- [learnlsdj](https://learnlsdj.github.io/index.html) y [tutorial TM8C](https://cs.uwaterloo.ca/~plragde/flaneries/TM8C/Prelude.html) (modelo de LSDJ).
- [Arm: Introducing Cortex-A35](https://developer.arm.com/community/arm-community-blogs/b/architectures-and-processors-blog/posts/introducing-cortex-a35-arm-s-most-efficient-application-processor), [AnandTech: ARM announces Cortex-A35](https://anandtech.com/show/9769), [SemiAccurate: A35](https://semiaccurate.com/?p=37720) (rendimiento relativo del A35).
- [Hardware de la RGB20S](../../hardware/rgb20s.md).
