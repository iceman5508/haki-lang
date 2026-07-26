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


/* HTTP server: excluded from bootstrap runtime */
