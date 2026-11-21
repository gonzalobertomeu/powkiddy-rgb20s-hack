# Powkiddy RGB20S — hack

Investigación y desarrollo para darle a la Powkiddy RGB20S usos distintos a la emulación
(groovebox musical, herramientas, etc.).

## Estructura

```
docs/
  hardware/    Arquitectura, specs y cómo emular/probar sin la consola
  firmware/    Sistemas operativos compatibles, arranque, kernel
  solutions/   Ideas y diseños de soluciones (groovebox, otras)
  decisions/   Registro de decisiones (ADR): qué se decidió y por qué
projects/      Código: el trabajo concreto de cada solución
infra/         Entorno de desarrollo: contenedores, compilación cruzada, QEMU, justfile
.claude/agents/  Agentes especializados del proyecto
```

## Agentes

| Agente | Rol |
|---|---|
| `alarm-image-builder` | Imagen Linux mínima con Arch Linux ARM: bootloader, kernel, device tree, rootfs |
| `rust-dev` | Apps en Rust: estructura, interfaz, secuenciador, integración (modo aprendizaje) |
| `dsp-engineer` | Motor de sonido: síntesis, filtros y efectos dentro del presupuesto del A35 |
| `groovebox-designer` | Investigación y specs del groovebox (no escribe código) |
| `dev-infra` | Infraestructura local: contenedores, compilación cruzada, QEMU, simulador, justfile |
| `device-ops` | Consola real por SSH: inventario, despliegue y mediciones |

Las reglas de delegación están en `CLAUDE.md`. Las skills del proyecto están en `.claude/skills/`:

| Skill | Usada por |
|---|---|
| `adr` | todos (menos device-ops) |
| `rust-realtime-audio` | rust-dev, dsp-engineer |
| `rust-aarch64-cross` | rust-dev, dev-infra |
| `rockchip-rk3326-boot` | alarm-image-builder |
| `alarm-rootfs` | alarm-image-builder, dev-infra |
| `dsp-recipes` | dsp-engineer |
| `groovebox-spec-writing` | groovebox-designer |
| `rgb20s-device-inventory` | device-ops |
| `audio-latency-test` | device-ops |

## Índice

- **[Blueprint: estado y próximos pasos](blueprint.md)**. Empezar por acá al retomar.

- [Hardware: especificaciones](docs/hardware/rgb20s.md)
- [Hardware: emulación y pruebas sin la consola](docs/hardware/emulacion.md)
- [Firmware: sistemas operativos](docs/firmware/sistemas-operativos.md)
- [Firmware: Arch Linux ARM (arranque, kernel, rootfs, imagen, riesgos)](docs/firmware/arch-linux-arm.md)
- [Soluciones: índice de ideas](docs/solutions/README.md)
  - [Groovebox: índice](docs/solutions/groovebox/README.md)
    - [Referentes](docs/solutions/groovebox/referentes.md)
    - [Spec de producto (features e hitos)](docs/solutions/groovebox/spec-producto.md)
    - [Spec de interfaz (mockups y controles)](docs/solutions/groovebox/spec-ui.md)
    - [Spec del motor (voces, secuenciador y CPU)](docs/solutions/groovebox/spec-motor.md)
    - [Spec de formatos (proyecto, samples, MIDI)](docs/solutions/groovebox/spec-formatos.md)
    - [DSP del motor (H2: voces, instrumentos, costos)](docs/solutions/groovebox/dsp.md)
- Decisiones
  - [0001 — Rust como lenguaje](docs/decisions/0001-lenguaje-rust.md)
  - [0002 — Interfaz híbrida grilla + tracker](docs/decisions/0002-interfaz-hibrida-grilla-tracker.md) (aceptada)
  - [0003 — Patrones globales y pistas monofónicas](docs/decisions/0003-modelo-de-secuenciacion.md) (reemplazada por 0006 y 0007)
  - [0004 — Proyectos en RON](docs/decisions/0004-formato-de-proyecto-ron.md) (aceptada)
  - [0005 — Configuración de audio](docs/decisions/0005-configuracion-de-audio.md) (aceptada)
  - [0006 — Canción de cadenas por pista](docs/decisions/0006-cadenas-por-pista.md) (aceptada)
  - [0007 — Polifonía en el MVP](docs/decisions/0007-polifonia-en-el-mvp.md) (aceptada)
  - [0008 — Compilación cruzada en contenedor x86_64 con libs arm64 de trixie](docs/decisions/0008-compilacion-cruzada-contenedor-x86-multiarch.md) (aceptada)
  - [0009 — Arranque con U-Boot mainline y miniloader de rkbin, tarjeta MBR](docs/decisions/0009-arranque-uboot-miniloader-rkbin.md) (aceptada)
  - [0010 — Kernel mainline 7.1.2 con DTS y drivers de ROCKNIX](docs/decisions/0010-kernel-mainline-con-dts-y-drivers-de-rocknix.md) (aceptada)
  - [0011 — Rootfs de Arch Linux ARM con systemd, armada en contenedores](docs/decisions/0011-rootfs-alarm-con-systemd.md) (aceptada)
  - [0012 — Acceso por USB gadget NCM + ACM con IP fija y DHCP](docs/decisions/0012-acceso-por-usb-gadget-ncm-acm.md) (aceptada)
  - [0013 — Tantas ranuras de fundido como voces en la bolsa](docs/decisions/0013-ranuras-de-fundido.md) (aceptada)
  - [0014 — Arrancar en un launcher propio en tty1, con un wrapper de sesión](docs/decisions/0014-sesion-launcher-en-tty1.md) (propuesta)
  - [0015 — Splash de arranque: logo propio en el kernel y un puente DRM en userspace](docs/decisions/0015-splash-de-arranque-logo-del-kernel-y-drm.md) (propuesta)
  - [0016 — Un servicio de sistema en Rust maneja el volumen y mezcla a mono en ALSA](docs/decisions/0016-servicio-de-audio-del-sistema.md) (propuesta)
- [Entorno de desarrollo: justfile, compilación cruzada, simulador y deploy](infra/README.md)
- [Proyectos](projects/README.md)
  - [launcher: menú al prender la consola](projects/launcher/README.md)
  - [linux-image: imagen de Arch Linux ARM](projects/linux-image/README.md)
    - [splash de arranque "KingaGroove"](projects/linux-image/splash/README.md)
  - [rgb20s-audio: botones de volumen y modo mono/estéreo](projects/rgb20s-audio/README.md)
  - [rgb20s-common: estado del audio y batería, compartidos por las apps](projects/rgb20s-common/README.md)

## Convenciones

- Los datos no confirmados se marcan con **(sin verificar)**.
- Cada documento lista sus fuentes al final.
- Fechas en formato AAAA-MM-DD.
