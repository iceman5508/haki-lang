/// arc.rs — ARC runtime declarations and C runtime source.

use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::values::FunctionValue;
use crate::types::ptr_ty;

pub struct ArcRuntime<'ctx> {
    pub retain:    FunctionValue<'ctx>,
    pub release:   FunctionValue<'ctx>,
    pub alloc:     FunctionValue<'ctx>,
    pub weak_load: FunctionValue<'ctx>,
}

impl<'ctx> ArcRuntime<'ctx> {
    pub fn declare(ctx: &'ctx Context, module: &Module<'ctx>) -> Self {
        let ptr  = ptr_ty(ctx);
        let void = ctx.void_type();
        let i64  = ctx.i64_type();

        let retain = module.add_function(
            "haki_retain",
            void.fn_type(&[ptr.into()], false),
            None,
        );
        let release = module.add_function(
            "haki_release",
            void.fn_type(&[ptr.into()], false),
            None,
        );
        let alloc = module.add_function(
            "haki_alloc",
            ptr.fn_type(&[i64.into()], false),
            None,
        );
        let weak_load = module.add_function(
            "haki_weak_load",
            ptr.fn_type(&[ptr.into()], false),
            None,
        );

        Self { retain, release, alloc, weak_load }
    }
}

/// C source for the ARC runtime shim. Emit alongside the IR and compile with:
///   clang -c haki_runtime.c -o haki_runtime.o
///   clang your_module.ll haki_runtime.o -o your_program
pub const RUNTIME_C_SOURCE: &str = r#"
// haki_runtime.c — Haki ARC runtime v0.1
#include <stdint.h>
#include <stdlib.h>
#include <stdatomic.h>
#include <string.h>
#include <stdio.h>

typedef struct {
    _Atomic int64_t refcount;
    void (*destructor)(void*);
} HakiObjHeader;

void haki_retain(void* obj) {
    if (!obj) return;
    HakiObjHeader* h = (HakiObjHeader*)obj;
    atomic_fetch_add_explicit(&h->refcount, 1, memory_order_relaxed);
}

void haki_release(void* obj) {
    if (!obj) return;
    HakiObjHeader* h = (HakiObjHeader*)obj;
    int64_t prev = atomic_fetch_sub_explicit(&h->refcount, 1, memory_order_acq_rel);
    if (prev == 1) {
        if (h->destructor) h->destructor(obj);
        free(obj);
    }
}

void* haki_alloc(int64_t size) {
    void* p = calloc(1, (size_t)size);
    if (!p) { fprintf(stderr, "haki: out of memory\n"); abort(); }
    HakiObjHeader* h = (HakiObjHeader*)p;
    atomic_store_explicit(&h->refcount, 1, memory_order_relaxed);
    return p;
}

void* haki_weak_load(void** slot) {
    void* obj = *slot;
    if (!obj) return NULL;
    HakiObjHeader* h = (HakiObjHeader*)obj;
    return (atomic_load_explicit(&h->refcount, memory_order_acquire) > 0) ? obj : NULL;
}

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

void haki_panic(const char* msg) {
    fprintf(stderr, "haki panic: %s\n", msg ? msg : "(none)");
    abort();
}
"#;
