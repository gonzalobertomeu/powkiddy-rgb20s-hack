#!/bin/bash
# Inventario de la Powkiddy RGB20S, pensado para correr como "port" desde el menú de
# ArkOS/dArkOS (EmulationStation > Ports) cuando todavía no hay red ni SSH.
#
# SOLO LECTURA: no carga módulos, no monta nada, no activa gadgets, no cambia servicios
# ni archivos del sistema. Lo único que escribe es el archivo de resultados, junto a este
# script (rgb20s-inventory-AAAAMMDD-HHMMSS.txt), y texto de progreso en la consola (tty).
# Si `sudo` funciona sin contraseña, lo usa solo para LEER (dmesg y debugfs).
#
# Uso en la consola: copiar a <partición EASYROMS>/ports/RGB20S-Inventario.sh y ejecutarlo
# desde Ports. Ver docs/hardware/inventario.md.
# Para probarlo en otra máquina: INVENTORY_OUT_DIR=/ruta bash inventory.sh

# Sin `set -e` a propósito: si un comando falla o no existe, se anota y se sigue.
set -u

SCRIPT_VERSION="1 (2026-11-19)"
CMD_TIMEOUT=30

script_dir="$(cd "$(dirname "$0")" 2>/dev/null && pwd)"
out_dir="${INVENTORY_OUT_DIR:-$script_dir}"
stamp="$(date +%Y%m%d-%H%M%S)"
# FAT/exFAT montada con umask=000 suele ser escribible por el usuario "ark" (sin verificar
# en ArkOS); si no, se cae a $HOME para no perder el resultado.
if ! touch "$out_dir/.inventory-write-test" 2>/dev/null; then
  out_dir="${HOME:-/tmp}"
fi
rm -f "$out_dir/.inventory-write-test" 2>/dev/null
out="$out_dir/rgb20s-inventory-$stamp.txt"

have() { command -v "$1" >/dev/null 2>&1; }

# sudo solo si no somos root y no pide contraseña (-n): nunca queda esperando input.
SUDO=""
if [ "$(id -u)" -ne 0 ] && have sudo && sudo -n true 2>/dev/null; then
  SUDO="sudo -n"
fi

# --- Mensajes en pantalla ------------------------------------------------------------
# PortMaster escribe en /dev/tty0 (consola activa). Si no se puede escribir directo, se
# intenta con sudo tee (no cambia permisos del dispositivo). Si nada funciona, se sigue
# sin pantalla: el archivo de resultados es lo importante.
screen_tty=""
for t in /dev/tty0 /dev/tty1; do
  if [ -w "$t" ]; then screen_tty="$t"; break; fi
done
say() {
  local msg="$1"
  echo "$msg"
  if [ -n "$screen_tty" ]; then
    printf '%s\n' "$msg" > "$screen_tty" 2>/dev/null
  elif [ -n "$SUDO" ]; then
    printf '%s\n' "$msg" | $SUDO tee /dev/tty0 >/dev/null 2>&1
  fi
}
clear_screen() {
  if [ -n "$screen_tty" ]; then
    printf '\033c' > "$screen_tty" 2>/dev/null
  elif [ -n "$SUDO" ]; then
    printf '\033c' | $SUDO tee /dev/tty0 >/dev/null 2>&1
  fi
}

# --- Ejecución de comandos -----------------------------------------------------------
# run "<comando de shell>": imprime el comando como encabezado, lo ejecuta con timeout,
# junta stdout y stderr, y anota el código de salida si no es 0.
run() {
  local cmd="$1" rc
  printf '\n----- $ %s\n' "$cmd"
  if have timeout; then
    timeout "$CMD_TIMEOUT" bash -c "$cmd" 2>&1 </dev/null
  else
    bash -c "$cmd" 2>&1 </dev/null
  fi
  rc=$?
  [ "$rc" -ne 0 ] && printf '[código de salida: %s%s]\n' "$rc" "$([ "$rc" -eq 124 ] && echo ', timeout')"
  return 0
}

# run_if <programa> "<comando>": como run, pero si el programa no existe lo dice y sigue.
run_if() {
  local prog="$1" cmd="$2"
  if have "$prog"; then
    run "$cmd"
  else
    printf '\n----- $ %s\n(comando no disponible: %s)\n' "$cmd" "$prog"
  fi
}

section() { printf '\n\n########## %s ##########\n' "$1"; }

# Lee un archivo de /proc/device-tree (strings separadas por \0).
dt_cat() { tr '\0' '\n' < "$1" 2>/dev/null | sed '/^$/d'; }

# Comando para el dmesg completo (con sudo si hace falta por dmesg_restrict).
dmesg_cmd() {
  if have dmesg && dmesg >/dev/null 2>&1; then
    echo "dmesg"
  elif [ -n "$SUDO" ] && have dmesg; then
    echo "$SUDO dmesg"
  elif have journalctl; then
    echo "journalctl -k -b --no-pager"
  else
    echo "cat /var/log/kern.log"
  fi
}

# Busca el config del kernel: /proc/config.gz (si IKCONFIG_PROC) o /boot/config-<versión>
# (dArkOS lo copia ahí al construir la imagen).
kernel_config_cmd() {
  local kver
  kver="$(uname -r)"
  if [ -r /proc/config.gz ]; then
    echo "zcat /proc/config.gz"
  elif [ -r "/boot/config-$kver" ]; then
    echo "cat /boot/config-$kver"
  else
    local f
    f="$(ls /boot/config-* 2>/dev/null | head -n1)"
    [ -n "$f" ] && echo "cat $f"
  fi
}

# --- Secciones -----------------------------------------------------------------------

inv_meta() {
  section "META"
  echo "Script: inventory.sh versión $SCRIPT_VERSION"
  echo "Archivo: $out"
  echo "Fecha local de la consola: $(date '+%Y-%m-%d %H:%M:%S %z') (sin red, el reloj puede estar mal)"
  echo "Ejecutado como: $(id)"
  echo "sudo sin contraseña: $([ -n "$SUDO" ] && echo sí || echo 'no (o ya es root)')"
  echo "Pantalla: ${screen_tty:-ninguna tty escribible directo}"
  echo "Script en: $script_dir"
  echo "Gobernador al lanzar (ES ejecuta 'sudo perfmax' antes de los ports en ArkOS/dArkOS):"
  cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>&1
}

inv_system() {
  section "SISTEMA"
  run "cat /etc/os-release"
  run "ls -la /etc/*release* /etc/*version* 2>/dev/null; cat /etc/debian_version /etc/lsb-release 2>/dev/null"
  # ArkOS/dArkOS (y PortMaster) guardan el nombre del sistema y del equipo en ~/.config/.OS
  # y ~/.config/.DEVICE (sin verificar el resto de los archivos ocultos).
  run "for f in /home/*/.config/.OS /home/*/.config/.DEVICE /home/*/.config/.VERSION /root/.config/.OS /root/.config/.DEVICE; do [ -f \"\$f\" ] && echo \"\$f: \$(tr -d '\\0' < \"\$f\")\"; done"
  run "ls -la /home/*/.config/ 2>/dev/null | head -60"
  run "grep -ih 'title' /usr/share/plymouth/themes/*/*.plymouth 2>/dev/null"
  run "uname -a"
  run "uname -r"
  run "hostname"
  run "uptime"
  run "cat /proc/cmdline"
  run "free -m"
  run "df -h"
  run_if lsblk "lsblk -o NAME,SIZE,FSTYPE,MOUNTPOINT,LABEL"
  run "cat /proc/mounts"
  run "ls -la /boot"
}

inv_devicetree() {
  section "DEVICE TREE"
  run "cat /proc/device-tree/model; echo"
  run "tr '\\0' '\\n' < /proc/device-tree/compatible"
}

inv_cpu() {
  section "CPU Y TEMPERATURA"
  run "cat /proc/cpuinfo"
  run "nproc"
  run "ls /sys/devices/system/cpu/cpu0/cpufreq/"
  run "cd /sys/devices/system/cpu/cpu0/cpufreq/ && for f in scaling_available_frequencies scaling_available_governors scaling_governor scaling_cur_freq cpuinfo_min_freq cpuinfo_max_freq; do echo \"\$f: \$(cat \$f 2>&1)\"; done"
  run "for z in /sys/class/thermal/thermal_zone*; do echo \"\$z: \$(cat \$z/type 2>&1) \$(cat \$z/temp 2>&1)\"; done"
  run "ls /sys/class/devfreq/ 2>/dev/null && for d in /sys/class/devfreq/*; do echo \"\$d: \$(cat \$d/governor 2>&1) cur=\$(cat \$d/cur_freq 2>&1)\"; done"
}

inv_libs() {
  section "LIBRERÍAS"
  run "ldd --version 2>&1 | head -n1"
  run "ls -l /lib/*/libc.so* /usr/lib/*/libc.so* 2>/dev/null"
  run "ls -l /usr/lib/*/libSDL2* /usr/lib/libSDL2* /usr/local/lib/libSDL2* /usr/lib/*/libSDL3* 2>/dev/null"
  run_if sdl2-config "sdl2-config --version"
  run "ls -l /usr/lib/*/libasound* /usr/lib/libasound* 2>/dev/null"
  run "ls -l /usr/lib/*/libmali* /usr/lib/*/libEGL* /usr/lib/*/libGLES* /usr/lib/*/libgbm* /usr/lib/*/libdrm.so* /usr/local/lib/*mali* 2>/dev/null"
  run_if dpkg-query "dpkg-query -W -f='\${Package} \${Version} \${Architecture}\n' 2>/dev/null | grep -iE 'sdl2|sdl3|asound|^libc6 |mali|libgbm|libdrm|libegl|openssh|dropbear|network-manager|wpasupplicant|rustc|gcc '"
  run "file -L /bin/bash 2>/dev/null || ls -l /bin/bash"
  run "ls -d /lib/arm-linux-gnueabihf /usr/lib/arm-linux-gnueabihf /lib/aarch64-linux-gnu 2>/dev/null"
}

inv_kernel() {
  section "KERNEL Y MÓDULOS"
  run_if lsmod "lsmod"
  run "ls /lib/modules/"
  run "ls /lib/modules/\$(uname -r)/ 2>&1 | head -30"
  local cfg
  cfg="$(kernel_config_cmd)"
  if [ -n "$cfg" ]; then
    run "$cfg | grep -E 'IKCONFIG|USB_GADGET|USB_CONFIGFS|USB_F_|USB_DWC2|USB_DWC3|USB_ETH|USB_G_|USB_LIBCOMPOSITE|USB_U_ETHER|CONFIGFS_FS|USB_OTG|USB_ROLE|EXTCON|PHY_ROCKCHIP|USB_NET|RNDIS|CDC|R8152|SND_SOC|PANFROST|MALI|DRM_ROCKCHIP|PREEMPT|HZ_|CPU_FREQ_DEFAULT|WLAN|RTL8|BRCM|SSV|AIC'"
  else
    printf '\n----- config del kernel\nNo hay /proc/config.gz ni /boot/config-*.\n'
  fi
}

inv_audio() {
  section "AUDIO (ALSA)"
  run_if aplay "aplay -l"
  run_if aplay "aplay -L"
  run_if amixer "amixer"
  run_if amixer "amixer contents"
  run "cat /proc/asound/version /proc/asound/cards /proc/asound/devices /proc/asound/pcm"
  run "ls -R /proc/asound/ | head -60"
  run "cat /proc/asound/card*/pcm*p/info 2>/dev/null"
  run "cat /etc/asound.conf /home/*/.asoundrc 2>/dev/null"
  run "ls -l /var/lib/alsa/ 2>/dev/null"
  run "ps -eo pid,user,comm | grep -iE 'pulse|pipewire|jackd|bluealsa' | grep -v grep"
}

inv_input() {
  section "INPUT"
  run "ls -l /dev/input/ /dev/input/by-path/ /dev/input/by-id/ 2>&1"
  run "cat /proc/bus/input/devices"
  run "ls -l /dev/uinput 2>&1"
}

inv_display() {
  section "PANTALLA Y GPU"
  run "ls -l /dev/dri/ /dev/fb* /dev/mali* 2>&1"
  run "for f in /sys/class/drm/*/modes /sys/class/drm/*/status; do echo \"\$f: \$(cat \$f 2>&1 | tr '\\n' ' ')\"; done"
  run "for f in /sys/class/graphics/fb0/modes /sys/class/graphics/fb0/virtual_size /sys/class/graphics/fb0/bits_per_pixel; do echo \"\$f: \$(cat \$f 2>&1)\"; done"
  run "for b in /sys/class/backlight/*; do echo \"\$b: max=\$(cat \$b/max_brightness 2>&1) actual=\$(cat \$b/brightness 2>&1)\"; done"
}

inv_power() {
  section "BATERÍA Y ALIMENTACIÓN"
  run "for p in /sys/class/power_supply/*; do echo \"== \$p\"; cat \$p/uevent 2>&1; done"
}

inv_storage() {
  section "ALMACENAMIENTO"
  run "for h in /sys/class/mmc_host/*; do for c in \$h/mmc*; do [ -e \"\$c\" ] && echo \"\$c: type=\$(cat \$c/type 2>&1) name=\$(cat \$c/name 2>&1)\"; done; done"
  run "cat /proc/partitions"
  run_if blkid "blkid"
}

inv_usb() {
  section "USB GENERAL"
  run_if lsusb "lsusb"
  run_if lsusb "lsusb -t"
  # A qué controlador pertenece cada bus: ff300000.usb = DWC2 (OTG, el único que puede
  # ser gadget); ff340000.usb / ff350000.usb = EHCI / OHCI (solo host). Direcciones
  # tomadas de px30.dtsi; RK3326 comparte el mapa (sin verificar en la RGB20S).
  run "for b in /sys/bus/usb/devices/usb*; do echo \"\$b -> \$(readlink -f \$b)\"; echo \"   producto: \$(cat \$b/product 2>&1) | velocidad: \$(cat \$b/speed 2>&1)\"; done"
  run "for d in /sys/bus/usb/devices/*-*; do case \$d in *:*) continue;; esac; echo \"\$d: \$(cat \$d/idVendor 2>&1):\$(cat \$d/idProduct 2>&1) \$(cat \$d/manufacturer 2>/dev/null) \$(cat \$d/product 2>/dev/null) speed=\$(cat \$d/speed 2>&1)\"; done"
  run "for i in /sys/bus/usb/devices/*:*; do echo \"\$i: class=\$(cat \$i/bInterfaceClass 2>&1) driver=\$(basename \"\$(readlink -f \$i/driver 2>/dev/null)\" 2>/dev/null)\"; done"
}

inv_gadget() {
  section "USB GADGET (¿puede la consola ser placa de red USB de la Mac?)"
  run "ls -la /sys/class/udc/"
  run "for u in /sys/class/udc/*; do [ -e \"\$u\" ] || continue; echo \"== \$u -> \$(readlink -f \$u)\"; for f in \$u/*; do [ -f \"\$f\" ] && echo \"  \$(basename \$f): \$(cat \$f 2>&1 | head -c 200)\"; done; done"
  run "ls -la /sys/class/usb_role/ 2>&1"
  run "for e in /sys/class/extcon/*; do [ -e \"\$e\" ] || continue; echo \"\$e: name=\$(cat \$e/name 2>&1) state=\$(cat \$e/state 2>&1 | tr '\\n' ' ')\"; done"
  run "ls /sys/bus/platform/drivers/ | grep -iE 'dwc|usb|ehci|ohci|phy|udc|extcon|otg'"
  run "for d in /sys/bus/platform/drivers/*; do case \$(basename \$d) in *dwc*|*usb*|*ehci*|*ohci*|*phy*|*udc*|*otg*) echo \"== \$d\"; ls \$d | grep -vE '^(bind|unbind|uevent|module|new_id|remove_id)\$' | sed 's/^/  /';; esac; done"
  run "ls -la /sys/devices/platform/ | grep -iE 'usb|phy|otg'"
  run "for p in /sys/devices/platform/ff300000.usb /sys/devices/platform/ff340000.usb /sys/devices/platform/ff350000.usb; do echo \"\$p: \$([ -e \$p ] && echo existe || echo NO existe) driver=\$(basename \"\$(readlink -f \$p/driver 2>/dev/null)\" 2>/dev/null)\"; done"
  # El PHY USB2 de Rockchip (kernel 4.4) expone 'otg_mode' en sysfs (sin verificar). Solo se lee.
  run "find /sys/devices/platform -maxdepth 4 -name otg_mode 2>/dev/null | while read -r f; do echo \"\$f: \$(cat \$f 2>&1)\"; done"
  # dmesg filtrado: probe de dwc2, PHY, OTG, extcon y conexiones USB recientes.
  local dm
  dm="$(dmesg_cmd)"
  run "$dm | grep -iE 'dwc2|dwc3|udc|gadget|otg|u2phy|usb2-phy|inno|extcon|ff300000|ff340000|ff350000|vbus|bvalid|g_ether|g_ncm|libcomposite|configfs|rndis|cdc_|usbnet|r8152|new .*USB device'"
  # configfs: solo se mira si ya está montado; no se monta.
  run "grep -E 'configfs|debugfs' /proc/mounts"
  run "ls -la /sys/kernel/config/ /sys/kernel/config/usb_gadget/ 2>&1"
  # debugfs (si está montado): dwc2 expone dr_mode y estado de registros (sin verificar ruta).
  if [ -n "$SUDO" ] || [ "$(id -u)" -eq 0 ]; then
    run "$SUDO ls /sys/kernel/debug/ 2>&1 | head -80"
    run "$SUDO find /sys/kernel/debug -maxdepth 3 -name dr_mode 2>/dev/null | while read -r f; do echo \"\$f: \$($SUDO cat \$f 2>&1)\"; done"
    run "$SUDO ls /sys/kernel/debug/usb/ 2>&1"
  else
    printf '\n----- debugfs\nSin root ni sudo sin contraseña: no se puede leer /sys/kernel/debug.\n'
  fi
  # Módulos disponibles en disco (no se cargan) y compilados dentro del kernel.
  run "find /lib/modules/\$(uname -r) \\( -name '*gadget*' -o -name 'libcomposite*' -o -name 'usb_f_*' -o -name 'u_ether*' -o -name 'g_*.ko*' -o -name 'dwc2*' -o -name 'dwc3*' -o -name 'configfs*' \\) 2>/dev/null"
  run "grep -E 'gadget|libcomposite|usb_f_|u_ether|g_ether|g_ncm|dwc2|dwc3|configfs' /lib/modules/\$(uname -r)/modules.builtin 2>&1"
  run "grep -E 'dwc2|libcomposite|usb_f_|u_ether|g_ether|g_ncm' /proc/modules"
  # Device tree: nodos USB/PHY con status y dr_mode.
  run "dt=/proc/device-tree; [ -d \$dt ] || dt=/sys/firmware/devicetree/base; find \$dt -name dr_mode 2>/dev/null | while read -r f; do echo \"\$f = \$(tr -d '\\0' < \$f)\"; done"
  run "dt=/proc/device-tree; [ -d \$dt ] || dt=/sys/firmware/devicetree/base; find \$dt \\( -name 'usb@*' -o -name 'usb2-phy@*' -o -name 'otg-port' -o -name 'host-port' -o -name '*otg*' \\) -type d 2>/dev/null | while read -r n; do echo \"\$n: status=\$(tr -d '\\0' < \$n/status 2>/dev/null || echo '(sin status = okay)') compatible=\$(tr '\\0' ' ' < \$n/compatible 2>/dev/null) dr_mode=\$(tr -d '\\0' < \$n/dr_mode 2>/dev/null)\"; done"
  run "ls /proc/device-tree/__symbols__/ 2>/dev/null | grep -iE 'usb|otg|phy' | while read -r s; do echo \"\$s -> \$(tr -d '\\0' < /proc/device-tree/__symbols__/\$s)\"; done"
  # Restos de la pila "usbdevice"/adbd de Rockchip o scripts de gadget del sistema.
  run "for p in adbd usbdevice umtprd; do command -v \$p; done"
  run "ls -la /etc/init.d/ 2>/dev/null | grep -iE 'usb|adb|gadget'; ls -la /etc/init.d/.usb_config /usr/bin/usbdevice /etc/usbdevice.d 2>&1"
  run "find /etc /usr/bin /usr/sbin /usr/local/bin /usr/local/sbin /opt /lib/systemd/system /etc/systemd/system -maxdepth 3 \\( -iname '*gadget*' -o -iname '*usbdevice*' -o -iname '*adbd*' -o -iname '*usb_config*' -o -iname '*otg*' \\) 2>/dev/null"
  run_if systemctl "systemctl list-unit-files --no-pager 2>/dev/null | grep -iE 'usb|adb|gadget|otg'"
}

inv_network() {
  section "RED, WI-FI Y SSH"
  run_if ip "ip -d link"
  run_if ip "ip addr"
  run_if ip "ip route"
  run "for n in /sys/class/net/*; do echo \"\$n: driver=\$(basename \"\$(readlink -f \$n/device/driver 2>/dev/null)\" 2>/dev/null) device=\$(readlink -f \$n/device 2>/dev/null)\"; done"
  run_if iw "iw dev"
  run_if rfkill "rfkill list"
  run "for d in /sys/bus/sdio/devices/*; do [ -e \"\$d\" ] && echo \"\$d: vendor=\$(cat \$d/vendor 2>&1) device=\$(cat \$d/device 2>&1) driver=\$(basename \"\$(readlink -f \$d/driver 2>/dev/null)\" 2>/dev/null)\"; done; ls /sys/bus/sdio/devices/ 2>&1"
  local dm
  dm="$(dmesg_cmd)"
  run "$dm | grep -iE 'wlan|wifi|sdio|rtl|rtw|brcm|esp|8189|8188|8723|8821|ssv|aic|cfg80211|mmc[0-9]'"
  # Drivers de red USB del lado host (tethering de Android, adaptadores USB-Ethernet).
  run "grep -E '^(usbnet|rndis_host|cdc_ether|cdc_ncm|cdc_eem|cdc_subset|r8152|ax88179_178a|asix) ' /proc/modules"
  run "find /lib/modules/\$(uname -r) \\( -name 'usbnet*' -o -name 'rndis_host*' -o -name 'cdc_ether*' -o -name 'cdc_ncm*' -o -name 'cdc_eem*' -o -name 'r8152*' -o -name 'ax88179*' -o -name 'asix*' \\) 2>/dev/null"
  run_if nmcli "nmcli -t -f DEVICE,TYPE,STATE,CONNECTION device"
  # SSH: si el servicio existe y está habilitado (no se lee ninguna clave).
  run_if systemctl "for s in ssh sshd dropbear NetworkManager; do echo \"\$s: enabled=\$(systemctl is-enabled \$s 2>&1) active=\$(systemctl is-active \$s 2>&1)\"; done"
  run "grep -E '^\\s*(Port|PermitRootLogin|PasswordAuthentication|ListenAddress)' /etc/ssh/sshd_config /etc/ssh/sshd_config.d/*.conf 2>&1"
  run_if ss "ss -tlnp 2>/dev/null"
}

inv_services() {
  section "SERVICIOS Y PROCESOS"
  run_if systemctl "systemctl list-units --type=service --state=running --no-pager"
  run "ps -eo pid,user,ni,pcpu,pmem,comm --sort=-pcpu | head -30"
}

# Controlador del que cuelga un dispositivo de sysfs (p. ej. ff300000.usb), o la ruta completa.
controller_of() {
  local p c
  p="$(readlink -f "$1")"
  c="$(echo "$p" | sed -n 's#.*/platform/\([^/]*\)/.*#\1#p')"
  echo "${c:-$p}"
}

# Resumen automático de las señales clave para el gadget USB.
inv_summary() {
  section "RESUMEN (señales para el gadget USB)"
  local udc cfg dm kcfg
  udc="$(ls /sys/class/udc/ 2>/dev/null | tr '\n' ' ')"
  echo "UDC en /sys/class/udc: ${udc:-NINGUNO}"
  echo "Driver de ff300000.usb (DWC2/OTG): $(basename "$(readlink -f /sys/devices/platform/ff300000.usb/driver 2>/dev/null)" 2>/dev/null)"
  for b in /sys/bus/usb/devices/usb*; do
    [ -e "$b" ] && echo "Bus $(basename "$b") cuelga de: $(controller_of "$b")"
  done
  echo "Dispositivos USB conectados (no hubs raíz):"
  for d in /sys/bus/usb/devices/*-*; do
    case "$d" in *:*) continue ;; esac
    [ -e "$d" ] && echo "  $(basename "$d"): $(cat "$d/idVendor" 2>/dev/null):$(cat "$d/idProduct" 2>/dev/null) $(cat "$d/product" 2>/dev/null) -> $(controller_of "$d")"
  done
  echo "configfs montado: $(grep -q ' configfs ' /proc/mounts && echo sí || echo no)"
  cfg="$(kernel_config_cmd)"
  if [ -n "$cfg" ]; then
    kcfg="$(bash -c "$cfg" 2>/dev/null)"
    for k in CONFIG_USB_DWC2 CONFIG_USB_DWC2_DUAL_ROLE CONFIG_USB_DWC2_PERIPHERAL CONFIG_USB_DWC2_HOST CONFIG_USB_GADGET CONFIG_USB_LIBCOMPOSITE CONFIG_USB_CONFIGFS CONFIG_USB_CONFIGFS_NCM CONFIG_USB_CONFIGFS_ECM CONFIG_USB_CONFIGFS_RNDIS CONFIG_USB_ETH CONFIG_USB_G_NCM CONFIG_CONFIGFS_FS; do
      echo "$k: $(echo "$kcfg" | grep -E "^$k=|^# $k is not set" | head -n1)"
    done
  else
    echo "Config del kernel: no disponible"
  fi
  echo "Nodos DT con dr_mode:"
  find /proc/device-tree -name dr_mode 2>/dev/null | while read -r f; do echo "  $f = $(tr -d '\0' < "$f")"; done
  echo "PHY otg-port en DT:"
  find /proc/device-tree -type d -name otg-port 2>/dev/null | while read -r n; do echo "  $n status=$(tr -d '\0' < "$n/status" 2>/dev/null || echo okay)"; done
  dm="$(dmesg_cmd)"
  echo "Líneas de dwc2 en dmesg: $(bash -c "$dm" 2>/dev/null | grep -ci dwc2)"
}

inv_annex() {
  section "ANEXO A: dmesg completo"
  run "$(dmesg_cmd)"
  section "ANEXO B: config completo del kernel"
  local cfg
  cfg="$(kernel_config_cmd)"
  if [ -n "$cfg" ]; then run "$cfg"; else echo "No disponible."; fi
}

# --- Principal -----------------------------------------------------------------------
main() {
  clear_screen
  say "Inventario RGB20S (solo lectura)"
  say "Resultado: $out"
  say ""
  {
    echo "Inventario de la RGB20S — $(date '+%Y-%m-%d %H:%M:%S')"
    inv_meta
  } > "$out" 2>&1
  local step
  for step in system devicetree cpu libs kernel audio input display power storage usb gadget network services summary annex; do
    say "  - $step"
    "inv_$step" >> "$out" 2>&1
  done
  echo -e "\n\n########## FIN ##########" >> "$out"
  # Importante en FAT: forzar la escritura antes de que el usuario apague o saque la SD.
  sync
  say ""
  say "Listo. Archivo:"
  say "$(basename "$out")"
  say "en $out_dir"
  say "Volviendo al menú en 8 s..."
  sleep 8
  clear_screen
}

main "$@"
exit 0
