ARG DEBIAN_IMAGE
# Contenedor de prueba (qemu-system-aarch64) para run-qemu-test.sh. Base fijada como el resto (infra/versions.env).
FROM --platform=linux/amd64 ${DEBIAN_IMAGE}
ARG DEBIAN_SNAPSHOT
COPY apt-snapshot.sh /usr/local/sbin/apt-snapshot.sh
RUN /usr/local/sbin/apt-snapshot.sh "${DEBIAN_SNAPSHOT}" && apt-get update \
 && apt-get install -y --no-install-recommends qemu-system-arm ipxe-qemu python3 libarchive-tools e2fsprogs \
 && rm -rf /var/lib/apt/lists/*
