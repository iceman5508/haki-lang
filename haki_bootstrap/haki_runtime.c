
/* haki_runtime.c — Haki v0.1 runtime
   Compile: clang -c haki_runtime.c -o haki_runtime.o               */

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

/* ── Error type ──────────────────────────────────────────────────────────── */

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
