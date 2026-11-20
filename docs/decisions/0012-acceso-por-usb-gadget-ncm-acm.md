# 0012 — Acceso de desarrollo por el USB-C: gadget compuesto NCM + ACM, IP fija y DHCP

- **Fecha:** 2026-11-19
- **Estado:** aceptada (por el usuario, 2026-11-20)

## Contexto

La consola no tiene Wi-Fi utilizable. El USB-C ya funcionó como **host** (tethering de Android con ArkOS/dArkOS).
El objetivo es desarrollar "estilo adb": un solo cable a la Mac (Linux), con red y SSH.

Lo que se sabe sin la consola (todo del código fuente, nada medido):

- En el `rk3326-odroid-go.dtsi` de ROCKNIX (y en mainline para OGA y RG351M) el `otg-port` del PHY USB2 está `disabled`;
  el controlador OTG `usb20_otg` (dwc2, `ff300000`) está `okay`.
- El `rk3326.dtsi` de ROCKNIX le agrega `usb-role-switch` con `role-switch-default-mode = "host"`.
- Con role switch, dwc2 fuerza la sesión A (host) o B (periférico) escribiendo `GOTGCTL` (`drivers/usb/dwc2/drd.c`):
  no depende del pin ID ni de la detección de VBUS del PHY, y el rol se puede cambiar desde userspace
  (`allow_userspace_control = true` → `/sys/class/usb_role/*/role`).
- ROCKNIX usa dwc2 como host en el USB-C de clones parecidos (RGB10X, R36S: `vbus-supply` en `usb20_otg`) con el
  `otg-port` deshabilitado, o sea que el puerto OTG funciona sin el driver del PHY, al menos como host.
- La GameForce Chi (mainline) habilita el `otg-port`.
- La config del kernel de ROCKNIX ya trae dwc2 dual-role, configfs y NCM/ECM/RNDIS.

## Alternativas consideradas

| Opción | A favor | En contra |
|---|---|---|
| **A. Gadget compuesto NCM + ACM por configfs, rol periférico por defecto (DT) y elegible en `/boot/rgb20s.conf`** | Red y además un login serie (`/dev/ttyACM0` en la Mac) aunque la red falle. Se cambia a host editando un archivo desde la Mac | Si el USB-C está cableado al puerto host del PHY, no hay gadget posible |
| B. Solo red ECM o RNDIS | Una función, más simple | Sin plan B si falla la red |
| C. Host por defecto + adaptador USB-Ethernet o tethering | Ya se sabe que el modo host funciona | Hace falta otro aparato y otra red; no es "un cable a la Mac" |
| D. Rol automático (periférico si hay VBUS de la Mac, host si no) | Sin configurar nada | No se sabe cómo detectar VBUS en esta placa; arriesga a no tener ninguno de los dos |

## Decisión

Opción **A**:
- DTB propio con `role-switch-default-mode = "peripheral"`; `rgb20s-usb.service` escribe el rol según `USB_ROLE`
  (`device` por defecto, `host` para tethering o adaptadores).
- Gadget `1d6b:0104` con funciones `ncm.usb0` (o `ecm`/`rndis` según `USB_NET`) y `acm.usb0`; MAC fijas derivadas de `machine-id`.
- Consola en **10.55.0.1/24** con servidor DHCP de `systemd-networkd` (pool .10 a .19, sin ruta ni DNS para no
  desviar el tráfico de la Mac) y mDNS. Getty en `ttyGS0`.
- La interfaz del gadget se reconoce por driver (`g_ether`) y no por nombre, para no confundirla con un teléfono en modo host.

## Motivos

- El role switch evita depender del pin ID y de la detección de VBUS, las dos cosas más inciertas sin la consola.
- ACM da un segundo camino independiente de la red, y el login por serie sirve para diagnosticar networkd o sshd.
- Con DHCP en la consola la Mac no necesita configuración manual.

## Consecuencias

- Riesgo principal: que el USB-C esté cableado al puerto **host** del PHY (EHCI/OHCI en `ff340000`/`ff350000`). Se
  diagnostica en `/boot/logs/*/20-usb.txt` (estado del UDC) y con `journalctl -k -f` en la Mac al enchufar.
  En ese caso queda volver a `USB_ROLE=host` y usar un adaptador o el tethering.
- En modo periférico la consola no alimenta el bus: cargar y transferir por el mismo cable a la Mac debería funcionar
  **(sin verificar)**. La salida de 5 V del BOOST del RK817 está siempre encendida en el DTS de ROCKNIX, como en el firmware de stock.
- Para la Mac (Linux) hace falta `cdc_ncm` (o `cdc_ether`) y `cdc_acm`, que vienen en el kernel de Arch.

## Pendientes

- [ ] Primer arranque: ¿aparece `ff300000.usb` en `/sys/class/udc` y llega a `configured` con el cable a la Mac?
- [ ] Si no enumera: probar habilitar `otg-port` en el DTS (como la GameForce Chi)
- [ ] Probar el modo host (`USB_ROLE=host`) con el tethering del Android
- [ ] Más adelante: funciones MIDI y HID en el mismo gadget

## Fuentes

- Linux 7.1.2: `drivers/usb/dwc2/drd.c` (role switch y overrides de `GOTGCTL`), `drivers/usb/gadget/function/u_ether.c`
  (`eth_get_drvinfo` informa `g_ether`), `arch/arm64/boot/dts/rockchip/px30.dtsi` (`usb20_otg`, `phys = <&u2phy_otg>`),
  `rk3326-gameforce-chi.dts`, `rk3326-odroid-go.dtsi`, `rk3326-anbernic-rg351m.dtsi`
- [ROCKNIX: `rk3326.dtsi`, `rk3326-odroid-go.dtsi`, `rk3326-powkiddy-rgb10x.dts`, `rk3326-gameconsole-r3xs.dtsi`](https://github.com/ROCKNIX/distribution/tree/61690fa8042acf631922deb2c80e8574e96958d2/projects/ROCKNIX/devices/RK3326/linux/dts/rockchip)
- [Linux: USB gadget configfs](https://docs.kernel.org/usb/gadget_configfs.html)
- [systemd.network: `[DHCPServer]`](https://www.freedesktop.org/software/systemd/man/latest/systemd.network.html)
- [docs/hardware/inventario.md](../hardware/inventario.md) (lo que se sabe del kernel de dArkOS)
