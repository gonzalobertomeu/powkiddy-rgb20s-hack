# 0015 — Splash de arranque: logo propio en el kernel y un puente DRM en userspace

- **Fecha:** 2026-11-20
- **Estado:** propuesta

## Contexto
Desde que se prende la consola hasta que aparece el launcher ([0014](0014-sesion-launcher-en-tty1.md)) la pantalla
queda negra unos 25 s (medido: 2,0 s de kernel + 24,3 s de userspace). Se quiere un splash ("KingaGroove", fondo oscuro
como el del launcher) **desde el kernel hasta el launcher**, sin perder la consola serie `ttyS2` ni los logs de
`/boot/logs/`, sin parpadeos innecesarios y sin bloquear al launcher, que tiene que ser DRM master.
Restricciones: imagen mínima de Arch Linux ARM sin initramfs, U-Boot sin driver de pantalla, panel DSI con
`DRM_FBDEV_EMULATION` y fbcon, y la sesión del launcher resetea y limpia tty1 al arrancar (`TTYReset`,
`TTYVTDisallocate`, `setterm --clear`).

## Alternativas consideradas
| Opción | A favor | En contra |
|---|---|---|
| **Kernel:** logo propio con `CONFIG_LOGO_LINUX_CLUT224_FILE` | Aparece en cuanto existe `fb0`, sin userspace; la opción existe en 7.1 sin parchear | Máximo 224 colores, P3; fbcon lo descarta con `quiet` y si el DRM se registra después de `late_initcall_sync`; tapado por cualquier redibujo de tty1 |
| Kernel: bootsplash/parches fuera de árbol | Splash completo desde el kernel | Parches a mantener sobre 7.1 |
| **Userspace: programa propio por DRM/KMS** (buffer propio en el CRTC, soltar master, salir cuando otro toma la pantalla) | No le afecta lo que pase en tty1; el launcher toma la pantalla sin pasar por negro; enciende el panel aunque fbcon no lo haya hecho; ~500 líneas de C, binario estático sin dependencias | Código propio; ventana de unos ms con master tomado; si fbcon pide la pantalla sin master (texto que hace scroll en tty1) hay que volver a ponerlo |
| Userspace: copiar un raw a `/dev/fb0` (shell) | Cero código | Lo borra el reset/clear de tty1 que hace la sesión: negro justo antes del launcher; depende del formato de `fb0`; no sirve si el panel no se enciende hasta un modeset |
| Userspace: psplash / fbsplash / Plymouth | Herramientas conocidas | psplash y el `fbsplash` de busybox dibujan en `fb0` (mismo problema) y no están en la imagen (psplash solo en AUR) **(sin verificar en los repos de ALARM)**; Plymouth trae pango, cairo y fuentes, y está pensado para arrancar desde el initramfs, que la imagen no usa |

## Decisión
Las dos capas: logo propio de 224 colores en el kernel (fbcon, 640×464 arriba, `fbcon=logo-count:1`,
`vt.global_cursor_default=0`, `loglevel=4` con `CONFIG_CONSOLE_LOGLEVEL_QUIET=3`, `systemd.show_status=auto`) y
`rgb20s-splash`, un programa en C estático que pone el splash completo por DRM/KMS al empezar userspace, suelta DRM master
y se va cuando el launcher toma la pantalla. El arte sale de un script (`make-splash.py`) que genera los dos formatos.

## Motivos
- El logo del kernel cubre el tramo sin userspace sin agregar nada al arranque; es una opción de Kconfig, no un parche.
- Por DRM el splash sobrevive al reset de tty1 que hace la sesión y el traspaso al launcher es directo (su primer cuadro
  reemplaza al buffer del splash). Con `fb0` se vería negro entre el reset y el launcher.
- C y no Rust: es una utilidad de sistema de bajo nivel (ADR [0001](0001-lenguaje-rust.md) deja C para eso), sin crates ni
  libdrm, y se compila en el mismo contenedor que el kernel (`libc6-dev-arm64-cross`), así `just img` lo incluye.
- `loglevel=4` en lugar de `quiet`: con `quiet` fbcon no dibuja el logo (`console_loglevel <= CONSOLE_LOGLEVEL_QUIET`).

## Consecuencias
- En pantalla y en `ttyS2` solo se ven errores del kernel. Para ver todo: entrada `arch-verbose` de `extlinux.conf`
  (o `ignore_loglevel`). `dmesg`, el journal y `/boot/logs/` no cambian.
- Un kernel panic después de que arrancó el splash no se ve (queda el splash en pantalla); antes, sí.
- Si el DRM de Rockchip se registra tarde (después de `late_initcall_sync`, por ejemplo por un proveedor en
  deferred probe), el logo del kernel no aparece y solo queda el splash de userspace. **(sin verificar)**
- El splash suelta master enseguida; si fbcon recupera la pantalla (scroll de texto en tty1 sin master) el splash la
  vuelve a tomar, hasta 20 veces. Puede verse un destello en ese caso.
- Archivos: `projects/linux-image/splash/`, `kernel/config.fragment`, `boot/boot.env`, `boot/extlinux.conf.in`,
  `infra/image/scripts/build-splash.sh` y `build-device-update.sh`, `infra/image/Dockerfile`.

## Pendientes
- [ ] Probar en la consola: logo del kernel (¿aparece? ¿cuándo?), splash, traspaso al launcher sin negro, `journalctl -u rgb20s-splash`.
- [ ] Averiguar por qué hoy la pantalla queda negra durante todo el arranque aun con `loglevel=7` (ver hipótesis en
      `docs/firmware/arch-linux-arm.md#splash-de-arranque`).
- [ ] Si el traspaso entre apps (launcher → groovebox) muestra la consola, evaluar dejar el splash como fondo entre apps.

## Fuentes
- Kernel 7.1.2: `drivers/video/logo/{Kconfig,logo.c}`, `drivers/video/fbdev/core/{fbcon.c,fb_logo.c}`,
  `Documentation/fb/fbcon.rst`, `drivers/gpu/drm/{drm_auth.c,drm_fb_helper.c}`.
- `sd_notify(3)`, `systemd.exec(5)`, `systemd.service(5)`.
- Prueba en QEMU (`virt` + `virtio-gpu`) descrita en `projects/linux-image/splash/README.md`.
