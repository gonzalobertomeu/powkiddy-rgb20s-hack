# Contenedor de build de la imagen de Arch Linux ARM para la RGB20S (x86_64).
# Compila U-Boot, el kernel, el módulo del joypad y rgb20s-splash (estático, libc6-dev-arm64-cross)
# con gcc cruzado, y arma la .img
# sin loop devices ni sudo: particiona un archivo con sfdisk, crea FAT con mtools y ext4
# con `mke2fs -d`, y escribe el bootloader con dd sobre el archivo.
# Misma base fijada que rgb20s-cross (infra/versions.env, ADR 0008).
ARG DEBIAN_IMAGE
FROM --platform=linux/amd64 ${DEBIAN_IMAGE}

ARG DEBIAN_SNAPSHOT
COPY apt-snapshot.sh /usr/local/sbin/apt-snapshot.sh
RUN /usr/local/sbin/apt-snapshot.sh "${DEBIAN_SNAPSHOT}" \
 && apt-get update \
 && apt-get install -y --no-install-recommends \
      ca-certificates build-essential bc bison flex libssl-dev libelf-dev libgnutls28-dev \
      gcc-aarch64-linux-gnu binutils-aarch64-linux-gnu libc6-dev-arm64-cross \
      python3 python3-dev python3-setuptools python3-pyelftools python3-libfdt swig \
      device-tree-compiler kmod cpio xz-utils zstd file rsync \
      libarchive-tools dosfstools mtools e2fsprogs fdisk uuid-runtime \
 && rm -rf /var/lib/apt/lists/*

ENV ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu-
WORKDIR /work
