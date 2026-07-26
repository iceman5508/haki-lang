/**
 * haki_fastcgi.c — FastCGI adapter for Haki web handlers.
 *
 * Runs as a persistent daemon that accepts FastCGI connections over a Unix
 * socket. nginx, Caddy, lighttpd, or any FastCGI-capable server can proxy
 * requests to it.
 *
 * Usage:
 *   hakic --target so handler.haki -o handler.so
 *   haki_fastcgi --socket /var/run/haki-handler.sock --handler ./handler.so
 *
 * nginx config:
 *   location /api {
 *       fastcgi_pass unix:/var/run/haki-handler.sock;
 *       include fastcgi_params;
 *   }
 *
 * Caddy config:
 *   reverse_proxy /api unix//var/run/haki-handler.sock {
 *       transport fastcgi
 *   }
 *
 * Build:
 *   gcc -O2 -lpthread haki_fastcgi.c -o haki_fastcgi -ldl
 *
 * The same handler.so compiled for mod_haki works here without recompilation.
 * This is the key benefit of the server-agnostic ABI design.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <unistd.h>
#include <signal.h>
#include <errno.h>
#include <dlfcn.h>

#include <sys/socket.h>
#include <sys/un.h>
#include <sys/stat.h>
#include <pthread.h>
#include <netinet/in.h>

#include "haki_abi.h"

/* ── FastCGI protocol constants ─────────────────────────────────────────── */

#define FCGI_VERSION_1       1
#define FCGI_BEGIN_REQUEST   1
#define FCGI_ABORT_REQUEST   2
#define FCGI_END_REQUEST     3
#define FCGI_PARAMS          4
#define FCGI_STDIN           5
#define FCGI_STDOUT          6
#define FCGI_STDERR          7
#define FCGI_DATA            8
#define FCGI_GET_VALUES      9
#define FCGI_GET_VALUES_RESULT 10
#define FCGI_UNKNOWN_TYPE    11

#define FCGI_RESPONDER       1
#define FCGI_AUTHORIZER      2
#define FCGI_FILTER          3

#define FCGI_REQUEST_COMPLETE 0
#define FCGI_CANT_MPX_CONN    1
#define FCGI_OVERLOADED       2
#define FCGI_UNKNOWN_ROLE     3

/* ── FastCGI wire structs ────────────────────────────────────────────────── */

#pragma pack(push, 1)

typedef struct {
    uint8_t  version;
    uint8_t  type;
    uint16_t requestId;    /* big-endian */
    uint16_t contentLength; /* big-endian */
    uint8_t  paddingLength;
    uint8_t  reserved;
} FCGI_Header;

typedef struct {
    uint16_t role;         /* big-endian */
    uint8_t  flags;
    uint8_t  reserved[5];
} FCGI_BeginRequestBody;

typedef struct {
    uint32_t appStatus;    /* big-endian */
    uint8_t  protocolStatus;
    uint8_t  reserved[3];
} FCGI_EndRequestBody;

#pragma pack(pop)

/* ── Handler function pointers ──────────────────────────────────────────── */

typedef HakiResponse* (*haki_handle_request_fn)(const HakiRequest*);
typedef void          (*haki_response_free_fn)(HakiResponse*);

static haki_handle_request_fn g_handle_fn  = NULL;
static haki_response_free_fn  g_free_fn    = NULL;
static void*                  g_dl_handle  = NULL;

/* ── Worker thread state ─────────────────────────────────────────────────── */

#define MAX_THREADS  32
#define PARAM_BUF  (64 * 1024)
#define BODY_BUF   (16 * 1024 * 1024)

typedef struct {
    int client_fd;
} WorkerArgs;

/* ── Byte-order helpers ──────────────────────────────────────────────────── */

static uint16_t be16(uint16_t x) {
    return ((x & 0xFF) << 8) | ((x >> 8) & 0xFF);
}
static uint32_t be32(uint32_t x) {
    return ((x & 0xFF) << 24) | (((x >> 8) & 0xFF) << 16) |
           (((x >> 16) & 0xFF) << 8) | ((x >> 24) & 0xFF);
}

/* ── FastCGI I/O helpers ─────────────────────────────────────────────────── */

static int read_exact(int fd, void* buf, size_t n) {
    size_t done = 0;
    while (done < n) {
        ssize_t r = read(fd, (char*)buf + done, n - done);
        if (r <= 0) return -1;
        done += r;
    }
    return 0;
}

static int write_exact(int fd, const void* buf, size_t n) {
    size_t done = 0;
    while (done < n) {
        ssize_t w = write(fd, (const char*)buf + done, n - done);
        if (w <= 0) return -1;
        done += w;
    }
    return 0;
}

static int send_record(int fd, uint8_t type, uint16_t req_id,
                       const void* data, uint16_t len) {
    FCGI_Header hdr;
    hdr.version       = FCGI_VERSION_1;
    hdr.type          = type;
    hdr.requestId     = be16(req_id);
    hdr.contentLength = be16(len);
    hdr.paddingLength = 0;
    hdr.reserved      = 0;
    if (write_exact(fd, &hdr, sizeof(hdr)) < 0) return -1;
    if (len > 0 && write_exact(fd, data, len) < 0) return -1;
    return 0;
}

/* ── FastCGI name-value pair parser ─────────────────────────────────────── */

typedef struct { char name[256]; char value[4096]; } FCGIParam;

static int parse_params(const uint8_t* buf, int len,
                        FCGIParam* params, int max_params, int* count) {
    int i = 0, n = 0;
    while (i < len && n < max_params) {
        uint32_t name_len, val_len;
        if (buf[i] >> 7) {
            if (i + 4 > len) return -1;
            name_len = be32(*(uint32_t*)(buf + i)) & 0x7FFFFFFF;
            i += 4;
        } else {
            name_len = buf[i++];
        }
        if (buf[i] >> 7) {
            if (i + 4 > len) return -1;
            val_len = be32(*(uint32_t*)(buf + i)) & 0x7FFFFFFF;
            i += 4;
        } else {
            val_len = buf[i++];
        }
        if (i + (int)name_len + (int)val_len > len) return -1;
        size_t nl = name_len < sizeof(params[n].name) - 1 ? name_len : sizeof(params[n].name) - 1;
        size_t vl = val_len  < sizeof(params[n].value) - 1 ? val_len : sizeof(params[n].value) - 1;
        memcpy(params[n].name,  buf + i,           nl); params[n].name[nl]  = 0;
        memcpy(params[n].value, buf + i + name_len, vl); params[n].value[vl] = 0;
        i += name_len + val_len;
        n++;
    }
    *count = n;
    return 0;
}

static const char* param_get(FCGIParam* params, int count, const char* name) {
    for (int i = 0; i < count; i++) {
        if (strcmp(params[i].name, name) == 0) return params[i].value;
    }
    return "";
}

/* ── Request processing ──────────────────────────────────────────────────── */

static void handle_connection(int client_fd) {
    FCGIParam params[HAKI_MAX_HEADERS];
    int       param_count = 0;
    char*     body_buf    = NULL;
    size_t    body_len    = 0;
    uint16_t  request_id  = 0;

    uint8_t param_buf[PARAM_BUF];
    size_t  param_buf_len = 0;

    /* Read all FastCGI records for this request */
    while (1) {
        FCGI_Header hdr;
        if (read_exact(client_fd, &hdr, sizeof(hdr)) < 0) goto done;

        uint16_t content_len = be16(hdr.contentLength);
        uint8_t  padding_len = hdr.paddingLength;

        uint8_t* content = NULL;
        if (content_len > 0) {
            content = (uint8_t*)malloc(content_len);
            if (!content) goto done;
            if (read_exact(client_fd, content, content_len) < 0) { free(content); goto done; }
        }
        /* Skip padding */
        if (padding_len > 0) {
            uint8_t pad[256];
            read_exact(client_fd, pad, padding_len);
        }

        request_id = be16(hdr.requestId);

        switch (hdr.type) {
            case FCGI_BEGIN_REQUEST:
                /* Nothing to do — we handle RESPONDER role only */
                break;

            case FCGI_PARAMS:
                if (content_len == 0) {
                    /* End of params — parse accumulated buffer */
                    parse_params(param_buf, (int)param_buf_len,
                                 params, HAKI_MAX_HEADERS, &param_count);
                } else if (param_buf_len + content_len <= PARAM_BUF) {
                    memcpy(param_buf + param_buf_len, content, content_len);
                    param_buf_len += content_len;
                }
                break;

            case FCGI_STDIN:
                if (content_len == 0) {
                    /* End of stdin — all data received, process request */
                    goto process;
                } else {
                    body_buf = (char*)realloc(body_buf, body_len + content_len + 1);
                    if (body_buf) {
                        memcpy(body_buf + body_len, content, content_len);
                        body_len += content_len;
                        body_buf[body_len] = 0;
                    }
                }
                break;

            case FCGI_ABORT_REQUEST:
                goto done;
        }
        free(content);
        continue;
process:
        free(content);
        break;
    }

    /* ── Build HakiRequest from CGI environment ──────────────────────── */
    HakiRequest req;
    memset(&req, 0, sizeof(req));
    req.abi_version = HAKI_ABI_VERSION;
    req.method      = param_get(params, param_count, "REQUEST_METHOD");
    req.path        = param_get(params, param_count, "DOCUMENT_URI");
    if (!req.path || !req.path[0])
        req.path    = param_get(params, param_count, "REQUEST_URI");
    req.query       = param_get(params, param_count, "QUERY_STRING");
    req.remote_addr = param_get(params, param_count, "REMOTE_ADDR");
    req.body        = body_buf ? body_buf : "";
    req.body_len    = body_len;

    /* Copy HTTP_ params as headers */
    uint32_t hdr_count = 0;
    for (int i = 0; i < param_count && hdr_count < HAKI_MAX_HEADERS; i++) {
        if (strncmp(params[i].name, "HTTP_", 5) == 0) {
            req.headers[hdr_count].name  = params[i].name + 5;
            req.headers[hdr_count].value = params[i].value;
            hdr_count++;
        }
    }
    req.header_count = hdr_count;

    /* ── Call Haki handler ─────────────────────────────────────────── */
    HakiResponse* resp = g_handle_fn(&req);

    /* ── Send FastCGI STDOUT with HTTP response ─────────────────────── */
    if (resp) {
        /* HTTP response headers */
        char header_buf[4096];
        int  header_len = snprintf(header_buf, sizeof(header_buf),
            "Status: %d\r\nContent-Type: %s\r\nContent-Length: %zu\r\n\r\n",
            resp->status,
            resp->content_type ? resp->content_type : "text/plain; charset=utf-8",
            resp->body_len);

        send_record(client_fd, FCGI_STDOUT, request_id,
                    header_buf, (uint16_t)header_len);

        if (resp->body && resp->body_len > 0) {
            /* Send body in 64KB chunks */
            size_t sent = 0;
            while (sent < resp->body_len) {
                size_t chunk = resp->body_len - sent;
                if (chunk > 65535) chunk = 65535;
                send_record(client_fd, FCGI_STDOUT, request_id,
                            resp->body + sent, (uint16_t)chunk);
                sent += chunk;
            }
        }
        /* End of STDOUT */
        send_record(client_fd, FCGI_STDOUT, request_id, NULL, 0);
        g_free_fn(resp);
    }

    /* ── Send END_REQUEST ──────────────────────────────────────────── */
    FCGI_EndRequestBody end;
    end.appStatus      = be32(0);
    end.protocolStatus = FCGI_REQUEST_COMPLETE;
    memset(end.reserved, 0, sizeof(end.reserved));
    send_record(client_fd, FCGI_END_REQUEST, request_id, &end, sizeof(end));

done:
    free(body_buf);
    close(client_fd);
}

/* ── Worker thread ───────────────────────────────────────────────────────── */

static void* worker_thread(void* arg) {
    WorkerArgs* wa = (WorkerArgs*)arg;
    handle_connection(wa->client_fd);
    free(wa);
    return NULL;
}

/* ── Usage / argument parsing ────────────────────────────────────────────── */

static void usage(const char* prog) {
    fprintf(stderr,
        "Usage: %s --handler <handler.so> --socket <path> [options]\n"
        "\n"
        "Options:\n"
        "  --handler <path>   Path to compiled Haki handler .so\n"
        "  --socket  <path>   Unix socket path (default: /var/run/haki.sock)\n"
        "  --threads <n>      Max worker threads (default: 8)\n"
        "  --help             Show this message\n"
        "\n"
        "nginx config:\n"
        "  fastcgi_pass unix:/var/run/haki.sock;\n"
        "  include fastcgi_params;\n",
        prog);
}

/* ── Main ────────────────────────────────────────────────────────────────── */

int main(int argc, char* argv[]) {
    const char* handler_path = NULL;
    const char* socket_path  = "/var/run/haki.sock";
    int         max_threads  = 8;

    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--handler") == 0 && i + 1 < argc) {
            handler_path = argv[++i];
        } else if (strcmp(argv[i], "--socket") == 0 && i + 1 < argc) {
            socket_path = argv[++i];
        } else if (strcmp(argv[i], "--threads") == 0 && i + 1 < argc) {
            max_threads = atoi(argv[++i]);
        } else if (strcmp(argv[i], "--help") == 0) {
            usage(argv[0]); return 0;
        }
    }

    if (!handler_path) { usage(argv[0]); return 1; }

    /* ── Load handler .so ──────────────────────────────────────────── */
    g_dl_handle = dlopen(handler_path, RTLD_NOW | RTLD_LOCAL);
    if (!g_dl_handle) {
        fprintf(stderr, "dlopen(%s): %s\n", handler_path, dlerror());
        return 1;
    }

    /* ABI version check */
    typedef uint32_t (*ver_fn_t)(void);
    ver_fn_t ver_fn = (ver_fn_t)dlsym(g_dl_handle, "haki_abi_version");
    if (ver_fn && ver_fn() != HAKI_ABI_VERSION) {
        fprintf(stderr, "ABI version mismatch: handler=%u, fastcgi=%u\n",
                ver_fn(), HAKI_ABI_VERSION);
        return 1;
    }

    g_handle_fn = (haki_handle_request_fn)dlsym(g_dl_handle, "haki_handle_request");
    g_free_fn   = (haki_response_free_fn)dlsym(g_dl_handle, "haki_response_free");
    if (!g_handle_fn || !g_free_fn) {
        fprintf(stderr, "Missing required symbols in %s\n", handler_path);
        return 1;
    }

    /* Optional init */
    typedef void (*init_fn_t)(void);
    init_fn_t init_fn = (init_fn_t)dlsym(g_dl_handle, "haki_handler_init");
    if (init_fn) init_fn();

    /* ── Create Unix socket ────────────────────────────────────────── */
    unlink(socket_path);  /* remove stale socket if present */

    int srv_fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (srv_fd < 0) { perror("socket"); return 1; }

    struct sockaddr_un addr;
    memset(&addr, 0, sizeof(addr));
    addr.sun_family = AF_UNIX;
    strncpy(addr.sun_path, socket_path, sizeof(addr.sun_path) - 1);

    if (bind(srv_fd, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        perror("bind"); return 1;
    }
    chmod(socket_path, 0666);  /* allow web server user to connect */

    if (listen(srv_fd, 128) < 0) { perror("listen"); return 1; }

    /* Ignore SIGPIPE — connections may close mid-write */
    signal(SIGPIPE, SIG_IGN);

    fprintf(stderr, "haki_fastcgi: listening on %s (max %d threads)\n",
            socket_path, max_threads);

    /* ── Accept loop ───────────────────────────────────────────────── */
    while (1) {
        int client_fd = accept(srv_fd, NULL, NULL);
        if (client_fd < 0) {
            if (errno == EINTR) continue;
            perror("accept");
            break;
        }

        WorkerArgs* wa = (WorkerArgs*)malloc(sizeof(WorkerArgs));
        if (!wa) { close(client_fd); continue; }
        wa->client_fd = client_fd;

        pthread_t tid;
        pthread_attr_t attr;
        pthread_attr_init(&attr);
        pthread_attr_setdetachstate(&attr, PTHREAD_CREATE_DETACHED);
        if (pthread_create(&tid, &attr, worker_thread, wa) != 0) {
            /* Thread creation failed — handle synchronously */
            handle_connection(client_fd);
            free(wa);
        }
        pthread_attr_destroy(&attr);
    }

    close(srv_fd);
    unlink(socket_path);
    dlclose(g_dl_handle);
    return 0;
}
