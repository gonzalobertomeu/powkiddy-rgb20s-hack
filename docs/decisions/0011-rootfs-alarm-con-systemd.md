# 0011 — Rootfs: Arch Linux ARM genérica con systemd y systemd-networkd, armada en contenedores

- **Fecha:** 2026-11-19
- **Estado:** aceptada (por el usuario, 2026-11-20)

## Contexto

El usuario eligió Arch Linux ARM. La imagen tiene que arrancar rápido, ser liviana (sin X ni Wayland) y, más adelante,
lanzar el groovebox en KMS. En la fase 1 basta con SSH y una consola en pantalla. Restricciones de la Mac: no usar `sudo`
ni instalar paquetes del host; sí hay Docker y `qemu-aarch64-static` con binfmt.

## Alternativas consideradas

| Opción | A favor | En contra |
|---|---|---|
| **A. systemd (el de ALARM) + systemd-networkd/resolved** | Soportado por Arch Linux ARM. `networkd` trae servidor DHCP (la Mac recibe IP sola), journal persistente, `systemd-analyze` para medir el arranque | Más pesado que un init mínimo (tiempo y RAM) |
| B. Init liviano (OpenRC, runit, busybox init) | Arranque más rápido y menos RAM | Arch Linux ARM no lo soporta: hay que mantener paquetes y unidades a mano |
| C. Rootfs con chroot y `sudo` en el host | El flujo clásico de la wiki de ALARM | Requiere root en la Mac (montar /proc, loop devices) |

Para construir: (1) `docker import` del tarball verificado como imagen arm64, (2) `pacman` dentro de un contenedor
emulado por binfmt, (3) `docker export`, (4) ext4 con `mke2fs -d`, FAT con `mtools` y particiones con `sfdisk` sobre un archivo.

## Decisión

Opción **A**, construida sin `sudo` con el flujo de contenedores descrito. Se quitan el kernel genérico, todo
`linux-firmware`, `mkinitcpio`, `netctl`, `dhcpcd`, `net-tools` y `vim`; se agregan `alsa-utils`, `usbutils`, `evtest`,
`dosfstools` y `sudo`. Sin initramfs. Usuario `alarm` (clave `alarm`, documentada), root bloqueado, SSH solo con clave si
al armar la imagen hay `infra/image/local/authorized_keys`.

## Motivos

- systemd es lo que trae ALARM: cambiarlo es mucho trabajo para una ganancia que todavía no está medida.
  El arranque se mide con `systemd-analyze` (queda en `/boot/logs/`) antes de pensar en otro init.
- `networkd` resuelve el DHCP del gadget sin paquetes extra.
- Construcción sin root en el host, reproducible por script y con insumos verificados (md5 + firma GPG de ALARM).

## Consecuencias

- Se deshabilitan `systemd-networkd-wait-online` (demoraría hasta 2 min sin red) y `systemd-timesyncd` (no hay internet),
  y se enmascara `systemd-firstboot` (podría pedir datos en tty1 y la consola no tiene teclado).
- El tarball "latest" de ALARM cambia sin aviso y los paquetes salen del repositorio del día: la imagen es reproducible
  en procedimiento, no bit a bit. La lista exacta de paquetes queda en `infra/build/out/rootfs/packages.txt`.
- Sin initramfs, todo lo necesario para montar la raíz (MMC, ext4) tiene que estar built-in (lo está en la config).

## Pendientes

- [ ] Medir tiempo de arranque y RAM en la consola
- [ ] Adelgazar más (paquetes de `base` que no se usan) con la medición en la mano
- [ ] Servicio del groovebox (KMS/DRM, `SDL_VIDEODRIVER=kmsdrm`) cuando exista el binario para Arch: requiere `mesa` y `sdl2`

## Fuentes

- [Arch Linux ARM: firma de paquetes](https://archlinuxarm.org/about/package-signing)
- [Arch Linux ARM: tarball genérico aarch64](http://os.archlinuxarm.org/os/ArchLinuxARM-aarch64-latest.tar.gz)
- [systemd.network: sección `[DHCPServer]`](https://www.freedesktop.org/software/systemd/man/latest/systemd.network.html)
- [mke2fs(8): opción `-d`](https://man7.org/linux/man-pages/man8/mke2fs.8.html)
