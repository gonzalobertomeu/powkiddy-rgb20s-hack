# 0014 — Arrancar en un launcher propio en tty1, con un wrapper de sesión en bash

- **Fecha:** 2026-11-20
- **Estado:** propuesta

## Contexto
La consola arranca nuestra imagen de Arch Linux ARM ([0011](0011-rootfs-alarm-con-systemd.md)) en el login de texto de
tty1, sin teclado. Hace falta un menú que aparezca al prender, que abra el groovebox (y futuras apps) y que permita
reiniciar y apagar. Las apps usan SDL sobre KMSDRM: solo una puede tener DRM y el gamepad a la vez. Sin compositor.

## Alternativas consideradas
| Opción | A favor | En contra |
|---|---|---|
| Launcher que lanza la app como hijo y espera | Un solo proceso que vigilar | El launcher tiene que soltar DRM y el gamepad (cerrar SDL) y volver a abrirlos; un crash del launcher se lleva la app |
| Launcher que sale y un wrapper de sesión que ejecuta la elección | DRM libre al lanzar; el wrapper reintenta si el launcher falla; launcher simple | Contrato entre dos programas (choice file) |
| Autologin en getty + `.bash_profile` | Nada de unidades nuevas | Frágil (un shell interactivo de por medio), difícil de revertir y de loguear |
| Compositor (cage, weston) con el launcher como cliente | Cambiar de app sin parpadeo | Más paquetes y otra capa sobre KMSDRM; las apps tendrían que hablar Wayland |

## Decisión
Unidad `rgb20s-session.service` en tty1 (en conflicto con `getty@tty1`, `PAMName=login`, usuario `alarm`) que corre
`rgb20s-session`, un bucle en bash: corre el launcher, que escribe la elección en un archivo (campos separados por NUL)
y sale; el wrapper la ejecuta y vuelve a empezar.

## Motivos
Cada app recibe DRM y el gamepad libres, como si arrancara sola. El formato con NUL se lee en bash sin `eval` ni
escapes. La sesión de logind (PAM) da `XDG_RUNTIME_DIR` y la sesión activa en seat0. Volver al login es un script.

## Consecuencias
- Al cambiar de app se ve un instante la consola de texto de tty1 (el wrapper la limpia y esconde el cursor).
- Sin polkit, logind no deja apagar a `alarm`: `install.sh` agrega una regla de sudo solo para
  `systemctl poweroff|reboot`. Alternativa: instalar polkit en la imagen.
- Archivos: `projects/launcher/` (app, `system/`).

## Pendientes
- [ ] Probar en la consola: sesión activa (`loginctl session-status`), KMSDRM y gamepad en el launcher y al pasar al groovebox, apagar y reiniciar.
- [ ] Decidir si la imagen trae polkit o la regla de sudo, y si se instala la sesión desde `customize.sh`.
- [ ] Bajar los mensajes del kernel en tty1 (`quiet`/`loglevel` en la línea de comandos), si se ven entre apps.

## Fuentes
- Unidades de ejemplo de weston y cage (`PAMName=login`, `TTYPath`, `Conflicts=getty@tty1.service`, `UtmpIdentifier`).
- Apagar sin root requiere polkit: [systemd-devel, "Unprivileged poweroff" (2014)](https://lists.freedesktop.org/archives/systemd-devel/2014-September/022945.html), [Arch FS#51265](https://bugs.archlinux.org/task/51265.html).
- La imagen no trae polkit: `infra/build/out/rootfs/packages.txt` (2026-11-19).
