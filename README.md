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
| `rust-mentor` | rust-dev, dsp-engineer |
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
- [Soluciones: índice de ideas](docs/solutions/README.md)
  - [Groovebox](docs/solutions/groovebox.md)
- Decisiones
  - [0001 — Rust como lenguaje](docs/decisions/0001-lenguaje-rust.md)
- [Proyectos](projects/README.md)

## Convenciones

- Los datos no confirmados se marcan con **(sin verificar)**.
- Cada documento lista sus fuentes al final.
- Fechas en formato AAAA-MM-DD.
