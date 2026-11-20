# launcher

Menú que aparece al prender la RGB20S, en lugar del login de texto de tty1: lista las apps de
`apps.toml` y, al final, **Reiniciar** y **Apagar**. Decisión: [ADR 0014](../../docs/decisions/0014-sesion-launcher-en-tty1.md) (propuesta).

Estado: probado en la Mac (simulador, driver offscreen y wrapper con apps de prueba) y compilado para aarch64
(glibc 2.34). **Sin probar en la consola.**

## Controles

| Consola | Simulador (`infra/sim/keymap.toml`) | Acción |
|---|---|---|
| Cruceta ↑/↓ | flechas | elegir (con repetición al mantener; un toque da la vuelta en los extremos, la repetición se detiene en ellos) |
| A | X | abrir |
| — | Esc | salir sin elegir (solo en el simulador) |

Arranca con la última app elegida seleccionada (`$XDG_STATE_HOME/launcher/last` o
`~/.local/state/launcher/last`; se guarda el nombre). Reiniciar y Apagar no se recuerdan.
Arriba a la derecha, la carga de la batería (`/sys/class/power_supply/*` con `type` = `Battery`; `+` = cargando).

Los botones frontales siguen la disposición **Nintendo** de la consola: A a la derecha, B abajo, Y a la
izquierda y X arriba. Verificado en el hardware (2026-11-20): el driver entrega A y B con los nombres de
Xbox de SDL (A abajo, B derecha), pero X e Y ya en su lugar, así que el código intercambia **solo A↔B**
(`PadLayout` en `src/input.rs`). `RGB20S_PAD_LAYOUT` cambia el mapeo: `xbox` usa los nombres de SDL tal
cual y `nintendo-full` intercambia A↔B y X↔Y (para consolas que entreguen los cuatro al estilo Xbox).

## apps.toml

```toml
[[app]]
name = "Groovebox"                         # obligatorio
command = "/home/alarm/groovebox/groovebox" # obligatorio; mejor con ruta absoluta
args = ["--algo"]                           # opcional
cwd = "/home/alarm/groovebox"               # opcional; por defecto, la carpeta del ejecutable
env = { SDL_VIDEODRIVER = "kmsdrm" }        # opcional
```

Se busca, en orden: `$LAUNCHER_APPS`; `apps.toml` junto al binario (override local: el deploy no lo
pisa); `assets/apps.toml` junto al binario (lo que copia `just deploy`); `assets/apps.toml` de la
carpeta actual (`just run`). Si falta o es inválido, se ve el error abajo y quedan Reiniciar y
Apagar. Las apps cuyo `command` (con `/`) no existe aparecen atenuadas como "no encontrada".

## Cómo funciona

El launcher **no lanza** las apps. Al elegir, cierra SDL por completo (libera DRM y el gamepad),
escribe la elección y sale; la ejecuta `system/rgb20s-session`, un bucle en bash:

1. Corre `launcher --choice-file $XDG_RUNTIME_DIR/rgb20s-choice`.
2. Código 0 y archivo no vacío: lo lee. Si es una app, la corre (cwd y env) y espera a que termine;
   si es `reboot`/`poweroff`, `systemctl reboot|poweroff` (si logind lo niega por no haber polkit,
   `sudo -n` con la regla que instala `install.sh`).
3. Vuelve a 1. Si el launcher falla 3 veces seguidas en menos de 5 s, espera 15 s.

Contrato del choice file (campos terminados en NUL; se lee con `mapfile -d ''`, sin `eval`):

```
reboot\0
poweroff\0
app\0<nombre>\0<cwd>\0<n>\0<K=V>\0 … (n veces) <argv0>\0<arg1>\0 …
```

`cwd` vacío = `$HOME`. Códigos de salida del launcher: 0 eligió, 1 error, 2 uso, 3 salió sin elegir.

Los logs del launcher, del wrapper y de las apps van al journal: `journalctl -u rgb20s-session -f`.

## Desarrollo

```sh
APP=launcher just run          # simulador (sin --choice-file imprime la elección)
APP=launcher just test
APP=launcher just build-arm && APP=launcher just check-arm
```

Variables: `RGB20S_SIM`, `RGB20S_SCALE`, `RGB20S_KEYMAP` (como el groovebox), `LAUNCHER_APPS`,
`LAUNCHER_REPEAT=delay,intervalo` (ms, defecto 300,90), `LAUNCHER_POWER_SUPPLY`,
`LAUNCHER_INPUT=down,a` (toques al arrancar, para probar sin teclado) y
`LAUNCHER_SHOT=captura.bmp` (guarda el primer cuadro y sale). Captura sin ventana:

```sh
SDL_VIDEODRIVER=offscreen RGB20S_SIM=1 LAUNCHER_SHOT=/tmp/l.bmp cargo run
```

El wrapper se prueba fuera de la consola con `LAUNCHER_BIN`, `RGB20S_SESSION_MAX_LOOPS=1` y
`RGB20S_SESSION_DRY_POWER=1` (no apaga).

## Instalación en la consola

```sh
APP=launcher just deploy                                  # binario + assets/apps.toml en /home/alarm/launcher
scp -r projects/launcher/system rgb20s:/home/alarm/launcher/
ssh rgb20s 'sudo /home/alarm/launcher/system/install.sh'
```

`install.sh` copia el wrapper a `/usr/local/bin`, la unidad a `/etc/systemd/system`, agrega
`/etc/sudoers.d/20-rgb20s-power` (alarm puede correr solo `systemctl poweroff` y `systemctl reboot`
sin clave), deshabilita `getty@tty1` y habilita y arranca `rgb20s-session`.

**Volver al login de texto:** `sudo /home/alarm/launcher/system/uninstall.sh` (por SSH). Si el
wrapper se cae 5 veces en un minuto, la unidad falla y systemd arranca `getty@tty1` solo
(`OnFailure=`).

## Licencias

La fuente Spleen (`fonts/`, BSD-2-Clause) está embebida; `fonts/LICENSE-spleen` se copia junto al
binario en el deploy.
