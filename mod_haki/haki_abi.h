/**
 * haki_abi.h — Stable C ABI for Haki web handlers.
 *
 * This header defines the memory contract between:
 *   - mod_haki (Apache/nginx module) which calls haki_handle_request()
 *   - Haki handler .so libraries which implement haki_handle_request()
 *   - hakic --target so which emits code that fills these structs
 *
 * OWNERSHIP RULES:
 *   - HakiRequest is allocated by the caller (mod_haki) and freed by the caller.
 *     The handler must not free it or hold pointers into it past the call.
 *   - HakiResponse is allocated by haki_handle_request() via malloc().
 *     The caller must free it using haki_response_free() after consuming it.
 *   - All strings are null-terminated and owned by their containing struct.
 *
 * THREAD SAFETY:
 *   haki_handle_request() may be called from multiple threads simultaneously.
 *   The handler is responsible for protecting any shared mutable state with
 *   Haki's Mutex<T> — the ABI itself imposes no locks.
 *
 * VERSIONING:
 *   HAKI_ABI_VERSION must match between mod_haki and the loaded .so.
 *   mod_haki checks this at load time and refuses mismatched handlers.
 */

#ifndef HAKI_ABI_H
#define HAKI_ABI_H

#include <stdint.h>
#include <stddef.h>

/* ABI version — bump when the struct layout changes. */
#define HAKI_ABI_VERSION 1

/* Maximum number of request/response headers. */
#define HAKI_MAX_HEADERS 64

/* Maximum request body size accepted by mod_haki (16 MB). */
#define HAKI_MAX_BODY_SIZE (16 * 1024 * 1024)

/* ── HTTP header ─────────────────────────────────────────────────────────── */

typedef struct {
    const char* name;   /* header name, e.g. "Content-Type" */
    const char* value;  /* header value, e.g. "application/json" */
} HakiHeader;

/* ── Request ─────────────────────────────────────────────────────────────── */

/**
 * HakiRequest — describes an incoming HTTP request.
 *
 * All char* fields point into memory owned by this struct (or by the
 * Apache request_rec pool). The handler must not free them.
 */
typedef struct {
    /* ABI version — set by mod_haki before calling the handler. */
    uint32_t        abi_version;

    /* HTTP method: "GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", etc. */
    const char*     method;

    /* Request path, e.g. "/users/42". Does not include query string. */
    const char*     path;

    /* Full query string, e.g. "page=2&sort=name". NULL if none. */
    const char*     query;

    /* Request body. NULL if no body (GET requests etc.). Not NUL-terminated
     * — use body_len to determine size. */
    const char*     body;
    size_t          body_len;

    /* Request headers. */
    HakiHeader      headers[HAKI_MAX_HEADERS];
    uint32_t        header_count;

    /* Remote IP address of the client, e.g. "192.168.1.1". */
    const char*     remote_addr;
} HakiRequest;

/* ── Response ────────────────────────────────────────────────────────────── */

/**
 * HakiResponse — describes the HTTP response to send.
 *
 * Allocated by haki_handle_request() via malloc(). The caller (mod_haki)
 * reads it and then calls haki_response_free() to release it.
 */
typedef struct {
    /* HTTP status code, e.g. 200, 404, 500. */
    int32_t         status;

    /* Response body. Must be a malloc'd string. Freed by haki_response_free(). */
    char*           body;
    size_t          body_len;

    /* Content-Type header value, e.g. "text/html; charset=utf-8".
     * Must be a malloc'd string. Freed by haki_response_free(). */
    char*           content_type;

    /* Additional response headers (beyond Content-Type). */
    HakiHeader      headers[HAKI_MAX_HEADERS];
    uint32_t        header_count;
} HakiResponse;

/* ── Handler entry point ─────────────────────────────────────────────────── */

/**
 * haki_handle_request — the symbol every handler .so must export.
 *
 * Called once per HTTP request by mod_haki. Must be thread-safe.
 *
 * @param req  Incoming request. Owned by caller; do not free.
 * @return     Heap-allocated response. Caller frees via haki_response_free().
 *             Must not return NULL — return a 500 response on internal error.
 */
HakiResponse* haki_handle_request(const HakiRequest* req);

/* ── Lifecycle ───────────────────────────────────────────────────────────── */

/**
 * haki_handler_init — called once when the .so is loaded (optional).
 *
 * Use to initialize shared state: connection pools, caches, etc.
 * If this symbol is absent, mod_haki silently skips it.
 */
void haki_handler_init(void);

/**
 * haki_handler_shutdown — called once when the .so is unloaded (optional).
 *
 * Use to flush state, close connections, etc.
 * If this symbol is absent, mod_haki silently skips it.
 */
void haki_handler_shutdown(void);

/* ── Memory management ───────────────────────────────────────────────────── */

/**
 * haki_response_free — release a HakiResponse returned by haki_handle_request.
 *
 * Must be called by mod_haki after it has consumed the response.
 * This function is exported by the handler .so (it knows its own allocation).
 */
void haki_response_free(HakiResponse* resp);

/**
 * haki_response_new — allocate and zero-initialize a HakiResponse.
 *
 * Convenience for handler code. Sets status=500 as a safe default.
 * Caller must fill in status, body, content_type before returning.
 */
static inline HakiResponse* haki_response_new(void) {
    HakiResponse* r = (HakiResponse*)malloc(sizeof(HakiResponse));
    if (!r) return NULL;
    r->status       = 500;
    r->body         = NULL;
    r->body_len     = 0;
    r->content_type = NULL;
    r->header_count = 0;
    return r;
}

/**
 * haki_response_set_body — set the response body from a C string.
 *
 * Copies the string — safe to pass stack-allocated strings.
 * Frees any previous body.
 */
static inline void haki_response_set_body(HakiResponse* r, const char* body) {
    free(r->body);
    if (!body) { r->body = NULL; r->body_len = 0; return; }
    size_t len = strlen(body);
    r->body = (char*)malloc(len + 1);
    if (r->body) { memcpy(r->body, body, len + 1); r->body_len = len; }
}

/**
 * haki_abi_version — returns the ABI version baked into this header.
 *
 * mod_haki calls this after loading the .so and refuses to proceed if
 * the version doesn't match HAKI_ABI_VERSION.
 */
static inline uint32_t haki_abi_version(void) {
    return HAKI_ABI_VERSION;
}

#endif /* HAKI_ABI_H */
