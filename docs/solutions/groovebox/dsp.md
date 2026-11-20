# Groovebox — DSP del motor (H2)

Última revisión: 2026-11-20 · Estado: implementado en `projects/groovebox/src/engine/` · Volver al [índice](README.md)

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
                                     │      DrumVoice / SynthVoice                           engine/drum.rs, synth.rs
                                     │    × volumen de pista (suavizado) → suma mono
                                     │    × ganancia master (suavizada) → soft clip
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
- Salida mono duplicada en todos los canales: el paneo llega con la Mezcla (H9).
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

## Mezcla, master y suavizado

- Volumen de pista (= volumen del instrumento por ahora): one-pole de 10 ms por muestra; cuando ya llegó al objetivo, se aplica constante.
  Sin voces en la pista, salta al objetivo (no hay señal que proteger).
- Master: ganancia suavizada (10 ms) y **soft clip** con el aproximante de Padé de `tanh`, `x(27 + x²)/(27 + 9x²)`, con la entrada
  limitada a ±3: ahí vale ±1 con derivada 0, así que la curva es continua y derivable (no agrega clicks). Error frente a `tanh` < 1e-4 para |x| < 0,1;
  comprime ~0,6 dB a −6 dBFS: es coloración, no un limitador transparente (el limitador con lookahead es de v1).

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
| Pista activa (volumen suavizado + suma) | ~4 |
| Master (ganancia + soft clip + pico) | ~10 |

Medido con `groovebox bench` (bloques de 256 frames; 10 s de audio por escenario en la Mac y 5 s en QEMU; % de un núcleo de **esa** máquina):

| Escenario | Mac (i7-1068NG7, nativo, release) | QEMU user (aarch64 emulado sobre la Mac) |
|---|---|---|
| Bolsa llena: 8 sintes, acordes de 4 notas INF en cada paso | prom. 0,70 %, p99 1,4 %, máx. 2,9 %; 9,1 ns por voz y muestra | prom. 13,3 %, p99 17,4 %, máx. 76 %; 173 ns |
| Demo (beat + bajo + acordes) | prom. 0,26 %, p99 0,38 %; 8,0 ns | prom. 6,7 %, p99 13,5 %; 204 ns |
| Percusión (5 pistas en cada paso) | prom. 0,18 %, p99 0,35 %; 8,2 ns | prom. 4,2 %, p99 8,3 %; 191 ns |
| Bolsa llena con 16 ranuras de fundido (ADR 0013) | prom. 0,86 %, p99 1,8 %, máx. 3,4 %; 11,2 ns | — |

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

Para escuchar: `groovebox render <carpeta>` escribe `h2-kit.wav` (cada sonido y acordes), `h3-beat.wav` (el proyecto demo tocado por el secuenciador, 8 compases; reemplaza a `h2-demo.wav`) y `h2-robo.wav` (estrés de robo).

## Pendiente de medir en la consola (`device-ops`)

- `groovebox bench` con el binario de `just build-arm`, registrando la frecuencia de la CPU (`/sys/devices/system/cpu/cpu0/cpufreq/`).
- CPU real en uso (barra de título / Proyecto / log cada 10 s) con audiciones rápidas, y si hay xruns con 128/256/512 frames.
- Que el FPCR.FZ efectivamente evite picos (comparar `bench` con y sin la llamada, si hiciera falta).
- Si conviene `SCHED_FIFO` para el hilo de audio (todavía no se pide).
- Escuchar en los parlantes y auriculares de la consola: niveles del kit, brillo de los hats y si el soft clip se nota.

## Fuentes

- Vadim Zavalishin, *The Art of VA Filter Design*, rev. 2.1.2, Native Instruments, 2020 — cap. 3 (integradores TPT) y cap. 4 (SVF).
- Andrew Simper, *Linear Trapezoidal Integrated State Variable Filter With Low Noise Optimisation*, Cytomic, 2013 (forma `a1, a2, a3`).
- Vesa Välimäki y Antti Huovilainen, "Antialiasing Oscillators in Subtractive Synthesis", *IEEE Signal Processing Magazine* 24(2), 2007 (PolyBLEP).
- Will Pirkle, *Designing Software Synthesizer Plug-Ins in C++*, Focal Press, 2015 — envolventes analógicas exponenciales y robo de voces.
- Julius O. Smith III, *Introduction to Digital Filters* y *Physical Audio Signal Processing* (CCRMA, en línea) — one-pole y suavizado.
- George Marsaglia, "Xorshift RNGs", *Journal of Statistical Software* 8(14), 2003.
- Arm, *Arm Architecture Reference Manual for A-profile*, registro FPCR (bit FZ, 24).
- Intel, *64 and IA-32 Architectures Software Developer's Manual*, vol. 1, §10.2.3 (MXCSR: FTZ bit 15, DAZ bit 6).
- Aproximantes de Padé de `tanh` para saturación: práctica habitual en DSP de audio (p. ej. musicdsp.org, "tanh approximation") **(sin verificar la cita exacta)**.
- Skill del proyecto `dsp-recipes` (recetas de percusión, PolyBLEP, SVF, envolventes).
