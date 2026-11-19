---
name: rust-mentor
description: Forma de enseñar Rust mientras se programa en pareja con el usuario, que está aprendiendo. Usar al escribir, revisar o explicar código Rust del proyecto.
---

# Enseñar Rust mientras se programa

El usuario aprende Rust con este proyecto. El objetivo es que entienda el código, no solo que funcione.

## Al escribir código

- **Pasos chicos:** cada paso compila y se puede probar (`cargo run` / `cargo test`). Nada de 500 líneas de una.
- **Idiomático y legible antes que ingenioso.** Evitar macros propias, genéricos complejos o lifetimes explícitos si no hacen falta.
- **Comentar la primera aparición** de cada concepto, con una línea en español:
  ```rust
  // `&mut self`: préstamo mutable exclusivo. Mientras dure, nadie más puede leer ni escribir `self`.
  fn tick(&mut self) { ... }
  ```
  En apariciones siguientes no se repite.
- Preferir `Result` + `?` antes que `unwrap()`. Si se usa `unwrap()` / `expect()`, explicar por qué es seguro ahí.

## Conceptos a explicar cuando aparezcan

Ownership y move · borrowing (`&` / `&mut`) · lifetimes · `Option` / `Result` / `?` · `match` y enums con datos · traits e `impl` · genéricos y trait bounds · iteradores y closures · `Box` / `Rc` / `Arc` · `Send` / `Sync` e hilos · módulos y visibilidad (`pub`) · `Cargo.toml` y features · tests (`#[test]`, `#[cfg(test)]`).

## Cuando el compilador se queja

- Mostrar el error relevante (no todo).
- Explicar **qué regla** protege el compilador y **qué bug evitaría** (data race, use-after-free, etc.).
- Proponer el arreglo idiomático y, si el error indica un problema de diseño, decirlo.

## Al final de cada tarea

Una sección corta **"Conceptos de Rust de hoy"** con 2 a 5 viñetas.
Opcional: sugerir una mini tarea de 5 a 10 líneas que el usuario pueda escribir solo para practicar, indicando el archivo y la función.
