# Blueprint — estado y próximos pasos

Última actualización: 2026-11-19 (fin de la sesión 1)

Lo que **no** está en otros docs: plan de trabajo, pendientes del usuario y notas de la sesión.
Para los datos técnicos, ver `README.md` y `docs/`.

## Estado actual

- ✅ Investigación inicial: hardware, sistemas operativos y emulación (`docs/`)
- ✅ Decisión: Rust para las apps ([ADR 0001](docs/decisions/0001-lenguaje-rust.md))
- ✅ 6 agentes, 10 skills y `CLAUDE.md` con delegación automática
- ⏳ Sin código todavía. `infra/` y `projects/groovebox/` no existen aún
- ⏳ La consola sigue con el sistema de fábrica (sin respaldo ni dArkOS)

## Próximos pasos (en orden)

### Sin la consola (se pueden hacer ya, en paralelo)
1. **`dev-infra`**: infraestructura base en `infra/`. Comparar el contenedor arm64 contra `cargo zigbuild` (con ADR), armar el justfile (`run`, `build-arm`, `deploy`) y documentar el simulador de 640×480.
2. **`groovebox-designer`**: referentes y spec del MVP en `docs/solutions/groovebox/`.
3. **`rust-dev`** (después del 1): "hola mundo" en `projects/groovebox/` con una ventana SDL2 de 640×480 y un tono con `cpal`. Valida el stack y es la primera lección de Rust.

### Con la consola
4. Respaldar la microSD original con `dd` (ver `docs/firmware/sistemas-operativos.md`).
5. Flashear dArkOS en otra microSD y configurar Wi-Fi, SSH y el host `rgb20s` en `~/.ssh/config`.
6. **`device-ops`**: inventario. Confirma glibc, SDL2, códec de audio, chip Wi-Fi y si hay UDC (USB gadget).
7. Primer deploy del "hola mundo" y medición de latencia de audio.

### Más adelante
8. **`alarm-image-builder`**: imagen propia de Arch Linux ARM, solo después de tener el inventario.
9. Explorar USB gadget (MIDI o HID) si el inventario confirma que hay UDC.

## Pendientes del usuario

- [ ] Reiniciar Claude Code para que carguen los agentes, las skills y el `CLAUDE.md`
- [ ] Actualizar el remote (el repo se renombró):
      `git remote set-url origin git@github.com:gonzalobertomeu/powkiddy-rgb20s-hack.git`
- [ ] (Opcional) Instalar plugins oficiales:
  - `rustup component add rust-analyzer` y `/plugin install rust-analyzer-lsp@claude-plugins-official`, para análisis de código Rust
  - `/plugin install context7@claude-plugins-official`, para documentación actualizada de crates (usa un servicio externo)
  - No se recomienda `learning-output-style`: la skill `rust-mentor` ya cubre el modo aprendizaje
- [ ] Responder:
  - ¿Qué sistema trae la consola hoy?
  - ¿Hay una segunda microSD libre?
  - ¿Hay un lector de microSD para esta Mac?

## Notas de la sesión

- **Delegación:** el `description` de los agentes sirve como trigger, pero en este entorno la delegación automática necesita la autorización explícita que está en `CLAUDE.md`. Si los agentes no se usan solos, revisar eso primero.
- **Datos no verificados en las skills** (escritos de memoria, a confirmar antes de usarlos): offsets de arranque del RK3326 (sector 64, particiones desde 16 MiB), nombres de opciones del kernel, API exacta de `cpal`, mecanismo de flush-to-zero en Rust para aarch64.
- **Herramientas en la Mac:** `qemu-aarch64-static`, binfmt, Docker y `cargo` 1.99 instalados. Faltan `qemu-system-aarch64` y `aarch64-linux-gnu-gcc`.
- **No confundir** la RGB20S (RK3326) con la RGB20SX (RK3566), que sí está en mainline.
