# 0004 — Proyectos en RON, autocontenidos y versionados

- **Fecha:** 2026-11-19
- **Estado:** aceptada (por el usuario, 2026-11-20)

## Contexto

El objetivo del MVP incluye **guardar** el beat. El formato de proyecto se usa desde el hito H6 y, una vez que hay proyectos
guardados, cambiarlo cuesta migraciones. Restricciones: Rust con `serde` ([ADR 0001](0001-lenguaje-rust.md)), datos en microSD
(probablemente FAT32 o exFAT, sin verificar), batería que puede agotarse en cualquier momento, y un usuario que aprende Rust
(el formato tiene que poder leerse y depurarse a mano).

## Alternativas consideradas

| Opción | A favor | En contra |
|---|---|---|
| **RON + `serde`** | Legible y con comentarios; representa `enum` con datos de forma natural; nativo del ecosistema Rust | Menos conocido fuera de Rust |
| TOML + `serde` | Muy legible para configuración | Incómodo para arrays anidados de structs y para `enum` con datos (canción × cadenas × frases × pasos) |
| JSON + `serde` | Universal, muchas herramientas | Sin comentarios; `enum` con datos más verbosos |
| Binario (`bincode`, `postcard`) | Compacto y rápido | No se puede leer ni arreglar a mano; versionado más frágil |
| Samples referenciados desde una biblioteca | Ahorra espacio | Proyectos que se rompen al mover o borrar archivos |
| **Samples copiados a la carpeta del proyecto** | Proyectos portables y autocontenidos | Duplica archivos en la SD |

## Decisión

- Cada proyecto es una **carpeta** con `proyecto.ron` y una subcarpeta `samples/` con **copias** de los WAV que usa.
- El archivo es **RON**, con un modelo de archivo propio (disperso: solo las frases y cadenas que existen y los pasos no vacíos; estructura según [ADR 0006](0006-cadenas-por-pista.md)) separado del modelo en memoria.
- Campo `format_version` (entero): sube solo con cambios incompatibles; los cambios aditivos usan valores por defecto. Migraciones en cadena al abrir; los archivos de versión mayor se abren en solo lectura.
- **Guardado atómico**: archivo temporal + `fsync` + renombrado, conservando un `.bak`.

## Motivos

- RON es el formato con mejor relación entre legibilidad y fidelidad al modelo de datos de Rust.
- Los proyectos autocontenidos no se rompen y se pueden copiar a la Mac o compartir como una carpeta.
- El guardado con `.bak` protege ante cortes de batería, que en una consola portátil son esperables.

## Consecuencias

- Hay que escribir la conversión entre `ProjectFile` y `Project` (buen ejercicio de Rust para el hito H6).
- La SD acumula copias de samples; se puede agregar deduplicación por hash más adelante.
- Detalle y ejemplo en [spec-formatos](../solutions/groovebox/spec-formatos.md).

## Pendientes

- [ ] El usuario confirma RON y la copia de samples
- [ ] Verificar el sistema de archivos y el punto de montaje de la segunda microSD en dArkOS
- [ ] Verificar la versión del crate `ron` y su soporte de `enum` al implementar
