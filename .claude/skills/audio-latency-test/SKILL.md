---
name: audio-latency-test
description: Procedimiento para medir el audio en la consola real (dispositivos ALSA, tamaños de buffer aceptados, xruns, uso de CPU del motor y latencia de salida) y registrar los resultados en docs/hardware/mediciones.md. Usar al validar el motor de audio o al elegir el tamaño de buffer.
---

# Medición de audio en la RGB20S

## 1. Salida básica

```sh
aplay -l                                            # tarjeta y dispositivo (hw:X,Y)
speaker-test -D hw:0,0 -c 2 -t sine -f 440 -l 1     # ajustar hw:X,Y; bajar el volumen antes (amixer)
```

## 2. Parámetros reales negociados

Con la app (o `speaker-test`) sonando:
```sh
cat /proc/asound/card0/pcm0p/sub0/hw_params    # rate, format, period_size, buffer_size reales
cat /proc/asound/card0/pcm0p/sub0/status       # estado, avail, delay
```
Comparar el `period_size` pedido con el obtenido: ALSA puede redondearlo.

## 3. Barrido de buffer y xruns

Para cada tamaño de buffer (64, 128, 256, 512 frames) correr la app durante 60 s con una carga representativa (un patrón con N voces) y registrar:
- **xruns:** contador que expone la app (el error callback de cpal incrementa un atómico; la app lo imprime al salir) o `speaker-test` con `-p/-b`.
- **CPU:** `top -b -n 30 -d 1 -p $(pidof app)`, o mejor que la app mida el tiempo de `render()` dividido por la duración del bloque (carga DSP en %).
- **Temperatura y frecuencia** al inicio y al final (ver la skill `rgb20s-device-inventory`).

Latencia de salida teórica = `buffer_size / sample_rate` (por ejemplo, 2 periodos × 256 / 48000 = 10.7 ms).

## 4. Latencia ida y vuelta (opcional, requiere entrada)

Solo si hay entrada de audio (USB o micrófono) **(sin verificar)**: un cable de loopback de la salida a la entrada y medir con `alsa_delay` (de zita-alsa-pcmi) o con un pulso grabado.

## 5. Registro (`docs/hardware/mediciones.md`)

```markdown
## AAAA-MM-DD — <binario y commit> — <sistema>
| Buffer | Rate | Voces | Carga DSP media / pico | Xruns en 60 s | Temp. inicio → fin |
|---|---|---|---|---|---|
Condiciones: volumen, auriculares o parlante, batería o cargador, governor de CPU.
Conclusión: buffer recomendado = ...
```

No extrapolar: si algo no se midió, se escribe "no medido".
