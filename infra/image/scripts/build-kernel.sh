#!/usr/bin/env bash
# Compila el kernel (mainline + parches/DTS/driver de panel de ROCKNIX), los DTB de la RGB20S
# y el módulo externo del joypad. Corre DENTRO de rgb20s-image-builder (in-builder.sh).
# Salida: infra/build/out/kernel/
set -euo pipefail
cd /work
. infra/image/sources.lock
li=projects/linux-image/kernel
b=infra/build/kernel
out=infra/build/out/kernel
src="$b/linux-$LINUX_VERSION"
jobs="${JOBS:-$(nproc)}"
log() { printf '\033[1;32m[kernel]\033[0m %s\n' "$*"; }

# 1. Fuentes limpias + parches (se rehace si cambió algún insumo)
stamp="$(cat infra/image/sources.lock "$li"/rocknix/patches/*.patch "$li"/rocknix/drivers/* | sha256sum | cut -c1-16)"
if [ "$(cat "$src/.rgb20s-stamp" 2>/dev/null)" != "$stamp" ]; then
  log "extrayendo linux-$LINUX_VERSION"
  rm -rf "$src"; mkdir -p "$b"
  tar -xJf "infra/downloads/linux/linux-$LINUX_VERSION.tar.xz" -C "$b"
  for p in $(ls "$li"/rocknix/patches/*.patch | sort); do
    log "parche $(basename "$p")"
    patch -d "$src" -p1 --forward --no-backup-if-mismatch < "$p"
  done
  # Driver de panel genérico de ROCKNIX, integrado como lo hace ROCKNIX (obj-y)
  cp "$li/rocknix/drivers/panel-generic-dsi.c" "$src/drivers/gpu/drm/panel/"
  echo 'obj-y += panel-generic-dsi.o' >> "$src/drivers/gpu/drm/panel/Makefile"
  echo "$stamp" > "$src/.rgb20s-stamp"
fi

# 2. Device trees: los de ROCKNIX reemplazan rk3326.dtsi y rk3326-odroid-go.dtsi de mainline
#    (igual que ROCKNIX); solo se compilan los DTB de la RGB20S.
dts_dir="$src/arch/arm64/boot/dts/rockchip"
cp "$li"/rocknix/dts/* "$li"/dts/* "$dts_dir/"
grep -q 'rk3326-powkiddy-rgb20s-alarm.dtb' "$dts_dir/Makefile" || cat >> "$dts_dir/Makefile" <<'EOF'
dtb-$(CONFIG_ARCH_ROCKCHIP) += rk3326-powkiddy-rgb20s.dtb
dtb-$(CONFIG_ARCH_ROCKCHIP) += rk3326-powkiddy-rgb20s-alarm.dtb
EOF

# 3. Config: base de ROCKNIX + fragmento propio
cp "$li/rocknix/linux.aarch64.conf" "$src/.config"
( cd "$src" && ./scripts/kconfig/merge_config.sh -m .config "/work/$li/config.fragment" >/dev/null \
  && make -s olddefconfig )
fail=0
while IFS= read -r line; do
  case "$line" in
    '') continue ;;
    '# CONFIG_'*' is not set') ;;   # también se verifica
    '#'*) continue ;;               # comentario
  esac
  grep -qxF -- "$line" "$src/.config" || { echo "  no quedó aplicado: $line" >&2; fail=1; }
done < "$li/config.fragment"
[ "$fail" = 0 ] || { echo "el fragmento no se aplicó completo (dependencias de Kconfig)" >&2; exit 1; }

# 4. Compilar
log "compilando Image, módulos y DTB con $jobs hilos"
make -C "$src" -j"$jobs" Image modules \
  rockchip/rk3326-powkiddy-rgb20s.dtb rockchip/rk3326-powkiddy-rgb20s-alarm.dtb
krel="$(make -C "$src" -s kernelrelease)"

# 5. Módulo externo del joypad (rocknix-singleadc-joypad)
jp="$b/rocknix-joypad"
rm -rf "$jp"; mkdir -p "$jp"
tar -xzf "infra/downloads/joypad/rocknix-joypad-$JOYPAD_COMMIT.tar.gz" -C "$jp" --strip-components=1
log "compilando rocknix-joypad"
# DEVICE=RK3326: con DEVICE vacío su Makefile compila solo rocknix-joypad.ko, no el singleadc
make -C "$src" M="/work/$jp" DEVICE=RK3326 -j"$jobs" modules
[ -f "$jp/rocknix-singleadc-joypad.ko" ] || { echo "no se compiló rocknix-singleadc-joypad.ko" >&2; exit 1; }

# 6. Instalar en un directorio de staging (Arch usa /usr/lib/modules)
rm -rf "$out"; mkdir -p "$out/modroot/usr" "$out/dtbs/rockchip"
make -C "$src" -s INSTALL_MOD_PATH="/work/$out/modroot/usr" INSTALL_MOD_STRIP=1 modules_install
make -C "$src" -s M="/work/$jp" DEVICE=RK3326 INSTALL_MOD_PATH="/work/$out/modroot/usr" INSTALL_MOD_STRIP=1 modules_install
rm -f "$out/modroot/usr/lib/modules/$krel/build" "$out/modroot/usr/lib/modules/$krel/source"
depmod -b "$out/modroot/usr" "$krel"
cp "$src/arch/arm64/boot/Image" "$src/System.map" "$out/"
cp "$src/.config" "$out/config"
cp "$dts_dir"/rk3326-powkiddy-rgb20s*.dtb "$out/dtbs/rockchip/"
echo "$krel" > "$out/kernel.release"
log "listo: $krel"
ls -la "$out" "$out/dtbs/rockchip"
