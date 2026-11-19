---
name: adr
description: Registrar una decisión de arquitectura o de diseño (ADR) en docs/decisions/. Usar cuando se elige entre alternativas (lenguaje, librería, formato, enfoque de build, distro, diseño del motor) y la elección afecta trabajo futuro.
---

# Registrar una decisión (ADR)

1. Buscar el próximo número: `ls docs/decisions/` y sumar uno al mayor (formato `NNNN`, por ejemplo `0002`).
2. Crear `docs/decisions/NNNN-titulo-en-kebab-case.md` con esta plantilla:

```markdown
# NNNN — Título corto en imperativo o como afirmación

- **Fecha:** AAAA-MM-DD
- **Estado:** propuesta | aceptada | reemplazada por [NNNN](NNNN-...md)

## Contexto
Qué problema hay y qué restricciones aplican (hardware, rendimiento, aprendizaje, tiempo).

## Alternativas consideradas
| Opción | A favor | En contra |
|---|---|---|

## Decisión
Qué se eligió, en una o dos frases.

## Motivos
Por qué esta opción y no las otras.

## Consecuencias
Qué implica: trabajo nuevo, limitaciones, riesgos, qué hay que verificar.

## Pendientes
- [ ] ...
```

3. Si la decisión la toma un agente y no el usuario, usar **Estado: propuesta** y avisarlo en el reporte. Solo el usuario la pasa a "aceptada".
4. Agregar el ADR a la sección "Decisiones" del `README.md` y enlazarlo desde el doc afectado.
5. Para cambiar una decisión, no se edita la vieja: se crea una nueva y se marca la anterior como "reemplazada por".
