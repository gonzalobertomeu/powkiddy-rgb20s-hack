# Blueprint — estado y próximos pasos

Última actualización: 2026-11-21 (fin de la sesión 2)

Lo que **no** está en otros docs: plan de trabajo, pendientes del usuario y notas de la sesión.
Para los datos técnicos, ver `README.md` y `docs/`.

## Estado actual

- ✅ Investigación inicial: hardware, sistemas operativos y emulación (`docs/`)
- ✅ Decisión: Rust para las apps ([ADR 0001](docs/decisions/0001-lenguaje-rust.md))
- ✅ 6 agentes con modelo asignado (opus: `rust-dev`, `dsp-engineer`, `alarm-image-builder`; sonnet: `dev-infra`, `device-ops`, `groovebox-designer`), 9 skills y `CLAUDE.md` con delegación automática, reportes breves y verificación con `just check`
- ✅ `infra/` y `justfile` ([ADR 0008](docs/decisions/0008-compilacion-cruzada-contenedor-x86-multiarch.md)). `just check` = fmt, clippy, tests, build aarch64 y glibc con una línea por paso. Ver `infra/README.md`
- ✅ **La consola corre nuestra imagen de Arch Linux ARM** (kernel 7.1.2-rgb20s) y se accede por USB gadget: `ssh rgb20s` (10.55.0.1, usuario `alarm`, clave `~/.ssh/powkiddy`, entrada en `~/.ssh/config`). Consola serie en `/dev/ttyACM0`. Sin RTC: el reloj arranca atrasado (10-nov)
- ✅ **Deploy por SSH:** `just deploy` (o `APP=launcher|rgb20s-audio just deploy`) compila y copia a `/home/alarm/<APP>/`; usa `scp` si la consola no tiene `rsync` y reemplaza el binario aunque esté corriendo
- ✅ **Arranque:** splash "KingaGroove" (logo del kernel + `rgb20s-splash` por DRM, [ADR 0015](docs/decisions/0015-splash-de-arranque-logo-del-kernel-y-drm.md)) → **launcher** en tty1 en lugar del login (`rgb20s-session`, [ADR 0014](docs/decisions/0014-sesion-launcher-en-tty1.md)): lista de `apps.toml` + Reiniciar/Apagar, vuelve al salir de la app. Instalados y probados en la consola
- ✅ **Audio del sistema:** servicio `rgb20s-audio` ([ADR 0016](docs/decisions/0016-servicio-de-audio-del-sistema.md)): botones de volumen (pasos de 3 dB), Mux fijo en HP, mono/estéreo con los dos botones a la vez (mezcla en ALSA), estado en `/run/rgb20s-audio/state`. Indicador de volumen en el launcher (barra) y en el título del groovebox (con batería). Instalado y probado
- ✅ **Mapeo de botones Nintendo** verificado: se intercambia solo A↔B (`RGB20S_PAD_LAYOUT`)
- ✅ Groovebox H0–H6, H4 y H10 corriendo en la consola (H4 y H10 probados por el usuario el 2026-11-21)
- Hallazgos de hardware del audio (detalle en `docs/hardware/inventario.md`): el parlante cuelga del canal L de la salida HP; `Playback Mux`=SPK no suena; el jack corta el parlante físicamente; la detección del jack (GPIO2_C6) nunca cambia. DTB sin detección y diagnóstico preparados ([ADR 0017](docs/decisions/0017-sin-deteccion-de-jack-en-simple-audio-card.md)), **sin instalar**

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
- ~~Pendiente del H5: X en Canción y Cadena toca solo la primera nota~~: resuelto en el H4 (vista previa "una vez" en el motor).
- ✅ H6 (**MVP mínimo: "un beat de 4 compases guardado"**): proyecto en RON (`ron` 0.12, formato 1, disperso, versionado, parámetros que faltan = valor por defecto del modelo), guardado atómico con `.bak` (y recuperación ofrecida si `proyecto.ron` no se lee), `config.ron` con el último proyecto y los ajustes. Proyecto: GUARDAR, GUARDAR COMO y renombrar con el editor de nombres de la spec, CARGAR con la lista de proyectos, NUEVO, confirmación con cambios sin guardar (Start guarda y sigue, A descarta, B cancela), `*` en el título. Carpeta: `GROOVEBOX_DATA` o `~/.local/share/groovebox` (en la consola, `/home/alarm/...`, sin verificar). 168 tests; capturas con `scripts/demo-h6.txt` y `demo-h6-bak.txt`. Probado en la consola.
- Pendientes del H6: autoguardado (v1), conservar en `config.ron` los campos que la app todavía no conoce (audio, input) cuando se agreguen, medir cuánto tarda GUARDAR en la SD y probar un corte de energía justo después de guardar (device-ops). Sigue abierta la pregunta de la segunda microSD (spec-formatos, pregunta 1): bastaría con apuntar `GROOVEBOX_DATA` a su punto de montaje.
- ✅ H4 (2026-11-20, hecho después del H6): **swing** global 50–75 % con precisión de muestra y sin deriva (`SetSwing`, conectado al RON), **tempo y swing editables en Proyecto**, **voces por pista** editables en Instrumento (por ahora el único parámetro de esa pantalla) con notas largas superpuestas y robo de la más vieja de la pista (al bajar el límite en vivo se roban todas las que sobran), y **X en Canción y Cadena** toca la frase entera una vez (`PreviewPhrase`, reloj propio). 186 tests. Demo: `groovebox render` → `h4-swing-voces.wav` (recto, swing 66 %, SINTE en 1 voz; idéntico nativo y aarch64 emulado) y capturas con `scripts/demo-h4.txt`. Bench sin cambios apreciables. Probado en la consola (2026-11-21).
- ✅ H10 (2026-11-20): **selección, copiar, cortar y pegar** en Grilla (celda → pista → sección), Frase (celda → columna → frase), Canción (celda → fila → bloque), Cadena (posiciones → cadena) e Instrumento. Y abre, la cruz extiende, A copia, X corta, B cancela, Select + A pega; portapapeles único con tipo (un tipo incompatible da un aviso). Pasos por valor; celdas y entradas por referencia (cortar quita referencias, no borra cadenas ni frases); pegar crea lo que falte (frase, cadena, fila). Sincroniza con el motor por `EngineMirror` y marca cambios sin guardar. Lógica en `src/selection.rs` (208 tests en total). Capturas con `scripts/demo-h10.txt`. Desvíos en el README del groovebox. Probado en la consola (2026-11-21); deshacer y A + cruz sobre la selección quedan para v1.
- ✅ H9 (2026-11-21, hecho antes que el H8): **mezcla estéreo** (volumen y paneo de potencia constante, suavizados por bloque; mute/solo siguen en el secuenciador), **envíos** a un **delay** estéreo sincronizado al tempo (ping-pong, 2^18 muestras) y a una **reverb** FDN de 4 líneas, **limitador** de picos sin anticipación delante del soft clip ([ADR 0018](docs/decisions/0018-limitador-sin-anticipacion-antes-del-soft-clip.md), **propuesta**: falta que el usuario la acepte), **pantalla Mezcla** según spec-ui (medidores, voces, efectos y master; B solo no sale, se vuelve con Select + ↑) y **sticks** en vivo (izquierdo: filtro de la pista, o filtro de bus en percusión; derecho: envíos). `fx` y `master` en el RON sin subir la versión. 236 tests; `h9-mezcla.wav` y capturas con `scripts/demo-h9.txt`. CPU en la Mac: FX suman ~30 % al peor caso (delay 39, reverb 61, limitador 14 ns/frame); estimado en el A35 ~1–1,5 % de un núcleo. Falta medir y escuchar en la consola (README del groovebox, "Pendientes para la consola"). `h3-beat` y `h5-cancion` cambiaron por el limitador.
- Pendientes del H4: pantalla Instrumento completa (H8); el swing no se ve en la Grilla (los pasos se dibujan en la grilla recta).
- Siguiente: **H8** (sustractivo polifónico, acordes y pantalla Instrumento; recomendado) o H7 (sampler y navegador de archivos). Después H11.
- Indicador de volumen en el título: con 8×16 en la Frase ya no entran el nombre del proyecto (con su `*`) ni la octava; revisar el orden de recorte si molesta.
- Al cambiar mono/estéreo con algo sonando, el groovebox reabre el audio y la reproducción vuelve a empezar (corte breve).
- Pendiente: licencias de los crates de Rust en el deploy (¿`cargo-about`?).

### Imagen de Arch Linux ARM (2026-11-19 a 21)
- ✅ Imagen armada (`just img`), flasheada y arrancando en la consola. ADR 0009 a 0012 aceptados. Splash instalado por SSH sin reflashear (`infra/build/out/device-update/`, con respaldo y entrada `arch-prev` en extlinux).
- **Pendiente: imagen completa reproducible.** La rootfs (19-nov) no trae SDL2 (`sdl2-compat`), `rsync`, launcher, groovebox, `rgb20s-session`, `rgb20s-audio` ni la clave `powkiddy.pub` (`infra/image/local/` vacía → SSH con contraseña). Si se reflashea hoy, se pierde todo eso. Tarea para `alarm-image-builder`. **Decisión del usuario (2026-11-21): se deja para el cierre del MVP (H11).** Hasta entonces no reflashear ni instalar DTB, kernel o imagen armados antes del merge `8d51d94` (no tienen `&u2phy_otg` y cortan el SSH por USB).
- Pendiente opcional: diagnóstico del jack con el usuario (`just img-dtb-update`, `projects/linux-image/jack/README.md`) e instalación del DTB sin detección.
- `rgb20s-bootlog.service` infla `systemd-analyze` (sleep 10 + volcado a la FAT, después del launcher): `BOOT_LOGS=no` cuando la imagen esté estable.
- Inventario completo pendiente (`infra/device/inventory.sh` adaptado a Arch) y medición de latencia de audio y CPU (`groovebox bench`) en la consola: parte del H11.
- **Primer arranque (2026-11-20, otra sesión):**
  - Respaldo de ArkOS en `~/rgb20s-backup/` (`tf1-arkos.img`, 16 361 455 616 bytes, sha256 `ff4034f7…7506533`, sin comparar contra la tarjeta; y `boot-files/`). La imagen grabada vino de Omarchy (`rgb20s-arch.img`, sha256 `1a782b5f…4214f1dd9`).
  - Riesgo 8 confirmado y resuelto: el DTS de ROCKNIX deja `otg-port` del PHY en `disabled` (`Requested PHY is disabled`, UDC `not attached`). Se agregó `&u2phy_otg { status = "okay"; }` al DTS del repo; en la SD se parcheó solo el DTB (el original quedó como `.dtb.orig`). **Falta recompilar la imagen** para que lo traiga.
  - El gadget **no enumera con cable USB-C a USB-C** (la Mac no entrega VBUS: el cargador marca `ONLINE=0`; probablemente faltan resistencias en CC del puerto OTG, sin verificar). **Con cable USB-A a USB-C funciona.** El puerto marcado OTG es el de datos; el DC solo carga.
  - Internet en la consola sin tocar la red de la Mac: túnel SOCKS inverso, `ssh -R 1080 …` y `env all_proxy=socks5h://127.0.0.1:1080 pacman …`. Antes, poner la hora (`sudo date -u -s`) porque no hay RTC.
  - Bugs de la imagen encontrados: (1) faltaba el llavero de pacman (`build-image.sh` lo borra y nadie lo recreaba): arreglado en `firstboot.sh`, sin probar en imagen nueva; (2) el kernel no trae Landlock y pacman 7 falla con su sandbox: por ahora `--disable-sandbox` (lo corre el usuario); **pendiente** `CONFIG_SECURITY_LANDLOCK=y` y agregarlo a `CONFIG_LSM`. `sudo` pide la contraseña de `alarm`.

### Más adelante
- Explorar USB gadget (MIDI o HID) además de red.
- Auriculares USB (HyperX Cloud III): el dongle es UAC estándar y el kernel ya lo soporta, pero el USB-C probablemente no da 5 V en modo host (falta `vbus-supply` en el DTS) y comparte puerto con el gadget de SSH. Nunca poner el puerto en host con la Mac conectada. Detalle en `docs/hardware/inventario.md`.
- Ajustar el contenedor de build a librerías de Arch ARM si hace falta (hoy usa Debian trixie: los binarios piden glibc 2.34, compatibles con Arch).

## Pendientes del usuario

- [ ] Decidir si se agrega una regla `opaque` de Hyprland para la ventana `rgb20s-sim` (hoy se ve translúcida por la opacidad por defecto de Omarchy)
- [x] Instalar `just`
- [ ] (Opcional, solo para `just qemu-boot`) `sudo pacman -S qemu-system-aarch64`
- [x] Aceptar el ADR 0008 (compilación cruzada)
- [x] Aceptar los ADR 0004, 0005 y 0009 a 0012 (2026-11-20)
- [x] Respaldar la SD de ArkOS y flashear `rgb20s-arch.img` (2026-11-20)
- [ ] Revisar y aceptar (o cambiar) los ADR 0014 a 0017, en estado *propuesta*
- [ ] (Opcional) Instalar `rsync` en la consola (`sudo pacman -S rsync`; necesita internet) o esperar a la imagen completa
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

## Notas de la sesión

- **Delegación:** el `description` de los agentes sirve como trigger, pero en este entorno la delegación automática necesita la autorización explícita que está en `CLAUDE.md`. Si los agentes no se usan solos, revisar eso primero.
- **Datos no verificados en las skills** (escritos de memoria, a confirmar antes de usarlos): offsets de arranque del RK3326 (sector 64, particiones desde 16 MiB), nombres de opciones del kernel, API exacta de `cpal`, mecanismo de flush-to-zero en Rust para aarch64.
- **Herramientas en la Mac:** `qemu-aarch64-static`, binfmt, Docker y `cargo` 1.99 instalados. Faltan `qemu-system-aarch64` y `aarch64-linux-gnu-gcc`.
- **glibc:** los binarios piden 2.34 (servicio de audio 2.39); la consola con Arch tiene 2.43. `just check` lo verifica contra `CONSOLE_GLIBC_MAX`.
- **Imágenes Docker en el host:** `rgb20s-cross:dev` (3,2 GB) y `rgb20s-arm64:dev` (1,9 GB).
- **No confundir** la RGB20S (RK3326) con la RGB20SX (RK3566), que sí está en mainline.
- **Sesión 2 (2026-11-20/21):** SSH y deploy por USB, launcher, splash, H6, H4, H10, mapeo Nintendo, servicio de audio con indicador de volumen, diagnóstico del jack. Los instaladores que necesitan `sudo` en la consola los corre el usuario (`ssh -t rgb20s 'sudo .../install.sh'`): el clasificador de permisos no deja usar la contraseña por defecto desde acá.
- **Instalar algo nuevo en la consola** = `just deploy` + `scp -r projects/<app>/system rgb20s:/home/alarm/<app>/` + el usuario corre `install.sh` con sudo. Para que el launcher tome un binario nuevo: `ssh rgb20s pkill -x launcher` (el wrapper lo relanza).
- **Decisión del usuario (2026-11-21):** en modo canción, una pista con la celda de inicio vacía no suena (regla 4 de spec-motor); se mantiene así, sin cambios. Pendiente sin commitear: H9 (mezcla) en master y la rama `worktree-agent-ae3067cb4b84e95f5` (`ece00b7`, indicador de modo y ayudas de Start) por mergear.
