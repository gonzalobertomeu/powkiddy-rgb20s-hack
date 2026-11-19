---
name: rust-realtime-audio
description: Patrones y reglas para audio en tiempo real en Rust (cpal/ALSA, callback de audio, rtrb, preasignación, detección de asignaciones, arquitectura interfaz-motor). Usar al implementar o revisar código que corre en el hilo de audio o que se comunica con él.
---

# Audio en tiempo real en Rust

## Arquitectura

```
Hilo de interfaz (SDL2: input, render a 60 fps)
   │  comandos (NoteOn, SetParam, SetStep, Play/Stop...)
   ▼  rtrb::Producer<Command>          (lock-free, SPSC)
Hilo de audio (callback de cpal)
   │  procesa comandos → avanza el secuenciador → renderiza voces
   ▼  rtrb::Producer<Event>            (posición de reproducción, picos de nivel)
Hilo de interfaz (dibuja el playhead y los medidores)
```

- **El secuenciador vive en el hilo de audio.** Así la sincronía es exacta a nivel de muestra. La interfaz solo envía ediciones.
- El estado compartido grande (un sample cargado) se prepara en el hilo de interfaz y se pasa **ya construido** por la cola (por ejemplo `Box<Sample>`). Para liberar el viejo sin hacerlo en el hilo de audio, se devuelve por otra cola, para que lo libere la interfaz.

## Reglas dentro del callback

Prohibido: asignar o liberar memoria (`Vec::push` que crezca, `Box::new`, `String`, `format!`, `clone()` de cosas con heap) · `Mutex` / `RwLock` · I/O, `println!`, logging · syscalls · `panic!` (abortaría el audio).

Permitido: `rtrb` (`pop`/`push` sin bloquear) · atómicos (`AtomicU32`, `AtomicBool`) · buffers preasignados · aritmética.

## Esqueleto con cpal

```rust
let (mut cmd_tx, mut cmd_rx) = rtrb::RingBuffer::<Command>::new(1024);
let config = cpal::StreamConfig {
    channels: 2,
    sample_rate: cpal::SampleRate(48_000),
    buffer_size: cpal::BufferSize::Fixed(256), // ~5.3 ms a 48 kHz
};
let mut engine = Engine::new(48_000.0); // preasigna todo acá, fuera del callback
let stream = device.build_output_stream(
    &config,
    move |out: &mut [f32], _| {
        while let Ok(cmd) = cmd_rx.pop() { engine.handle(cmd); }
        engine.render(out); // interleaved estéreo
    },
    |err| { /* xrun u otro error: contar con un atómico, no loggear acá */ },
    None,
)?;
stream.play()?;
```

Verificar la API exacta contra la versión de `cpal` del `Cargo.lock`: cambió entre versiones.
El tamaño de buffer real que acepta ALSA en la consola **(sin verificar)** lo mide `device-ops`.

## Detectar asignaciones en debug

Usar el crate `assert_no_alloc` en builds de debug, envolviendo el render:
`assert_no_alloc(|| engine.render(out))`. Hace panic si se asigna memoria dentro del callback.

## Testeo sin hardware

- El `Engine` no depende de cpal: `engine.render(&mut buf)` se testea en `cargo test`.
- Render offline a WAV (`hound`) para escucharlo y comparar.
- Medir el tiempo de `render` por bloque contra el presupuesto: `256 / 48000 s = 5.33 ms`. Apuntar a usar menos del 50 %.
