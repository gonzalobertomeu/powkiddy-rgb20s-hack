# Proyecto: hack de la Powkiddy RGB20S

Darle a la consola (RK3326, aarch64) usos distintos a la emulación, empezando por un groovebox en Rust.
Índice y estructura: `README.md`. Estado y próximos pasos: `blueprint.md` (leerlo al retomar y actualizarlo al cerrar cada sesión).

## Convenciones (aplican a la sesión principal y a todos los agentes)

- Todo en **español**: respuestas, documentación y comentarios. Los identificadores del código, en inglés.
- Datos no confirmados: se marcan **(sin verificar)**. Cada documento cita sus fuentes al final.
- Decisiones importantes: ADR en `docs/decisions/NNNN-titulo.md` (skill `adr`).
- Investigación en `docs/` (`hardware/`, `firmware/`, `solutions/`); código en `projects/<nombre>/`; entorno de build en `infra/`.
- **Sin modo aprendizaje:** el usuario no quiere explicaciones de Rust. Escribir el código directamente; los reportes cuentan qué se hizo, no enseñan conceptos (2026-11-19).
- No commitear salvo pedido explícito.

## Delegación a agentes (autorizada)

El usuario **autoriza delegar sin preguntar** en los agentes de `.claude/agents/` cuando la tarea
encaja claramente con uno. Las tareas independientes se lanzan en paralelo. Tareas chicas
(una pregunta corta, una edición de una línea) se hacen directamente, sin delegar.

| Si la tarea es sobre... | Delegar en |
|---|---|
| Bootloader, kernel, device tree, drivers, rootfs o imagen de Arch Linux ARM | `alarm-image-builder` |
| Escribir, revisar o depurar código Rust de las apps (estructura, interfaz, secuenciador) | `rust-dev` |
| Síntesis, filtros, efectos, calidad de audio, optimización de DSP | `dsp-engineer` |
| Investigar referentes o escribir y ampliar la spec del groovebox | `groovebox-designer` |
| Contenedores, compilación cruzada, QEMU, simulador, justfile, tiempos de build | `dev-infra` |
| Cualquier cosa que requiera la consola física (SSH, inventario, deploy, mediciones) | `device-ops` |

Reglas de orquestación:
- Los agentes no pueden lanzar otros agentes: **la sesión principal coordina**. Si un agente reporta que necesita algo de otro, la sesión principal lo deriva.
- Los agentes no ven esta conversación: al delegar, pasar todo el contexto necesario (qué, por qué, archivos relevantes, decisiones del usuario).
- Al recibir el reporte, resumirle al usuario lo importante: el reporte del agente no le llega directo.
- **Verificar con `just check`** (`APP=<crate> just check`): una línea por paso. No correr test, clippy, fmt y build por separado salvo para depurar una falla.
- **Reportes breves** (máximo ~40 líneas): qué se hizo, verificaciones con su resultado, riesgos y pendientes. Sin listar cada archivo tocado ni repetir lo que ya está en los docs: el detalle va al README o al doc correspondiente, y el reporte lo enlaza.
- Modelos (frontmatter de cada agente): `opus` para `rust-dev`, `dsp-engineer` y `alarm-image-builder`; `sonnet` para `dev-infra`, `device-ops` y `groovebox-designer`.
- Nada que escriba en una SD o dispositivo real, instale paquetes en el host o modifique la consola sin confirmación del usuario, aunque lo pida un agente.
