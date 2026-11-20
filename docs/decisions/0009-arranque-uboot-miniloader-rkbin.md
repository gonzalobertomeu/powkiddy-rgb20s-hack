# 0009 — Arrancar con U-Boot mainline cargado por el miniloader de Rockchip, como ROCKNIX, en una tarjeta MBR

- **Fecha:** 2026-11-19
- **Estado:** aceptada (por el usuario, 2026-11-20)

## Contexto

La imagen de Arch Linux ARM necesita un bootloader para el RK3326 que funcione en la RGB20S
**en el primer intento**: no hay consola serie accesible (sin verificar), U-Boot no enciende la pantalla
y el usuario tiene una sola microSD. Cada intento fallido cuesta sacar la tarjeta, regrabarla y volver a probar.

En el RK3326 el BootROM lee el sector 64 de la SD. A partir de ahí hay dos flujos documentados
en `doc/board/rockchip/rockchip.rst` de U-Boot:

1. **TPL/SPL de U-Boot + binman:** un solo `u-boot-rockchip.bin` en el sector 64. El init de DRAM es el TPL de U-Boot
   o el blob de rkbin (`ROCKCHIP_TPL`), y BL31 sale de TF-A (plataforma `px30`) o de rkbin.
2. **Miniloader de Rockchip:** `idbloader.img` (blob de DDR + miniloader) en el sector 64, `uboot.img` en el 16384
   y `trust.img` (BL31) en el 24576. U-Boot mainline queda solo como último paso (BL33).

U-Boot mainline tiene `odroid-go2_defconfig` (Odroid Go Advance, mismo SoC), pero no la RGB20S.

## Alternativas consideradas

| Opción | A favor | En contra |
|---|---|---|
| **A. Igual que ROCKNIX:** U-Boot mainline v2025.10 con su `rk3326-handheld_defconfig` y su DT genérico, cargado por el miniloader + BL31 de rkbin | Es el bootloader que ROCKNIX usa hoy en la RGB20S (imagen "b"). Todas las piezas fijadas por versión, commit y hash. Se compila desde fuente salvo los blobs | Tres blobs cerrados (DDR, miniloader, BL31) y dos herramientas cerradas de rkbin (`loaderimage`, `trust_merger`). U-Boot 2025.10 y no la última |
| B. TPL/SPL de U-Boot (`odroid-go2_defconfig`) + TF-A libre + binman | Menos blobs (o ninguno si el TPL de U-Boot inicializa la DDR3L). Flujo moderno y documentado | Nadie lo usa en la RGB20S: hay que adaptar el DT de U-Boot (la OGA tiene flash SPI y otro cableado). Si falla el init de DRAM o el SPL, la pantalla queda negra sin ninguna pista |
| C. Copiar los sectores del bootloader de la SD actual (ArkOS/dArkOS, U-Boot 2017.09 de Hardkernel) | Probado en esta misma unidad | Binario sin fuente reproducible; usa `boot.ini` y no extlinux; depende del respaldo que todavía no existe |

## Decisión

Opción **A**: replicar el bootloader "b" de ROCKNIX para RK3326 (U-Boot v2025.10 + `rk3326-handheld_defconfig`
+ `rk3326-common-handheld.dts` + parche de ROCKNIX; DDR `rk3326_ddr_333MHz_v2.12`, miniloader `v1.40` y BL31 `v1.37`
de rkbin en el commit `3e288fe8`). Un `boot.scr` propio llama a `sysboot` sobre `/extlinux/extlinux.conf`.
La tarjeta usa **MBR**: p1 FAT32 de 256 MiB desde el sector 32768 (16 MiB), booteable; p2 ext4 con la rootfs.

## Motivos

- Minimiza lo desconocido en el primer arranque: es exactamente la cadena que ROCKNIX distribuye para la RGB20S.
- Los offsets 64 / 16384 / 24576 están en la documentación oficial de U-Boot (sección "Package the image with
  Rockchip miniloader") y coinciden con el `rkhelper` de ROCKNIX.
- MBR y no GPT: el defconfig ubica el entorno de U-Boot en `CONFIG_ENV_OFFSET=0x4000` (sectores 32 a 63), que se
  pisaría con la tabla GPT (sectores 2 a 33). ROCKNIX también usa MBR (`PARTITION_TABLE="msdos"`).
- El defconfig arranca con `bootmeth order script; bootflow scan -b`: solo ejecuta `boot.scr`, por eso hace falta.

## Consecuencias

- Blobs cerrados en la cadena de arranque (los mismos que ROCKNIX y ArkOS). Se verifican por hash de objeto git y sha256.
- `uboot.bin` ocupa exactamente hasta el sector 32768: no se puede agrandar sin mover la primera partición.
- U-Boot no muestra nada en pantalla: si se cuelga antes del kernel, la pantalla queda negra (ver riesgos en
  [arch-linux-arm.md](../firmware/arch-linux-arm.md#riesgos-del-primer-arranque)).
- Con la consola serie (`ttyS2`, 1 500 000 baudios) se vería todo, si se encuentran los pads **(sin verificar)**.

## Pendientes

- [ ] Primer arranque en la consola: confirmar que U-Boot ejecuta `boot.scr` y carga `extlinux.conf`
- [ ] Probar la opción B (TPL/SPL + TF-A) cuando haya una forma de diagnosticar (serie o un arranque A que funcione)
- [ ] Actualizar U-Boot a la versión que use ROCKNIX en el futuro (hoy fija v2025.10)

## Fuentes

- [U-Boot `doc/board/rockchip/rockchip.rst` (v2026.10)](https://github.com/u-boot/u-boot/blob/v2026.10/doc/board/rockchip/rockchip.rst)
- [U-Boot `configs/odroid-go2_defconfig`](https://github.com/u-boot/u-boot/blob/v2026.10/configs/odroid-go2_defconfig)
- [ROCKNIX: `devices/RK3326/packages/u-boot/package.mk`](https://github.com/ROCKNIX/distribution/blob/61690fa8042acf631922deb2c80e8574e96958d2/projects/ROCKNIX/devices/RK3326/packages/u-boot/package.mk)
- [ROCKNIX: `bootloader/rkhelper`](https://github.com/ROCKNIX/distribution/blob/61690fa8042acf631922deb2c80e8574e96958d2/projects/ROCKNIX/bootloader/rkhelper)
- [ROCKNIX: `devices/RK3326/bootloader/update.sh`](https://github.com/ROCKNIX/distribution/blob/61690fa8042acf631922deb2c80e8574e96958d2/projects/ROCKNIX/devices/RK3326/bootloader/update.sh) (escritura en el byte 32 KiB)
- [ROCKNIX: `config.xml`](https://github.com/ROCKNIX/distribution/blob/61690fa8042acf631922deb2c80e8574e96958d2/projects/ROCKNIX/config.xml) (la RGB20S está en la imagen "b")
- [rkbin, commit 3e288fe8](https://github.com/rockchip-linux/rkbin/tree/3e288fe814e059dd06833495f845cab04ac20a5c/bin/rk33)
