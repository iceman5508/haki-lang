/**
 * test_harness.c — Standalone test harness for Haki handler .so files.
 *
 * Verifies the mod_haki ABI without requiring Apache. Loads handler.so via
 * dlopen, builds mock HakiRequest structs, calls haki_handle_request, and
 * checks the HakiResponse. Exit 0 = all tests passed.
 *
 * Build: gcc -o test_harness test_harness.c -ldl
 * Usage: ./test_harness ./handler.so
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <dlfcn.h>
#include <assert.h>

#include "haki_abi.h"

/* ── Function pointer types ─────────────────────────────────────────────── */

typedef HakiResponse* (*haki_handle_fn)(const HakiRequest*);
typedef void          (*haki_free_fn)(HakiResponse*);
typedef void          (*haki_init_fn)(void);
typedef void          (*haki_shutdown_fn)(void);
typedef uint32_t      (*haki_ver_fn)(void);

/* ── Test helpers ────────────────────────────────────────────────────────── */

static int tests_run    = 0;
static int tests_passed = 0;
static int tests_failed = 0;

static HakiRequest make_request(const char* method, const char* path, const char* body) {
    HakiRequest req;
    memset(&req, 0, sizeof(req));
    req.abi_version  = HAKI_ABI_VERSION;
    req.method       = method;
    req.path         = path;
    req.query        = NULL;
    req.body         = body;
    req.body_len     = body ? strlen(body) : 0;
    req.remote_addr  = "127.0.0.1";
    req.header_count = 0;
    return req;
}

#define EXPECT(label, cond) do { \
    tests_run++; \
    if (cond) { tests_passed++; printf("  PASS  %s\n", label); } \
    else       { tests_failed++; printf("  FAIL  %s\n", label); } \
} while(0)

#define EXPECT_STATUS(label, resp, expected) \
    EXPECT(label ": status == " #expected, (resp) && (resp)->status == (expected))

#define EXPECT_BODY(label, resp, expected) \
    EXPECT(label ": body contains '" expected "'", \
        (resp) && (resp)->body && strstr((resp)->body, (expected)) != NULL)

/* ── Main test runner ────────────────────────────────────────────────────── */

int main(int argc, char* argv[]) {
    if (argc < 2) {
        fprintf(stderr, "usage: %s <handler.so>\n", argv[0]);
        return 1;
    }

    /* ── Load the handler .so ─────────────────────────────────────────── */
    printf("Loading %s...\n", argv[1]);
    void* dl = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
    if (!dl) {
        fprintf(stderr, "dlopen failed: %s\n", dlerror());
        return 1;
    }

    /* ── Verify ABI version ──────────────────────────────────────────── */
    haki_ver_fn ver_fn = (haki_ver_fn)dlsym(dl, "haki_abi_version");
    if (!ver_fn) {
        fprintf(stderr, "Missing symbol: haki_abi_version\n");
        dlclose(dl);
        return 1;
    }
    uint32_t handler_ver = ver_fn();
    printf("Handler ABI version: %u (expected %u)\n", handler_ver, HAKI_ABI_VERSION);
    if (handler_ver != HAKI_ABI_VERSION) {
        fprintf(stderr, "ABI version mismatch!\n");
        dlclose(dl);
        return 1;
    }

    /* ── Load required symbols ───────────────────────────────────────── */
    haki_handle_fn handle  = (haki_handle_fn)dlsym(dl, "haki_handle_request");
    haki_free_fn   freefn  = (haki_free_fn)dlsym(dl, "haki_response_free");
    haki_init_fn   initfn  = (haki_init_fn)dlsym(dl, "haki_handler_init");

    if (!handle || !freefn) {
        fprintf(stderr, "Missing required symbols (haki_handle_request, haki_response_free)\n");
        dlclose(dl);
        return 1;
    }

    /* ── Optional init hook ──────────────────────────────────────────── */
    if (initfn) {
        printf("Calling haki_handler_init()...\n");
        initfn();
    }

    printf("\nRunning tests:\n");

    /* ── Test 1: GET /health ─────────────────────────────────────────── */
    {
        HakiRequest req  = make_request("GET", "/health", NULL);
        HakiResponse* r  = handle(&req);
        EXPECT_STATUS("GET /health", r, 200);
        EXPECT_BODY("GET /health", r, "ok");
        freefn(r);
    }

    /* ── Test 2: GET /hello ──────────────────────────────────────────── */
    {
        HakiRequest req  = make_request("GET", "/hello", NULL);
        HakiResponse* r  = handle(&req);
        EXPECT_STATUS("GET /hello", r, 200);
        EXPECT_BODY("GET /hello", r, "Hello");
        freefn(r);
    }

    /* ── Test 3: GET /missing → 404 ─────────────────────────────────── */
    {
        HakiRequest req  = make_request("GET", "/missing", NULL);
        HakiResponse* r  = handle(&req);
        EXPECT_STATUS("GET /missing (404)", r, 404);
        freefn(r);
    }

    /* ── Test 4: response has content_type ──────────────────────────── */
    {
        HakiRequest req  = make_request("GET", "/health", NULL);
        HakiResponse* r  = handle(&req);
        EXPECT("content_type set", r && r->content_type && r->content_type[0]);
        freefn(r);
    }

    /* ── Test 5: NULL body doesn't crash ─────────────────────────────── */
    {
        HakiRequest req  = make_request("POST", "/health", "");
        HakiResponse* r  = handle(&req);
        EXPECT_STATUS("POST /health (no crash)", r, 200);
        freefn(r);
    }

    /* ── Test 6: concurrent calls (basic, no threading) ─────────────── */
    {
        int ok = 1;
        for (int i = 0; i < 100; i++) {
            HakiRequest req = make_request("GET", "/health", NULL);
            HakiResponse* r = handle(&req);
            if (!r || r->status != 200) { ok = 0; freefn(r); break; }
            freefn(r);
        }
        EXPECT("100 sequential calls succeed", ok);
    }

    /* ── Optional shutdown ───────────────────────────────────────────── */
    haki_shutdown_fn shutdownfn = (haki_shutdown_fn)dlsym(dl, "haki_handler_shutdown");
    if (shutdownfn) shutdownfn();

    dlclose(dl);

    /* ── Summary ─────────────────────────────────────────────────────── */
    printf("\n%d/%d tests passed", tests_passed, tests_run);
    if (tests_failed > 0) printf(" (%d FAILED)", tests_failed);
    printf("\n");

    return tests_failed > 0 ? 1 : 0;
}
