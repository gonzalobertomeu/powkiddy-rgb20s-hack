# Blueprint — estado y próximos pasos

Última actualización: 2026-11-19 (fin de la sesión 1)

Lo que **no** está en otros docs: plan de trabajo, pendientes del usuario y notas de la sesión.
Para los datos técnicos, ver `README.md` y `docs/`.

## Estado actual

- ✅ Investigación inicial: hardware, sistemas operativos y emulación (`docs/`)
- ✅ Decisión: Rust para las apps ([ADR 0001](docs/decisions/0001-lenguaje-rust.md))
- ✅ 6 agentes, 10 skills y `CLAUDE.md` con delegación automática
- ✅ `infra/` y `justfile` armados ([ADR 0008](docs/decisions/0008-compilacion-cruzada-contenedor-x86-multiarch.md), aceptado): contenedor x86 con cross gcc y librerías arm64 de Debian trixie (la misma base que dArkOS). Build limpio en ~20 s e incremental en ~2 s. Ver `infra/README.md`
- ✅ H0 en `projects/groovebox/`: ventana SDL2 + tono de 440 Hz con cpal/rtrb, probado nativo y en aarch64 emulado (glibc 2.34). Falta probarlo en la consola (video KMSDRM, mapeo de botones, períodos ALSA con `GROOVEBOX_FRAMES`)
- ✅ La consola tiene ArkOS, casi seguro **(sin verificar)**: `boot.ini` de tipo `odroidgoa-uboot-config`, `Image` de 2021, `uInitrd` de 2020 y DTB `rk3326-rg351mp-linux.dtb` (con variantes `.orig` y `.tony`), cargado desde `mmc 1:1`, raíz por UUID. TF1 de 16,4 GB: BOOT FAT32 117 MB + raíz ext4 10,8 GB + EASYROMS exFAT 5,4 GB (2026-11-20)

## Próximos pasos (en orden)

### Sin la consola (se pueden hacer ya, en paralelo)
1. ✅ **`dev-infra`**: infraestructura base en `infra/`. Comparar el contenedor arm64 contra `cargo zigbuild` (con ADR), armar el justfile (`run`, `build-arm`, `deploy`) y documentar el simulador de 640×480.
2. ✅ **`groovebox-designer`**: referentes y spec del MVP en [`docs/solutions/groovebox/`](docs/solutions/groovebox/README.md). Los ADR 0002 a 0005 están en estado *propuesta*, y las preguntas abiertas para el usuario están en el README de la spec.
3. ✅ **`rust-dev`** (después del 1): "hola mundo" en `projects/groovebox/` con una ventana SDL2 de 640×480 y un tono con `cpal`. Valida el stack. Lo escribe Claude, sin modo aprendizaje.

### Hitos del groovebox (de la spec, cada uno termina en algo que suena o se ve)
H0 ventana SDL2 + tono · H1 input, fuente y grilla (prueba de legibilidad en la consola) · H2 hilo de audio con `rtrb`, bombo con X · H3 secuenciador con bolsa de voces simple (**primer beat**) · H4 8 pistas, vista Frase, voces por pista · H5 cadenas y canción (se puede partir en H5a y H5b) · H6 guardar/cargar en RON (**MVP mínimo**) · H7 sampler · H8 sustractivo polifónico, acordes y robo de voces · H9 mezcla, efectos y sticks · H10 clonar y copiar/pegar · H11 medición en la consola, prueba de estrés y lanzador en Ports. Detalle en `docs/solutions/groovebox/spec-producto.md`.

### Estado del groovebox
- ✅ H0 (ventana + tono) y ✅ H1 (fuente Spleen 8×16/12×24, input con repetición y gestos, modelo song/chain/phrase, pantallas Grilla y Frase; Canción y Cadena de solo lectura; 73 tests). Capturas reproducibles con `GROOVEBOX_SCRIPT` (ver `projects/groovebox/README.md`).
- ✅ H2: motor real (colas rtrb, bolsa de voces con robo, percusión sintetizada, sinte PolyBLEP + SVF, flush-to-zero, soft clip, medidor de CPU). Subcomandos `groovebox render` (WAV offline) y `groovebox bench`. 99 tests. DSP documentado en `docs/solutions/groovebox/dsp.md`.
- CPU con la bolsa llena (no es la consola): ~0,7 % en la Mac y ~13 % en QEMU. Falta medirlo en la consola con `groovebox bench`.
- ✅ ADR 0013 aceptado: 16 ranuras de fundido (`FADE_SLOTS = POOL_VOICES`).
- ✅ H3: secuenciador en el hilo de audio, con precisión de muestra y sin deriva. Loop de la sección visible con las 8 pistas, cabezal en la Grilla y la Frase, edición en vivo (`src/sync.rs` manda diferencias al motor), mute/solo y proyecto demo (`GROOVEBOX_DEMO=1`). 119 tests. El contador de voces ahora mide por subbloque y el log informa `notas N`.
- ✅ H5: cabezales por pista con la regla de bloques, modos canción, sección, cadena y frase, Start contextual (Select + Start = loop de sección), pantallas Canción y Cadena editables y "seguir reproducción" (activado por defecto). 138 tests. `h5-cancion.wav` con la canción demo. El agente se detuvo antes de entregar el reporte; la sesión principal verificó el resultado (tests, clippy, fmt, cross).
- Pendiente del H5: X en Canción y Cadena toca solo la primera nota (falta un modo "una vez" en el motor para escuchar la frase completa).
- Siguiente: H6 (guardar y cargar en RON: **MVP mínimo**) o H4 (swing y voces superpuestas).
- Pendiente: licencias de los crates de Rust en el deploy (¿`cargo-about`?).

### Camino a Arch Linux ARM (decisión del usuario, 2026-11-19: ir directo a Arch, sin relevar antes el sistema actual)
4. ✅ (armada, sin probar) **`alarm-image-builder`**: investigar y armar la imagen de Arch Linux ARM para la RGB20S (U-Boot, kernel mainline con el DTS de ROCKNIX, rootfs mínima, red por USB gadget y SSH, autoarranque de una app). Todo sobre archivos `.img`, nunca sobre la SD. Resultado: `infra/build/out/rgb20s-arch.img` (1235 MiB) con `just img`, que pasa los 41 chequeos de `just img-check`. Kernel 7.1.2 con el DTS de ROCKNIX, gadget NCM + ACM (consola en 10.55.0.1, usuario `alarm`/`alarm`) y logs de cada arranque en `/boot/logs/`. ADR 0009 a 0012 aceptados. Riesgos del primer arranque en `docs/firmware/arch-linux-arm.md`.
**Primer arranque (2026-11-20): ✅ Arch arranca y hay SSH por USB.**
- Respaldo de ArkOS en `~/rgb20s-backup/` (`tf1-arkos.img`, 16 361 455 616 bytes, sha256 `ff4034f7…7506533`, sin comparar contra la tarjeta; y `boot-files/`). La imagen grabada vino de Omarchy (`rgb20s-arch.img`, sha256 `1a782b5f…4214f1dd9`).
- Riesgo 8 confirmado y resuelto: el DTS de ROCKNIX deja `otg-port` del PHY en `disabled` (`Requested PHY is disabled`, UDC `not attached`). Se agregó `&u2phy_otg { status = "okay"; }` al DTS del repo; en la SD se parcheó solo el DTB (el original quedó como `.dtb.orig`). **Falta recompilar la imagen** para que lo traiga.
- El gadget **no enumera con cable USB-C a USB-C** (la Mac no entrega VBUS: el cargador marca `ONLINE=0`; probablemente faltan resistencias en CC del puerto OTG, sin verificar). **Con cable USB-A a USB-C funciona.** El puerto marcado OTG es el de datos; el DC solo carga.
- Acceso: `ssh -i ~/.ssh/powkiddy alarm@10.55.0.1` (la Mac recibe 10.55.0.15). La clave se agregó a mano por serie (`/dev/cu.usbmodem*`). El SSH no acepta contraseña.
- Estado: kernel 7.1.2-rgb20s, sistema `running` sin fallas, ALSA ve `rk817int`, input `event0-3` y `js0`. Raíz de 930 MB sin agrandar (`GROW_ROOT=no`). Sin reloj: la fecha queda en 2026-11-10.
- ✅ Raíz agrandada a 15 GB (`GROW_ROOT=yes`). ✅ `sdl2-compat` 2.32.74 + `sdl3` 3.4.20 instalados con `pacman -Syu` (175 paquetes, 867 MB usados).
- Internet en la consola sin tocar la red de la Mac: túnel SOCKS inverso, `ssh -R 1080 …` y `env all_proxy=socks5h://127.0.0.1:1080 pacman …`. Antes, poner la hora (`sudo date -u -s`) porque no hay RTC.
- Bugs de la imagen encontrados: (1) faltaba el llavero de pacman (`build-image.sh` lo borra y nadie lo recreaba): arreglado en `firstboot.sh`, sin probar en imagen nueva; (2) el kernel no trae Landlock y pacman 7 falla con su sandbox: por ahora `--disable-sandbox` (lo corre el usuario); **pendiente** `CONFIG_SECURITY_LANDLOCK=y` y agregarlo a `CONFIG_LSM`. `sudo` pide la contraseña de `alarm`.
- Siguiente (viejo): `GROW_ROOT=yes`, instalar `sdl2` (sin internet en la consola: NAT por USB desde la Mac o `pacman -U`), build aarch64 en la Mac (el contenedor de Omarchy no está; `cargo zigbuild` o adaptar), primer deploy del groovebox, inventario y medición de audio.

5. ✅ **Antes de flashear:** con la SD actual montada en la Mac, respaldar en solo lectura la partición de arranque (DTB, U-Boot, config) y, si hay espacio, una imagen completa con `dd`. Es la única referencia de un sistema que arranca en esta consola.
6. ✅ Flashear la imagen de Arch (2026-11-20).
7. Primer arranque. Sin Wi-Fi ni serie, el diagnóstico depende de: la pantalla, logs que el sistema deje en la partición de arranque y, si funciona, SSH por USB gadget.
8. **`device-ops`**: inventario en Arch (adaptar `infra/device/inventory.sh` o correrlo por SSH). Primer deploy del H0 y medición de latencia de audio.

### Más adelante
9. Explorar USB gadget (MIDI o HID) además de red.
10. Ajustar el contenedor de build a librerías de Arch ARM si hace falta (hoy usa Debian trixie: los binarios piden glibc 2.34, compatibles con Arch).

## Pendientes del usuario

- [ ] Decidir si se agrega una regla `opaque` de Hyprland para la ventana `rgb20s-sim` (hoy se ve translúcida por la opacidad por defecto de Omarchy)
- [ ] Instalar `just`: `sudo pacman -S just` (sin esto no anda ninguna receta)
- [ ] (Opcional, solo para `just qemu-boot`) `sudo pacman -S qemu-system-aarch64`
- [x] Aceptar el ADR 0008 (compilación cruzada)
- [x] Aceptar los ADR 0004, 0005 y 0009 a 0012 (2026-11-20)
- [x] Respaldar la SD actual y flashear `rgb20s-arch.img` (2026-11-20)
- [x] Reiniciar Claude Code para que carguen los agentes, las skills y el `CLAUDE.md`
- [x] Actualizar el remote (el repo se renombró):
      `git remote set-url origin git@github.com:gonzalobertomeu/powkiddy-rgb20s-hack.git`
- [ ] (Opcional) Instalar plugins oficiales:
  - `rustup component add rust-analyzer` y `/plugin install rust-analyzer-lsp@claude-plugins-official`, para análisis de código Rust
  - `/plugin install context7@claude-plugins-official`, para documentación actualizada de crates (usa un servicio externo)
- [x] Responder (2026-11-19):
  - Sistema actual: ArkOS o dArkOS (a confirmar cuál)
  - Segunda microSD: sí, la consola usa dos (TF1 sistema, TF2 datos; corregido el 2026-11-20). No hace falta conservar el sistema original
  - Lector de microSD: sí. También hay auriculares o parlante para probar el audio
- [x] Wi-Fi: la consola no tiene. Hay que usar la SD o USB OTG

## Decisiones del usuario sobre el groovebox (2026-11-19)

- Interfaz híbrida grilla + tracker (ADR 0002).
- Instrumento fijo por pista, acordes con columna ACORDE, celda vacía vuelve al inicio del bloque y creación implícita desde la Grilla.
- **Cadenas por pista** (estilo LSDJ/M8), no patrón global: cambia la propuesta del ADR 0003.
- **Polifonía en el MVP.**
- P-locks en v1.
- Reflejado en los ADR 0006 y 0007 (aceptados); el 0003 queda reemplazado. Las preguntas abiertas (4 nuevas + 12 anteriores) están en `docs/solutions/groovebox/README.md`.

## Gadget USB: lo que se sabe sin la consola

Según el código de dArkOS, el kernel trae dwc2 dual-role, configfs y ECM/RNDIS. Pero el DTS del RG351MP tiene `otg-port` en `disabled`: si la RGB20S usa ese DTB, el USB-C podría estar cableado al controlador solo-host (sin verificar). Lo decide el inventario (`infra/device/inventory.sh`, guía en `docs/hardware/inventario.md`), con tres corridas: A sin nada, B con tethering y C con cable a la Mac.

## Notas de la sesión

- **Delegación:** el `description` de los agentes sirve como trigger, pero en este entorno la delegación automática necesita la autorización explícita que está en `CLAUDE.md`. Si los agentes no se usan solos, revisar eso primero.
- **Datos no verificados en las skills** (escritos de memoria, a confirmar antes de usarlos): offsets de arranque del RK3326 (sector 64, particiones desde 16 MiB), nombres de opciones del kernel, API exacta de `cpal`, mecanismo de flush-to-zero en Rust para aarch64.
- **Herramientas en la Mac:** `qemu-aarch64-static`, binfmt, Docker y `cargo` 1.99 instalados. Faltan `qemu-system-aarch64` y `aarch64-linux-gnu-gcc`.
- **Riesgo de glibc:** `build-arm` genera binarios que piden glibc 2.34. Si la consola tiene ArkOS viejo (glibc 2.30), no arrancan: pasar a dArkOS o usar `cargo zigbuild` (plan B, ya probado).
- **Imágenes Docker en el host:** `rgb20s-cross:dev` (3,2 GB) y `rgb20s-arm64:dev` (1,9 GB).
- **No confundir** la RGB20S (RK3326) con la RGB20SX (RK3566), que sí está en mainline.
