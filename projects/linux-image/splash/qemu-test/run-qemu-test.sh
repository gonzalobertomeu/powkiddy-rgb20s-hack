#!/usr/bin/env bash
# Prueba de rgb20s-splash sin la consola: QEMU virt (aarch64, TCG) con virtio-gpu de 640x480, el kernel y la
# rootfs genéricos de ALARM (infra/downloads/alarm), la unidad real rgb20s-splash.service y un "launcher falso"
# (rgb20s-splash -m con otra imagen) que toma DRM master a los 12 s. Toma capturas por QMP cada 2 s y vuelca
# el journal por la consola serie. No prueba el panel DSI, fbcon de la RGB20S ni el logo del kernel.
# Uso (desde la raíz del repo; requiere just img-splash e img-fetch alarm): projects/linux-image/splash/qemu-test/run-qemu-test.sh
# Resultado: infra/build/qemu-splash-test/{shots/*.ppm,serial.log}. Tarda ~5 min.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../../.." && pwd)"
img=rgb20s-qemu-splash-test:tmp
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
set -a; . "$repo/infra/versions.env"; set +a
cp "$repo/infra/docker/apt-snapshot.sh" "$here/Dockerfile" "$work/"
docker build -q -t "$img" --build-arg DEBIAN_IMAGE --build-arg DEBIAN_SNAPSHOT "$work" >/dev/null
cp "$here/prepare.sh" "$here/run.py" "$work/"
python3 -I "$repo/projects/linux-image/splash/make-splash.py" --outdir "$work/other" \
  --title-a Fake --title-b Launcher --subtitle "otra app con DRM master" --only ppm >/dev/null
# Disco de prueba: como root DEL CONTENEDOR (sin --privileged ni loop devices)
docker run --rm -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" -v "$repo:/work" -v "$work:/test:ro" "$img" bash /test/prepare.sh
docker run --rm --user "$(id -u):$(id -g)" -v "$repo:/work" -v "$work:/test:ro" "$img" python3 -I /test/run.py 300
out="$repo/infra/build/qemu-splash-test"
rm -f "$out/disk.ext4"
tr -d '\r' < "$out/serial.log" | grep -a 'DUMP:' | sed 's/.*DUMP: //'
echo "capturas: $out/shots/"
