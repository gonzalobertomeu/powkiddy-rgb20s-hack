# splash — pantalla de arranque "KingaGroove"

Reemplaza la pantalla negra del arranque por un splash, desde el kernel hasta que aparece el launcher.
Decisión y alternativas: [ADR 0015](../../../docs/decisions/0015-splash-de-arranque-logo-del-kernel-y-drm.md) (propuesta).
Contexto del arranque: [`docs/firmware/arch-linux-arm.md`](../../../docs/firmware/arch-linux-arm.md#splash-de-arranque).

![splash](generated/splash.png)

## Cómo funciona

| Tramo | Qué se ve | Quién lo dibuja |
|---|---|---|
| BootROM → U-Boot | negro (U-Boot no enciende el panel) | — |
| Kernel, desde que el DRM de Rockchip registra `fb0` | logo propio (640×464, arriba) + una fila de texto abajo, solo para errores | fbcon (`CONFIG_LOGO_LINUX_CLUT224_FILE`) |
| systemd, desde el primer segundo de userspace | el splash completo (640×480) | `rgb20s-splash.service` (DRM/KMS) |
| Launcher | el launcher | `rgb20s-session` (SDL2/KMSDRM) |

`rgb20s-splash` (C, estático, sin libdrm) crea un buffer en la memoria de video, lo pinta con
`splash.ppm`, lo pone en pantalla y **suelta DRM master**: cuando el launcher abre la tarjeta, queda como
master y su primer cuadro reemplaza al splash sin pasar por negro. El splash se da cuenta (el framebuffer
del CRTC cambió y hay otro master) y sale. Si lo que cambió fue fbcon (texto en tty1, sin master), vuelve a
poner el splash. Tiempo máximo: 60 s. Si tty1 tiene el login de texto en vez de la sesión, el drop-in de
`getty@tty1` lo saca cuando aparece el login.

Por qué DRM y no escribir en `/dev/fb0`: el wrapper de la sesión y systemd limpian tty1 antes de abrir el
launcher (`TTYReset`, `setterm --clear`), y eso borra lo que haya en `fb0`: se vería negro hasta el primer
cuadro del launcher. Un buffer propio en el CRTC no se ve afectado. Detalle en el ADR.

## Archivos

| Archivo | Qué es |
|---|---|
| `make-splash.py` | **Fuente del arte.** Python sin dependencias: rasteriza Spleen (BDF de `projects/launcher/fonts/`). Textos, colores, fuentes, escalas y el patrón de 16 pasos están en `CONFIG` |
| `generated/splash.png` | referencia 640×480 (va al repo) |
| `generated/splash.ppm` | P6 640×480: lo que dibuja `rgb20s-splash` (se regenera; no va al repo) |
| `generated/logo_rgb20s_clut224.ppm` | P3 640×464, 5 colores (máx. 224): el logo del kernel (se regenera; no va al repo) |
| `rgb20s-splash.c` | el programa de userspace |
| `system/rgb20s-splash.service` | unidad: `DefaultDependencies=no`, `Type=notify`, `WantedBy=sysinit.target`, antes de `rgb20s-session` y `getty@tty1` |
| `system/getty-tty1-splash.conf` | drop-in de `getty@tty1`: detiene el splash al aparecer el login |
| `install-on-device.sh` | instala todo en una consola en uso, sin reflashear |
| `rollback-on-device.sh` | lo deshace (por SSH) |
| `qemu-test/` | prueba de `rgb20s-splash` en QEMU (`virtio-gpu`), sin la consola |

## Cambiar el arte

```sh
$EDITOR projects/linux-image/splash/make-splash.py     # CONFIG: textos, colores, fuentes...
python3 projects/linux-image/splash/make-splash.py     # mirar generated/splash.png
just img-splash img-kernel img-device-update          # o `just img` para una imagen nueva
```

O por línea de comandos: `make-splash.py --title-a Kinga --title-b Groove --subtitle ''`.
Reglas: el contenido tiene que entrar en los primeros 464 px de alto (es lo que muestra el kernel; el script
avisa si no) y el logo no puede pasar de 224 colores (no hay suavizado, así que sobra margen).
Para cambiar **solo** la imagen de userspace en la consola alcanza con reemplazar
`/usr/local/share/rgb20s/splash.ppm` por otro P6 (cualquier tamaño hasta 640×480: se centra sobre el color
del píxel de arriba a la izquierda).

## Línea de comandos del kernel

Está en [`boot/boot.env`](../boot/boot.env) (la usan la imagen y el paquete de actualización):

```
... console=ttyS2,1500000n8 console=tty0 consoleblank=0 panic=0
    loglevel=4 vt.global_cursor_default=0 fbcon=logo-count:1 systemd.show_status=auto udev.log_level=3
```

- `loglevel=4` y no `quiet`: fbcon no dibuja el logo si el nivel de consola es `<= CONFIG_CONSOLE_LOGLEVEL_QUIET`
  (ahora 3). `quiet` lo bajaría a 3 y el logo desaparecería.
- La consola serie `ttyS2` sigue activa (login y mensajes), pero con el mismo filtro: solo errores. `dmesg`,
  el journal y `/boot/logs/` siguen teniendo todo.
- Entrada `arch-verbose` en `extlinux.conf`: la línea de antes (`loglevel=7 systemd.show_status=1`), sin logo.

## Instalar en la consola en uso (sin reflashear)

En la Mac:

```sh
just img-splash img-kernel img-device-update          # arma infra/build/out/device-update/ (~30 MB)
scp -r infra/build/out/device-update rgb20s:/home/alarm/
```

En la consola (el usuario, con sudo; por SSH):

```sh
cd ~/device-update
sudo ./install-on-device.sh --dry-run    # muestra qué haría, no toca nada
sudo ./install-on-device.sh
sudo reboot
```

`install-on-device.sh` verifica `SHA256SUMS` y que sea una RGB20S; si el `Image` cambia, respalda en
`/boot/backup-<fecha>/` el `Image`, los DTB, `extlinux.conf` y la línea de comandos actual; instala el `Image`,
los DTB que cambien y los módulos si difieren (los viejos van a `/var/backups/rgb20s/`); escribe el
`extlinux.conf` nuevo con una entrada **`arch-prev`** que arranca el kernel respaldado con la línea de
comandos anterior; instala y habilita `rgb20s-splash.service` (no lo arranca: el launcher tiene la pantalla).
`DEFAULT` queda en `arch`. Correrlo dos veces no hace nada nuevo.

Después del reinicio, por SSH:

```sh
journalctl -b -u rgb20s-splash -o short-monotonic   # "splash en pantalla a los ...", "otra app tomó la pantalla"
dmesg | grep -iE 'fbcon|Console: switching|logo|rockchip-drm'
systemd-analyze critical-chain rgb20s-session.service
```

## Qué se verificó (2026-11-20) y qué no

| Qué | Cómo | Resultado |
|---|---|---|
| Arte reproducible | `make-splash.py` en la Mac (Python 3.14) y en `rgb20s-image-builder` (3.13) | Salidas idénticas byte a byte |
| Logo en el kernel | `just img-kernel`: `pnmtologo` generó `logo_linux_clut224.c` de 640×464 y 5 colores; `CONFIG_LOGO*` y `CONSOLE_LOGLEVEL_QUIET=3` en el `.config`; `logo_linux_clut224` en `System.map`; la paleta, dentro del `Image` | OK |
| Imagen | `just img-assemble && just img-check` (chequeos nuevos: entradas de `extlinux`, línea de comandos, `ttyS2` en todas, binario, PPM, unidad habilitada, drop-in, logo) | 48/48 |
| `rgb20s-splash` en QEMU | [`qemu-test/run-qemu-test.sh`](qemu-test/run-qemu-test.sh): ALARM genérico, `virtio-gpu` 640×480, la unidad real, un "launcher falso" (`rgb20s-splash -m`) que toma master a los 12 s | El splash sale apenas aparece `card0` (antes, la pantalla estaba apagada); `getty@tty1` escribe en tty1 y el splash sigue en pantalla; la app falsa toma la pantalla y el splash sale 80 ms después, sin cuadros negros en las capturas (cada 2 s); la unidad termina con `Result=success` |
| `install-on-device.sh` / `rollback-on-device.sh` | En un contenedor con un `/boot` simulado (Image falso, `extlinux.conf` viejo), `systemctl` de mentira y `RGB20S_ASSUME_RGB20S=1` | Respaldo, `arch-prev`, módulos, idempotencia (la segunda corrida no cambia nada) y rollback completo |
| **En la consola** | — | **Nada.** Falta: si el logo del kernel aparece (depende de cuándo se registra `fb0`), el modeset del panel DSI desde el splash, el traspaso real a SDL2/KMSDRM, los tiempos |

## Volver atrás

| Situación | Qué hacer |
|---|---|
| Arranca, pero el splash molesta | `sudo systemctl disable --now rgb20s-splash` |
| Arranca, pero se quiere todo como antes | `sudo ~/device-update/rollback-on-device.sh && sudo reboot` |
| **No arranca** (o pantalla rara) | SD a la Mac, montar `RGB20S_BOOT` y en `extlinux/extlinux.conf` cambiar `DEFAULT arch` por `DEFAULT arch-prev` (kernel y línea de antes). Alternativa: copiar `backup-<fecha>/Image` sobre `/Image` y `backup-<fecha>/extlinux/extlinux.conf` sobre `extlinux/extlinux.conf` |
| Arranca pero sin ver nada útil | `DEFAULT arch-verbose`: mismo kernel, todos los mensajes |

Ojo al editar `extlinux.conf` en la Mac: sin espacios al final de las líneas (el parser de U-Boot es estricto).
Con `arch-prev` el kernel viejo usa los módulos nuevos (mismo `7.1.2-rgb20s`); solo cambian si
`install-on-device.sh` lo dijo, y `rollback-on-device.sh` los devuelve.

## Fuentes

- Kernel 7.1.2: `drivers/video/logo/Kconfig` (`LOGO_LINUX_CLUT224_FILE`), `drivers/video/logo/logo.c`
  (los logos se liberan en `late_initcall_sync`), `drivers/video/fbdev/core/fbcon.c` (logo desactivado con
  `console_loglevel <= CONSOLE_LOGLEVEL_QUIET`, opciones `logo-count`/`logo-pos`, descarte si el logo no entra),
  `Documentation/fb/fbcon.rst`, `drivers/gpu/drm/drm_auth.c` (el primero que abre la tarjeta sin master queda
  como master; `DROP_MASTER`), `drivers/gpu/drm/drm_fb_helper.c` (fbdev no toca la pantalla si hay master;
  `KD_TEXT` fuerza la restauración).
- `sd_notify(3)` (protocolo de `NOTIFY_SOCKET`), `systemd.exec(5)` (`TTYReset`, `TTYVTDisallocate`).
- Fuente Spleen 2.3.0 (BSD-2-Clause), copia en `projects/launcher/fonts/`.
