# Groovebox — DSP del motor (H2 y H9)

Última revisión: 2026-11-21 (H9: mezcla, efectos, limitador y sticks) · Estado: implementado en `projects/groovebox/src/engine/` · Volver al [índice](README.md)

Qué hay en el motor del hito H2, por qué está hecho así, cuánto cuesta y qué falta medir en la consola.
La arquitectura general está en la [spec del motor](spec-motor.md); las decisiones, en los ADR
[0005](../../decisions/0005-configuracion-de-audio.md) (48 kHz, `f32`, 256 frames, subbloques de 32),
[0007](../../decisions/0007-polifonia-en-el-mvp.md) (bolsa de voces) y [0013](../../decisions/0013-ranuras-de-fundido.md) (propuesta: ranuras de fundido).

## Estructura

```
interfaz ──Command (rtrb, 1024)──► Processor (callback de cpal)                     audio.rs
                                     ├─ flush-to-zero (primer callback)
                                     ├─ vacía la cola → Engine::handle
                                     ├─ Engine::render en subbloques de ≤ 32 frames         engine/mod.rs
                                     │    VoicePool::render → un buffer por pista            engine/pool.rs
                                     │      DrumVoice / SynthVoice (+ stick izq. en el SVF)  engine/drum.rs, synth.rs
                                     │    filtro de bus (stick izq., percusión)              engine/fx.rs
                                     │    × volumen y paneo → suma L/R; envíos → delay, reverb
                                     │    delay estéreo + reverb FDN → retornos a la suma
                                     │    × ganancia master → limitador → soft clip
                                     ├─ convierte al formato del dispositivo (f32/i16/…)
                                     └─ mide el tiempo → LoadMeter → Event::Load (rtrb, 64) ──► interfaz
```

- `Command`: `SetInstrument { track, instrument }`, `SetTempo(bpm)`, `Audition { track, notes, len, velocity, gate_ticks }`.
  Todo es `Copy`: el callback nunca libera memoria. La interfaz manda solo los cambios del proyecto (`EngineMirror` en `main.rs`).
- `Event::Load(LoadReport)`: CPU promedio y pico (en % del presupuesto del buffer), voces máximas y período real, cada 0,5 s de audio.
  La interfaz lo muestra en la barra de título (`CPU n%`) y en Proyecto, y lo escribe en el log cada 10 s.
- `Engine::note_on(track, notes, velocity, gate_samples)` es la entrada que usa el secuenciador (H3); `gate_samples = None` es LARGO INF (la próxima nota de la pista la suelta).
- Los parámetros de los instrumentos son `u8` 0–127 (`instrument.rs`, mapeos de la spec) y se convierten a coeficientes al
  recibir `SetInstrument` (`DrumSetup`, `SynthSetup`), no por nota ni por muestra. Las voces que ya suenan conservan su copia.
- Salida estéreo desde el H9 (en un dispositivo mono, el promedio de L y R; con más de 2 canales, silencio en los demás).
- Nada en el callback asigna, bloquea ni hace I/O: lo verifican tests con un asignador que cuenta asignaciones por hilo
  (`alloc_check.rs`), tanto en `Engine` como en el `Processor` completo con las colas.

## Bolsa de voces

Implementa las reglas del ADR 0007 tal cual: 16 voces + 16 ranuras de fundido de 5 ms (ADR 0013; antes 4), arrays fijos (`[Option<Voice>; 16]`).

| Situación | Qué hace |
|---|---|
| La pista ya usa sus `voces` | Roba la más vieja de la pista |
| Hay voz libre | La usa |
| Bolsa llena | Roba la más vieja en relajación; si no hay, la más vieja de la pista que más voces usa; si no, la más vieja |
| Acorde | Nunca roba una nota del mismo disparo; si la pista tiene menos voces que notas, descarta las más agudas |
| Percusión | Toca la raíz e ignora el largo. Sus voces cuentan como "en relajación" (son one-shot) |
| Sinte con 1 voz | **Legato**: no roba; la misma voz cambia de nota, la fase sigue y la envolvente sube desde su nivel actual |
| Voz robada | Pasa a una ranura de fundido con rampa lineal de 5 ms (240 muestras). Sin ranura libre, se pisa la de menor ganancia (corte en seco, se cuenta en `hard_cuts`) |
| Voz robada que todavía no sonó, o ya inaudible (< −80 dB) | Se descarta sin ocupar ranura: no hay discontinuidad que tapar |

Los note-off se aplican en la muestra exacta (el bloque se parte en dos). Las audiciones con largo INF duran 16 pasos.

**Hallazgo:** en la prueba de estrés de la spec (8 pistas, acordes de 4 notas con largo INF en cada paso) cada paso pide 32 voces:
las 16 que sonaban se roban a la vez y solo 4 tienen ranura de fundido. Medido en 32 pasos: **880 cortes en seco**, **372** después de
descartar las voces que no sonaron, **0** con 16 ranuras. Con uso normal (un acorde por paso, percusión mono) no hay cortes.
Propuesta y números en el [ADR 0013](../../decisions/0013-ranuras-de-fundido.md); hasta que el usuario decida, quedan 4.

## Instrumentos

### Percusión sintetizada

Todas las envolventes son exponenciales: `amp *= k` con `k = exp(ln(0,001) / (T·fs))` (llega a −60 dB en `T`), y la voz termina a −80 dB.
La nota mueve la afinación respecto de C-4 (60), como en una pista de percusión de tracker.

| Modelo | Síntesis | Parámetros (defecto) |
|---|---|---|
| BOMBO | Seno con barrido exponencial `f = f_fin·(1 + (r−1)·e^(−t/τ))` + click de ruido de 4 ms | 48 Hz, barrido ×3,5 en 40 ms, caída 450 ms, golpe 50 |
| CAJA | Seno de 185 Hz que cae media octava en 15 ms (caída = mitad de la del ruido) + ruido por SVF (HP + ½·BP) a 10× la afinación (~1,85 kHz) + click | caída 220 ms, color 55 % de ruido |
| HAT-CER / HAT-ABI | Ruido por SVF a 7,5 kHz, salida HP + ½·BP; color = resonancia (k de 1,6 a 0,4) | caída 60 ms / 450 ms |
| PALMAS | Ruido por pasa-banda SVF a 1,2 kHz (normalizado: k·BP); 4 picos (3 ráfagas que caen 30 dB antes de la siguiente + la cola) | separación 10 ms, cola 250 ms |

En la caja y los hats la envolvente se aplica **después** del filtro: el estado del SVF siempre ve ruido a nivel constante y no decae
hacia subnormales. Los hats son solo ruido (sin las 6 cuadradas inarmónicas del 808) para no meter aliasing; se puede sumar en v1 con osciladores PolyBLEP.

### Voz sustractiva (BAJO y SINTE)

Oscilador → SVF pasa-bajos → ADSR de amplitud. El resto de la spec (sub, ruido, ancho de pulso, LFO, glide) llega en H8.

- **Oscilador PolyBLEP** (Välimäki y Huovilainen): sierra `2p − 1 − blep(p, dt)`, cuadrada con dos residuos, seno polinómico.
  El residuo polinómico de 2 muestras suaviza la discontinuidad. Medido (test `saw_aliasing_is_low`): con una sierra de 3520 Hz, los armónicos 13 y 14
  se pliegan a 2240 y 1280 Hz con **−70,6 y −74,1 dB** respecto del fundamental, contra −22,2 y −22,8 dB de la sierra ingenua.
- **SVF TPT** (Zavalishin, cap. 4; Simper): `g = tan(π·fc/fs)`, `a1 = 1/(1 + g(g + k))`, `a2 = g·a1`, `a3 = g·a2`. Estable con modulación;
  el corte se limita a 0,45·fs. Se recalcula **una vez por llamada a `render`** (≤ 32 muestras): `exp2` + `tan` amortizados en ~2–4 ciclos por muestra.
- **Corte**: `fc = corte · 2^(octavas_env · env_filtro) · (f_nota / 261,63 Hz)^0,5` (seguimiento de teclado fijo del 50 %, parámetro en H8).
  La envolvente de filtro es una caída exponencial redisparada en cada nota.
- **ADSR exponencial** (one-pole hacia un objetivo, Pirkle): `env += (objetivo − env)·c`, `c = 1 − e^(−1/(τ·fs))`.
  En el ataque el objetivo es 1,2 y se corta en 1, así que `τ = T / ln(1,2/0,2) = T / ln 6` hace que llegue a 1 en `T`.
  En caída y relajación `τ = T / ln 1000` (−60 dB en `T`). Redisparar o soltar parte del nivel actual: nunca salta.
  Cerca del sostenido, la diferencia se fuerza a 0 por debajo de 1e-6 (si no, con sostenido 0 tendería a subnormales).
- **Ganancia por velocidad** suavizada (2 ms) para que el legato con otra velocidad no haga click.
- Presets: BAJO (sierra, ataque 2 ms, corte ~250 Hz con +3 octavas de envolvente en 180 ms, mono/legato) y SINTE (sierra, ataque 15 ms,
  corte ~1,5 kHz, relajación 400 ms, 4 voces).

## Mezcla, efectos y master (H9)

Código en `src/engine/fx.rs` (DSP), `src/engine/mod.rs` (buses), `src/mix.rs` (ajustes y leyes) y `src/mixer.rs`
(pantalla). Decisión del master en el [ADR 0018](../../decisions/0018-limitador-sin-anticipacion-antes-del-soft-clip.md) (propuesta).

```
pista: voces → [filtro de bus del stick, solo percusión/sample] → ×L, ×R ──► suma L/R
                                                               ├─ ×L·envío, ×R·envío ──► bus delay (estéreo)
                                                               └─ ×volumen·envío ──────► bus reverb (mono)
bus delay → Delay → ×retorno ─┐
bus reverb → Reverb → ×retorno┴► suma L/R → ×ganancia master → limitador (si está activo) → soft clip → salida
```

- **Leyes**: volumen, envíos y retornos en 0–127 con **ley cuadrática, 100 = 0 dB** (`(v/100)²`: 127 = +4,2 dB,
  50 = −12 dB, 0 = silencio). Paneo −64..63 con **ley seno/coseno de potencia constante normalizada a 0 dB en el
  centro** (`L = √2·cos θ`, `R = √2·sin θ`, `θ = (p+1)·π/4`; L² + R² = 2 siempre): una pista centrada suena igual que
  antes del H9 y en un extremo un canal queda a +3 dB (el limitador cubre ese caso). La ganancia de la pista es
  `volumen del instrumento × volumen de la mezcla`, y con 100 y centro las dos multiplicaciones dan exactamente el
  valor anterior: **el audio sin mezclar es el mismo bit a bit**.
- **Envíos post-fader**: al delay, después del paneo (estéreo: el delay respeta la imagen); a la reverb, después del
  volumen y antes del paneo (la reverb es mono a la entrada).
- **Suavizado por bloque** (`Ramp`): un one-pole de 10 ms evaluado al final de cada subbloque (≤ 32 muestras) y una
  rampa lineal adentro; quieto, la ganancia se aplica constante (1 multiplicación y 1 suma por muestra y bus). Reemplaza
  al one-pole por muestra del H2. Sin señal en la pista, salta al objetivo. Volumen, paneo, envíos, retornos y
  ganancia master no hacen clicks (tests `volume_and_pan_changes_are_smoothed`, `ramp_reaches_its_target_without_steps`).
- **Mute y solo** siguen en el secuenciador (H3): la pista no dispara y lo sostenido se suelta; las colas de las voces y
  de los efectos terminan solas. No se pasó a ganancia: la audición con X en una pista en mute sigue sonando (para
  editarla) y no hizo falta cortar colas.

### Delay estéreo (envío)

Dos buffers circulares de **2^18 muestras** (5,46 s, 1 MB cada uno; la spec pedía 2^17: con 2^18 entra un compás
desde 44 BPM, más lento se recorta), lectura con **interpolación lineal**, pasa-bajos de un polo en la
realimentación y cruce L/R. Tiempo sincronizado al tempo (1/16, 1/8, 3/16, 1/4, 3/8, 1/2, 1 compás); al cambiar el
tempo o el tiempo, la longitud se **desliza** (one-pole de 50 ms sobre las muestras de retardo, como una cinta), sin
saltos de lectura. Realimentación 0–95 % (suavizada), tono = corte del pasa-bajos de 500 Hz a 16 kHz.

**Ancho** `w`: la entrada izquierda recibe `(1−w)·L + w·(L+R)/2`, la derecha `(1−w)·R`, y la realimentación de cada
lado es `(1−w)·propio + w·opuesto`. Con `w = 0` son ecos estéreo en su lugar; con `w = 1`, ping-pong (primer eco a
la izquierda, el siguiente a la derecha...). La ganancia del lazo es ≤ 0,95 (|1−w| + |w| = 1, pasa-bajos ≤ 1): estable.

### Reverb liviana (envío)

**FDN de 4 líneas** (Jot y Chaigne; Smith, *PASP*) con **matriz de Hadamard** 4×4 normalizada (ortogonal: 8 sumas y
4 multiplicaciones), un **pasa-bajos de un polo** por línea (amortiguación) y una ganancia por línea
`g = 10^(−3·L/(RT60·fs))` para que cada vuelta cueste lo mismo en dB por segundo. A la entrada, **predelay** de 0–100 ms
(interpolado, se desliza) y **2 allpass de Schroeder** (5 y 8 ms, g = 0,6) para difundir los transitorios. Largos
a 48 kHz: 1553, 1871, 2243 y 2647 muestras (32–55 ms, primos), escalados con la frecuencia de muestreo. Entrada mono,
salida L = líneas 0 + 2 y R = 1 + 3 (decorrelacionadas: correlación < 0,5 en el test).

"Tamaño" es el **RT60** (0,3 a 8 s, exponencial); "amort." el corte del pasa-bajos (18 kHz a 1,5 kHz). Al cambiar el
tamaño, las ganancias recorren el 20 % de la distancia por bloque (sin escalones en la cola). Elegido sobre un
Schroeder chico (4 combs + 2 allpass) porque cuesta lo mismo y la matriz mezcla las líneas: menos "metálico". No hay
modulación de las líneas (costaría interpolar 4 lecturas). Memoria: ~80 KB.

El test `reverb_tail_follows_rt60` mide la pendiente de la cola entre 0,2 y 0,7 s y la compara con el RT60 del
ajuste (±25 %).

### Efectos en reposo

Cada efecto mide el pico de su entrada y su salida por bloque. Después de **un buffer entero** por debajo de −100 dBFS
(5,5 s el delay; ~11 000 muestras la reverb) se apaga y el motor deja de llamarlo; con un envío que vuelve a mandar
señal, se prende. Esperar el buffer entero garantiza que lo que queda en las líneas está por debajo de −100 dB (y que no
reaparecen ecos viejos si después se alarga el tiempo). Sin voces y con los dos efectos apagados, el motor devuelve
silencio sin procesar nada, como antes. Tampoco llegan a subnormales: en esa ventana la cola baja de 1e-5 a ~1e-7.

### Master: ganancia, limitador y soft clip

Ganancia master −24..+12 dB (suavizada). **Limitador de picos sin anticipación** (ADR 0018): envolvente de pico del
máximo de |L| y |R| con **ataque instantáneo**, **retención de 25 ms** (más que un período de 40 Hz) y relajación
exponencial de 120 ms; ganancia `1 / envolvente` cuando pasa de 1. Por construcción **ningún pico pasa de 0 dBFS** a la
entrada del soft clip, y por debajo del umbral no toca la señal (bit a bit). La retención evita que la relajación module
la ganancia dentro de cada ciclo de un bajo (test: un seno de 50 Hz a +6 dB queda con el 2.º armónico < 1 %). El
**soft clip** de Padé del H2 queda siempre al final: con el limitador la salida no pasa de `soft_clip(1) = 0,78`
(−2,2 dBFS); sin él (limitador NO), la suma satura como antes. La interfaz muestra la reducción (dB) en la fila MASTER.

El ataque instantáneo cambia la ganancia de golpe en el pico que la dispara; como pasa justo en un máximo de la señal,
el salto de la salida es chico. Un limitador con anticipación (1–2 ms de retardo) lo evitaría a cambio de latencia y de
correr los WAV: queda para v1 como dice la spec.

### Sticks (performance)

La interfaz manda `Perform { track, offsets: [corte, resonancia, envío a delay, envío a reverb] }` en −1..1 (solo lo que
cambia, cuantizado a 1/256, y 0 a la pista anterior al cambiar de pista). El motor los suaviza con un one-pole de 30 ms
(llega o vuelve al valor guardado en ~100 ms, como pide spec-ui). Un stick al extremo suma ±64 al parámetro 0–127:

- **Sinte (BAJO, SINTE)**: el corte del SVF de **cada voz** (±5 octavas) y la resonancia (±0,5 en 0–1), sobre el valor
  guardado y la envolvente de filtro; se recalcula con el resto del corte una vez por subbloque.
- **Percusión y sample** (no tienen filtro propio): un **filtro de bus** SVF por pista, pasa-bajos de 20 kHz a 110 Hz a
  la izquierda y pasa-altos de 20 Hz a 5 kHz a la derecha (filtro de DJ), con la resonancia del eje vertical. Cerca del
  centro (|x| < 0,1) se mezcla con la señal seca: entra y sale sin clicks y en el centro no cuesta nada.
- **Envíos**: el stick derecho suma al envío a delay (X) y a reverb (Y), aunque el envío guardado sea 0 ("tirar" una
  pista al efecto).

## Denormales

Dos capas:

1. **Diseño**: todas las envolventes terminan en −80 dB (y la voz se libera); los decaimientos auxiliares (barrido, click, envolvente de filtro)
   se fuerzan a 0 por debajo de 1e-6 una vez por bloque; los filtros con ruido reciben la envolvente después. Un test renderiza todos los
   instrumentos **sin** flush-to-zero y verifica que la salida no tenga subnormales.
2. **Flush-to-zero** en el hilo de audio (primer callback) y en los renders offline, con `asm!`:
   aarch64 → bit **FZ (24) del FPCR** (`mrs/msr fpcr`); x86_64 → **FTZ (15) y DAZ (6) del MXCSR** (`stmxcsr/ldmxcsr`). Es por hilo.
   Un test comprueba que `MIN_POSITIVE · 0,25` da 0 después de activarlo.
   Advertencia: cambiar el entorno de punto flotante no está modelado por LLVM. Es la práctica habitual en audio (lo hace el crate
   `no_denormals`) y este código no depende de subnormales, pero por eso la capa 1 es la que garantiza el resultado.

## Otros detalles

- **Seno**: `sin(2πp)` por reflexión a [−¼, ¼] y Taylor impar hasta x⁹ (~12 operaciones, error < 4e-6), sin llamar a libm.
- **Ruido**: xorshift32 (Marsaglia) por voz, con semilla derivada del número de disparo: el render es **determinístico**.
  Los WAV renderizados en x86_64 nativo y en aarch64 (QEMU) son **idénticos bit a bit**.

## Costo

Operaciones por muestra estimadas a mano (sin contar la conversión de formato):

| Bloque | Operaciones por muestra |
|---|---|
| BOMBO (seno con barrido + click) | ~25 |
| CAJA (seno + ruido + SVF + click) | ~40 |
| HATS (ruido + SVF) | ~20 |
| PALMAS (ruido + SVF + ráfagas) | ~22 |
| Sinte (PolyBLEP + SVF + ADSR + ganancia) | ~45, más `tan` + `exp2` cada ≤ 32 muestras |
| Voz en fundido | la de su instrumento + 3 |
| Pista activa: pico, L y R (sin envíos) | ~5 |
| Pista activa con los dos envíos | ~9 |
| Filtro de bus del stick (percusión, mientras actúa) | ~14, más `tan` por subbloque |
| Delay estéreo (2 lecturas interpoladas por canal, pasa-bajos, cruce) | ~30 por frame |
| Reverb FDN (predelay, 2 allpass, 4 líneas, Hadamard, 4 pasa-bajos) | ~55 por frame |
| Retornos (2 efectos × L/R) | ~4 por frame |
| Master: ganancia + limitador + soft clip + pico | ~22 por frame (una división por muestra solo mientras limita) |

Peor caso del H9 estimado para el A35 (8 pistas con los dos envíos, los dos efectos y el limitador): 8 × 9 + 30 + 55 +
4 + 22 ≈ **190 operaciones por frame**, unos 250–400 ciclos: ~1–1,5 % de un núcleo a 1,3 GHz (27 083 ciclos por frame).
Coincide con lo que reservaba la spec (canales 160 + delay 40 + reverb 200 + master 40). **Estimación, no medición.**

Medido con `groovebox bench` (bloques de 256 frames; 10 s de audio por escenario en la Mac y 5 s en QEMU; % de un núcleo de **esa** máquina):

| Escenario | Mac (i7-1068NG7, nativo, release) | QEMU user (aarch64 emulado sobre la Mac) |
|---|---|---|
| Bolsa llena: 8 sintes, acordes de 4 notas INF en cada paso | prom. 0,70 %, p99 1,4 %, máx. 2,9 %; 9,1 ns por voz y muestra | prom. 13,3 %, p99 17,4 %, máx. 76 %; 173 ns |
| Demo (beat + bajo + acordes) | prom. 0,26 %, p99 0,38 %; 8,0 ns | prom. 6,7 %, p99 13,5 %; 204 ns |
| Percusión (5 pistas en cada paso) | prom. 0,18 %, p99 0,35 %; 8,2 ns | prom. 4,2 %, p99 8,3 %; 191 ns |
| Bolsa llena con 16 ranuras de fundido (ADR 0013) | prom. 0,86 %, p99 1,8 %, máx. 3,4 %; 11,2 ns | — |

H9 (2026-11-21, `groovebox bench 5`; **la Mac estaba en `powersave`**, así que todo salió ~3 veces más lento que la
tabla de arriba: se comparan las columnas entre sí, no con el H2):

| Escenario | Mac antes del H9 | Mac con el H9 | QEMU con el H9 (orientativo) |
|---|---|---|---|
| Bolsa llena (sin mezclar) | prom. 2,22 %, p99 4,4 % | prom. 2,08 %, p99 4,6 % | prom. 58 %, p99 105 % |
| Bolsa llena + FX (paneo y 2 envíos en 8 pistas, master +6 dB limitando) | — | prom. 2,71 %, p99 5,3 % | prom. 77 %, p99 124 % |
| Demo mezclada (`h9-mezcla`, modo canción) | — | prom. 1,45 %, p99 2,1 % | prom. 45 %, p99 63 % |
| Proyecto demo (sin envíos) | prom. 0,81 % | prom. 0,88 % | prom. 25 % |
| Motor vacío | 0,37 µs/bloque | 1,14 µs/bloque (cuenta voces por pista) | 16 µs/bloque |
| Delay solo (ruido a la entrada) | — | 39 ns/frame (0,19 %) | 1766 ns/frame |
| Reverb sola | — | 61 ns/frame (0,29 %) | 1689 ns/frame |
| Limitador limitando | — | 14 ns/frame (0,07 %) | 278 ns/frame |

Mezcla y efectos suman ~30 % al peor caso de la bolsa (en la Mac y en QEMU), sin cambiar el orden de magnitud. En
QEMU el delay cuesta lo mismo que la reverb (las conversiones float → índice se emulan caras); en el A35 se espera
bastante menos. Si en la consola el delay resultara caro, la lectura se puede hacer sin interpolar cuando el tiempo
está quieto (siempre es un número entero de muestras).

**Ninguno de estos números es de la consola.** La Mac es un núcleo fuera de orden a hasta 4,1 GHz; QEMU user traduce instrucciones y su costo
no se parece al de un Cortex-A35 (los máximos son ruido del emulador y del host). La spec estima 90–140 ciclos por voz sustractiva en el A35;
el número real lo mide `device-ops` con `groovebox bench` en la consola.

## Pruebas

`cargo test` (en `engine/tests.rs`, `engine/dsp.rs`, `engine/load.rs`, `audio.rs`, `instrument.rs`):

- Cada percusión del kit suena (pico 0,15–0,95), termina en < 3 s y su salida es finita, sin subnormales y en ±1; los extremos de todos los parámetros no rompen nada.
- Sinte: frecuencia de A4 por cruces por cero; release hasta silencio exacto; el gate sigue al tempo.
- **Aliasing** de la sierra: alias plegados por debajo de −40 dB (medidos −70 dB), con control del método sobre una sierra ingenua (Goertzel).
- **Envolventes sin saltos**: con un seno de 220 Hz, el salto máximo entre muestras no supera `A·2π·f/fs` (+5 %).
- **Robo sin clicks**: la tercera nota de una pista de 2 voces roba a la primera en su pico; el salto máximo queda por debajo de
  `(3·A·2π·f/fs + A/240)·1,05`, menos del 10 % de lo que daría un corte en seco.
- Reglas de robo: límite por pista, relajación primero, pista con más voces, acorde que no se roba a sí mismo, notas agudas descartadas, legato mono, percusión mono con fundido.
- Estrés con la bolsa llena: nunca más de 20 voces, salida limpia.
- Cambio de volumen suavizado; determinismo bit a bit; canales idénticos y bloques parciales.
- **Sin asignaciones** en `Engine::handle` + `render` y en el `Processor` completo (colas, conversión a `f32` e `i16`, medición).
- SVF (DC pasa por el LP y no por el HP), seno polinómico, xorshift, soft clip, flush-to-zero, medidor de carga.
- **H9**: leyes de nivel y paneo (potencia constante, centro en 0 dB); delay con el eco en la muestra exacta de 1/16 a
  120 BPM, segundo eco atenuado por la realimentación y ping-pong; delay que se apaga tras un buffer en silencio;
  RT60 de la reverb medido (±25 %) y canales decorrelacionados; limitador transparente debajo del umbral, sin picos
  arriba de 0 dBFS y sin distorsionar un seno de 50 Hz; filtro de bus sin saltos; en el motor: centro idéntico en L y
  R, extremo izquierdo con R en silencio, cambios de volumen y paneo suavizados, eco del envío a delay y cola que
  termina en silencio exacto, cola estéreo de la reverb, limitador contra soft clip con +12 dB, stick que abre el filtro
  del bajo y vuelve al valor guardado, stick que cierra un hat, medidores, y **sin asignaciones** con mezcla, efectos,
  sticks y cambios de ajustes.
- WAV: `h9-mezcla.wav` es estéreo, no pasa de `soft_clip(1)`, sale igual con buffers de 64 y 256 frames y sus colas
  terminan en silencio.

Para escuchar: `groovebox render <carpeta>` escribe `h9-mezcla.wav` (la canción demo mezclada; ver el README del
groovebox), `h2-kit.wav` (cada sonido y acordes), `h3-beat.wav` (el proyecto demo tocado por el secuenciador, 8 compases; reemplaza a `h2-demo.wav`) y `h2-robo.wav` (estrés de robo).

## Pendiente de medir en la consola (`device-ops`)

- `groovebox bench` con el binario de `just build-arm`, registrando la frecuencia de la CPU (`/sys/devices/system/cpu/cpu0/cpufreq/`).
- CPU real en uso (barra de título / Proyecto / log cada 10 s) con audiciones rápidas, y si hay xruns con 128/256/512 frames.
- Que el FPCR.FZ efectivamente evite picos (comparar `bench` con y sin la llamada, si hiciera falta).
- Si conviene `SCHED_FIFO` para el hilo de audio (todavía no se pide).
- Escuchar en los parlantes y auriculares de la consola: niveles del kit, brillo de los hats y si el soft clip se nota.
- H9: `groovebox bench` con los escenarios "bolsa llena + FX", "demo mezclada" y los tres efectos solos (anotar la
  frecuencia de la CPU). Escuchar `h9-mezcla.wav` y la Mezcla con auriculares: paneo, ping-pong, cola de la reverb,
  bombeo del limitador con la demo a +4 dB y si el ataque instantáneo se oye; con el parlante (solo L, o mono por
  `rgb20s-audio`), que lo paneado a la derecha no desaparezca. Mover los sticks de verdad (zona muerta, ruido del eje,
  cuántos `Perform` por segundo llegan) y probar el filtro de bus en los hats.

## Fuentes

- Vadim Zavalishin, *The Art of VA Filter Design*, rev. 2.1.2, Native Instruments, 2020 — cap. 3 (integradores TPT) y cap. 4 (SVF).
- Andrew Simper, *Linear Trapezoidal Integrated State Variable Filter With Low Noise Optimisation*, Cytomic, 2013 (forma `a1, a2, a3`).
- Vesa Välimäki y Antti Huovilainen, "Antialiasing Oscillators in Subtractive Synthesis", *IEEE Signal Processing Magazine* 24(2), 2007 (PolyBLEP).
- Will Pirkle, *Designing Software Synthesizer Plug-Ins in C++*, Focal Press, 2015 — envolventes analógicas exponenciales y robo de voces.
- Julius O. Smith III, *Introduction to Digital Filters* y *Physical Audio Signal Processing* (CCRMA, en línea) — one-pole y suavizado.
- George Marsaglia, "Xorshift RNGs", *Journal of Statistical Software* 8(14), 2003.
- Arm, *Arm Architecture Reference Manual for A-profile*, registro FPCR (bit FZ, 24).
- Jean-Marc Jot y Antoine Chaigne, "Digital Delay Networks for Designing Artificial Reverberators", 90.ª convención de la AES, 1991 (FDN, ganancias por línea según el RT60).
- Julius O. Smith III, *Physical Audio Signal Processing* (CCRMA, en línea) — "Feedback Delay Networks" (matriz de Hadamard) y "Allpass Filters" (Schroeder).
- Manfred R. Schroeder, "Natural Sounding Artificial Reverberation", *JAES* 10(3), 1962 (allpass de difusión).
- Udo Zölzer (ed.), *DAFX: Digital Audio Effects*, 2.ª ed., Wiley, 2011 — cap. 2 (delays, ping-pong) y cap. 4 (limitador: detector de pico con ataque y relajación).
- Dimitrios Giannoulis, Michael Massberg y Joshua D. Reiss, "Digital Dynamic Range Compressor Design — A Tutorial and Analysis", *JAES* 60(6), 2012 (detectores de pico, ataque instantáneo).
- Ley de paneo seno/coseno de potencia constante: práctica estándar de mezcla (p. ej. Pirkle, *Designing Audio Effect Plugins in C++*, 2.ª ed., 2019) **(sin verificar la página)**.
- Intel, *64 and IA-32 Architectures Software Developer's Manual*, vol. 1, §10.2.3 (MXCSR: FTZ bit 15, DAZ bit 6).
- Aproximantes de Padé de `tanh` para saturación: práctica habitual en DSP de audio (p. ej. musicdsp.org, "tanh approximation") **(sin verificar la cita exacta)**.
- Skill del proyecto `dsp-recipes` (recetas de percusión, PolyBLEP, SVF, envolventes).
