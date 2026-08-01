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
    void* v = malloc((size_t)m->val_size);
    if (!k || !v) abort();
    memcpy(v, val_ptr, (size_t)m->val_size);
    haki_map_insert_entry(m, k, v);
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

int haki_select(int n, HakiChan** chans, int* ops, void** vals) {
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

        /* Unlock all and wait for a signal */
        pthread_mutex_lock(&sel_mu);
        for (int i = n-1; i >= 0; i--)
            pthread_mutex_unlock(&chans[order[i]]->mu);
        pthread_cond_wait(&sel_cond, &sel_mu);
        pthread_mutex_unlock(&sel_mu);

        /* Remove waiters before re-locking channels */
        for (int i = 0; i < n; i++)
            pthread_mutex_lock(&chans[order[i]]->mu);
        for (int i = 0; i < n; i++)
            haki_chan_remove_waiter(chans[i], &waiters[i]);
        for (int i = n-1; i >= 0; i--)
            pthread_mutex_unlock(&chans[order[i]]->mu);
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
    void* v = malloc((size_t)m->val_size);
    if (!k || !v) abort();
    memcpy(v, val_ptr, (size_t)m->val_size);
    haki_map_insert_entry(m, k, v);
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

int haki_select(int n, HakiChan** chans, int* ops, void** vals) {
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

        /* Unlock all and wait for a signal */
        pthread_mutex_lock(&sel_mu);
        for (int i = n-1; i >= 0; i--)
            pthread_mutex_unlock(&chans[order[i]]->mu);
        pthread_cond_wait(&sel_cond, &sel_mu);
        pthread_mutex_unlock(&sel_mu);

        /* Remove waiters before re-locking channels */
        for (int i = 0; i < n; i++)
            pthread_mutex_lock(&chans[order[i]]->mu);
        for (int i = 0; i < n; i++)
            haki_chan_remove_waiter(chans[i], &waiters[i]);
        for (int i = n-1; i >= 0; i--)
            pthread_mutex_unlock(&chans[order[i]]->mu);
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

/* ── HTTP Server (via libmicrohttpd) ─────────────────────────────
   haki_http_server_new(port, handler) starts a server that calls
   handler(HakiHttpRequest*) → HakiHttpResponse* for each request.
   haki_http_server_listen(server) blocks the calling thread.      */

#include <microhttpd.h>
#include <stdarg.h>

typedef struct {
    const char*             method;
    const char*             path;
    const char*             body;
    size_t                  body_len;
    struct MHD_Connection*  connection; /* for query/header lookup */
} HakiHttpRequest;

typedef struct {
    int         status;
    const char* body;
    const char* content_type;
} HakiHttpResponse;

typedef HakiHttpResponse* (*HakiHttpHandler)(HakiHttpRequest*);

typedef struct {
    struct MHD_Daemon* daemon;
    HakiHttpHandler    handler;
} HakiHttpServer;

/* Body accumulator for POST/PUT requests */
typedef struct { char* data; size_t len; } HakiBodyBuf;

static enum MHD_Result haki_mhd_callback(
    void* cls,
    struct MHD_Connection* conn,
    const char* url,
    const char* method,
    const char* version,
    const char* upload_data,
    size_t*     upload_data_size,
    void**      con_cls)
{
    (void)version;
    HakiHttpHandler handler = (HakiHttpHandler)cls;

    /* First call: allocate body buffer */
    if (*con_cls == NULL) {
        HakiBodyBuf* buf = (HakiBodyBuf*)calloc(1, sizeof(HakiBodyBuf));
        *con_cls = buf;
        return MHD_YES;
    }

    /* Accumulate body */
    HakiBodyBuf* buf = (HakiBodyBuf*)*con_cls;
    if (*upload_data_size > 0) {
        buf->data = (char*)realloc(buf->data, buf->len + *upload_data_size + 1);
        memcpy(buf->data + buf->len, upload_data, *upload_data_size);
        buf->len += *upload_data_size;
        buf->data[buf->len] = '\0';
        *upload_data_size = 0;
        return MHD_YES;
    }

    /* Build HakiHttpRequest and call handler */
    HakiHttpRequest req = {
        .method     = method,
        .path       = url,
        .body       = buf->data ? buf->data : "",
        .body_len   = buf->len,
        .connection = conn,
    };
    HakiHttpResponse* resp = handler(&req);
    int status = resp ? resp->status : 500;
    const char* body = (resp && resp->body) ? resp->body : "";
    const char* ct   = (resp && resp->content_type) ? resp->content_type : "text/plain";

    struct MHD_Response* mhd_resp = MHD_create_response_from_buffer(
        strlen(body), (void*)body, MHD_RESPMEM_MUST_COPY);
    /* Remove any default Content-Type MHD may have added, then set ours. */
    MHD_del_response_header(mhd_resp, "Content-Type", NULL);
    MHD_add_response_header(mhd_resp, "Content-Type", ct);
    MHD_add_response_header(mhd_resp, "Connection", "close");
    enum MHD_Result rc = MHD_queue_response(conn, (unsigned int)status, mhd_resp);
    MHD_destroy_response(mhd_resp);

    if (resp) free(resp);
    if (buf->data) free(buf->data);
    free(buf);
    *con_cls = NULL;

    return rc;
}

HakiHttpServer* haki_http_server_new(int64_t port, HakiHttpHandler handler) {
    HakiHttpServer* s = (HakiHttpServer*)malloc(sizeof(HakiHttpServer));
    if (!s) abort();
    s->handler = handler;
    s->daemon  = MHD_start_daemon(
        MHD_USE_THREAD_PER_CONNECTION,
        (uint16_t)port,
        NULL, NULL,
        haki_mhd_callback, (void*)handler,
        MHD_OPTION_END);
    if (!s->daemon) {
        fprintf(stderr, "haki: failed to start HTTP server on port %lld\n", (long long)port);
        free(s);
        return NULL;
    }
    return s;
}

void haki_http_server_listen(HakiHttpServer* s) {
    if (!s) return;
    fprintf(stderr, "haki: HTTP server listening (press Ctrl+C to stop)\n");
    /* Block forever — MHD handles threads internally */
    for (;;) { sleep(3600); }
}

/* Request field accessors */
const char* haki_http_request_path(HakiHttpRequest* r)   { return r ? r->path   : ""; }
const char* haki_http_request_method(HakiHttpRequest* r) { return r ? r->method : ""; }
const char* haki_http_request_body(HakiHttpRequest* r)   { return r ? r->body   : ""; }

/* Response constructor */
HakiHttpResponse* haki_http_response_new(int64_t status, const char* body) {
    HakiHttpResponse* r = (HakiHttpResponse*)malloc(sizeof(HakiHttpResponse));
    if (!r) abort();
    r->status       = (int)status;
    r->body         = body ? strdup(body) : strdup("");
    r->content_type = "text/plain";
    return r;
}

HakiHttpResponse* haki_http_response_json(int64_t status, const char* json) {
    HakiHttpResponse* r = haki_http_response_new(status, json);
    r->content_type = "application/json";
    return r;
}

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

/* ── Router ─────────────────────────────────────────────────────────
   Simple pattern-matching router for Haki HTTP servers.

   Patterns:
     /users        — exact match
     /users/:id    — segment wildcard, captures into params map
     /files/*      — prefix wildcard (matches anything after)

   Routes are stored in insertion order; first match wins.          */

typedef HakiHttpResponse* (*HakiRouteHandler)(HakiHttpRequest*);

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

HakiRouter* haki_router_new(void) {
    HakiRouter* r = (HakiRouter*)calloc(1, sizeof(HakiRouter));
    if (!r) abort();
    return r;
}

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
    HakiHttpRequest base;     /* must be first — cast-compatible */
    HakiMap*        params;   /* extracted route parameters */
} HakiRoutedRequest;

static HakiHttpResponse* haki_not_found(void) {
    return haki_http_response_new(404, "Not found");
}

static HakiHttpResponse* haki_method_not_allowed(void) {
    return haki_http_response_new(405, "Method not allowed");
}

/* The router's main dispatch function — called per request. */
HakiHttpResponse* haki_router_dispatch(HakiRouter* router, HakiHttpRequest* req) {
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

        HakiHttpResponse* resp = route->handler((HakiHttpRequest*)rr);
        haki_map_free(rr->params);
        free(rr);
        return resp;
    }

    return path_matched ? haki_method_not_allowed() : haki_not_found();
}

/* Accessor: get a route param by key from the request.
   Returns empty string if not found (router-dispatched requests only). */
const char* haki_request_param(HakiHttpRequest* req, const char* key) {
    HakiRoutedRequest* rr = (HakiRoutedRequest*)req;
    if (!rr || !rr->params || !key) return "";
    void* val_ptr = haki_map_get(rr->params, key);
    if (!val_ptr) return "";
    return *(const char**)val_ptr;
}

/* Phase 3 extras: query string, headers, content-type on response */

/* Parse query string — use MHD's built-in lookup if connection is available,
   fall back to manual parsing of req->path for non-MHD requests.            */
const char* haki_request_query(HakiHttpRequest* req, const char* key) {
    if (!req || !key) return "";
    /* Use MHD's query parameter lookup when available */
    if (req->connection) {
        const char* val = MHD_lookup_connection_value(
            req->connection, MHD_GET_ARGUMENT_KIND, key);
        return val ? val : "";
    }
    /* Fallback: manual parse of query string from path */
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
HakiHttpResponse* haki_http_response_typed(int64_t status, const char* body, const char* content_type) {
    HakiHttpResponse* r = haki_http_response_new(status, body);
    r->content_type = content_type ? strdup(content_type) : "text/plain";
    return r;
}

/* HttpResponse field accessors */
int64_t     haki_http_response_status(HakiHttpResponse* r) { return r ? r->status : 0; }
const char* haki_http_response_body(HakiHttpResponse* r)   { return r ? r->body   : ""; }

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
static const char* infer_content_type(const char* path) {
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
HakiHttpResponse* haki_serve_file(const char* path) {
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
    HakiHttpResponse* resp = haki_http_response_typed(200, body, infer_content_type(path));
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

/* haki_json_decode: parse flat JSON object {"k": "v"} into a HakiMap.
   Returns map on success, NULL + *out_error on failure.              */
void haki_json_decode(const char* s, HakiMap** out_map, char** out_error) {
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
    haki_json_decode(s, &m, &err);
    if (err || !m) { if (err) free(err); return ""; }
    void* vp = haki_map_get(m, key);
    const char* result = vp ? strdup(*(const char**)vp) : strdup("");
    haki_map_free(m);
    return result;
}

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
