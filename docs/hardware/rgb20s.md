# Powkiddy RGB20S — hardware

Última revisión: 2026-11-19

## Especificaciones

| Componente | Detalle |
|---|---|
| SoC | Rockchip **RK3326** (familia PX30) |
| CPU | 4× ARM **Cortex-A35**, hasta 1.5 GHz. ARMv8-A, **aarch64**, con NEON |
| GPU | **Mali-G31 MP2** (Bifrost). Driver libre Panfrost en mainline; también hay blob propietario |
| RAM | 1 GB DDR3L |
| Pantalla | IPS 3.5", 640×480 (4:3) |
| PMIC / audio | **RK817** con códec integrado, como en la Odroid Go Advance **(sin verificar en la RGB20S)** |
| Almacenamiento | Sin memoria interna. Dos microSD: la primera (TF1) arranca el sistema, la segunda (TF2) guarda datos |
| Conectividad | USB-C con OTG. **Sin Wi-Fi utilizable**: la nota de lanzamiento anuncia Wi-Fi, pero según la experiencia del usuario la consola no tiene antena. Sí se conectó a internet con el *USB tethering* de un Android (ver abajo) |
| Audio | Parlante y jack de 3.5 mm |
| Controles | D-pad, botones frontales, gatillos y dos sticks analógicos |
| Batería | 3500 mAh |

## Implicancias para el hack

- **No se puede "brickear"**: al no tener memoria interna, todo vive en la microSD. Si algo falla, se cambia la tarjeta.
- El **Cortex-A35** es el núcleo ARMv8 más chico. Rinde algo menos que un A53 a igual frecuencia. Para audio en tiempo real conviene usar C/C++ y aprovechar NEON.
- **Red por USB (confirmado por el usuario, 2026-11-19):** con el *USB tethering* de un Android, la consola tuvo internet. El teléfono actúa como gadget RNDIS/NCM y la consola como **host USB**, así que el OTG funciona en modo host y el sistema (ArkOS/dArkOS) tiene un driver de red USB (`rndis_host` o `cdc_ether`) **(sin verificar cuál)**. Por la misma razón, un adaptador USB-Ethernet probablemente también funcione **(sin verificar)**.
- **USB-C OTG**: podría funcionar como host (para teclados MIDI, audio USB o adaptadores de red) y quizás como gadget (MIDI o HID hacia un PC). El modo gadget depende del kernel y del cableado del puerto **(sin verificar)**.
- Los **sticks analógicos** sirven como controles expresivos (pitch, filtro).

## Placas hermanas (RK3326 en Linux mainline)

Sirven de referencia para el device tree y los drivers:

- Odroid Go Advance (`rk3326-odroid-go2`)
- Anbernic RG351M / RG351V (`rk3326-anbernic-rg351m`)
- GameForce Chi (`rk3326-gameforce-chi`)

Ojo: la **RGB20SX** es otra consola, con un chip distinto (RK3566).

## Pendientes

- [ ] Confirmar el códec de audio, abriendo la consola o con `dmesg` / `aplay -l`
- [ ] Confirmar si hay un chip Wi-Fi sin antena o si directamente no hay chip (`lsmod`, `dmesg`, `lsusb`)
- [ ] Identificar el driver de red USB que usa el tethering (`lsmod`, `dmesg` al conectar el teléfono)
- [ ] Confirmar si el puerto USB-C soporta modo periférico (gadget)
- [ ] Revisar `/proc/cpuinfo` y medir el rendimiento real

## Fuentes

- Experiencia del usuario con la consola (2026-11-19): sin antena Wi-Fi; internet por USB tethering de Android.

- [Notebookcheck: RGB20S revealed (Wi-Fi, 2× microSD)](https://www.notebookcheck.net/Powkiddy-RGB20S-New-retro-handheld-revealed-with-USB-Type-C-Wi-Fi-and-two-microSD-card-readers.620381.0.html)
- [handhelds.wiki: Powkiddy RGB20S](https://handhelds.wiki/Powkiddy_RGB20S)
- [CNX Software: RK3308 & RK3326 datasheets](https://www.cnx-software.com/2018/07/23/rockchip-rk3308-rk3326-datasheet/)
- [Datasheet RK3326 (PDF, LCSC)](https://wmsc.lcsc.com/wmsc/upload/file/pdf/v2/lcsc/2404251631_Rockchip-RK3326_C5182195.pdf)
- [Rockchip Roadmap 2019 (RK817)](https://www.cnx-software.com/pdf/Rockchip-Roadmap-2019-.11_mts.pdf)
- [DTS mainline rk3326-anbernic-rg351m](https://git.aurel32.net/linux.git/plain/arch/arm64/boot/dts/rockchip/rk3326-anbernic-rg351m.dtsi)
- [DTS mainline rk3326-odroid-go](https://grok.openbmc.org/xref/openbmc/linux/arch/arm64/boot/dts/rockchip/rk3326-odroid-go.dtsi?r=a5961bed)
- [Parche RGB20SX (RK3566)](https://lists.infradead.org/pipermail/linux-rockchip/2024-October/051630.html)
