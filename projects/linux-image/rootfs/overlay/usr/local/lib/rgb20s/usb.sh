#!/bin/bash
# Rol del USB-C (dwc2) y gadget USB compuesto: red (NCM/ECM/RNDIS) + puerto serie (ACM).
# Uso: usb.sh start|stop     (lo llama rgb20s-usb.service)
#
# - Rol: escribe device|host en /sys/class/usb_role/*/role (role switch de dwc2, ver el DTS
#   rk3326-powkiddy-rgb20s-alarm.dts). Se configura con USB_ROLE en /boot/rgb20s.conf.
# - Gadget: configfs en /sys/kernel/config/usb_gadget/rgb20s.
#   * Red: la consola queda en USB_ADDRESS (10.55.0.1/24) y sirve DHCP a la Mac (networkd).
#   * Serie: /dev/ttyGS0 en la consola = /dev/ttyACM0 en la Mac, con login (serial-getty).
set -u
. /usr/local/lib/rgb20s/common.sh

G=/sys/kernel/config/usb_gadget/rgb20s
NET_RUNTIME=/run/systemd/network/10-rgb20s-gadget.network

set_role() {
  local want="$1" sw found=0 i
  # dwc2 es built-in, pero su probe puede diferirse (regulador, PHY): esperar hasta 10 s
  for i in $(seq 1 20); do
    ls /sys/class/usb_role/*/role >/dev/null 2>&1 && break
    sleep 0.5
  done
  for sw in /sys/class/usb_role/*/role; do
    [ -e "$sw" ] || continue
    found=1
    if echo "$want" > "$sw" 2>/dev/null; then
      log "rol USB: $(cat "$sw") ($sw)"
    else
      log "AVISO: no se pudo poner el rol '$want' en $sw"
    fi
  done
  [ "$found" = 1 ] || log "AVISO: no hay role switch en /sys/class/usb_role (¿dwc2 no cargó? revisar dmesg | grep dwc2)"
}

# MAC estables (localmente administradas) derivadas de machine-id, para que la Mac
# reconozca siempre la misma placa de red.
mac_from() {
  local h
  h="$(printf '%s-%s' "$(cat /etc/machine-id 2>/dev/null)" "$1" | sha256sum | cut -c1-10)"
  printf '02:%s:%s:%s:%s:%s' "${h:0:2}" "${h:2:2}" "${h:4:2}" "${h:6:2}" "${h:8:2}"
}

gadget_stop() {
  [ -d "$G" ] || return 0
  echo "" > "$G/UDC" 2>/dev/null || true
  rm -f "$G"/configs/c.1/*.usb0 2>/dev/null || true
  rmdir "$G"/configs/c.1/strings/0x409 "$G"/configs/c.1 2>/dev/null || true
  rmdir "$G"/functions/* 2>/dev/null || true
  rmdir "$G"/strings/0x409 "$G" 2>/dev/null || true
}

gadget_start() {
  local net addr udc serial
  net="$(conf_get USB_NET ncm)"
  case "$net" in ncm|ecm|rndis) ;; *) log "USB_NET=$net no válido, uso ncm"; net=ncm ;; esac
  addr="$(conf_get USB_ADDRESS 10.55.0.1/24)"

  mountpoint -q /sys/kernel/config || mount -t configfs none /sys/kernel/config
  gadget_stop
  mkdir -p "$G" || { log "ERROR: no se pudo crear $G (¿falta CONFIG_USB_CONFIGFS?)"; return 1; }
  cd "$G" || return 1

  echo 0x1d6b > idVendor          # Linux Foundation
  echo 0x0104 > idProduct         # Multifunction Composite Gadget
  echo 0x0100 > bcdDevice
  echo 0x0200 > bcdUSB
  echo 0xEF > bDeviceClass        # Misc + IAD: necesario para funciones compuestas
  echo 0x02 > bDeviceSubClass
  echo 0x01 > bDeviceProtocol
  serial="$(cut -c1-16 /etc/machine-id 2>/dev/null)"
  mkdir -p strings/0x409
  echo "Powkiddy" > strings/0x409/manufacturer
  echo "RGB20S (Arch Linux ARM)" > strings/0x409/product
  echo "${serial:-rgb20s}" > strings/0x409/serialnumber

  mkdir -p configs/c.1/strings/0x409
  echo "red + serie" > configs/c.1/strings/0x409/configuration
  echo 500 > configs/c.1/MaxPower

  if mkdir -p "functions/$net.usb0"; then
    mac_from dev  > "functions/$net.usb0/dev_addr"
    mac_from host > "functions/$net.usb0/host_addr"
    ln -s "functions/$net.usb0" configs/c.1/
  else
    log "AVISO: falta la función $net en el kernel"
  fi
  if mkdir -p functions/acm.usb0; then
    ln -s functions/acm.usb0 configs/c.1/
  else
    log "AVISO: falta la función acm en el kernel"
  fi

  udc="$(ls /sys/class/udc 2>/dev/null | head -n1)"
  if [ -z "$udc" ]; then
    log "ERROR: no hay UDC en /sys/class/udc: dwc2 no está en modo periférico o no cargó"
    return 1
  fi
  echo "$udc" > UDC && log "gadget $net+acm activo en $udc (estado: $(cat /sys/class/udc/$udc/state 2>/dev/null))"

  # Red del gadget: config de networkd en /run (la IP sale de rgb20s.conf).
  # Se matchea por driver (u_ether.c, eth_get_drvinfo, informa "g_ether") para no confundirla con un
  # teléfono en modo host (rndis_host), que también puede llamarse usb0.
  mkdir -p /run/systemd/network
  cat > "$NET_RUNTIME" <<EOF
# Generado por /usr/local/lib/rgb20s/usb.sh
[Match]
Driver=g_ether

[Network]
Address=$addr
DHCPServer=yes
LinkLocalAddressing=ipv6
MulticastDNS=yes
ConfigureWithoutCarrier=yes

[DHCPServer]
PoolOffset=10
PoolSize=10
EmitDNS=no
EmitRouter=no
EmitNTP=no
EOF
  networkctl reload 2>/dev/null || true

  systemctl --no-block start serial-getty@ttyGS0.service
}

case "${1:-start}" in
  start)
    role="$(conf_get USB_ROLE device)"
    case "$role" in
      device) set_role device; gadget_start ;;
      host)   gadget_stop; rm -f "$NET_RUNTIME"; set_role host ;;
      *)      log "USB_ROLE=$role no válido (device|host); uso device"; set_role device; gadget_start ;;
    esac
    ;;
  stop) gadget_stop; rm -f "$NET_RUNTIME" ;;
  *) echo "uso: $0 start|stop" >&2; exit 2 ;;
esac
exit 0
