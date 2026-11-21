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
| [groovebox](groovebox/README.md) | [Groovebox](../docs/solutions/groovebox/README.md) | H6 (guardar y abrir proyectos en RON); el H5 corre en la consola (KMSDRM, ALSA, gamepad), el H6 sin probar ahí |
| [linux-image](linux-image/README.md) | [Arch Linux ARM](../docs/firmware/arch-linux-arm.md) | Arranca en la consola: SSH y red por USB gadget. [Splash de arranque](linux-image/splash/README.md): sin probar en la consola |
| [launcher](launcher/README.md) | Menú de arranque ([ADR 0014](../docs/decisions/0014-sesion-launcher-en-tty1.md)) | Instalado en la consola: arranca solo en tty1 y abre el groovebox |
| [rgb20s-audio](rgb20s-audio/README.md) | Servicio de audio del sistema ([ADR 0016](../docs/decisions/0016-servicio-de-audio-del-sistema.md)) | Instalado en la consola: botones de volumen e indicador funcionan |
| [rgb20s-common](rgb20s-common/README.md) | Biblioteca compartida: estado del audio y batería | Con tests; la usan launcher, groovebox y rgb20s-audio |
