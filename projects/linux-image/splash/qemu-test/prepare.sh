#!/bin/bash
# Prepara un disco de prueba con la rootfs genérica de ALARM + rgb20s-splash (corre como root en el contenedor).
set -euo pipefail
W=/work/infra/build/qemu-splash-test
T=/test
rm -rf "$W"; mkdir -p "$W/root"
bsdtar -xpf /work/infra/downloads/alarm/ArchLinuxARM-aarch64-latest.tar.gz -C "$W/root" --numeric-owner
r="$W/root"
cp "$r/boot/Image" "$W/Image"; cp "$r/boot/initramfs-linux.img" "$W/initramfs.img"
install -D -m755 /work/infra/build/out/splash/rgb20s-splash "$r/usr/local/bin/rgb20s-splash"
install -D -m644 /work/infra/build/out/splash/splash.ppm "$r/usr/local/share/rgb20s/splash.ppm"
install -D -m644 "$T/other/splash.ppm" "$r/usr/local/share/rgb20s/other.ppm"
install -D -m644 /work/projects/linux-image/splash/system/rgb20s-splash.service "$r/etc/systemd/system/rgb20s-splash.service"
mkdir -p "$r/etc/systemd/system/sysinit.target.wants" "$r/etc/systemd/system/multi-user.target.wants"
ln -sfn /etc/systemd/system/rgb20s-splash.service "$r/etc/systemd/system/sysinit.target.wants/rgb20s-splash.service"
cat > "$r/etc/systemd/system/fake-launcher.service" <<'U'
[Unit]
Description=Prueba: app que toma la pantalla (rgb20s-splash -m con otra imagen)
After=rgb20s-splash.service
[Service]
ExecStartPre=/bin/sleep 12
ExecStart=/usr/local/bin/rgb20s-splash -m -t 25 -w 5 /usr/local/share/rgb20s/other.ppm
[Install]
WantedBy=multi-user.target
U
cat > "$r/etc/systemd/system/dump-test.service" <<'U'
[Unit]
Description=Prueba: vuelca el journal del splash a la consola serie
After=fake-launcher.service
[Service]
Type=oneshot
ExecStartPre=/bin/sleep 20
ExecStart=/bin/sh -c '{ echo ==== JOURNAL ====; journalctl -b -o short-monotonic --no-pager -u rgb20s-splash -u fake-launcher -u getty@tty1; journalctl -b -k -o short-monotonic --no-pager | grep -iE "virtio_gpu|virtio-gpu|fbcon|Console: switching"; systemctl show -p Result -p ExecMainStatus rgb20s-splash; echo ==== FIN ====; } | while IFS= read -r l; do echo "<3>DUMP: $$l" > /dev/kmsg; done'
[Install]
WantedBy=multi-user.target
U
ln -sfn /etc/systemd/system/fake-launcher.service "$r/etc/systemd/system/multi-user.target.wants/fake-launcher.service"
ln -sfn /etc/systemd/system/dump-test.service "$r/etc/systemd/system/multi-user.target.wants/dump-test.service"
mke2fs -q -t ext4 -d "$r" "$W/disk.ext4" 4G
rm -rf "$r"
chown -R "${HOST_UID:-0}:${HOST_GID:-0}" "$W"
ls -la "$W"
