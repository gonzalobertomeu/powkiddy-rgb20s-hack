// rgb20s-splash: muestra el splash de arranque por DRM/KMS hasta que otra app toma la pantalla.
//
// Flujo (ver README.md de esta carpeta y el ADR 0015):
//   1. Espera a que exista una tarjeta DRM con un conector conectado (/dev/dri/card*).
//   2. Crea un dumb buffer XRGB8888 del tamaño del modo, lo pinta con el PPM (P6) centrado sobre el
//      color del píxel (0,0) y lo pone en el CRTC (SETCRTC). Si el CRTC ya tiene modo (el de fbcon),
//      lo reusa: así no hay modeset completo ni parpadeo del panel.
//   3. Suelta DRM master (DROP_MASTER): el launcher (SDL2/KMSDRM) se vuelve master al abrir la tarjeta.
//      El buffer sigue en pantalla mientras este proceso tenga el fd abierto.
//   4. Avisa a systemd (READY=1) y vigila el CRTC cada 100 ms:
//        - si el framebuffer en pantalla cambió y hay otro master: el launcher tomó la pantalla -> sale;
//        - si cambió y no hay master (fbcon recuperó la pantalla): vuelve a ponerse (SET_MASTER,
//          SETCRTC, DROP_MASTER), con un límite de reintentos;
//        - SIGTERM o -t segundos: sale. Al cerrar, el kernel borra el buffer y fbcon vuelve.
//
// Sin dependencias: libc + headers uapi del kernel (drm.h, drm_mode.h). Se compila estático.
// Uso: rgb20s-splash [-t segundos] [-w segundos_espera] [-d /dev/dri/cardN] [-m] [imagen.ppm]
//   -m  para pruebas: no suelta DRM master (se comporta como una app que toma la pantalla)

#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <time.h>
#include <unistd.h>

#include <drm/drm.h>
#include <drm/drm_mode.h>

#define DEFAULT_IMAGE "/usr/local/share/rgb20s/splash.ppm"
#define MAX_REASSERT 20

static volatile sig_atomic_t stop_requested;

static void on_signal(int sig) {
    (void)sig;
    stop_requested = 1;
}

static void logmsg(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    fputs("rgb20s-splash: ", stderr);
    vfprintf(stderr, fmt, ap);
    fputc('\n', stderr);
    va_end(ap);
}

static double now_s(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec / 1e9;
}

static void sleep_ms(long ms) {
    struct timespec ts = {ms / 1000, (ms % 1000) * 1000000L};
    while (nanosleep(&ts, &ts) == -1 && errno == EINTR && !stop_requested) {
    }
}

static int xioctl(int fd, unsigned long req, void *arg) {
    int r;
    do {
        r = ioctl(fd, req, arg);
    } while (r == -1 && (errno == EINTR || errno == EAGAIN));
    return r;
}

// sd_notify mínimo (protocolo documentado en sd_notify(3)): un datagrama a $NOTIFY_SOCKET.
static void notify(const char *msg) {
    const char *path = getenv("NOTIFY_SOCKET");
    if (!path || !*path)
        return;
    struct sockaddr_un sa = {.sun_family = AF_UNIX};
    size_t len = strlen(path);
    if (len >= sizeof(sa.sun_path))
        return;
    memcpy(sa.sun_path, path, len);
    if (sa.sun_path[0] == '@')
        sa.sun_path[0] = '\0'; // socket abstracto
    int s = socket(AF_UNIX, SOCK_DGRAM | SOCK_CLOEXEC, 0);
    if (s < 0)
        return;
    sendto(s, msg, strlen(msg), MSG_NOSIGNAL, (struct sockaddr *)&sa,
           (socklen_t)(offsetof(struct sockaddr_un, sun_path) + len));
    close(s);
}

// --- imagen PPM (P6, maxval 255) ---------------------------------------------------------------

struct image {
    int w, h;
    uint8_t *rgb; // w*h*3
};

static int ppm_token(FILE *f, int *out) {
    int c, v = 0, got = 0;
    for (;;) {
        c = fgetc(f);
        if (c == '#') {
            while (c != '\n' && c != EOF)
                c = fgetc(f);
            continue;
        }
        if (c == EOF)
            return -1;
        if (c == ' ' || c == '\t' || c == '\n' || c == '\r')
            continue;
        break;
    }
    while (c >= '0' && c <= '9') {
        v = v * 10 + (c - '0');
        got = 1;
        if (v > 100000)
            return -1;
        c = fgetc(f);
    }
    if (!got)
        return -1;
    // c es el separador (un solo espacio en blanco antes de los datos binarios)
    *out = v;
    return 0;
}

static int load_ppm(const char *path, struct image *img) {
    FILE *f = fopen(path, "rb");
    if (!f) {
        logmsg("no pude abrir %s: %s (se usa fondo liso)", path, strerror(errno));
        return -1;
    }
    int w, h, maxval;
    if (fgetc(f) != 'P' || fgetc(f) != '6' || ppm_token(f, &w) || ppm_token(f, &h) ||
        ppm_token(f, &maxval) || maxval != 255 || w <= 0 || h <= 0 || w > 4096 || h > 4096) {
        logmsg("%s no es un PPM P6 de 8 bits (se usa fondo liso)", path);
        fclose(f);
        return -1;
    }
    size_t n = (size_t)w * (size_t)h * 3;
    uint8_t *buf = malloc(n);
    if (!buf || fread(buf, 1, n, f) != n) {
        logmsg("%s: datos incompletos (se usa fondo liso)", path);
        free(buf);
        fclose(f);
        return -1;
    }
    fclose(f);
    img->w = w;
    img->h = h;
    img->rgb = buf;
    return 0;
}

// --- DRM ---------------------------------------------------------------------------------------

struct output {
    int fd;
    uint32_t conn_id, crtc_id;
    struct drm_mode_modeinfo mode;
    uint32_t fb_id, handle, pitch;
    uint64_t size;
    uint8_t *map;
};

static void *xcalloc(size_t n, size_t sz) {
    void *p = calloc(n ? n : 1, sz);
    if (!p) {
        logmsg("sin memoria");
        exit(1);
    }
    return p;
}

// Busca un conector conectado con modos y un CRTC para él. 0 si lo encontró.
static int find_output(int fd, struct output *o) {
    struct drm_mode_card_res res = {0};
    if (xioctl(fd, DRM_IOCTL_MODE_GETRESOURCES, &res))
        return -1;
    if (!res.count_connectors || !res.count_crtcs)
        return -1;
    uint32_t *conns = xcalloc(res.count_connectors, sizeof(uint32_t));
    uint32_t *crtcs = xcalloc(res.count_crtcs, sizeof(uint32_t));
    uint32_t *encs = xcalloc(res.count_encoders, sizeof(uint32_t));
    uint32_t *fbs = xcalloc(res.count_fbs, sizeof(uint32_t));
    res.connector_id_ptr = (uintptr_t)conns;
    res.crtc_id_ptr = (uintptr_t)crtcs;
    res.encoder_id_ptr = (uintptr_t)encs;
    res.fb_id_ptr = (uintptr_t)fbs;
    int found = -1;
    if (xioctl(fd, DRM_IOCTL_MODE_GETRESOURCES, &res))
        goto out;

    for (uint32_t i = 0; i < res.count_connectors && found; i++) {
        struct drm_mode_get_connector c = {.connector_id = conns[i]};
        if (xioctl(fd, DRM_IOCTL_MODE_GETCONNECTOR, &c))
            continue;
        if (c.connection != 1 || !c.count_modes) // 1 = DRM_MODE_CONNECTED
            continue;
        uint32_t nmodes = c.count_modes, nenc = c.count_encoders;
        struct drm_mode_modeinfo *modes = xcalloc(nmodes, sizeof(*modes));
        uint32_t *cencs = xcalloc(nenc, sizeof(uint32_t));
        uint32_t *props = xcalloc(c.count_props, sizeof(uint32_t));
        uint64_t *pvals = xcalloc(c.count_props, sizeof(uint64_t));
        c.modes_ptr = (uintptr_t)modes;
        c.encoders_ptr = (uintptr_t)cencs;
        c.props_ptr = (uintptr_t)props;
        c.prop_values_ptr = (uintptr_t)pvals;
        if (xioctl(fd, DRM_IOCTL_MODE_GETCONNECTOR, &c) || !c.count_modes || c.count_modes > nmodes ||
            c.count_encoders > nenc) {
            free(modes), free(cencs), free(props), free(pvals);
            continue;
        }

        // CRTC: el del encoder actual o el primero posible
        uint32_t crtc = 0;
        if (c.encoder_id) {
            struct drm_mode_get_encoder e = {.encoder_id = c.encoder_id};
            if (!xioctl(fd, DRM_IOCTL_MODE_GETENCODER, &e))
                crtc = e.crtc_id;
        }
        for (uint32_t k = 0; !crtc && k < c.count_encoders; k++) {
            struct drm_mode_get_encoder e = {.encoder_id = cencs[k]};
            if (xioctl(fd, DRM_IOCTL_MODE_GETENCODER, &e))
                continue;
            for (uint32_t j = 0; j < res.count_crtcs; j++)
                if (e.possible_crtcs & (1u << j)) {
                    crtc = crtcs[j];
                    break;
                }
        }
        if (crtc) {
            // Modo: el que ya tiene el CRTC (fbcon) o el preferido del panel
            struct drm_mode_crtc cur = {.crtc_id = crtc};
            if (!xioctl(fd, DRM_IOCTL_MODE_GETCRTC, &cur) && cur.mode_valid) {
                o->mode = cur.mode;
            } else {
                o->mode = modes[0];
                for (uint32_t k = 0; k < c.count_modes; k++)
                    if (modes[k].type & DRM_MODE_TYPE_PREFERRED) {
                        o->mode = modes[k];
                        break;
                    }
            }
            o->conn_id = conns[i];
            o->crtc_id = crtc;
            found = 0;
        }
        free(modes), free(cencs), free(props), free(pvals);
    }
out:
    free(conns), free(crtcs), free(encs), free(fbs);
    return found;
}

// Abre la primera /dev/dri/cardN con un conector conectado.
static int open_card(const char *forced, struct output *o) {
    char path[300];
    if (forced) {
        int fd = open(forced, O_RDWR | O_CLOEXEC);
        if (fd < 0)
            return -1;
        if (find_output(fd, o)) {
            close(fd);
            return -1;
        }
        o->fd = fd;
        return 0;
    }
    DIR *d = opendir("/dev/dri");
    if (!d)
        return -1;
    struct dirent *de;
    int ok = -1;
    while (ok && (de = readdir(d))) {
        if (strncmp(de->d_name, "card", 4))
            continue;
        snprintf(path, sizeof(path), "/dev/dri/%s", de->d_name);
        int fd = open(path, O_RDWR | O_CLOEXEC);
        if (fd < 0)
            continue;
        if (find_output(fd, o)) {
            close(fd);
            continue;
        }
        o->fd = fd;
        ok = 0;
        logmsg("%s: conector %u, CRTC %u, modo %s %ux%u", path, o->conn_id, o->crtc_id, o->mode.name,
               o->mode.hdisplay, o->mode.vdisplay);
    }
    closedir(d);
    return ok;
}

static int create_fb(struct output *o) {
    struct drm_mode_create_dumb cd = {.width = o->mode.hdisplay, .height = o->mode.vdisplay, .bpp = 32};
    if (xioctl(o->fd, DRM_IOCTL_MODE_CREATE_DUMB, &cd)) {
        logmsg("CREATE_DUMB: %s", strerror(errno));
        return -1;
    }
    o->handle = cd.handle;
    o->pitch = cd.pitch;
    o->size = cd.size;
    struct drm_mode_map_dumb md = {.handle = cd.handle};
    if (xioctl(o->fd, DRM_IOCTL_MODE_MAP_DUMB, &md)) {
        logmsg("MAP_DUMB: %s", strerror(errno));
        return -1;
    }
    o->map = mmap(NULL, o->size, PROT_READ | PROT_WRITE, MAP_SHARED, o->fd, (off_t)md.offset);
    if (o->map == MAP_FAILED) {
        logmsg("mmap: %s", strerror(errno));
        o->map = NULL;
        return -1;
    }
    struct drm_mode_fb_cmd fb = {
        .width = cd.width, .height = cd.height, .pitch = cd.pitch, .bpp = 32, .depth = 24, .handle = cd.handle};
    if (xioctl(o->fd, DRM_IOCTL_MODE_ADDFB, &fb)) {
        logmsg("ADDFB: %s", strerror(errno));
        return -1;
    }
    o->fb_id = fb.fb_id;
    return 0;
}

static void paint(struct output *o, const struct image *img) {
    uint32_t W = o->mode.hdisplay, H = o->mode.vdisplay;
    uint8_t r = 13, g = 14, b = 20; // fondo del launcher si no hay imagen
    if (img->rgb)
        r = img->rgb[0], g = img->rgb[1], b = img->rgb[2];
    uint32_t bg = 0xff000000u | (uint32_t)r << 16 | (uint32_t)g << 8 | b;
    for (uint32_t y = 0; y < H; y++) {
        uint32_t *row = (uint32_t *)(o->map + (size_t)y * o->pitch);
        for (uint32_t x = 0; x < W; x++)
            row[x] = bg;
    }
    if (!img->rgb)
        return;
    // Centrada; si es más grande que la pantalla, se recorta el centro.
    int dx = ((int)W - img->w) / 2, dy = ((int)H - img->h) / 2;
    for (int y = 0; y < img->h; y++) {
        int sy = y + dy;
        if (sy < 0 || sy >= (int)H)
            continue;
        uint32_t *row = (uint32_t *)(o->map + (size_t)sy * o->pitch);
        const uint8_t *src = img->rgb + (size_t)y * img->w * 3;
        for (int x = 0; x < img->w; x++) {
            int sx = x + dx;
            if (sx < 0 || sx >= (int)W)
                continue;
            row[sx] = 0xff000000u | (uint32_t)src[3 * x] << 16 | (uint32_t)src[3 * x + 1] << 8 | src[3 * x + 2];
        }
    }
}

static int set_crtc(struct output *o) {
    struct drm_mode_crtc c = {
        .crtc_id = o->crtc_id,
        .fb_id = o->fb_id,
        .set_connectors_ptr = (uintptr_t)&o->conn_id,
        .count_connectors = 1,
        .mode = o->mode,
        .mode_valid = 1,
    };
    return xioctl(o->fd, DRM_IOCTL_MODE_SETCRTC, &c);
}

static uint32_t current_fb(struct output *o) {
    struct drm_mode_crtc c = {.crtc_id = o->crtc_id};
    if (xioctl(o->fd, DRM_IOCTL_MODE_GETCRTC, &c))
        return 0;
    return c.fb_id;
}

static void usage(void) {
    fprintf(stderr, "uso: rgb20s-splash [-t segundos] [-w segundos_espera] [-d /dev/dri/cardN] [-m] [imagen.ppm]\n");
    exit(2);
}

int main(int argc, char **argv) {
    double max_secs = 60, wait_secs = 20;
    const char *card = NULL, *image_path = DEFAULT_IMAGE;
    int opt, keep_master = 0;
    while ((opt = getopt(argc, argv, "t:w:d:mh")) != -1) {
        switch (opt) {
        case 't': max_secs = atof(optarg); break;
        case 'w': wait_secs = atof(optarg); break;
        case 'd': card = optarg; break;
        case 'm': keep_master = 1; break;
        default: usage();
        }
    }
    if (optind < argc)
        image_path = argv[optind++];
    if (optind < argc)
        usage();

    struct sigaction sa = {.sa_handler = on_signal};
    sigaction(SIGTERM, &sa, NULL);
    sigaction(SIGINT, &sa, NULL);
    signal(SIGPIPE, SIG_IGN);

    struct image img = {0};
    load_ppm(image_path, &img);

    // 1. Esperar la tarjeta. Si no está todavía, avisar a systemd igual (no frenar el arranque):
    //    si el launcher llega primero y es master, SETCRTC falla y este proceso sale sin tocar nada.
    struct output o = {.fd = -1};
    double t0 = now_s();
    int notified = 0;
    while (open_card(card, &o)) {
        if (!notified) {
            notify("READY=1\nSTATUS=esperando /dev/dri/card*");
            notified = 1;
        }
        if (stop_requested || now_s() - t0 > wait_secs) {
            logmsg("no apareció una pantalla DRM conectada en %g s", wait_secs);
            return 0;
        }
        sleep_ms(100);
    }

    // 2. Dibujar y mostrar
    if (create_fb(&o)) {
        notify("READY=1");
        return 1;
    }
    paint(&o, &img);
    free(img.rgb);
    if (set_crtc(&o)) {
        // EACCES: otro proceso ya es DRM master (el launcher llegó antes): no molestar.
        logmsg("SETCRTC: %s (¿otra app ya tiene la pantalla?)", strerror(errno));
        notify("READY=1");
        return 0;
    }
    // 3. Soltar master para que el launcher lo tome al abrir la tarjeta
    if (keep_master) {
        logmsg("-m: conservo DRM master (modo prueba)");
    } else if (xioctl(o.fd, DRM_IOCTL_DROP_MASTER, NULL)) {
        // Sin soltar master el launcher no podría usar la pantalla: mejor salir ya.
        logmsg("DROP_MASTER: %s; salgo para no bloquear al launcher", strerror(errno));
        notify("READY=1");
        return 1;
    }
    logmsg("splash en pantalla a los %.2f s de arrancar el servicio", now_s() - t0);
    notify("READY=1\nSTATUS=splash en pantalla");

    // 4. Vigilar hasta que otra app tome la pantalla
    int reasserts = 0;
    while (!stop_requested && now_s() - t0 < max_secs) {
        sleep_ms(100);
        if (keep_master)
            continue;
        uint32_t fb = current_fb(&o);
        if (fb == o.fb_id)
            continue;
        // Cambió lo que se ve. ¿Hay un master nuevo? SET_MASTER falla (EBUSY) si lo hay.
        if (xioctl(o.fd, DRM_IOCTL_SET_MASTER, NULL)) {
            logmsg("otra app tomó la pantalla (fb %u): salgo", fb);
            return 0;
        }
        // Nadie es master: fue fbcon (texto en tty1). Volver a poner el splash.
        logmsg("fbcon tomó la pantalla (fb %u): vuelvo a poner el splash (%d)", fb, reasserts + 1);
        int bad = set_crtc(&o);
        xioctl(o.fd, DRM_IOCTL_DROP_MASTER, NULL);
        if (bad || ++reasserts > MAX_REASSERT) {
            logmsg("fbcon recupera la pantalla una y otra vez: dejo de insistir");
            return 0;
        }
    }
    if (stop_requested)
        logmsg("detenido");
    else
        logmsg("tiempo máximo (%g s) cumplido: salgo", max_secs);
    return 0;
}
