/**
 * mod_haki.c — Apache 2.4 module for Haki web handlers.
 *
 * Loads a compiled Haki handler (.so) and dispatches HTTP requests to it.
 * Haki handlers run in Apache's worker thread pool — true multithreading,
 * shared state protected by Haki's Mutex<T>.
 *
 * Apache configuration:
 *
 *   LoadModule haki_module modules/mod_haki.so
 *
 *   <Location /api>
 *       SetHandler haki-handler
 *       HakiModule /path/to/handler.so
 *   </Location>
 *
 * Build:
 *   apxs -c -i mod_haki.c -ldl
 *
 * The handler .so is compiled with:
 *   hakic --target so handler.haki -o handler.so
 */

#include "httpd.h"
#include "http_config.h"
#include "http_protocol.h"
#include "http_log.h"
#include "ap_config.h"
#include "apr_strings.h"
#include "apr_tables.h"

#include <dlfcn.h>
#include <string.h>
#include <stdlib.h>
#include <stdint.h>

#include "haki_abi.h"

/* ── Module declaration ──────────────────────────────────────────────────── */

module AP_MODULE_DECLARE_DATA haki_module;

/* ── Per-directory configuration ─────────────────────────────────────────── */

typedef struct {
    const char*  module_path;   /* Path to the handler .so */
    void*        dl_handle;     /* dlopen handle — NULL until first request */

    /* Loaded function pointers */
    HakiResponse* (*handle_request)(const HakiRequest*);
    void          (*handler_init)(void);
    void          (*handler_shutdown)(void);
    void          (*response_free)(HakiResponse*);
    uint32_t      (*abi_version)(void);
} haki_dir_config;

/* ── Configuration directives ─────────────────────────────────────────────── */

static void* haki_create_dir_config(apr_pool_t* p, char* dir) {
    haki_dir_config* cfg = apr_pcalloc(p, sizeof(haki_dir_config));
    cfg->module_path    = NULL;
    cfg->dl_handle      = NULL;
    cfg->handle_request = NULL;
    return cfg;
}

static const char* set_haki_module(cmd_parms* cmd, void* cfg_void,
                                   const char* arg) {
    haki_dir_config* cfg = (haki_dir_config*)cfg_void;
    cfg->module_path = arg;
    return NULL;
}

static const command_rec haki_cmds[] = {
    AP_INIT_TAKE1(
        "HakiModule",
        set_haki_module,
        NULL,
        ACCESS_CONF | OR_ALL,
        "Path to the compiled Haki handler .so file"
    ),
    { NULL }
};

/* ── Handler .so loading ─────────────────────────────────────────────────── */

/**
 * Load the handler .so if not already loaded.
 * Returns APR_SUCCESS or logs an error and returns a failure code.
 */
static int haki_load_module(request_rec* r, haki_dir_config* cfg) {
    if (cfg->dl_handle) return OK;  /* already loaded */

    if (!cfg->module_path) {
        ap_log_rerror(APLOG_MARK, APLOG_ERR, 0, r,
            "mod_haki: HakiModule not configured for this location");
        return HTTP_INTERNAL_SERVER_ERROR;
    }

    /* Load the shared library */
    cfg->dl_handle = dlopen(cfg->module_path, RTLD_NOW | RTLD_LOCAL);
    if (!cfg->dl_handle) {
        ap_log_rerror(APLOG_MARK, APLOG_ERR, 0, r,
            "mod_haki: dlopen(%s) failed: %s", cfg->module_path, dlerror());
        return HTTP_INTERNAL_SERVER_ERROR;
    }

    /* Verify ABI version */
    typedef uint32_t (*abi_version_fn)(void);
    abi_version_fn ver_fn = (abi_version_fn)dlsym(cfg->dl_handle, "haki_abi_version");
    if (ver_fn) {
        uint32_t handler_ver = ver_fn();
        if (handler_ver != HAKI_ABI_VERSION) {
            ap_log_rerror(APLOG_MARK, APLOG_ERR, 0, r,
                "mod_haki: ABI version mismatch — mod_haki expects %u, "
                "handler exports %u. Recompile the handler.",
                HAKI_ABI_VERSION, handler_ver);
            dlclose(cfg->dl_handle);
            cfg->dl_handle = NULL;
            return HTTP_INTERNAL_SERVER_ERROR;
        }
    }

    /* Load required symbol */
    cfg->handle_request = (HakiResponse*(*)(const HakiRequest*))
        dlsym(cfg->dl_handle, "haki_handle_request");
    if (!cfg->handle_request) {
        ap_log_rerror(APLOG_MARK, APLOG_ERR, 0, r,
            "mod_haki: %s does not export haki_handle_request: %s",
            cfg->module_path, dlerror());
        dlclose(cfg->dl_handle);
        cfg->dl_handle = NULL;
        return HTTP_INTERNAL_SERVER_ERROR;
    }

    /* Load response_free — required */
    cfg->response_free = (void(*)(HakiResponse*))
        dlsym(cfg->dl_handle, "haki_response_free");
    if (!cfg->response_free) {
        ap_log_rerror(APLOG_MARK, APLOG_ERR, 0, r,
            "mod_haki: %s does not export haki_response_free",
            cfg->module_path);
        dlclose(cfg->dl_handle);
        cfg->dl_handle = NULL;
        return HTTP_INTERNAL_SERVER_ERROR;
    }

    /* Load optional lifecycle hooks */
    cfg->handler_init     = (void(*)(void))dlsym(cfg->dl_handle, "haki_handler_init");
    cfg->handler_shutdown = (void(*)(void))dlsym(cfg->dl_handle, "haki_handler_shutdown");

    /* Call init if present */
    if (cfg->handler_init) {
        cfg->handler_init();
        ap_log_rerror(APLOG_MARK, APLOG_INFO, 0, r,
            "mod_haki: loaded %s, haki_handler_init() called",
            cfg->module_path);
    } else {
        ap_log_rerror(APLOG_MARK, APLOG_INFO, 0, r,
            "mod_haki: loaded %s", cfg->module_path);
    }

    return OK;
}

/* ── Request → HakiRequest conversion ───────────────────────────────────── */

static HakiRequest* build_haki_request(request_rec* r, apr_pool_t* p) {
    HakiRequest* req = (HakiRequest*)apr_pcalloc(p, sizeof(HakiRequest));

    req->abi_version = HAKI_ABI_VERSION;
    req->method      = r->method;
    req->path        = r->uri;
    req->query       = r->args;   /* NULL if no query string */
    req->remote_addr = r->useragent_ip;

    /* Copy request headers */
    req->header_count = 0;
    const apr_array_header_t* hdrs = apr_table_elts(r->headers_in);
    const apr_table_entry_t*  elts = (const apr_table_entry_t*)hdrs->elts;
    for (int i = 0; i < hdrs->nelts && req->header_count < HAKI_MAX_HEADERS; i++) {
        req->headers[req->header_count].name  = elts[i].key;
        req->headers[req->header_count].value = elts[i].val;
        req->header_count++;
    }

    /* Read request body */
    if (ap_should_client_block(r)) {
        char buf[8192];
        apr_size_t total = 0;
        char* body_buf  = NULL;

        int rc = ap_setup_client_block(r, REQUEST_CHUNKED_DECHUNK);
        if (rc == OK) {
            while (1) {
                long n = ap_get_client_block(r, buf, sizeof(buf));
                if (n <= 0) break;
                if (total + n > HAKI_MAX_BODY_SIZE) break;

                body_buf = (char*)realloc(body_buf, total + n);
                if (!body_buf) break;
                memcpy(body_buf + total, buf, n);
                total += n;
            }
        }
        req->body     = body_buf;
        req->body_len = total;
        /* body_buf is not in the pool — we free it manually below if needed.
         * For simplicity, register a cleanup with the pool. */
        if (body_buf) {
            apr_pool_cleanup_register(p, body_buf,
                (apr_status_t(*)(void*))free,
                apr_pool_cleanup_null);
        }
    }

    return req;
}

/* ── HakiResponse → Apache response ──────────────────────────────────────── */

static int send_haki_response(request_rec* r, HakiResponse* resp,
                              haki_dir_config* cfg) {
    /* Status */
    r->status = resp->status;

    /* Content-Type */
    if (resp->content_type && resp->content_type[0]) {
        ap_set_content_type(r, resp->content_type);
    } else {
        ap_set_content_type(r, "text/plain; charset=utf-8");
    }

    /* Additional response headers */
    for (uint32_t i = 0; i < resp->header_count; i++) {
        apr_table_set(r->headers_out,
            resp->headers[i].name,
            resp->headers[i].value);
    }

    /* Body */
    if (resp->body && resp->body_len > 0) {
        ap_rwrite(resp->body, (int)resp->body_len, r);
    }

    /* Free the response — handler .so owns this memory */
    cfg->response_free(resp);

    return OK;
}

/* ── Main request handler ────────────────────────────────────────────────── */

static int haki_handler(request_rec* r) {
    /* Only handle requests directed at us */
    if (!r->handler || strcmp(r->handler, "haki-handler") != 0) {
        return DECLINED;
    }

    /* Get per-directory config */
    haki_dir_config* cfg = (haki_dir_config*)
        ap_get_module_config(r->per_dir_config, &haki_module);

    /* Load handler .so on first request (lazy) */
    int load_rc = haki_load_module(r, cfg);
    if (load_rc != OK) return load_rc;

    /* HEAD requests: same as GET but suppress body */
    const char* effective_method = r->method;
    int is_head = (strcmp(r->method, "HEAD") == 0);

    /* Build request struct from Apache's request_rec */
    apr_pool_t* req_pool;
    apr_pool_create(&req_pool, r->pool);
    HakiRequest* haki_req = build_haki_request(r, req_pool);

    /* Call the Haki handler — runs in Apache's thread pool */
    HakiResponse* haki_resp = cfg->handle_request(haki_req);

    /* Guard against NULL (shouldn't happen — haki_response_new returns 500) */
    if (!haki_resp) {
        ap_log_rerror(APLOG_MARK, APLOG_ERR, 0, r,
            "mod_haki: haki_handle_request returned NULL");
        apr_pool_destroy(req_pool);
        return HTTP_INTERNAL_SERVER_ERROR;
    }

    /* For HEAD: clear the body but keep headers/status */
    if (is_head) {
        free(haki_resp->body);
        haki_resp->body     = NULL;
        haki_resp->body_len = 0;
    }

    int rc = send_haki_response(r, haki_resp, cfg);
    apr_pool_destroy(req_pool);
    return rc;
}

/* ── Module lifecycle ────────────────────────────────────────────────────── */

static void haki_register_hooks(apr_pool_t* p) {
    ap_hook_handler(haki_handler, NULL, NULL, APR_HOOK_MIDDLE);
}

/* ── Module descriptor ───────────────────────────────────────────────────── */

module AP_MODULE_DECLARE_DATA haki_module = {
    STANDARD20_MODULE_STUFF,
    haki_create_dir_config,  /* per-directory config creator */
    NULL,                    /* per-directory config merger  */
    NULL,                    /* per-server config creator    */
    NULL,                    /* per-server config merger     */
    haki_cmds,               /* configuration directives     */
    haki_register_hooks,     /* hook registration            */
};
