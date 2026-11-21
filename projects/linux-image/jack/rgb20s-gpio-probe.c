// SPDX-License-Identifier: GPL-2.0-or-later
/*
 * rgb20s-gpio-probe: lee el nivel de TODOS los pines GPIO del RK3326 (PX30) sin pedirlos al
 * kernel, para buscar el pin de detección del jack de auriculares de la RGB20S.
 *
 * Lee por /dev/mem (mmap, PROT_READ) los registros de los 4 bancos GPIO:
 *   GPIO_SWPORTA_DDR (0x04, 1 = salida) y GPIO_EXT_PORTA (0x50, nivel del pad).
 * No cambia el mux, la dirección ni el pull: no interfiere con los drivers. La única escritura
 * posible es `pull`, que cambia el pull-up/down de UN pin (registro GRF con máscara por pin) y
 * se pierde al reiniciar.
 *
 * Direcciones (px30.dtsi y drivers/gpio/gpio-rockchip.c, gpio_regs_v1, kernel 7.1.2):
 *   gpio0 0xff040000, gpio1 0xff250000, gpio2 0xff260000, gpio3 0xff270000
 * Pull (drivers/pinctrl/pinctrl-rockchip.c, px30_calc_pull_reg_and_bit): 2 bits por pin,
 *   banco 0 en PMUGRF 0xff010000 + 0x10, bancos 1-3 en GRF 0xff140000 + 0x60 + (banco-1)*0x10,
 *   + (pin/8)*4; valores 0 = sin pull, 1 = pull-up, 2 = pull-down, 3 = bus-hold.
 *
 * Uso (como root):
 *   rgb20s-gpio-probe dump                       nivel y dirección de los 128 pines
 *   rgb20s-gpio-probe watch [segundos]           muestra cada cambio de nivel en pines de ENTRADA
 *   rgb20s-gpio-probe get gpio2-c6               nivel, dirección y pull de un pin
 *   rgb20s-gpio-probe pull gpio2-c6 up|down|none cambia el pull de un pin (hasta reiniciar)
 *
 * Requiere CONFIG_DEVMEM=y y que no esté CONFIG_IO_STRICT_DEVMEM (así está el kernel 7.1.2-rgb20s).
 */
#define _DEFAULT_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <time.h>
#include <unistd.h>

#define NBANKS 4
#define REG_DDR 0x04
#define REG_EXT 0x50
#define MAP_SIZE 0x1000

static const uint32_t bank_base[NBANKS] = { 0xff040000, 0xff250000, 0xff260000, 0xff270000 };
#define PMUGRF_BASE 0xff010000u
#define GRF_BASE 0xff140000u

static int memfd = -1;
static volatile uint32_t *bank_regs[NBANKS];
static volatile sig_atomic_t stop;

static void on_signal(int sig) { (void)sig; stop = 1; }

static volatile uint32_t *map_page(uint32_t phys, int writable)
{
	void *p = mmap(NULL, MAP_SIZE, PROT_READ | (writable ? PROT_WRITE : 0), MAP_SHARED, memfd,
		       (off_t)phys);
	if (p == MAP_FAILED) {
		fprintf(stderr, "mmap de 0x%08x: %s\n", phys, strerror(errno));
		exit(1);
	}
	return (volatile uint32_t *)p;
}

static void open_mem(int writable)
{
	memfd = open("/dev/mem", (writable ? O_RDWR : O_RDONLY) | O_SYNC);
	if (memfd < 0) {
		fprintf(stderr, "/dev/mem: %s (correr como root)\n", strerror(errno));
		exit(1);
	}
	for (int b = 0; b < NBANKS; b++)
		bank_regs[b] = map_page(bank_base[b], 0);
}

static uint32_t rd(int bank, uint32_t off) { return bank_regs[bank][off / 4]; }

/* "gpio2-c6", "gpio2_c6" o "GPIO2_C6" -> banco 2, pin 22 */
static int parse_pin(const char *s, int *bank, int *pin)
{
	char port;
	int b, n;
	if (sscanf(s, "gpio%d%*[-_]%c%d", &b, &port, &n) != 3 &&
	    sscanf(s, "GPIO%d%*[-_]%c%d", &b, &port, &n) != 3)
		return -1;
	if (port >= 'a' && port <= 'd')
		port = (char)(port - 'a' + 'A');
	if (b < 0 || b >= NBANKS || port < 'A' || port > 'D' || n < 0 || n > 7)
		return -1;
	*bank = b;
	*pin = (port - 'A') * 8 + n;
	return 0;
}

static void pin_name(int bank, int pin, char *buf, size_t len)
{
	snprintf(buf, len, "GPIO%d_%c%d", bank, 'A' + pin / 8, pin % 8);
}

static void pull_reg(int bank, int pin, uint32_t *phys, int *bit)
{
	if (bank == 0)
		*phys = PMUGRF_BASE + 0x10;
	else
		*phys = GRF_BASE + 0x60 + (uint32_t)(bank - 1) * 0x10;
	*phys += (uint32_t)(pin / 8) * 4;
	*bit = (pin % 8) * 2;
}

static const char *pull_name(unsigned v)
{
	static const char *names[] = { "ninguno", "pull-up", "pull-down", "bus-hold" };
	return names[v & 3];
}

static unsigned read_pull(int bank, int pin)
{
	uint32_t phys;
	int bit;
	pull_reg(bank, pin, &phys, &bit);
	volatile uint32_t *page = map_page(phys & ~(MAP_SIZE - 1), 0);
	unsigned v = (page[(phys & (MAP_SIZE - 1)) / 4] >> bit) & 3;
	munmap((void *)page, MAP_SIZE);
	return v;
}

static void now(char *buf, size_t len)
{
	struct timespec ts;
	clock_gettime(CLOCK_MONOTONIC, &ts);
	snprintf(buf, len, "%5ld.%03ld", (long)ts.tv_sec, ts.tv_nsec / 1000000);
}

static int cmd_dump(void)
{
	for (int b = 0; b < NBANKS; b++) {
		uint32_t ddr = rd(b, REG_DDR), ext = rd(b, REG_EXT);
		printf("gpio%d @0x%08x  DDR=0x%08x  EXT=0x%08x\n", b, bank_base[b], ddr, ext);
		for (int p = 0; p < 32; p++) {
			char n[16];
			pin_name(b, p, n, sizeof(n));
			printf("  %-9s %s %s%s", n, (ddr >> p) & 1 ? "out" : "in ", (ext >> p) & 1 ? "1" : "0",
			       p % 4 == 3 ? "\n" : "   ");
		}
	}
	return 0;
}

static int cmd_get(const char *spec)
{
	int bank, pin;
	char n[16];
	if (parse_pin(spec, &bank, &pin)) {
		fprintf(stderr, "pin inválido: %s (ej. gpio2-c6)\n", spec);
		return 2;
	}
	pin_name(bank, pin, n, sizeof(n));
	uint32_t ddr = rd(bank, REG_DDR), ext = rd(bank, REG_EXT);
	printf("%s: nivel=%u dir=%s pull=%s (línea %d de gpiochip%d)\n", n, (ext >> pin) & 1,
	       (ddr >> pin) & 1 ? "salida" : "entrada", pull_name(read_pull(bank, pin)), pin, bank);
	return 0;
}

static int cmd_watch(int seconds)
{
	uint32_t prev[NBANKS];
	char t[24], n[16];
	for (int b = 0; b < NBANKS; b++)
		prev[b] = rd(b, REG_EXT);
	now(t, sizeof(t));
	printf("%s inicio: gpio0=0x%08x gpio1=0x%08x gpio2=0x%08x gpio3=0x%08x\n", t, prev[0], prev[1],
	       prev[2], prev[3]);
	printf("solo pines de entrada; las salidas (mux del joypad, LED, etc.) se ignoran. Ctrl-C para salir.\n");
	fflush(stdout);
	time_t end = seconds > 0 ? time(NULL) + seconds : 0;
	while (!stop && (end == 0 || time(NULL) < end)) {
		for (int b = 0; b < NBANKS; b++) {
			uint32_t cur = rd(b, REG_EXT);
			uint32_t changed = (cur ^ prev[b]) & ~rd(b, REG_DDR);
			for (int p = 0; changed && p < 32; p++) {
				if (!((changed >> p) & 1))
					continue;
				now(t, sizeof(t));
				pin_name(b, p, n, sizeof(n));
				printf("%s %-9s %u -> %u\n", t, n, (prev[b] >> p) & 1, (cur >> p) & 1);
				fflush(stdout);
			}
			prev[b] = cur;
		}
		usleep(20000);
	}
	return 0;
}

static int cmd_pull(const char *spec, const char *mode)
{
	int bank, pin, bit;
	uint32_t phys, val;
	char n[16];
	if (parse_pin(spec, &bank, &pin)) {
		fprintf(stderr, "pin inválido: %s\n", spec);
		return 2;
	}
	if (!strcmp(mode, "none"))
		val = 0;
	else if (!strcmp(mode, "up"))
		val = 1;
	else if (!strcmp(mode, "down"))
		val = 2;
	else {
		fprintf(stderr, "modo inválido: %s (up|down|none)\n", mode);
		return 2;
	}
	pin_name(bank, pin, n, sizeof(n));
	pull_reg(bank, pin, &phys, &bit);
	unsigned before = read_pull(bank, pin);
	volatile uint32_t *page = map_page(phys & ~(MAP_SIZE - 1), 1);
	/* Registros GRF con "write mask" en los 16 bits altos: solo cambian los 2 bits del pin. */
	page[(phys & (MAP_SIZE - 1)) / 4] = (3u << (bit + 16)) | (val << bit);
	munmap((void *)page, MAP_SIZE);
	printf("%s: pull %s -> %s (registro 0x%08x, bits %d-%d; vuelve al valor anterior al reiniciar)\n",
	       n, pull_name(before), pull_name(read_pull(bank, pin)), phys, bit, bit + 1);
	usleep(10000);
	return cmd_get(spec);
}

static void usage(void)
{
	fprintf(stderr, "uso: rgb20s-gpio-probe dump | watch [segundos] | get <pin> | pull <pin> up|down|none\n"
			"     <pin> = gpioB-Pn, por ejemplo gpio2-c6\n");
}

int main(int argc, char **argv)
{
	if (argc < 2) {
		usage();
		return 2;
	}
	signal(SIGINT, on_signal);
	signal(SIGTERM, on_signal);
	int writable = !strcmp(argv[1], "pull");
	if (!strcmp(argv[1], "dump") || !strcmp(argv[1], "watch") || !strcmp(argv[1], "get") || writable) {
		open_mem(writable);
	} else {
		usage();
		return 2;
	}
	if (!strcmp(argv[1], "dump"))
		return cmd_dump();
	if (!strcmp(argv[1], "watch"))
		return cmd_watch(argc > 2 ? atoi(argv[2]) : 0);
	if (!strcmp(argv[1], "get") && argc == 3)
		return cmd_get(argv[2]);
	if (writable && argc == 4)
		return cmd_pull(argv[2], argv[3]);
	usage();
	return 2;
}
