# Proyectos

El trabajo concreto: código, builds y scripts. Cada proyecto vive en su propia carpeta y se
corresponde con una solución documentada en [`docs/solutions/`](../docs/solutions/README.md).

Convención por proyecto:

```
projects/<nombre>/
  README.md     Qué es, cómo compilar, cómo desplegar en la consola
  src/
  ...
```

| Proyecto | Solución | Estado |
|---|---|---|
| [groovebox](groovebox/README.md) | [Groovebox](../docs/solutions/groovebox/README.md) | H5 (canción y cadenas); corre en la consola (KMSDRM, ALSA, gamepad) |
| [linux-image](linux-image/README.md) | [Arch Linux ARM](../docs/firmware/arch-linux-arm.md) | Arranca en la consola: SSH y red por USB gadget |
| [launcher](launcher/README.md) | Menú de arranque ([ADR 0014](../docs/decisions/0014-sesion-launcher-en-tty1.md)) | Instalado en la consola: arranca solo en tty1 y abre el groovebox |
