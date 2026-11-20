# groovebox

Groovebox para la Powkiddy RGB20S. Diseño y hitos en [docs/solutions/groovebox](../../docs/solutions/groovebox/README.md).

Estado: **H6** (guardar y abrir proyectos en RON: con esto se cumple el objetivo mínimo del MVP, "un beat de 4
compases guardado") sobre **H5** (cadenas y canción: cabezales por pista con la regla de bloques, modos canción / sección / cadena /
frase, Start contextual, pantallas Canción y Cadena editables, seguir reproducción) sobre **H3** (secuenciador, edición
en vivo, cabezales), **H2** (motor de sonido: bolsa de voces, percusión sintetizada, sinte, medición de CPU) y **H1**
(input, fuente bitmap, Grilla y Frase editables). Probado en la Mac (nativo y sin escritorio) y emulado (aarch64); **sin probar en la consola**. Diseño del
DSP: [dsp.md](../../docs/solutions/groovebox/dsp.md). Secuenciador: [spec-motor](../../docs/solutions/groovebox/spec-motor.md#secuenciador).

## Qué hace

- **Canción (H5)**: cada pista avanza por su columna con su propio cabezal: frase por frase por su cadena (con la
  transposición de cada entrada), al terminar la cadena pasa a la fila siguiente de su columna y, si esa celda está
  vacía, vuelve a la primera fila de su **bloque** (celdas no vacías contiguas). Si las cadenas de una fila tienen
  largos distintos, las pistas se desalinean (como en LSDJ/M8). Una pista cuya celda de inicio está vacía no suena.
- **Start contextual** (spec-ui, "Qué reproduce Start"): en Canción, Grilla, Mezcla y Proyecto, la **canción** desde la
  fila del cursor (o de la sección); en Cadena, el **loop de la cadena** (solo esa pista); en Frase e Instrumento, el
  **loop de la frase** (solo esa pista). **Select + Start**: loop de la **sección** `(fila, entrada)` con las 8 pistas,
  en cualquier pantalla. Con algo sonando, los dos paran. Mantener Select + Start 1 s sigue saliendo de la app.
- **Seguir reproducción** (Proyecto, activado por defecto): mientras suena, la Grilla y la Frase saltan a la sección
  del cabezal de la pista del cursor cada vez que ese cabezal cambia de sección (entre medio, L2/R2 pueden mirar otra).
  En la Grilla cada pista muestra la frase donde está su propio cabezal: la FRA en verde y una línea "en otra
  sección: PALMA fila 00 sec. 2" dicen cuáles están desalineadas.
- **Pantalla Canción**: filas × 8 pistas con el número de cadena (`*` si se usa en más de un lugar), la columna LARGO
  con las frases de cada cadena y `!` en las filas desparejas, la celda que suena en cada pista en verde con una barra
  de avance por la cadena, y abajo el aviso de la fila ("HH-CE 3, PALMA 2; las demás 4. Las pistas se desalinean"), la
  cadena del cursor (frases, usos) y la duración de su bloque. Edición: A pone la última cadena usada (si no hay, una
  nueva); doble toque, una cadena nueva vacía; A + cruz cambia el número (±1 / ±10; crea la cadena si no existía);
  A + B vacía la celda; A + Y clona la cadena. B + ↑/↓ y L2/R2 saltan de a 16 filas; la fila siguiente a la última
  se puede usar para extender la canción.
- **Pantalla Cadena**: las 16 posiciones (FRASE y TRANSP.) de la cadena de la pista en la fila, con el cabezal (`>` y
  barra de avance) y una vista previa de la frase bajo el cursor (con la fuente chica). En FRASE, los mismos gestos que
  la FRA de la Grilla (A última frase, doble toque nueva, A + cruz número, A + B quitar la entrada, A + Y clonar; crea
  la cadena si la fila no tenía). En TRANSP., A + ←/→ ±1, A + ↑/↓ ±12 (−48…+48) y A + B la pone en 0. L2/R2 cambian
  de fila.
- **Navegación con contexto**: Canción → Grilla abre la sección `(fila del cursor, 0)` en la pista del cursor;
  Grilla → Cadena abre la cadena de esa pista y fila con el cursor en la sección; Cadena → Frase abre la frase de la
  posición del cursor. La pista y la fila se comparten entre todas las pantallas.
- **Secuenciador (H3)**: Select + Start reproduce en loop la **sección** que se ve en la Grilla: cada pista
  repite la frase que tiene en esa fila y entrada de cadena (percusión, bajo y sinte con acordes, con su velocidad, su
  largo y la transposición de la entrada). Vive en el hilo de audio con reloj en muestras: cada paso
  suena en su muestra exacta dentro del buffer, sin deriva (la posición se calcula, no se acumula). El tempo
  (Select + L2/R2) cambia desde el paso siguiente conservando la fase. Mute y solo (Select + X / Y) se aplican al
  reproducir: la pista no dispara y lo que tenía sostenido se suelta. LARGO INF dura hasta la próxima nota de la pista.
- **Edición en vivo**: todo lo que se edita mientras suena (notas, acordes, velocidad, largo, FRA, clonar, crear
  frases, celdas de la canción, entradas y transposición de las cadenas) llega al motor como comandos chicos; un paso que se edita adelante del cabezal suena en esta vuelta, uno de
  atrás en la siguiente. Mecanismo en [spec-motor](../../docs/solutions/groovebox/spec-motor.md#edición-en-vivo).
- **Cabezal**: en la Grilla, barras verdes arriba y abajo del paso que suena en cada pista (grises si está en mute) y el
  número de paso en verde; en la Frase, `>` y el número en verde; en la Cadena, `>` en la posición; en la Canción, la
  celda en verde. En la Grilla y la Frase solo se ve si la sección que suena es la que se mira.
  En la barra de título, `>` = sonando, `■` = parado.
- **Grilla**: las 8 pistas × 16 pasos de la sección actual `(fila, entrada de cadena)`, con encabezados, columna
  FRA, panel de información y pie con ayuda y minimapa. Paso con nota = bloque del color del tipo de pista
  (percusión naranja, sample turquesa, sinte violeta) con intensidad según la velocidad; acorde = rayas; largo =
  barra que sigue hasta donde llega la nota; pista en mute atenuada con `M`.
- **Frase**: vista tracker de la frase de la pista actual en la sección (PASO, NOTA, ACORDE, VEL, LARGO; las
  columnas de v1 se ven deshabilitadas con la fuente chica y no aparecen con la grande).
- **Creación implícita**: poner una nota donde la pista no tiene frase crea la fila de la canción, la cadena y/o
  la frase que falten (solo en la primera entrada libre de la cadena; si hay un hueco, avisa).
- **Instrumento** y **Mezcla**: pendientes.
- **Proyecto (H6)**: nombre (renombrar), **GUARDAR**, **GUARDAR COMO...**, **CARGAR...** y **NUEVO**; los ajustes
  (fuente 8×16 / 12×24, audición al editar, seguir reproducción), SALIR, la carpeta de datos y, si entran (con la
  fuente chica), una **prueba de controles** (botones apretados y posición de los sticks) y una muestra de texto en
  los dos tamaños. Detalle abajo, en "Proyectos y carpeta de datos".
- **Cambios sin guardar**: `*` junto al nombre en el título de todas las pantallas y `* sin guardar` en Proyecto.
  NUEVO, CARGAR y salir (SALIR o Select + Start mantenido) preguntan antes de descartarlos.
- **Audición**: X hace sonar el paso (o el instrumento con la última nota) con el instrumento real de la pista; poner y
  editar notas también suena. Los acordes disparan varias voces; el largo (LARGO) se respeta al tempo actual (INF = 16 pasos).
- **Motor (H2)**: bolsa de 16 voces + 4 de fundido con las reglas de robo del ADR 0007; kit por defecto: BOMBO, CAJA,
  HH-CER, HH-ABI y PALMAS sintetizados, SAMPLE en silencio (llega en H7), BAJO (sierra + filtro, mono con legato) y
  SINTE (sierra + filtro, 4 voces). Soft clip en el master, volúmenes suavizados, flush-to-zero en el hilo de audio.
- **Carga de CPU**: `CPU n%` en la barra de título, la línea "CPU de audio" en Proyecto (promedio, pico y voces) y una
  línea en el log cada 10 s con CPU, voces máximas y **notas disparadas** en el intervalo (si dice `notas 0`, al motor
  no llegó nada). Las voces se miden en cada subbloque de 32 muestras, no al final del callback.
- Redibuja solo cuando cambia algo (y duerme hasta el próximo evento o repetición); sonando, revisa los cabezales cada 8 ms.

## Controles

Según el [mapa global de spec-ui](../../docs/solutions/groovebox/spec-ui.md#mapa-global-de-controles).

| Control | Solo | + A (mantener) | + B (mantener) | + Select |
|---|---|---|---|---|
| Cruz | Mover el cursor (repite: 250 ms y cada 33 ms) | Cambiar el valor: ←/→ ±1, ↑/↓ ±12 en notas, ±10 en FRA y VEL; ×4 tras 1 s | Salto: primera/última pista o paso | Cambiar de pantalla |
| A | Poner nota (última nota, acorde, velocidad y largo de la pista). En FRA / FRASE vacía: última frase usada; en una celda de la Canción vacía: última cadena; doble toque: frase o cadena nueva | — | Borrar (A + B) | Pegar (H10) |
| B | Cancelar / volver (en Proyecto) | Borrar | — | — |
| X | Escuchar | — | — | Mute de la pista |
| Y | Selección (H10) | Clonar la frase (en FRA, FRASE o la Frase) o la cadena (en la Canción) | — | Solo de la pista |
| L1 / R1 | Pista − / + | Acorde − / + (no en percusión) | — | Octava base − / + |
| L2 / R2 | Grilla: sección − / + (pasa a la fila siguiente; incluye una sección y una fila nuevas). Canción: 16 filas. Cadena: fila. Frase: entrada de la cadena | — | — | Tempo − / + |
| Start | Reproducir / parar según la pantalla (canción, cadena o frase) | — | — | Loop de la sección / parar. **Mantener 1 s: salir** (con cambios sin guardar, pregunta) |

En la Frase, sobre ACORDE: A + ←/→ cambia el tipo y A + ↑/↓ la inversión; sobre LARGO, A + ←/→ recorre
1/4, 1/2, 3/4, 1…16 e INF. A + B en ACORDE, VEL o LARGO vuelve al valor por defecto; en NOTA borra el paso.
Mientras se mantiene Select, el minimapa se agranda.

### Proyecto y modales (H6)

| Dónde | Controles |
|---|---|
| Proyecto | ↑/↓ elegir; **A** ejecuta (GUARDAR, GUARDAR COMO..., CARGAR..., NUEVO, SALIR; sobre "nombre", renombrar) o cambia un ajuste (A + cruz también); B o Select + ↓ vuelven |
| Editor de nombres | ←/→ mover (también con A apretado); **A + ↑/↓** cambiar la letra (también ↑/↓ solos); en el hueco del final, agregar; B + ←/→ principio o final; **A + B** borrar; **Start** aceptar; B cancelar |
| Cargar proyecto | ↑/↓ mover; B + ↑/↓ de a 10; **A** (o →) abrir; B cancelar |
| Diálogo de cambios sin guardar | **Start** guardar y seguir; **A** seguir sin guardar; **B** cancelar |
| Diálogo del `.bak` / de reemplazar | **A** abrir la copia / reemplazar; **B** cancelar |

Con un modal abierto, Select + cruz, los gatillos, Start (salvo para aceptar) y X no hacen nada. En los diálogos solo
confirma un toque de A separado: el segundo toque de un doble toque se ignora.

### Teclado del simulador

Mapeo de [`infra/sim/keymap.toml`](../../infra/sim/keymap.toml) (si falta `RGB20S_KEYMAP`, la app usa una copia incorporada):

| Consola | Teclado |
|---|---|
| Cruz | flechas |
| A / B / X / Y | `X` / `Z` / `S` / `A` |
| L1 / R1 | `Q` / `W` |
| L2 / R2 | `1` / `2` |
| Select / Start | `Shift` derecho / `Enter` |
| FN (no se usa) | `Esc` (en el simulador, Esc sale; con cambios sin guardar pregunta y un segundo Esc sale igual) |
| Stick izquierdo | `I` `J` `K` `L` |
| Stick derecho | `8` `4` `2` `6` del teclado numérico |

Ejemplos: poner una nota y subirla un semitono = `X`, y sin soltar `X`, `→`. Borrar = `X` + `Z`. Ir a la Frase =
`Shift` derecho + `→` dos veces. Salir = `Shift` derecho + `Enter` durante 1 s.

En la consola los botones llegan como GameController de SDL; L2/R2 analógicos (ejes) se convierten en botones con
histéresis, y los sticks se leen con zona muerta del 15 %.

Los botones frontales siguen la disposición **Nintendo** de la consola: A a la derecha, B abajo, Y a la
izquierda y X arriba. SDL los nombra por posición al estilo Xbox, así que el código intercambia A↔B y X↔Y
(`PadLayout` en `src/input.rs`). Si en la consola quedan al revés, `RGB20S_PAD_LAYOUT=xbox` usa los
nombres de SDL tal cual **(sin verificar en el hardware)**.

## Fuente

**Spleen 2.3.0** de Frederic Cambus, licencia **BSD-2-Clause** ([`fonts/LICENSE-spleen`](fonts/LICENSE-spleen)),
de <https://github.com/fcambus/spleen/releases/tag/2.3.0> (sha256 del tarball
`7b5729aaa8378e9987eccdb98c644180a875dcc22ee7258d8be3791238b87a02`). Se usan los BDF sin modificar en sus tamaños
nativos **8×16** y **12×24** (1018 glifos cada uno: ASCII, Latin-1 con acentos y ñ, y símbolos). Se embeben en el
binario con `include_str!`, se parsean al arrancar y se arma un atlas en una textura: no hace falta SDL2_ttf ni
archivos externos. La licencia pide reproducir el aviso de copyright en las distribuciones binarias: hay que
copiar `fonts/LICENSE-spleen` junto al binario al empaquetar (`just deploy-sd` ya lo hace: copia todo `LICENSE*` del crate junto al binario).

Cambio de tamaño: Proyecto → fuente (A), o `GROOVEBOX_FONT=12` al arrancar. Con 12×24 la Grilla muestra solo los
números de tiempo (1, 5, 9, 13) y el del cursor, y la Frase pierde las columnas de v1 y las líneas de ayuda.

## Proyectos y carpeta de datos

```
<datos>/
  config.ron                    último proyecto abierto y ajustes de Proyecto
  proyectos/<nombre>/
    proyecto.ron                el proyecto (RON, formato 1)
    proyecto.ron.bak            la versión anterior
```

- `<datos>` = `GROOVEBOX_DATA`, o `$XDG_DATA_HOME/groovebox`, o `~/.local/share/groovebox`. En la consola, con la
  imagen de Arch, `/home/alarm/.local/share/groovebox/` **(sin verificar)**. La app lo escribe en el log al arrancar
  (`datos: ...`) y lo muestra en Proyecto.
- Al arrancar se abre el último proyecto (`last_project` de `config.ron`) o, si no hay, uno nuevo `proyecto-NNN`.
  `GROOVEBOX_DEMO=1` sigue abriendo la demo (se puede guardar: queda en `proyectos/demo/`).
- **Guardado atómico**: `proyecto.ron.tmp` + `fsync`, el `proyecto.ron` anterior pasa a `.bak` y el `.tmp` a
  `proyecto.ron`. Si `proyecto.ron` no se puede leer y hay `.bak`, la app ofrece abrir la copia; abierta, queda con
  cambios sin guardar y GUARDAR repara `proyecto.ron` (sin pisar el `.bak` bueno con el roto).
- Un archivo de un formato más nuevo se abre en **solo lectura** (GUARDAR se niega; GUARDAR COMO con otro nombre sí).
- Los errores de lectura y escritura se ven en la línea de mensajes (y, si hubo cosas corregidas al leer, cuántas).
  Todo corre en el hilo de interfaz: el audio no hace E/S.
- Formato: [spec-formatos](../../docs/solutions/groovebox/spec-formatos.md#implementado-h6). Ejemplo abreviado:

```ron
// groovebox: archivo de proyecto (formato en docs/solutions/groovebox/spec-formatos.md)
#![enable(implicit_some)]
(
    format_version: 1,
    app_version: "0.1.0",
    name: "beat_01",
    tempo_bpm: 120.0,
    swing: 50,
    tracks: [
        (
            name: "BOMBO",
            instrument: DrumSynth((model: Kick, voices: 1, level: 110, vel_to_amp: 127, tune: 64, decay: 84, sweep: 76, sweep_time: 51, color: 64, punch: 50, spread: 48)),
            mix: (volume: 100, pan: 0, delay_send: 0, reverb_send: 0),
        ),
        // ... las otras 7 pistas ...
    ],
    song: [
        (0, 1, 2, 3, 4, 5, 6, 7),
    ],
    chains: {
        0: [
            (phrase: 0),
        ],
        // ...
    },
    phrases: {
        0: [
            (i: 0, note: On(60)),
            (i: 4, note: On(60)),
        ],
        // ...
    },
)
```

## Uso

```sh
just run          # nativo en la Mac (simulador)
just test         # tests (modelo, edición, gestos, fuente, guion, motor, secuenciador, archivos)
just build-arm    # binario aarch64
just check-arm    # glibc y librerías requeridas
just run-arm      # aarch64 emulado (sin pantalla ni audio)
just bench        # costo del motor, nativo en release (ver abajo)
```

Subcomandos sin ventana ni audio (sirven también con `just run-arm`):

```sh
groovebox render [carpeta]   # h5-cancion.wav, h3-beat.wav, h2-kit.wav y h2-robo.wav (48 kHz, 16 bits, estéreo)
groovebox bench [segundos]   # bolsa llena, demo en loop de secciones y en modo canción, secuenciador solo, motor vacío y percusión
just run-arm bench 5         # lo mismo emulado: solo para comparar, no es el número de la consola
```

| Variable | Uso |
|---|---|
| `GROOVEBOX_FONT=12` | arrancar con la fuente de 12×24 |
| `GROOVEBOX_REPEAT=250,33` | repetición de la cruz: espera e intervalo en ms |
| `GROOVEBOX_AUDIO=0` | sin audio |
| `GROOVEBOX_AUDIO=null` | el motor corre en un hilo propio al ritmo real, sin dispositivo ni salida (probar sin placa de sonido: cabezales, carga, log) |
| `GROOVEBOX_DEMO=1` | arrancar con el proyecto de demostración (canción de 3 filas con batería, bajo y acordes) |
| `GROOVEBOX_DATA=carpeta` | carpeta de datos (proyectos y `config.ron`); para probar sin tocar la real |
| `GROOVEBOX_FRAMES` | período de audio pedido (defecto 256; para medir 128/512 en la consola) |
| `GROOVEBOX_SCRIPT=archivo` | guion de input y capturas (abajo) |

Las de `RGB20S_*` están en [infra/README.md](../../infra/README.md).

### Guion de input y capturas

`GROOVEBOX_SCRIPT` ejecuta un guion (`tap`, `press`, `release`, `wait`, `shot`, `quit`; formato en `src/script.rs`)
contra el mismo camino de input que el teclado y el gamepad, y guarda capturas BMP de la pantalla. Sirve para
probar sin escritorio y para sacar capturas en la consola. [`scripts/demo-h1.txt`](scripts/demo-h1.txt) arma un
beat, edita largos en la Frase, recorre las pantallas y compara las fuentes:

```sh
cd /carpeta/de/capturas
SDL_VIDEODRIVER=offscreen RGB20S_SIM=1 GROOVEBOX_AUDIO=0 \
  GROOVEBOX_SCRIPT=/ruta/a/projects/groovebox/scripts/demo-h1.txt /ruta/al/groovebox
```

Con el binario aarch64 emulado (QEMU) las 9 capturas salen idénticas píxel a píxel a las nativas.

[`scripts/demo-h3.txt`](scripts/demo-h3.txt) es el flujo F1: pone un bombo, Start, agrega caja y hi-hats mientras
suena, mute, tempo, la Frase con el cabezal y Stop. Necesita el motor andando: `GROOVEBOX_AUDIO=null` (sin salida) o
el audio real.

[`scripts/demo-h5.txt`](scripts/demo-h5.txt) usa el proyecto demo (`GROOVEBOX_DEMO=1` y `GROOVEBOX_AUDIO=null`): la
Canción parada y con el aviso de la fila 2, Start en la Canción, una cadena nueva puesta y borrada mientras suena, la
Grilla siguiendo la reproducción (fila 1 y la desalineación de la fila 2), la Cadena del SINTE con su cabezal, la
transposición editada y el loop de la cadena, y la opción en Proyecto (10 capturas).

[`scripts/demo-h6.txt`](scripts/demo-h6.txt) arranca con una carpeta de datos vacía: arma un bombo, Proyecto con
`* sin guardar`, GUARDAR, GUARDAR COMO con el editor de nombres (escribe `beat_01`), Proyecto con la fuente grande,
NUEVO con el diálogo de cambios sin guardar, CARGAR con la lista, la Grilla del proyecto abierto y el diálogo de
salida (Start guarda y sale): 11 capturas. [`scripts/demo-h6-bak.txt`](scripts/demo-h6-bak.txt), después de romper
`proyecto.ron`, muestra el diálogo del `.bak`, el proyecto recuperado y GUARDAR reparándolo (3 capturas):

```sh
export SDL_VIDEODRIVER=offscreen RGB20S_SIM=1 GROOVEBOX_AUDIO=0 GROOVEBOX_DATA=/tmp/gb-datos
GROOVEBOX_SCRIPT=scripts/demo-h6.txt target/debug/groovebox
echo basura > /tmp/gb-datos/proyectos/beat_01/proyecto.ron
GROOVEBOX_SCRIPT=scripts/demo-h6-bak.txt target/debug/groovebox
```

Los guiones que pasan por Proyecto escriben `config.ron` en la carpeta de datos: conviene usar `GROOVEBOX_DATA`.
Con `just run-arm` (QEMU), `GROOVEBOX_DATA` tiene que estar dentro de `RUN_ARM_OUT` para poder escribirse; las
capturas salen iguales a las nativas salvo la ruta de datos, las fechas y algún mensaje que vence antes por la lentitud.

El proyecto demo (`src/demo.rs`) es una canción de 3 filas: **A** (4 compases: Am7 F G E, la batería con relleno en el
4), **B** (2 compases: las frases de A transpuestas por la cadena a Dm7 y Em7) y **C** (4 compases: la progresión
+3). En la fila 2 los hi-hats cerrados tienen una cadena de 3 frases y las palmas de 2: la fila sale marcada con `!`
y esas pistas vuelven a la fila 0 antes que el resto. SAMPLE no tiene cadenas.

- `h5-cancion.wav`: la canción en modo canción desde la fila 0, 10 frases (lo que tarda la pista más larga en recorrer
  el bloque; las palmas y los hi-hats ya volvieron a la fila 0) y Stop.
- `h3-beat.wav`: las 4 secciones de la fila 0, cada una en loop 2 vueltas (Select + Start en cada una) y Stop.

Los dos salen idénticos byte a byte con el binario nativo y con el aarch64 emulado (`just run-arm render`).

## Estructura

| Archivo | Contenido |
|---|---|
| `src/main.rs` | Ventana, bucle de eventos (duerme hasta el próximo evento o vencimiento), traducción a botones, guion, capturas |
| `src/input.rs` | `enum Button`, keymap (TOML, botones y sticks), gamepad, sticks con zona muerta, gatillos analógicos |
| `src/gestures.rs` | Botones → gestos: modificadores A/B/Select, A + B, doble toque, repetición, Select + Start mantenido. Sin SDL |
| `src/model.rs` | Proyecto: canción → cadenas → frases → pasos (IDs `u8`), acordes, largos, creación implícita. Sin SDL |
| `src/app.rs` | Estado de la interfaz y edición a partir de gestos; archivo (guardar, abrir, nuevo, renombrar, salir) y modales. Sin SDL |
| `src/project_file.rs` | Formato de archivo (`ProjectFile` disperso, versión, conversión y validación hacia `Project`). Sin I/O |
| `src/storage.rs` | Carpeta de datos, guardado atómico con `.bak`, lista de proyectos, `config.ron` |
| `src/modal.rs` | Editor de nombres, navegador de proyectos y diálogos (estado y edición) |
| `src/script.rs` | Guion de input |
| `src/ui/` | Dibujo: tema, barra de título, pie, minimapa (`mod.rs`), fuente BDF y atlas (`font.rs`), Grilla, Frase, Canción, Cadena y Proyecto (`screens.rs`), modales de archivo (`files.rs`) |
| `src/audio.rs` | Salida con cpal (48 kHz f32 preferido) o backend nulo; `Processor`: lo que corre en el callback (comandos, render, conversión, medición, eventos de carga y cabezales) |
| `src/sync.rs` | `EngineMirror`: manda al motor las diferencias entre el proyecto y lo último que se le mandó |
| `src/demo.rs` | Proyecto de demostración (render, bench y `GROOVEBOX_DEMO=1`) |
| `src/engine/mod.rs` | `Engine`: comandos, eventos, render con disparos en la muestra exacta, `note_on`, mezcla por pista, master con soft clip. Sin cpal |
| `src/engine/sequencer.rs` | Secuenciador: copia de frases, cadenas y canción (`SongData`), regla de bloques (`next_position`), reloj, cabezales por pista, modos de reproducción |
| `src/engine/seq_tests.rs` | Tests del secuenciador (tiempos exactos, deriva, tempo, gate, INF, edición en vivo, mute/solo, cabezales, cadenas, canción, bloques, desalineación, modos) |
| `src/engine/pool.rs` | Bolsa de voces y ranuras de fundido, reglas de robo |
| `src/engine/drum.rs`, `synth.rs` | Percusión sintetizada y voz sustractiva |
| `src/engine/dsp.rs` | SVF TPT, PolyBLEP, seno polinómico, xorshift, soft clip, suavizado, flush-to-zero |
| `src/engine/load.rs` | Medidor de carga (CPU promedio y pico por ventana de 0,5 s) |
| `src/engine/tests.rs` | Tests del motor (envolventes, robo sin clicks, aliasing, determinismo, sin asignaciones) |
| `src/instrument.rs` | Parámetros de instrumento en 0–127, mapeos de la spec y kit por defecto |
| `src/offline.rs` | `render` a WAV (la canción y el beat, con el secuenciador) y `bench` |
| `src/alloc_check.rs` | (tests) asignador que cuenta asignaciones por hilo |
| `fonts/` | Spleen 8×16 y 12×24 (BDF) y su licencia |
| `scripts/` | Guiones de input |

## Dependencias

H2 agrega `hound` (escribir WAV en `groovebox render`). H3 y H5 no agregan dependencias. H6 agrega `ron`.

| Crate | Por qué |
|---|---|
| `sdl2 0.39` (sin `bundled` ni `static-link`) | Video e input; enlaza la `libSDL2` de la consola (ADR 0008) |
| `cpal 0.18` | Salida de audio (ALSA) |
| `rtrb 0.3` | Colas SPSC sin locks: comandos interfaz → motor y eventos motor → interfaz |
| `hound 3.5` | Escribir WAV (render offline) |
| `serde 1` + `toml 1` | Leer `infra/sim/keymap.toml` (luego, la config de botones) |
| `ron 0.12` | Proyecto y `config.ron` en RON ([ADR 0004](../../docs/decisions/0004-formato-de-proyecto-ron.md)); trae `bitflags`, `typeid` y `unicode-ident` |

## Desvíos respecto de la spec

- **Archivo (H6)**: NUEVO también pide confirmación con cambios sin guardar; el diálogo suma **Start: guardar y
  seguir**; "nombre" renombra (mueve la carpeta) y GUARDAR COMO guarda una copia; en el editor de nombres ↑/↓ sin A
  también cambian la letra; el navegador muestra las fechas en UTC. Tempo y swing no se editan en Proyecto todavía.
  Detalle en [spec-ui](../../docs/solutions/groovebox/spec-ui.md#7-proyecto).
- **Formato (H6)**: filas de la canción como tuplas, `implicit_some`, instrumentos con los parámetros del motor actual
  y parámetros opcionales con el valor por defecto de su modelo; un `proyecto.ron` ilegible no se rota a `.bak`.
  Detalle en [spec-formatos](../../docs/solutions/groovebox/spec-formatos.md#implementado-h6). Sin autoguardado.

- **Salir**: la spec no define una combinación para salir (Select + Start es "loop de la sección"). Se usa
  **Select + Start mantenido 1 s**; un toque corto sigue siendo el loop (H3). También Proyecto → SALIR.
- **OFF** existe en el modelo y se dibuja, pero la spec no dice cómo se ingresa: no se puede poner todavía.
- **Sección y fila nuevas**: L2/R2 llegan una sección más allá de la cadena más larga de la fila, y una fila más
  allá de la última, para poder extender con la creación implícita.
- **Doble toque de A en un paso**: igual que un toque (la spec solo lo define para números de frase o cadena).
- Las columnas FRA y PISTA de la Grilla, los números de frase y el minimapa siguen los mockups; el minimapa del
  pie es gráfico (bloques) en lugar de texto, para que entre con las dos fuentes.

- **Select + Start con algo sonando** también para (la spec no lo define).
- **Modos de una pista por posición**: la spec define `Chain { track, chain }` y `Phrase { track, phrase }`; se usa
  `Chain { track, row }` y `Phrase { track, row, entry }`. El cabezal resuelve la cadena y la frase en cada paso igual
  que en la canción: cambiar la celda o la FRA mientras suena se oye en el paso siguiente, el loop de frase toca con
  la transposición de su entrada (lo mismo que se oye en la canción) y el cabezal se dibuja en la pantalla que
  corresponde sin traducir IDs.
- **Regla de bloques, casos que la spec no cubre**: una cadena sin entradas dura una frase de silencio (así no hay
  bucles de largo cero); al terminar la fila 255 se vuelve al inicio del bloque; si se vacía en vivo la celda que está
  sonando, al terminar la frase el cabezal sigue a la fila siguiente si tiene cadena, y si no, vuelve al inicio del
  bloque de arriba (o se queda en esa fila, en silencio, hasta que se le ponga una cadena).
- **Seguir reproducción**: el mockup de Proyecto muestra "NO"; se deja **activado por defecto** porque Start en la
  Grilla ahora toca la canción y, sin seguir, el cabezal desaparece al pasar a la sección siguiente. La sección salta
  solo cuando el cabezal de la pista del cursor cambia de sección (no en cada paso), para que L2/R2 sigan sirviendo
  mientras suena. Sigue en la Grilla y la Frase (spec-ui); en la Canción y la Cadena el cursor no se mueve solo.
- **X en la Canción y la Cadena**: la spec pide escuchar la frase completa una vez; por ahora suena la primera nota
  de la frase (con la transposición de la entrada). La vista previa de una frase completa necesita un modo "una vez"
  en el motor.
- **A en una celda vacía de la Canción sin "última cadena"** (al abrir el proyecto) pone una cadena nueva; el doble
  toque inmediato se queda con esa en lugar de crear otra (no deja cadenas huérfanas). Lo mismo con FRA y FRASE.
- **X en la Grilla y la Frase** suena con la transposición de la entrada de cadena (lo que se oye al reproducir).
- **Mute (H3)**: se aplica en el secuenciador (la pista no dispara y suelta lo sostenido), no como ganancia 0: las colas
  de lo que ya sonaba terminan solas y la audición con X sigue sonando en una pista en mute (para editarla). Con la
  Mezcla (H9) puede pasar a ser ganancia.
- **Cambiar de sección con L2/R2 mientras suena** no mueve la reproducción; con "seguir reproducción", la vista vuelve
  a la sección del cabezal cuando este cambia de sección.
- **Motor (H2)**: la spec de producto pedía solo un bombo; se hizo la base completa (bolsa de voces, 5 percusiones y sinte
  mínimo). Del sinte faltan sub, ruido, ancho de pulso y LFO (H8). Salida mono hasta la Mezcla (H9). El modelo guarda el
  instrumento de cada pista (`Track::instrument`; `kind()` y las voces salen de ahí).
- **Ranuras de fundido**: 16 en lugar de las 4 del ADR 0007 ([ADR 0013](../../docs/decisions/0013-ranuras-de-fundido.md), aceptado):
  sin cortes en seco cuando varias pistas polifónicas cambian de acorde a la vez.

## Pendientes para la consola (device-ops)

- H6: guardar, salir, volver a abrir la app (debe abrir el último proyecto) y apagar la consola de golpe justo después
  de guardar, para ver que `proyecto.ron` o el `.bak` sobreviven. Ver cuánto tarda GUARDAR en la SD (la interfaz se
  congela ese rato) y que la carpeta de datos quede en `/home/alarm/.local/share/groovebox/` con el launcher.

- `groovebox bench` con el binario de `just build-arm` (y la frecuencia de la CPU durante la medición), y la CPU real que
  muestra la app al tocar. Escenarios del H3 y H5: "secuenciador: proyecto demo", "secuenciador: canción demo (modo
  canción)" y "secuenciador solo".
- Recorrer la canción demo con `GROOVEBOX_DEMO=1` (Start en la Canción) y la legibilidad de la Canción con la fuente
  de 12×24 (los nombres de pista quedan en 3 letras).
- Escuchar el beat (`GROOVEBOX_DEMO=1`, Start) y confirmar que el cabezal acompaña al audio (la latencia de salida no se
  compensa en el dibujo).

- Comparar la legibilidad de 8×16 y 12×24 (Proyecto → fuente) y la ergonomía de A + cruz, A + B y Select + cruz.
- Ver en la prueba de controles qué botones físicos llegan como cada botón (mapeo de dArkOS, **sin verificar**),
  si L2/R2 llegan como botones o como ejes, y si el SO intercepta Select + Start.
- Confirmar que SDL abre KMSDRM en pantalla completa y que el área lógica coincide con el panel.
- Medir el período real que acepta ALSA (`GROOVEBOX_FRAMES=128|256|512`) y si hay errores del stream.
