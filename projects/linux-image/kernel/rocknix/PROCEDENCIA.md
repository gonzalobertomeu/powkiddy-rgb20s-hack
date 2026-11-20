# Procedencia de los archivos de ROCKNIX

Copiados sin cambios de [ROCKNIX/distribution](https://github.com/ROCKNIX/distribution), commit
`61690fa8042acf631922deb2c80e8574e96958d2` (rama `next`, 2026-11-19). Licencia GPL-2.0 (los DTS, GPL-2.0+ o MIT).
Cada línea: hash de objeto git (`git hash-object <archivo>`, igual al de GitHub) y ruta.

| Archivo local | Ruta en ROCKNIX |
|---|---|
| `kernel/rocknix/dts/*` | `projects/ROCKNIX/devices/RK3326/linux/dts/rockchip/` |
| `kernel/rocknix/patches/*` | `projects/ROCKNIX/devices/RK3326/patches/linux/` (salvo `mainline-0004-*`) |
| `kernel/rocknix/patches/mainline-0004-*.patch` | `projects/ROCKNIX/packages/linux/patches/mainline/0004-input-adc-keys-redirect-keycode-316-to-rocknix-joypa.patch` |
| `kernel/rocknix/drivers/panel-generic-dsi.c` | `projects/ROCKNIX/packages/linux-drivers/generic-dsi/sources/` |
| `kernel/rocknix/linux.aarch64.conf` | `projects/ROCKNIX/devices/RK3326/linux/linux.aarch64.conf` |
| `uboot/rocknix/sources/*`, `uboot/rocknix/patches/*` | `projects/ROCKNIX/devices/RK3326/packages/u-boot/` |
| `uboot/rocknix/b_boot.ini.reference` | `projects/ROCKNIX/devices/RK3326/packages/u-boot/config/b_boot.ini` |

```
8af36ef4064fe18e30d44a2c8e0bcce06fb5f622  kernel/rocknix/drivers/panel-generic-dsi.c
dc06f1c7423325dbf4e1e5c3d1b5d293aa21a934  kernel/rocknix/dts/rk3326.dtsi
483c15354af80f7d1c81fcc4e71544b56ed5efff  kernel/rocknix/dts/rk3326-odroid-go.dtsi
20a579ba1a7f6b7f6196c376cf9c11d8e1562c83  kernel/rocknix/dts/rk3326-powkiddy-rgb20s.dts
08d3a380f9842622579bdb164ca853ad8961d325  kernel/rocknix/linux.aarch64.conf
63485303579799ff70ed8692d7674e7a58a4b908  kernel/rocknix/patches/0008-mmc-sd-retry-a-failed-resume-after-a-power-cycle.patch
02bdbdc8f9745ec0fd157f4214c43e4ba1d07396  kernel/rocknix/patches/003-usb-dwc2-gadget-fix-null-deref-on-register-restore.patch
d3af8532f75bc79ebd7564f5b2f74360241655bd  kernel/rocknix/patches/004-usb-dwc2-skip-pm-callbacks-after-failed-core-reset.patch
8dc800258cd7ece1720a4b47e7d38c94b418e510  kernel/rocknix/patches/010-dwc2-track-vbus-supply-state.patch
48676ae47bf0d7975f766c8111c24ec960fd3be7  kernel/rocknix/patches/035-dwc2-release-vbus-supply-on-role-switch.patch
62a34b72f546734790e92d9fa5392a62df6dba4c  kernel/rocknix/patches/038-dwc2-no-pullup-on-id-change-without-gadget.patch
6cdd6bced2d98c6ae73b4b0f130d5f7871a18727  kernel/rocknix/patches/mainline-0004-input-adc-keys-redirect-keycode-316-to-rocknix-joypad.patch
6436c6c9922af66451e376729dad79f5f2f9f859  uboot/rocknix/b_boot.ini.reference
3d73d62fe95ebf3bc330044d9c0330546a4fcab6  uboot/rocknix/patches/0001-add-support-for-k36-clone-with-emmc-rk3326.patch
b25e54d7bb74683d0ce1fe6d2fe19b4d4ad0af2a  uboot/rocknix/sources/arch/arm/dts/rk3326-odroid-go2-emmc.dtsi
b2bd0be717c428d4ebf3e950a3c9ee3d0b48ac16  uboot/rocknix/sources/arch/arm/dts/rk3326-odroid-go2-uart5.dtsi
fa4fb13982108f71658c36340d3e9439d9e24729  uboot/rocknix/sources/configs/rk3326-handheld_defconfig
da74a0a651f4492f68ef0e7f6433b31c0e2ebe96  uboot/rocknix/sources/dts/upstream/src/arm64/rockchip/rk3326-common-handheld.dts
```
