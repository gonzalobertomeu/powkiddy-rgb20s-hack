# Arranca QEMU (virt, virtio-gpu sin pantalla) y toma capturas por QMP cada 2 s.
import json, os, socket, subprocess, sys, time
W = "/work/infra/build/qemu-splash-test"
os.makedirs(f"{W}/shots", exist_ok=True)
qmp = f"{W}/qmp.sock"
if os.path.exists(qmp): os.remove(qmp)
cmdline = ("root=/dev/vda rw rootwait console=ttyAMA0 console=tty0 consoleblank=0 loglevel=4 "
           "vt.global_cursor_default=0 fbcon=logo-count:1 systemd.show_status=auto udev.log_level=3")
p = subprocess.Popen(["qemu-system-aarch64", "-M", "virt", "-cpu", "cortex-a53", "-smp", "4", "-m", "1024",
    "-kernel", f"{W}/Image", "-initrd", f"{W}/initramfs.img", "-append", cmdline,
    "-drive", f"if=virtio,format=raw,file={W}/disk.ext4,snapshot=on",
    "-device", "virtio-gpu-pci,xres=640,yres=480", "-display", "none",
    "-serial", f"file:{W}/serial.log", "-qmp", f"unix:{qmp},server=on,wait=off", "-monitor", "none"])
for _ in range(100):
    if os.path.exists(qmp): break
    time.sleep(0.1)
s = socket.socket(socket.AF_UNIX); s.connect(qmp); f = s.makefile("rw")
def cmd(c, **a):
    f.write(json.dumps({"execute": c, "arguments": a} if a else {"execute": c}) + "\n"); f.flush()
    while True:
        r = json.loads(f.readline())
        if "return" in r or "error" in r: return r
f.readline(); cmd("qmp_capabilities")
t0 = time.time(); i = 0
dur = float(sys.argv[1]) if len(sys.argv) > 1 else 150
while time.time() - t0 < dur:
    cmd("screendump", filename=f"{W}/shots/{i:03d}-{int(time.time()-t0):03d}s.ppm"); i += 1
    time.sleep(2)
    if "==== FIN ====" in open(f"{W}/serial.log", errors="replace").read():
        cmd("screendump", filename=f"{W}/shots/{i:03d}-{int(time.time()-t0):03d}s.ppm"); break
cmd("quit"); p.wait()
