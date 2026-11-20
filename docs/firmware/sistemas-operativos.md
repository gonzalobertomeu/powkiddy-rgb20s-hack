# Sistemas operativos compatibles

Última revisión: 2026-11-19

## Distros para handhelds

| SO | Base | Estado | Para el hack |
|---|---|---|---|
| **dArkOS** | Debian | Activo (releases en 2026) | **Recomendado.** Tiene `apt`, compiladores y librerías: se puede desarrollar en la consola |
| **ROCKNIX** (ex JELOS) | Propia, sistema de solo lectura | Activo | Muy pulido y con SSH, pero sin gestor de paquetes. Mejor para jugar |
| **AmberELEC** | Propia | Lista la RGB20S **(sin verificar su estado en 2026)** | Misma limitación que ROCKNIX |
| **ArkOS** (original) | Ubuntu | Repositorio archivado a fines de 2025 | Se usa la imagen del RG351MP. Mejor ir a dArkOS, su sucesor del mismo autor |

PortMaster (gestor de ports) funciona en dArkOS, ROCKNIX y AmberELEC.

## Linux genérico (mainline)

- La RGB20S **no tiene device tree en mainline**. Hay un `rk3326-powkiddy-rgb20s` dentro del árbol de ROCKNIX, que se aplica como parche aparte.
- Mainline sí tiene placas hermanas muy parecidas (ver [hardware](../hardware/rgb20s.md#placas-hermanas-rk3326-en-linux-mainline)).
- Camino posible: **Arch Linux ARM, Debian o Alpine** con kernel mainline más el device tree de ROCKNIX. Hay que armar el arranque a mano (U-Boot, kernel y device tree en la SD). **Todavía no se probó.**
- Camino elegido: **Arch Linux ARM** (decisión del usuario, 2026-11-19). Investigación y build en [arch-linux-arm.md](arch-linux-arm.md).
- Variante "appliance": un sistema mínimo que arranca directo en una sola aplicación (por ejemplo el groovebox).

## Procedimiento seguro

1. **Respaldar la microSD original** antes de tocar nada:
   ```sh
   lsblk                                   # identificar la SD (ej. /dev/sdX)
   sudo dd if=/dev/sdX of=rgb20s-stock.img bs=4M status=progress conv=fsync
   zstd rgb20s-stock.img
   ```
2. Flashear el nuevo sistema en **otra** microSD, sin pisar la original.
3. Habilitar Wi-Fi y SSH, y anotar la IP o el hostname.

## Pendientes

- [ ] Identificar qué sistema trae de fábrica la unidad
- [ ] Respaldar la SD original
- [ ] Instalar dArkOS y probar Wi-Fi, SSH y audio
- [ ] Explorar el arranque con mainline usando el device tree de ROCKNIX

## Fuentes

- [dArkOS releases (GitHub)](https://github.com/christianhaitian/dArkOS/releases)
- [handhelds.wiki: Powkiddy RGB20S](https://handhelds.wiki/POWKIDDY_RGB20S)
- [retrocompass: RGB20S custom firmware](https://retrocompass.web.app/en/powkiddy/rgb20s/custom-firmware) (agregador, poco confiable)
- [tech-insider: muOS vs ArkOS vs JELOS 2026](https://tech-insider.org/muos-vs-arkos-vs-jelos-2026/) (con inconsistencias)
- [distribution: device switch RK3326 / kernel 6.7](https://git.wbrawner.com/wbrawner/distribution/commit/c79cff7994e3c643422e97772fb96f8dcba5020d)
- [PortMaster Starter Guide (Retro Game Corps)](https://retrogamecorps.com/2024/07/12/portmaster-starter-guide/)
