/// runtime.rs — The Haki v0.1 C runtime source.
///
/// This is emitted as `haki_runtime.c` alongside the compiled `.ll` file.
/// Compile and link:
///   clang -c haki_runtime.c -o haki_runtime.o
///   clang your_module.ll haki_runtime.o -o your_program
///
/// The runtime provides:
///   - ARC retain/release/alloc/weak_load
///   - print / print_int / print_float / print_bool
///   - String utilities: concat, length, int_to_string, etc.
///   - panic handler


pub const CORE_RUNTIME_C_SOURCE: &str = r#"
/* haki_runtime.c — Haki v0.1 runtime
   Compile: clang -c haki_runtime.c -o haki_runtime.o               */

/* Closure fat pointer: { fn_ptr, env_ptr } packed into void*[2] on heap */
#include <stdlib.h>
static inline void* haki_make_closure(void* fn_ptr, void* env_ptr) {
    void** fat = (void**)malloc(2 * sizeof(void*));
    fat[0] = fn_ptr;
    fat[1] = env_ptr;
    return (void*)fat;
}

/* Multi-field enum payload structs used by generated match arms */
/* f0/f1 etc store pointers to the actual values (indirection level) */
typedef struct { void* f0; void* f1; } __PayloadTuple2;
typedef struct { void* f0; void* f1; void* f2; } __PayloadTuple3;
typedef struct { void* f0; void* f1; void* f2; void* f3; } __PayloadTuple4;
/* Helper: dereference a void* as a string (char**) */
#define __HAKI_STR(p) (*(const char**)(p))
#define __HAKI_I64(p) (*(int64_t*)(p))
#define __HAKI_FN(p)  (*(void**)(p))

#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <stdatomic.h>
#include <ctype.h>
#include <errno.h>

/* ── ARC ─────────────────────────────────────────────────────────── */

typedef struct {
    _Atomic int64_t refcount;
    void (*destructor)(void*);
} HakiObjHeader;

void haki_retain(void* obj) {
    if (!obj) return;
    atomic_fetch_add_explicit(
        &((HakiObjHeader*)obj)->refcount, 1, memory_order_relaxed);
}

void haki_release(void* obj) {
    if (!obj) return;
    HakiObjHeader* h = (HakiObjHeader*)obj;
    int64_t prev = atomic_fetch_sub_explicit(
        &h->refcount, 1, memory_order_acq_rel);
    if (prev == 1) {
        if (h->destructor) h->destructor(obj);
        free(obj);
    }
}

void* haki_alloc(int64_t size) {
    void* p = calloc(1, (size_t)size);
    if (!p) { fprintf(stderr, "haki: out of memory\n"); abort(); }
    atomic_store_explicit(
        &((HakiObjHeader*)p)->refcount, 1, memory_order_relaxed);
    return p;
}

void* haki_weak_load(void** slot) {
    void* obj = *slot;
    if (!obj) return NULL;
    int64_t rc = atomic_load_explicit(
        &((HakiObjHeader*)obj)->refcount, memory_order_acquire);
    return (rc > 0) ? obj : NULL;
}

/* ── Print ───────────────────────────────────────────────────────── */

void haki_print(const char* msg) {
    if (!msg) msg = "(null)";
    puts(msg);
}

void haki_print_int(int64_t n) {
    printf("%lld\n", (long long)n);
}

void haki_print_float(double f) {
    printf("%g\n", f);
}

void haki_print_bool(int8_t b) {
    puts(b ? "true" : "false");
}

/* ── String utilities ────────────────────────────────────────────── */

void* haki_string_concat(const char* a, const char* b) {
    if (!a) a = "";
    if (!b) b = "";
    size_t la = strlen(a), lb = strlen(b);
    char* r = (char*)malloc(la + lb + 1);
    if (!r) abort();
    memcpy(r, a, la);
    memcpy(r + la, b, lb + 1);
    return r;
}

int64_t haki_string_length(const char* s) {
    return s ? (int64_t)strlen(s) : 0;
}

/* Returns a heap-allocated string — caller owns it. */
void* haki_int_to_string(int64_t n) {
    char buf[32];
    snprintf(buf, sizeof(buf), "%lld", (long long)n);
    return strdup(buf);
}

void* haki_float_to_string(double f) {
    char buf[64];
    snprintf(buf, sizeof(buf), "%g", f);
    return strdup(buf);
}

void* haki_bool_to_string(int8_t b) {
    return strdup(b ? "true" : "false");
}

/* ── Panic ───────────────────────────────────────────────────────── */

void haki_panic(const char* msg) {
    fflush(stdout);  /* flush buffered output before aborting */
    fprintf(stderr, "\nhaki panic: %s\n", msg ? msg : "(no message)");
    abort();
}

/* ── Array<T> ───────────────────────────────────────────────────── */

typedef struct {
    int64_t length;
    int64_t capacity;
    void*   data;
    int64_t elem_size;
} HakiArray;

HakiArray* haki_array_new(int64_t elem_size) {
    HakiArray* a = (HakiArray*)malloc(sizeof(HakiArray));
    if (!a) abort();
    a->length    = 0;
    a->capacity  = 8;
    a->elem_size = elem_size;
    a->data      = malloc((size_t)(8 * elem_size));
    if (!a->data) abort();
    return a;
}

HakiArray* haki_array_from(void* src, int64_t count, int64_t elem_size) {
    HakiArray* a = haki_array_new(elem_size);
    a->length   = count;
    a->capacity = count > 8 ? count : 8;
    free(a->data);
    a->data = malloc((size_t)(a->capacity * elem_size));
    if (!a->data) abort();
    if (src && count > 0) memcpy(a->data, src, (size_t)(count * elem_size));
    return a;
}

void haki_array_append(HakiArray* a, void* item) {
    if (!a) return;
    if (a->length == a->capacity) {
        a->capacity *= 2;
        a->data = realloc(a->data, (size_t)(a->capacity * a->elem_size));
        if (!a->data) abort();
    }
    memcpy((char*)a->data + a->length * a->elem_size, item, (size_t)a->elem_size);
    a->length++;
}

void* haki_array_get(HakiArray* a, int64_t idx) {
    if (!a || idx < 0 || idx >= a->length) {
        fprintf(stderr, "haki panic: array index %lld out of bounds (length %lld)\n",
            (long long)idx, a ? (long long)a->length : 0LL);
        abort();
    }
    return (char*)a->data + idx * a->elem_size;
}

int64_t haki_array_length(HakiArray* a) {
    return a ? a->length : 0;
}

void haki_array_free(HakiArray* a) {
    if (a) { free(a->data); free(a); }
}

/* removeLast: pop the last element; returns pointer to it (caller must read
   before next append which may realloc). Returns NULL if empty.             */
void* haki_array_remove_last(HakiArray* a) {
    if (!a || a->length == 0) return NULL;
    a->length--;
    return (char*)a->data + a->length * a->elem_size;
}

/* removeAt: remove element at index, shift left, return heap copy of element. */
void* haki_array_remove_at(HakiArray* a, int64_t idx) {
    if (!a || idx < 0 || idx >= a->length) return NULL;
    void* removed = malloc((size_t)a->elem_size);
    if (!removed) return NULL;
    memcpy(removed, (char*)a->data + idx * a->elem_size, (size_t)a->elem_size);
    memmove((char*)a->data + idx * a->elem_size,
            (char*)a->data + (idx + 1) * a->elem_size,
            (size_t)((a->length - idx - 1) * a->elem_size));
    a->length--;
    return removed;
}

/* contains (int): 1 if val is present, 0 otherwise. */
int64_t haki_array_contains_int(HakiArray* a, int64_t val) {
    if (!a) return 0;
    for (int64_t i = 0; i < a->length; i++) {
        if (*(int64_t*)((char*)a->data + i * a->elem_size) == val) return 1;
    }
    return 0;
}

/* contains (string): 1 if val is present, 0 otherwise. */
int64_t haki_array_contains_str(HakiArray* a, const char* val) {
    if (!a || !val) return 0;
    for (int64_t i = 0; i < a->length; i++) {
        const char* e = *(const char**)((char*)a->data + i * a->elem_size);
        if (e && strcmp(e, val) == 0) return 1;
    }
    return 0;
}

/* indexOf (int): returns first index == val, or -1. */
int64_t haki_array_index_of_int(HakiArray* a, int64_t val) {
    if (!a) return -1;
    for (int64_t i = 0; i < a->length; i++) {
        if (*(int64_t*)((char*)a->data + i * a->elem_size) == val) return i;
    }
    return -1;
}

/* indexOf (string): returns first index matching val, or -1. */
int64_t haki_array_index_of_str(HakiArray* a, const char* val) {
    if (!a || !val) return -1;
    for (int64_t i = 0; i < a->length; i++) {
        const char* e = *(const char**)((char*)a->data + i * a->elem_size);
        if (e && strcmp(e, val) == 0) return i;
    }
    return -1;
}

/* first / last: pointer to first/last element slot. NULL if empty. */
void* haki_array_first(HakiArray* a) {
    return (a && a->length > 0) ? a->data : NULL;
}
void* haki_array_last(HakiArray* a) {
    return (a && a->length > 0)
        ? (char*)a->data + (a->length - 1) * a->elem_size : NULL;
}

/* join: concatenate string array with separator. Caller owns the result. */
const char* haki_array_join(HakiArray* a, const char* sep) {
    if (!a || a->length == 0) return strdup("");
    if (!sep) sep = "";
    size_t sep_len = strlen(sep), total = 0;
    for (int64_t i = 0; i < a->length; i++) {
        const char* e = *(const char**)((char*)a->data + i * a->elem_size);
        if (e) total += strlen(e);
        if (i < a->length - 1) total += sep_len;
    }
    char* r = (char*)malloc(total + 1); char* p = r;
    for (int64_t i = 0; i < a->length; i++) {
        const char* e = *(const char**)((char*)a->data + i * a->elem_size);
        if (e) { size_t l = strlen(e); memcpy(p, e, l); p += l; }
        if (i < a->length - 1) { memcpy(p, sep, sep_len); p += sep_len; }
    }
    *p = '\0'; return r;
}

/* ── Map<K, V> ──────────────────────────────────────────────────────
   Open-addressing hash map with string keys (v0.1).
   Values are stored as opaque byte blobs of fixed elem_size.
   String keys are copied and owned by the map.               */

#define HAKI_MAP_INIT_CAP 16

typedef struct HakiMapEntry {
    char*  key;     /* heap-owned copy; NULL = empty slot */
    void*  value;   /* heap-owned value bytes             */
} HakiMapEntry;

typedef struct {
    HakiMapEntry* entries;
    int64_t       capacity;
    int64_t       length;   /* number of live entries */
    int64_t       val_size; /* sizeof(V) in bytes     */
} HakiMap;

static uint64_t haki_fnv1a(const char* s) {
    uint64_t h = 14695981039346656037ULL;
    while (*s) { h ^= (uint8_t)*s++; h *= 1099511628211ULL; }
    return h;
}

HakiMap* haki_map_new(int64_t val_size) {
    HakiMap* m = (HakiMap*)calloc(1, sizeof(HakiMap));
    if (!m) abort();
    m->val_size = val_size;
    m->capacity = HAKI_MAP_INIT_CAP;
    m->entries  = (HakiMapEntry*)calloc((size_t)m->capacity, sizeof(HakiMapEntry));
    if (!m->entries) abort();
    return m;
}

static void haki_map_insert_entry(HakiMap* m, char* key, void* val) {
    uint64_t idx = haki_fnv1a(key) % (uint64_t)m->capacity;
    for (int64_t i = 0; i < m->capacity; i++) {
        int64_t slot = (int64_t)((idx + (uint64_t)i) % (uint64_t)m->capacity);
        HakiMapEntry* e = &m->entries[slot];
        if (!e->key) {
            e->key   = key;
            e->value = val;
            m->length++;
            return;
        }
        if (strcmp(e->key, key) == 0) {
            free(e->value);
            e->value = val;
            free(key); /* duplicate key — discard */
            return;
        }
    }
    abort(); /* should never happen if load < 0.75 */
}

static void haki_map_grow(HakiMap* m) {
    int64_t old_cap = m->capacity;
    HakiMapEntry* old = m->entries;
    m->capacity *= 2;
    m->length   = 0;
    m->entries  = (HakiMapEntry*)calloc((size_t)m->capacity, sizeof(HakiMapEntry));
    if (!m->entries) abort();
    for (int64_t i = 0; i < old_cap; i++) {
        if (old[i].key) haki_map_insert_entry(m, old[i].key, old[i].value);
    }
    free(old);
}

void haki_map_set(HakiMap* m, const char* key, void* val_ptr) {
    if (!m || !key) return;
    if (m->length * 4 >= m->capacity * 3) haki_map_grow(m);
    char* k = strdup(key);
    if (!k) abort();
    // Store val_ptr directly — values are either pointer-sized primitives
    // (int/float/bool stored as (void*)(intptr_t)value) or pointer-to-heap-object.
    // No malloc/memcpy needed; the pointer IS the value.
    haki_map_insert_entry(m, k, val_ptr);
}

/* Returns pointer to value, or NULL if not found. */
void* haki_map_get(HakiMap* m, const char* key) {
    if (!m || !key || m->length == 0) return NULL;
    uint64_t idx = haki_fnv1a(key) % (uint64_t)m->capacity;
    for (int64_t i = 0; i < m->capacity; i++) {
        int64_t slot = (int64_t)((idx + (uint64_t)i) % (uint64_t)m->capacity);
        HakiMapEntry* e = &m->entries[slot];
        if (!e->key) return NULL; /* empty slot = not found */
        if (strcmp(e->key, key) == 0) return e->value;
    }
    return NULL;
}

int8_t haki_map_has(HakiMap* m, const char* key) {
    return haki_map_get(m, key) != NULL ? 1 : 0;
}

void haki_map_delete(HakiMap* m, const char* key) {
    if (!m || !key) return;
    uint64_t idx = haki_fnv1a(key) % (uint64_t)m->capacity;
    for (int64_t i = 0; i < m->capacity; i++) {
        int64_t slot = (int64_t)((idx + (uint64_t)i) % (uint64_t)m->capacity);
        HakiMapEntry* e = &m->entries[slot];
        if (!e->key) return;
        if (strcmp(e->key, key) == 0) {
            free(e->key);  e->key   = NULL;
            free(e->value); e->value = NULL;
            m->length--;
            return;
        }
    }
}

int64_t haki_map_length(HakiMap* m) { return m ? m->length : 0; }

/* getOrDefault: returns pointer to value if found, else pointer to default. */
void* haki_map_get_or_default(HakiMap* m, const char* key, void* default_val) {
    void* found = haki_map_get(m, key);
    return found ? found : default_val;
}

void haki_map_free(HakiMap* m) {
    if (!m) return;
    for (int64_t i = 0; i < m->capacity; i++) {
        if (m->entries[i].key)   free(m->entries[i].key);
        if (m->entries[i].value) free(m->entries[i].value);
    }
    free(m->entries);
    free(m);
}

/* Map iteration helpers */
int64_t haki_map_capacity(HakiMap* m) { return m ? m->capacity : 0; }
const char* haki_map_entry_key(HakiMap* m, int64_t i) {
    if (!m || i < 0 || i >= m->capacity) return NULL;
    return m->entries[i].key;
}
void* haki_map_entry_value(HakiMap* m, int64_t i) {
    if (!m || i < 0 || i >= m->capacity) return NULL;
    return m->entries[i].value;
}
#define HAKI_MAP_ENTRY_DEFINED


/* ── Thread (OS 1:1 pthread wrapper) ────────────────────────────── */

#include <pthread.h>

typedef struct {
    pthread_t tid;
} HakiThread;

HakiThread* haki_thread_spawn(void (*fn)(void)) {
    HakiThread* t = (HakiThread*)malloc(sizeof(HakiThread));
    if (!t) abort();
    /* pthread entry must be void* (*)(void*); wrap the no-arg fn */
    typedef void* (*pthread_fn)(void*);
    if (pthread_create(&t->tid, NULL, (pthread_fn)(void*)fn, NULL) != 0) {
        fprintf(stderr, "haki: thread_spawn failed\n");
        abort();
    }
    return t;
}

void haki_thread_join(HakiThread* t) {
    if (!t) return;
    pthread_join(t->tid, NULL);
    free(t);
}

/* ── Task<T> — call-site async + thread pool ─────────────────────
   haki_task_spawn(fn, arg) → HakiTask*
     Submits fn(arg) to the global thread pool. Returns a Task handle.
   haki_task_await(task) → void*
     Blocks until complete, returns the result pointer.
   haki_task_detach(task)
     Drops the user handle — task runs to completion in background.  */

#define HAKI_POOL_THREADS 8
#define HAKI_QUEUE_CAP    256

typedef void* (*HakiTaskFn)(void*);

typedef struct {
    _Atomic int64_t refcount;
    HakiTaskFn      fn;
    void*           arg;
    void*           result;
    int8_t          done;
    pthread_mutex_t done_mu;
    pthread_cond_t  done_cond;
} HakiTask;

static struct {
    pthread_t       threads[HAKI_POOL_THREADS];
    HakiTask*       queue[HAKI_QUEUE_CAP];
    int             head, tail, size;
    pthread_mutex_t mu;
    pthread_cond_t  cond;
    int             running;
} haki_pool_g;

static void haki_task_deref(HakiTask* t) {
    if (!t) return;
    if (atomic_fetch_sub_explicit(&t->refcount, 1, memory_order_acq_rel) == 1) {
        pthread_mutex_destroy(&t->done_mu);
        pthread_cond_destroy(&t->done_cond);
        free(t);
    }
}

static void* haki_pool_worker(void* unused) {
    (void)unused;
    for (;;) {
        pthread_mutex_lock(&haki_pool_g.mu);
        while (haki_pool_g.size == 0)
            pthread_cond_wait(&haki_pool_g.cond, &haki_pool_g.mu);
        HakiTask* t = haki_pool_g.queue[haki_pool_g.head];
        haki_pool_g.head = (haki_pool_g.head + 1) % HAKI_QUEUE_CAP;
        haki_pool_g.size--;
        pthread_mutex_unlock(&haki_pool_g.mu);

        void* res = t->fn(t->arg);

        pthread_mutex_lock(&t->done_mu);
        t->result = res;
        t->done   = 1;
        pthread_cond_broadcast(&t->done_cond);
        pthread_mutex_unlock(&t->done_mu);

        haki_task_deref(t); /* release executor reference */
    }
    return NULL;
}

static void haki_pool_ensure_started(void) {
    if (haki_pool_g.running) return;
    pthread_mutex_init(&haki_pool_g.mu, NULL);
    pthread_cond_init(&haki_pool_g.cond, NULL);
    for (int i = 0; i < HAKI_POOL_THREADS; i++)
        pthread_create(&haki_pool_g.threads[i], NULL, haki_pool_worker, NULL);
    haki_pool_g.running = 1;
}

HakiTask* haki_task_spawn(HakiTaskFn fn, void* arg) {
    haki_pool_ensure_started();
    HakiTask* t = (HakiTask*)calloc(1, sizeof(HakiTask));
    if (!t) abort();
    atomic_store_explicit(&t->refcount, 2, memory_order_relaxed); /* user + executor */
    t->fn  = fn;
    t->arg = arg;
    pthread_mutex_init(&t->done_mu, NULL);
    pthread_cond_init(&t->done_cond, NULL);

    pthread_mutex_lock(&haki_pool_g.mu);
    if (haki_pool_g.size >= HAKI_QUEUE_CAP) {
        pthread_mutex_unlock(&haki_pool_g.mu);
        fprintf(stderr, "haki: task queue full\n");
        abort();
    }
    haki_pool_g.queue[haki_pool_g.tail] = t;
    haki_pool_g.tail = (haki_pool_g.tail + 1) % HAKI_QUEUE_CAP;
    haki_pool_g.size++;
    pthread_cond_signal(&haki_pool_g.cond);
    pthread_mutex_unlock(&haki_pool_g.mu);
    return t;
}

void* haki_task_await(HakiTask* t) {
    if (!t) return NULL;
    pthread_mutex_lock(&t->done_mu);
    while (!t->done)
        pthread_cond_wait(&t->done_cond, &t->done_mu);
    void* res = t->result;
    pthread_mutex_unlock(&t->done_mu);
    haki_task_deref(t); /* release user reference */
    return res;
}

void haki_task_detach(HakiTask* t) {
    haki_task_deref(t); /* release user reference; executor ref keeps it alive */
}

/* ── Mutex<T> ────────────────────────────────────────────────────── */

typedef struct {
    pthread_mutex_t mu;
    void*           value;
} HakiMutex;

typedef struct {
    HakiMutex* mutex;
    void*      value;
} HakiMutexGuard;

HakiMutex* haki_mutex_new(void* initial) {
    HakiMutex* m = (HakiMutex*)malloc(sizeof(HakiMutex));
    if (!m) abort();
    pthread_mutex_init(&m->mu, NULL);
    m->value = initial;
    return m;
}

HakiMutexGuard* haki_mutex_lock(HakiMutex* m) {
    pthread_mutex_lock(&m->mu);
    HakiMutexGuard* g = (HakiMutexGuard*)malloc(sizeof(HakiMutexGuard));
    if (!g) abort();
    g->mutex = m;
    g->value = m->value;
    return g;
}

void haki_mutex_unlock(HakiMutexGuard* g) {
    if (!g) return;
    pthread_mutex_unlock(&g->mutex->mu);
    free(g);
}

void haki_mutex_free(HakiMutex* m) {
    if (!m) return;
    pthread_mutex_destroy(&m->mu);
    free(m);
}


/* ── Chan<T> — bounded/unbounded MPMC channel ────────────────────────────────
   Based on a ring buffer (bounded) or a linked list (unbounded, cap=0).
   Thread-safe via a mutex + two condition variables (not_full, not_empty).
   Close semantics: after close(), receive() drains remaining items then
   returns NULL. Further sends after close() abort (programming error).

   select() support: each channel maintains a list of registered HakiSelectWaiter
   structs. When an item arrives (or the channel closes), the channel signals
   all waiters via a shared condition variable so select() can re-evaluate.
   ─────────────────────────────────────────────────────────────────────────── */

/* Maximum capacity for a bounded channel (0 = unbounded linked-list mode). */
#define HAKI_CHAN_UNBOUNDED 0

/* Linked-list node for unbounded channels. */
typedef struct HakiChanNode {
    void*              val;
    struct HakiChanNode* next;
} HakiChanNode;

/* Waiter registered by select() — woken when any channel becomes ready. */
typedef struct HakiSelectWaiter {
    pthread_mutex_t*        mu;    /* the shared select mutex to signal on */
    pthread_cond_t*         cond;  /* the shared select cond to broadcast on */
    struct HakiSelectWaiter* next;
} HakiSelectWaiter;

typedef struct {
    /* Ring buffer (bounded mode, cap > 0) */
    void**              buf;
    int64_t             cap;        /* 0 = unbounded */
    int64_t             head;
    int64_t             tail;
    int64_t             count;

    /* Linked list tail/head (unbounded mode, cap == 0) */
    HakiChanNode*       ll_head;
    HakiChanNode*       ll_tail;

    int8_t              closed;

    pthread_mutex_t     mu;
    pthread_cond_t      not_empty;
    pthread_cond_t      not_full;

    /* select() waiter list — protected by mu */
    HakiSelectWaiter*   waiters;
} HakiChan;
#define HAKI_CHAN_TYPES_DEFINED

HakiChan* haki_chan_new(int64_t capacity) {
    HakiChan* ch = (HakiChan*)calloc(1, sizeof(HakiChan));
    if (!ch) abort();
    ch->cap = capacity;
    if (capacity > 0) {
        ch->buf = (void**)calloc((size_t)capacity, sizeof(void*));
        if (!ch->buf) abort();
    }
    pthread_mutex_init(&ch->mu, NULL);
    pthread_cond_init(&ch->not_empty, NULL);
    pthread_cond_init(&ch->not_full, NULL);
    return ch;
}

/* Wake all registered select() waiters — call with ch->mu held. */
static void haki_chan_wake_waiters(HakiChan* ch) {
    HakiSelectWaiter* w = ch->waiters;
    while (w) {
        pthread_mutex_lock(w->mu);
        pthread_cond_broadcast(w->cond);
        pthread_mutex_unlock(w->mu);
        w = w->next;
    }
}

void haki_chan_send(HakiChan* ch, void* val) {
    if (!ch) abort();
    pthread_mutex_lock(&ch->mu);
    if (ch->closed) {
        pthread_mutex_unlock(&ch->mu);
        fprintf(stderr, "haki: send on closed channel\n");
        abort();
    }
    if (ch->cap > 0) {
        /* Bounded: block while full */
        while (ch->count >= ch->cap && !ch->closed)
            pthread_cond_wait(&ch->not_full, &ch->mu);
        if (ch->closed) { pthread_mutex_unlock(&ch->mu); abort(); }
        ch->buf[ch->tail] = val;
        ch->tail = (ch->tail + 1) % ch->cap;
        ch->count++;
    } else {
        /* Unbounded: allocate a node */
        HakiChanNode* node = (HakiChanNode*)malloc(sizeof(HakiChanNode));
        if (!node) abort();
        node->val  = val;
        node->next = NULL;
        if (ch->ll_tail) ch->ll_tail->next = node;
        else             ch->ll_head = node;
        ch->ll_tail = node;
        ch->count++;
    }
    haki_chan_wake_waiters(ch);
    pthread_cond_signal(&ch->not_empty);
    pthread_mutex_unlock(&ch->mu);
}

/* Returns NULL if channel is closed and empty. Blocks otherwise. */
void* haki_chan_receive(HakiChan* ch) {
    if (!ch) return NULL;
    pthread_mutex_lock(&ch->mu);
    while (ch->count == 0 && !ch->closed)
        pthread_cond_wait(&ch->not_empty, &ch->mu);
    if (ch->count == 0 && ch->closed) {
        pthread_mutex_unlock(&ch->mu);
        return NULL;
    }
    void* val;
    if (ch->cap > 0) {
        val = ch->buf[ch->head];
        ch->head = (ch->head + 1) % ch->cap;
    } else {
        HakiChanNode* node = ch->ll_head;
        val = node->val;
        ch->ll_head = node->next;
        if (!ch->ll_head) ch->ll_tail = NULL;
        free(node);
    }
    ch->count--;
    pthread_cond_signal(&ch->not_full);
    pthread_mutex_unlock(&ch->mu);
    return val;
}

/* Non-blocking receive — returns NULL immediately if nothing available. */
void* haki_chan_try_receive(HakiChan* ch) {
    if (!ch) return NULL;
    pthread_mutex_lock(&ch->mu);
    if (ch->count == 0) { pthread_mutex_unlock(&ch->mu); return NULL; }
    void* val;
    if (ch->cap > 0) {
        val = ch->buf[ch->head];
        ch->head = (ch->head + 1) % ch->cap;
    } else {
        HakiChanNode* node = ch->ll_head;
        val = node->val;
        ch->ll_head = node->next;
        if (!ch->ll_head) ch->ll_tail = NULL;
        free(node);
    }
    ch->count--;
    pthread_cond_signal(&ch->not_full);
    pthread_mutex_unlock(&ch->mu);
    return val;
}

void haki_chan_close(HakiChan* ch) {
    if (!ch) return;
    pthread_mutex_lock(&ch->mu);
    ch->closed = 1;
    haki_chan_wake_waiters(ch);
    pthread_cond_broadcast(&ch->not_empty);
    pthread_cond_broadcast(&ch->not_full);
    pthread_mutex_unlock(&ch->mu);
}

int8_t haki_chan_is_closed(HakiChan* ch) {
    if (!ch) return 1;
    pthread_mutex_lock(&ch->mu);
    int8_t c = ch->closed && ch->count == 0;
    pthread_mutex_unlock(&ch->mu);
    return c;
}

int8_t haki_chan_has_item(HakiChan* ch) {
    if (!ch) return 0;
    pthread_mutex_lock(&ch->mu);
    int8_t ok = ch->count > 0;
    pthread_mutex_unlock(&ch->mu);
    return ok;
}

void haki_chan_free(HakiChan* ch) {
    if (!ch) return;
    if (ch->buf) free(ch->buf);
    /* drain linked list */
    HakiChanNode* n = ch->ll_head;
    while (n) { HakiChanNode* nx = n->next; free(n); n = nx; }
    pthread_mutex_destroy(&ch->mu);
    pthread_cond_destroy(&ch->not_empty);
    pthread_cond_destroy(&ch->not_full);
    free(ch);
}

/* Register a select waiter on a channel. Call with ch->mu held. */
static void haki_chan_add_waiter(HakiChan* ch, HakiSelectWaiter* w) {
    w->next    = ch->waiters;
    ch->waiters = w;
}

/* Remove a select waiter from a channel. Call with ch->mu held. */
static void haki_chan_remove_waiter(HakiChan* ch, HakiSelectWaiter* w) {
    HakiSelectWaiter** p = &ch->waiters;
    while (*p && *p != w) p = &(*p)->next;
    if (*p) *p = w->next;
}

/* ── select() — multiplex over N channels ────────────────────────────────────
   Usage:
     HakiChan* chans[N]  = { ch1, ch2, ... };
     int        ops[N]   = { 0 (recv), 0, 1 (send), ... };
     void*      vals[N]  = { NULL, NULL, send_val, ... };
     int ready = haki_select(N, chans, ops, vals);
     // ready: index of the arm that fired, -1 if all closed

   Algorithm (deadlock-free):
     1. Sort channels by pointer address to define a global lock order.
     2. Lock all channels in order.
     3. Check each for readiness — take first ready arm.
     4. If none ready: register a HakiSelectWaiter on each, wait on shared cond.
     5. Unlock all channels, wait.
     6. On wakeup: re-lock all (in order), re-check, take ready arm.
     7. Remove waiters from all channels, unlock, return index.
   ─────────────────────────────────────────────────────────────────────────── */

#define HAKI_SELECT_MAX 64

/* Sort indices by channel pointer for consistent lock ordering. */
static void haki_select_sort_order(int n, HakiChan** chans, int* order) {
    for (int i = 0; i < n; i++) order[i] = i;
    /* Insertion sort — n is small (≤ 64) */
    for (int i = 1; i < n; i++) {
        int key = order[i];
        int j   = i - 1;
        while (j >= 0 && (uintptr_t)chans[order[j]] > (uintptr_t)chans[key]) {
            order[j+1] = order[j];
            j--;
        }
        order[j+1] = key;
    }
}

/* Check if arm i is ready (call with all channels locked). */
static int haki_select_arm_ready(int i, HakiChan** chans, int* ops) {
    HakiChan* ch = chans[i];
    if (ops[i] == 0) /* receive */
        return ch->count > 0 || (ch->closed);
    else              /* send */
        return !ch->closed && (ch->cap == 0 || ch->count < ch->cap);
}

/* timeout_ms: -1 = block forever; >= 0 = return -1 after that many milliseconds */
int haki_select(int n, HakiChan** chans, int* ops, void** vals, int64_t timeout_ms) {
    if (n <= 0 || n > HAKI_SELECT_MAX) return -1;

    int order[HAKI_SELECT_MAX];
    haki_select_sort_order(n, chans, order);

    pthread_mutex_t sel_mu;
    pthread_cond_t  sel_cond;
    pthread_mutex_init(&sel_mu, NULL);
    pthread_cond_init(&sel_cond, NULL);

    HakiSelectWaiter waiters[HAKI_SELECT_MAX];
    for (int i = 0; i < n; i++) {
        waiters[i].mu   = &sel_mu;
        waiters[i].cond = &sel_cond;
        waiters[i].next = NULL;
    }

    /* Compute absolute deadline once (CLOCK_REALTIME for pthread_cond_timedwait) */
    struct timespec deadline = {0, 0};
    int has_timeout = (timeout_ms >= 0);
    if (has_timeout) {
        clock_gettime(CLOCK_REALTIME, &deadline);
        int64_t ns = deadline.tv_nsec + (timeout_ms % 1000LL) * 1000000LL;
        deadline.tv_sec  += (time_t)(timeout_ms / 1000LL) + (time_t)(ns / 1000000000LL);
        deadline.tv_nsec  = (long)(ns % 1000000000LL);
    }

    int result = -1;

    for (;;) {
        /* Lock all channels in consistent order */
        for (int i = 0; i < n; i++)
            pthread_mutex_lock(&chans[order[i]]->mu);

        /* Check readiness — pick first ready arm */
        /* Randomise start offset to avoid starvation */
        int start = (int)(((uintptr_t)&result >> 4) % (uintptr_t)n);
        for (int k = 0; k < n; k++) {
            int i = (start + k) % n;
            if (!haki_select_arm_ready(i, chans, ops)) continue;
            HakiChan* ch = chans[i];
            if (ops[i] == 0) { /* receive */
                void* v = NULL;
                if (ch->count > 0) {
                    if (ch->cap > 0) {
                        v = ch->buf[ch->head];
                        ch->head = (ch->head + 1) % ch->cap;
                    } else {
                        HakiChanNode* node = ch->ll_head;
                        v = node->val;
                        ch->ll_head = node->next;
                        if (!ch->ll_head) ch->ll_tail = NULL;
                        free(node);
                    }
                    ch->count--;
                    pthread_cond_signal(&ch->not_full);
                }
                vals[i] = v;
            } else {           /* send */
                if (ch->cap > 0) {
                    ch->buf[ch->tail] = vals[i];
                    ch->tail = (ch->tail + 1) % ch->cap;
                    ch->count++;
                } else {
                    HakiChanNode* node = (HakiChanNode*)malloc(sizeof(HakiChanNode));
                    if (!node) abort();
                    node->val  = vals[i];
                    node->next = NULL;
                    if (ch->ll_tail) ch->ll_tail->next = node;
                    else             ch->ll_head = node;
                    ch->ll_tail = node;
                    ch->count++;
                }
                pthread_cond_signal(&ch->not_empty);
                haki_chan_wake_waiters(ch);
            }
            result = i;
            break;
        }

        if (result >= 0) {
            /* Unlock in reverse order */
            for (int i = n-1; i >= 0; i--)
                pthread_mutex_unlock(&chans[order[i]]->mu);
            break;
        }

        /* Check if all channels are closed and empty — unblockable */
        int all_dead = 1;
        for (int i = 0; i < n; i++) {
            if (ops[i] == 0 && !(chans[i]->closed && chans[i]->count == 0))
                { all_dead = 0; break; }
        }
        if (all_dead) {
            for (int i = n-1; i >= 0; i--)
                pthread_mutex_unlock(&chans[order[i]]->mu);
            result = -1;
            break;
        }

        /* Register waiters on each channel */
        for (int i = 0; i < n; i++)
            haki_chan_add_waiter(chans[i], &waiters[i]);

        /* Unlock all and wait for a signal (or timeout) */
        pthread_mutex_lock(&sel_mu);
        for (int i = n-1; i >= 0; i--)
            pthread_mutex_unlock(&chans[order[i]]->mu);

        int wait_rc = 0;
        if (has_timeout) {
            wait_rc = pthread_cond_timedwait(&sel_cond, &sel_mu, &deadline);
        } else {
            pthread_cond_wait(&sel_cond, &sel_mu);
        }
        pthread_mutex_unlock(&sel_mu);

        /* Remove waiters before re-locking channels */
        for (int i = 0; i < n; i++)
            pthread_mutex_lock(&chans[order[i]]->mu);
        for (int i = 0; i < n; i++)
            haki_chan_remove_waiter(chans[i], &waiters[i]);
        for (int i = n-1; i >= 0; i--)
            pthread_mutex_unlock(&chans[order[i]]->mu);

        if (wait_rc == ETIMEDOUT) {
            result = -1;
            break;
        }
    }

    pthread_mutex_destroy(&sel_mu);
    pthread_cond_destroy(&sel_cond);
    return result;
}

/* ── TaskGroup<T> — structured concurrency ───────────────────────────────────
   TaskGroup owns a dynamic array of spawned tasks. awaitAll() blocks until
   every task completes and returns an Array<T> of results.
   cancel() sets the cancel flag — tasks check haki_taskgroup_canceled(g).
   Dynamic realloc growth: starts at 8, doubles when full.
   ─────────────────────────────────────────────────────────────────────────── */

typedef struct {
    HakiTask**      tasks;
    int64_t         count;
    int64_t         cap;
    _Atomic int8_t  canceled;
    pthread_mutex_t mu;
} HakiTaskGroup;
#define HAKI_TASK_TYPES_DEFINED

HakiTaskGroup* haki_taskgroup_new(void) {
    HakiTaskGroup* g = (HakiTaskGroup*)calloc(1, sizeof(HakiTaskGroup));
    if (!g) abort();
    g->cap   = 8;
    g->tasks = (HakiTask**)malloc((size_t)g->cap * sizeof(HakiTask*));
    if (!g->tasks) abort();
    atomic_store_explicit(&g->canceled, 0, memory_order_relaxed);
    pthread_mutex_init(&g->mu, NULL);
    return g;
}

/* Add an already-spawned task to a TaskGroup (for `group.add(async fn(...))`) */
void haki_taskgroup_add(HakiTaskGroup* g, HakiTask* t) {
    if (!g || !t) return;
    pthread_mutex_lock(&g->mu);
    if (g->count >= g->cap) {
        g->cap = g->cap ? g->cap * 2 : 8;
        g->tasks = (HakiTask**)realloc(g->tasks, g->cap * sizeof(HakiTask*));
    }
    g->tasks[g->count++] = t;
    pthread_mutex_unlock(&g->mu);
}
void haki_taskgroup_spawn(HakiTaskGroup* g, HakiTaskFn fn, void* arg) {
    if (!g) abort();
    HakiTask* t = haki_task_spawn(fn, arg);
    pthread_mutex_lock(&g->mu);
    if (g->count >= g->cap) {
        g->cap *= 2;
        g->tasks = (HakiTask**)realloc(g->tasks, (size_t)g->cap * sizeof(HakiTask*));
        if (!g->tasks) abort();
    }
    g->tasks[g->count++] = t;
    pthread_mutex_unlock(&g->mu);
}

/* Returns a HakiArray* of result pointers (void*). */
HakiArray* haki_taskgroup_await_all(HakiTaskGroup* g) {
    if (!g) return haki_array_new(sizeof(void*));
    pthread_mutex_lock(&g->mu);
    int64_t n     = g->count;
    HakiTask** ts = g->tasks;
    pthread_mutex_unlock(&g->mu);

    HakiArray* results = haki_array_new(sizeof(void*));
    for (int64_t i = 0; i < n; i++) {
        void* res = haki_task_await(ts[i]);
        haki_array_append(results, &res);
    }
    return results;
}

void haki_taskgroup_cancel(HakiTaskGroup* g) {
    if (!g) return;
    atomic_store_explicit(&g->canceled, 1, memory_order_release);
}

int8_t haki_taskgroup_canceled(HakiTaskGroup* g) {
    if (!g) return 0;
    return atomic_load_explicit(&g->canceled, memory_order_acquire);
}

void haki_taskgroup_free(HakiTaskGroup* g) {
    if (!g) return;
    pthread_mutex_lock(&g->mu);
    free(g->tasks);
    pthread_mutex_unlock(&g->mu);
    pthread_mutex_destroy(&g->mu);
    free(g);
}


#include <unistd.h>

/* ── std/net — TCP/UDP socket primitives ─────────────────────────────────────
   Thin wrappers around POSIX socket API. Error handling: functions return -1
   on failure; use haki_net_last_error() for the errno message.
   ─────────────────────────────────────────────────────────────────────────── */

#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <netdb.h>
#include <fcntl.h>
#include <errno.h>

/* Last error string — set on every socket failure. */
static char haki_net_errbuf[256] = "";

static void haki_net_set_error(const char* ctx) {
    snprintf(haki_net_errbuf, sizeof(haki_net_errbuf), "%s: %s", ctx, strerror(errno));
}

const char* haki_net_last_error(void) {
    return haki_net_errbuf;
}

/* Create a TCP socket. Returns fd or -1. */
int64_t haki_tcp_socket(void) {
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd < 0) { haki_net_set_error("socket"); return -1; }
    /* SO_REUSEADDR for servers */
    int yes = 1;
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &yes, sizeof(yes));
    return (int64_t)fd;
}

/* Create a UDP socket. Returns fd or -1. */
int64_t haki_udp_socket(void) {
    int fd = socket(AF_INET, SOCK_DGRAM, 0);
    if (fd < 0) { haki_net_set_error("socket"); return -1; }
    return (int64_t)fd;
}

/* Connect to host:port. Returns 0 on success, -1 on failure. */
int64_t haki_net_connect(int64_t fd, const char* host, int64_t port) {
    struct addrinfo hints = {0}, *res = NULL;
    hints.ai_family   = AF_INET;
    hints.ai_socktype = SOCK_STREAM;
    char portbuf[16];
    snprintf(portbuf, sizeof(portbuf), "%lld", (long long)port);
    if (getaddrinfo(host, portbuf, &hints, &res) != 0 || !res) {
        haki_net_set_error("getaddrinfo");
        return -1;
    }
    int r = connect((int)fd, res->ai_addr, res->ai_addrlen);
    freeaddrinfo(res);
    if (r < 0) { haki_net_set_error("connect"); return -1; }
    return 0;
}

/* Bind to port (server). Returns 0 or -1. */
int64_t haki_net_bind(int64_t fd, int64_t port) {
    struct sockaddr_in addr = {0};
    addr.sin_family      = AF_INET;
    addr.sin_port        = htons((uint16_t)port);
    addr.sin_addr.s_addr = INADDR_ANY;
    if (bind((int)fd, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        haki_net_set_error("bind"); return -1;
    }
    return 0;
}

/* Listen for connections. backlog=128. Returns 0 or -1. */
int64_t haki_net_listen(int64_t fd) {
    if (listen((int)fd, 128) < 0) { haki_net_set_error("listen"); return -1; }
    return 0;
}

/* Accept a connection. Returns new fd or -1. */
int64_t haki_net_accept(int64_t fd) {
    struct sockaddr_in peer = {0};
    socklen_t peerlen = sizeof(peer);
    int client = accept((int)fd, (struct sockaddr*)&peer, &peerlen);
    if (client < 0) { haki_net_set_error("accept"); return -1; }
    return (int64_t)client;
}

/* Send data. Returns bytes sent or -1. */
int64_t haki_net_send(int64_t fd, const char* data, int64_t len) {
    ssize_t r = send((int)fd, data, (size_t)len, 0);
    if (r < 0) { haki_net_set_error("send"); return -1; }
    return (int64_t)r;
}

/* Receive data into a heap-allocated string (null-terminated). Returns "" on EOF. */
const char* haki_net_recv(int64_t fd, int64_t max_bytes) {
    char* buf = (char*)malloc((size_t)(max_bytes + 1));
    if (!buf) abort();
    ssize_t r = recv((int)fd, buf, (size_t)max_bytes, 0);
    if (r <= 0) { free(buf); return ""; }
    buf[r] = '\0';
    return buf;
}

/* Close a socket. */
void haki_net_close(int64_t fd) {
    close((int)fd);
}

/* Set socket to non-blocking mode. Returns 0 or -1. */
int64_t haki_net_set_nonblocking(int64_t fd) {
    int flags = fcntl((int)fd, F_GETFL, 0);
    if (flags < 0) { haki_net_set_error("fcntl"); return -1; }
    if (fcntl((int)fd, F_SETFL, flags | O_NONBLOCK) < 0) {
        haki_net_set_error("fcntl"); return -1;
    }
    return 0;
}

/* Get peer IP address as a string. */
const char* haki_net_peer_addr(int64_t fd) {
    struct sockaddr_in peer = {0};
    socklen_t peerlen = sizeof(peer);
    if (getpeername((int)fd, (struct sockaddr*)&peer, &peerlen) < 0) return "";
    char* buf = (char*)malloc(INET_ADDRSTRLEN);
    if (!buf) return "";
    inet_ntop(AF_INET, &peer.sin_addr, buf, INET_ADDRSTRLEN);
    return buf;
}

/* ── std/crypto — SHA-256, Base64, HMAC-SHA256 ────────────────────────────────
   Self-contained implementation — no OpenSSL or libssl dependency.
   ─────────────────────────────────────────────────────────────────────────── */

/* ── SHA-256 ────────────────────────────────────────────────────────────────── */

static const uint32_t SHA256_K[64] = {
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
    0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
    0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
    0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
    0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2,
};

#define SHA256_ROR32(x,n) (((x)>>(n))|((x)<<(32-(n))))
#define SHA256_CH(x,y,z)  (((x)&(y))^(~(x)&(z)))
#define SHA256_MAJ(x,y,z) (((x)&(y))^((x)&(z))^((y)&(z)))
#define SHA256_S0(x)      (SHA256_ROR32(x,2)^SHA256_ROR32(x,13)^SHA256_ROR32(x,22))
#define SHA256_S1(x)      (SHA256_ROR32(x,6)^SHA256_ROR32(x,11)^SHA256_ROR32(x,25))
#define SHA256_R0(x)      (SHA256_ROR32(x,7)^SHA256_ROR32(x,18)^((x)>>3))
#define SHA256_R1(x)      (SHA256_ROR32(x,17)^SHA256_ROR32(x,19)^((x)>>10))

typedef struct {
    uint64_t  total;
    uint32_t  state[8];
    uint8_t   buf[64];
    uint32_t  buflen;
} HakiSHA256;

static void haki_sha256_init(HakiSHA256* ctx) {
    ctx->total = ctx->buflen = 0;
    ctx->state[0]=0x6a09e667; ctx->state[1]=0xbb67ae85;
    ctx->state[2]=0x3c6ef372; ctx->state[3]=0xa54ff53a;
    ctx->state[4]=0x510e527f; ctx->state[5]=0x9b05688c;
    ctx->state[6]=0x1f83d9ab; ctx->state[7]=0x5be0cd19;
}

static void haki_sha256_block(HakiSHA256* ctx, const uint8_t* data) {
    uint32_t W[64], a,b,c,d,e,f,g,h,T1,T2;
    int i;
    for (i=0;i<16;i++) W[i]=((uint32_t)data[i*4]<<24)|((uint32_t)data[i*4+1]<<16)|
                             ((uint32_t)data[i*4+2]<<8)|(uint32_t)data[i*4+3];
    for (i=16;i<64;i++) W[i]=SHA256_R1(W[i-2])+W[i-7]+SHA256_R0(W[i-15])+W[i-16];
    a=ctx->state[0]; b=ctx->state[1]; c=ctx->state[2]; d=ctx->state[3];
    e=ctx->state[4]; f=ctx->state[5]; g=ctx->state[6]; h=ctx->state[7];
    for (i=0;i<64;i++) {
        T1=h+SHA256_S1(e)+SHA256_CH(e,f,g)+SHA256_K[i]+W[i];
        T2=SHA256_S0(a)+SHA256_MAJ(a,b,c);
        h=g; g=f; f=e; e=d+T1; d=c; c=b; b=a; a=T1+T2;
    }
    ctx->state[0]+=a; ctx->state[1]+=b; ctx->state[2]+=c; ctx->state[3]+=d;
    ctx->state[4]+=e; ctx->state[5]+=f; ctx->state[6]+=g; ctx->state[7]+=h;
}

static void haki_sha256_update(HakiSHA256* ctx, const uint8_t* data, size_t len) {
    ctx->total += len;
    if (ctx->buflen + len < 64) {
        memcpy(ctx->buf + ctx->buflen, data, len);
        ctx->buflen += (uint32_t)len;
        return;
    }
    size_t gap = 64 - ctx->buflen;
    memcpy(ctx->buf + ctx->buflen, data, gap);
    haki_sha256_block(ctx, ctx->buf);
    data += gap; len -= gap; ctx->buflen = 0;
    while (len >= 64) { haki_sha256_block(ctx, data); data += 64; len -= 64; }
    memcpy(ctx->buf, data, len);
    ctx->buflen = (uint32_t)len;
}

static void haki_sha256_final(HakiSHA256* ctx, uint8_t out[32]) {
    uint64_t bits = ctx->total * 8;
    ctx->buf[ctx->buflen++] = 0x80;
    if (ctx->buflen > 56) {
        memset(ctx->buf + ctx->buflen, 0, 64 - ctx->buflen);
        haki_sha256_block(ctx, ctx->buf);
        ctx->buflen = 0;
    }
    memset(ctx->buf + ctx->buflen, 0, 56 - ctx->buflen);
    for (int i = 7; i >= 0; i--) { ctx->buf[56 + (7-i)] = (uint8_t)(bits >> (i*8)); }
    haki_sha256_block(ctx, ctx->buf);
    for (int i = 0; i < 8; i++) {
        out[i*4+0]=(uint8_t)(ctx->state[i]>>24); out[i*4+1]=(uint8_t)(ctx->state[i]>>16);
        out[i*4+2]=(uint8_t)(ctx->state[i]>>8);  out[i*4+3]=(uint8_t)(ctx->state[i]);
    }
}

/* Hash a string → 64-char lowercase hex digest. */
const char* haki_crypto_sha256(const char* input) {
    HakiSHA256 ctx;
    uint8_t digest[32];
    haki_sha256_init(&ctx);
    haki_sha256_update(&ctx, (const uint8_t*)input, strlen(input));
    haki_sha256_final(&ctx, digest);
    char* out = (char*)malloc(65);
    if (!out) abort();
    for (int i = 0; i < 32; i++) snprintf(out + i*2, 3, "%02x", digest[i]);
    out[64] = '\0';
    return out;
}

/* Hash raw bytes (length explicit). */
const char* haki_crypto_sha256_bytes(const char* data, int64_t len) {
    HakiSHA256 ctx;
    uint8_t digest[32];
    haki_sha256_init(&ctx);
    haki_sha256_update(&ctx, (const uint8_t*)data, (size_t)len);
    haki_sha256_final(&ctx, digest);
    char* out = (char*)malloc(65);
    if (!out) abort();
    for (int i = 0; i < 32; i++) snprintf(out + i*2, 3, "%02x", digest[i]);
    out[64] = '\0';
    return out;
}

/* ── HMAC-SHA256 ───────────────────────────────────────────────────────────── */

const char* haki_crypto_hmac_sha256(const char* key, const char* msg) {
    size_t klen = strlen(key), mlen = strlen(msg);
    uint8_t kbuf[64] = {0};
    HakiSHA256 ctx;
    uint8_t digest[32];

    /* Key preprocessing */
    if (klen > 64) {
        haki_sha256_init(&ctx);
        haki_sha256_update(&ctx, (const uint8_t*)key, klen);
        haki_sha256_final(&ctx, kbuf);
    } else {
        memcpy(kbuf, key, klen);
    }

    /* Inner hash: H(K XOR ipad || msg) */
    uint8_t ipad[64], opad[64];
    for (int i = 0; i < 64; i++) { ipad[i] = kbuf[i] ^ 0x36; opad[i] = kbuf[i] ^ 0x5c; }
    haki_sha256_init(&ctx);
    haki_sha256_update(&ctx, ipad, 64);
    haki_sha256_update(&ctx, (const uint8_t*)msg, mlen);
    haki_sha256_final(&ctx, digest);

    /* Outer hash: H(K XOR opad || inner) */
    haki_sha256_init(&ctx);
    haki_sha256_update(&ctx, opad, 64);
    haki_sha256_update(&ctx, digest, 32);
    haki_sha256_final(&ctx, digest);

    char* out = (char*)malloc(65);
    if (!out) abort();
    for (int i = 0; i < 32; i++) snprintf(out + i*2, 3, "%02x", digest[i]);
    out[64] = '\0';
    return out;
}

/* ── Base64 ────────────────────────────────────────────────────────────────── */

static const char B64_TABLE[] =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

const char* haki_crypto_base64_encode(const char* input) {
    size_t ilen = strlen(input);
    size_t olen = 4 * ((ilen + 2) / 3);
    char* out = (char*)malloc(olen + 1);
    if (!out) abort();
    size_t i = 0, j = 0;
    const uint8_t* in = (const uint8_t*)input;
    for (; i + 2 < ilen; i += 3) {
        out[j++] = B64_TABLE[in[i] >> 2];
        out[j++] = B64_TABLE[((in[i] & 3) << 4) | (in[i+1] >> 4)];
        out[j++] = B64_TABLE[((in[i+1] & 15) << 2) | (in[i+2] >> 6)];
        out[j++] = B64_TABLE[in[i+2] & 63];
    }
    if (i < ilen) {
        out[j++] = B64_TABLE[in[i] >> 2];
        if (i + 1 < ilen) {
            out[j++] = B64_TABLE[((in[i] & 3) << 4) | (in[i+1] >> 4)];
            out[j++] = B64_TABLE[(in[i+1] & 15) << 2];
        } else {
            out[j++] = B64_TABLE[(in[i] & 3) << 4];
            out[j++] = '=';
        }
        out[j++] = '=';
    }
    out[j] = '\0';
    return out;
}

const char* haki_crypto_base64_decode(const char* input) {
    static const int8_t DT[256] = {
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,62,-1,-1,-1,63,52,53,54,55,56,57,58,59,60,61,-1,-1,-1,-2,-1,-1,
        -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,-1,-1,-1,-1,-1,
        -1,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,-1,-1,-1,-1,-1,
    };
    size_t ilen = strlen(input);
    if (ilen % 4 != 0) return "";
    size_t olen = ilen / 4 * 3;
    if (ilen && input[ilen-1] == '=') olen--;
    if (ilen > 1 && input[ilen-2] == '=') olen--;
    char* out = (char*)malloc(olen + 1);
    if (!out) abort();
    size_t i = 0, j = 0;
    while (i < ilen) {
        int a=DT[(uint8_t)input[i]], b=DT[(uint8_t)input[i+1]],
            c=DT[(uint8_t)input[i+2]], d=DT[(uint8_t)input[i+3]];
        if (a<0||b<0) break;
        out[j++] = (char)((a<<2)|(b>>4));
        if (c!=-2) out[j++] = (char)((b<<4)|(c>>2));
        if (d!=-2) out[j++] = (char)((c<<6)|d);
        i += 4;
    }
    out[j] = '\0';
    return out;
}

/* ── URL-safe Base64 (no padding, + → -, / → _) ──────────────────────────── */

const char* haki_crypto_base64url_encode(const char* input) {
    const char* b64 = haki_crypto_base64_encode(input);
    size_t len = strlen(b64);
    char* out = (char*)malloc(len + 1);
    if (!out) abort();
    for (size_t i = 0; i < len; i++) {
        if      (b64[i] == '+') out[i] = '-';
        else if (b64[i] == '/') out[i] = '_';
        else if (b64[i] == '=') { out[i] = '\0'; break; }
        else                     out[i] = b64[i];
    }
    out[len] = '\0';
    return out;
}

/* ── String methods ──────────────────────────────────────────────
   All string operations work on null-terminated C strings (const char*).
   Functions that return new strings heap-allocate via malloc.
   The caller owns the result.                                     */

#include <ctype.h>

/* s.split(sep) → Array<string> (HakiArray* of char* elements) */
HakiArray* haki_string_split(const char* s, const char* sep) {
    HakiArray* arr = haki_array_new(sizeof(char*));
    if (!s || !sep || sep[0] == '\0') {
        char* dup = strdup(s ? s : "");
        haki_array_append(arr, &dup);
        return arr;
    }
    size_t sep_len = strlen(sep);
    const char* cur = s;
    while (1) {
        const char* found = strstr(cur, sep);
        size_t piece_len = found ? (size_t)(found - cur) : strlen(cur);
        char* piece = (char*)malloc(piece_len + 1);
        memcpy(piece, cur, piece_len);
        piece[piece_len] = '\0';
        haki_array_append(arr, &piece);
        if (!found) break;
        cur = found + sep_len;
    }
    return arr;
}

char* haki_string_trim(const char* s) {
    if (!s) return strdup("");
    while (isspace((unsigned char)*s)) s++;
    size_t len = strlen(s);
    while (len > 0 && isspace((unsigned char)s[len-1])) len--;
    char* r = (char*)malloc(len + 1);
    memcpy(r, s, len);
    r[len] = '\0';
    return r;
}

char* haki_string_trim_start(const char* s) {
    if (!s) return strdup("");
    while (isspace((unsigned char)*s)) s++;
    return strdup(s);
}

char* haki_string_trim_end(const char* s) {
    if (!s) return strdup("");
    size_t len = strlen(s);
    while (len > 0 && isspace((unsigned char)s[len-1])) len--;
    char* r = (char*)malloc(len + 1);
    memcpy(r, s, len);
    r[len] = '\0';
    return r;
}

char* haki_string_to_upper(const char* s) {
    if (!s) return strdup("");
    char* r = strdup(s);
    for (char* p = r; *p; p++) *p = (char)toupper((unsigned char)*p);
    return r;
}

char* haki_string_to_lower(const char* s) {
    if (!s) return strdup("");
    char* r = strdup(s);
    for (char* p = r; *p; p++) *p = (char)tolower((unsigned char)*p);
    return r;
}

int8_t haki_string_contains(const char* s, const char* sub) {
    if (!s || !sub) return 0;
    return strstr(s, sub) != NULL ? 1 : 0;
}

int8_t haki_string_starts_with(const char* s, const char* prefix) {
    if (!s || !prefix) return 0;
    return strncmp(s, prefix, strlen(prefix)) == 0 ? 1 : 0;
}

int8_t haki_string_ends_with(const char* s, const char* suffix) {
    if (!s || !suffix) return 0;
    size_t sl = strlen(s), fl = strlen(suffix);
    return (sl >= fl && strcmp(s + sl - fl, suffix) == 0) ? 1 : 0;
}

char* haki_string_replace(const char* s, const char* from, const char* to) {
    if (!s || !from || from[0] == '\0') return strdup(s ? s : "");
    size_t from_len = strlen(from), to_len = strlen(to ? to : "");
    /* Count occurrences */
    size_t count = 0;
    const char* cur = s;
    while ((cur = strstr(cur, from))) { count++; cur += from_len; }
    /* Allocate result */
    size_t orig_len = strlen(s);
    size_t new_len  = orig_len + count * (to_len - from_len + (to_len >= from_len ? 0 : 0));
    char* result = (char*)malloc(new_len + 1);
    char* dst = result;
    cur = s;
    while (*cur) {
        if (strncmp(cur, from, from_len) == 0) {
            if (to) { memcpy(dst, to, to_len); dst += to_len; }
            cur += from_len;
        } else {
            *dst++ = *cur++;
        }
    }
    *dst = '\0';
    return result;
}

int64_t haki_string_index_of(const char* s, const char* sub) {
    if (!s || !sub) return -1;
    const char* found = strstr(s, sub);
    return found ? (int64_t)(found - s) : -1;
}

char* haki_string_substring(const char* s, int64_t start, int64_t end) {
    if (!s) return strdup("");
    int64_t len = (int64_t)strlen(s);
    if (start < 0) start = 0;
    if (end > len) end = len;
    if (start >= end) return strdup("");
    int64_t sub_len = end - start;
    char* r = (char*)malloc((size_t)sub_len + 1);
    memcpy(r, s + start, (size_t)sub_len);
    r[sub_len] = '\0';
    return r;
}
#ifndef HAKI_STRING_EXTRAS_DEFINED
#define HAKI_STRING_EXTRAS_DEFINED
int8_t haki_string_is_empty(const char* s) {
    return (!s || s[0] == '\0') ? 1 : 0;
}
char* haki_string_char_at(const char* s, int64_t idx) {
    if (!s) return strdup("");
    int64_t len = (int64_t)strlen(s);
    if (idx < 0 || idx >= len) return strdup("");
    char* r = (char*)malloc(2);
    r[0] = s[idx]; r[1] = '\0';
    return r;
}
int64_t haki_string_char_code_at(const char* s, int64_t idx) {
    if (!s) return -1;
    int64_t len = (int64_t)strlen(s);
    if (idx < 0 || idx >= len) return -1;
    return (int64_t)(unsigned char)s[idx];
}
#endif /* HAKI_STRING_EXTRAS_DEFINED */


/* ── File I/O ────────────────────────────────────────────────────
   Simple blocking file operations for scripting use.             */

void haki_file_read(const char* path, char** out_content, char** out_error) {
    *out_content = NULL;
    *out_error   = NULL;
    FILE* f = fopen(path, "rb");
    if (!f) { *out_error = strdup(strerror(errno)); return; }
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    rewind(f);
    if (size < 0) { fclose(f); *out_error = strdup("seek failed"); return; }
    char* buf = (char*)malloc((size_t)size + 1);
    if (!buf) { fclose(f); *out_error = strdup("out of memory"); return; }
    size_t n = fread(buf, 1, (size_t)size, f);
    fclose(f);
    buf[n] = '\0';
    *out_content = buf;
}

/* haki_read_file — single-return wrapper matching C emitter's calling convention.
 * Returns a heap-allocated void* pair { content_ptr, error_ptr } matching
 * readFile(path) -> (string, Error?) semantics.
 * Uses raw void* fields to avoid forward-declaration dependency on HakiError/__Tuple2. */
void* haki_read_file(const char* path) {
    char* content = NULL;
    char* err_msg = NULL;
    haki_file_read(path, &content, &err_msg);
    /* Allocate a simple two-pointer tuple: [content, error] */
    void** t = (void**)malloc(2 * sizeof(void*));
    t[0] = (void*)(content ? content : (char*)"");
    if (err_msg) {
        /* Allocate a minimal error object: [arc_count, message] */
        void** e = (void**)malloc(2 * sizeof(void*));
        int64_t* arc = (int64_t*)malloc(sizeof(int64_t));
        *arc = 1;
        e[0] = (void*)arc;
        e[1] = (void*)err_msg;
        t[1] = (void*)e;
    } else {
        t[1] = NULL;
    }
    return (void*)t;
}

/* haki_write_file — matches C emitter's calling convention for writeFile(). */
void* haki_write_file(const char* path, const char* content) {
    /* Forward declaration needed — haki_file_write is defined below */
    char* haki_file_write(const char*, const char*);
    char* err = haki_file_write(path, content);
    if (!err) return NULL;
    /* Return error as a simple two-pointer [arc, message] object */
    void** e = (void**)malloc(2 * sizeof(void*));
    int64_t* arc = (int64_t*)malloc(sizeof(int64_t));
    *arc = 1;
    e[0] = (void*)arc;
    e[1] = (void*)err;
    return (void*)e;
}

char* haki_file_write(const char* path, const char* content) {
    FILE* f = fopen(path, "wb");
    if (!f) return strdup(strerror(errno));
    size_t len = content ? strlen(content) : 0;
    size_t n = fwrite(content, 1, len, f);
    fclose(f);
    if (n != len) return strdup("write failed");
    return NULL;
}

int8_t haki_file_exists(const char* path) {
    FILE* f = fopen(path, "rb");
    if (f) { fclose(f); return 1; }
    return 0;
}

/* ── Command-line arguments ──────────────────────────────────────────────── */

static int   _haki_argc = 0;
static char** _haki_argv = NULL;

void haki_runtime_init(int argc, char** argv) {
    _haki_argc = argc;
    _haki_argv = argv;
}

/* argv() -> Array<string>: returns program arguments as a Haki array. */
void* haki_argv(void) {
    void* arr = haki_array_new(sizeof(void*));
    for (int i = 0; i < _haki_argc; i++) {
        char* s = strdup(_haki_argv[i]);
        haki_array_append(arr, &s);  /* pass pointer-to-pointer; append copies sizeof(void*) bytes */
    }
    return arr;
}

/* ── std/env, std/time, std/process, std/regex ───────────────────────────── */
/* Forward declaration — haki_error_new is defined in the Error section below */
void* haki_error_new(const char* message);

#ifndef _WIN32
#include <unistd.h>   /* getcwd, chdir, setenv, unsetenv */
#include <errno.h>


/* ── std/env ─────────────────────────────────────────────────────────────── */

void* haki_env_get(const char* name) {
    const char* val = getenv(name);
    if (!val) {
        void* err = haki_error_new("environment variable not set");
        void** result = (void**)malloc(2 * sizeof(void*));
        result[0] = (void*)strdup("");
        result[1] = err;
        return (void*)result;
    }
    void** result = (void**)malloc(2 * sizeof(void*));
    result[0] = (void*)strdup(val);
    result[1] = NULL;
    return (void*)result;
}

void* haki_env_set(const char* name, const char* value) {
#ifdef _WIN32
    if (_putenv_s(name, value) != 0)
        return haki_error_new("failed to set environment variable");
#else
    if (setenv(name, value, 1) != 0)
        return haki_error_new(strerror(errno));
#endif
    return NULL;
}

void* haki_env_unset(const char* name) {
#ifdef _WIN32
    _putenv_s(name, "");
#else
    unsetenv(name);
#endif
    return NULL;
}

void* haki_env_cwd(void) {
    char buf[4096];
#ifdef _WIN32
    if (!_getcwd(buf, sizeof(buf))) {
#else
    if (!getcwd(buf, sizeof(buf))) {
#endif
        void** r = (void**)malloc(2 * sizeof(void*));
        r[0] = (void*)strdup("");
        r[1] = haki_error_new(strerror(errno));
        return (void*)r;
    }
    void** r = (void**)malloc(2 * sizeof(void*));
    r[0] = (void*)strdup(buf);
    r[1] = NULL;
    return (void*)r;
}

void* haki_env_chdir(const char* path) {
#ifdef _WIN32
    if (_chdir(path) != 0)
#else
    if (chdir(path) != 0)
#endif
        return haki_error_new(strerror(errno));
    return NULL;
}

/* ── std/time ────────────────────────────────────────────────────────────── */

#include <time.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <sys/time.h>
#endif

int64_t haki_time_now_ms(void) {
#ifdef _WIN32
    FILETIME ft;
    GetSystemTimeAsFileTime(&ft);
    ULONGLONG t = ((ULONGLONG)ft.dwHighDateTime << 32) | ft.dwLowDateTime;
    return (int64_t)((t - 116444736000000000ULL) / 10000);
#else
    struct timeval tv;
    gettimeofday(&tv, NULL);
    return (int64_t)tv.tv_sec * 1000LL + tv.tv_usec / 1000LL;
#endif
}

void haki_time_sleep_ms(int64_t ms) {
    if (ms <= 0) return;
#ifdef _WIN32
    Sleep((DWORD)ms);
#else
    struct timespec ts;
    ts.tv_sec  = ms / 1000;
    ts.tv_nsec = (ms % 1000) * 1000000L;
    nanosleep(&ts, NULL);
#endif
}

const char* haki_time_format(int64_t unix_sec) {
    time_t t = (time_t)unix_sec;
    struct tm* tm_info = gmtime(&t);
    char* buf = (char*)malloc(32);
    strftime(buf, 32, "%Y-%m-%dT%H:%M:%SZ", tm_info);
    return buf;
}

/* ── std/process ─────────────────────────────────────────────────────────── */

#include <stdio.h>

/* run: capture stdout, return as string */
void* haki_process_run(const char* cmd, void* args_arr) {
    /* Build command string from cmd + args array */
    char buf[4096];
    int64_t len = haki_array_length(args_arr);
    snprintf(buf, sizeof(buf), "%s", cmd);
    for (int64_t i = 0; i < len; i++) {
        const char* arg = *(const char**)haki_array_get(args_arr, i);
        strncat(buf, " ", sizeof(buf) - strlen(buf) - 1);
        strncat(buf, arg, sizeof(buf) - strlen(buf) - 1);
    }
#ifdef _WIN32
    FILE* fp = _popen(buf, "r");
#else
    FILE* fp = popen(buf, "r");
#endif
    if (!fp) {
        void** r = (void**)malloc(2 * sizeof(void*));
        r[0] = (void*)strdup("");
        r[1] = haki_error_new("failed to run command");
        return (void*)r;
    }
    /* Read stdout */
    char out[65536]; out[0] = '\0';
    char line[1024];
    while (fgets(line, sizeof(line), fp)) {
        strncat(out, line, sizeof(out) - strlen(out) - 1);
    }
    int status;
#ifdef _WIN32
    status = _pclose(fp);
#else
    status = pclose(fp);
#endif
    void** r = (void**)malloc(2 * sizeof(void*));
    if (status != 0) {
        r[0] = (void*)strdup(out);
        r[1] = haki_error_new("command exited with non-zero status");
    } else {
        r[0] = (void*)strdup(out);
        r[1] = NULL;
    }
    return (void*)r;
}

/* exec: inherit stdio, return exit code */
void* haki_process_exec(const char* cmd, void* args_arr) {
    char buf[4096];
    int64_t len = haki_array_length(args_arr);
    snprintf(buf, sizeof(buf), "%s", cmd);
    for (int64_t i = 0; i < len; i++) {
        const char* arg = *(const char**)haki_array_get(args_arr, i);
        strncat(buf, " ", sizeof(buf) - strlen(buf) - 1);
        strncat(buf, arg, sizeof(buf) - strlen(buf) - 1);
    }
    int code = system(buf);
    void** r = (void**)malloc(2 * sizeof(void*));
    int64_t* code_ptr = (int64_t*)malloc(sizeof(int64_t));
    *code_ptr = (int64_t)code;
    r[0] = (void*)code_ptr;
    r[1] = NULL;
    return (void*)r;
}

/* shell: run via /bin/sh -c, capture output */
void* haki_process_shell(const char* cmd) {
#ifdef _WIN32
    FILE* fp = _popen(cmd, "r");
#else
    FILE* fp = popen(cmd, "r");
#endif
    if (!fp) {
        void** r = (void**)malloc(2 * sizeof(void*));
        r[0] = (void*)strdup("");
        r[1] = haki_error_new("failed to run shell command");
        return (void*)r;
    }
    char out[65536]; out[0] = '\0';
    char line[1024];
    while (fgets(line, sizeof(line), fp)) {
        strncat(out, line, sizeof(out) - strlen(out) - 1);
    }
#ifdef _WIN32
    _pclose(fp);
#else
    pclose(fp);
#endif
    void** r = (void**)malloc(2 * sizeof(void*));
    r[0] = (void*)strdup(out);
    r[1] = NULL;
    return (void*)r;
}

void haki_process_exit(int64_t code) {
    exit((int)code);
}

/* ── std/regex ───────────────────────────────────────────────────────────── */

#include <regex.h>

int8_t haki_regex_matches(const char* s, const char* pattern) {
    regex_t re;
    if (regcomp(&re, pattern, REG_EXTENDED | REG_NOSUB) != 0) return 0;
    int match = regexec(&re, s, 0, NULL, 0);
    regfree(&re);
    return match == 0 ? 1 : 0;
}

void* haki_regex_find(const char* s, const char* pattern) {
    regex_t re;
    void** r = (void**)malloc(2 * sizeof(void*));
    if (regcomp(&re, pattern, REG_EXTENDED) != 0) {
        r[0] = (void*)strdup("");
        r[1] = haki_error_new("invalid regex pattern");
        return (void*)r;
    }
    regmatch_t match;
    if (regexec(&re, s, 1, &match, 0) == 0) {
        int len = match.rm_eo - match.rm_so;
        char* found = (char*)malloc(len + 1);
        strncpy(found, s + match.rm_so, len);
        found[len] = '\0';
        r[0] = (void*)found;
        r[1] = NULL;
    } else {
        r[0] = (void*)strdup("");
        r[1] = haki_error_new("no match found");
    }
    regfree(&re);
    return (void*)r;
}

const char* haki_regex_replace_all(const char* s, const char* pattern, const char* replacement) {
    regex_t re;
    if (regcomp(&re, pattern, REG_EXTENDED) != 0) return strdup(s);
    /* Allocate generous output buffer */
    size_t outsize = strlen(s) * 4 + 1024;
    char* out = (char*)malloc(outsize);
    out[0] = '\0';
    const char* cur = s;
    regmatch_t match;
    while (*cur && regexec(&re, cur, 1, &match, 0) == 0) {
        /* Append text before match */
        strncat(out, cur, match.rm_so);
        /* Append replacement */
        strncat(out, replacement, outsize - strlen(out) - 1);
        cur += match.rm_eo;
        if (match.rm_eo == 0) { /* zero-width match guard */
            if (*cur) { char c[2] = {*cur, '\0'}; strncat(out, c, 1); cur++; }
            else break;
        }
    }
    strncat(out, cur, outsize - strlen(out) - 1);
    regfree(&re);
    return out;
}

void* haki_regex_split(const char* s, const char* pattern) {
    regex_t re;
    void* arr = haki_array_new(sizeof(void*));
    if (regcomp(&re, pattern, REG_EXTENDED) != 0) {
        char* copy = strdup(s);
        haki_array_append(arr, &copy);
        return arr;
    }
    const char* cur = s;
    regmatch_t match;
    while (*cur && regexec(&re, cur, 1, &match, 0) == 0) {
        if (match.rm_so > 0) {
            char* part = (char*)malloc(match.rm_so + 1);
            strncpy(part, cur, match.rm_so);
            part[match.rm_so] = '\0';
            haki_array_append(arr, &part);
        }
        cur += match.rm_eo;
        if (match.rm_eo == 0) { if (*cur) cur++; else break; }
    }
    if (*cur) {
        char* rest = strdup(cur);
        haki_array_append(arr, &rest);
    }
    regfree(&re);
    return arr;
}


/* haki_regex_find_groups: return Array<string> of capture groups (groups[0] = first capture).
   Returns empty array if no match or invalid pattern.
   Uses POSIX ERE — patterns should use [0-9] not \d, [a-z] not \w, etc. */
void* haki_regex_find_groups(const char* s, const char* pattern) {
    void* farr = haki_array_new(sizeof(void*));
    regex_t fre;
    /* count capture groups (unescaped '(') */
    int fngroups = 0;
    for (const char* fp = pattern; *fp; fp++) {
        if (*fp == '\\') { fp++; continue; }
        if (*fp == '(')  fngroups++;
    }
    if (fngroups == 0) return farr;  /* no capture groups */
    if (fngroups > 32) fngroups = 32;
    /* need fngroups+1 slots: slot 0 = full match, slots 1..fngroups = captures */
    int fntotal = fngroups + 1;
    if (regcomp(&fre, pattern, REG_EXTENDED) != 0) return farr;
    regmatch_t fgrp[33];
    if (regexec(&fre, s, (size_t)fntotal, fgrp, 0) == 0) {
        /* skip fgrp[0] (full match), return only capture groups */
        for (int fi = 1; fi < fntotal; fi++) {
            if (fgrp[fi].rm_so < 0) {
                char* fe = strdup(""); haki_array_append(farr, &fe);
            } else {
                int fgl = fgrp[fi].rm_eo - fgrp[fi].rm_so;
                char* fp2 = (char*)malloc(fgl + 1);
                strncpy(fp2, s + fgrp[fi].rm_so, fgl); fp2[fgl] = '\0';
                haki_array_append(farr, &fp2);
            }
        }
    }
    regfree(&fre);
    return farr;
}

/* ── std/time extensions ─────────────────────────────────────────────────── */

int64_t haki_time_parse(const char* s) {
    struct tm ttm; memset(&ttm, 0, sizeof(ttm));
    int tn = sscanf(s, "%d-%d-%dT%d:%d:%d",
        &ttm.tm_year, &ttm.tm_mon, &ttm.tm_mday,
        &ttm.tm_hour, &ttm.tm_min, &ttm.tm_sec);
    if (tn < 3) return -1;
    ttm.tm_year -= 1900; ttm.tm_mon -= 1; ttm.tm_isdst = 0;
#ifdef _WIN32
    return (int64_t)_mkgmtime(&ttm);
#else
    return (int64_t)timegm(&ttm);
#endif
}

const char* haki_time_format_pattern(int64_t unix_sec, const char* pattern) {
    time_t tt = (time_t)unix_sec;
    struct tm* tmi = gmtime(&tt);
    char* tbuf = (char*)malloc(256);
    strftime(tbuf, 256, pattern, tmi);
    return tbuf;
}

int64_t haki_time_diff_sec(int64_t a, int64_t b) { return a - b; }

const char* haki_time_format_tz(int64_t unix_sec, int64_t offset_minutes) {
    time_t tt2 = (time_t)(unix_sec + offset_minutes * 60);
    struct tm* tmi2 = gmtime(&tt2);
    char* tbuf2 = (char*)malloc(32);
    strftime(tbuf2, 32, "%Y-%m-%dT%H:%M:%S", tmi2);
    int tabs = (int)(offset_minutes < 0 ? -offset_minutes : offset_minutes);
    char tsign = offset_minutes >= 0 ? '+' : '-';
    char ttz[8]; snprintf(ttz, sizeof(ttz), "%c%02d:%02d", tsign, tabs/60, tabs%60);
    strncat(tbuf2, ttz, 31 - strlen(tbuf2));
    return tbuf2;
}

int64_t haki_time_day_of_week(int64_t unix_sec) {
    time_t tt3 = (time_t)unix_sec;
    return (int64_t)gmtime(&tt3)->tm_wday;
}

int64_t haki_time_start_of_day(int64_t unix_sec) {
    return unix_sec - (unix_sec % 86400);
}

const char* haki_time_day_name(int64_t wday) {
    const char* tdays[] = {"Sunday","Monday","Tuesday","Wednesday","Thursday","Friday","Saturday"};
    if (wday < 0 || wday > 6) return strdup("Unknown");
    return strdup(tdays[wday]);
}

const char* haki_time_month_name(int64_t month) {
    const char* tmonths[] = {"","January","February","March","April","May","June",
                              "July","August","September","October","November","December"};
    if (month < 1 || month > 12) return strdup("Unknown");
    return strdup(tmonths[month]);
}

/* ── std/json nested parser ──────────────────────────────────────────────── */

typedef struct { const char* src; int jpos; int jlen; } JParser;
static void jskip(JParser* jp) {
    while (jp->jpos < jp->jlen && (jp->src[jp->jpos]==' '||jp->src[jp->jpos]=='\t'||
           jp->src[jp->jpos]=='\n'||jp->src[jp->jpos]=='\r')) jp->jpos++;
}
static char* jstr(JParser* jp) {
    if (jp->jpos >= jp->jlen || jp->src[jp->jpos] != '"') return strdup("");
    jp->jpos++;
    int js = jp->jpos, jol = 0;
    while (jp->jpos < jp->jlen && jp->src[jp->jpos] != '"') {
        if (jp->src[jp->jpos] == '\\') { jp->jpos++; jol++; }
        else jol++;
        jp->jpos++;
    }
    int je = jp->jpos;
    if (jp->jpos < jp->jlen) jp->jpos++;
    char* jo = (char*)malloc(jol + 1); int joi = 0;
    for (int ji = js; ji < je; ) {
        if (jp->src[ji] == '\\' && ji + 1 < je) {
            ji++;
            switch (jp->src[ji]) {
                case '"': jo[joi++]='"'; break; case '\\': jo[joi++]='\\'; break;
                case 'n': jo[joi++]='\n'; break; case 'r': jo[joi++]='\r'; break;
                case 't': jo[joi++]='\t'; break; default: jo[joi++]=jp->src[ji]; break;
            }
            ji++;
        } else jo[joi++] = jp->src[ji++];
    }
    jo[joi] = '\0'; return jo;
}
static char* jval(JParser* jp) {
    jskip(jp);
    if (jp->jpos >= jp->jlen) return strdup("null");
    int jvs = jp->jpos; char jvc = jp->src[jp->jpos];
    if (jvc == '"') {
        jp->jpos++;
        while (jp->jpos < jp->jlen && jp->src[jp->jpos] != '"') {
            if (jp->src[jp->jpos] == '\\') jp->jpos++;
            jp->jpos++;
        }
        if (jp->jpos < jp->jlen) jp->jpos++;
    } else if (jvc == '{' || jvc == '[') {
        char jcl = (jvc=='{') ? '}' : ']'; int jd = 1; jp->jpos++;
        while (jp->jpos < jp->jlen && jd > 0) {
            char jcc = jp->src[jp->jpos];
            if (jcc == '"') { jp->jpos++;
                while (jp->jpos < jp->jlen && jp->src[jp->jpos] != '"') {
                    if (jp->src[jp->jpos] == '\\') jp->jpos++; jp->jpos++; }
                if (jp->jpos < jp->jlen) jp->jpos++;
            } else if (jcc==jvc) { jd++; jp->jpos++; }
            else if (jcc==jcl) { jd--; jp->jpos++; }
            else jp->jpos++;
        }
        (void)jcl;
    } else {
        while (jp->jpos < jp->jlen) {
            char jcc2 = jp->src[jp->jpos];
            if (jcc2==','||jcc2=='}'||jcc2==']'||jcc2==' '||jcc2=='\n'||jcc2=='\r'||jcc2=='\t') break;
            jp->jpos++;
        }
    }
    int jrl = jp->jpos - jvs; char* jr = (char*)malloc(jrl+1);
    strncpy(jr, jp->src + jvs, jrl); jr[jrl]='\0'; return jr;
}
static HakiMap* jpobj(JParser* jp) {
    HakiMap* jm = haki_map_new(sizeof(void*));
    jskip(jp);
    if (jp->jpos >= jp->jlen || jp->src[jp->jpos] != '{') return jm;
    jp->jpos++;
    while (1) {
        jskip(jp);
        if (jp->jpos >= jp->jlen || jp->src[jp->jpos] == '}') { jp->jpos++; break; }
        if (jp->src[jp->jpos] == ',') { jp->jpos++; continue; }
        char* jk = jstr(jp); jskip(jp);
        if (jp->jpos < jp->jlen && jp->src[jp->jpos] == ':') jp->jpos++;
        jskip(jp);
        char* jv2 = (jp->jpos < jp->jlen && jp->src[jp->jpos] == '"') ? jstr(jp) : jval(jp);
        haki_map_set(jm, jk, (void*)jv2); free(jk);
    }
    return jm;
}
void* haki_json_parse_nested(const char* s) {
    JParser jp; jp.src=s; jp.jpos=0; jp.jlen=(int)strlen(s);
    return (void*)jpobj(&jp);
}
const char* haki_json_encode_nested(void* jm_ptr) {
    HakiMap* jm = (HakiMap*)jm_ptr;
    size_t jcap=64; char* jout=(char*)malloc(jcap); strcpy(jout,"{"); int jfirst=1;
    for (int64_t ji=0; ji<(jm?jm->capacity:0); ji++) {
        const char* jk2=haki_map_entry_key(jm,ji);
        if(!jk2) continue;
        void* jvp=haki_map_get(jm,jk2); if(!jvp) continue;
        const char* jv3=(const char*)jvp;
        size_t jn=strlen(jout)+strlen(jk2)+strlen(jv3)+16;
        if(jn>jcap){jcap=jn*2;jout=(char*)realloc(jout,jcap);}
        if(!jfirst) strncat(jout,",",jcap-strlen(jout)-1);
        jfirst=0;
        strncat(jout,"\"",jcap-strlen(jout)-1); strncat(jout,jk2,jcap-strlen(jout)-1);
        strncat(jout,"\":",jcap-strlen(jout)-1);
        char jf=jv3[0];
        int jraw=(jf=='{'||jf=='['||(jf>='0'&&jf<='9')||jf=='-'||
                  strcmp(jv3,"true")==0||strcmp(jv3,"false")==0||strcmp(jv3,"null")==0);
        if(jraw) strncat(jout,jv3,jcap-strlen(jout)-1);
        else { strncat(jout,"\"",jcap-strlen(jout)-1); strncat(jout,jv3,jcap-strlen(jout)-1); strncat(jout,"\"",jcap-strlen(jout)-1); }
    }
    strncat(jout,"}",jcap-strlen(jout)-1); return jout;
}

/* ── JSON flat API (haki_json_str/num/flag/encode_object/encode_array/decode/decode_get)
   Called by std/json stdlib. Self-contained: uses JParser/jpobj/haki_map_xx/haki_array_get
   and haki_error_new defined above. Works in both HTTP and non-HTTP builds. */
#ifndef HAKI_JSON_FLAT_DEFINED
#define HAKI_JSON_FLAT_DEFINED
#ifndef HAKI_JSON_TUPLE2_DEFINED
#define HAKI_JSON_TUPLE2_DEFINED
typedef struct { void* f0; void* f1; } HakiJsonTuple2;
#endif
const char* haki_json_str(const char* s) {
    if(!s) return strdup("null");
    size_t len=strlen(s); char* out=(char*)malloc(len*6+3); char* p=out; *p++='"';
    while(*s){
        unsigned char c=(unsigned char)*s++;
        switch(c){
            case '"':  *p++='\\'; *p++='"';  break;
            case '\\': *p++='\\'; *p++='\\'; break;
            case '\n': *p++='\\'; *p++='n';  break;
            case '\r': *p++='\\'; *p++='r';  break;
            case '\t': *p++='\\'; *p++='t';  break;
            default: if(c<0x20){p+=sprintf(p,"\\u%04x",c);}else{*p++=(char)c;} break;
        }
    }
    *p++='"'; *p='\0'; return out;
}
const char* haki_json_num(int64_t n) {
    char buf[32]; snprintf(buf,sizeof(buf),"%lld",(long long)n); return strdup(buf);
}
const char* haki_json_flag(int b) { return strdup(b?"true":"false"); }
const char* haki_json_encode_object(void* m_ptr) {
    HakiMap* m=(HakiMap*)m_ptr;
    if(!m) return strdup("{}");
    size_t cap=32,olen=0; char* out=(char*)malloc(cap); out[olen++]='{'; int jeo_first=1;
    for(int64_t i=0;i<m->capacity;i++){
        const char* k=haki_map_entry_key(m,i); if(!k) continue;
        void* vp=haki_map_get(m,k);
        const char* v=vp?(const char*)vp:"null";
        const char* ks=haki_json_str(k);
        size_t ksl=strlen(ks); size_t vl=v?strlen(v):4;
        size_t needed=olen+ksl+1+vl+3;
        while(cap<=needed){cap*=2;out=(char*)realloc(out,cap);}
        if(!jeo_first) out[olen++]=','; jeo_first=0;
        memcpy(out+olen,ks,ksl); olen+=ksl; free((void*)ks);
        out[olen++]=':';
        if(v){memcpy(out+olen,v,vl);olen+=vl;}else{memcpy(out+olen,"null",4);olen+=4;}
    }
    out[olen++]='}'; out[olen]='\0'; return out;
}
const char* haki_json_encode_array(void* arr_ptr) {
    HakiArray* arr=(HakiArray*)arr_ptr;
    if(!arr||arr->length==0) return strdup("[]");
    size_t cap=32,olen=0; char* out=(char*)malloc(cap); out[olen++]='[';
    for(int64_t i=0;i<arr->length;i++){
        void* ep=haki_array_get(arr,i);
        const char* v=ep?*(const char**)ep:"null";
        size_t vl=v?strlen(v):4;
        size_t needed=olen+vl+3;
        while(cap<=needed){cap*=2;out=(char*)realloc(out,cap);}
        if(i>0) out[olen++]=',';
        if(v){memcpy(out+olen,v,vl);olen+=vl;}else{memcpy(out+olen,"null",4);olen+=4;}
    }
    out[olen++]=']'; out[olen]='\0'; return out;
}
void* haki_json_decode(const char* s) {
    HakiMap* jd_m=NULL; const char* jd_err=NULL;
    if(!s){ jd_err="null input"; }
    else {
        JParser jp; jp.src=s; jp.jpos=0; jp.jlen=(int)strlen(s);
        jskip(&jp);
        if(jp.jpos>=jp.jlen||jp.src[jp.jpos]!='{'){ jd_err="expected JSON object"; }
        else { jd_m=jpobj(&jp); }
    }
    HakiJsonTuple2* t=(HakiJsonTuple2*)malloc(sizeof(HakiJsonTuple2));
    t->f0=jd_m; t->f1=jd_err?haki_error_new(jd_err):NULL;
    return (void*)t;
}
const char* haki_json_decode_get(const char* s,const char* key) {
    if(!s||!key) return strdup("");
    JParser jp; jp.src=s; jp.jpos=0; jp.jlen=(int)strlen(s);
    HakiMap* m=jpobj(&jp);
    if(!m) return strdup("");
    void* vp=haki_map_get(m,key);
    const char* result=vp?strdup((const char*)vp):strdup("");
    haki_map_free(m); return result;
}
#endif /* HAKI_JSON_FLAT_DEFINED */

/* ── std/csv ─────────────────────────────────────────────────────────────── */

#ifndef HAKI_CSV_DEFINED
#define HAKI_CSV_DEFINED

/* haki_csv_parse_row: parse one CSV/TSV line into HakiArray* of char*.
   sep = delimiter as int64_t (44 = comma, 9 = tab).
   NOTE: uses haki_array_append(&field) so elem_size=sizeof(char*) is correct. */
void* haki_csv_parse_row(const char* line, int64_t sep_i) {
    char sep = (char)(int)sep_i;
    HakiArray* arr = haki_array_new(sizeof(char*));
    if (!line) return (void*)arr;
    const char* p = line;
    int csv_at_end = 0;
    do {
        char* field = NULL;
        size_t flen = 0, fcap = 16;
        field = (char*)malloc(fcap);
        if (*p == '"') {
            /* Quoted field */
            p++;
            while (*p) {
                if (*p == '"') {
                    if (*(p+1) == '"') {
                        /* escaped quote: "" -> " */
                        if (flen+1 >= fcap) { fcap*=2; field=(char*)realloc(field,fcap); }
                        field[flen++] = '"'; p += 2;
                    } else { p++; break; /* closing quote */ }
                } else {
                    if (flen+1 >= fcap) { fcap*=2; field=(char*)realloc(field,fcap); }
                    field[flen++] = *p++;
                }
            }
            if (*p == sep) p++;
            else csv_at_end = 1;
        } else {
            /* Unquoted field */
            while (*p && *p != sep) {
                if (flen+1 >= fcap) { fcap*=2; field=(char*)realloc(field,fcap); }
                field[flen++] = *p++;
            }
            if (*p == sep) p++;
            else csv_at_end = 1;
        }
        field[flen] = '\0';
        haki_array_append(arr, &field); /* &field: copy char* value into array */
    } while (!csv_at_end);
    return (void*)arr;
}

/* haki_csv_encode_row: encode HakiArray* of char* into a CSV/TSV row string. */
const char* haki_csv_encode_row(void* fields_ptr, int64_t sep_i) {
    char sep = (char)(int)sep_i;
    HakiArray* arr = (HakiArray*)fields_ptr;
    if (!arr || arr->length == 0) return strdup("");
    size_t cap = 64, olen = 0;
    char* out = (char*)malloc(cap);
    for (int64_t ci = 0; ci < arr->length; ci++) {
        const char* field = *(const char**)haki_array_get(arr, ci); /* double deref */
        if (!field) field = "";
        if (ci > 0) {
            if (olen+1 >= cap) { cap*=2; out=(char*)realloc(out,cap); }
            out[olen++] = sep;
        }
        /* check if quoting needed */
        int needs_q = 0;
        const char* fp = field;
        while (*fp) {
            if (*fp == sep || *fp == '"' || *fp == '\n' || *fp == '\r') { needs_q=1; break; }
            fp++;
        }
        if (needs_q) {
            size_t flen = strlen(field);
            while (olen + flen*2 + 4 >= cap) { cap*=2; out=(char*)realloc(out,cap); }
            out[olen++] = '"';
            fp = field;
            while (*fp) {
                if (*fp == '"') out[olen++] = '"'; /* escape */
                out[olen++] = *fp++;
            }
            out[olen++] = '"';
        } else {
            size_t flen = strlen(field);
            while (olen + flen + 2 >= cap) { cap*=2; out=(char*)realloc(out,cap); }
            memcpy(out+olen, field, flen); olen += flen;
        }
    }
    out[olen] = '\0';
    return out;
}

/* haki_csv_parse: parse full CSV/TSV string into HakiArray* of HakiArray*.
   Returns HakiJsonTuple2 { f0=rows, f1=error? }. */
void* haki_csv_parse(const char* s, int64_t sep) {
    HakiArray* rows = NULL;
    const char* csv_parse_err = NULL;
    if (!s) { csv_parse_err = "null input"; }
    else {
        rows = haki_array_new(sizeof(HakiArray*));
        const char* p = s;
        size_t total = strlen(s);
        const char* csv_end = s + total;
        while (p <= csv_end) {
            const char* line_start = p;
            int in_q = 0;
            while (p < csv_end) {
                if (*p == '"') { in_q = !in_q; p++; }
                else if (!in_q && (*p == '\n' || *p == '\r')) { break; }
                else { p++; }
            }
            size_t llen = (size_t)(p - line_start);
            char* lbuf = (char*)malloc(llen + 1);
            memcpy(lbuf, line_start, llen); lbuf[llen] = '\0';
            if (p < csv_end && *p == '\r') p++;
            if (p < csv_end && *p == '\n') p++;
            if (llen == 0 && p >= csv_end) { free(lbuf); break; }
            HakiArray* row = (HakiArray*)haki_csv_parse_row(lbuf, sep);
            free(lbuf);
            haki_array_append(rows, &row); /* &row: copy HakiArray* value */
        }
    }
    HakiJsonTuple2* t = (HakiJsonTuple2*)malloc(sizeof(HakiJsonTuple2));
    t->f0 = (void*)rows;
    t->f1 = csv_parse_err ? haki_error_new(csv_parse_err) : NULL;
    return (void*)t;
}

/* haki_csv_encode: encode HakiArray* of HakiArray* into a CSV/TSV string. */
const char* haki_csv_encode(void* rows_ptr, int64_t sep) {
    HakiArray* rows = (HakiArray*)rows_ptr;
    if (!rows || rows->length == 0) return strdup("");
    size_t cap = 256, olen = 0;
    char* out = (char*)malloc(cap);
    for (int64_t ri = 0; ri < rows->length; ri++) {
        HakiArray* row = *(HakiArray**)haki_array_get(rows, ri); /* double deref */
        if (!row) continue;
        if (ri > 0) {
            if (olen+2 >= cap) { cap*=2; out=(char*)realloc(out,cap); }
            out[olen++] = '\n';
        }
        const char* row_str = haki_csv_encode_row((void*)row, sep);
        size_t rlen = strlen(row_str);
        while (olen + rlen + 2 >= cap) { cap*=2; out=(char*)realloc(out,cap); }
        memcpy(out+olen, row_str, rlen); olen += rlen;
        free((void*)row_str);
    }
    out[olen] = '\0';
    return out;
}

#endif /* HAKI_CSV_DEFINED */


/* ── std/xml ─────────────────────────────────────────────────────────────── */

const char* haki_xml_get_element(const char* xml, const char* tag) {
    char xopen[256], xclose[256];
    snprintf(xopen,sizeof(xopen),"<%s",tag); snprintf(xclose,sizeof(xclose),"</%s>",tag);
    const char* xs=strstr(xml,xopen); if(!xs) return strdup("");
    xs=strchr(xs,'>'); if(!xs) return strdup(""); xs++;
    const char* xe=strstr(xs,xclose); if(!xe) return strdup("");
    int xl=(int)(xe-xs); char* xr=(char*)malloc(xl+1);
    strncpy(xr,xs,xl); xr[xl]='\0'; return xr;
}
void* haki_xml_parse_attrs(const char* attr_str) {
    HakiMap* xm=haki_map_new(sizeof(void*));
    const char* xp=attr_str;
    while(*xp){
        while(*xp==' '||*xp=='\t'||*xp=='\n') xp++;
        if(!*xp) break;
        const char* xks=xp; while(*xp&&*xp!='='&&*xp!=' ') xp++;
        if(!*xp||*xp!='=') break;
        int xkl=(int)(xp-xks); char* xk=(char*)malloc(xkl+1);
        strncpy(xk,xks,xkl); xk[xkl]='\0'; xp++;
        char xq=0; if(*xp=='"'||*xp=='\''){xq=*xp;xp++;}
        const char* xvs=xp; while(*xp&&(xq?*xp!=xq:(*xp!=' '&&*xp!='\t'))) xp++;
        int xvl=(int)(xp-xvs); char* xv=(char*)malloc(xvl+1);
        strncpy(xv,xvs,xvl); xv[xvl]='\0'; if(xq&&*xp) xp++;
        haki_map_set(xm,xk,(void*)xv); free(xk);  /* store char* directly as void* */
    }
    return (void*)xm;
}
const char* haki_xml_get_attr(const char* tag_str, const char* attr_name) {
    size_t xal=strlen(attr_name); const char* xap=tag_str;
    while((xap=strstr(xap,attr_name))!=NULL){
        if(xap>tag_str&&(*(xap-1)==' '||*(xap-1)=='\t'||*(xap-1)=='<')){
            xap+=xal; while(*xap==' ')xap++;
            if(*xap=='='){xap++; while(*xap==' ')xap++;
                char xq2=0; if(*xap=='"'||*xap=='\''){xq2=*xap;xap++;}
                const char* xvs2=xap;
                while(*xap&&(xq2?*xap!=xq2:(*xap!=' '&&*xap!='>')))xap++;
                int xvl2=(int)(xap-xvs2); char* xv2=(char*)malloc(xvl2+1);
                strncpy(xv2,xvs2,xvl2); xv2[xvl2]='\0'; return xv2;
            }
        }
        xap++;
    }
    return strdup("");
}
const char* haki_xml_emit_element(const char* tag, const char* content) {
    size_t xel=strlen(tag)*2+strlen(content)+8; char* xeo=(char*)malloc(xel);
    snprintf(xeo,xel,"<%s>%s</%s>",tag,content,tag); return xeo;
}

const char* haki_xml_emit_tag(const char* tag, HakiMap* attrs) {
    size_t cap = 256;
    char* out = (char*)malloc(cap);
    snprintf(out, cap, "<%s", tag);
    if (attrs) {
        for (int64_t i = 0; i < attrs->capacity; i++) {
            if (attrs->entries[i].key) {
                const char* k = attrs->entries[i].key;
                void* vp = attrs->entries[i].value;
                const char* v = vp ? (const char*)vp : "";  /* Map<string,string> stores char* directly */
                size_t need = strlen(out) + strlen(k) + strlen(v) + 8;
                while (need > cap) { cap *= 2; out = (char*)realloc(out, cap); }
                size_t l = strlen(out);
                snprintf(out + l, cap - l, " %s=\"%s\"", k, v);
            }
        }
    }
    size_t l = strlen(out);
    while (l + 4 > cap) { cap *= 2; out = (char*)realloc(out, cap); }
    out[l++] = '/'; out[l++] = '>'; out[l] = '\0';
    return out;
}
const char* haki_xml_escape(const char* s) {
    size_t xslen=strlen(s); size_t xcap2=xslen*6+1; char* xout=(char*)malloc(xcap2); char* xo=xout;
    for(size_t xi=0;xi<xslen;xi++){
        switch(s[xi]){
            case '&': memcpy(xo,"&amp;",5); xo+=5; break;
            case '<': memcpy(xo,"&lt;",4); xo+=4; break;
            case '>': memcpy(xo,"&gt;",4); xo+=4; break;
            case '"': memcpy(xo,"&quot;",6); xo+=6; break;
            case '\'': memcpy(xo,"&apos;",6); xo+=6; break;
            default: *xo++=s[xi]; break;
        }
    }
    *xo='\0'; return xout;
}

/* ── std/template ────────────────────────────────────────────────────────── */

const char* haki_template_render(const char* tmpl, HakiMap* vars) {
    size_t ttlen=strlen(tmpl); size_t ttcap=ttlen*2+256;
    char* ttout=(char*)malloc(ttcap); ttout[0]='\0';
    const char* ttp=tmpl;
    while(*ttp){
        if(ttp[0]=='{'&&ttp[1]=='{'){
            ttp+=2; while(*ttp==' ')ttp++;
            const char* ttks=ttp; while(*ttp&&!(ttp[0]=='}'&&ttp[1]=='}'))ttp++;
            const char* ttke=ttp; while(ttke>ttks&&*(ttke-1)==' ')ttke--;
            int ttkl=(int)(ttke-ttks); char* ttk=(char*)malloc(ttkl+1);
            strncpy(ttk,ttks,ttkl); ttk[ttkl]='\0';
            if(ttp[0]=='}'&&ttp[1]=='}')ttp+=2;
            void* ttvp=haki_map_get(vars,ttk); const char* ttv=ttvp?(const char*)ttvp:"";
            free(ttk);
            size_t ttn=strlen(ttout)+strlen(ttv)+64;
            if(ttn>ttcap){ttcap=ttn*2;ttout=(char*)realloc(ttout,ttcap);}
            strncat(ttout,ttv,ttcap-strlen(ttout)-1);
        } else {
            size_t ttc=strlen(ttout);
            if(ttc+2>ttcap){ttcap*=2;ttout=(char*)realloc(ttout,ttcap);}
            ttout[ttc]=*ttp; ttout[ttc+1]='\0'; ttp++;
        }
    }
    return ttout;
}
const char* haki_template_html_escape(const char* s) {
    if (!s) return "";
    size_t cap = strlen(s) * 6 + 64;
    char* out = (char*)malloc(cap);
    char* p = out;
    while (*s) {
        if (*s == '&') { memcpy(p,"&amp;",5); p+=5; }
        else if (*s == '<') { memcpy(p,"&lt;",4); p+=4; }
        else if (*s == '>') { memcpy(p,"&gt;",4); p+=4; }
        else if (*s == '"') { memcpy(p,"&quot;",6); p+=6; }
        else if (*s == '\'') { memcpy(p,"&#39;",5); p+=5; }
        else { *p++ = *s; }
        s++;
    }
    *p = '\0';
    return out;
}

/* haki_template_render_full — full template engine with if/else/for blocks.
   Tags supported:
     {{var}}                          variable substitution
     {{#if var}}...{{/if}}            conditional (truthy = non-empty, not "false", not "0")
     {{#if var}}...{{#else}}...{{/if}} conditional with else branch
     {{#for item in list}}...{{/for}} iteration (list = newline-separated values)
*/

/* Copy a HakiMap, then add/override one extra key=value. Used by template for loop. */
static HakiMap* haki_map_copy_with(HakiMap* src, const char* key, const char* val) {
    HakiMap* dst = haki_map_new(sizeof(void*));
    if (src) {
        for (int64_t i = 0; i < src->capacity; i++) {
            if (src->entries[i].key) {
                haki_map_set(dst, src->entries[i].key, src->entries[i].value);
            }
        }
    }
    char* vs = strdup(val);
    haki_map_set(dst, key, (void*)vs);  /* store char* directly as void*, not &vs */
    return dst;
}
static int trf_truthy(const char* v) {
    if (!v || v[0]=='\0') return 0;
    if (strcmp(v,"false")==0||strcmp(v,"0")==0) return 0;
    return 1;
}

static char* trf_mapget(HakiMap* vars, const char* key) {
    if (!vars) return NULL;
    void* vp = haki_map_get(vars, key);
    if (!vp) return NULL;
    return (char*)vp;  /* Map<string,string> stores char* directly as void* */
}

static void trf_append(char** outp, size_t* lenp, size_t* capp, const char* s, size_t sl) {
    if (sl == 0) return;
    if (*lenp + sl + 1 > *capp) {
        while (*lenp + sl + 1 > *capp) *capp *= 2;
        *outp = (char*)realloc(*outp, *capp);
    }
    memcpy(*outp + *lenp, s, sl);
    *lenp += sl;
    (*outp)[*lenp] = '\0';
}

/* forward decl */
static const char* trf_render(const char* p, const char* end, HakiMap* vars, char** outp, size_t* lenp, size_t* capp, int skip);

/* parse {{tag_name rest}} — returns pointer past closing }}, fills tag and rest */
static const char* trf_parse_tag(const char* p, const char* end, char* tag, size_t tsz, char* rest, size_t rsz) {
    /* p points just past '{{' */
    while (p < end && *p == ' ') p++;
    const char* ts = p;
    while (p < end && *p != ' ' && !(p[0]=='}' && p[1]=='}')) p++;
    size_t tl = (size_t)(p - ts);
    if (tl >= tsz) tl = tsz - 1;
    strncpy(tag, ts, tl); tag[tl] = '\0';
    while (p < end && *p == ' ') p++;
    const char* rs = p;
    while (p < end && !(p[0]=='}' && p[1]=='}')) p++;
    size_t rl = (size_t)(p - rs);
    while (rl > 0 && rs[rl-1] == ' ') rl--;
    if (rl >= rsz) rl = rsz - 1;
    strncpy(rest, rs, rl); rest[rl] = '\0';
    if (p+1 < end) p += 2; /* skip }} */
    return p;
}

static const char* trf_render(const char* p, const char* end, HakiMap* vars, char** outp, size_t* lenp, size_t* capp, int skip) {
    char tag[256]; char rest[512];
    while (p < end) {
        if (p[0]=='{' && p+1<end && p[1]=='{') {
            const char* tp = p + 2;
            tp = trf_parse_tag(tp, end, tag, sizeof(tag), rest, sizeof(rest));
            if (tag[0]=='#') {
                /* block open */
                const char* block_name = tag + 1; /* "if" or "for" */
                if (strcmp(block_name, "if") == 0) {
                    /* find matching {{/if}}, respecting nesting */
                    /* render true branch, skip false branch (or vice versa) */
                    char* val = trf_mapget(vars, rest);
                    int cond = trf_truthy(val);
                    int depth = 1;
                    const char* branch_start = tp;
                    const char* else_p = NULL;
                    const char* close_p = NULL;
                    /* scan to find {{#else}} and {{/if}} at depth 1 */
                    const char* sp = tp;
                    while (sp < end) {
                        if (sp[0]=='{' && sp+1<end && sp[1]=='{') {
                            char st[256]; char sr[512];
                            const char* np = trf_parse_tag(sp+2, end, st, sizeof(st), sr, sizeof(sr));
                            if (st[0]=='#' && (strcmp(st+1,"if")==0||strcmp(st+1,"for")==0)) depth++;
                            else if (st[0]=='/' && strcmp(st+1,"if")==0) {
                                depth--;
                                if (depth==0) { close_p = sp; tp = np; break; }
                            } else if (strcmp(st,"\x23" "else")==0 && depth==1) {
                                else_p = sp; sp = np; continue;
                            }
                            sp = np;
                        } else sp++;
                    }
                    if (!skip) {
                        const char* true_end = else_p ? else_p : close_p;
                        const char* false_start = NULL;
                        const char* false_end = close_p;
                        if (else_p) {
                            char st[256]; char sr[512];
                            false_start = trf_parse_tag(else_p+2, end, st, sizeof(st), sr, sizeof(sr));
                        }
                        if (cond) {
                            trf_render(branch_start, true_end ? true_end : end, vars, outp, lenp, capp, 0);
                        } else if (false_start) {
                            trf_render(false_start, false_end ? false_end : end, vars, outp, lenp, capp, 0);
                        }
                    }
                    p = tp;
                } else if (strcmp(block_name, "for") == 0) {
                    /* rest = "item in list_var" */
                    char iter_var[128] = ""; char list_var[128] = "";
                    sscanf(rest, "%127s in %127s", iter_var, list_var);
                    char* list_val = trf_mapget(vars, list_var);
                    /* find {{/for}} */
                    int depth = 1;
                    const char* body_start = tp;
                    const char* close_p = NULL;
                    const char* sp = tp;
                    while (sp < end) {
                        if (sp[0]=='{' && sp+1<end && sp[1]=='{') {
                            char st[256]; char sr[512];
                            const char* np = trf_parse_tag(sp+2, end, st, sizeof(st), sr, sizeof(sr));
                            if (st[0]=='#' && (strcmp(st+1,"for")==0||strcmp(st+1,"if")==0)) depth++;
                            else if (st[0]=='/' && strcmp(st+1,"for")==0) {
                                depth--;
                                if (depth==0) { close_p = sp; tp = np; break; }
                            }
                            sp = np;
                        } else sp++;
                    }
                    if (!skip && list_val && iter_var[0]) {
                        /* iterate newline-separated items */
                        char* buf = strdup(list_val);
                        char* line = strtok(buf, "\n");
                        while (line) {
                            while (*line == '\r') line++;
                            char* le = line + strlen(line);
                            while (le > line && (*(le-1)=='\r'||*(le-1)==' ')) { le--; *le='\0'; }
                            if (*line) {
                                HakiMap* iter_vars = haki_map_copy_with(vars, iter_var, line);
                                trf_render(body_start, close_p ? close_p : end, iter_vars, outp, lenp, capp, 0);
                            }
                            line = strtok(NULL, "\n");
                        }
                        free(buf);
                    }
                    p = tp;
                } else {
                    /* unknown block — skip */
                    p = tp;
                }
            } else if (tag[0]=='/') {
                /* unmatched close tag — stop (caller handles it) */
                break;
            } else if (strcmp(tag,"\x23" "else")==0) {
                /* unmatched else — stop */
                break;
            } else {
                /* variable substitution */
                if (!skip) {
                    char* val = trf_mapget(vars, tag);
                    if (val) trf_append(outp, lenp, capp, val, strlen(val));
                }
                p = tp;
            }
        } else {
            if (!skip) trf_append(outp, lenp, capp, p, 1);
            p++;
        }
    }
    return p;
}

const char* haki_template_render_full(const char* tmpl, HakiMap* vars) {
    if (!tmpl) return "";
    size_t cap = strlen(tmpl) * 2 + 256;
    char* out = (char*)malloc(cap);
    out[0] = '\0';
    size_t len = 0;
    trf_render(tmpl, tmpl + strlen(tmpl), vars, &out, &len, &cap, 0);
    return out;
}


typedef struct {
    void*       __arc;    /* ARC header — must be first */
    const char* message;
    void*       cause;    /* Error? — null if no cause */
} HakiError;

void* haki_error_new(const char* message) {
    HakiError* e = (HakiError*)haki_alloc(sizeof(HakiError));
    e->message = message ? strdup(message) : "";
    e->cause   = NULL;
    return (void*)e;
}

void* haki_error_new_with_cause(const char* message, void* cause) {
    HakiError* e = (HakiError*)haki_alloc(sizeof(HakiError));
    e->message = message ? strdup(message) : "";
    e->cause   = cause;
    return (void*)e;
}

void* haki_error_wrap(const char* message) {
    if (!message) return NULL;
    return haki_error_new(message);
}

const char* haki_error_message(void* err) {
    if (!err) return "";
    return ((HakiError*)err)->message;
}

void* haki_error_cause(void* err) {
    if (!err) return NULL;
    return ((HakiError*)err)->cause;
}

/* ── HTTP Client (libcurl-based) ────────────────────────────────────────────*/
#if (defined(__APPLE__) || defined(__linux__)) && defined(HAKI_BUILD_HTTP)
#include <curl/curl.h>
#define HAKI_HAS_CURL 1
#endif

#ifdef HAKI_HAS_CURL
struct haki_curl_buf { char* data; size_t len; };
static size_t haki_curl_write(char* ptr, size_t sz, size_t nmemb, void* ud) {
    struct haki_curl_buf* b = (struct haki_curl_buf*)ud;
    size_t n = sz * nmemb;
    b->data = realloc(b->data, b->len + n + 1);
    if (!b->data) return 0;
    memcpy(b->data + b->len, ptr, n);
    b->len += n; b->data[b->len] = 0;
    return n;
}
static const char* haki_curl_do(const char* url, const char* method, const char* body, const char* ct, long* status_out) {
    CURL* c = curl_easy_init(); if (!c) { if (status_out) *status_out = -1; return strdup(""); }
    struct haki_curl_buf buf = {0};
    struct curl_slist* hdrs = NULL;
    if (ct && ct[0]) { char h[512]; snprintf(h,512,"Content-Type: %s",ct); hdrs=curl_slist_append(hdrs,h); }
    curl_easy_setopt(c, CURLOPT_URL, url);
    curl_easy_setopt(c, CURLOPT_WRITEFUNCTION, haki_curl_write);
    curl_easy_setopt(c, CURLOPT_WRITEDATA, &buf);
    curl_easy_setopt(c, CURLOPT_FOLLOWLOCATION, 1L);
    if (hdrs) curl_easy_setopt(c, CURLOPT_HTTPHEADER, hdrs);
    if (method && strcmp(method,"POST")==0) {
        curl_easy_setopt(c, CURLOPT_POST, 1L);
        curl_easy_setopt(c, CURLOPT_POSTFIELDS, body ? body : "");
    } else if (method && strcmp(method,"PUT")==0) {
        curl_easy_setopt(c, CURLOPT_CUSTOMREQUEST, "PUT");
        curl_easy_setopt(c, CURLOPT_POSTFIELDS, body ? body : "");
    } else if (method && strcmp(method,"DELETE")==0) {
        curl_easy_setopt(c, CURLOPT_CUSTOMREQUEST, "DELETE");
    }
    CURLcode res = curl_easy_perform(c);
    long code = (res == CURLE_OK) ? 200 : 0;
    curl_easy_getinfo(c, CURLINFO_RESPONSE_CODE, &code);
    if (status_out) *status_out = code;
    curl_slist_free_all(hdrs); curl_easy_cleanup(c);
    return buf.data ? buf.data : strdup("");
}
int64_t haki_http_client_get_status(const char* url) { long s=-1; haki_curl_do(url,"GET",NULL,NULL,&s); return s; }
const char* haki_http_client_get_body(const char* url) { return haki_curl_do(url,"GET",NULL,NULL,NULL); }
int64_t haki_http_client_post_status(const char* url,const char* body,const char* ct){long s=-1;haki_curl_do(url,"POST",body,ct,&s);return s;}
const char* haki_http_client_post_body(const char* url,const char* body,const char* ct){return haki_curl_do(url,"POST",body,ct,NULL);}
int64_t haki_http_client_put_status(const char* url,const char* body,const char* ct){long s=-1;haki_curl_do(url,"PUT",body,ct,&s);return s;}
const char* haki_http_client_put_body(const char* url,const char* body,const char* ct){return haki_curl_do(url,"PUT",body,ct,NULL);}
int64_t haki_http_client_delete_status(const char* url){long s=-1;haki_curl_do(url,"DELETE",NULL,NULL,&s);return s;}
const char* haki_http_client_delete_body(const char* url){return haki_curl_do(url,"DELETE",NULL,NULL,NULL);}
int64_t haki_http_client_get_headers_status(const char* url,const char* hdrs){long s=-1;haki_curl_do(url,"GET",NULL,hdrs,&s);return s;}
const char* haki_http_client_get_headers_body(const char* url,const char* hdrs){return haki_curl_do(url,"GET",NULL,hdrs,NULL);}
/* Combined single-request fetch: returns __PayloadTuple2 { f0=status(int), f1=body(string) } */
void* haki_http_fetch(const char* url, const char* method, const char* body, const char* ct) {
    long status = 0;
    const char* resp = haki_curl_do(url, method, body, ct, &status);
    __PayloadTuple2* t = (__PayloadTuple2*)malloc(sizeof(__PayloadTuple2));
    t->f0 = (void*)(intptr_t)status;
    t->f1 = (void*)resp;
    return (void*)t;
}
#else
int64_t haki_http_client_get_status(const char* u){return -1;}
const char* haki_http_client_get_body(const char* u){return "curl not available";}
int64_t haki_http_client_post_status(const char* u,const char* b,const char* c){return -1;}
const char* haki_http_client_post_body(const char* u,const char* b,const char* c){return "curl not available";}
int64_t haki_http_client_put_status(const char* u,const char* b,const char* c){return -1;}
const char* haki_http_client_put_body(const char* u,const char* b,const char* c){return "curl not available";}
int64_t haki_http_client_delete_status(const char* u){return -1;}
const char* haki_http_client_delete_body(const char* u){return "curl not available";}
int64_t haki_http_client_get_headers_status(const char* u,const char* h){return -1;}
const char* haki_http_client_get_headers_body(const char* u,const char* h){return "curl not available";}
void* haki_http_fetch(const char* u,const char* m,const char* b,const char* ct){
    __PayloadTuple2* t=(__PayloadTuple2*)malloc(sizeof(__PayloadTuple2));
    t->f0=(void*)(intptr_t)-1; t->f1=(void*)"curl not available"; return (void*)t;
}
#endif


/* ── Filesystem (std/fs) ────────────────────────────────────────────────────*/
#include <dirent.h>
#include <sys/stat.h>
#include <unistd.h>
#include <errno.h>

#define HAKI_FS_MAX_ENTRIES 4096
static char* haki_fs_dir_names[HAKI_FS_MAX_ENTRIES];
static char* haki_fs_dir_paths[HAKI_FS_MAX_ENTRIES];
static int   haki_fs_dir_isdirs[HAKI_FS_MAX_ENTRIES];
static int64_t haki_fs_dir_sizes[HAKI_FS_MAX_ENTRIES];
static int   haki_fs_dir_count = 0;

int64_t haki_fs_readdir_count(const char* path) {
    DIR* d = opendir(path); if (!d) return -1;
    haki_fs_dir_count = 0;
    struct dirent* e;
    while ((e = readdir(d)) && haki_fs_dir_count < HAKI_FS_MAX_ENTRIES) {
        if (strcmp(e->d_name,".")==0 || strcmp(e->d_name,"..")==0) continue;
        char full[4096]; snprintf(full,4096,"%s/%s",path,e->d_name);
        haki_fs_dir_names[haki_fs_dir_count] = strdup(e->d_name);
        haki_fs_dir_paths[haki_fs_dir_count] = strdup(full);
        struct stat st; stat(full,&st);
        haki_fs_dir_isdirs[haki_fs_dir_count] = S_ISDIR(st.st_mode) ? 1 : 0;
        haki_fs_dir_sizes[haki_fs_dir_count]  = (int64_t)st.st_size;
        haki_fs_dir_count++;
    }
    closedir(d);
    return haki_fs_dir_count;
}
const char* haki_fs_readdir_name(const char* p,int64_t i){(void)p;return(i>=0&&i<haki_fs_dir_count)?haki_fs_dir_names[i]:"";}
const char* haki_fs_readdir_path(const char* p,int64_t i){(void)p;return(i>=0&&i<haki_fs_dir_count)?haki_fs_dir_paths[i]:"";}
int64_t haki_fs_readdir_isdir(const char* p,int64_t i){(void)p;return(i>=0&&i<haki_fs_dir_count)?haki_fs_dir_isdirs[i]:0;}
int64_t haki_fs_readdir_size(const char* p,int64_t i){(void)p;return(i>=0&&i<haki_fs_dir_count)?haki_fs_dir_sizes[i]:0;}

int64_t haki_fs_mkdir(const char* p){return mkdir(p,0755)<0?-1:0;}
int64_t haki_fs_rmdir(const char* p){return rmdir(p)<0?-1:0;}
int64_t haki_fs_delete_file(const char* p){return unlink(p)<0?-1:0;}
int64_t haki_fs_exists(const char* p){struct stat s;return stat(p,&s)==0?1:0;}
int64_t haki_fs_is_dir(const char* p){struct stat s;return(stat(p,&s)==0&&S_ISDIR(s.st_mode))?1:0;}
int64_t haki_fs_is_file(const char* p){struct stat s;return(stat(p,&s)==0&&S_ISREG(s.st_mode))?1:0;}
/* bool-returning path check wrappers for std/fs */
int8_t haki_fs_path_exists(const char* p){struct stat s;return(int8_t)(stat(p,&s)==0);}
int8_t haki_fs_path_is_dir(const char* p){struct stat s;return(int8_t)(stat(p,&s)==0&&S_ISDIR(s.st_mode));}
int8_t haki_fs_path_is_file(const char* p){struct stat s;return(int8_t)(stat(p,&s)==0&&S_ISREG(s.st_mode));}


int64_t haki_fs_mkdir_all(const char* path) {
    char tmp[4096]; snprintf(tmp,4096,"%s",path);
    for (char* p=tmp+1;*p;p++) {
        if (*p=='/') { *p=0; mkdir(tmp,0755); *p='/'; }
    }
    return mkdir(tmp,0755)<0 && errno!=EEXIST ? -1 : 0;
}
int64_t haki_fs_copy_file(const char* src,const char* dst) {
    FILE* in=fopen(src,"rb"); if(!in) return -1;
    FILE* out=fopen(dst,"wb"); if(!out){fclose(in);return -1;}
    char buf[65536]; size_t n;
    while((n=fread(buf,1,sizeof(buf),in))>0) fwrite(buf,1,n,out);
    fclose(in); fclose(out); return 0;
}
const char* haki_fs_ext(const char* p) {
    const char* d=strrchr(p,'.'); return d?d:"";
}
const char* haki_fs_basename(const char* p) {
    const char* s=strrchr(p,'/'); return s?s+1:p;
}
const char* haki_fs_dirname(const char* p) {
    static char buf[4096]; snprintf(buf,4096,"%s",p);
    char* s=strrchr(buf,'/'); if(s){*s=0;return buf;} return ".";
}



/* ── String parsing ─────────────────────────────────────────────────────────*/
/* string_to_int: returns Tuple2{int64_t, void*} where void* is null on success */
/* string_to_int/float defined in RUNTIME_C_SOURCE after __Tuple2 */

#endif /* _WIN32 — closes env block */

/* ── Terminal I/O ────────────────────────────────────────────────────────────*/
/* Read a line from stdin (strips trailing newline). Returns heap string. */
const char* haki_read_line(void) {
    char buf[4096];
    if (!fgets(buf, sizeof(buf), stdin)) return strdup("");
    size_t len = strlen(buf);
    if (len > 0 && buf[len-1] == '\n') buf[len-1] = '\0';
    return strdup(buf);
}
/* Print without trailing newline (for prompts). */
void haki_print_no_newline(const char* s) {
    fputs(s, stdout); fflush(stdout);
}
/* Read a single character from stdin (no echo needed). */
int64_t haki_read_char(void) {
    int c = getchar();
    return (c == EOF) ? -1 : (int64_t)c;
}
/* Check if stdin has more input (non-blocking). */
int8_t haki_stdin_has_input(void) { return 1; }

"#;

pub const RUNTIME_C_SOURCE: &str = r#"

/* haki_runtime.c — Haki v0.1 runtime
   Compile: clang -c haki_runtime.c -o haki_runtime.o               */

/* Closure fat pointer: { fn_ptr, env_ptr } packed into void*[2] on heap */
#include <stdlib.h>
static inline void* haki_make_closure(void* fn_ptr, void* env_ptr) {
    void** fat = (void**)malloc(2 * sizeof(void*));
    fat[0] = fn_ptr;
    fat[1] = env_ptr;
    return (void*)fat;
}

/* Multi-field enum payload structs used by generated match arms */
/* f0/f1 etc store pointers to the actual values (indirection level) */
typedef struct { void* f0; void* f1; } __PayloadTuple2;
typedef struct { void* f0; void* f1; void* f2; } __PayloadTuple3;
typedef struct { void* f0; void* f1; void* f2; void* f3; } __PayloadTuple4;
/* Helper: dereference a void* as a string (char**) */
#define __HAKI_STR(p) (*(const char**)(p))
#define __HAKI_I64(p) (*(int64_t*)(p))
#define __HAKI_FN(p)  (*(void**)(p))

#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <stdatomic.h>
#include <ctype.h>
#include <errno.h>

/* ── ARC ─────────────────────────────────────────────────────────── */

typedef struct {
    _Atomic int64_t refcount;
    void (*destructor)(void*);
} HakiObjHeader;

void haki_retain(void* obj) {
    if (!obj) return;
    atomic_fetch_add_explicit(
        &((HakiObjHeader*)obj)->refcount, 1, memory_order_relaxed);
}

void haki_release(void* obj) {
    if (!obj) return;
    HakiObjHeader* h = (HakiObjHeader*)obj;
    int64_t prev = atomic_fetch_sub_explicit(
        &h->refcount, 1, memory_order_acq_rel);
    if (prev == 1) {
        if (h->destructor) h->destructor(obj);
        free(obj);
    }
}

void* haki_alloc(int64_t size) {
    void* p = calloc(1, (size_t)size);
    if (!p) { fprintf(stderr, "haki: out of memory\n"); abort(); }
    atomic_store_explicit(
        &((HakiObjHeader*)p)->refcount, 1, memory_order_relaxed);
    return p;
}

void* haki_weak_load(void** slot) {
    void* obj = *slot;
    if (!obj) return NULL;
    int64_t rc = atomic_load_explicit(
        &((HakiObjHeader*)obj)->refcount, memory_order_acquire);
    return (rc > 0) ? obj : NULL;
}

/* ── Print ───────────────────────────────────────────────────────── */

void haki_print(const char* msg) {
    if (!msg) msg = "(null)";
    puts(msg);
}

void haki_print_int(int64_t n) {
    printf("%lld\n", (long long)n);
}

void haki_print_float(double f) {
    printf("%g\n", f);
}

void haki_print_bool(int8_t b) {
    puts(b ? "true" : "false");
}

/* ── String utilities ────────────────────────────────────────────── */

void* haki_string_concat(const char* a, const char* b) {
    if (!a) a = "";
    if (!b) b = "";
    size_t la = strlen(a), lb = strlen(b);
    char* r = (char*)malloc(la + lb + 1);
    if (!r) abort();
    memcpy(r, a, la);
    memcpy(r + la, b, lb + 1);
    return r;
}

int64_t haki_string_length(const char* s) {
    return s ? (int64_t)strlen(s) : 0;
}

/* Returns a heap-allocated string — caller owns it. */
void* haki_int_to_string(int64_t n) {
    char buf[32];
    snprintf(buf, sizeof(buf), "%lld", (long long)n);
    return strdup(buf);
}

void* haki_float_to_string(double f) {
    char buf[64];
    snprintf(buf, sizeof(buf), "%g", f);
    return strdup(buf);
}

void* haki_bool_to_string(int8_t b) {
    return strdup(b ? "true" : "false");
}

/* ── Panic ───────────────────────────────────────────────────────── */

void haki_panic(const char* msg) {
    fflush(stdout);  /* flush buffered output before aborting */
    fprintf(stderr, "\nhaki panic: %s\n", msg ? msg : "(no message)");
    abort();
}

/* ── Array<T> ───────────────────────────────────────────────────── */

typedef struct {
    int64_t length;
    int64_t capacity;
    void*   data;
    int64_t elem_size;
} HakiArray;

HakiArray* haki_array_new(int64_t elem_size) {
    HakiArray* a = (HakiArray*)malloc(sizeof(HakiArray));
    if (!a) abort();
    a->length    = 0;
    a->capacity  = 8;
    a->elem_size = elem_size;
    a->data      = malloc((size_t)(8 * elem_size));
    if (!a->data) abort();
    return a;
}

HakiArray* haki_array_from(void* src, int64_t count, int64_t elem_size) {
    HakiArray* a = haki_array_new(elem_size);
    a->length   = count;
    a->capacity = count > 8 ? count : 8;
    free(a->data);
    a->data = malloc((size_t)(a->capacity * elem_size));
    if (!a->data) abort();
    if (src && count > 0) memcpy(a->data, src, (size_t)(count * elem_size));
    return a;
}

void haki_array_append(HakiArray* a, void* item) {
    if (!a) return;
    if (a->length == a->capacity) {
        a->capacity *= 2;
        a->data = realloc(a->data, (size_t)(a->capacity * a->elem_size));
        if (!a->data) abort();
    }
    memcpy((char*)a->data + a->length * a->elem_size, item, (size_t)a->elem_size);
    a->length++;
}

void* haki_array_get(HakiArray* a, int64_t idx) {
    if (!a || idx < 0 || idx >= a->length) {
        fprintf(stderr, "haki panic: array index %lld out of bounds (length %lld)\n",
            (long long)idx, a ? (long long)a->length : 0LL);
        abort();
    }
    return (char*)a->data + idx * a->elem_size;
}

int64_t haki_array_length(HakiArray* a) {
    return a ? a->length : 0;
}

void haki_array_free(HakiArray* a) {
    if (a) { free(a->data); free(a); }
}

/* removeLast: pop the last element; returns pointer to it (caller must read
   before next append which may realloc). Returns NULL if empty.             */
void* haki_array_remove_last(HakiArray* a) {
    if (!a || a->length == 0) return NULL;
    a->length--;
    return (char*)a->data + a->length * a->elem_size;
}

/* removeAt: remove element at index, shift left, return heap copy of element. */
void* haki_array_remove_at(HakiArray* a, int64_t idx) {
    if (!a || idx < 0 || idx >= a->length) return NULL;
    void* removed = malloc((size_t)a->elem_size);
    if (!removed) return NULL;
    memcpy(removed, (char*)a->data + idx * a->elem_size, (size_t)a->elem_size);
    memmove((char*)a->data + idx * a->elem_size,
            (char*)a->data + (idx + 1) * a->elem_size,
            (size_t)((a->length - idx - 1) * a->elem_size));
    a->length--;
    return removed;
}

/* contains (int): 1 if val is present, 0 otherwise. */
int64_t haki_array_contains_int(HakiArray* a, int64_t val) {
    if (!a) return 0;
    for (int64_t i = 0; i < a->length; i++) {
        if (*(int64_t*)((char*)a->data + i * a->elem_size) == val) return 1;
    }
    return 0;
}

/* contains (string): 1 if val is present, 0 otherwise. */
int64_t haki_array_contains_str(HakiArray* a, const char* val) {
    if (!a || !val) return 0;
    for (int64_t i = 0; i < a->length; i++) {
        const char* e = *(const char**)((char*)a->data + i * a->elem_size);
        if (e && strcmp(e, val) == 0) return 1;
    }
    return 0;
}

/* indexOf (int): returns first index == val, or -1. */
int64_t haki_array_index_of_int(HakiArray* a, int64_t val) {
    if (!a) return -1;
    for (int64_t i = 0; i < a->length; i++) {
        if (*(int64_t*)((char*)a->data + i * a->elem_size) == val) return i;
    }
    return -1;
}

/* indexOf (string): returns first index matching val, or -1. */
int64_t haki_array_index_of_str(HakiArray* a, const char* val) {
    if (!a || !val) return -1;
    for (int64_t i = 0; i < a->length; i++) {
        const char* e = *(const char**)((char*)a->data + i * a->elem_size);
        if (e && strcmp(e, val) == 0) return i;
    }
    return -1;
}

/* first / last: pointer to first/last element slot. NULL if empty. */
void* haki_array_first(HakiArray* a) {
    return (a && a->length > 0) ? a->data : NULL;
}
void* haki_array_last(HakiArray* a) {
    return (a && a->length > 0)
        ? (char*)a->data + (a->length - 1) * a->elem_size : NULL;
}

/* join: concatenate string array with separator. Caller owns the result. */
const char* haki_array_join(HakiArray* a, const char* sep) {
    if (!a || a->length == 0) return strdup("");
    if (!sep) sep = "";
    size_t sep_len = strlen(sep), total = 0;
    for (int64_t i = 0; i < a->length; i++) {
        const char* e = *(const char**)((char*)a->data + i * a->elem_size);
        if (e) total += strlen(e);
        if (i < a->length - 1) total += sep_len;
    }
    char* r = (char*)malloc(total + 1); char* p = r;
    for (int64_t i = 0; i < a->length; i++) {
        const char* e = *(const char**)((char*)a->data + i * a->elem_size);
        if (e) { size_t l = strlen(e); memcpy(p, e, l); p += l; }
        if (i < a->length - 1) { memcpy(p, sep, sep_len); p += sep_len; }
    }
    *p = '\0'; return r;
}

/* ── Map<K, V> ──────────────────────────────────────────────────────
   Open-addressing hash map with string keys (v0.1).
   Values are stored as opaque byte blobs of fixed elem_size.
   String keys are copied and owned by the map.               */

#define HAKI_MAP_INIT_CAP 16

typedef struct HakiMapEntry {
    char*  key;     /* heap-owned copy; NULL = empty slot */
    void*  value;   /* heap-owned value bytes             */
} HakiMapEntry;

typedef struct {
    HakiMapEntry* entries;
    int64_t       capacity;
    int64_t       length;   /* number of live entries */
    int64_t       val_size; /* sizeof(V) in bytes     */
} HakiMap;

static uint64_t haki_fnv1a(const char* s) {
    uint64_t h = 14695981039346656037ULL;
    while (*s) { h ^= (uint8_t)*s++; h *= 1099511628211ULL; }
    return h;
}

HakiMap* haki_map_new(int64_t val_size) {
    HakiMap* m = (HakiMap*)calloc(1, sizeof(HakiMap));
    if (!m) abort();
    m->val_size = val_size;
    m->capacity = HAKI_MAP_INIT_CAP;
    m->entries  = (HakiMapEntry*)calloc((size_t)m->capacity, sizeof(HakiMapEntry));
    if (!m->entries) abort();
    return m;
}

static void haki_map_insert_entry(HakiMap* m, char* key, void* val) {
    uint64_t idx = haki_fnv1a(key) % (uint64_t)m->capacity;
    for (int64_t i = 0; i < m->capacity; i++) {
        int64_t slot = (int64_t)((idx + (uint64_t)i) % (uint64_t)m->capacity);
        HakiMapEntry* e = &m->entries[slot];
        if (!e->key) {
            e->key   = key;
            e->value = val;
            m->length++;
            return;
        }
        if (strcmp(e->key, key) == 0) {
            free(e->value);
            e->value = val;
            free(key); /* duplicate key — discard */
            return;
        }
    }
    abort(); /* should never happen if load < 0.75 */
}

static void haki_map_grow(HakiMap* m) {
    int64_t old_cap = m->capacity;
    HakiMapEntry* old = m->entries;
    m->capacity *= 2;
    m->length   = 0;
    m->entries  = (HakiMapEntry*)calloc((size_t)m->capacity, sizeof(HakiMapEntry));
    if (!m->entries) abort();
    for (int64_t i = 0; i < old_cap; i++) {
        if (old[i].key) haki_map_insert_entry(m, old[i].key, old[i].value);
    }
    free(old);
}

void haki_map_set(HakiMap* m, const char* key, void* val_ptr) {
    if (!m || !key) return;
    if (m->length * 4 >= m->capacity * 3) haki_map_grow(m);
    char* k = strdup(key);
    if (!k) abort();
    // Store val_ptr directly — values are either pointer-sized primitives
    // (int/float/bool stored as (void*)(intptr_t)value) or pointer-to-heap-object.
    // No malloc/memcpy needed; the pointer IS the value.
    haki_map_insert_entry(m, k, val_ptr);
}

/* Returns pointer to value, or NULL if not found. */
void* haki_map_get(HakiMap* m, const char* key) {
    if (!m || !key || m->length == 0) return NULL;
    uint64_t idx = haki_fnv1a(key) % (uint64_t)m->capacity;
    for (int64_t i = 0; i < m->capacity; i++) {
        int64_t slot = (int64_t)((idx + (uint64_t)i) % (uint64_t)m->capacity);
        HakiMapEntry* e = &m->entries[slot];
        if (!e->key) return NULL; /* empty slot = not found */
        if (strcmp(e->key, key) == 0) return e->value;
    }
    return NULL;
}

int8_t haki_map_has(HakiMap* m, const char* key) {
    return haki_map_get(m, key) != NULL ? 1 : 0;
}

void haki_map_delete(HakiMap* m, const char* key) {
    if (!m || !key) return;
    uint64_t idx = haki_fnv1a(key) % (uint64_t)m->capacity;
    for (int64_t i = 0; i < m->capacity; i++) {
        int64_t slot = (int64_t)((idx + (uint64_t)i) % (uint64_t)m->capacity);
        HakiMapEntry* e = &m->entries[slot];
        if (!e->key) return;
        if (strcmp(e->key, key) == 0) {
            free(e->key);  e->key   = NULL;
            free(e->value); e->value = NULL;
            m->length--;
            return;
        }
    }
}

int64_t haki_map_length(HakiMap* m) { return m ? m->length : 0; }

/* getOrDefault: returns pointer to value if found, else pointer to default. */
void* haki_map_get_or_default(HakiMap* m, const char* key, void* default_val) {
    void* found = haki_map_get(m, key);
    return found ? found : default_val;
}

void haki_map_free(HakiMap* m) {
    if (!m) return;
    for (int64_t i = 0; i < m->capacity; i++) {
        if (m->entries[i].key)   free(m->entries[i].key);
        if (m->entries[i].value) free(m->entries[i].value);
    }
    free(m->entries);
    free(m);
}

/* Map iteration helpers */
int64_t haki_map_capacity(HakiMap* m) { return m ? m->capacity : 0; }
#ifndef HAKI_MAP_ENTRY_DEFINED
#define HAKI_MAP_ENTRY_DEFINED
const char* haki_map_entry_key(HakiMap* m, int64_t i) {
    if (!m || i < 0 || i >= m->capacity) return NULL;
    return m->entries[i].key;
}
void* haki_map_entry_value(HakiMap* m, int64_t i) {
    if (!m || i < 0 || i >= m->capacity) return NULL;
    return m->entries[i].value;
}
#endif /* HAKI_MAP_ENTRY_DEFINED */


/* ── Thread (OS 1:1 pthread wrapper) ────────────────────────────── */

#include <pthread.h>

typedef struct {
    pthread_t tid;
} HakiThread;

HakiThread* haki_thread_spawn(void (*fn)(void)) {
    HakiThread* t = (HakiThread*)malloc(sizeof(HakiThread));
    if (!t) abort();
    /* pthread entry must be void* (*)(void*); wrap the no-arg fn */
    typedef void* (*pthread_fn)(void*);
    if (pthread_create(&t->tid, NULL, (pthread_fn)(void*)fn, NULL) != 0) {
        fprintf(stderr, "haki: thread_spawn failed\n");
        abort();
    }
    return t;
}

void haki_thread_join(HakiThread* t) {
    if (!t) return;
    pthread_join(t->tid, NULL);
    free(t);
}

/* ── Task<T> — call-site async + thread pool ─────────────────────
   haki_task_spawn(fn, arg) → HakiTask*
     Submits fn(arg) to the global thread pool. Returns a Task handle.
   haki_task_await(task) → void*
     Blocks until complete, returns the result pointer.
   haki_task_detach(task)
     Drops the user handle — task runs to completion in background.  */

#define HAKI_POOL_THREADS 8
#define HAKI_QUEUE_CAP    256

typedef void* (*HakiTaskFn)(void*);

typedef struct {
    _Atomic int64_t refcount;
    HakiTaskFn      fn;
    void*           arg;
    void*           result;
    int8_t          done;
    pthread_mutex_t done_mu;
    pthread_cond_t  done_cond;
} HakiTask;

static struct {
    pthread_t       threads[HAKI_POOL_THREADS];
    HakiTask*       queue[HAKI_QUEUE_CAP];
    int             head, tail, size;
    pthread_mutex_t mu;
    pthread_cond_t  cond;
    int             running;
} haki_pool_g;

static void haki_task_deref(HakiTask* t) {
    if (!t) return;
    if (atomic_fetch_sub_explicit(&t->refcount, 1, memory_order_acq_rel) == 1) {
        pthread_mutex_destroy(&t->done_mu);
        pthread_cond_destroy(&t->done_cond);
        free(t);
    }
}

static void* haki_pool_worker(void* unused) {
    (void)unused;
    for (;;) {
        pthread_mutex_lock(&haki_pool_g.mu);
        while (haki_pool_g.size == 0)
            pthread_cond_wait(&haki_pool_g.cond, &haki_pool_g.mu);
        HakiTask* t = haki_pool_g.queue[haki_pool_g.head];
        haki_pool_g.head = (haki_pool_g.head + 1) % HAKI_QUEUE_CAP;
        haki_pool_g.size--;
        pthread_mutex_unlock(&haki_pool_g.mu);

        void* res = t->fn(t->arg);

        pthread_mutex_lock(&t->done_mu);
        t->result = res;
        t->done   = 1;
        pthread_cond_broadcast(&t->done_cond);
        pthread_mutex_unlock(&t->done_mu);

        haki_task_deref(t); /* release executor reference */
    }
    return NULL;
}

static void haki_pool_ensure_started(void) {
    if (haki_pool_g.running) return;
    pthread_mutex_init(&haki_pool_g.mu, NULL);
    pthread_cond_init(&haki_pool_g.cond, NULL);
    for (int i = 0; i < HAKI_POOL_THREADS; i++)
        pthread_create(&haki_pool_g.threads[i], NULL, haki_pool_worker, NULL);
    haki_pool_g.running = 1;
}

HakiTask* haki_task_spawn(HakiTaskFn fn, void* arg) {
    haki_pool_ensure_started();
    HakiTask* t = (HakiTask*)calloc(1, sizeof(HakiTask));
    if (!t) abort();
    atomic_store_explicit(&t->refcount, 2, memory_order_relaxed); /* user + executor */
    t->fn  = fn;
    t->arg = arg;
    pthread_mutex_init(&t->done_mu, NULL);
    pthread_cond_init(&t->done_cond, NULL);

    pthread_mutex_lock(&haki_pool_g.mu);
    if (haki_pool_g.size >= HAKI_QUEUE_CAP) {
        pthread_mutex_unlock(&haki_pool_g.mu);
        fprintf(stderr, "haki: task queue full\n");
        abort();
    }
    haki_pool_g.queue[haki_pool_g.tail] = t;
    haki_pool_g.tail = (haki_pool_g.tail + 1) % HAKI_QUEUE_CAP;
    haki_pool_g.size++;
    pthread_cond_signal(&haki_pool_g.cond);
    pthread_mutex_unlock(&haki_pool_g.mu);
    return t;
}

void* haki_task_await(HakiTask* t) {
    if (!t) return NULL;
    pthread_mutex_lock(&t->done_mu);
    while (!t->done)
        pthread_cond_wait(&t->done_cond, &t->done_mu);
    void* res = t->result;
    pthread_mutex_unlock(&t->done_mu);
    haki_task_deref(t); /* release user reference */
    return res;
}

void haki_task_detach(HakiTask* t) {
    haki_task_deref(t); /* release user reference; executor ref keeps it alive */
}

/* ── Mutex<T> ────────────────────────────────────────────────────── */

typedef struct {
    pthread_mutex_t mu;
    void*           value;
} HakiMutex;

typedef struct {
    HakiMutex* mutex;
    void*      value;
} HakiMutexGuard;

HakiMutex* haki_mutex_new(void* initial) {
    HakiMutex* m = (HakiMutex*)malloc(sizeof(HakiMutex));
    if (!m) abort();
    pthread_mutex_init(&m->mu, NULL);
    m->value = initial;
    return m;
}

HakiMutexGuard* haki_mutex_lock(HakiMutex* m) {
    pthread_mutex_lock(&m->mu);
    HakiMutexGuard* g = (HakiMutexGuard*)malloc(sizeof(HakiMutexGuard));
    if (!g) abort();
    g->mutex = m;
    g->value = m->value;
    return g;
}

void haki_mutex_unlock(HakiMutexGuard* g) {
    if (!g) return;
    pthread_mutex_unlock(&g->mutex->mu);
    free(g);
}

void haki_mutex_free(HakiMutex* m) {
    if (!m) return;
    pthread_mutex_destroy(&m->mu);
    free(m);
}


/* ── Chan<T> — bounded/unbounded MPMC channel ────────────────────────────────
   Based on a ring buffer (bounded) or a linked list (unbounded, cap=0).
   Thread-safe via a mutex + two condition variables (not_full, not_empty).
   Close semantics: after close(), receive() drains remaining items then
   returns NULL. Further sends after close() abort (programming error).

   select() support: each channel maintains a list of registered HakiSelectWaiter
   structs. When an item arrives (or the channel closes), the channel signals
   all waiters via a shared condition variable so select() can re-evaluate.
   ─────────────────────────────────────────────────────────────────────────── */

/* Maximum capacity for a bounded channel (0 = unbounded linked-list mode). */
#define HAKI_CHAN_UNBOUNDED 0

/* Linked-list node for unbounded channels. */
typedef struct HakiChanNode {
    void*              val;
    struct HakiChanNode* next;
} HakiChanNode;

/* Waiter registered by select() — woken when any channel becomes ready. */
typedef struct HakiSelectWaiter {
    pthread_mutex_t*        mu;    /* the shared select mutex to signal on */
    pthread_cond_t*         cond;  /* the shared select cond to broadcast on */
    struct HakiSelectWaiter* next;
} HakiSelectWaiter;

typedef struct {
    /* Ring buffer (bounded mode, cap > 0) */
    void**              buf;
    int64_t             cap;        /* 0 = unbounded */
    int64_t             head;
    int64_t             tail;
    int64_t             count;

    /* Linked list tail/head (unbounded mode, cap == 0) */
    HakiChanNode*       ll_head;
    HakiChanNode*       ll_tail;

    int8_t              closed;

    pthread_mutex_t     mu;
    pthread_cond_t      not_empty;
    pthread_cond_t      not_full;

    /* select() waiter list — protected by mu */
    HakiSelectWaiter*   waiters;
} HakiChan;
#define HAKI_CHAN_TYPES_DEFINED

HakiChan* haki_chan_new(int64_t capacity) {
    HakiChan* ch = (HakiChan*)calloc(1, sizeof(HakiChan));
    if (!ch) abort();
    ch->cap = capacity;
    if (capacity > 0) {
        ch->buf = (void**)calloc((size_t)capacity, sizeof(void*));
        if (!ch->buf) abort();
    }
    pthread_mutex_init(&ch->mu, NULL);
    pthread_cond_init(&ch->not_empty, NULL);
    pthread_cond_init(&ch->not_full, NULL);
    return ch;
}

/* Wake all registered select() waiters — call with ch->mu held. */
static void haki_chan_wake_waiters(HakiChan* ch) {
    HakiSelectWaiter* w = ch->waiters;
    while (w) {
        pthread_mutex_lock(w->mu);
        pthread_cond_broadcast(w->cond);
        pthread_mutex_unlock(w->mu);
        w = w->next;
    }
}

void haki_chan_send(HakiChan* ch, void* val) {
    if (!ch) abort();
    pthread_mutex_lock(&ch->mu);
    if (ch->closed) {
        pthread_mutex_unlock(&ch->mu);
        fprintf(stderr, "haki: send on closed channel\n");
        abort();
    }
    if (ch->cap > 0) {
        /* Bounded: block while full */
        while (ch->count >= ch->cap && !ch->closed)
            pthread_cond_wait(&ch->not_full, &ch->mu);
        if (ch->closed) { pthread_mutex_unlock(&ch->mu); abort(); }
        ch->buf[ch->tail] = val;
        ch->tail = (ch->tail + 1) % ch->cap;
        ch->count++;
    } else {
        /* Unbounded: allocate a node */
        HakiChanNode* node = (HakiChanNode*)malloc(sizeof(HakiChanNode));
        if (!node) abort();
        node->val  = val;
        node->next = NULL;
        if (ch->ll_tail) ch->ll_tail->next = node;
        else             ch->ll_head = node;
        ch->ll_tail = node;
        ch->count++;
    }
    haki_chan_wake_waiters(ch);
    pthread_cond_signal(&ch->not_empty);
    pthread_mutex_unlock(&ch->mu);
}

/* Returns NULL if channel is closed and empty. Blocks otherwise. */
void* haki_chan_receive(HakiChan* ch) {
    if (!ch) return NULL;
    pthread_mutex_lock(&ch->mu);
    while (ch->count == 0 && !ch->closed)
        pthread_cond_wait(&ch->not_empty, &ch->mu);
    if (ch->count == 0 && ch->closed) {
        pthread_mutex_unlock(&ch->mu);
        return NULL;
    }
    void* val;
    if (ch->cap > 0) {
        val = ch->buf[ch->head];
        ch->head = (ch->head + 1) % ch->cap;
    } else {
        HakiChanNode* node = ch->ll_head;
        val = node->val;
        ch->ll_head = node->next;
        if (!ch->ll_head) ch->ll_tail = NULL;
        free(node);
    }
    ch->count--;
    pthread_cond_signal(&ch->not_full);
    pthread_mutex_unlock(&ch->mu);
    return val;
}

/* Non-blocking receive — returns NULL immediately if nothing available. */
void* haki_chan_try_receive(HakiChan* ch) {
    if (!ch) return NULL;
    pthread_mutex_lock(&ch->mu);
    if (ch->count == 0) { pthread_mutex_unlock(&ch->mu); return NULL; }
    void* val;
    if (ch->cap > 0) {
        val = ch->buf[ch->head];
        ch->head = (ch->head + 1) % ch->cap;
    } else {
        HakiChanNode* node = ch->ll_head;
        val = node->val;
        ch->ll_head = node->next;
        if (!ch->ll_head) ch->ll_tail = NULL;
        free(node);
    }
    ch->count--;
    pthread_cond_signal(&ch->not_full);
    pthread_mutex_unlock(&ch->mu);
    return val;
}

void haki_chan_close(HakiChan* ch) {
    if (!ch) return;
    pthread_mutex_lock(&ch->mu);
    ch->closed = 1;
    haki_chan_wake_waiters(ch);
    pthread_cond_broadcast(&ch->not_empty);
    pthread_cond_broadcast(&ch->not_full);
    pthread_mutex_unlock(&ch->mu);
}

int8_t haki_chan_is_closed(HakiChan* ch) {
    if (!ch) return 1;
    pthread_mutex_lock(&ch->mu);
    int8_t c = ch->closed && ch->count == 0;
    pthread_mutex_unlock(&ch->mu);
    return c;
}

int8_t haki_chan_has_item(HakiChan* ch) {
    if (!ch) return 0;
    pthread_mutex_lock(&ch->mu);
    int8_t ok = ch->count > 0;
    pthread_mutex_unlock(&ch->mu);
    return ok;
}

void haki_chan_free(HakiChan* ch) {
    if (!ch) return;
    if (ch->buf) free(ch->buf);
    /* drain linked list */
    HakiChanNode* n = ch->ll_head;
    while (n) { HakiChanNode* nx = n->next; free(n); n = nx; }
    pthread_mutex_destroy(&ch->mu);
    pthread_cond_destroy(&ch->not_empty);
    pthread_cond_destroy(&ch->not_full);
    free(ch);
}

/* Register a select waiter on a channel. Call with ch->mu held. */
static void haki_chan_add_waiter(HakiChan* ch, HakiSelectWaiter* w) {
    w->next    = ch->waiters;
    ch->waiters = w;
}

/* Remove a select waiter from a channel. Call with ch->mu held. */
static void haki_chan_remove_waiter(HakiChan* ch, HakiSelectWaiter* w) {
    HakiSelectWaiter** p = &ch->waiters;
    while (*p && *p != w) p = &(*p)->next;
    if (*p) *p = w->next;
}

/* ── select() — multiplex over N channels ────────────────────────────────────
   Usage:
     HakiChan* chans[N]  = { ch1, ch2, ... };
     int        ops[N]   = { 0 (recv), 0, 1 (send), ... };
     void*      vals[N]  = { NULL, NULL, send_val, ... };
     int ready = haki_select(N, chans, ops, vals);
     // ready: index of the arm that fired, -1 if all closed

   Algorithm (deadlock-free):
     1. Sort channels by pointer address to define a global lock order.
     2. Lock all channels in order.
     3. Check each for readiness — take first ready arm.
     4. If none ready: register a HakiSelectWaiter on each, wait on shared cond.
     5. Unlock all channels, wait.
     6. On wakeup: re-lock all (in order), re-check, take ready arm.
     7. Remove waiters from all channels, unlock, return index.
   ─────────────────────────────────────────────────────────────────────────── */

#define HAKI_SELECT_MAX 64

/* Sort indices by channel pointer for consistent lock ordering. */
static void haki_select_sort_order(int n, HakiChan** chans, int* order) {
    for (int i = 0; i < n; i++) order[i] = i;
    /* Insertion sort — n is small (≤ 64) */
    for (int i = 1; i < n; i++) {
        int key = order[i];
        int j   = i - 1;
        while (j >= 0 && (uintptr_t)chans[order[j]] > (uintptr_t)chans[key]) {
            order[j+1] = order[j];
            j--;
        }
        order[j+1] = key;
    }
}

/* Check if arm i is ready (call with all channels locked). */
static int haki_select_arm_ready(int i, HakiChan** chans, int* ops) {
    HakiChan* ch = chans[i];
    if (ops[i] == 0) /* receive */
        return ch->count > 0 || (ch->closed);
    else              /* send */
        return !ch->closed && (ch->cap == 0 || ch->count < ch->cap);
}

/* timeout_ms: -1 = block forever; >= 0 = return -1 after that many milliseconds */
int haki_select(int n, HakiChan** chans, int* ops, void** vals, int64_t timeout_ms) {
    if (n <= 0 || n > HAKI_SELECT_MAX) return -1;

    int order[HAKI_SELECT_MAX];
    haki_select_sort_order(n, chans, order);

    pthread_mutex_t sel_mu;
    pthread_cond_t  sel_cond;
    pthread_mutex_init(&sel_mu, NULL);
    pthread_cond_init(&sel_cond, NULL);

    HakiSelectWaiter waiters[HAKI_SELECT_MAX];
    for (int i = 0; i < n; i++) {
        waiters[i].mu   = &sel_mu;
        waiters[i].cond = &sel_cond;
        waiters[i].next = NULL;
    }

    /* Compute absolute deadline once (CLOCK_REALTIME for pthread_cond_timedwait) */
    struct timespec deadline = {0, 0};
    int has_timeout = (timeout_ms >= 0);
    if (has_timeout) {
        clock_gettime(CLOCK_REALTIME, &deadline);
        int64_t ns = deadline.tv_nsec + (timeout_ms % 1000LL) * 1000000LL;
        deadline.tv_sec  += (time_t)(timeout_ms / 1000LL) + (time_t)(ns / 1000000000LL);
        deadline.tv_nsec  = (long)(ns % 1000000000LL);
    }

    int result = -1;

    for (;;) {
        /* Lock all channels in consistent order */
        for (int i = 0; i < n; i++)
            pthread_mutex_lock(&chans[order[i]]->mu);

        /* Check readiness — pick first ready arm */
        /* Randomise start offset to avoid starvation */
        int start = (int)(((uintptr_t)&result >> 4) % (uintptr_t)n);
        for (int k = 0; k < n; k++) {
            int i = (start + k) % n;
            if (!haki_select_arm_ready(i, chans, ops)) continue;
            HakiChan* ch = chans[i];
            if (ops[i] == 0) { /* receive */
                void* v = NULL;
                if (ch->count > 0) {
                    if (ch->cap > 0) {
                        v = ch->buf[ch->head];
                        ch->head = (ch->head + 1) % ch->cap;
                    } else {
                        HakiChanNode* node = ch->ll_head;
                        v = node->val;
                        ch->ll_head = node->next;
                        if (!ch->ll_head) ch->ll_tail = NULL;
                        free(node);
                    }
                    ch->count--;
                    pthread_cond_signal(&ch->not_full);
                }
                vals[i] = v;
            } else {           /* send */
                if (ch->cap > 0) {
                    ch->buf[ch->tail] = vals[i];
                    ch->tail = (ch->tail + 1) % ch->cap;
                    ch->count++;
                } else {
                    HakiChanNode* node = (HakiChanNode*)malloc(sizeof(HakiChanNode));
                    if (!node) abort();
                    node->val  = vals[i];
                    node->next = NULL;
                    if (ch->ll_tail) ch->ll_tail->next = node;
                    else             ch->ll_head = node;
                    ch->ll_tail = node;
                    ch->count++;
                }
                pthread_cond_signal(&ch->not_empty);
                haki_chan_wake_waiters(ch);
            }
            result = i;
            break;
        }

        if (result >= 0) {
            /* Unlock in reverse order */
            for (int i = n-1; i >= 0; i--)
                pthread_mutex_unlock(&chans[order[i]]->mu);
            break;
        }

        /* Check if all channels are closed and empty — unblockable */
        int all_dead = 1;
        for (int i = 0; i < n; i++) {
            if (ops[i] == 0 && !(chans[i]->closed && chans[i]->count == 0))
                { all_dead = 0; break; }
        }
        if (all_dead) {
            for (int i = n-1; i >= 0; i--)
                pthread_mutex_unlock(&chans[order[i]]->mu);
            result = -1;
            break;
        }

        /* Register waiters on each channel */
        for (int i = 0; i < n; i++)
            haki_chan_add_waiter(chans[i], &waiters[i]);

        /* Unlock all and wait for a signal (or timeout) */
        pthread_mutex_lock(&sel_mu);
        for (int i = n-1; i >= 0; i--)
            pthread_mutex_unlock(&chans[order[i]]->mu);

        int wait_rc = 0;
        if (has_timeout) {
            wait_rc = pthread_cond_timedwait(&sel_cond, &sel_mu, &deadline);
        } else {
            pthread_cond_wait(&sel_cond, &sel_mu);
        }
        pthread_mutex_unlock(&sel_mu);

        /* Remove waiters before re-locking channels */
        for (int i = 0; i < n; i++)
            pthread_mutex_lock(&chans[order[i]]->mu);
        for (int i = 0; i < n; i++)
            haki_chan_remove_waiter(chans[i], &waiters[i]);
        for (int i = n-1; i >= 0; i--)
            pthread_mutex_unlock(&chans[order[i]]->mu);

        if (wait_rc == ETIMEDOUT) {
            result = -1;
            break;
        }
    }

    pthread_mutex_destroy(&sel_mu);
    pthread_cond_destroy(&sel_cond);
    return result;
}

/* ── TaskGroup<T> — structured concurrency ───────────────────────────────────
   TaskGroup owns a dynamic array of spawned tasks. awaitAll() blocks until
   every task completes and returns an Array<T> of results.
   cancel() sets the cancel flag — tasks check haki_taskgroup_canceled(g).
   Dynamic realloc growth: starts at 8, doubles when full.
   ─────────────────────────────────────────────────────────────────────────── */

typedef struct {
    HakiTask**      tasks;
    int64_t         count;
    int64_t         cap;
    _Atomic int8_t  canceled;
    pthread_mutex_t mu;
} HakiTaskGroup;

HakiTaskGroup* haki_taskgroup_new(void) {
    HakiTaskGroup* g = (HakiTaskGroup*)calloc(1, sizeof(HakiTaskGroup));
    if (!g) abort();
    g->cap   = 8;
    g->tasks = (HakiTask**)malloc((size_t)g->cap * sizeof(HakiTask*));
    if (!g->tasks) abort();
    atomic_store_explicit(&g->canceled, 0, memory_order_relaxed);
    pthread_mutex_init(&g->mu, NULL);
    return g;
}

void haki_taskgroup_spawn(HakiTaskGroup* g, HakiTaskFn fn, void* arg) {
    if (!g) abort();
    HakiTask* t = haki_task_spawn(fn, arg);
    pthread_mutex_lock(&g->mu);
    if (g->count >= g->cap) {
        g->cap *= 2;
        g->tasks = (HakiTask**)realloc(g->tasks, (size_t)g->cap * sizeof(HakiTask*));
        if (!g->tasks) abort();
    }
    g->tasks[g->count++] = t;
    pthread_mutex_unlock(&g->mu);
}

/* Add an already-spawned task to a TaskGroup */
void haki_taskgroup_add(HakiTaskGroup* g, HakiTask* t) {
    if (!g || !t) return;
    pthread_mutex_lock(&g->mu);
    if (g->count >= g->cap) {
        g->cap = g->cap ? g->cap * 2 : 8;
        g->tasks = (HakiTask**)realloc(g->tasks, g->cap * sizeof(HakiTask*));
    }
    g->tasks[g->count++] = t;
    pthread_mutex_unlock(&g->mu);
}

/* Returns a HakiArray* of result pointers (void*). */
HakiArray* haki_taskgroup_await_all(HakiTaskGroup* g) {
    if (!g) return haki_array_new(sizeof(void*));
    pthread_mutex_lock(&g->mu);
    int64_t n     = g->count;
    HakiTask** ts = g->tasks;
    pthread_mutex_unlock(&g->mu);

    HakiArray* results = haki_array_new(sizeof(void*));
    for (int64_t i = 0; i < n; i++) {
        void* res = haki_task_await(ts[i]);
        haki_array_append(results, &res);
    }
    return results;
}

void haki_taskgroup_cancel(HakiTaskGroup* g) {
    if (!g) return;
    atomic_store_explicit(&g->canceled, 1, memory_order_release);
}

int8_t haki_taskgroup_canceled(HakiTaskGroup* g) {
    if (!g) return 0;
    return atomic_load_explicit(&g->canceled, memory_order_acquire);
}

void haki_taskgroup_free(HakiTaskGroup* g) {
    if (!g) return;
    pthread_mutex_lock(&g->mu);
    free(g->tasks);
    pthread_mutex_unlock(&g->mu);
    pthread_mutex_destroy(&g->mu);
    free(g);
}


#include <unistd.h>

/* ── std/net — TCP/UDP socket primitives ─────────────────────────────────────
   Thin wrappers around POSIX socket API. Error handling: functions return -1
   on failure; use haki_net_last_error() for the errno message.
   ─────────────────────────────────────────────────────────────────────────── */

#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <netdb.h>
#include <fcntl.h>
#include <errno.h>

/* Last error string — set on every socket failure. */
static char haki_net_errbuf[256] = "";

static void haki_net_set_error(const char* ctx) {
    snprintf(haki_net_errbuf, sizeof(haki_net_errbuf), "%s: %s", ctx, strerror(errno));
}

const char* haki_net_last_error(void) {
    return haki_net_errbuf;
}

/* Create a TCP socket. Returns fd or -1. */
int64_t haki_tcp_socket(void) {
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd < 0) { haki_net_set_error("socket"); return -1; }
    /* SO_REUSEADDR for servers */
    int yes = 1;
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &yes, sizeof(yes));
    return (int64_t)fd;
}

/* Create a UDP socket. Returns fd or -1. */
int64_t haki_udp_socket(void) {
    int fd = socket(AF_INET, SOCK_DGRAM, 0);
    if (fd < 0) { haki_net_set_error("socket"); return -1; }
    return (int64_t)fd;
}

/* Connect to host:port. Returns 0 on success, -1 on failure. */
int64_t haki_net_connect(int64_t fd, const char* host, int64_t port) {
    struct addrinfo hints = {0}, *res = NULL;
    hints.ai_family   = AF_INET;
    hints.ai_socktype = SOCK_STREAM;
    char portbuf[16];
    snprintf(portbuf, sizeof(portbuf), "%lld", (long long)port);
    if (getaddrinfo(host, portbuf, &hints, &res) != 0 || !res) {
        haki_net_set_error("getaddrinfo");
        return -1;
    }
    int r = connect((int)fd, res->ai_addr, res->ai_addrlen);
    freeaddrinfo(res);
    if (r < 0) { haki_net_set_error("connect"); return -1; }
    return 0;
}

/* Bind to port (server). Returns 0 or -1. */
int64_t haki_net_bind(int64_t fd, int64_t port) {
    struct sockaddr_in addr = {0};
    addr.sin_family      = AF_INET;
    addr.sin_port        = htons((uint16_t)port);
    addr.sin_addr.s_addr = INADDR_ANY;
    if (bind((int)fd, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        haki_net_set_error("bind"); return -1;
    }
    return 0;
}

/* Listen for connections. backlog=128. Returns 0 or -1. */
int64_t haki_net_listen(int64_t fd) {
    if (listen((int)fd, 128) < 0) { haki_net_set_error("listen"); return -1; }
    return 0;
}

/* Accept a connection. Returns new fd or -1. */
int64_t haki_net_accept(int64_t fd) {
    struct sockaddr_in peer = {0};
    socklen_t peerlen = sizeof(peer);
    int client = accept((int)fd, (struct sockaddr*)&peer, &peerlen);
    if (client < 0) { haki_net_set_error("accept"); return -1; }
    return (int64_t)client;
}

/* Send data. Returns bytes sent or -1. */
int64_t haki_net_send(int64_t fd, const char* data, int64_t len) {
    ssize_t r = send((int)fd, data, (size_t)len, 0);
    if (r < 0) { haki_net_set_error("send"); return -1; }
    return (int64_t)r;
}

/* Receive data into a heap-allocated string (null-terminated). Returns "" on EOF. */
const char* haki_net_recv(int64_t fd, int64_t max_bytes) {
    char* buf = (char*)malloc((size_t)(max_bytes + 1));
    if (!buf) abort();
    ssize_t r = recv((int)fd, buf, (size_t)max_bytes, 0);
    if (r <= 0) { free(buf); return ""; }
    buf[r] = '\0';
    return buf;
}

/* Close a socket. */
void haki_net_close(int64_t fd) {
    close((int)fd);
}

/* Set socket to non-blocking mode. Returns 0 or -1. */
int64_t haki_net_set_nonblocking(int64_t fd) {
    int flags = fcntl((int)fd, F_GETFL, 0);
    if (flags < 0) { haki_net_set_error("fcntl"); return -1; }
    if (fcntl((int)fd, F_SETFL, flags | O_NONBLOCK) < 0) {
        haki_net_set_error("fcntl"); return -1;
    }
    return 0;
}

/* Get peer IP address as a string. */
const char* haki_net_peer_addr(int64_t fd) {
    struct sockaddr_in peer = {0};
    socklen_t peerlen = sizeof(peer);
    if (getpeername((int)fd, (struct sockaddr*)&peer, &peerlen) < 0) return "";
    char* buf = (char*)malloc(INET_ADDRSTRLEN);
    if (!buf) return "";
    inet_ntop(AF_INET, &peer.sin_addr, buf, INET_ADDRSTRLEN);
    return buf;
}

/* ── std/crypto — SHA-256, Base64, HMAC-SHA256 ────────────────────────────────
   Self-contained implementation — no OpenSSL or libssl dependency.
   ─────────────────────────────────────────────────────────────────────────── */

/* ── SHA-256 ────────────────────────────────────────────────────────────────── */

static const uint32_t SHA256_K[64] = {
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
    0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
    0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
    0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
    0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2,
};

#define SHA256_ROR32(x,n) (((x)>>(n))|((x)<<(32-(n))))
#define SHA256_CH(x,y,z)  (((x)&(y))^(~(x)&(z)))
#define SHA256_MAJ(x,y,z) (((x)&(y))^((x)&(z))^((y)&(z)))
#define SHA256_S0(x)      (SHA256_ROR32(x,2)^SHA256_ROR32(x,13)^SHA256_ROR32(x,22))
#define SHA256_S1(x)      (SHA256_ROR32(x,6)^SHA256_ROR32(x,11)^SHA256_ROR32(x,25))
#define SHA256_R0(x)      (SHA256_ROR32(x,7)^SHA256_ROR32(x,18)^((x)>>3))
#define SHA256_R1(x)      (SHA256_ROR32(x,17)^SHA256_ROR32(x,19)^((x)>>10))

typedef struct {
    uint64_t  total;
    uint32_t  state[8];
    uint8_t   buf[64];
    uint32_t  buflen;
} HakiSHA256;

static void haki_sha256_init(HakiSHA256* ctx) {
    ctx->total = ctx->buflen = 0;
    ctx->state[0]=0x6a09e667; ctx->state[1]=0xbb67ae85;
    ctx->state[2]=0x3c6ef372; ctx->state[3]=0xa54ff53a;
    ctx->state[4]=0x510e527f; ctx->state[5]=0x9b05688c;
    ctx->state[6]=0x1f83d9ab; ctx->state[7]=0x5be0cd19;
}

static void haki_sha256_block(HakiSHA256* ctx, const uint8_t* data) {
    uint32_t W[64], a,b,c,d,e,f,g,h,T1,T2;
    int i;
    for (i=0;i<16;i++) W[i]=((uint32_t)data[i*4]<<24)|((uint32_t)data[i*4+1]<<16)|
                             ((uint32_t)data[i*4+2]<<8)|(uint32_t)data[i*4+3];
    for (i=16;i<64;i++) W[i]=SHA256_R1(W[i-2])+W[i-7]+SHA256_R0(W[i-15])+W[i-16];
    a=ctx->state[0]; b=ctx->state[1]; c=ctx->state[2]; d=ctx->state[3];
    e=ctx->state[4]; f=ctx->state[5]; g=ctx->state[6]; h=ctx->state[7];
    for (i=0;i<64;i++) {
        T1=h+SHA256_S1(e)+SHA256_CH(e,f,g)+SHA256_K[i]+W[i];
        T2=SHA256_S0(a)+SHA256_MAJ(a,b,c);
        h=g; g=f; f=e; e=d+T1; d=c; c=b; b=a; a=T1+T2;
    }
    ctx->state[0]+=a; ctx->state[1]+=b; ctx->state[2]+=c; ctx->state[3]+=d;
    ctx->state[4]+=e; ctx->state[5]+=f; ctx->state[6]+=g; ctx->state[7]+=h;
}

static void haki_sha256_update(HakiSHA256* ctx, const uint8_t* data, size_t len) {
    ctx->total += len;
    if (ctx->buflen + len < 64) {
        memcpy(ctx->buf + ctx->buflen, data, len);
        ctx->buflen += (uint32_t)len;
        return;
    }
    size_t gap = 64 - ctx->buflen;
    memcpy(ctx->buf + ctx->buflen, data, gap);
    haki_sha256_block(ctx, ctx->buf);
    data += gap; len -= gap; ctx->buflen = 0;
    while (len >= 64) { haki_sha256_block(ctx, data); data += 64; len -= 64; }
    memcpy(ctx->buf, data, len);
    ctx->buflen = (uint32_t)len;
}

static void haki_sha256_final(HakiSHA256* ctx, uint8_t out[32]) {
    uint64_t bits = ctx->total * 8;
    ctx->buf[ctx->buflen++] = 0x80;
    if (ctx->buflen > 56) {
        memset(ctx->buf + ctx->buflen, 0, 64 - ctx->buflen);
        haki_sha256_block(ctx, ctx->buf);
        ctx->buflen = 0;
    }
    memset(ctx->buf + ctx->buflen, 0, 56 - ctx->buflen);
    for (int i = 7; i >= 0; i--) { ctx->buf[56 + (7-i)] = (uint8_t)(bits >> (i*8)); }
    haki_sha256_block(ctx, ctx->buf);
    for (int i = 0; i < 8; i++) {
        out[i*4+0]=(uint8_t)(ctx->state[i]>>24); out[i*4+1]=(uint8_t)(ctx->state[i]>>16);
        out[i*4+2]=(uint8_t)(ctx->state[i]>>8);  out[i*4+3]=(uint8_t)(ctx->state[i]);
    }
}

/* Hash a string → 64-char lowercase hex digest. */
const char* haki_crypto_sha256(const char* input) {
    HakiSHA256 ctx;
    uint8_t digest[32];
    haki_sha256_init(&ctx);
    haki_sha256_update(&ctx, (const uint8_t*)input, strlen(input));
    haki_sha256_final(&ctx, digest);
    char* out = (char*)malloc(65);
    if (!out) abort();
    for (int i = 0; i < 32; i++) snprintf(out + i*2, 3, "%02x", digest[i]);
    out[64] = '\0';
    return out;
}

/* Hash raw bytes (length explicit). */
const char* haki_crypto_sha256_bytes(const char* data, int64_t len) {
    HakiSHA256 ctx;
    uint8_t digest[32];
    haki_sha256_init(&ctx);
    haki_sha256_update(&ctx, (const uint8_t*)data, (size_t)len);
    haki_sha256_final(&ctx, digest);
    char* out = (char*)malloc(65);
    if (!out) abort();
    for (int i = 0; i < 32; i++) snprintf(out + i*2, 3, "%02x", digest[i]);
    out[64] = '\0';
    return out;
}

/* ── HMAC-SHA256 ───────────────────────────────────────────────────────────── */

const char* haki_crypto_hmac_sha256(const char* key, const char* msg) {
    size_t klen = strlen(key), mlen = strlen(msg);
    uint8_t kbuf[64] = {0};
    HakiSHA256 ctx;
    uint8_t digest[32];

    /* Key preprocessing */
    if (klen > 64) {
        haki_sha256_init(&ctx);
        haki_sha256_update(&ctx, (const uint8_t*)key, klen);
        haki_sha256_final(&ctx, kbuf);
    } else {
        memcpy(kbuf, key, klen);
    }

    /* Inner hash: H(K XOR ipad || msg) */
    uint8_t ipad[64], opad[64];
    for (int i = 0; i < 64; i++) { ipad[i] = kbuf[i] ^ 0x36; opad[i] = kbuf[i] ^ 0x5c; }
    haki_sha256_init(&ctx);
    haki_sha256_update(&ctx, ipad, 64);
    haki_sha256_update(&ctx, (const uint8_t*)msg, mlen);
    haki_sha256_final(&ctx, digest);

    /* Outer hash: H(K XOR opad || inner) */
    haki_sha256_init(&ctx);
    haki_sha256_update(&ctx, opad, 64);
    haki_sha256_update(&ctx, digest, 32);
    haki_sha256_final(&ctx, digest);

    char* out = (char*)malloc(65);
    if (!out) abort();
    for (int i = 0; i < 32; i++) snprintf(out + i*2, 3, "%02x", digest[i]);
    out[64] = '\0';
    return out;
}

/* ── Base64 ────────────────────────────────────────────────────────────────── */

static const char B64_TABLE[] =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

const char* haki_crypto_base64_encode(const char* input) {
    size_t ilen = strlen(input);
    size_t olen = 4 * ((ilen + 2) / 3);
    char* out = (char*)malloc(olen + 1);
    if (!out) abort();
    size_t i = 0, j = 0;
    const uint8_t* in = (const uint8_t*)input;
    for (; i + 2 < ilen; i += 3) {
        out[j++] = B64_TABLE[in[i] >> 2];
        out[j++] = B64_TABLE[((in[i] & 3) << 4) | (in[i+1] >> 4)];
        out[j++] = B64_TABLE[((in[i+1] & 15) << 2) | (in[i+2] >> 6)];
        out[j++] = B64_TABLE[in[i+2] & 63];
    }
    if (i < ilen) {
        out[j++] = B64_TABLE[in[i] >> 2];
        if (i + 1 < ilen) {
            out[j++] = B64_TABLE[((in[i] & 3) << 4) | (in[i+1] >> 4)];
            out[j++] = B64_TABLE[(in[i+1] & 15) << 2];
        } else {
            out[j++] = B64_TABLE[(in[i] & 3) << 4];
            out[j++] = '=';
        }
        out[j++] = '=';
    }
    out[j] = '\0';
    return out;
}

const char* haki_crypto_base64_decode(const char* input) {
    static const int8_t DT[256] = {
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
        -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,62,-1,-1,-1,63,52,53,54,55,56,57,58,59,60,61,-1,-1,-1,-2,-1,-1,
        -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,-1,-1,-1,-1,-1,
        -1,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,-1,-1,-1,-1,-1,
    };
    size_t ilen = strlen(input);
    if (ilen % 4 != 0) return "";
    size_t olen = ilen / 4 * 3;
    if (ilen && input[ilen-1] == '=') olen--;
    if (ilen > 1 && input[ilen-2] == '=') olen--;
    char* out = (char*)malloc(olen + 1);
    if (!out) abort();
    size_t i = 0, j = 0;
    while (i < ilen) {
        int a=DT[(uint8_t)input[i]], b=DT[(uint8_t)input[i+1]],
            c=DT[(uint8_t)input[i+2]], d=DT[(uint8_t)input[i+3]];
        if (a<0||b<0) break;
        out[j++] = (char)((a<<2)|(b>>4));
        if (c!=-2) out[j++] = (char)((b<<4)|(c>>2));
        if (d!=-2) out[j++] = (char)((c<<6)|d);
        i += 4;
    }
    out[j] = '\0';
    return out;
}

/* ── URL-safe Base64 (no padding, + → -, / → _) ──────────────────────────── */

const char* haki_crypto_base64url_encode(const char* input) {
    const char* b64 = haki_crypto_base64_encode(input);
    size_t len = strlen(b64);
    char* out = (char*)malloc(len + 1);
    if (!out) abort();
    for (size_t i = 0; i < len; i++) {
        if      (b64[i] == '+') out[i] = '-';
        else if (b64[i] == '/') out[i] = '_';
        else if (b64[i] == '=') { out[i] = '\0'; break; }
        else                     out[i] = b64[i];
    }
    out[len] = '\0';
    return out;
}

/* ── String methods ──────────────────────────────────────────────
   All string operations work on null-terminated C strings (const char*).
   Functions that return new strings heap-allocate via malloc.
   The caller owns the result.                                     */

#include <ctype.h>

/* s.split(sep) → Array<string> (HakiArray* of char* elements) */
HakiArray* haki_string_split(const char* s, const char* sep) {
    HakiArray* arr = haki_array_new(sizeof(char*));
    if (!s || !sep || sep[0] == '\0') {
        char* dup = strdup(s ? s : "");
        haki_array_append(arr, &dup);
        return arr;
    }
    size_t sep_len = strlen(sep);
    const char* cur = s;
    while (1) {
        const char* found = strstr(cur, sep);
        size_t piece_len = found ? (size_t)(found - cur) : strlen(cur);
        char* piece = (char*)malloc(piece_len + 1);
        memcpy(piece, cur, piece_len);
        piece[piece_len] = '\0';
        haki_array_append(arr, &piece);
        if (!found) break;
        cur = found + sep_len;
    }
    return arr;
}

char* haki_string_trim(const char* s) {
    if (!s) return strdup("");
    while (isspace((unsigned char)*s)) s++;
    size_t len = strlen(s);
    while (len > 0 && isspace((unsigned char)s[len-1])) len--;
    char* r = (char*)malloc(len + 1);
    memcpy(r, s, len);
    r[len] = '\0';
    return r;
}

char* haki_string_trim_start(const char* s) {
    if (!s) return strdup("");
    while (isspace((unsigned char)*s)) s++;
    return strdup(s);
}

char* haki_string_trim_end(const char* s) {
    if (!s) return strdup("");
    size_t len = strlen(s);
    while (len > 0 && isspace((unsigned char)s[len-1])) len--;
    char* r = (char*)malloc(len + 1);
    memcpy(r, s, len);
    r[len] = '\0';
    return r;
}

char* haki_string_to_upper(const char* s) {
    if (!s) return strdup("");
    char* r = strdup(s);
    for (char* p = r; *p; p++) *p = (char)toupper((unsigned char)*p);
    return r;
}

char* haki_string_to_lower(const char* s) {
    if (!s) return strdup("");
    char* r = strdup(s);
    for (char* p = r; *p; p++) *p = (char)tolower((unsigned char)*p);
    return r;
}

int8_t haki_string_contains(const char* s, const char* sub) {
    if (!s || !sub) return 0;
    return strstr(s, sub) != NULL ? 1 : 0;
}

int8_t haki_string_starts_with(const char* s, const char* prefix) {
    if (!s || !prefix) return 0;
    return strncmp(s, prefix, strlen(prefix)) == 0 ? 1 : 0;
}

int8_t haki_string_ends_with(const char* s, const char* suffix) {
    if (!s || !suffix) return 0;
    size_t sl = strlen(s), fl = strlen(suffix);
    return (sl >= fl && strcmp(s + sl - fl, suffix) == 0) ? 1 : 0;
}

char* haki_string_replace(const char* s, const char* from, const char* to) {
    if (!s || !from || from[0] == '\0') return strdup(s ? s : "");
    size_t from_len = strlen(from), to_len = strlen(to ? to : "");
    /* Count occurrences */
    size_t count = 0;
    const char* cur = s;
    while ((cur = strstr(cur, from))) { count++; cur += from_len; }
    /* Allocate result */
    size_t orig_len = strlen(s);
    size_t new_len  = orig_len + count * (to_len - from_len + (to_len >= from_len ? 0 : 0));
    char* result = (char*)malloc(new_len + 1);
    char* dst = result;
    cur = s;
    while (*cur) {
        if (strncmp(cur, from, from_len) == 0) {
            if (to) { memcpy(dst, to, to_len); dst += to_len; }
            cur += from_len;
        } else {
            *dst++ = *cur++;
        }
    }
    *dst = '\0';
    return result;
}

int64_t haki_string_index_of(const char* s, const char* sub) {
    if (!s || !sub) return -1;
    const char* found = strstr(s, sub);
    return found ? (int64_t)(found - s) : -1;
}

char* haki_string_substring(const char* s, int64_t start, int64_t end) {
    if (!s) return strdup("");
    int64_t len = (int64_t)strlen(s);
    if (start < 0) start = 0;
    if (end > len) end = len;
    if (start >= end) return strdup("");
    int64_t sub_len = end - start;
    char* r = (char*)malloc((size_t)sub_len + 1);
    memcpy(r, s + start, (size_t)sub_len);
    r[sub_len] = '\0';
    return r;
}
#ifndef HAKI_STRING_EXTRAS_DEFINED
#define HAKI_STRING_EXTRAS_DEFINED
int8_t haki_string_is_empty(const char* s) {
    return (!s || s[0] == '\0') ? 1 : 0;
}
char* haki_string_char_at(const char* s, int64_t idx) {
    if (!s) return strdup("");
    int64_t len = (int64_t)strlen(s);
    if (idx < 0 || idx >= len) return strdup("");
    char* r = (char*)malloc(2);
    r[0] = s[idx]; r[1] = '\0';
    return r;
}
int64_t haki_string_char_code_at(const char* s, int64_t idx) {
    if (!s) return -1;
    int64_t len = (int64_t)strlen(s);
    if (idx < 0 || idx >= len) return -1;
    return (int64_t)(unsigned char)s[idx];
}
#endif /* HAKI_STRING_EXTRAS_DEFINED */


/* ── File I/O ────────────────────────────────────────────────────
   Simple blocking file operations for scripting use.             */

void haki_file_read(const char* path, char** out_content, char** out_error) {
    *out_content = NULL;
    *out_error   = NULL;
    FILE* f = fopen(path, "rb");
    if (!f) { *out_error = strdup(strerror(errno)); return; }
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    rewind(f);
    if (size < 0) { fclose(f); *out_error = strdup("seek failed"); return; }
    char* buf = (char*)malloc((size_t)size + 1);
    if (!buf) { fclose(f); *out_error = strdup("out of memory"); return; }
    size_t n = fread(buf, 1, (size_t)size, f);
    fclose(f);
    buf[n] = '\0';
    *out_content = buf;
}

/* haki_read_file — single-return wrapper matching C emitter's calling convention.
 * Returns a heap-allocated void* pair { content_ptr, error_ptr } matching
 * readFile(path) -> (string, Error?) semantics.
 * Uses raw void* fields to avoid forward-declaration dependency on HakiError/__Tuple2. */
void* haki_read_file(const char* path) {
    char* content = NULL;
    char* err_msg = NULL;
    haki_file_read(path, &content, &err_msg);
    /* Allocate a simple two-pointer tuple: [content, error] */
    void** t = (void**)malloc(2 * sizeof(void*));
    t[0] = (void*)(content ? content : (char*)"");
    if (err_msg) {
        /* Allocate a minimal error object: [arc_count, message] */
        void** e = (void**)malloc(2 * sizeof(void*));
        int64_t* arc = (int64_t*)malloc(sizeof(int64_t));
        *arc = 1;
        e[0] = (void*)arc;
        e[1] = (void*)err_msg;
        t[1] = (void*)e;
    } else {
        t[1] = NULL;
    }
    return (void*)t;
}

/* haki_write_file — matches C emitter's calling convention for writeFile(). */
void* haki_write_file(const char* path, const char* content) {
    /* Forward declaration needed — haki_file_write is defined below */
    char* haki_file_write(const char*, const char*);
    char* err = haki_file_write(path, content);
    if (!err) return NULL;
    /* Return error as a simple two-pointer [arc, message] object */
    void** e = (void**)malloc(2 * sizeof(void*));
    int64_t* arc = (int64_t*)malloc(sizeof(int64_t));
    *arc = 1;
    e[0] = (void*)arc;
    e[1] = (void*)err;
    return (void*)e;
}

#include <sys/stat.h>
#include <dirent.h>
#include <unistd.h>
/* ── String parsing ─────────────────────────────────────────────────────────*/
/* Uses a self-contained 2-field tuple (doesn't depend on cemit-generated __Tuple2) */
typedef struct { void* _r0; void* _r1; } __ParseResult;
static void* haki_string_to_int(const char* s) {
    __ParseResult* t = (__ParseResult*)malloc(sizeof(__ParseResult));
    if (!s || !*s) { t->_r0=(void*)0; t->_r1=(void*)"empty string"; return t; }
    char* end; long long v = strtoll(s, &end, 10);
    if (*end != '\0' && *end != '\n') { t->_r0=(void*)0; t->_r1=(void*)"not a number"; }
    else { t->_r0=(void*)(intptr_t)v; t->_r1=NULL; }
    return (void*)t;
}
static void* haki_string_to_float(const char* s) {
    __ParseResult* t = (__ParseResult*)malloc(sizeof(__ParseResult));
    if (!s || !*s) { t->_r0=(void*)0; t->_r1=(void*)"empty string"; return t; }
    char* end; double v = strtod(s, &end);
    if (*end != '\0' && *end != '\n') { t->_r0=(void*)0; t->_r1=(void*)"not a number"; }
    else { union { double d; void* p; } u; u.d=v; t->_r0=u.p; t->_r1=NULL; }
    return (void*)t;
}
/* ── Terminal I/O ────────────────────────────────────────────────────────────*/
const char* haki_read_line(void) {
    char buf[4096];
    if (!fgets(buf, sizeof(buf), stdin)) return strdup("");
    size_t len=strlen(buf);
    if (len>0 && buf[len-1]=='\n') buf[len-1]='\0';
    return strdup(buf);
}
void haki_print_no_newline(const char* s) { fputs(s,stdout); fflush(stdout); }
int64_t haki_read_char(void) { int c=getchar(); return c==EOF?-1:(int64_t)c; }


/* ── std/math — float wrappers ──────────────────────────────────────────── */
#include <math.h>
double haki_math_sqrt(double x) { return sqrt(x); }
double haki_math_floor(double x) { return floor(x); }
double haki_math_ceil(double x) { return ceil(x); }
double haki_math_pow_f(double base, double exp) { return pow(base, exp); }
double haki_math_log(double x) { return log(x); }
double haki_math_sin(double x) { return sin(x); }
double haki_math_cos(double x) { return cos(x); }
double haki_math_abs_f(double x) { return fabs(x); }
int64_t haki_math_floor_to_int(double x) { return (int64_t)floor(x); }
int64_t haki_math_ceil_to_int(double x) { return (int64_t)ceil(x); }
int64_t haki_math_round_to_int(double x) { return (int64_t)round(x); }

/* haki_fs_* wrappers — match extern declarations in std/fs.haki */
const char* haki_fs_read_file(const char* p) {
    char* content = NULL; char* err = NULL;
    haki_file_read(p, &content, &err);
    if (err) { free(err); }
    return content ? content : strdup("");
}
int64_t haki_fs_write_file(const char* p, const char* c) {
    FILE* f=fopen(p,"wb"); if(!f) return -1; fwrite(c,1,strlen(c),f); fclose(f); return 0;
}
int64_t haki_fs_append_file(const char* p, const char* c) {
    FILE* f=fopen(p,"ab"); if(!f) return -1; fwrite(c,1,strlen(c),f); fclose(f); return 0;
}
int64_t haki_fs_copy_file(const char* s, const char* d) {
    FILE* sf=fopen(s,"rb"); if(!sf) return -1; FILE* df=fopen(d,"wb"); if(!df){fclose(sf);return -1;}
    char buf[4096]; size_t n; while((n=fread(buf,1,4096,sf))>0) fwrite(buf,1,n,df);
    fclose(sf); fclose(df); return 0;
}
int64_t haki_fs_delete_file(const char* p) { return unlink(p)<0?-1:0; }
int64_t haki_fs_exists(const char* p) { struct stat s; return stat(p,&s)==0?1:0; }
int64_t haki_fs_is_dir(const char* p) { struct stat s; return (stat(p,&s)==0&&S_ISDIR(s.st_mode))?1:0; }
int64_t haki_fs_is_file(const char* p) { struct stat s; return (stat(p,&s)==0&&S_ISREG(s.st_mode))?1:0; }
int64_t haki_fs_size(const char* p) { struct stat s; return stat(p,&s)==0?(int64_t)s.st_size:-1; }
int64_t haki_fs_mkdir(const char* p) { return mkdir(p,0755)<0?-1:0; }
int64_t haki_fs_mkdir_all(const char* p) { (void)p; return 0; }
int64_t haki_fs_rmdir(const char* p) { return rmdir(p)<0?-1:0; }
const char* haki_fs_ext(const char* p) { const char* d=strrchr(p,'.'); return d?d:""; }
const char* haki_fs_basename(const char* p) { const char* s=strrchr(p,'/'); return s?s+1:p; }
const char* haki_fs_dirname(const char* p) {
    static char buf[4096]; snprintf(buf,4096,"%s",p);
    char* s=strrchr(buf,'/'); if(s){*s=0;return buf;} return ".";
}
int64_t haki_fs_readdir_count(const char* p) {
    DIR* d=opendir(p); if(!d) return -1; int64_t n=0; struct dirent* e;
    while((e=readdir(d))!=NULL) if(e->d_name[0]!='.') n++; closedir(d); return n;
}
static const char* __readdir_get(const char* path, int64_t i, int mode) {
    DIR* d=opendir(path); if(!d) return ""; struct dirent* e; int64_t j=0;
    while((e=readdir(d))!=NULL) { if(e->d_name[0]=='.') continue; if(j++==i) {
        static char buf[4096];
        if(mode==0) { snprintf(buf,4096,"%s",e->d_name); }
        else { snprintf(buf,4096,"%s/%s",path,e->d_name); }
        closedir(d); return buf;
    }}
    closedir(d); return "";
}
const char* haki_fs_readdir_name(const char* p, int64_t i) { return __readdir_get(p,i,0); }
const char* haki_fs_readdir_path(const char* p, int64_t i) { return __readdir_get(p,i,1); }
int64_t haki_fs_readdir_isdir(const char* p, int64_t i) {
    const char* fp=haki_fs_readdir_path(p,i); struct stat s;
    return (stat(fp,&s)==0&&S_ISDIR(s.st_mode))?1:0;
}
int64_t haki_fs_readdir_size(const char* p, int64_t i) {
    const char* fp=haki_fs_readdir_path(p,i); struct stat s;
    return stat(fp,&s)==0?(int64_t)s.st_size:0;
}

char* haki_file_write(const char* path, const char* content) {
    FILE* f = fopen(path, "wb");
    if (!f) return strdup(strerror(errno));
    size_t len = content ? strlen(content) : 0;
    size_t n = fwrite(content, 1, len, f);
    fclose(f);
    if (n != len) return strdup("write failed");
    return NULL;
}

int8_t haki_file_exists(const char* path) {
    FILE* f = fopen(path, "rb");
    if (f) { fclose(f); return 1; }
    return 0;
}

/* ── Command-line arguments ──────────────────────────────────────────────── */

static int   _haki_argc = 0;
static char** _haki_argv = NULL;

void haki_runtime_init(int argc, char** argv) {
    _haki_argc = argc;
    _haki_argv = argv;
}

/* argv() -> Array<string>: returns program arguments as a Haki array. */
void* haki_argv(void) {
    void* arr = haki_array_new(sizeof(void*));
    for (int i = 0; i < _haki_argc; i++) {
        char* s = strdup(_haki_argv[i]);
        haki_array_append(arr, &s);  /* pass pointer-to-pointer; append copies sizeof(void*) bytes */
    }
    return arr;
}

/* ── std/env, std/time, std/process, std/regex ───────────────────────────── */
/* Forward declaration — haki_error_new is defined in the Error section below */
void* haki_error_new(const char* message);

#ifndef _WIN32
#include <unistd.h>   /* getcwd, chdir, setenv, unsetenv */
#include <errno.h>
#endif

/* ── std/env ─────────────────────────────────────────────────────────────── */

void* haki_env_get(const char* name) {
    const char* val = getenv(name);
    if (!val) {
        void* err = haki_error_new("environment variable not set");
        void** result = (void**)malloc(2 * sizeof(void*));
        result[0] = (void*)strdup("");
        result[1] = err;
        return (void*)result;
    }
    void** result = (void**)malloc(2 * sizeof(void*));
    result[0] = (void*)strdup(val);
    result[1] = NULL;
    return (void*)result;
}

void* haki_env_set(const char* name, const char* value) {
#ifdef _WIN32
    if (_putenv_s(name, value) != 0)
        return haki_error_new("failed to set environment variable");
#else
    if (setenv(name, value, 1) != 0)
        return haki_error_new(strerror(errno));
#endif
    return NULL;
}

void* haki_env_unset(const char* name) {
#ifdef _WIN32
    _putenv_s(name, "");
#else
    unsetenv(name);
#endif
    return NULL;
}

void* haki_env_cwd(void) {
    char buf[4096];
#ifdef _WIN32
    if (!_getcwd(buf, sizeof(buf))) {
#else
    if (!getcwd(buf, sizeof(buf))) {
#endif
        void** r = (void**)malloc(2 * sizeof(void*));
        r[0] = (void*)strdup("");
        r[1] = haki_error_new(strerror(errno));
        return (void*)r;
    }
    void** r = (void**)malloc(2 * sizeof(void*));
    r[0] = (void*)strdup(buf);
    r[1] = NULL;
    return (void*)r;
}

void* haki_env_chdir(const char* path) {
#ifdef _WIN32
    if (_chdir(path) != 0)
#else
    if (chdir(path) != 0)
#endif
        return haki_error_new(strerror(errno));
    return NULL;
}

/* ── std/time ────────────────────────────────────────────────────────────── */

#include <time.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <sys/time.h>
#endif

int64_t haki_time_now_ms(void) {
#ifdef _WIN32
    FILETIME ft;
    GetSystemTimeAsFileTime(&ft);
    ULONGLONG t = ((ULONGLONG)ft.dwHighDateTime << 32) | ft.dwLowDateTime;
    return (int64_t)((t - 116444736000000000ULL) / 10000);
#else
    struct timeval tv;
    gettimeofday(&tv, NULL);
    return (int64_t)tv.tv_sec * 1000LL + tv.tv_usec / 1000LL;
#endif
}

void haki_time_sleep_ms(int64_t ms) {
    if (ms <= 0) return;
#ifdef _WIN32
    Sleep((DWORD)ms);
#else
    struct timespec ts;
    ts.tv_sec  = ms / 1000;
    ts.tv_nsec = (ms % 1000) * 1000000L;
    nanosleep(&ts, NULL);
#endif
}

const char* haki_time_format(int64_t unix_sec) {
    time_t t = (time_t)unix_sec;
    struct tm* tm_info = gmtime(&t);
    char* buf = (char*)malloc(32);
    strftime(buf, 32, "%Y-%m-%dT%H:%M:%SZ", tm_info);
    return buf;
}

/* ── std/process ─────────────────────────────────────────────────────────── */

#include <stdio.h>

/* run: capture stdout, return as string */
void* haki_process_run(const char* cmd, void* args_arr) {
    /* Build command string from cmd + args array */
    char buf[4096];
    int64_t len = haki_array_length(args_arr);
    snprintf(buf, sizeof(buf), "%s", cmd);
    for (int64_t i = 0; i < len; i++) {
        const char* arg = *(const char**)haki_array_get(args_arr, i);
        strncat(buf, " ", sizeof(buf) - strlen(buf) - 1);
        strncat(buf, arg, sizeof(buf) - strlen(buf) - 1);
    }
#ifdef _WIN32
    FILE* fp = _popen(buf, "r");
#else
    FILE* fp = popen(buf, "r");
#endif
    if (!fp) {
        void** r = (void**)malloc(2 * sizeof(void*));
        r[0] = (void*)strdup("");
        r[1] = haki_error_new("failed to run command");
        return (void*)r;
    }
    /* Read stdout */
    char out[65536]; out[0] = '\0';
    char line[1024];
    while (fgets(line, sizeof(line), fp)) {
        strncat(out, line, sizeof(out) - strlen(out) - 1);
    }
    int status;
#ifdef _WIN32
    status = _pclose(fp);
#else
    status = pclose(fp);
#endif
    void** r = (void**)malloc(2 * sizeof(void*));
    if (status != 0) {
        r[0] = (void*)strdup(out);
        r[1] = haki_error_new("command exited with non-zero status");
    } else {
        r[0] = (void*)strdup(out);
        r[1] = NULL;
    }
    return (void*)r;
}

/* exec: inherit stdio, return exit code */
void* haki_process_exec(const char* cmd, void* args_arr) {
    char buf[4096];
    int64_t len = haki_array_length(args_arr);
    snprintf(buf, sizeof(buf), "%s", cmd);
    for (int64_t i = 0; i < len; i++) {
        const char* arg = *(const char**)haki_array_get(args_arr, i);
        strncat(buf, " ", sizeof(buf) - strlen(buf) - 1);
        strncat(buf, arg, sizeof(buf) - strlen(buf) - 1);
    }
    int code = system(buf);
    void** r = (void**)malloc(2 * sizeof(void*));
    int64_t* code_ptr = (int64_t*)malloc(sizeof(int64_t));
    *code_ptr = (int64_t)code;
    r[0] = (void*)code_ptr;
    r[1] = NULL;
    return (void*)r;
}

/* shell: run via /bin/sh -c, capture output */
void* haki_process_shell(const char* cmd) {
#ifdef _WIN32
    FILE* fp = _popen(cmd, "r");
#else
    FILE* fp = popen(cmd, "r");
#endif
    if (!fp) {
        void** r = (void**)malloc(2 * sizeof(void*));
        r[0] = (void*)strdup("");
        r[1] = haki_error_new("failed to run shell command");
        return (void*)r;
    }
    char out[65536]; out[0] = '\0';
    char line[1024];
    while (fgets(line, sizeof(line), fp)) {
        strncat(out, line, sizeof(out) - strlen(out) - 1);
    }
#ifdef _WIN32
    _pclose(fp);
#else
    pclose(fp);
#endif
    void** r = (void**)malloc(2 * sizeof(void*));
    r[0] = (void*)strdup(out);
    r[1] = NULL;
    return (void*)r;
}

void haki_process_exit(int64_t code) {
    exit((int)code);
}

/* ── std/regex ───────────────────────────────────────────────────────────── */

#include <regex.h>

int8_t haki_regex_matches(const char* s, const char* pattern) {
    regex_t re;
    if (regcomp(&re, pattern, REG_EXTENDED | REG_NOSUB) != 0) return 0;
    int match = regexec(&re, s, 0, NULL, 0);
    regfree(&re);
    return match == 0 ? 1 : 0;
}

void* haki_regex_find(const char* s, const char* pattern) {
    regex_t re;
    void** r = (void**)malloc(2 * sizeof(void*));
    if (regcomp(&re, pattern, REG_EXTENDED) != 0) {
        r[0] = (void*)strdup("");
        r[1] = haki_error_new("invalid regex pattern");
        return (void*)r;
    }
    regmatch_t match;
    if (regexec(&re, s, 1, &match, 0) == 0) {
        int len = match.rm_eo - match.rm_so;
        char* found = (char*)malloc(len + 1);
        strncpy(found, s + match.rm_so, len);
        found[len] = '\0';
        r[0] = (void*)found;
        r[1] = NULL;
    } else {
        r[0] = (void*)strdup("");
        r[1] = haki_error_new("no match found");
    }
    regfree(&re);
    return (void*)r;
}

const char* haki_regex_replace_all(const char* s, const char* pattern, const char* replacement) {
    regex_t re;
    if (regcomp(&re, pattern, REG_EXTENDED) != 0) return strdup(s);
    /* Allocate generous output buffer */
    size_t outsize = strlen(s) * 4 + 1024;
    char* out = (char*)malloc(outsize);
    out[0] = '\0';
    const char* cur = s;
    regmatch_t match;
    while (*cur && regexec(&re, cur, 1, &match, 0) == 0) {
        /* Append text before match */
        strncat(out, cur, match.rm_so);
        /* Append replacement */
        strncat(out, replacement, outsize - strlen(out) - 1);
        cur += match.rm_eo;
        if (match.rm_eo == 0) { /* zero-width match guard */
            if (*cur) { char c[2] = {*cur, '\0'}; strncat(out, c, 1); cur++; }
            else break;
        }
    }
    strncat(out, cur, outsize - strlen(out) - 1);
    regfree(&re);
    return out;
}

void* haki_regex_split(const char* s, const char* pattern) {
    regex_t re;
    void* arr = haki_array_new(sizeof(void*));
    if (regcomp(&re, pattern, REG_EXTENDED) != 0) {
        char* copy = strdup(s);
        haki_array_append(arr, &copy);
        return arr;
    }
    const char* cur = s;
    regmatch_t match;
    while (*cur && regexec(&re, cur, 1, &match, 0) == 0) {
        if (match.rm_so > 0) {
            char* part = (char*)malloc(match.rm_so + 1);
            strncpy(part, cur, match.rm_so);
            part[match.rm_so] = '\0';
            haki_array_append(arr, &part);
        }
        cur += match.rm_eo;
        if (match.rm_eo == 0) { if (*cur) cur++; else break; }
    }
    if (*cur) {
        char* rest = strdup(cur);
        haki_array_append(arr, &rest);
    }
    regfree(&re);
    return arr;
}


/* haki_regex_find_groups: return Array<string> of capture groups (groups[0] = first capture).
   Returns empty array if no match or invalid pattern.
   Uses POSIX ERE — patterns should use [0-9] not \d, [a-z] not \w, etc. */
void* haki_regex_find_groups(const char* s, const char* pattern) {
    void* farr = haki_array_new(sizeof(void*));
    regex_t fre;
    /* count capture groups (unescaped '(') */
    int fngroups = 0;
    for (const char* fp = pattern; *fp; fp++) {
        if (*fp == '\\') { fp++; continue; }
        if (*fp == '(')  fngroups++;
    }
    if (fngroups == 0) return farr;  /* no capture groups */
    if (fngroups > 32) fngroups = 32;
    /* need fngroups+1 slots: slot 0 = full match, slots 1..fngroups = captures */
    int fntotal = fngroups + 1;
    if (regcomp(&fre, pattern, REG_EXTENDED) != 0) return farr;
    regmatch_t fgrp[33];
    if (regexec(&fre, s, (size_t)fntotal, fgrp, 0) == 0) {
        /* skip fgrp[0] (full match), return only capture groups */
        for (int fi = 1; fi < fntotal; fi++) {
            if (fgrp[fi].rm_so < 0) {
                char* fe = strdup(""); haki_array_append(farr, &fe);
            } else {
                int fgl = fgrp[fi].rm_eo - fgrp[fi].rm_so;
                char* fp2 = (char*)malloc(fgl + 1);
                strncpy(fp2, s + fgrp[fi].rm_so, fgl); fp2[fgl] = '\0';
                haki_array_append(farr, &fp2);
            }
        }
    }
    regfree(&fre);
    return farr;
}

/* ── std/time extensions ─────────────────────────────────────────────────── */

int64_t haki_time_parse(const char* s) {
    struct tm ttm; memset(&ttm, 0, sizeof(ttm));
    int tn = sscanf(s, "%d-%d-%dT%d:%d:%d",
        &ttm.tm_year, &ttm.tm_mon, &ttm.tm_mday,
        &ttm.tm_hour, &ttm.tm_min, &ttm.tm_sec);
    if (tn < 3) return -1;
    ttm.tm_year -= 1900; ttm.tm_mon -= 1; ttm.tm_isdst = 0;
#ifdef _WIN32
    return (int64_t)_mkgmtime(&ttm);
#else
    return (int64_t)timegm(&ttm);
#endif
}

const char* haki_time_format_pattern(int64_t unix_sec, const char* pattern) {
    time_t tt = (time_t)unix_sec;
    struct tm* tmi = gmtime(&tt);
    char* tbuf = (char*)malloc(256);
    strftime(tbuf, 256, pattern, tmi);
    return tbuf;
}

int64_t haki_time_diff_sec(int64_t a, int64_t b) { return a - b; }

const char* haki_time_format_tz(int64_t unix_sec, int64_t offset_minutes) {
    time_t tt2 = (time_t)(unix_sec + offset_minutes * 60);
    struct tm* tmi2 = gmtime(&tt2);
    char* tbuf2 = (char*)malloc(32);
    strftime(tbuf2, 32, "%Y-%m-%dT%H:%M:%S", tmi2);
    int tabs = (int)(offset_minutes < 0 ? -offset_minutes : offset_minutes);
    char tsign = offset_minutes >= 0 ? '+' : '-';
    char ttz[8]; snprintf(ttz, sizeof(ttz), "%c%02d:%02d", tsign, tabs/60, tabs%60);
    strncat(tbuf2, ttz, 31 - strlen(tbuf2));
    return tbuf2;
}

int64_t haki_time_day_of_week(int64_t unix_sec) {
    time_t tt3 = (time_t)unix_sec;
    return (int64_t)gmtime(&tt3)->tm_wday;
}

int64_t haki_time_start_of_day(int64_t unix_sec) {
    return unix_sec - (unix_sec % 86400);
}

const char* haki_time_day_name(int64_t wday) {
    const char* tdays[] = {"Sunday","Monday","Tuesday","Wednesday","Thursday","Friday","Saturday"};
    if (wday < 0 || wday > 6) return strdup("Unknown");
    return strdup(tdays[wday]);
}

const char* haki_time_month_name(int64_t month) {
    const char* tmonths[] = {"","January","February","March","April","May","June",
                              "July","August","September","October","November","December"};
    if (month < 1 || month > 12) return strdup("Unknown");
    return strdup(tmonths[month]);
}

/* ── std/json nested parser ──────────────────────────────────────────────── */

typedef struct { const char* src; int jpos; int jlen; } JParser;
static void jskip(JParser* jp) {
    while (jp->jpos < jp->jlen && (jp->src[jp->jpos]==' '||jp->src[jp->jpos]=='\t'||
           jp->src[jp->jpos]=='\n'||jp->src[jp->jpos]=='\r')) jp->jpos++;
}
static char* jstr(JParser* jp) {
    if (jp->jpos >= jp->jlen || jp->src[jp->jpos] != '"') return strdup("");
    jp->jpos++;
    int js = jp->jpos, jol = 0;
    while (jp->jpos < jp->jlen && jp->src[jp->jpos] != '"') {
        if (jp->src[jp->jpos] == '\\') { jp->jpos++; jol++; }
        else jol++;
        jp->jpos++;
    }
    int je = jp->jpos;
    if (jp->jpos < jp->jlen) jp->jpos++;
    char* jo = (char*)malloc(jol + 1); int joi = 0;
    for (int ji = js; ji < je; ) {
        if (jp->src[ji] == '\\' && ji + 1 < je) {
            ji++;
            switch (jp->src[ji]) {
                case '"': jo[joi++]='"'; break; case '\\': jo[joi++]='\\'; break;
                case 'n': jo[joi++]='\n'; break; case 'r': jo[joi++]='\r'; break;
                case 't': jo[joi++]='\t'; break; default: jo[joi++]=jp->src[ji]; break;
            }
            ji++;
        } else jo[joi++] = jp->src[ji++];
    }
    jo[joi] = '\0'; return jo;
}
static char* jval(JParser* jp) {
    jskip(jp);
    if (jp->jpos >= jp->jlen) return strdup("null");
    int jvs = jp->jpos; char jvc = jp->src[jp->jpos];
    if (jvc == '"') {
        jp->jpos++;
        while (jp->jpos < jp->jlen && jp->src[jp->jpos] != '"') {
            if (jp->src[jp->jpos] == '\\') jp->jpos++;
            jp->jpos++;
        }
        if (jp->jpos < jp->jlen) jp->jpos++;
    } else if (jvc == '{' || jvc == '[') {
        char jcl = (jvc=='{') ? '}' : ']'; int jd = 1; jp->jpos++;
        while (jp->jpos < jp->jlen && jd > 0) {
            char jcc = jp->src[jp->jpos];
            if (jcc == '"') { jp->jpos++;
                while (jp->jpos < jp->jlen && jp->src[jp->jpos] != '"') {
                    if (jp->src[jp->jpos] == '\\') jp->jpos++; jp->jpos++; }
                if (jp->jpos < jp->jlen) jp->jpos++;
            } else if (jcc==jvc) { jd++; jp->jpos++; }
            else if (jcc==jcl) { jd--; jp->jpos++; }
            else jp->jpos++;
        }
        (void)jcl;
    } else {
        while (jp->jpos < jp->jlen) {
            char jcc2 = jp->src[jp->jpos];
            if (jcc2==','||jcc2=='}'||jcc2==']'||jcc2==' '||jcc2=='\n'||jcc2=='\r'||jcc2=='\t') break;
            jp->jpos++;
        }
    }
    int jrl = jp->jpos - jvs; char* jr = (char*)malloc(jrl+1);
    strncpy(jr, jp->src + jvs, jrl); jr[jrl]='\0'; return jr;
}
static HakiMap* jpobj(JParser* jp) {
    HakiMap* jm = haki_map_new(sizeof(void*));
    jskip(jp);
    if (jp->jpos >= jp->jlen || jp->src[jp->jpos] != '{') return jm;
    jp->jpos++;
    while (1) {
        jskip(jp);
        if (jp->jpos >= jp->jlen || jp->src[jp->jpos] == '}') { jp->jpos++; break; }
        if (jp->src[jp->jpos] == ',') { jp->jpos++; continue; }
        char* jk = jstr(jp); jskip(jp);
        if (jp->jpos < jp->jlen && jp->src[jp->jpos] == ':') jp->jpos++;
        jskip(jp);
        char* jv2 = (jp->jpos < jp->jlen && jp->src[jp->jpos] == '"') ? jstr(jp) : jval(jp);
        haki_map_set(jm, jk, (void*)jv2); free(jk);
    }
    return jm;
}
void* haki_json_parse_nested(const char* s) {
    JParser jp; jp.src=s; jp.jpos=0; jp.jlen=(int)strlen(s);
    return (void*)jpobj(&jp);
}
const char* haki_json_encode_nested(void* jm_ptr) {
    HakiMap* jm = (HakiMap*)jm_ptr;
    size_t jcap=64; char* jout=(char*)malloc(jcap); strcpy(jout,"{"); int jfirst=1;
    for (int64_t ji=0; ji<(jm?jm->capacity:0); ji++) {
        const char* jk2=haki_map_entry_key(jm,ji);
        if(!jk2) continue;
        void* jvp=haki_map_get(jm,jk2); if(!jvp) continue;
        const char* jv3=(const char*)jvp;
        size_t jn=strlen(jout)+strlen(jk2)+strlen(jv3)+16;
        if(jn>jcap){jcap=jn*2;jout=(char*)realloc(jout,jcap);}
        if(!jfirst) strncat(jout,",",jcap-strlen(jout)-1);
        jfirst=0;
        strncat(jout,"\"",jcap-strlen(jout)-1); strncat(jout,jk2,jcap-strlen(jout)-1);
        strncat(jout,"\":",jcap-strlen(jout)-1);
        char jf=jv3[0];
        int jraw=(jf=='{'||jf=='['||(jf>='0'&&jf<='9')||jf=='-'||
                  strcmp(jv3,"true")==0||strcmp(jv3,"false")==0||strcmp(jv3,"null")==0);
        if(jraw) strncat(jout,jv3,jcap-strlen(jout)-1);
        else { strncat(jout,"\"",jcap-strlen(jout)-1); strncat(jout,jv3,jcap-strlen(jout)-1); strncat(jout,"\"",jcap-strlen(jout)-1); }
    }
    strncat(jout,"}",jcap-strlen(jout)-1); return jout;
}

/* ── JSON flat API (haki_json_str/num/flag/encode_object/encode_array/decode/decode_get)
   Called by std/json stdlib. Self-contained: uses JParser/jpobj/haki_map_xx/haki_array_get
   and haki_error_new defined above. Works in both HTTP and non-HTTP builds. */
#ifndef HAKI_JSON_FLAT_DEFINED
#define HAKI_JSON_FLAT_DEFINED
#ifndef HAKI_JSON_TUPLE2_DEFINED
#define HAKI_JSON_TUPLE2_DEFINED
typedef struct { void* f0; void* f1; } HakiJsonTuple2;
#endif
const char* haki_json_str(const char* s) {
    if(!s) return strdup("null");
    size_t len=strlen(s); char* out=(char*)malloc(len*6+3); char* p=out; *p++='"';
    while(*s){
        unsigned char c=(unsigned char)*s++;
        switch(c){
            case '"':  *p++='\\'; *p++='"';  break;
            case '\\': *p++='\\'; *p++='\\'; break;
            case '\n': *p++='\\'; *p++='n';  break;
            case '\r': *p++='\\'; *p++='r';  break;
            case '\t': *p++='\\'; *p++='t';  break;
            default: if(c<0x20){p+=sprintf(p,"\\u%04x",c);}else{*p++=(char)c;} break;
        }
    }
    *p++='"'; *p='\0'; return out;
}
const char* haki_json_num(int64_t n) {
    char buf[32]; snprintf(buf,sizeof(buf),"%lld",(long long)n); return strdup(buf);
}
const char* haki_json_flag(int b) { return strdup(b?"true":"false"); }
const char* haki_json_encode_object(void* m_ptr) {
    HakiMap* m=(HakiMap*)m_ptr;
    if(!m) return strdup("{}");
    size_t cap=32,olen=0; char* out=(char*)malloc(cap); out[olen++]='{'; int jeo_first=1;
    for(int64_t i=0;i<m->capacity;i++){
        const char* k=haki_map_entry_key(m,i); if(!k) continue;
        void* vp=haki_map_get(m,k);
        const char* v=vp?(const char*)vp:"null";
        const char* ks=haki_json_str(k);
        size_t ksl=strlen(ks); size_t vl=v?strlen(v):4;
        size_t needed=olen+ksl+1+vl+3;
        while(cap<=needed){cap*=2;out=(char*)realloc(out,cap);}
        if(!jeo_first) out[olen++]=','; jeo_first=0;
        memcpy(out+olen,ks,ksl); olen+=ksl; free((void*)ks);
        out[olen++]=':';
        if(v){memcpy(out+olen,v,vl);olen+=vl;}else{memcpy(out+olen,"null",4);olen+=4;}
    }
    out[olen++]='}'; out[olen]='\0'; return out;
}
const char* haki_json_encode_array(void* arr_ptr) {
    HakiArray* arr=(HakiArray*)arr_ptr;
    if(!arr||arr->length==0) return strdup("[]");
    size_t cap=32,olen=0; char* out=(char*)malloc(cap); out[olen++]='[';
    for(int64_t i=0;i<arr->length;i++){
        void* ep=haki_array_get(arr,i);
        const char* v=ep?*(const char**)ep:"null";
        size_t vl=v?strlen(v):4;
        size_t needed=olen+vl+3;
        while(cap<=needed){cap*=2;out=(char*)realloc(out,cap);}
        if(i>0) out[olen++]=',';
        if(v){memcpy(out+olen,v,vl);olen+=vl;}else{memcpy(out+olen,"null",4);olen+=4;}
    }
    out[olen++]=']'; out[olen]='\0'; return out;
}
void* haki_json_decode(const char* s) {
    HakiMap* jd_m=NULL; const char* jd_err=NULL;
    if(!s){ jd_err="null input"; }
    else {
        JParser jp; jp.src=s; jp.jpos=0; jp.jlen=(int)strlen(s);
        jskip(&jp);
        if(jp.jpos>=jp.jlen||jp.src[jp.jpos]!='{'){ jd_err="expected JSON object"; }
        else { jd_m=jpobj(&jp); }
    }
    HakiJsonTuple2* t=(HakiJsonTuple2*)malloc(sizeof(HakiJsonTuple2));
    t->f0=jd_m; t->f1=jd_err?haki_error_new(jd_err):NULL;
    return (void*)t;
}
const char* haki_json_decode_get(const char* s,const char* key) {
    if(!s||!key) return strdup("");
    JParser jp; jp.src=s; jp.jpos=0; jp.jlen=(int)strlen(s);
    HakiMap* m=jpobj(&jp);
    if(!m) return strdup("");
    void* vp=haki_map_get(m,key);
    const char* result=vp?strdup((const char*)vp):strdup("");
    haki_map_free(m); return result;
}
#endif /* HAKI_JSON_FLAT_DEFINED */

/* ── std/csv ─────────────────────────────────────────────────────────────── */

#ifndef HAKI_CSV_DEFINED
#define HAKI_CSV_DEFINED

/* haki_csv_parse_row: parse one CSV/TSV line into HakiArray* of char*.
   sep = delimiter as int64_t (44 = comma, 9 = tab).
   NOTE: uses haki_array_append(&field) so elem_size=sizeof(char*) is correct. */
void* haki_csv_parse_row(const char* line, int64_t sep_i) {
    char sep = (char)(int)sep_i;
    HakiArray* arr = haki_array_new(sizeof(char*));
    if (!line) return (void*)arr;
    const char* p = line;
    int csv_at_end = 0;
    do {
        char* field = NULL;
        size_t flen = 0, fcap = 16;
        field = (char*)malloc(fcap);
        if (*p == '"') {
            /* Quoted field */
            p++;
            while (*p) {
                if (*p == '"') {
                    if (*(p+1) == '"') {
                        /* escaped quote: "" -> " */
                        if (flen+1 >= fcap) { fcap*=2; field=(char*)realloc(field,fcap); }
                        field[flen++] = '"'; p += 2;
                    } else { p++; break; /* closing quote */ }
                } else {
                    if (flen+1 >= fcap) { fcap*=2; field=(char*)realloc(field,fcap); }
                    field[flen++] = *p++;
                }
            }
            if (*p == sep) p++;
            else csv_at_end = 1;
        } else {
            /* Unquoted field */
            while (*p && *p != sep) {
                if (flen+1 >= fcap) { fcap*=2; field=(char*)realloc(field,fcap); }
                field[flen++] = *p++;
            }
            if (*p == sep) p++;
            else csv_at_end = 1;
        }
        field[flen] = '\0';
        haki_array_append(arr, &field); /* &field: copy char* value into array */
    } while (!csv_at_end);
    return (void*)arr;
}

/* haki_csv_encode_row: encode HakiArray* of char* into a CSV/TSV row string. */
const char* haki_csv_encode_row(void* fields_ptr, int64_t sep_i) {
    char sep = (char)(int)sep_i;
    HakiArray* arr = (HakiArray*)fields_ptr;
    if (!arr || arr->length == 0) return strdup("");
    size_t cap = 64, olen = 0;
    char* out = (char*)malloc(cap);
    for (int64_t ci = 0; ci < arr->length; ci++) {
        const char* field = *(const char**)haki_array_get(arr, ci); /* double deref */
        if (!field) field = "";
        if (ci > 0) {
            if (olen+1 >= cap) { cap*=2; out=(char*)realloc(out,cap); }
            out[olen++] = sep;
        }
        /* check if quoting needed */
        int needs_q = 0;
        const char* fp = field;
        while (*fp) {
            if (*fp == sep || *fp == '"' || *fp == '\n' || *fp == '\r') { needs_q=1; break; }
            fp++;
        }
        if (needs_q) {
            size_t flen = strlen(field);
            while (olen + flen*2 + 4 >= cap) { cap*=2; out=(char*)realloc(out,cap); }
            out[olen++] = '"';
            fp = field;
            while (*fp) {
                if (*fp == '"') out[olen++] = '"'; /* escape */
                out[olen++] = *fp++;
            }
            out[olen++] = '"';
        } else {
            size_t flen = strlen(field);
            while (olen + flen + 2 >= cap) { cap*=2; out=(char*)realloc(out,cap); }
            memcpy(out+olen, field, flen); olen += flen;
        }
    }
    out[olen] = '\0';
    return out;
}

/* haki_csv_parse: parse full CSV/TSV string into HakiArray* of HakiArray*.
   Returns HakiJsonTuple2 { f0=rows, f1=error? }. */
void* haki_csv_parse(const char* s, int64_t sep) {
    HakiArray* rows = NULL;
    const char* csv_parse_err = NULL;
    if (!s) { csv_parse_err = "null input"; }
    else {
        rows = haki_array_new(sizeof(HakiArray*));
        const char* p = s;
        size_t total = strlen(s);
        const char* csv_end = s + total;
        while (p <= csv_end) {
            const char* line_start = p;
            int in_q = 0;
            while (p < csv_end) {
                if (*p == '"') { in_q = !in_q; p++; }
                else if (!in_q && (*p == '\n' || *p == '\r')) { break; }
                else { p++; }
            }
            size_t llen = (size_t)(p - line_start);
            char* lbuf = (char*)malloc(llen + 1);
            memcpy(lbuf, line_start, llen); lbuf[llen] = '\0';
            if (p < csv_end && *p == '\r') p++;
            if (p < csv_end && *p == '\n') p++;
            if (llen == 0 && p >= csv_end) { free(lbuf); break; }
            HakiArray* row = (HakiArray*)haki_csv_parse_row(lbuf, sep);
            free(lbuf);
            haki_array_append(rows, &row); /* &row: copy HakiArray* value */
        }
    }
    HakiJsonTuple2* t = (HakiJsonTuple2*)malloc(sizeof(HakiJsonTuple2));
    t->f0 = (void*)rows;
    t->f1 = csv_parse_err ? haki_error_new(csv_parse_err) : NULL;
    return (void*)t;
}

/* haki_csv_encode: encode HakiArray* of HakiArray* into a CSV/TSV string. */
const char* haki_csv_encode(void* rows_ptr, int64_t sep) {
    HakiArray* rows = (HakiArray*)rows_ptr;
    if (!rows || rows->length == 0) return strdup("");
    size_t cap = 256, olen = 0;
    char* out = (char*)malloc(cap);
    for (int64_t ri = 0; ri < rows->length; ri++) {
        HakiArray* row = *(HakiArray**)haki_array_get(rows, ri); /* double deref */
        if (!row) continue;
        if (ri > 0) {
            if (olen+2 >= cap) { cap*=2; out=(char*)realloc(out,cap); }
            out[olen++] = '\n';
        }
        const char* row_str = haki_csv_encode_row((void*)row, sep);
        size_t rlen = strlen(row_str);
        while (olen + rlen + 2 >= cap) { cap*=2; out=(char*)realloc(out,cap); }
        memcpy(out+olen, row_str, rlen); olen += rlen;
        free((void*)row_str);
    }
    out[olen] = '\0';
    return out;
}

#endif /* HAKI_CSV_DEFINED */


/* ── std/xml ─────────────────────────────────────────────────────────────── */

const char* haki_xml_get_element(const char* xml, const char* tag) {
    char xopen[256], xclose[256];
    snprintf(xopen,sizeof(xopen),"<%s",tag); snprintf(xclose,sizeof(xclose),"</%s>",tag);
    const char* xs=strstr(xml,xopen); if(!xs) return strdup("");
    xs=strchr(xs,'>'); if(!xs) return strdup(""); xs++;
    const char* xe=strstr(xs,xclose); if(!xe) return strdup("");
    int xl=(int)(xe-xs); char* xr=(char*)malloc(xl+1);
    strncpy(xr,xs,xl); xr[xl]='\0'; return xr;
}
void* haki_xml_parse_attrs(const char* attr_str) {
    HakiMap* xm=haki_map_new(sizeof(void*));
    const char* xp=attr_str;
    while(*xp){
        while(*xp==' '||*xp=='\t'||*xp=='\n') xp++;
        if(!*xp) break;
        const char* xks=xp; while(*xp&&*xp!='='&&*xp!=' ') xp++;
        if(!*xp||*xp!='=') break;
        int xkl=(int)(xp-xks); char* xk=(char*)malloc(xkl+1);
        strncpy(xk,xks,xkl); xk[xkl]='\0'; xp++;
        char xq=0; if(*xp=='"'||*xp=='\''){xq=*xp;xp++;}
        const char* xvs=xp; while(*xp&&(xq?*xp!=xq:(*xp!=' '&&*xp!='\t'))) xp++;
        int xvl=(int)(xp-xvs); char* xv=(char*)malloc(xvl+1);
        strncpy(xv,xvs,xvl); xv[xvl]='\0'; if(xq&&*xp) xp++;
        haki_map_set(xm,xk,(void*)xv); free(xk);  /* store char* directly as void* */
    }
    return (void*)xm;
}
const char* haki_xml_get_attr(const char* tag_str, const char* attr_name) {
    size_t xal=strlen(attr_name); const char* xap=tag_str;
    while((xap=strstr(xap,attr_name))!=NULL){
        if(xap>tag_str&&(*(xap-1)==' '||*(xap-1)=='\t'||*(xap-1)=='<')){
            xap+=xal; while(*xap==' ')xap++;
            if(*xap=='='){xap++; while(*xap==' ')xap++;
                char xq2=0; if(*xap=='"'||*xap=='\''){xq2=*xap;xap++;}
                const char* xvs2=xap;
                while(*xap&&(xq2?*xap!=xq2:(*xap!=' '&&*xap!='>')))xap++;
                int xvl2=(int)(xap-xvs2); char* xv2=(char*)malloc(xvl2+1);
                strncpy(xv2,xvs2,xvl2); xv2[xvl2]='\0'; return xv2;
            }
        }
        xap++;
    }
    return strdup("");
}
const char* haki_xml_emit_element(const char* tag, const char* content) {
    size_t xel=strlen(tag)*2+strlen(content)+8; char* xeo=(char*)malloc(xel);
    snprintf(xeo,xel,"<%s>%s</%s>",tag,content,tag); return xeo;
}

const char* haki_xml_emit_tag(const char* tag, HakiMap* attrs) {
    size_t cap = 256;
    char* out = (char*)malloc(cap);
    snprintf(out, cap, "<%s", tag);
    if (attrs) {
        for (int64_t i = 0; i < attrs->capacity; i++) {
            if (attrs->entries[i].key) {
                const char* k = attrs->entries[i].key;
                void* vp = attrs->entries[i].value;
                const char* v = vp ? (const char*)vp : "";  /* Map<string,string> stores char* directly */
                size_t need = strlen(out) + strlen(k) + strlen(v) + 8;
                while (need > cap) { cap *= 2; out = (char*)realloc(out, cap); }
                size_t l = strlen(out);
                snprintf(out + l, cap - l, " %s=\"%s\"", k, v);
            }
        }
    }
    size_t l = strlen(out);
    while (l + 4 > cap) { cap *= 2; out = (char*)realloc(out, cap); }
    out[l++] = '/'; out[l++] = '>'; out[l] = '\0';
    return out;
}
const char* haki_xml_escape(const char* s) {
    size_t xslen=strlen(s); size_t xcap2=xslen*6+1; char* xout=(char*)malloc(xcap2); char* xo=xout;
    for(size_t xi=0;xi<xslen;xi++){
        switch(s[xi]){
            case '&': memcpy(xo,"&amp;",5); xo+=5; break;
            case '<': memcpy(xo,"&lt;",4); xo+=4; break;
            case '>': memcpy(xo,"&gt;",4); xo+=4; break;
            case '"': memcpy(xo,"&quot;",6); xo+=6; break;
            case '\'': memcpy(xo,"&apos;",6); xo+=6; break;
            default: *xo++=s[xi]; break;
        }
    }
    *xo='\0'; return xout;
}

/* ── std/template ────────────────────────────────────────────────────────── */

const char* haki_template_render(const char* tmpl, HakiMap* vars) {
    size_t ttlen=strlen(tmpl); size_t ttcap=ttlen*2+256;
    char* ttout=(char*)malloc(ttcap); ttout[0]='\0';
    const char* ttp=tmpl;
    while(*ttp){
        if(ttp[0]=='{'&&ttp[1]=='{'){
            ttp+=2; while(*ttp==' ')ttp++;
            const char* ttks=ttp; while(*ttp&&!(ttp[0]=='}'&&ttp[1]=='}'))ttp++;
            const char* ttke=ttp; while(ttke>ttks&&*(ttke-1)==' ')ttke--;
            int ttkl=(int)(ttke-ttks); char* ttk=(char*)malloc(ttkl+1);
            strncpy(ttk,ttks,ttkl); ttk[ttkl]='\0';
            if(ttp[0]=='}'&&ttp[1]=='}')ttp+=2;
            void* ttvp=haki_map_get(vars,ttk); const char* ttv=ttvp?(const char*)ttvp:"";
            free(ttk);
            size_t ttn=strlen(ttout)+strlen(ttv)+64;
            if(ttn>ttcap){ttcap=ttn*2;ttout=(char*)realloc(ttout,ttcap);}
            strncat(ttout,ttv,ttcap-strlen(ttout)-1);
        } else {
            size_t ttc=strlen(ttout);
            if(ttc+2>ttcap){ttcap*=2;ttout=(char*)realloc(ttout,ttcap);}
            ttout[ttc]=*ttp; ttout[ttc+1]='\0'; ttp++;
        }
    }
    return ttout;
}
const char* haki_template_html_escape(const char* s) {
    if (!s) return "";
    size_t cap = strlen(s) * 6 + 64;
    char* out = (char*)malloc(cap);
    char* p = out;
    while (*s) {
        if (*s == '&') { memcpy(p,"&amp;",5); p+=5; }
        else if (*s == '<') { memcpy(p,"&lt;",4); p+=4; }
        else if (*s == '>') { memcpy(p,"&gt;",4); p+=4; }
        else if (*s == '"') { memcpy(p,"&quot;",6); p+=6; }
        else if (*s == '\'') { memcpy(p,"&#39;",5); p+=5; }
        else { *p++ = *s; }
        s++;
    }
    *p = '\0';
    return out;
}

/* haki_template_render_full — full template engine with if/else/for blocks.
   Tags supported:
     {{var}}                          variable substitution
     {{#if var}}...{{/if}}            conditional (truthy = non-empty, not "false", not "0")
     {{#if var}}...{{#else}}...{{/if}} conditional with else branch
     {{#for item in list}}...{{/for}} iteration (list = newline-separated values)
*/

/* Copy a HakiMap, then add/override one extra key=value. Used by template for loop. */
static HakiMap* haki_map_copy_with(HakiMap* src, const char* key, const char* val) {
    HakiMap* dst = haki_map_new(sizeof(void*));
    if (src) {
        for (int64_t i = 0; i < src->capacity; i++) {
            if (src->entries[i].key) {
                haki_map_set(dst, src->entries[i].key, src->entries[i].value);
            }
        }
    }
    char* vs = strdup(val);
    haki_map_set(dst, key, (void*)vs);  /* store char* directly as void*, not &vs */
    return dst;
}
static int trf_truthy(const char* v) {
    if (!v || v[0]=='\0') return 0;
    if (strcmp(v,"false")==0||strcmp(v,"0")==0) return 0;
    return 1;
}

static char* trf_mapget(HakiMap* vars, const char* key) {
    if (!vars) return NULL;
    void* vp = haki_map_get(vars, key);
    if (!vp) return NULL;
    return (char*)vp;  /* Map<string,string> stores char* directly as void* */
}

static void trf_append(char** outp, size_t* lenp, size_t* capp, const char* s, size_t sl) {
    if (sl == 0) return;
    if (*lenp + sl + 1 > *capp) {
        while (*lenp + sl + 1 > *capp) *capp *= 2;
        *outp = (char*)realloc(*outp, *capp);
    }
    memcpy(*outp + *lenp, s, sl);
    *lenp += sl;
    (*outp)[*lenp] = '\0';
}

/* forward decl */
static const char* trf_render(const char* p, const char* end, HakiMap* vars, char** outp, size_t* lenp, size_t* capp, int skip);

/* parse {{tag_name rest}} — returns pointer past closing }}, fills tag and rest */
static const char* trf_parse_tag(const char* p, const char* end, char* tag, size_t tsz, char* rest, size_t rsz) {
    /* p points just past '{{' */
    while (p < end && *p == ' ') p++;
    const char* ts = p;
    while (p < end && *p != ' ' && !(p[0]=='}' && p[1]=='}')) p++;
    size_t tl = (size_t)(p - ts);
    if (tl >= tsz) tl = tsz - 1;
    strncpy(tag, ts, tl); tag[tl] = '\0';
    while (p < end && *p == ' ') p++;
    const char* rs = p;
    while (p < end && !(p[0]=='}' && p[1]=='}')) p++;
    size_t rl = (size_t)(p - rs);
    while (rl > 0 && rs[rl-1] == ' ') rl--;
    if (rl >= rsz) rl = rsz - 1;
    strncpy(rest, rs, rl); rest[rl] = '\0';
    if (p+1 < end) p += 2; /* skip }} */
    return p;
}

static const char* trf_render(const char* p, const char* end, HakiMap* vars, char** outp, size_t* lenp, size_t* capp, int skip) {
    char tag[256]; char rest[512];
    while (p < end) {
        if (p[0]=='{' && p+1<end && p[1]=='{') {
            const char* tp = p + 2;
            tp = trf_parse_tag(tp, end, tag, sizeof(tag), rest, sizeof(rest));
            if (tag[0]=='#') {
                /* block open */
                const char* block_name = tag + 1; /* "if" or "for" */
                if (strcmp(block_name, "if") == 0) {
                    /* find matching {{/if}}, respecting nesting */
                    /* render true branch, skip false branch (or vice versa) */
                    char* val = trf_mapget(vars, rest);
                    int cond = trf_truthy(val);
                    int depth = 1;
                    const char* branch_start = tp;
                    const char* else_p = NULL;
                    const char* close_p = NULL;
                    /* scan to find {{#else}} and {{/if}} at depth 1 */
                    const char* sp = tp;
                    while (sp < end) {
                        if (sp[0]=='{' && sp+1<end && sp[1]=='{') {
                            char st[256]; char sr[512];
                            const char* np = trf_parse_tag(sp+2, end, st, sizeof(st), sr, sizeof(sr));
                            if (st[0]=='#' && (strcmp(st+1,"if")==0||strcmp(st+1,"for")==0)) depth++;
                            else if (st[0]=='/' && strcmp(st+1,"if")==0) {
                                depth--;
                                if (depth==0) { close_p = sp; tp = np; break; }
                            } else if (strcmp(st,"\x23" "else")==0 && depth==1) {
                                else_p = sp; sp = np; continue;
                            }
                            sp = np;
                        } else sp++;
                    }
                    if (!skip) {
                        const char* true_end = else_p ? else_p : close_p;
                        const char* false_start = NULL;
                        const char* false_end = close_p;
                        if (else_p) {
                            char st[256]; char sr[512];
                            false_start = trf_parse_tag(else_p+2, end, st, sizeof(st), sr, sizeof(sr));
                        }
                        if (cond) {
                            trf_render(branch_start, true_end ? true_end : end, vars, outp, lenp, capp, 0);
                        } else if (false_start) {
                            trf_render(false_start, false_end ? false_end : end, vars, outp, lenp, capp, 0);
                        }
                    }
                    p = tp;
                } else if (strcmp(block_name, "for") == 0) {
                    /* rest = "item in list_var" */
                    char iter_var[128] = ""; char list_var[128] = "";
                    sscanf(rest, "%127s in %127s", iter_var, list_var);
                    char* list_val = trf_mapget(vars, list_var);
                    /* find {{/for}} */
                    int depth = 1;
                    const char* body_start = tp;
                    const char* close_p = NULL;
                    const char* sp = tp;
                    while (sp < end) {
                        if (sp[0]=='{' && sp+1<end && sp[1]=='{') {
                            char st[256]; char sr[512];
                            const char* np = trf_parse_tag(sp+2, end, st, sizeof(st), sr, sizeof(sr));
                            if (st[0]=='#' && (strcmp(st+1,"for")==0||strcmp(st+1,"if")==0)) depth++;
                            else if (st[0]=='/' && strcmp(st+1,"for")==0) {
                                depth--;
                                if (depth==0) { close_p = sp; tp = np; break; }
                            }
                            sp = np;
                        } else sp++;
                    }
                    if (!skip && list_val && iter_var[0]) {
                        /* iterate newline-separated items */
                        char* buf = strdup(list_val);
                        char* line = strtok(buf, "\n");
                        while (line) {
                            while (*line == '\r') line++;
                            char* le = line + strlen(line);
                            while (le > line && (*(le-1)=='\r'||*(le-1)==' ')) { le--; *le='\0'; }
                            if (*line) {
                                HakiMap* iter_vars = haki_map_copy_with(vars, iter_var, line);
                                trf_render(body_start, close_p ? close_p : end, iter_vars, outp, lenp, capp, 0);
                            }
                            line = strtok(NULL, "\n");
                        }
                        free(buf);
                    }
                    p = tp;
                } else {
                    /* unknown block — skip */
                    p = tp;
                }
            } else if (tag[0]=='/') {
                /* unmatched close tag — stop (caller handles it) */
                break;
            } else if (strcmp(tag,"\x23" "else")==0) {
                /* unmatched else — stop */
                break;
            } else {
                /* variable substitution */
                if (!skip) {
                    char* val = trf_mapget(vars, tag);
                    if (val) trf_append(outp, lenp, capp, val, strlen(val));
                }
                p = tp;
            }
        } else {
            if (!skip) trf_append(outp, lenp, capp, p, 1);
            p++;
        }
    }
    return p;
}

const char* haki_template_render_full(const char* tmpl, HakiMap* vars) {
    if (!tmpl) return "";
    size_t cap = strlen(tmpl) * 2 + 256;
    char* out = (char*)malloc(cap);
    out[0] = '\0';
    size_t len = 0;
    trf_render(tmpl, tmpl + strlen(tmpl), vars, &out, &len, &cap, 0);
    return out;
}


typedef struct {
    void*       __arc;    /* ARC header — must be first */
    const char* message;
    void*       cause;    /* Error? — null if no cause */
} HakiError;

void* haki_error_new(const char* message) {
    HakiError* e = (HakiError*)haki_alloc(sizeof(HakiError));
    e->message = message ? strdup(message) : "";
    e->cause   = NULL;
    return (void*)e;
}

void* haki_error_new_with_cause(const char* message, void* cause) {
    HakiError* e = (HakiError*)haki_alloc(sizeof(HakiError));
    e->message = message ? strdup(message) : "";
    e->cause   = cause;
    return (void*)e;
}

void* haki_error_wrap(const char* message) {
    if (!message) return NULL;
    return haki_error_new(message);
}

const char* haki_error_message(void* err) {
    if (!err) return "";
    return ((HakiError*)err)->message;
}

void* haki_error_cause(void* err) {
    if (!err) return NULL;
    return ((HakiError*)err)->cause;
}

/* ── HTTP Server (self-contained) ───────────────────────────────
   haki_http_server_new(port, handler) starts a server that calls
   handler(HttpRequest*) → HttpResponse* for each request.
   haki_http_server_listen(server) blocks the calling thread.      */

#ifndef HAKI_HTTP_TYPES_DEFINED
/* Standalone definitions used when compiled without the user program */
typedef struct {
    const char* method;
    const char* path;
    const char* body;
    size_t      body_len;
    void*       connection;
} HttpRequest;
typedef struct {
    int         status;
    const char* body;
    const char* contentType;
} HttpResponse;
#endif /* HAKI_HTTP_TYPES_DEFINED */

typedef HttpResponse* (*HttpHandler)(HttpRequest*);
typedef struct {
    int64_t           port;
    HttpHandler handler;
    void*             daemon;
} HttpServer;

/* ── Haki self-contained HTTP/1.1 server ─────────────────────────────────
   Pure POSIX sockets + pthreads. No external deps. Zero system installs.
   Works on macOS and Linux. Windows: swap in Winsock if needed.         */

#ifdef _WIN32
#  include <winsock2.h>
#  include <ws2tcpip.h>
   typedef SOCKET haki_sock_t;
#  define HAKI_INVALID_SOCK INVALID_SOCKET
#  define haki_sock_close(s) closesocket(s)
#else
#  include <sys/socket.h>
#  include <netinet/in.h>
#  include <arpa/inet.h>
#  include <unistd.h>
   typedef int haki_sock_t;
#  define HAKI_INVALID_SOCK (-1)
#  define haki_sock_close(s) close(s)
#endif

static int haki_http_recv_headers(haki_sock_t fd, char* buf, int maxlen) {
    int total = 0;
    while (total < maxlen - 1) {
        int n = (int)recv(fd, buf + total, maxlen - total - 1, 0);
        if (n <= 0) break;
        total += n;
        buf[total] = '\0';
        if (strstr(buf, "\r\n\r\n")) break;
    }
    buf[total] = '\0';
    return total;
}

static int haki_http_recv_body(haki_sock_t fd, char* buf, int len) {
    int got = 0;
    while (got < len) {
        int n = (int)recv(fd, buf + got, len - got, 0);
        if (n <= 0) break;
        got += n;
    }
    return got;
}

static int haki_http_content_len(const char* hdr) {
    const char* p = hdr;
    while (*p) {
        if (strncasecmp(p, "Content-Length:", 15) == 0) {
            p += 15;
            while (*p == ' ') p++;
            return atoi(p);
        }
        const char* nl = strchr(p, '\n');
        if (!nl) break;
        p = nl + 1;
    }
    return 0;
}

static const char* haki_http_status_text(int code) {
    switch (code) {
        case 200: return "OK";
        case 201: return "Created";
        case 204: return "No Content";
        case 301: return "Moved Permanently";
        case 302: return "Found";
        case 400: return "Bad Request";
        case 401: return "Unauthorized";
        case 403: return "Forbidden";
        case 404: return "Not Found";
        case 405: return "Method Not Allowed";
        case 500: return "Internal Server Error";
        default:  return "OK";
    }
}

static void haki_http_send_resp(haki_sock_t fd, HttpResponse* resp) {
    const char* ct   = (resp->contentType && resp->contentType[0]) ? resp->contentType : "text/plain";
    const char* body = resp->body ? resp->body : "";
    int blen = (int)strlen(body);
    char hdr[1024];
    int hlen = snprintf(hdr, sizeof(hdr),
        "HTTP/1.1 %d %s\r\nContent-Type: %s\r\nContent-Length: %d\r\nConnection: close\r\n\r\n",
        resp->status, haki_http_status_text(resp->status), ct, blen);
    send(fd, hdr, hlen, 0);
    if (blen > 0) send(fd, body, blen, 0);
}

typedef HttpResponse* (*HttpHandler)(HttpRequest*);
typedef struct { haki_sock_t fd; HttpHandler handler; } HakiConnCtx;

static void* haki_http_conn_thread(void* arg) {
    HakiConnCtx* ctx = (HakiConnCtx*)arg;
    haki_sock_t fd   = ctx->fd;
    HttpHandler h    = ctx->handler;
    free(ctx);

    char hdr_buf[8192];
    int hlen = haki_http_recv_headers(fd, hdr_buf, sizeof(hdr_buf));
    if (hlen <= 0) { haki_sock_close(fd); return NULL; }

    char method[16]={0}, path[2048]={0};
    sscanf(hdr_buf, "%15s %2047s", method, path);

    char* body_start = strstr(hdr_buf, "\r\n\r\n");
    if (!body_start) { haki_sock_close(fd); return NULL; }
    body_start += 4;

    int clen = haki_http_content_len(hdr_buf);
    char* body_buf = NULL;
    if (clen > 0) {
        body_buf = (char*)calloc(1, clen + 1);
        if (body_buf) {
            int already = (int)(hlen - (int)(body_start - hdr_buf));
            if (already > clen) already = clen;
            if (already > 0) memcpy(body_buf, body_start, already);
            if (already < clen) haki_http_recv_body(fd, body_buf + already, clen - already);
        }
    }

    HttpRequest req;
    req.method     = method;
    req.path       = path;
    req.body       = body_buf ? body_buf : "";
    req.body_len   = clen;
    req.connection = (void*)(intptr_t)fd;

    /* h is a Haki closure fat pointer: void*[2] = { fn_ptr, env_ptr }
       Unpack and call: fn_ptr(env, &req) */
    typedef HttpResponse* (*HakiHttpFn)(void*, HttpRequest*);
    HakiHttpFn haki_http_fn_ptr = (HakiHttpFn)((void**)h)[0];
    void*       haki_http_env   = ((void**)h)[1];
    HttpResponse* resp = haki_http_fn_ptr(haki_http_env, &req);
    if (resp) haki_http_send_resp(fd, resp);
    else {
        const char* e = "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n";
        send(fd, e, (int)strlen(e), 0);
    }

    if (body_buf) free(body_buf);
    haki_sock_close(fd);
    return NULL;
}

typedef struct { HttpServer* srv; } HakiAcceptArg;

static void* haki_http_accept_loop(void* arg) {
    HttpServer* s = ((HakiAcceptArg*)arg)->srv;
    free(arg);
    haki_sock_t srv_fd = (haki_sock_t)(intptr_t)s->daemon;
    fprintf(stderr, "haki: HTTP server listening on :%lld (Ctrl+C to stop)\n", (long long)s->port);
    while (1) {
        struct sockaddr_in ca;
#ifdef _WIN32
        int al = sizeof(ca);
#else
        socklen_t al = sizeof(ca);
#endif
        haki_sock_t cfd = accept(srv_fd, (struct sockaddr*)&ca, &al);
        if (cfd == HAKI_INVALID_SOCK) break;
        HakiConnCtx* ctx = (HakiConnCtx*)malloc(sizeof(HakiConnCtx));
        if (!ctx) { haki_sock_close(cfd); continue; }
        ctx->fd = cfd; ctx->handler = s->handler;
        pthread_t tid;
        pthread_create(&tid, NULL, haki_http_conn_thread, ctx);
        pthread_detach(tid);
    }
    return NULL;
}

HttpServer* haki_http_server_new(int64_t port, HttpHandler handler) {
    HttpServer* s = (HttpServer*)calloc(1, sizeof(HttpServer));
    if (!s) abort();
    s->port = port; s->handler = handler; s->daemon = NULL;
    return s;
}
/* Compat alias — Haki programs may call either name */
HttpServer* haki_http_server_new_compat(int64_t port, HttpHandler handler) {
    return haki_http_server_new(port, handler);
}

void haki_http_server_listen(HttpServer* s) {
    if (!s) return;
#ifdef _WIN32
    WSADATA wsa; WSAStartup(MAKEWORD(2,2), &wsa);
#endif
    haki_sock_t fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd == HAKI_INVALID_SOCK) { fprintf(stderr, "haki: socket() failed\n"); return; }
    int opt = 1;
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, (const char*)&opt, sizeof(opt));
    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET; addr.sin_addr.s_addr = INADDR_ANY;
    addr.sin_port = htons((uint16_t)s->port);
    if (bind(fd, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        fprintf(stderr, "haki: bind() failed on port %lld\n", (long long)s->port);
        haki_sock_close(fd); return;
    }
    if (listen(fd, 128) < 0) { fprintf(stderr, "haki: listen() failed\n"); haki_sock_close(fd); return; }
    s->daemon = (void*)(intptr_t)fd;
    HakiAcceptArg* aa = (HakiAcceptArg*)malloc(sizeof(HakiAcceptArg));
    if (!aa) { haki_sock_close(fd); return; }
    aa->srv = s;
    haki_http_accept_loop(aa); /* blocks */
}
/* Compat alias */
void haki_http_server_listen_compat(HttpServer* s) { haki_http_server_listen(s); }

void haki_http_server_stop(HttpServer* s) {
    if (!s || !s->daemon) return;
    haki_sock_close((haki_sock_t)(intptr_t)s->daemon);
    s->daemon = NULL;
}

const char* haki_http_request_path(HttpRequest* r)   { return r ? r->path   : ""; }
const char* haki_http_request_method(HttpRequest* r) { return r ? r->method : ""; }
const char* haki_http_request_body(HttpRequest* r)   { return r ? r->body   : ""; }


/* Map entry accessors: defined in CORE for user.c, also needed in standalone runtime.c */
#ifndef HAKI_MAP_ENTRY_DEFINED
#define HAKI_MAP_ENTRY_DEFINED
const char* haki_map_entry_key(HakiMap* m, int64_t i) {
    if (!m || i < 0 || i >= m->capacity) return NULL;
    return m->entries[i].key;
}
void* haki_map_entry_value(HakiMap* m, int64_t i) {
    if (!m || i < 0 || i >= m->capacity) return NULL;
    return m->entries[i].value;
}
#endif /* HAKI_MAP_ENTRY_DEFINED */

/* ── JSON ────────────────────────────────────────────────────────
   Minimal JSON encoder. For v0.4 we support encoding only.
   Full parsing comes in v0.4+.                                    */

/* Encode a string with JSON escaping */
char* haki_json_string(const char* s) {
    if (!s) return strdup("null");
    size_t len = strlen(s);
    /* Worst case: every char becomes \uXXXX (6 chars) + quotes */
    char* out = (char*)malloc(len * 6 + 3);
    char* p = out;
    *p++ = '"';
    while (*s) {
        unsigned char c = (unsigned char)*s++;
        switch (c) {
            case '"':  *p++ = '\\'; *p++ = '"';  break;
            case '\\': *p++ = '\\'; *p++ = '\\'; break;
            case '\n': *p++ = '\\'; *p++ = 'n';  break;
            case '\r': *p++ = '\\'; *p++ = 'r';  break;
            case '\t': *p++ = '\\'; *p++ = 't';  break;
            default:
                if (c < 0x20) { p += sprintf(p, "\\u%04x", c); }
                else { *p++ = (char)c; }
        }
    }
    *p++ = '"';
    *p   = '\0';
    return out;
}

char* haki_json_int(int64_t n) {
    char buf[32];
    snprintf(buf, sizeof(buf), "%lld", (long long)n);
    return strdup(buf);
}

char* haki_json_float(double f) {
    char buf[64];
    snprintf(buf, sizeof(buf), "%g", f);
    return strdup(buf);
}

char* haki_json_bool(int8_t b) {
    return strdup(b ? "true" : "false");
}

/* Concatenate two JSON fragments with a separator */
char* haki_json_concat(const char* a, const char* sep, const char* b) {
    if (!a) a = "";
    if (!b) b = "";
    if (!sep) sep = "";
    size_t la = strlen(a), ls = strlen(sep), lb = strlen(b);
    char* r = (char*)malloc(la + ls + lb + 1);
    memcpy(r, a, la);
    memcpy(r + la, sep, ls);
    memcpy(r + la + ls, b, lb);
    r[la + ls + lb] = '\0';
    return r;
}



#ifndef HAKI_JSON_FLAT_DEFINED
void haki_json_decode_impl(const char* s, HakiMap** out_map, char** out_error); /* forward decl */
#endif

/* ── JSON API wrappers (Haki API names → C implementations, legacy) ────────
   These wrappers are dead code when std/json uses the new haki_json_* API.  */

static inline const char* jsonString(const char* s) { return haki_json_string(s); }
static inline const char* jsonInt(int64_t n) { return haki_json_int(n); }
static inline const char* jsonBool(int8_t b) { return haki_json_bool(b); }

/* jsonEncodeObject: build JSON object from Map<string,string>. */
static const char* jsonEncodeObject(HakiMap* m) {
    if (!m) return strdup("{}");
    int64_t n = haki_map_length(m);
    size_t cap = 32, olen = 0;
    char* out = (char*)malloc(cap);
    out[olen++] = '{';
    for (int64_t i = 0; i < n; i++) {
        const char* k = haki_map_entry_key(m, i);
        void* vp = haki_map_entry_value(m, i);
        const char* v = vp ? *(const char**)vp : NULL;
        if (!k) continue;
        char* ks = haki_json_string(k);
        const char* vs = v ? v : "null";
        size_t needed = olen + strlen(ks) + 1 + strlen(vs) + 3;
        while (cap <= needed) { cap *= 2; out = (char*)realloc(out, cap); }
        if (i > 0) out[olen++] = ',';
        strcpy(out + olen, ks); olen += strlen(ks);
        out[olen++] = ':';
        strcpy(out + olen, vs); olen += strlen(vs);
        free(ks);
    }
    out[olen++] = '}'; out[olen] = '\0';
    return out;
}

/* jsonEncodeArray: build JSON array from Array<string>. */
static const char* jsonEncodeArray(HakiArray* arr) {
    if (!arr || arr->length == 0) return strdup("[]");
    size_t cap = 32, olen = 0;
    char* out = (char*)malloc(cap);
    out[olen++] = '[';
    for (int64_t i = 0; i < arr->length; i++) {
        const char* v = *(const char**)haki_array_get(arr, i);
        if (!v) v = "null";
        size_t needed = olen + strlen(v) + 3;
        while (cap <= needed) { cap *= 2; out = (char*)realloc(out, cap); }
        if (i > 0) out[olen++] = ',';
        strcpy(out + olen, v); olen += strlen(v);
    }
    out[olen++] = ']'; out[olen] = '\0';
    return out;
}

/* jsonDecode: returns a struct{f0=map, f1=error} matching Haki multi-return Tuple2. */
#ifndef HAKI_JSON_TUPLE2_DEFINED
#define HAKI_JSON_TUPLE2_DEFINED
typedef struct { void* f0; void* f1; } HakiJsonTuple2;
#endif
#ifndef HAKI_JSON_FLAT_DEFINED
static void* jsonDecode(const char* s) {
    HakiMap* m = NULL; char* err = NULL;
    haki_json_decode_impl(s, &m, &err);
    HakiJsonTuple2* t = (HakiJsonTuple2*)malloc(sizeof(HakiJsonTuple2));
    t->f0 = m;
    t->f1 = err ? haki_error_new(err) : NULL;
    return t;
}
#endif

#ifndef HAKI_JSON_FLAT_DEFINED
/* Forward declaration needed since haki_json_decode_get is defined later */
const char* haki_json_decode_get(const char* s, const char* key);
#endif

#ifndef HAKI_JSON_FLAT_DEFINED
static const char* jsonDecodeGet(const char* s, const char* key) {
    return haki_json_decode_get(s, key);
}
#endif


/* HTTP response constructors — MHD-independent */
HttpResponse* haki_http_response_new(int64_t status, const char* body) {
    HttpResponse* r = (HttpResponse*)malloc(sizeof(HttpResponse));
    if (!r) abort();
    r->status       = (int)status;
    r->body         = body ? strdup(body) : strdup("");
    r->contentType = "text/plain";
    return r;
}

HttpResponse* haki_http_response_json(int64_t status, const char* json) {
    HttpResponse* r = haki_http_response_new(status, json);
    r->contentType = "application/json";
    return r;
}

/* Helper called from Haki HttpResponse(...) constructor syntax */
static inline HttpResponse* haki_make_http_response(int status, const char* body, const char* ct) {
    HttpResponse* r = (HttpResponse*)malloc(sizeof(HttpResponse));
    r->status = status; r->body = body ? strdup(body) : strdup(""); r->contentType = ct; return r;
}


/* ── Router ─────────────────────────────────────────────────────────
   Simple pattern-matching router for Haki HTTP servers.

   Patterns:
     /users        — exact match
     /users/:id    — segment wildcard, captures into params map
     /files/*      — prefix wildcard (matches anything after)

   Routes are stored in insertion order; first match wins.          */

typedef HttpResponse* (*HakiRouteHandler)(HttpRequest*);

typedef struct HakiRoute {
    char*             method;   /* "GET", "POST", etc. or "*" = any */
    char*             pattern;  /* e.g. "/users/:id" */
    HakiRouteHandler  handler;
} HakiRoute;

#define HAKI_ROUTER_MAX_ROUTES 256
#define HAKI_ROUTER_MAX_PARAMS  16

typedef struct {
    HakiRoute routes[HAKI_ROUTER_MAX_ROUTES];
    int       count;
} HakiRouter;

typedef HakiRouter Router;

HakiRouter* haki_router_new(void) {
    HakiRouter* r = (HakiRouter*)calloc(1, sizeof(HakiRouter));
    if (!r) abort();
    return r;
}

static void* HttpServer__router(void* s) {
    (void)s;
    return (void*)haki_router_new();
}

static inline void* haki_router_new_opaque(void) { return (void*)haki_router_new(); }

void haki_router_add(HakiRouter* router, const char* method, const char* pattern, HakiRouteHandler handler) {
    if (!router || router->count >= HAKI_ROUTER_MAX_ROUTES) return;
    HakiRoute* r = &router->routes[router->count++];
    r->method  = strdup(method  ? method  : "*");
    r->pattern = strdup(pattern ? pattern : "/");
    r->handler = handler;
}

/* Match a URL path against a pattern.
   Fills params_keys/params_vals (up to HAKI_ROUTER_MAX_PARAMS pairs).
   Returns 1 on match, 0 on no match.                                */
static int haki_route_match(
    const char* pattern, const char* path,
    char** param_keys, char** param_vals, int* param_count)
{
    *param_count = 0;
    const char* p = pattern;
    const char* u = path;

    while (*p && *u) {
        if (*p == '*') { return 1; }  /* wildcard — match rest */
        if (*p == ':') {
            /* Named segment: consume up to next '/' in both */
            p++;
            const char* key_start = p;
            while (*p && *p != '/') p++;
            size_t key_len = (size_t)(p - key_start);

            const char* val_start = u;
            while (*u && *u != '/') u++;
            size_t val_len = (size_t)(u - val_start);

            if (*param_count < HAKI_ROUTER_MAX_PARAMS) {
                param_keys[*param_count] = strndup(key_start, key_len);
                param_vals[*param_count] = strndup(val_start, val_len);
                (*param_count)++;
            }
        } else {
            if (*p != *u) return 0;
            p++; u++;
        }
    }
    /* Both must be exhausted (or pattern ends with wildcard) */
    return (*p == '\0' || *p == '*') && *u == '\0';
}

/* Internal request wrapper that carries params */
typedef struct {
    HttpRequest base;     /* must be first — cast-compatible */
    HakiMap*        params;   /* extracted route parameters */
} HakiRoutedRequest;

static HttpResponse* haki_not_found(void) {
    return haki_http_response_new(404, "Not found");
}

static HttpResponse* haki_method_not_allowed(void) {
    return haki_http_response_new(405, "Method not allowed");
}

/* The router's main dispatch function — called per request. */
HttpResponse* haki_router_dispatch(HakiRouter* router, HttpRequest* req) {
    if (!router || !req) return haki_not_found();

    const char* method = req->method ? req->method : "";
    const char* path   = req->path   ? req->path   : "/";

    int path_matched = 0;

    for (int i = 0; i < router->count; i++) {
        HakiRoute* route = &router->routes[i];

        char* param_keys[HAKI_ROUTER_MAX_PARAMS];
        char* param_vals[HAKI_ROUTER_MAX_PARAMS];
        int   param_count = 0;

        if (!haki_route_match(route->pattern, path, param_keys, param_vals, &param_count))
            continue;

        path_matched = 1;

        /* HEAD requests are handled by the GET handler (HTTP spec §4.3.2). */
        int method_matches = strcmp(route->method, "*") == 0
            || strcmp(route->method, method) == 0
            || (strcmp(method, "HEAD") == 0 && strcmp(route->method, "GET") == 0);

        if (!method_matches) {
            for (int j = 0; j < param_count; j++) { free(param_keys[j]); free(param_vals[j]); }
            continue;
        }

        /* Build a routed request with params map */
        HakiRoutedRequest* rr = (HakiRoutedRequest*)calloc(1, sizeof(HakiRoutedRequest));
        rr->base.method     = req->method;
        rr->base.path       = req->path;
        rr->base.body       = req->body;
        rr->base.body_len   = req->body_len;
        rr->base.connection = req->connection; /* propagate for query string lookup */
        rr->params = haki_map_new(sizeof(char*)); /* string values */
        for (int j = 0; j < param_count; j++) {
            char* val = param_vals[j];
            haki_map_set(rr->params, param_keys[j], &val);
            free(param_keys[j]);
        }

        HttpResponse* resp = route->handler((HttpRequest*)rr);
        haki_map_free(rr->params);
        free(rr);
        return resp;
    }

    return path_matched ? haki_method_not_allowed() : haki_not_found();
}

/* Accessor: get a route param by key from the request.
   Returns empty string if not found (router-dispatched requests only). */
const char* haki_request_param(HttpRequest* req, const char* key) {
    HakiRoutedRequest* rr = (HakiRoutedRequest*)req;
    if (!rr || !rr->params || !key) return "";
    void* val_ptr = haki_map_get(rr->params, key);
    if (!val_ptr) return "";
    return *(const char**)val_ptr;
}

/* Phase 3 extras: query string, headers, content-type on response */

/* Parse query string from req->path (?key=val&key2=val2 …) */
const char* haki_request_query(HttpRequest* req, const char* key) {
    if (!req || !key) return "";
    /* Manual parse of query string from path */
    if (!req->path) return "";
    const char* q = strchr(req->path, '?');
    if (!q) return "";
    q++;
    size_t key_len = strlen(key);
    while (*q) {
        if (strncmp(q, key, key_len) == 0 && (q[key_len] == '=' || q[key_len] == '\0')) {
            if (q[key_len] == '=') {
                const char* vs = q + key_len + 1;
                const char* ve = strchr(vs, '&');
                size_t vl = ve ? (size_t)(ve - vs) : strlen(vs);
                return strndup(vs, vl);
            }
            return "";
        }
        q = strchr(q, '&');
        if (!q) break;
        q++;
    }
    return "";
}

/* Build response with explicit content-type */
HttpResponse* haki_http_response_typed(int64_t status, const char* body, const char* contentType) {
    HttpResponse* r = haki_http_response_new(status, body);
    r->contentType = contentType ? strdup(contentType) : "text/plain";
    return r;
}

/* HttpResponse field accessors */
int64_t     haki_http_response_status(HttpResponse* r) { return r ? r->status : 0; }
const char* haki_http_response_body(HttpResponse* r)   { return r ? r->body   : ""; }

/* ── Phase 4: Template rendering + static files ──────────────────────
   renderTemplate(path, key1, val1, key2, val2, ...) reads a file and
   replaces {{key}} placeholders with corresponding values.
   Up to 16 key/value pairs supported.
   Returns heap-allocated result string, or NULL + sets error.        */

/* Replace all occurrences of `needle` in `haystack` with `replacement`.
   Returns a new heap-allocated string.                                */
static char* str_replace_all(const char* haystack, const char* needle, const char* replacement) {
    if (!haystack || !needle || !*needle) return strdup(haystack ? haystack : "");
    if (!replacement) replacement = "";
    size_t nlen = strlen(needle);
    size_t rlen = strlen(replacement);

    /* Count occurrences */
    size_t count = 0;
    const char* p = haystack;
    while ((p = strstr(p, needle))) { count++; p += nlen; }
    if (!count) return strdup(haystack);

    size_t hlen   = strlen(haystack);
    size_t newlen = hlen + count * (rlen - nlen);
    char*  result = (char*)malloc(newlen + 1);
    char*  out    = result;

    p = haystack;
    const char* found;
    while ((found = strstr(p, needle))) {
        size_t chunk = (size_t)(found - p);
        memcpy(out, p, chunk);
        out += chunk;
        memcpy(out, replacement, rlen);
        out += rlen;
        p = found + nlen;
    }
    strcpy(out, p);
    return result;
}

/* renderTemplate: reads `path`, replaces {{key}} with values.
   keys/vals are parallel arrays, count is the number of pairs.
   Returns heap-allocated result string; sets *out_error on failure.  */
void haki_render_template(
    const char* path,
    const char** keys, const char** vals, int64_t count,
    char** out_result, char** out_error)
{
    *out_result = NULL;
    *out_error  = NULL;

    FILE* f = fopen(path, "rb");
    if (!f) { *out_error = strdup(strerror(errno)); return; }
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    rewind(f);
    if (size < 0) { fclose(f); *out_error = strdup("seek failed"); return; }
    char* content = (char*)malloc((size_t)size + 1);
    fread(content, 1, (size_t)size, f);
    fclose(f);
    content[size] = '\0';

    char* current = content;
    for (int64_t i = 0; i < count; i++) {
        const char* key = keys[i];
        const char* val = vals[i] ? vals[i] : "";
        /* Wrap key in {{ }} */
        size_t placeholder_len = strlen(key) + 4; /* "{{" + key + "}}" */
        char*  placeholder     = (char*)malloc(placeholder_len + 1);
        snprintf(placeholder, placeholder_len + 1, "{{%s}}", key);
        char* next = str_replace_all(current, placeholder, val);
        free(placeholder);
        if (current != content) free(current);
        current = next;
    }
    if (current == content) {
        *out_result = content;
    } else {
        *out_result = current;
        free(content);
    }
}

/* Infer content-type from file extension */
static const char* infer_contentType(const char* path) {
    const char* dot = strrchr(path, '.');
    if (!dot) return "application/octet-stream";
    if (strcmp(dot, ".html") == 0 || strcmp(dot, ".htm") == 0) return "text/html; charset=utf-8";
    if (strcmp(dot, ".css")  == 0) return "text/css";
    if (strcmp(dot, ".js")   == 0) return "application/javascript";
    if (strcmp(dot, ".json") == 0) return "application/json";
    if (strcmp(dot, ".png")  == 0) return "image/png";
    if (strcmp(dot, ".jpg")  == 0 || strcmp(dot, ".jpeg") == 0) return "image/jpeg";
    if (strcmp(dot, ".gif")  == 0) return "image/gif";
    if (strcmp(dot, ".svg")  == 0) return "image/svg+xml";
    if (strcmp(dot, ".txt")  == 0) return "text/plain";
    if (strcmp(dot, ".ico")  == 0) return "image/x-icon";
    if (strcmp(dot, ".woff2")== 0) return "font/woff2";
    if (strcmp(dot, ".pdf")  == 0) return "application/pdf";
    return "application/octet-stream";
}

/* serveFile: reads `path` and returns an HttpResponse with correct content-type.
   Returns 404 if the file is not found.                              */
HttpResponse* haki_serve_file(const char* path) {
    FILE* f = fopen(path, "rb");
    if (!f) return haki_http_response_new(404, "Not found");

    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    rewind(f);
    if (size < 0) { fclose(f); return haki_http_response_new(500, "Server error"); }

    char* body = (char*)malloc((size_t)size + 1);
    size_t n = fread(body, 1, (size_t)size, f);
    fclose(f);
    body[n] = '\0';

    /* Use haki_http_response_typed so the struct is correctly initialised
       the same way all other responses are — avoids raw-malloc pitfalls. */
    HttpResponse* resp = haki_http_response_typed(200, body, infer_contentType(path));
    free(body); /* haki_http_response_typed strdup's the body */
    return resp;
}

/* ── Phase 5: JSON decode ────────────────────────────────────────────
   Flat JSON object parser: {"key": "value", "n": 42}
   Only string values in v0.4.5 — numbers stored as strings too.     */

/* Skip whitespace */
static const char* json_skip_ws(const char* p) {
    while (*p == ' ' || *p == '\t' || *p == '\n' || *p == '\r') p++;
    return p;
}

/* Parse a JSON string. Returns heap-allocated value, advances *pp past closing quote.
   Returns NULL on error.                                              */
static char* json_parse_string(const char** pp) {
    const char* p = *pp;
    if (*p != '"') return NULL;
    p++; /* skip opening quote */
    const char* start = p;
    while (*p && *p != '"') {
        if (*p == '\\') p++; /* skip escaped char */
        if (*p) p++;
    }
    if (!*p) return NULL;
    size_t len = (size_t)(p - start);
    char*  result = strndup(start, len);
    *pp = p + 1; /* skip closing quote */
    return result;
}

/* Parse a JSON number (stored as string) */
static char* json_parse_number(const char** pp) {
    const char* start = *pp;
    if (*start == '-') (*pp)++;
    while (**pp >= '0' && **pp <= '9') (*pp)++;
    if (**pp == '.') { (*pp)++; while (**pp >= '0' && **pp <= '9') (*pp)++; }
    return strndup(start, (size_t)(*pp - start));
}

#ifndef HAKI_JSON_FLAT_DEFINED
/* haki_json_decode_impl: parse flat JSON object {"k": "v"} into a HakiMap (internal).
   Returns map on success, NULL + *out_error on failure.              */
void haki_json_decode_impl(const char* s, HakiMap** out_map, char** out_error) {
    *out_map    = NULL;
    *out_error  = NULL;
    if (!s) { *out_error = strdup("null input"); return; }

    const char* p = json_skip_ws(s);
    if (*p != '{') { *out_error = strdup("expected '{'"); return; }
    p++;

    HakiMap* m = haki_map_new(sizeof(char*));

    while (1) {
        p = json_skip_ws(p);
        if (*p == '}') break;
        if (*p == ',') { p++; p = json_skip_ws(p); }
        if (*p == '}') break;

        /* Parse key */
        char* key = json_parse_string(&p);
        if (!key) { *out_error = strdup("expected string key"); haki_map_free(m); return; }

        p = json_skip_ws(p);
        if (*p != ':') { free(key); *out_error = strdup("expected ':'"); haki_map_free(m); return; }
        p++;
        p = json_skip_ws(p);

        /* Parse value — string or number only in v0.4.5 */
        char* val = NULL;
        if (*p == '"') {
            val = json_parse_string(&p);
        } else if (*p == '-' || (*p >= '0' && *p <= '9')) {
            val = json_parse_number(&p);
        } else if (strncmp(p, "true", 4) == 0)  { val = strdup("true");  p += 4; }
        else if (strncmp(p, "false", 5) == 0)    { val = strdup("false"); p += 5; }
        else if (strncmp(p, "null",  4) == 0)    { val = strdup("");      p += 4; }
        else { free(key); *out_error = strdup("unsupported value type"); haki_map_free(m); return; }

        haki_map_set(m, key, &val);
        free(key);
    }

    *out_map = m;
}

/* renderTemplate_map: takes a HakiMap<string,string> and replaces {{key}} placeholders. */
void haki_render_template_map(const char* path, HakiMap* data, char** out_result, char** out_error) {
    *out_result = NULL; *out_error = NULL;
    FILE* f = fopen(path, "rb");
    if (!f) { *out_error = strdup(strerror(errno)); return; }
    fseek(f, 0, SEEK_END); long size = ftell(f); rewind(f);
    if (size < 0) { fclose(f); *out_error = strdup("seek failed"); return; }
    char* content = (char*)malloc((size_t)size + 1);
    fread(content, 1, (size_t)size, f); fclose(f); content[size] = '\0';
    if (!data) { *out_result = content; return; }
    char* current = content;
    for (int64_t i = 0; i < data->capacity; i++) {
        HakiMapEntry* e = &data->entries[i];
        if (!e->key) continue;
        const char* val = e->value ? *(const char**)e->value : "";
        if (!val) val = "";
        size_t klen = strlen(e->key);
        char* ph = (char*)malloc(klen + 5);
        snprintf(ph, klen + 5, "{{%s}}", e->key);
        char* next = str_replace_all(current, ph, val);
        free(ph);
        if (current != content) free(current);
        current = next;
    }
    if (current == content) { *out_result = content; }
    else { *out_result = current; free(content); }
}

/* jsonDecodeGet: decode flat JSON and return one key's value. */
const char* haki_json_decode_get(const char* s, const char* key) {
    HakiMap* m = NULL; char* err = NULL;
    haki_json_decode_impl(s, &m, &err);
    if (err || !m) { if (err) free(err); return ""; }
    void* vp = haki_map_get(m, key);
    const char* result = vp ? strdup(*(const char**)vp) : strdup("");
    haki_map_free(m);
    return result;
}
#endif /* HAKI_JSON_FLAT_DEFINED */

#ifndef HAKI_CHAN_TYPES_DEFINED
#define HAKI_CHAN_TYPES_DEFINED
/* ── Channels (Chan<T>) ─────────────────────────────────────────────────────
 *
 * Thread-safe bounded FIFO ring buffer with mutex + condition variables.
 */

typedef struct HakiChan {
    pthread_mutex_t mu;
    pthread_cond_t  not_empty;
    pthread_cond_t  not_full;
    int64_t         elem_size;
    int64_t         capacity;
    int64_t         head;
    int64_t         tail;
    int64_t         count;
    int             closed;
    char*           buf;
} HakiChan;


HakiChan* haki_chan_new(int64_t capacity, int64_t elem_size) {
    HakiChan* ch = (HakiChan*)calloc(1, sizeof(HakiChan));
    pthread_mutex_init(&ch->mu, NULL);
    pthread_cond_init(&ch->not_empty, NULL);
    pthread_cond_init(&ch->not_full, NULL);
    ch->elem_size = elem_size;
    ch->capacity  = capacity > 0 ? capacity : 1024;
    ch->buf       = (char*)malloc(ch->capacity * elem_size);
    return ch;
}

int haki_chan_send(HakiChan* ch, void* val) {
    pthread_mutex_lock(&ch->mu);
    while (ch->count == ch->capacity && !ch->closed)
        pthread_cond_wait(&ch->not_full, &ch->mu);
    if (ch->closed) { pthread_mutex_unlock(&ch->mu); return 1; }
    memcpy(ch->buf + ch->tail * ch->elem_size, val, ch->elem_size);
    ch->tail = (ch->tail + 1) % ch->capacity;
    ch->count++;
    pthread_cond_signal(&ch->not_empty);
    pthread_mutex_unlock(&ch->mu);
    return 0;
}

int haki_chan_recv(HakiChan* ch, void* out) {
    pthread_mutex_lock(&ch->mu);
    while (ch->count == 0 && !ch->closed)
        pthread_cond_wait(&ch->not_empty, &ch->mu);
    if (ch->count == 0) { pthread_mutex_unlock(&ch->mu); return 1; }
    memcpy(out, ch->buf + ch->head * ch->elem_size, ch->elem_size);
    ch->head = (ch->head + 1) % ch->capacity;
    ch->count--;
    pthread_cond_signal(&ch->not_full);
    pthread_mutex_unlock(&ch->mu);
    return 0;
}

void haki_chan_close(HakiChan* ch) {
    pthread_mutex_lock(&ch->mu);
    ch->closed = 1;
    pthread_cond_broadcast(&ch->not_empty);
    pthread_cond_broadcast(&ch->not_full);
    pthread_mutex_unlock(&ch->mu);
}

int haki_chan_is_closed(HakiChan* ch) {
    pthread_mutex_lock(&ch->mu);
    int c = ch->closed;
    pthread_mutex_unlock(&ch->mu);
    return c;
}

void haki_chan_free(HakiChan* ch) {
    pthread_mutex_destroy(&ch->mu);
    pthread_cond_destroy(&ch->not_empty);
    pthread_cond_destroy(&ch->not_full);
    free(ch->buf);
    free(ch);
}


/* ── TaskGroup ───────────────────────────────────────────────────────────────
 *
 * Structured concurrency: owns N spawned threads, joins all at awaitAll().
 */

#define HAKI_TASKGROUP_MAX 256

typedef struct {
    pthread_t thread;
    void*     result;
} HakiTask;

typedef struct HakiTaskGroup {
    HakiTask        tasks[HAKI_TASKGROUP_MAX];
    int64_t         count;
    pthread_mutex_t mu;
} HakiTaskGroup;

typedef struct { void* (*fn)(void*); void* arg; void** out; } HakiTaskArg;

static void* haki_task_trampoline(void* raw) {
    HakiTaskArg* ta = (HakiTaskArg*)raw;
    *ta->out = ta->fn(ta->arg);
    free(ta);
    return NULL;
}

HakiTaskGroup* haki_taskgroup_new(void) {
    HakiTaskGroup* g = (HakiTaskGroup*)calloc(1, sizeof(HakiTaskGroup));
    pthread_mutex_init(&g->mu, NULL);
    return g;
}

void haki_taskgroup_spawn(HakiTaskGroup* g, void* (*fn)(void*), void* arg) {
    pthread_mutex_lock(&g->mu);
    if (g->count >= HAKI_TASKGROUP_MAX) { pthread_mutex_unlock(&g->mu); return; }
    int64_t idx = g->count++;
    pthread_mutex_unlock(&g->mu);
    HakiTaskArg* ta = (HakiTaskArg*)malloc(sizeof(HakiTaskArg));
    ta->fn  = fn;
    ta->arg = arg;
    ta->out = &g->tasks[idx].result;
    pthread_create(&g->tasks[idx].thread, NULL, haki_task_trampoline, ta);
}

HakiArray* haki_taskgroup_await_all(HakiTaskGroup* g) {
    HakiArray* out = haki_array_new(sizeof(void*));
    for (int64_t i = 0; i < g->count; i++) {
        pthread_join(g->tasks[i].thread, NULL);
        void* r = g->tasks[i].result;
        haki_array_append(out, &r);
    }
    pthread_mutex_destroy(&g->mu);
    free(g);
    return out;
}

/* ── select ──────────────────────────────────────────────────────────────────
 *
 * Polls N channels. Returns index of first ready channel, or -1 on timeout.
 * Timeout of -1 means block forever.
 */

typedef struct {
    HakiChan* ch;
    void*     out;
    int       ready;
} HakiSelectCase;

int haki_select(HakiSelectCase* cases, int64_t n, int64_t timeout_ms) {
    struct timespec start, now;
    clock_gettime(CLOCK_MONOTONIC, &start);
    int64_t sleep_ns = 10000LL; /* 10µs */
    while (1) {
        for (int64_t i = 0; i < n; i++) {
            HakiChan* ch = cases[i].ch;
            if (!ch) continue;
            pthread_mutex_lock(&ch->mu);
            int has_data = (ch->count > 0);
            if (has_data) {
                memcpy(cases[i].out, ch->buf + ch->head * ch->elem_size, ch->elem_size);
                ch->head = (ch->head + 1) % ch->capacity;
                ch->count--;
                pthread_cond_signal(&ch->not_full);
                cases[i].ready = 1;
                pthread_mutex_unlock(&ch->mu);
                return (int)i;
            }
            pthread_mutex_unlock(&ch->mu);
        }
        if (timeout_ms >= 0) {
            clock_gettime(CLOCK_MONOTONIC, &now);
            int64_t elapsed_ms = (now.tv_sec - start.tv_sec) * 1000LL
                               + (now.tv_nsec - start.tv_nsec) / 1000000LL;
            if (elapsed_ms >= timeout_ms) return -1;
        }
        struct timespec ts = { 0, sleep_ns };
        nanosleep(&ts, NULL);
        if (sleep_ns < 1000000LL) sleep_ns *= 2;
    }
}

#endif /* HAKI_CHAN_TYPES_DEFINED */
"#;

/// The GTK 3 UI runtime — compiled only when the program uses haki_ui types.
/// Requires GTK 3 headers and libs at compile time.
pub const UI_RUNTIME_C_SOURCE: &str = r#"
/**
 * haki_ui_gtk.c — GTK 3 platform backend for haki_ui v2.x
 *
 * Virtual Tree + diff architecture. The C layer only ever sees integers (node_id).
 * Haki owns the VNode graph and the callback closures. GTK sees no Haki memory.
 *
 * Haki → C:  haki_gtk_create_*(parent_id, ...) → new node_id
 *            haki_gtk_set_text(node_id, text)
 *            haki_gtk_insert_child(parent_id, index, child_id)
 *            haki_gtk_remove_child(node_id)
 *
 * C → Haki:  trigger_haki_callback(node_id)
 *            (Haki registered haki_set_callback_dispatcher before gtk_main)
 *
 * Build (linked by hakic --target gtk automatically):
 *   gcc -O2 $(pkg-config --cflags gtk+-3.0) haki_ui_gtk.c \
 *       $(pkg-config --libs gtk+-3.0) -o myapp
 */

#include <gtk/gtk.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

/* Forward declarations */
static void do_rerender(void);  /* defined later, called by on_button_clicked */

/* Haki closure = fat pointer: { fn_ptr(void* env), env_ptr }
   Stored as void*[2]: [0]=fn_ptr, [1]=env_ptr */
#define HAKI_MAX_CALLBACKS 4096
static void* g_callbacks_fwd[HAKI_MAX_CALLBACKS];  /* void* closure fat pointer */

static void haki_fire_callback(int64_t node_id) {
    if (node_id <= 0 || node_id >= HAKI_MAX_CALLBACKS) return;
    void* closure = g_callbacks_fwd[node_id];
    if (!closure) return;
    /* haki_make_closure builds void*[2] = { fn_ptr, env_ptr }
       fn_ptr signature: void fn(void* __env) */
    void** fat = (void**)closure;
    void (*fn_ptr)(void*) = (void(*)(void*))fat[0];
    void* env_ptr = fat[1];
    fn_ptr(env_ptr);
}

/* Payload structs for multi-field enum variants */
typedef struct { void* f0; void* f1; } __PayloadTuple2;
typedef struct { void* f0; void* f1; void* f2; } __PayloadTuple3;

// ── Node registry ─────────────────────────────────────────────────────────────
// Maps integer node_id → GtkWidget*.
// GTK only ever sees GtkWidget* internally; Haki only sees int64_t.
// Max 65536 nodes per window — sufficient for any practical UI.

#define HAKI_MAX_NODES 65536

static GtkWidget* g_nodes[HAKI_MAX_NODES];
static int64_t    g_next_id = 1;   // 0 = root window

static GtkWidget* node_get(int64_t id) {
    if (id <= 0 || id >= HAKI_MAX_NODES) return NULL;
    return g_nodes[id];
}

static int64_t node_alloc(GtkWidget* w) {
    int64_t id = g_next_id++;
    if (id >= HAKI_MAX_NODES) { fprintf(stderr, "haki_ui: node limit reached\n"); abort(); }
    g_nodes[id] = w;
    return id;
}

static void node_free(int64_t id) {
    if (id > 0 && id < HAKI_MAX_NODES) g_nodes[id] = NULL;
}

// ── Callback dispatcher ───────────────────────────────────────────────────────
// Haki calls haki_set_callback_dispatcher() before gtk_main() to register
// the Haki-side closure lookup function.
// When a GTK button is clicked, g_dispatcher(node_id) calls back into Haki.

typedef void (*HakiDispatchFn)(int64_t node_id);
static HakiDispatchFn g_dispatcher = NULL;

void haki_set_callback_dispatcher(HakiDispatchFn fn) {
    g_dispatcher = fn;
}

// GTK signal handler
static void on_button_clicked(GtkWidget* widget, gpointer user_data) {
    int64_t node_id = (int64_t)(intptr_t)user_data;
    haki_fire_callback(node_id);
    if (g_dispatcher) g_dispatcher(node_id);
    do_rerender();
}

// ── Window ────────────────────────────────────────────────────────────────────

static GtkWidget* g_window = NULL;

int64_t haki_gtk_create_window(const char* title, int64_t width, int64_t height) {
    gtk_init(NULL, NULL);
    g_window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    gtk_window_set_title(GTK_WINDOW(g_window), title);
    gtk_window_set_default_size(GTK_WINDOW(g_window), (int)width, (int)height);
    g_signal_connect(g_window, "destroy", G_CALLBACK(gtk_main_quit), NULL);
    // Window itself is node 0 — not in the registry, referenced directly
    return 0;
}

// ── Widget creation ───────────────────────────────────────────────────────────

int64_t haki_gtk_create_label(int64_t parent_id, const char* text) {
    GtkWidget* label = gtk_label_new(text);
    int64_t id = node_alloc(label);
    // Attach to parent if parent is a container
    GtkWidget* parent = node_get(parent_id);
    if (parent && GTK_IS_BOX(parent)) {
        gtk_box_pack_start(GTK_BOX(parent), label, FALSE, FALSE, 4);
    } else if (g_window && parent_id == 0) {
        // Root container
    }
    gtk_widget_show(label);
    return id;
}

int64_t haki_gtk_create_button(int64_t parent_id, const char* label_text, int64_t node_id_hint) {
    GtkWidget* btn = gtk_button_new_with_label(label_text);
    // Use the hint as the node_id for the callback — it must match the
    // node_id Haki stored in its callback registry.
    int64_t id = (node_id_hint > 0) ? node_id_hint : node_alloc(btn);
    if (node_id_hint > 0) g_nodes[node_id_hint] = btn;
    g_signal_connect(btn, "clicked", G_CALLBACK(on_button_clicked),
                     (gpointer)(intptr_t)id);
    GtkWidget* parent = node_get(parent_id);
    if (parent && GTK_IS_BOX(parent)) {
        gtk_box_pack_start(GTK_BOX(parent), btn, FALSE, FALSE, 4);
    }
    gtk_widget_show(btn);
    return id;
}

// horizontal=1 → GtkHBox, horizontal=0 → GtkVBox
int64_t haki_gtk_create_box(int64_t parent_id, int64_t horizontal) {
    GtkWidget* box = gtk_box_new(
        horizontal ? GTK_ORIENTATION_HORIZONTAL : GTK_ORIENTATION_VERTICAL, 8);
    int64_t id = node_alloc(box);
    GtkWidget* parent = node_get(parent_id);
    if (parent && GTK_IS_BOX(parent)) {
        gtk_box_pack_start(GTK_BOX(parent), box, TRUE, TRUE, 4);
    } else if (parent_id == 0 && g_window) {
        gtk_container_add(GTK_CONTAINER(g_window), box);
    }
    gtk_widget_show(box);
    return id;
}

// ── Surgical mutations ────────────────────────────────────────────────────────
// These are the ONLY functions GTK calls. Haki drives all mutations;
// GTK just executes them on its widget tree.

void haki_gtk_set_text(int64_t node_id, const char* text) {
    GtkWidget* w = node_get(node_id);
    fprintf(stderr, "[haki_ui] set_text: node_id=%lld widget=%p text=%s\n",
            (long long)node_id, (void*)w, text ? text : "(null)");
    if (!w) { fprintf(stderr, "[haki_ui] set_text: WIDGET NOT FOUND\n"); return; }
    if (GTK_IS_LABEL(w)) {
        fprintf(stderr, "[haki_ui] set_text: IS_LABEL, updating\n");
        gtk_label_set_text(GTK_LABEL(w), text);
        gtk_widget_queue_draw(w);
    }
    if (GTK_IS_BUTTON(w)) gtk_button_set_label(GTK_BUTTON(w), text);
}

void haki_gtk_set_visible(int64_t node_id, int64_t visible) {
    GtkWidget* w = node_get(node_id);
    if (!w) return;
    if (visible) gtk_widget_show(w);
    else         gtk_widget_hide(w);
}

void haki_gtk_insert_child(int64_t parent_id, int64_t index, int64_t child_id) {
    GtkWidget* parent = node_get(parent_id);
    GtkWidget* child  = node_get(child_id);
    if (!parent || !child) return;
    if (GTK_IS_BOX(parent)) {
        gtk_box_pack_start(GTK_BOX(parent), child, FALSE, FALSE, 4);
        // Reorder to the requested index
        gtk_box_reorder_child(GTK_BOX(parent), child, (int)index);
    }
    gtk_widget_show(child);
}

void haki_gtk_remove_child(int64_t node_id) {
    GtkWidget* w = node_get(node_id);
    if (!w) return;
    GtkWidget* parent = gtk_widget_get_parent(w);
    if (parent) gtk_container_remove(GTK_CONTAINER(parent), w);
    node_free(node_id);
}

// ── Callback registry ────────────────────────────────────────────────────────
// Maps node_id → Haki closure function pointer.
// Haki calls haki_register_callback(id, fn_ptr) during mount.
// GTK button-clicked signal calls haki_fire_callback(id).

void haki_register_callback(int64_t node_id, void* closure) {
    fprintf(stderr, "[haki_ui] register_callback: node_id=%lld closure=%p\n",
            (long long)node_id, closure);
    if (node_id > 0 && node_id < HAKI_MAX_CALLBACKS)
        g_callbacks_fwd[node_id] = closure;
}

int64_t haki_gtk_alloc_node_id_debug(void) {
    int64_t id = g_next_id++;
    fprintf(stderr, "[haki_ui] alloc_node_id: %lld\n", (long long)id);
    return id;
}

// Allocate a stable node_id (separate from widget node_ids)
// Used for buttons so their id is known before widget creation
int64_t haki_gtk_alloc_node_id(void) {
    int64_t id = g_next_id++;
    fprintf(stderr, "[haki_ui] alloc_node_id → %lld\n", (long long)id);
    return id;
}

/* Peek at the next node_id without allocating it */
int64_t haki_gtk_peek_next_id(void) {
    return g_next_id;
}

/* Mark a node as the primary label for rerender */
static int64_t g_marked_label_id = 0;
void haki_gtk_mark_label(int64_t node_id) {
    if (g_marked_label_id == 0) g_marked_label_id = node_id;
    fprintf(stderr, "[haki_ui] mark_label: %lld\n", (long long)node_id);
}
int64_t haki_gtk_get_label_id(void) {
    return g_marked_label_id;
}

// ── Re-render support ────────────────────────────────────────────────────────
// After a button click, call the Haki fn()->string closure to get new label text
// then update the label widget via haki_gtk_set_text.
//
// The closure is a fat pointer: void*[2] = { fn_ptr, env_ptr }
// fn_ptr signature: const char* fn(void* env)

typedef const char* (*HakiStrFn)(void*);
static HakiStrFn g_rerender_fn  = NULL;
static void*     g_rerender_env = NULL;
static int64_t   g_label_node_id = 0;

void haki_set_rerender_callback(int64_t label_id, void* closure) {
    g_label_node_id = label_id;
    fprintf(stderr, "[haki_ui] set_rerender: label_id=%lld closure=%p\n",
            (long long)label_id, closure);
    if (closure) {
        void** fat = (void**)closure;
        fprintf(stderr, "[haki_ui] rerender fat[0]=%p fat[1]=%p\n", fat[0], fat[1]);
        g_rerender_fn  = (HakiStrFn)fat[0];
        g_rerender_env = fat[1];
    }
}

static void do_rerender_debug(void) {
    fprintf(stderr, "[haki_ui] do_rerender: fn=%p env=%p label=%lld\n",
            (void*)g_rerender_fn, g_rerender_env, (long long)g_label_node_id);
}

static void do_rerender(void) {
    fprintf(stderr, "[haki_ui] do_rerender called: fn=%p label=%lld\n",
            (void*)g_rerender_fn, (long long)g_label_node_id);
    if (!g_rerender_fn || !g_label_node_id) return;
    const char* new_text = g_rerender_fn(g_rerender_env);
    fprintf(stderr, "[haki_ui] new_text=%s\n", new_text ? new_text : "(null)");
    if (new_text) haki_gtk_set_text(g_label_node_id, new_text);
}

// ── Event loop ────────────────────────────────────────────────────────────────

void haki_platform_run(void) {
    if (g_window) gtk_widget_show_all(g_window);
    gtk_main();
}

// ── haki_app_run — called by App.run() in app.haki ───────────────────────────
// This is the legacy entry point that the old JSON bridge used.
// In the new architecture it's still called by App.run() but immediately
// delegates to the VNode mount sequence which Haki drives.
// The `json` param is ignored — App.run() will call mount/diff directly.

void haki_app_run(const char* json, const char* title, int64_t width, int64_t height) {
    (void)json; // VNode architecture: Haki calls haki_gtk_create_* directly
    haki_gtk_create_window(title, width, height);
    // Haki will call haki_platform_run() after mounting the VNode tree
    // For backwards compat: if no VNode tree is mounted, just run the loop
    haki_platform_run();
}

"#;

/// Names of all extern functions the codegen must `declare` in the IR.
/// Keyed by the Haki-level name → C-level mangled name.
pub const EXTERN_FN_MAP: &[(&str, &str)] = &[
    ("print",          "haki_print"),
    ("print_int",      "haki_print_int"),
    ("print_float",    "haki_print_float"),
    ("print_bool",     "haki_print_bool"),
    ("int_to_string",  "haki_int_to_string"),
    ("float_to_string","haki_float_to_string"),
    ("bool_to_string", "haki_bool_to_string"),
    ("string_length",  "haki_string_length"),
];

pub const UI_RUNTIME_WIN32_C_SOURCE: &str = r#"
/*
 * haki_ui_win32.c — Haki UI platform layer for Windows (Win32 API)
 *
 * Implements the exact same integer node_id FFI boundary as haki_ui_gtk.c.
 * The Haki VNode diff engine calls these functions with integer IDs only —
 * no Haki memory ever crosses this boundary.
 *
 * Build: cl /c haki_ui_win32.c /link user32.lib comctl32.lib
 *        or: gcc -c haki_ui_win32.c -luser32 -lcomctl32
 *
 * Requires: Windows XP SP3 or later (tested on Windows 10/11)
 * Dependencies: NONE (pure Win32 API, ships with every Windows installation)
 */

#ifndef _WIN32
#error "haki_ui_win32.c is Windows-only. Use haki_ui_gtk.c on Linux/macOS."
#endif

#define WIN32_LEAN_AND_MEAN
#define UNICODE
#define _UNICODE
#include <windows.h>
#include <commctrl.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <wchar.h>

/* Enable visual styles (themed controls) via manifest */
#pragma comment(linker,"/manifestdependency:\"type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'\"")
#pragma comment(lib, "user32.lib")
#pragma comment(lib, "comctl32.lib")

/* ── Node ID registry ────────────────────────────────────────────────────── */

#define HAKI_MAX_NODES 8192

static HWND  g_node_id_map[HAKI_MAX_NODES];  /* VNode id → HWND           */
static int64_t g_next_id = 1;
static HWND  g_window = NULL;                 /* top-level window          */

/* Layout tracking for Column/Row containers */
typedef struct {
    int is_row;          /* 1=horizontal, 0=vertical */
    HWND children[256];
    int  child_count;
    int  padding;
} HakiBox;
static HakiBox g_boxes[HAKI_MAX_NODES];

/* ── UTF-8 ↔ UTF-16 helpers ─────────────────────────────────────────────── */

static wchar_t* utf8_to_wide(const char* s) {
    if (!s) return _wcsdup(L"");
    int len = MultiByteToWideChar(CP_UTF8, 0, s, -1, NULL, 0);
    wchar_t* w = (wchar_t*)malloc(len * sizeof(wchar_t));
    if (w) MultiByteToWideChar(CP_UTF8, 0, s, -1, w, len);
    return w;
}

static char* wide_to_utf8(const wchar_t* w) {
    if (!w) return _strdup("");
    int len = WideCharToMultiByte(CP_UTF8, 0, w, -1, NULL, 0, NULL, NULL);
    char* s = (char*)malloc(len);
    if (s) WideCharToMultiByte(CP_UTF8, 0, w, -1, s, len, NULL, NULL);
    return s;
}

/* ── Callback registry ──────────────────────────────────────────────────── */

#define HAKI_MAX_CALLBACKS 4096
typedef void (*HakiVoidFn)(void*);
static HakiVoidFn g_callbacks[HAKI_MAX_CALLBACKS];
static void*      g_callback_envs[HAKI_MAX_CALLBACKS];

void haki_register_callback(int64_t node_id, void* closure) {
    if (node_id <= 0 || node_id >= HAKI_MAX_CALLBACKS || !closure) return;
    void** fat = (void**)closure;
    g_callbacks[node_id]     = (HakiVoidFn)fat[0];
    g_callback_envs[node_id] = fat[1];
}

static void fire_callback(int64_t node_id) {
    if (node_id <= 0 || node_id >= HAKI_MAX_CALLBACKS) return;
    if (g_callbacks[node_id]) {
        g_callbacks[node_id](g_callback_envs[node_id]);
    }
}

/* ── Rerender fn (diff engine) ──────────────────────────────────────────── */

static HakiVoidFn g_rerender_fn  = NULL;
static void*      g_rerender_env = NULL;

void haki_set_rerender_fn(void* closure) {
    if (!closure) return;
    void** fat = (void**)closure;
    g_rerender_fn  = (HakiVoidFn)fat[0];
    g_rerender_env = fat[1];
}

void haki_trigger_rerender(void) {
    if (g_rerender_fn) {
        g_rerender_fn(g_rerender_env);
        /* Force repaint of all windows */
        if (g_window) InvalidateRect(g_window, NULL, TRUE);
    }
}

/* Legacy single-label rerender (v3.1 compat) */
void haki_set_rerender_callback(int64_t label_id, void* closure) {
    (void)label_id; (void)closure; /* superseded by diff engine */
}

/* ── Layout engine ─────────────────────────────────────────────────────── */

static void haki_reflow_box(int64_t box_id) {
    if (box_id <= 0 || box_id >= HAKI_MAX_NODES) return;
    HWND parent = g_node_id_map[box_id];
    if (!parent) return;
    HakiBox* box = &g_boxes[box_id];
    if (box->child_count == 0) return;

    RECT rc;
    GetClientRect(parent, &rc);
    int W = rc.right  - rc.left;
    int H = rc.bottom - rc.top;
    int pad = box->padding;
    int n = box->child_count;

    if (box->is_row) {
        /* Horizontal: divide width equally */
        int slot_w = (W - pad * (n + 1)) / n;
        for (int i = 0; i < n; i++) {
            int x = pad + i * (slot_w + pad);
            SetWindowPos(box->children[i], NULL, x, pad, slot_w, H - 2*pad,
                         SWP_NOZORDER | SWP_NOACTIVATE);
        }
    } else {
        /* Vertical: divide height equally */
        int slot_h = (H - pad * (n + 1)) / n;
        for (int i = 0; i < n; i++) {
            int y = pad + i * (slot_h + pad);
            SetWindowPos(box->children[i], NULL, pad, y, W - 2*pad, slot_h,
                         SWP_NOZORDER | SWP_NOACTIVATE);
        }
    }
}

/* ── WndProc ────────────────────────────────────────────────────────────── */

static LRESULT CALLBACK haki_wndproc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
    switch (msg) {
    case WM_SIZE:
        /* Reflow all top-level boxes when window is resized */
        for (int64_t i = 1; i < g_next_id; i++) {
            if (g_node_id_map[i] && GetParent(g_node_id_map[i]) == hwnd) {
                /* Check if it's a container */
                if (g_boxes[i].child_count > 0) {
                    haki_reflow_box(i);
                }
            }
        }
        return 0;

    case WM_COMMAND:
        /* Button/checkbox/menu clicks come here */
        if (HIWORD(wp) == BN_CLICKED || HIWORD(wp) == 0) {
            int64_t node_id = (int64_t)GetWindowLongPtrW((HWND)lp, GWLP_ID);
            fire_callback(node_id);
            /* Trigger rerender after any button click */
            if (g_rerender_fn) {
                PostMessage(hwnd, WM_USER + 1, 0, 0);
            }
        }
        return 0;

    case WM_USER + 1:
        /* Deferred rerender — safe to call Haki here */
        haki_trigger_rerender();
        return 0;

    case WM_DESTROY:
        PostQuitMessage(0);
        return 0;

    default:
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
}

/* ── Window creation ────────────────────────────────────────────────────── */

static void init_common_controls(void) {
    INITCOMMONCONTROLSEX icc = { sizeof(icc), ICC_WIN95_CLASSES | ICC_STANDARD_CLASSES };
    InitCommonControlsEx(&icc);
}

static const wchar_t* HAKI_WNDCLASS = L"HakiAppWindow";

static void register_wndclass(void) {
    WNDCLASSEXW wc = {0};
    wc.cbSize        = sizeof(wc);
    wc.style         = CS_HREDRAW | CS_VREDRAW;
    wc.lpfnWndProc   = haki_wndproc;
    wc.hInstance     = GetModuleHandleW(NULL);
    wc.hCursor       = LoadCursorW(NULL, IDC_ARROW);
    wc.hbrBackground = (HBRUSH)(COLOR_WINDOW + 1);
    wc.lpszClassName = HAKI_WNDCLASS;
    wc.hIcon         = LoadIconW(NULL, IDI_APPLICATION);
    RegisterClassExW(&wc);
}

int64_t haki_gtk_create_window(const char* title, int64_t width, int64_t height) {
    init_common_controls();
    register_wndclass();

    wchar_t* wtitle = utf8_to_wide(title ? title : "Haki App");
    g_window = CreateWindowExW(
        0, HAKI_WNDCLASS, wtitle,
        WS_OVERLAPPEDWINDOW,
        CW_USEDEFAULT, CW_USEDEFAULT,
        (int)width > 0 ? (int)width : 800,
        (int)height > 0 ? (int)height : 600,
        NULL, NULL, GetModuleHandleW(NULL), NULL
    );
    free(wtitle);

    int64_t id = g_next_id++;
    g_node_id_map[id] = g_window;
    return id;
}

/* ── Widget creation ────────────────────────────────────────────────────── */

static HWND get_parent_hwnd(int64_t parent_id) {
    if (parent_id > 0 && parent_id < HAKI_MAX_NODES && g_node_id_map[parent_id])
        return g_node_id_map[parent_id];
    return g_window;
}

int64_t haki_gtk_create_label(int64_t parent_id, const char* text) {
    int64_t id = g_next_id++;
    if (id >= HAKI_MAX_NODES) return -1;
    wchar_t* wtext = utf8_to_wide(text ? text : "");
    HWND hw = CreateWindowExW(
        0, L"STATIC", wtext,
        WS_CHILD | WS_VISIBLE | SS_LEFT,
        0, 0, 200, 24,
        get_parent_hwnd(parent_id), (HMENU)(intptr_t)id,
        GetModuleHandleW(NULL), NULL
    );
    free(wtext);
    g_node_id_map[id] = hw;
    return id;
}

int64_t haki_gtk_create_button(int64_t parent_id, const char* label, int64_t node_id_hint) {
    int64_t id = node_id_hint > 0 ? node_id_hint : g_next_id++;
    if (id >= HAKI_MAX_NODES) return -1;
    if (id >= g_next_id) g_next_id = id + 1;
    wchar_t* wlabel = utf8_to_wide(label ? label : "");
    HWND hw = CreateWindowExW(
        0, L"BUTTON", wlabel,
        WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
        0, 0, 120, 32,
        get_parent_hwnd(parent_id), (HMENU)(intptr_t)id,
        GetModuleHandleW(NULL), NULL
    );
    free(wlabel);
    g_node_id_map[id] = hw;
    return id;
}

int64_t haki_gtk_create_box(int64_t parent_id, int64_t horizontal) {
    int64_t id = g_next_id++;
    if (id >= HAKI_MAX_NODES) return -1;
    /* Boxes are plain windows that manage layout of their children */
    HWND hw = CreateWindowExW(
        0, L"STATIC", L"",
        WS_CHILD | WS_VISIBLE | SS_LEFT,
        0, 0, 400, 400,
        get_parent_hwnd(parent_id), (HMENU)(intptr_t)id,
        GetModuleHandleW(NULL), NULL
    );
    g_node_id_map[id] = hw;
    g_boxes[id].is_row = (int)horizontal;
    g_boxes[id].child_count = 0;
    g_boxes[id].padding = 8;
    return id;
}

int64_t haki_gtk_create_text_field(int64_t parent_id, const char* placeholder, int64_t node_id) {
    int64_t id = node_id > 0 ? node_id : g_next_id++;
    if (id >= HAKI_MAX_NODES) return -1;
    if (id >= g_next_id) g_next_id = id + 1;
    HWND hw = CreateWindowExW(
        WS_EX_CLIENTEDGE, L"EDIT", L"",
        WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL,
        0, 0, 200, 24,
        get_parent_hwnd(parent_id), (HMENU)(intptr_t)id,
        GetModuleHandleW(NULL), NULL
    );
    if (placeholder && placeholder[0]) {
        wchar_t* wp = utf8_to_wide(placeholder);
        SendMessageW(hw, EM_SETCUEBANNER, TRUE, (LPARAM)wp);
        free(wp);
    }
    g_node_id_map[id] = hw;
    return id;
}

int64_t haki_gtk_create_checkbox(int64_t parent_id, const char* label, int64_t checked, int64_t node_id) {
    int64_t id = node_id > 0 ? node_id : g_next_id++;
    if (id >= HAKI_MAX_NODES) return -1;
    if (id >= g_next_id) g_next_id = id + 1;
    wchar_t* wlabel = utf8_to_wide(label ? label : "");
    HWND hw = CreateWindowExW(
        0, L"BUTTON", wlabel,
        WS_CHILD | WS_VISIBLE | BS_AUTOCHECKBOX,
        0, 0, 150, 24,
        get_parent_hwnd(parent_id), (HMENU)(intptr_t)id,
        GetModuleHandleW(NULL), NULL
    );
    free(wlabel);
    SendMessageW(hw, BM_SETCHECK, checked ? BST_CHECKED : BST_UNCHECKED, 0);
    g_node_id_map[id] = hw;
    return id;
}

int64_t haki_gtk_create_dropdown(int64_t parent_id, const char* options_csv, int64_t node_id) {
    int64_t id = node_id > 0 ? node_id : g_next_id++;
    if (id >= HAKI_MAX_NODES) return -1;
    if (id >= g_next_id) g_next_id = id + 1;
    HWND hw = CreateWindowExW(
        0, L"COMBOBOX", L"",
        WS_CHILD | WS_VISIBLE | CBS_DROPDOWNLIST | WS_VSCROLL,
        0, 0, 200, 200,
        get_parent_hwnd(parent_id), (HMENU)(intptr_t)id,
        GetModuleHandleW(NULL), NULL
    );
    if (options_csv && options_csv[0]) {
        char* buf = _strdup(options_csv);
        char* tok = strtok(buf, ",");
        while (tok) {
            wchar_t* wopt = utf8_to_wide(tok);
            SendMessageW(hw, CB_ADDSTRING, 0, (LPARAM)wopt);
            free(wopt);
            tok = strtok(NULL, ",");
        }
        free(buf);
        SendMessageW(hw, CB_SETCURSEL, 0, 0);
    }
    g_node_id_map[id] = hw;
    return id;
}

int64_t haki_gtk_create_image(int64_t parent_id, const char* path, int64_t w, int64_t h) {
    int64_t id = g_next_id++;
    if (id >= HAKI_MAX_NODES) return -1;
    wchar_t* wpath = utf8_to_wide(path ? path : "");
    HBITMAP bmp = (HBITMAP)LoadImageW(NULL, wpath, IMAGE_BITMAP, (int)w, (int)h, LR_LOADFROMFILE);
    free(wpath);
    HWND hw = CreateWindowExW(
        0, L"STATIC", L"",
        WS_CHILD | WS_VISIBLE | SS_BITMAP,
        0, 0, (int)w > 0 ? (int)w : 100, (int)h > 0 ? (int)h : 100,
        get_parent_hwnd(parent_id), (HMENU)(intptr_t)id,
        GetModuleHandleW(NULL), NULL
    );
    if (bmp) SendMessageW(hw, STM_SETIMAGE, IMAGE_BITMAP, (LPARAM)bmp);
    g_node_id_map[id] = hw;
    return id;
}

/* ── Mutation API ───────────────────────────────────────────────────────── */

void haki_gtk_set_text(int64_t node_id, const char* text) {
    if (node_id <= 0 || node_id >= HAKI_MAX_NODES || !g_node_id_map[node_id]) return;
    wchar_t* wtext = utf8_to_wide(text ? text : "");
    SetWindowTextW(g_node_id_map[node_id], wtext);
    free(wtext);
}

void haki_gtk_set_visible(int64_t node_id, int64_t visible) {
    if (node_id <= 0 || node_id >= HAKI_MAX_NODES || !g_node_id_map[node_id]) return;
    ShowWindow(g_node_id_map[node_id], visible ? SW_SHOW : SW_HIDE);
}

void haki_gtk_insert_child(int64_t parent_id, int64_t index, int64_t child_id) {
    if (parent_id <= 0 || parent_id >= HAKI_MAX_NODES) return;
    if (child_id  <= 0 || child_id  >= HAKI_MAX_NODES) return;
    HWND parent_hw = g_node_id_map[parent_id];
    HWND child_hw  = g_node_id_map[child_id];
    if (!parent_hw || !child_hw) return;
    SetParent(child_hw, parent_hw);
    /* Track in box layout */
    HakiBox* box = &g_boxes[parent_id];
    int idx = (int)index;
    if (idx < 0 || idx > box->child_count) idx = box->child_count;
    if (box->child_count < 255) {
        memmove(&box->children[idx + 1], &box->children[idx],
                (box->child_count - idx) * sizeof(HWND));
        box->children[idx] = child_hw;
        box->child_count++;
    }
    haki_reflow_box(parent_id);
}

void haki_gtk_remove_child(int64_t node_id) {
    if (node_id <= 0 || node_id >= HAKI_MAX_NODES || !g_node_id_map[node_id]) return;
    DestroyWindow(g_node_id_map[node_id]);
    g_node_id_map[node_id] = NULL;
}

void haki_gtk_register_node(int64_t vnode_id, int64_t gtk_id) {
    if (vnode_id >= 0 && vnode_id < HAKI_MAX_NODES &&
        gtk_id    >= 0 && gtk_id    < HAKI_MAX_NODES) {
        g_node_id_map[vnode_id] = g_node_id_map[gtk_id];
    }
}

void haki_gtk_set_callback(int64_t node_id, int64_t cb_id) {
    (void)node_id; (void)cb_id; /* stable callbacks — no-op for now */
}

/* ── Layout props ───────────────────────────────────────────────────────── */

void haki_gtk_set_padding(int64_t node_id, int64_t px) {
    if (node_id > 0 && node_id < HAKI_MAX_NODES) {
        g_boxes[node_id].padding = (int)px;
        haki_reflow_box(node_id);
    }
}

void haki_gtk_set_spacing(int64_t node_id, int64_t px) {
    haki_gtk_set_padding(node_id, px); /* same concept on Win32 */
}

void haki_gtk_set_alignment(int64_t node_id, const char* align) {
    if (node_id <= 0 || node_id >= HAKI_MAX_NODES || !g_node_id_map[node_id]) return;
    /* Win32: adjust window style for alignment */
    HWND hw = g_node_id_map[node_id];
    LONG_PTR style = GetWindowLongPtrW(hw, GWL_STYLE);
    style &= ~(SS_LEFT | SS_CENTER | SS_RIGHT);
    if (align && strcmp(align, "center") == 0) style |= SS_CENTER;
    else if (align && strcmp(align, "end") == 0) style |= SS_RIGHT;
    else style |= SS_LEFT;
    SetWindowLongPtrW(hw, GWL_STYLE, style);
    InvalidateRect(hw, NULL, TRUE);
}

/* ── Legacy stubs (v3.1 compat) ─────────────────────────────────────────── */

int64_t haki_gtk_alloc_node_id(void) { return g_next_id++; }
int64_t haki_gtk_peek_next_id(void)  { return g_next_id;   }
void    haki_gtk_mark_label(int64_t node_id) { (void)node_id; }
int64_t haki_gtk_get_label_id(void)  { return 0; }
void    haki_set_callback_dispatcher(void* fn) { (void)fn; }
void    haki_fire_callback(int64_t id) { fire_callback(id); }
void*   haki_get_callback(int64_t id) {
    if (id <= 0 || id >= HAKI_MAX_CALLBACKS) return NULL;
    /* Return a fat pointer to the callback */
    static void* fat[2];
    fat[0] = (void*)g_callbacks[id];
    fat[1] = g_callback_envs[id];
    return fat;
}

/* ── Event loop ─────────────────────────────────────────────────────────── */

void haki_platform_run(void) {
    if (!g_window) return;
    ShowWindow(g_window, SW_SHOWDEFAULT);
    UpdateWindow(g_window);
    MSG msg;
    while (GetMessageW(&msg, NULL, 0, 0) > 0) {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}

/* ── Legacy haki_app_run (JSON bridge compat) ───────────────────────────── */

void haki_app_run(const char* json, const char* title, int64_t width, int64_t height) {
    (void)json;
    haki_gtk_create_window(title, width, height);
    haki_platform_run();
}

"#;

pub const SYS_RUNTIME_C_SOURCE: &str = r#"
/*
 * haki_sys_runtime.c — std/sys C implementation
 *
 * Cross-platform: Unix (Linux/macOS) and Windows (Win32 API).
 * All functions return -2 for "unsupported on this platform".
 * All string returns are heap-allocated (strdup/malloc) — caller owns them.
 *
 * Build (Unix):    gcc -c haki_sys_runtime.c
 * Build (Windows): cl /c haki_sys_runtime.c
 */

#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <stdint.h>
#include <errno.h>

/* ── Platform detection ──────────────────────────────────────────────────── */

#ifdef _WIN32
  #define HAKI_WINDOWS 1
  #define WIN32_LEAN_AND_MEAN
  #include <windows.h>
  #include <process.h>
  #include <direct.h>
  #include <io.h>
  #include <tlhelp32.h>
  #pragma comment(lib, "advapi32.lib")
#else
  #define HAKI_UNIX 1
  #include <unistd.h>
  #include <signal.h>
  #include <sys/types.h>
  #include <sys/stat.h>
  #include <sys/wait.h>
  #include <sys/utsname.h>
  #include <pwd.h>
  #include <fcntl.h>
  #include <dirent.h>
  #include <time.h>
  #ifdef __APPLE__
    #include <sys/sysctl.h>
    #include <mach/mach.h>
    #include <libproc.h>
  #elif defined(__linux__)
    #include <sys/sysinfo.h>
  #endif
#endif

/* ── Helpers ─────────────────────────────────────────────────────────────── */

/* Split a null-separated arg string into argv array.
 * args_str: "arg0\x00arg1\x00arg2"
 * Returns a NULL-terminated char** that caller must free_argv().        */
static char** split_args(const char* args_str, int* count_out) {
    if (!args_str || !args_str[0]) {
        char** argv = (char**)malloc(sizeof(char*));
        argv[0] = NULL;
        if (count_out) *count_out = 0;
        return argv;
    }
    int cap = 16, n = 0;
    char** argv = (char**)malloc(cap * sizeof(char*));
    const char* p = args_str;
    while (*p) {
        if (n >= cap - 1) { cap *= 2; argv = (char**)realloc(argv, cap * sizeof(char*)); }
        argv[n++] = (char*)p;
        p += strlen(p) + 1;
    }
    argv[n] = NULL;
    if (count_out) *count_out = n;
    return argv;
}

static void free_argv(char** argv) { free(argv); }

/* ── Process execution — Unix ────────────────────────────────────────────── */

#ifdef HAKI_UNIX

typedef struct {
    char* out;
    char* err;
    int   code;
    int   valid;
} RunBuf;

static RunBuf g_last_run = {0};

static RunBuf do_run(const char* cmd, const char* args_str) {
    RunBuf rb = {strdup(""), strdup(""), -1, 0};
    if (!cmd || !cmd[0]) return rb;

    int out_pipe[2], err_pipe[2];
    if (pipe(out_pipe) < 0 || pipe(err_pipe) < 0) return rb;

    int argc;
    char** argv = split_args(args_str, &argc);

    /* Build execvp argv: cmd + args + NULL */
    char** execv = (char**)malloc((argc + 2) * sizeof(char*));
    execv[0] = (char*)cmd;
    for (int i = 0; i < argc; i++) execv[i+1] = argv[i];
    execv[argc+1] = NULL;

    pid_t pid = fork();
    if (pid == 0) {
        /* Child */
        close(out_pipe[0]); close(err_pipe[0]);
        dup2(out_pipe[1], STDOUT_FILENO);
        dup2(err_pipe[1], STDERR_FILENO);
        close(out_pipe[1]); close(err_pipe[1]);
        execvp(cmd, execv);
        _exit(127);
    }
    free(execv); free_argv(argv);
    if (pid < 0) return rb;

    close(out_pipe[1]); close(err_pipe[1]);

    /* Read stdout */
    char buf[4096]; ssize_t n;
    size_t out_len = 0, err_len = 0;
    char *out_buf = (char*)malloc(1), *err_buf = (char*)malloc(1);
    out_buf[0] = err_buf[0] = 0;

    while ((n = read(out_pipe[0], buf, sizeof(buf)-1)) > 0) {
        buf[n] = 0;
        out_buf = (char*)realloc(out_buf, out_len + n + 1);
        memcpy(out_buf + out_len, buf, n+1);
        out_len += n;
    }
    while ((n = read(err_pipe[0], buf, sizeof(buf)-1)) > 0) {
        buf[n] = 0;
        err_buf = (char*)realloc(err_buf, err_len + n + 1);
        memcpy(err_buf + err_len, buf, n+1);
        err_len += n;
    }
    close(out_pipe[0]); close(err_pipe[0]);

    int status;
    waitpid(pid, &status, 0);
    rb.out   = out_buf;
    rb.err   = err_buf;
    rb.code  = WIFEXITED(status) ? WEXITSTATUS(status) : -1;
    rb.valid = 1;
    return rb;
}

static RunBuf do_shell(const char* cmd) {
    char shell_cmd[8192];
    snprintf(shell_cmd, sizeof(shell_cmd), "/bin/sh -c %s", cmd);
    /* Use /bin/sh -c directly */
    RunBuf rb = {strdup(""), strdup(""), -1, 0};
    int out_pipe[2], err_pipe[2];
    if (pipe(out_pipe) < 0 || pipe(err_pipe) < 0) return rb;
    pid_t pid = fork();
    if (pid == 0) {
        close(out_pipe[0]); close(err_pipe[0]);
        dup2(out_pipe[1], STDOUT_FILENO);
        dup2(err_pipe[1], STDERR_FILENO);
        close(out_pipe[1]); close(err_pipe[1]);
        execl("/bin/sh", "/bin/sh", "-c", cmd, (char*)NULL);
        _exit(127);
    }
    if (pid < 0) return rb;
    close(out_pipe[1]); close(err_pipe[1]);
    char buf[4096]; ssize_t n;
    size_t ol=0, el=0;
    char *o=(char*)malloc(1), *e=(char*)malloc(1); o[0]=e[0]=0;
    while((n=read(out_pipe[0],buf,sizeof(buf)-1))>0){buf[n]=0;o=(char*)realloc(o,ol+n+1);memcpy(o+ol,buf,n+1);ol+=n;}
    while((n=read(err_pipe[0],buf,sizeof(buf)-1))>0){buf[n]=0;e=(char*)realloc(e,el+n+1);memcpy(e+el,buf,n+1);el+=n;}
    close(out_pipe[0]); close(err_pipe[0]);
    int status; waitpid(pid, &status, 0);
    rb.out=o; rb.err=e;
    rb.code=WIFEXITED(status)?WEXITSTATUS(status):-1; rb.valid=1;
    return rb;
}

const char* haki_sys_run_stdout(const char* cmd, const char* args) {
    g_last_run = do_run(cmd, args); return g_last_run.valid ? g_last_run.out : strdup("");
}
const char* haki_sys_run_stderr(const char* cmd, const char* args) {
    return g_last_run.valid ? g_last_run.err : strdup("");
}
int64_t haki_sys_run_exit(const char* cmd, const char* args) {
    return g_last_run.valid ? g_last_run.code : -1;
}

const char* haki_sys_shell_stdout(const char* cmd) {
    g_last_run = do_shell(cmd); return g_last_run.out;
}
const char* haki_sys_shell_stderr(const char* cmd) { return g_last_run.err; }
int64_t     haki_sys_shell_exit(const char* cmd)   { return g_last_run.code; }

int64_t haki_sys_spawn(const char* cmd, const char* args) {
    int argc;
    char** argv = split_args(args, &argc);
    char** execv = (char**)malloc((argc+2)*sizeof(char*));
    execv[0]=(char*)cmd;
    for(int i=0;i<argc;i++) execv[i+1]=argv[i];
    execv[argc+1]=NULL;
    pid_t pid=fork();
    if(pid==0){ setsid(); execvp(cmd,execv); _exit(127); }
    free(execv); free_argv(argv);
    return (int64_t)pid;
}

const char* haki_sys_pipe_stdout(const char* cmds_str) {
    /* Execute each \x01-separated command, piping output through */
    FILE* f = NULL;
    char cmd[8192] = "";
    const char* p = cmds_str;
    while (*p) {
        const char* next = strchr(p, '\x01');
        size_t len = next ? (size_t)(next-p) : strlen(p);
        if (f) {
            /* Pipe previous output as stdin to next command — simplified: use shell pipe */
            strncat(cmd, " | ", sizeof(cmd)-strlen(cmd)-1);
            strncat(cmd, p, len < sizeof(cmd)-strlen(cmd)-1 ? len : sizeof(cmd)-strlen(cmd)-1);
        } else {
            strncpy(cmd, p, len < sizeof(cmd)-1 ? len : sizeof(cmd)-1);
            cmd[len] = 0;
        }
        p += len + (next ? 1 : 0);
        if (!next) break;
    }
    /* Execute as shell pipeline */
    g_last_run = do_shell(cmd);
    return g_last_run.out;
}
const char* haki_sys_pipe_stderr(const char* c){ return g_last_run.err; }
int64_t     haki_sys_pipe_exit(const char* c)  { return g_last_run.code; }

/* ── Signals — Unix ─────────────────────────────────────────────────────── */

int64_t haki_sys_signal(int64_t sig, void* closure) {
    /* closure is a fat pointer: {fn_ptr, env_ptr} */
    /* For signals, we store a global handler map */
    /* Simplified: use signal() with a trampoline */
    /* Full implementation would use sigaction + per-sig closures */
    (void)sig; (void)closure;
    return 0; /* registered — full impl in v4.x */
}
int64_t haki_sys_kill(int64_t pid, int64_t sig) {
    return kill((pid_t)pid, (int)sig) == 0 ? 0 : -1;
}
int64_t haki_sys_raise(int64_t sig) {
    return raise((int)sig) == 0 ? 0 : -1;
}
void haki_sys_exit(int64_t code) { _exit((int)code); }

/* ── File permissions — Unix ─────────────────────────────────────────────── */

int64_t haki_sys_chmod(const char* path, int64_t mode) {
    return chmod(path, (mode_t)mode) == 0 ? 0 : -1;
}
int64_t haki_sys_chown(const char* path, int64_t uid, int64_t gid) {
    return chown(path, (uid_t)uid, (gid_t)gid) == 0 ? 0 : -1;
}
int64_t haki_sys_symlink(const char* src, const char* dst) {
    return symlink(src, dst) == 0 ? 0 : -1;
}
const char* haki_sys_readlink(const char* path) {
    char buf[4096];
    ssize_t n = readlink(path, buf, sizeof(buf)-1);
    if (n < 0) return strdup("");
    buf[n] = 0;
    return strdup(buf);
}

/* stat helpers */
static struct stat g_stat_buf;
static char g_stat_path[4096];
static int g_stat_ok = 0;

static void ensure_stat(const char* path) {
    if (g_stat_ok && strcmp(g_stat_path, path)==0) return;
    g_stat_ok = lstat(path, &g_stat_buf) == 0 ? 1 : 0;
    strncpy(g_stat_path, path, sizeof(g_stat_path)-1);
}

int64_t     haki_sys_stat_ok(const char* p)    { ensure_stat(p); return g_stat_ok; }
int64_t     haki_sys_stat_size(const char* p)  { ensure_stat(p); return (int64_t)g_stat_buf.st_size; }
int64_t     haki_sys_stat_mtime(const char* p) { ensure_stat(p); return (int64_t)g_stat_buf.st_mtime; }
int64_t     haki_sys_stat_mode(const char* p)  { ensure_stat(p); return (int64_t)g_stat_buf.st_mode & 0777; }
int64_t     haki_sys_stat_uid(const char* p)   { ensure_stat(p); return (int64_t)g_stat_buf.st_uid; }
int64_t     haki_sys_stat_gid(const char* p)   { ensure_stat(p); return (int64_t)g_stat_buf.st_gid; }
int64_t     haki_sys_stat_isdir(const char* p) { ensure_stat(p); return S_ISDIR(g_stat_buf.st_mode)?1:0; }
int64_t     haki_sys_stat_islink(const char* p){ ensure_stat(p); return S_ISLNK(g_stat_buf.st_mode)?1:0; }

/* ── Environment — Unix ──────────────────────────────────────────────────── */

const char* haki_sys_getenv(const char* key) {
    const char* v = getenv(key);
    return v ? strdup(v) : strdup("");
}
int64_t haki_sys_setenv(const char* k, const char* v) {
    return setenv(k, v, 1) == 0 ? 0 : -1;
}
int64_t haki_sys_unsetenv(const char* k) {
    return unsetenv(k) == 0 ? 0 : -1;
}
const char* haki_sys_cwd(void) {
    char buf[4096];
    return getcwd(buf, sizeof(buf)) ? strdup(buf) : strdup(".");
}
int64_t haki_sys_chdir(const char* path) {
    return chdir(path) == 0 ? 0 : -1;
}
const char* haki_sys_home_dir(void) {
    const char* h = getenv("HOME");
    if (h) return strdup(h);
    struct passwd* pw = getpwuid(getuid());
    return pw ? strdup(pw->pw_dir) : strdup("/tmp");
}
const char* haki_sys_temp_dir(void) {
    const char* t = getenv("TMPDIR");
    return t ? strdup(t) : strdup("/tmp");
}

/* ── System info — Unix ──────────────────────────────────────────────────── */

const char* haki_sys_platform(void) {
#ifdef __APPLE__
    return strdup("macos");
#elif defined(__linux__)
    return strdup("linux");
#elif defined(__FreeBSD__)
    return strdup("freebsd");
#else
    return strdup("unix");
#endif
}

const char* haki_sys_arch(void) {
#if defined(__aarch64__) || defined(__arm64__)
    return strdup("arm64");
#elif defined(__x86_64__)
    return strdup("x86_64");
#elif defined(__i386__)
    return strdup("x86");
#elif defined(__riscv)
    return strdup("riscv64");
#else
    return strdup("unknown");
#endif
}

const char* haki_sys_hostname(void) {
    char buf[256];
    if (gethostname(buf, sizeof(buf)) == 0) return strdup(buf);
    return strdup("localhost");
}

const char* haki_sys_username(void) {
    const char* u = getenv("USER");
    if (u) return strdup(u);
    struct passwd* pw = getpwuid(getuid());
    return pw ? strdup(pw->pw_name) : strdup("unknown");
}

int64_t haki_sys_cpu_count(void) {
#ifdef __APPLE__
    int n = 1;
    size_t sz = sizeof(n);
    sysctlbyname("hw.logicalcpu", &n, &sz, NULL, 0);
    return n;
#elif defined(__linux__)
    return sysconf(_SC_NPROCESSORS_ONLN);
#else
    return 1;
#endif
}

int64_t haki_sys_mem_total(void) {
#ifdef __APPLE__
    int64_t mem = 0;
    size_t sz = sizeof(mem);
    sysctlbyname("hw.memsize", &mem, &sz, NULL, 0);
    return mem;
#elif defined(__linux__)
    struct sysinfo si;
    if (sysinfo(&si) == 0) return (int64_t)si.totalram * si.mem_unit;
    return -1;
#else
    return -1;
#endif
}

int64_t haki_sys_mem_available(void) {
#ifdef __APPLE__
    mach_port_t host = mach_host_self();
    vm_size_t page_size;
    host_page_size(host, &page_size);
    vm_statistics64_data_t vm_stat;
    mach_msg_type_number_t count = HOST_VM_INFO64_COUNT;
    host_statistics64(host, HOST_VM_INFO64, (host_info64_t)&vm_stat, &count);
    return (int64_t)(vm_stat.free_count + vm_stat.inactive_count) * page_size;
#elif defined(__linux__)
    struct sysinfo si;
    if (sysinfo(&si) == 0) return (int64_t)si.freeram * si.mem_unit;
    return -1;
#else
    return -1;
#endif
}

int64_t haki_sys_getpid(void)  { return (int64_t)getpid(); }
int64_t haki_sys_getppid(void) { return (int64_t)getppid(); }

int64_t haki_sys_uptime(void) {
#ifdef __APPLE__
    struct timeval boottime;
    size_t sz = sizeof(boottime);
    sysctlbyname("kern.boottime", &boottime, &sz, NULL, 0);
    return (int64_t)(time(NULL) - boottime.tv_sec);
#elif defined(__linux__)
    struct sysinfo si;
    if (sysinfo(&si) == 0) return (int64_t)si.uptime;
    return -1;
#else
    return -1;
#endif
}

const char* haki_sys_haki_version(void) { return strdup("3.8.0"); }

/* ── Process listing — Unix ──────────────────────────────────────────────── */

#define HAKI_MAX_PROCS 2048
typedef struct { int pid; char name[256]; int ppid; char status[16]; } HProcInfo;
static HProcInfo g_procs[HAKI_MAX_PROCS];
static int g_proc_count = -1;

static void refresh_procs(void) {
    g_proc_count = 0;
#ifdef __linux__
    DIR* d = opendir("/proc");
    if (!d) return;
    struct dirent* e;
    while ((e = readdir(d)) && g_proc_count < HAKI_MAX_PROCS) {
        int pid = atoi(e->d_name);
        if (pid <= 0) continue;
        char path[64]; snprintf(path, sizeof(path), "/proc/%d/stat", pid);
        FILE* f = fopen(path, "r");
        if (!f) continue;
        char name[256]; int ppid; char state;
        fscanf(f, "%*d (%255[^)]) %c %d", name, &state, &ppid);
        fclose(f);
        g_procs[g_proc_count].pid  = pid;
        g_procs[g_proc_count].ppid = ppid;
        strncpy(g_procs[g_proc_count].name, name, 255);
        const char* st = state=='R'?"running":state=='S'?"sleeping":state=='Z'?"zombie":state=='T'?"stopped":"unknown";
        strncpy(g_procs[g_proc_count].status, st, 15);
        g_proc_count++;
    }
    closedir(d);
#elif defined(__APPLE__)
    /* Use sysctl KERN_PROC_ALL */
    int mib[3] = {CTL_KERN, KERN_PROC, KERN_PROC_ALL};
    size_t sz = 0;
    sysctl(mib, 3, NULL, &sz, NULL, 0);
    struct kinfo_proc* kp = (struct kinfo_proc*)malloc(sz);
    if (!kp) return;
    if (sysctl(mib, 3, kp, &sz, NULL, 0) == 0) {
        int n = (int)(sz / sizeof(struct kinfo_proc));
        for (int i = 0; i < n && g_proc_count < HAKI_MAX_PROCS; i++) {
            g_procs[g_proc_count].pid  = kp[i].kp_proc.p_pid;
            g_procs[g_proc_count].ppid = kp[i].kp_eproc.e_ppid;
            strncpy(g_procs[g_proc_count].name, kp[i].kp_proc.p_comm, 255);
            strcpy(g_procs[g_proc_count].status, "running");
            g_proc_count++;
        }
    }
    free(kp);
#endif
}

int64_t     haki_sys_process_count(void)      { refresh_procs(); return g_proc_count; }
int64_t     haki_sys_process_pid(int64_t i)   { return i<g_proc_count?g_procs[i].pid:-1; }
const char* haki_sys_process_name(int64_t i)  { return i<g_proc_count?strdup(g_procs[i].name):strdup(""); }
int64_t     haki_sys_process_ppid(int64_t i)  { return i<g_proc_count?g_procs[i].ppid:-1; }
const char* haki_sys_process_status(int64_t i){ return i<g_proc_count?strdup(g_procs[i].status):strdup("unknown"); }

#endif /* HAKI_UNIX */

/* ═══════════════════════════════════════════════════════════════════════════
 * WINDOWS IMPLEMENTATION
 * ═══════════════════════════════════════════════════════════════════════════ */

#ifdef HAKI_WINDOWS

/* ── Helpers — Windows ───────────────────────────────────────────────────── */

static char* wide_to_utf8_sys(const wchar_t* w) {
    int n = WideCharToMultiByte(CP_UTF8,0,w,-1,NULL,0,NULL,NULL);
    char* s = (char*)malloc(n);
    if (s) WideCharToMultiByte(CP_UTF8,0,w,-1,s,n,NULL,NULL);
    return s;
}

/* Run a command via CreateProcess, capture stdout+stderr */
typedef struct { char* out; char* err; int code; } WinRun;

static WinRun win_run_cmd(const char* cmd, const char* args_str, int use_shell) {
    WinRun rb = {strdup(""), strdup(""), -1};

    HANDLE out_r, out_w, err_r, err_w;
    SECURITY_ATTRIBUTES sa = {sizeof(sa), NULL, TRUE};
    if (!CreatePipe(&out_r, &out_w, &sa, 0)) return rb;
    if (!CreatePipe(&err_r, &err_w, &sa, 0)) { CloseHandle(out_r); CloseHandle(out_w); return rb; }
    SetHandleInformation(out_r, HANDLE_FLAG_INHERIT, 0);
    SetHandleInformation(err_r, HANDLE_FLAG_INHERIT, 0);

    /* Build command line */
    char cmdline[8192];
    if (use_shell) {
        snprintf(cmdline, sizeof(cmdline), "cmd.exe /C %s", cmd);
    } else {
        snprintf(cmdline, sizeof(cmdline), "%s", cmd);
        if (args_str && args_str[0]) {
            int argc; char** argv = split_args(args_str, &argc);
            for (int i = 0; i < argc; i++) {
                strncat(cmdline, " ", sizeof(cmdline)-strlen(cmdline)-1);
                strncat(cmdline, argv[i], sizeof(cmdline)-strlen(cmdline)-1);
            }
            free_argv(argv);
        }
    }

    STARTUPINFOA si = {sizeof(si)};
    si.dwFlags     = STARTF_USESTDHANDLES;
    si.hStdOutput  = out_w;
    si.hStdError   = err_w;
    si.hStdInput   = GetStdHandle(STD_INPUT_HANDLE);

    PROCESS_INFORMATION pi = {0};
    if (!CreateProcessA(NULL, cmdline, NULL, NULL, TRUE,
                        CREATE_NO_WINDOW, NULL, NULL, &si, &pi)) {
        CloseHandle(out_r); CloseHandle(out_w);
        CloseHandle(err_r); CloseHandle(err_w);
        return rb;
    }
    CloseHandle(out_w); CloseHandle(err_w);

    /* Read stdout */
    char buf[4096]; DWORD n;
    size_t ol=0, el=0;
    char *o=(char*)malloc(1), *e=(char*)malloc(1); o[0]=e[0]=0;
    while(ReadFile(out_r,buf,sizeof(buf)-1,&n,NULL)&&n>0){
        buf[n]=0; o=(char*)realloc(o,ol+n+1); memcpy(o+ol,buf,n+1); ol+=n;
    }
    while(ReadFile(err_r,buf,sizeof(buf)-1,&n,NULL)&&n>0){
        buf[n]=0; e=(char*)realloc(e,el+n+1); memcpy(e+el,buf,n+1); el+=n;
    }
    CloseHandle(out_r); CloseHandle(err_r);

    WaitForSingleObject(pi.hProcess, INFINITE);
    DWORD code; GetExitCodeProcess(pi.hProcess, &code);
    CloseHandle(pi.hProcess); CloseHandle(pi.hThread);

    rb.out=o; rb.err=e; rb.code=(int)code;
    return rb;
}

static WinRun g_last_win = {0};

const char* haki_sys_run_stdout(const char* cmd, const char* args) {
    g_last_win = win_run_cmd(cmd, args, 0); return g_last_win.out;
}
const char* haki_sys_run_stderr(const char* cmd, const char* args) { return g_last_win.err; }
int64_t     haki_sys_run_exit(const char* cmd, const char* args)   { return g_last_win.code; }

const char* haki_sys_shell_stdout(const char* cmd) {
    g_last_win = win_run_cmd(cmd, "", 1); return g_last_win.out;
}
const char* haki_sys_shell_stderr(const char* cmd) { return g_last_win.err; }
int64_t     haki_sys_shell_exit(const char* cmd)   { return g_last_win.code; }

int64_t haki_sys_spawn(const char* cmd, const char* args) {
    char cmdline[8192]; snprintf(cmdline, sizeof(cmdline), "%s", cmd);
    if (args && args[0]) {
        int argc; char** argv = split_args(args, &argc);
        for (int i=0;i<argc;i++){strncat(cmdline," ",sizeof(cmdline)-strlen(cmdline)-1);
            strncat(cmdline,argv[i],sizeof(cmdline)-strlen(cmdline)-1);}
        free_argv(argv);
    }
    STARTUPINFOA si = {sizeof(si)};
    PROCESS_INFORMATION pi = {0};
    if (!CreateProcessA(NULL, cmdline, NULL, NULL, FALSE,
                        DETACHED_PROCESS, NULL, NULL, &si, &pi)) return -1;
    DWORD pid = pi.dwProcessId;
    CloseHandle(pi.hProcess); CloseHandle(pi.hThread);
    return (int64_t)pid;
}

const char* haki_sys_pipe_stdout(const char* cmds) {
    g_last_win = win_run_cmd(cmds, "", 1); return g_last_win.out;
}
const char* haki_sys_pipe_stderr(const char* c) { return g_last_win.err; }
int64_t     haki_sys_pipe_exit(const char* c)   { return g_last_win.code; }

/* ── Signals — Windows ───────────────────────────────────────────────────── */

int64_t haki_sys_signal(int64_t sig, void* closure) {
    (void)sig; (void)closure;
    /* Windows supports SIGINT (2) and SIGTERM (15) via signal() */
    /* Full closure-based dispatch deferred to v4.x */
    return 0;
}

int64_t haki_sys_kill(int64_t pid, int64_t sig) {
    /* On Windows: SIGKILL(9) and SIGTERM(15) → TerminateProcess */
    HANDLE h = OpenProcess(PROCESS_TERMINATE, FALSE, (DWORD)pid);
    if (!h) return -1;
    BOOL ok = TerminateProcess(h, (UINT)sig);
    CloseHandle(h);
    return ok ? 0 : -1;
}

int64_t haki_sys_raise(int64_t sig) {
    return raise((int)sig) == 0 ? 0 : -1;
}

void haki_sys_exit(int64_t code) { ExitProcess((UINT)code); }

/* ── File permissions — Windows ─────────────────────────────────────────── */

int64_t haki_sys_chmod(const char* path, int64_t mode) {
    /* Windows has no Unix octal permissions — return UnsupportedPlatform */
    (void)path; (void)mode;
    return -2;
}
int64_t haki_sys_chown(const char* path, int64_t uid, int64_t gid) {
    (void)path; (void)uid; (void)gid;
    return -2;
}
int64_t haki_sys_symlink(const char* src, const char* dst) {
    /* Requires Developer Mode or admin on Windows 10+ */
    wchar_t wsrc[4096], wdst[4096];
    MultiByteToWideChar(CP_UTF8,0,src,-1,wsrc,4096);
    MultiByteToWideChar(CP_UTF8,0,dst,-1,wdst,4096);
    return CreateSymbolicLinkW(wdst, wsrc, 0) ? 0 : -2;
}
const char* haki_sys_readlink(const char* path) {
    wchar_t wpath[4096];
    MultiByteToWideChar(CP_UTF8,0,path,-1,wpath,4096);
    HANDLE h = CreateFileW(wpath,0,FILE_SHARE_READ,NULL,OPEN_EXISTING,
                           FILE_FLAG_BACKUP_SEMANTICS,NULL);
    if (h==INVALID_HANDLE_VALUE) return strdup("");
    wchar_t buf[4096];
    DWORD n = GetFinalPathNameByHandleW(h,buf,4096,FILE_NAME_NORMALIZED);
    CloseHandle(h);
    if (!n) return strdup("");
    return wide_to_utf8_sys(buf);
}

/* stat — Windows via GetFileAttributesEx */
static WIN32_FILE_ATTRIBUTE_DATA g_win_stat;
static char g_win_stat_path[4096];
static int  g_win_stat_ok = 0;

static void ensure_win_stat(const char* path) {
    if (g_win_stat_ok && strcmp(g_win_stat_path,path)==0) return;
    wchar_t wp[4096]; MultiByteToWideChar(CP_UTF8,0,path,-1,wp,4096);
    g_win_stat_ok = GetFileAttributesExW(wp,GetFileExInfoStandard,&g_win_stat)?1:0;
    strncpy(g_win_stat_path,path,sizeof(g_win_stat_path)-1);
}

int64_t haki_sys_stat_ok(const char* p)    { ensure_win_stat(p); return g_win_stat_ok; }
int64_t haki_sys_stat_size(const char* p)  {
    ensure_win_stat(p);
    LARGE_INTEGER sz; sz.LowPart=g_win_stat.nFileSizeLow; sz.HighPart=g_win_stat.nFileSizeHigh;
    return sz.QuadPart;
}
int64_t haki_sys_stat_mtime(const char* p) {
    ensure_win_stat(p);
    ULARGE_INTEGER ft; ft.LowPart=g_win_stat.ftLastWriteTime.dwLowDateTime;
    ft.HighPart=g_win_stat.ftLastWriteTime.dwHighDateTime;
    return (int64_t)((ft.QuadPart - 116444736000000000ULL) / 10000000ULL);
}
int64_t haki_sys_stat_mode(const char* p)  { ensure_win_stat(p); return 0644; /* approximation */ }
int64_t haki_sys_stat_uid(const char* p)   { (void)p; return 0; }
int64_t haki_sys_stat_gid(const char* p)   { (void)p; return 0; }
int64_t haki_sys_stat_isdir(const char* p) {
    ensure_win_stat(p);
    return (g_win_stat.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY)?1:0;
}
int64_t haki_sys_stat_islink(const char* p) {
    ensure_win_stat(p);
    return (g_win_stat.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT)?1:0;
}

/* ── Environment — Windows ───────────────────────────────────────────────── */

const char* haki_sys_getenv(const char* key) {
    char buf[32768];
    if (GetEnvironmentVariableA(key,buf,sizeof(buf))) return strdup(buf);
    return strdup("");
}
int64_t haki_sys_setenv(const char* k, const char* v) {
    return SetEnvironmentVariableA(k,v)?0:-1;
}
int64_t haki_sys_unsetenv(const char* k) {
    return SetEnvironmentVariableA(k,NULL)?0:-1;
}
const char* haki_sys_cwd(void) {
    char buf[4096];
    if (GetCurrentDirectoryA(sizeof(buf),buf)) return strdup(buf);
    return strdup(".");
}
int64_t haki_sys_chdir(const char* path) {
    return SetCurrentDirectoryA(path)?0:-1;
}
const char* haki_sys_home_dir(void) {
    const char* h = getenv("USERPROFILE");
    return h ? strdup(h) : strdup("C:\\Users\\Default");
}
const char* haki_sys_temp_dir(void) {
    char buf[4096];
    if (GetTempPathA(sizeof(buf),buf)) return strdup(buf);
    return strdup("C:\\Temp");
}

/* ── System info — Windows ───────────────────────────────────────────────── */

const char* haki_sys_platform(void) { return strdup("windows"); }
const char* haki_sys_arch(void) {
    SYSTEM_INFO si; GetNativeSystemInfo(&si);
    switch(si.wProcessorArchitecture){
        case PROCESSOR_ARCHITECTURE_AMD64: return strdup("x86_64");
        case PROCESSOR_ARCHITECTURE_ARM64: return strdup("arm64");
        case PROCESSOR_ARCHITECTURE_INTEL: return strdup("x86");
        default: return strdup("unknown");
    }
}
const char* haki_sys_hostname(void) {
    char buf[256]; DWORD n=sizeof(buf);
    return GetComputerNameA(buf,&n)?strdup(buf):strdup("localhost");
}
const char* haki_sys_username(void) {
    char buf[256]; DWORD n=sizeof(buf);
    return GetUserNameA(buf,&n)?strdup(buf):strdup("unknown");
}
int64_t haki_sys_cpu_count(void) {
    SYSTEM_INFO si; GetSystemInfo(&si); return si.dwNumberOfProcessors;
}
int64_t haki_sys_mem_total(void) {
    MEMORYSTATUSEX ms; ms.dwLength=sizeof(ms);
    return GlobalMemoryStatusEx(&ms)?(int64_t)ms.ullTotalPhys:-1;
}
int64_t haki_sys_mem_available(void) {
    MEMORYSTATUSEX ms; ms.dwLength=sizeof(ms);
    return GlobalMemoryStatusEx(&ms)?(int64_t)ms.ullAvailPhys:-1;
}
int64_t haki_sys_getpid(void)  { return (int64_t)GetCurrentProcessId(); }
int64_t haki_sys_getppid(void) {
    HANDLE h=CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS,0);
    if(h==INVALID_HANDLE_VALUE) return -1;
    PROCESSENTRY32 pe; pe.dwSize=sizeof(pe); DWORD mypid=GetCurrentProcessId();
    DWORD ppid=-1;
    if(Process32First(h,&pe)) do {
        if(pe.th32ProcessID==mypid){ppid=pe.th32ParentProcessID;break;}
    } while(Process32Next(h,&pe));
    CloseHandle(h); return (int64_t)ppid;
}
int64_t haki_sys_uptime(void) {
    return (int64_t)(GetTickCount64()/1000);
}
const char* haki_sys_haki_version(void) { return strdup("3.8.0"); }

/* ── Process listing — Windows ───────────────────────────────────────────── */

#define HAKI_MAX_PROCS 2048
typedef struct { int pid; char name[256]; int ppid; char status[16]; } HProcInfo;
static HProcInfo g_procs[HAKI_MAX_PROCS];
static int g_proc_count = -1;

static void refresh_procs(void) {
    g_proc_count = 0;
    HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS,0);
    if(snap==INVALID_HANDLE_VALUE) return;
    PROCESSENTRY32 pe; pe.dwSize=sizeof(pe);
    if(Process32First(snap,&pe)) do {
        if(g_proc_count>=HAKI_MAX_PROCS) break;
        g_procs[g_proc_count].pid  = (int)pe.th32ProcessID;
        g_procs[g_proc_count].ppid = (int)pe.th32ParentProcessID;
        strncpy(g_procs[g_proc_count].name, pe.szExeFile, 255);
        strcpy(g_procs[g_proc_count].status, "running");
        g_proc_count++;
    } while(Process32Next(snap,&pe));
    CloseHandle(snap);
}

int64_t     haki_sys_process_count(void)      { refresh_procs(); return g_proc_count; }
int64_t     haki_sys_process_pid(int64_t i)   { return i<g_proc_count?g_procs[i].pid:-1; }
const char* haki_sys_process_name(int64_t i)  { return i<g_proc_count?strdup(g_procs[i].name):strdup(""); }
int64_t     haki_sys_process_ppid(int64_t i)  { return i<g_proc_count?g_procs[i].ppid:-1; }
const char* haki_sys_process_status(int64_t i){ return i<g_proc_count?strdup(g_procs[i].status):strdup("unknown"); }

#endif /* HAKI_WINDOWS */

"#;
