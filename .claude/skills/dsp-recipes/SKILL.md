---
name: dsp-recipes
description: Recetas de DSP de audio baratas y correctas para un Cortex-A35 (PolyBLEP, SVF TPT, envolventes, smoothing de parámetros, baterías sintetizadas, FM, ruido, denormales, delay). Usar al implementar o revisar bloques del motor de sonido.
---

# Recetas de DSP

Convenciones: `f32`, `fs` = sample rate, fase normalizada en [0, 1), `dt = freq / fs`.

## Osciladores sin aliasing: PolyBLEP

```rust
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt { let t = t / dt; 2.0 * t - t * t - 1.0 }
    else if t > 1.0 - dt { let t = (t - 1.0) / dt; t * t + 2.0 * t + 1.0 }
    else { 0.0 }
}
// sierra: 2*phase - 1 - poly_blep(phase, dt)
// cuadrada: (if phase < 0.5 {1} else {-1}) + poly_blep(phase, dt) - poly_blep((phase + 0.5) % 1.0, dt)
```

Para una triangular: integrar la cuadrada con PolyBLEP (con un integrador con pérdida) o usar una wavetable.

## Filtro: SVF TPT (Zavalishin / Cytomic)

```rust
// setup (solo cuando cambian el cutoff o la resonancia)
let g = (PI * cutoff / fs).tan();
let k = 2.0 - 2.0 * res;            // res en [0, 1)
let a1 = 1.0 / (1.0 + g * (g + k));
let a2 = g * a1;
let a3 = g * a2;
// por muestra
let v3 = x - ic2;
let v1 = a1 * ic1 + a2 * v3;
let v2 = ic2 + a2 * ic1 + a3 * v3;
ic1 = 2.0 * v1 - ic1;
ic2 = 2.0 * v2 - ic2;
let (low, band, high) = (v2, v1, x - k * v1 - v2);
```

Es estable frente a modulación rápida. `tan()` es caro: recalcular por bloque o usar una tabla o aproximación.

## Envolvente ADSR exponencial

Un one-pole hacia un objetivo: `env += (target - env) * coef`, con `coef = 1 - exp(-1 / (tiempo_s * fs))`.
En el ataque, apuntar a un objetivo de aproximadamente 1.2 y cortar en 1.0, para que la curva no sea demasiado lenta al final.

## Smoothing de parámetros (sin zipper noise)

One-pole de unos 5 a 20 ms sobre cada parámetro modulable: `p += (target - p) * coef`.

## Baterías sintetizadas

- **Kick:** seno con barrido de pitch exponencial (por ejemplo, de 150 Hz a 50 Hz en unos 50 ms) más una envolvente de amplitud. Opcional: un click corto de ruido.
- **Snare:** seno o triangular de unos 180 Hz con decay corto, más ruido blanco pasado por un pasa-banda o pasa-altos con decay más largo.
- **Hi-hat:** ruido, o una suma de cuadradas con frecuencias inarmónicas, pasado por un pasa-altos de unos 7 kHz, con decay de 30 a 300 ms (abierto o cerrado).
- **Clap:** ráfagas de ruido filtrado (3 o 4 picos separados por unos 10 ms) más una cola.

## FM de 2 operadores

`out = sin(2π·phase_c + index · sin(2π·phase_m))`, con `phase_m` a `ratio · freq`. Usar una tabla de seno de 2048 muestras con interpolación lineal en lugar de `sin()`.

## Ruido

Xorshift32 en el hilo de audio: sin asignar memoria, determinístico y barato.

## Denormales

En aarch64, activar flush-to-zero (bit FZ del registro FPCR) al iniciar el hilo de audio, o sumar un offset mínimo (1e-20) en la realimentación de filtros y reverbs. Verificar el mecanismo exacto en Rust para aarch64 antes de usarlo **(sin verificar)**.

## Delay

Buffer circular preasignado con largo potencia de 2 (para usar `& mask` en lugar de `%`) e interpolación lineal para tiempos fraccionarios. Feedback menor que 1, con un pasa-bajos en el loop.

## Costo

Anotar las operaciones aproximadas por muestra de cada bloque. Por ejemplo, un SVF son ~10 multiplicaciones y sumas; un oscilador PolyBLEP, ~10 más una rama.

## Fuentes

- Vadim Zavalishin, *The Art of VA Filter Design*.
- Andrew Simper (Cytomic), *SVF linear trapezoidal* (notas técnicas).
- Will Pirkle, *Designing Software Synthesizer Plug-Ins in C++*.
- Välimäki y Huovilainen, *Antialiasing oscillators in subtractive synthesis* (PolyBLEP).
