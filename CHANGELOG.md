# Changelog

## v4.9.9 — Correctness

**90/90 QA tests passing. 5 new v4.9.9 tests added.**

### Bugs fixed

**Array subscript assignment in LLVM codegen** (`crates/haki_codegen/src/codegen.rs`)
`arr[i] = value` was silently a no-op in the LLVM path. The `emit_assign` function's target expression matched `_` (fallthrough) for `MonoExprKind::Index`, discarding the store. Fixed by adding an `Index` arm that calls the runtime `haki_array_get(arr, idx)` to get a write-through pointer, then emits a `store` into that pointer. Direct in-LLVM swaps and sorts now work correctly.

**`&&` and `||` short-circuit evaluation in LLVM codegen** (`crates/haki_codegen/src/codegen.rs`)
`BinaryOp::And` emitted `build_and` (bitwise AND, both operands evaluated eagerly). `BinaryOp::Or` emitted `build_or` (same problem). This meant `j >= 0 && arr[j] > 5` with `j = -1` would evaluate `arr[-1]` and crash with an out-of-bounds panic — even though the correct semantics are to skip the right-hand side when the left-hand side already determines the result. Fixed by restructuring `emit_binary` for `And`/`Or` to split into three basic blocks (LHS eval → conditional branch → RHS block → merge block) and use a PHI node to select the result. The fix is symmetric for `||` (branch skips RHS when LHS is `true`).

### New subcommands

**`hakic build <file> [-o output] [--release]`** (`hakic/src/main.rs`)
New `build` subcommand compiles a `.haki` source file to a native binary via the full LLVM pipeline. Without `-o`, the binary is placed next to the source file. `--release` passes through to the optimizer. This is distinct from `hakic run`, which also executes the binary after compilation.

**`hakic init [name]`** (`hakic/src/main.rs`, `crates/haki_pkg/src/commands.rs`)
`init` now also scaffolds `src/main.haki` with a hello-world template after writing `haki.toml`. Previously it only created the manifest.

### QA coverage added

5 new tests:
- Array subscript assignment (LLVM swap test: `arr[0]=arr[1]; arr[1]=tmp`)
- `&&` short-circuit: `j=-1 && arr[j] > 5` does not crash
- `||` short-circuit: `x=5 || arr[99] > 0` does not crash
- `hakic build` produces an executable that runs correctly
- `hakic init` creates both `haki.toml` and `src/main.haki`

## v4.9.8 — Performance

**85/85 QA tests passing. 6 new v4.9.8 tests added.**

### LLVM optimization

The `llc` invocation now passes `-O2`, enabling LLVM's full optimization pipeline (inlining, constant folding, loop unrolling, dead store elimination) before native object code is emitted. Previously the LLVM IR was compiled at `-O0`.

### `--release` flag

`hakic file.haki --release` now uses `-O3` on the C-emit path (vs the default `-O2`). Enables aggressive loop vectorization, additional inlining, and LTO-friendly code layout. The LLVM path always uses `-O2`.

### Dead function elimination

After monomorphization, a call-graph pass removes functions never reachable from `main`. Starting from `main`, the pass follows `Call(name, …)` edges to fixed point, then prunes any `MonoFn` not in the reachable set (closures and compiler-generated `__`-prefixed functions are always kept — they are dispatched via function pointer, not direct call). Typical programs see 1–5 functions pruned. The `[dce]` log line in `--verbose` output reports the count.

### Known limitation documented

Array element assignment (`arr[j] = value`) inside a called function does not write through to the caller's array — a pre-existing bug in the C emitter's array subscript assignment path. Targeted for v4.9.9.

## v4.9.7 — Tooling

**78/78 QA tests passing. 10 new v4.9.7 tests added.**

### Bugs fixed

**`hakic fmt --check` crash when flag precedes filename** (`hakic/src/main.rs`)
`hakic fmt --check file.haki` panicked with index-out-of-bounds because the subcommand hardcoded `args[2]` as the source path. `--check` occupied that slot, leaving the real filename at `args[3]` — or missing entirely. Fixed by scanning for the first non-flag argument, matching the same logic used by `hakic test` and `hakic doc`.

**`hakic fmt --check` wrong exit code on single file** (`hakic/src/main.rs`)
When checking a single file (rather than a directory), `fmt_file()` returned whether formatting was needed but the caller discarded the value and always exited 0. Fixed by capturing the return value and calling `process::exit(1)` when `check_only && needs_fmt`.

**`hakic fmt` strips match-arm guard conditions** (`hakic/src/main.rs`)
Match arms with a guard (`_ if n > 0 { … }`) were formatted as `_ { … }` — the `if guard` clause was silently dropped. Fixed by emitting `if <guard>` before the arm body in both `StmtKind::Match` and `ExprKind::Match` arm-rendering loops.

### QA coverage added

10 new tests covering all four tooling subcommands:
- `hakic check` on valid file (exit 0 + "ok")
- `hakic check` on file with typo (exit 1 + did-you-mean hint)
- `hakic fmt --check` on already-formatted file (exit 0)
- `hakic fmt --check` on unformatted file — flag-first and flag-last variants (exit 1 each)
- `hakic fmt` match-guard roundtrip (guard preserved after format)
- `hakic doc` generates HTML with `///` doc-comment entries for each function
- `hakic test` with `@skip` and panic-based assertions (2 passed, 1 skipped)

## v4.9.6 — Stdlib Completeness

**68/68 QA tests passing. 7 new v4.9.6 tests added.**

### Stdlib coverage expanded

Exercised and validated five stdlib modules with new QA tests: `std/json` (array encoding + nested parse with numeric values), `std/strings` (padLeft/padRight/isEmpty), `std/math` (clamp/pow), `std/env` (set/get/getOrDefault/unset), `std/process` (shell command), and `std/fs` (readDir listing).

### Bugs fixed

**`cemit`: `env__getOrDefault` crasher** (`crates/haki_cemit/src/lib.rs`)
The `__getOrDefault` interceptor in the C emitter had no arg-count guard, so calling any module-level function named `*__getOrDefault` (e.g. `env.getOrDefault`) with 2 args caused an out-of-bounds panic trying to access `args[2]`. Fixed by adding `args.len() == 3` to the Map interceptor.

**`codegen`: `Map.getOrDefault()` segfault in LLVM path** (`crates/haki_codegen/src/codegen.rs`)
`emit_map_get_or_default` stored the default value in an alloca and passed the alloca pointer as `default_val`, then did a `build_load` on the returned void*. This was inconsistent: when the key is found, `haki_map_get_or_default` returns the raw value (e.g. `char*`), not a pointer-to-value — loading from it dereferenced the string data as a pointer, causing a segfault. Fixed by passing the default value directly as `void*` (consistent with how the C emitter does it) and removing the extra `build_load`.

## v4.9.5 — Type System: Aliases, Optional Chaining, Match Guards

**61/61 QA tests passing. 6 new v4.9.5 tests added.**

### `type UserId = int` — Transparent type aliases

Type aliases are resolved before type-checking — they are fully transparent at the IR and codegen level. Aliases can be used anywhere a type is expected: function parameters, return types, struct fields, local bindings.

```haki
type UserId = int
type Score = int

fn greet(id: UserId) -> string { return "user:" + int_to_string(id) }
```

Changes:
- `crates/haki_ast/src/lib.rs`: `ItemKind::TypeAlias { name, ty, span }` variant.
- `crates/haki_typeck/src/collector.rs`: `type_aliases: HashMap<String, Ty>` on `SymbolTable`; `resolve_ty` checks aliases before raising `UnknownType`.
- `hakic/src/main.rs`: `rename_item` and `fmt_item` handle `TypeAlias`.

### `expr?.field` / `expr?.method()` — Optional chaining

Safe field/method access on nullable receivers. Returns `null` when the receiver is `null`, otherwise accesses the field normally. Chains correctly (`u?.address?.city`).

```haki
fn city_name(u: User?) -> string? {
    return u?.address?.city
}
```

Implementation:
- Parser: postfix loop detects `?` followed by `.` (guarded before ternary and binary infix paths to avoid conflict with `?` ternary operator).
- C emitter: expands to `((recv) != NULL ? (recv)->field : NULL)`.
- LLVM codegen: returns `Err` to force C-emitter fallback.

### `case x if condition` — Match guard conditions

Guards allow runtime filtering of match arms. If the guard is false the arm is skipped; the next arm is tried.

```haki
fn classify(n: int) -> string {
    match n {
        0          { return "zero" }
        _ if n > 0 { return "positive" }
        _          { return "negative" }
    }
}
```

Guards also work with enum arms. Multiple arms with the same discriminant are grouped and chained as `if/else if/else`:
```haki
Ok(v) if v > 100 { return "big" }
Ok(v)            { return "small:" + int_to_string(v) }
```

Implementation:
- `crates/haki_ast/src/lib.rs`: `guard: Option<Expr>` on `MatchArm`.
- `crates/haki_typeck/src/typed_ast.rs`: `guard: Option<TypedExpr>` on `TypedMatchArm`.
- `crates/haki_mono/src/mono_ast.rs`: `guard: Option<MonoExpr>` on `MonoArm`.
- `crates/haki_cemit/src/lib.rs`:
  - Int match: uses if-else chain when any arm has a guard (C switch can't express guard fallthrough).
  - String match: merges guard into strcmp condition.
  - Enum match: arms with the same discriminant are grouped; guards chain as `if/else if/else` within one outer `if (__tag == disc)` block.
- `crates/haki_codegen/src/codegen.rs`: returns `Err` when any arm has a guard, forcing C-emitter fallback.

---

## v4.9.4 — Annotation system: `@requires` and `@error`

**55/55 QA tests passing. 5 new annotation tests added.**

### `@requires(condition)` — entry guard assertions

`@requires(cond)` on a function emits a C guard immediately after the opening brace:

```c
if (!(cond)) { haki_panic("@requires(cond) failed"); }
```

The condition is a full Haki expression — identifiers, operators (`>`, `>=`, `&&`, `||`, `!`), literals, and nested parens all work. Multiple `@requires` annotations are supported and each emits its own guard in order.

Multiple annotations can be stacked: `@requires(n >= 0)` + `@inline` both apply correctly.

Changes:
- `crates/haki_cemit/src/lib.rs`: in `emit_fn()`, after the opening brace and `main` init, loop over `@requires` attrs and emit the C guard.
- `crates/haki_parser/src/parser.rs`: `parse_attributes()` now collects annotation args as raw token text up to the matching `)`, supporting full operator expressions. Also added bare string literal shorthand (`@error "msg"` without parens).

### `@error "msg"` — panic-on-error functions

`@error "msg"` on a function that returns `(T, Error?)` ensures the error is never propagated — any return where the error field is non-null panics instead. Callers do not need to `try` or check the error.

If `msg` contains `{err}`, the actual error message is interpolated via `snprintf`:

```haki
@error "parse failed: {err}"
fn parseInt(s: string) -> (int, Error?) { ... }
```

Generated C guard (before every multi-value return):
```c
if (__ret->f1 != NULL) { char __err_buf[512]; snprintf(..., "parse failed: %s", (char*)__ret->f1); haki_panic(__err_buf); }
```

Changes:
- `crates/haki_cemit/src/lib.rs`: added `current_fn_error_msg: RefCell<Option<String>>` field to `Cx`. Set at top of `emit_fn()` from `@error` attr. In the multi-return branch of `emit_stmt`, check the flag and emit panic guard before `return __ret`.

### Files changed

- `crates/haki_cemit/src/lib.rs`
- `crates/haki_parser/src/parser.rs`
- `qa/run_qa6.sh` (55 tests total, +5 v4.9.4)

---

## v4.9.3 — Concurrency v0.2: select timeout, sync module, Chan<T> cast fixes

**48/48 QA tests passing. 7 new concurrency tests added.**

### `select` with `timeout(ms)` — now works correctly

Previously `select { ... timeout(200) { ... } }` blocked forever — the cemit emitted a comment and then an `else` branch that never triggered, while the underlying `haki_select` had no timeout support.

Fixed in two places:
- `crates/haki_stdlib/src/runtime.rs`: `haki_select` now takes a fifth `int64_t timeout_ms` parameter. When `>= 0`, it computes an absolute deadline via `clock_gettime(CLOCK_REALTIME, ...)` and uses `pthread_cond_timedwait` instead of `pthread_cond_wait`. Returns `-1` on timeout. Both copies of the function (in `CORE_RUNTIME_C_SOURCE` and `RUNTIME_C_SOURCE`) were updated.
- `crates/haki_cemit/src/lib.rs`: the `Select` statement emitter now extracts the `timeout_ms` expression (or `-1` if no timeout arm), passes it as the fifth argument, and emits `else if (__sel_ready_{uid} == -1) { /* timeout body */ }`.

### `sync.chan<T>(n)` and `sync.group<T>()` — module-qualified constructors

`sync` is now a recognized builtin namespace. `sync.chan<string>(3)` produces a `Chan<string>` with capacity 3. `sync.group<int>()` produces a `TaskGroup<int>`.

Two changes:
- `crates/haki_parser/src/parser.rs`: the postfix method-call parser now speculatively tries `<type_args>` before checking for `(`. If it finds `<T>(args)`, it encodes the type args into the method name as `"chan<string>"` — matching the same encoding used for constructor calls.
- `crates/haki_typeck/src/infer.rs`: `infer_method_call` now pre-processes encoded generic method names (strips `<...>`, injects into `type_args` as `T`, `U`, ...) and adds a special case for `sync.chan` → `haki_chan_new(capacity)` returning `Chan<T>`, and `sync.group` → `haki_taskgroup_new()` returning `TaskGroup<T>`.

### `Chan<T>` send/recv cast correctness

`haki_chan_send` and `haki_chan_receive` previously always cast through `(void*)(intptr_t)`, which corrupted pointer values (strings, structs) stored in channels. Fixed in `crates/haki_cemit/src/lib.rs`:
- Send: pointer types (`string`, optional, named, generic) use `(void*)(val)` directly; primitives keep `(void*)(intptr_t)(val)`.
- Receive: pointer types cast directly `(T*)haki_chan_receive(ch)`; primitives use `(T)(intptr_t)haki_chan_receive(ch)`.

### Files changed

- `crates/haki_stdlib/src/runtime.rs` — `haki_select` signature updated (+ `timeout_ms` param), `pthread_cond_timedwait` support
- `crates/haki_cemit/src/lib.rs` — select emitter updated, chan send/recv cast fix
- `crates/haki_typeck/src/infer.rs` — `sync` builtin namespace, generic method name pre-processing
- `crates/haki_parser/src/parser.rs` — `obj.method<T>(args)` generic method call parsing
- `qa/run_qa6.sh` — 7 new v4.9.3 tests (now 48 tests)

---

## v4.9.2 — Zero external HTTP dependency: self-contained HTTP/1.1 server

**41/41 QA tests passing. libmicrohttpd fully removed from the build.**

### libmicrohttpd removed

The Haki HTTP server now runs on pure POSIX sockets + pthreads with no external library required. Previously `hakic/src/main.rs` ran `pkg-config --exists libmicrohttpd` at compile time and, if found, passed `-DHAKI_MHD_SERVER` to GCC — which activated the MHD code path in `runtime.rs`. That detection existed in three separate places in the driver (C-emit compile, C-emit link, LLVM runtime compile) and has been removed from all three.

`runtime.rs` previously contained two full server implementations guarded by `#ifdef HAKI_MHD_SERVER` / `#ifndef HAKI_MHD_SERVER`. The conditional guards and the MHD implementation are gone; only the self-contained socket/pthread implementation remains. The `haki_request_query` function that conditionally called `MHD_lookup_connection_value` now uses only the manual query-string parser.

A compiled HTTP server binary links only `libcurl` (for the HTTP *client* functions) and `libc` — `libmicrohttpd` does not appear in `ldd` output.

New QA test: starts a Haki HTTP server on port 19878, hits `/ping` (expects `"pong"`, 200) and `/missing` (expects 404), verifies both responses, then kills the server.

### Files changed

- `hakic/src/main.rs` — removed MHD detection + linking in 3 locations
- `crates/haki_stdlib/src/runtime.rs` — removed MHD implementation + conditional guards
- `qa/run_qa6.sh` — added `v4.9.2: self-contained HTTP server` test (now 41 tests)

---

## v4.9.1 — Language Completeness: Map Iteration, Inherited Fields, Mutable Closure Capture

**40/40 QA tests passing. Three language-completeness fixes from roadmap v4.0.**

### `for k, v in map` — map key-value iteration

LLVM codegen (`emit_for` in `haki_codegen/src/codegen.rs`) now detects `SemTy::Generic("Map", _)` and dispatches to a new `emit_for_map` method. Map iteration calls `haki_map_capacity` for the loop bound, `haki_map_entry_key` (null-checking for deleted slots), and `haki_map_entry_value`. Integer/bool values are recovered via `ptrtoint`; pointer-typed values are passed directly.

### Inherited fields — `class Dog extends Animal`

`emit_class_def` in `haki_cemit/src/lib.rs` now flattens parent fields directly into the child struct (instead of embedding via `__super`), so `dog->name` accesses work correctly. Constructor `class_field_types` lookup extended to include parent fields so `Dog(name: "Rex", breed: "Lab")` initializes inherited fields with the correct types.

### Mutable closure capture — implicit free-variable detection

Inner functions/closures now automatically capture outer `let` bindings they reference, without requiring explicit `fn[count]() -> int { ... }` syntax. The fix is in `haki_typeck/src/infer.rs`: after resolving explicit captures, a standalone AST scanner (`collect_free_idents_block`) finds all free variable references in the body, cross-checks them against the outer scope via `lookup_var`, and adds them as implicit captures with correct mutability (`let` → mutable capture via pointer). The single-capture ABI (`__env` = `&outer_var`) in the mono engine and cemit is unchanged.

---

## v4.9.0 — Float Math, f64 Extern Support, fs.readLines, Strings Extended

**37/37 QA tests passing. Float math functions, f64 extern type support in both LLVM and C backends, fs.readLines, and 5 new string utilities.**

### Float math — `std/math` f64 functions

`math.sqrt`, `math.floor`, `math.ceil`, `math.powf`, `math.log`, `math.sin`, `math.cos`, `math.absf`, `math.floorInt`, `math.ceilInt`, `math.roundInt` are now fully operational. C runtime wrappers added in `RUNTIME_C_SOURCE`; `-lm` added to the LLVM-path link step in `hakic/src/main.rs` (was previously only in the C-emit path).

### f64 extern type — both backends fixed

`extern "c"` functions with `f64` parameter/return types now generate correct types in both backends:
- **C-emit**: `ast_ty_to_c` in `haki_cemit/src/lib.rs` maps `"f64"` → `double` (was falling through to `void*`).
- **LLVM IR**: `declare_extern_js_fns` in `haki_codegen/src/codegen.rs` maps `"f64"` → `f64_type` for both params and return (was falling through to `ptr`).

### `fs.readLines` added

`fs.readLines(path)` reads a file and returns `(Array<string>, Error?)` — each element is one line (newline stripped). Implemented in pure Haki on top of `readFile` + `split`.

### Strings extended — 5 new functions

`strings.trimLeft`, `strings.trimRight`, `strings.replaceAll`, `strings.indexOf`, `strings.substring` added to `stdlib/strings.haki`. All delegate to existing built-in string method dispatch (C runtime already had the symbols).

---

## v4.8.0 — Optional Narrowing, stdlib Completeness, and HTTP End-to-End

**34/34 QA tests passing. Guard-clause narrowing, fs.readFile fix, strings.count, template.vars(), xml/csv/regex test correctness, HTTP localhost verified.**

### Guard-clause optional narrowing (new in v4.8)

After `if x == null { return }` (or `break`/`panic`), the type checker now narrows `x` from `T?` to `T` for all subsequent statements in the enclosing block. Implemented via `block_always_terminates()` helper and post-if-stmt narrowing pass in `infer_block`. Example: `fn greet(id: int) -> string { const user = findUser(id); if user == null { return "unknown" }; return "Hello, " + user }` — the final line compiles without an unwrap because `user` is narrowed to `string` after the guard.

### `fs.readFile` fix — garbage bytes resolved

`haki_fs_read_file` in `RUNTIME_C_SOURCE` was calling `haki_read_file()` (which returns a `void*` tuple `[content, error]`) and casting the tuple pointer to `const char*`. The callee in Haki expected a plain string, so it received garbage. Fixed to call `haki_file_read()` directly and return just the content string.

### `strings.count` added

`strings.count(s, sub)` counts non-overlapping occurrences of `sub` in `s`. Implemented in pure Haki via `s.split(sub)` — returns `parts.length - 1`.

### `template.vars()` — zero-arg API

`template.vars(key, value)` (2-arg, single-entry map) changed to `template.vars()` (0-arg, returns empty `Map<string,string>`). Callers now call `.set()` to populate the map, making it more flexible for multiple variables.

### Regex find tuple — test alignment

`regex.find` returns `(string, Error?)`. QA tests updated to destructure the tuple before printing: `const found, ferr = regex.find(...)`.

### XML API — test alignment

`xml.getAttr(tagStr, attrName)` takes 2 args (the tag/doc string and the attribute name). Tests updated from erroneous 3-arg call. `xml.parseAttrs` takes the attribute portion of a tag (e.g. `id="1" class="x"`), not a full element tag — tests updated.

### CSV — semicolons fix

Haki does not support semicolon-separated statements. QA test updated to use multi-line if-block for `csv.parse` error handling.

### HTTP end-to-end verified

`http.get` is now verified against a real localhost Python HTTP server in the QA suite. Status 200 and body "pong" are both checked.

---

## v4.7.0 — String Methods, JSON Wiring, and Full QA Green

**28/28 QA tests passing. String extras, JSON flat API, regex findGroups, and CSV are all wired end-to-end.**

### String extras (new in v4.7)

`s.isEmpty()` — returns `true` if the string has length 0. `s.charAt(n)` — returns the character at index `n` as a single-character string. `s.charCodeAt(n)` — returns the Unicode code point at index `n` as an `int`. All three are implemented in `haki_string_is_empty`, `haki_string_char_at`, `haki_string_char_code_at` in the runtime and wired via the codegen `emit_string_method` dispatch.

### `std/json` — Full flat API wired (bug fixes)

Fixed `haki_json_decode` registration in `codegen.rs`: was incorrectly declared as `void(ptr, ptr, ptr)` (3-arg out-param) but the actual C function is `void*(ptr)` (1-arg, returns the parsed map). Updated `emit_json_decode` to call the 1-arg form and return the map directly. Fixed all JSON flat-API functions (`haki_json_str`, `haki_json_num`, `haki_json_flag`, `haki_json_encode_object`, `haki_json_encode_array`, `haki_json_decode`, `haki_json_decode_get`) from `static` to external linkage so they are visible to the compiled Haki object file at link time.

### `std/csv` — Linker fix

`haki_csv_parse_row`, `haki_csv_encode_row`, `haki_csv_parse`, `haki_csv_encode` were `static` and invisible to the Haki object file. Removed `static` from all four functions in the CORE section to expose them as external symbols.

### `std/regex` — findGroups fix

`haki_regex_find_groups` was returning the full match at groups[0] (POSIX group 0) instead of the first capture group. Fixed to skip `fgrp[0]` and return only capture groups at indices 0..N-1. Correctly handles patterns with no capture groups (returns empty array). Test updated to use POSIX ERE syntax (`[0-9]`) instead of `\d`.

### `emit_map_set` — Critical bug fix

Map set was allocating a stack alloca and passing its address as the value pointer instead of passing the value directly. `haki_map_get` then returned the alloca address, causing garbage output when reading map entries. Fixed by passing `inttoptr` of int/bool values and pointer values directly.

### HTTP curl status fix

`haki_curl_do` was initializing `long code = 200` before calling `curl_easy_perform`. Failed connections returned status 200 instead of 0. Fixed to check `CURLcode res` and only default to 200 on `CURLE_OK`.

### QA

All 28 tests now pass: core language (fibonacci, map/array ops, classes, enums, closures, optionals, multi-return, while loop), string methods, JSON (parse/stringify/decodeGet/object builder), regex (matches/find/replaceAll/split/findGroups), template (render/vars/conditionals), XML (element parsing/emission), CSV (row and document API), and HTTP (module compilation).

---

## v4.6.0 — CSV, Regex, and Stdlib Stability

**All stdlib modules are now importable and fully tested. Five runtime bugs in map-value dereferencing were fixed across the template, XML, and CSV engines. Regex module now loads correctly.**

### `std/csv` — CSV and TSV parsing / encoding (new)

**Row API** — `csv.parseRow(line)` / `csv.encodeRow(fields)` / `csv.parseRowTSV(line)` / `csv.encodeRowTSV(fields)` — single-row parse and encode with RFC 4180 quoting rules (embedded commas, quoted fields, escaped `""`).

**File API** — `csv.parse(s)` → `Array<Array<string>>` / `csv.encode(rows)` — full document round-trip with newline-separated rows; TSV variants `csv.parseTSV` / `csv.encodeTSV` for tab-separated data.

### `std/regex` — Fixes

Module now loads without conflicting C type declarations. `regex.findGroups` registered in the type system (previously missing). All five functions — `matches`, `find`, `replaceAll`, `split`, `findGroups` — verified working.

### `std/template` — Bug fixes

Fixed map-value double-dereference segfault in `trf_mapget` (`*(char**)vp` → `(char*)vp`) and in `haki_map_copy_with` (`&vs` → `(void*)vs`). Template `{{#for}}` iteration now works correctly.

### `std/xml` — Bug fixes

Fixed map-value double-dereference in `haki_xml_emit_tag` and `haki_xml_parse_attrs`. Both functions now store and retrieve `char*` values as direct `void*` pointers, consistent with the Haki map convention.

### Stdlib import fixes

`std/template`, `std/xml`, and `std/csv` were missing from `stdlib_source()` and silently fell back to filesystem lookup (which always failed). All three are now embedded via `include_str!` and importable.

### Internal: Haki Map convention documented

Map values are stored as `void*` directly — `haki_map_set(m, key, (void*)char_ptr)`. Retrieval: `(const char*)haki_map_get(m, key)` — **not** `*(const char**)...`. Violating this caused all four runtime segfaults fixed in this release.

---

## v4.5.0 — Stdlib Depth

**Standard library coverage reaches production-ready breadth: template engine, XML utilities, and comprehensive JSON + time APIs are now available to every Haki program.**

### `std/template` — HTML and text templating engine (new)

**`{{var}}`** — variable substitution with automatic lookup in a `Map<string, string>` context.

**`{{#if var}}...{{/if}}`** — conditional blocks. Truthy: any non-empty, non-`"false"`, non-`"0"` value.

**`{{#if var}}...{{#else}}...{{/if}}`** — optional else branch inside any if block.

**`{{#for item in list}}...{{/for}}`** — iteration over newline-separated value lists.

**`template.escape(s)`** — HTML-safe escaping of `& < > " '` entities.

**`template.vars(key, value)`** — convenience constructor for single-entry variable maps.

### `std/xml` — Lightweight XML read/write utilities (new)

**`xml.getElement(xmlStr, tag)`** — extract the text content of the first `<tag>...</tag>`.

**`xml.parseAttrs(attrStr)`** — parse `key="value"` attribute strings into a `Map<string, string>`.

**`xml.getAttr(tagStr, attrName)`** — extract a single named attribute from a raw tag string.

**`xml.emitElement(tag, content)`** — wrap content in `<tag>content</tag>`.

**`xml.emitTag(tag, attrs)`** — build a self-closing `<tag attr="val"/>` from an attribute map.

**`xml.escape(s)`** — XML/HTML entity encoding for `& < > " '`.

### `std/json` — Extended JSON API

**Flat encode API** — `json.str(s)`, `json.num(n)`, `json.flag(b)`, `json.nullValue()` produce pre-encoded JSON fragments for assembly into objects and arrays.

**`json.object(fields)`** / **`json.array(items)`** — build JSON objects and arrays from `Map<string,string>` and `Array<string>` containing pre-encoded values.

**`json.decode(s)`** — parse a flat JSON object into `Map<string,string>`, returning `(map, Error?)`. All values come back as plain strings; nested objects/arrays retain their raw JSON text.

**`json.decodeGet(s, key)`** — decode a JSON object and retrieve a single key in one call.

**`json.parse(s)`** / **`json.stringify(fields)`** — nested round-trip API: `parse` decodes string values and keeps nested objects as raw JSON; `stringify` auto-detects raw JSON vs plain strings and quotes accordingly.

**Bug fix** — `json.parse` / `json.stringify` map iteration now walks by capacity with null-slot guards instead of by length, eliminating index-out-of-bounds crashes on maps with gaps.

### `std/time` — Extended time utilities

**Formatting** — `time.format(ts)` (ISO 8601 UTC), `time.formatPattern(ts, pattern)` (strftime), `time.formatTz(ts, offsetMinutes)` (ISO 8601 with tz offset).

**Parsing** — `time.parse(s)` accepts `"YYYY-MM-DD"` and `"YYYY-MM-DDTHH:MM:SSZ"` formats.

**Calendar helpers** — `time.dayOfWeek(ts)`, `time.dayName(wday)`, `time.monthName(month)`, `time.startOfDay(ts)`, `time.startOfNextDay(ts)`, `time.sameDay(a, b)`.

**Duration constants** — `time.minuteSec()`, `time.hourSec()`, `time.daySec()`, `time.weekSec()`.

**Arithmetic** — `time.addSeconds`, `time.addMs`, `time.diffSec`, `time.diffMinutes`, `time.diffHours`, `time.diffDays`.

---

## v4.4.0 — Tooling + Ecosystem

**Compiler toolchain polish: test runner, formatter, doc generator, type checker, and VS Code extension all gain significant new capabilities.**

### `hakic test` — test runner overhaul

**`@timeout(ms: N)` annotation** — fail a test if it exceeds the given wall-clock limit. Works at any value; the default cap is 30 000 ms. Implemented as a polling loop (`try_wait` every 10 ms) so no platform threads are consumed waiting.

**Parallel execution** — tests run across up to 4 threads by default (`--sequential` to disable). Results are re-sorted to declaration order before printing, so output is deterministic regardless of scheduling.

**Per-test timing** — each result line shows elapsed time in milliseconds: `pass  test_add  (12ms)`.

**Directory mode** — `hakic test .` finds and runs every `.haki` file in a directory, printing a `=== total: N passed, M failed ===` summary and exiting non-zero on any failure.

**`@skip` attribute** — annotate a test function with `@skip` to exclude it from the run.

### `hakic fmt` — formatter fixes

**`AnnotationDef` items now format correctly.** Previously, `annotation @name(params) { body }` was silently dropped from formatted output. Fixed: the formatter now emits the full definition.

**Directory mode** — `hakic fmt .` formats all `.haki` files in a directory. `--check` counts files that need formatting and exits non-zero if any do.

### `hakic doc` — documentation generator

**`@param` / `@returns` tag parsing** — `///` doc comments are now parsed for `@param name description` and `@returns description` lines. These render as an HTML parameter table and a styled returns section, not just raw comment text.

**Directory mode** — `hakic doc .` generates one HTML file per `.haki` source file, plus an `index.html` card grid linking them all.

**`--out <dir>` flag** — redirect all generated HTML to a specific output directory (created if it doesn't exist). Without `--out`, output lands next to the source file.

### `hakic check` — type checker improvements

**Directory mode** — `hakic check .` typechecks all `.haki` files and prints a `N/M files ok` summary.

**"Did you mean?" suggestions** — when `UnknownVar` or `UnknownFn` is reported, Haki computes the Levenshtein edit distance against all top-level names and common builtins (capped at distance 2) and prints a `hint: did you mean \`X\`?` line if a close match exists.

### Parser — attribute improvements

**Integer and named-parameter arguments** in `@` annotations. The attribute argument parser now accepts:
  - Integer literals: `@timeout(5000)`
  - Named `key: value` pairs: `@timeout(ms: 5000)`
  - Float literals and bare identifiers

**Keyword annotation names** — annotations whose name is a Haki reserved word (e.g. `@timeout`, `@async`, `@select`) now parse without error. The parser extracts the keyword text and treats it as the annotation name.

### VS Code extension v0.4.0

**`chan<T>` channel type highlighting** — `chan` renders as `support.type.haki`; the `<T>` type parameter renders as `entity.name.type.haki`. Grammar version bumped to 4.4.0.

**`hakic test` task provider** — `hakic test .` appears as a VS Code Task in the `Test` group. Run it from the Tasks menu or bind it to a keyboard shortcut.

**Snippet completions** — five new lightweight snippets (fire before LSP, feel instant):
  - `test_` → `fn test_${name}() { }` template
  - `@t` → `@timeout(ms: 5000)`
  - `///` → doc comment with `@param` / `@returns` scaffold
  - `chan<` → `chan<int>`, `chan<float>`, `chan<string>`, `chan<bool>` completions
  - `annotation` → full annotation definition template with `__original__()`

### QA

14/14 QA programs pass. Unit tests: 234/234.

---

## v4.3.0 — Windows Desktop (Win32/WebView2)

**`haki-desktop` now works natively on Windows with no external dependencies.**

### What's new

**Win32 backend — no MSYS2, no GTK, no X server required.**
The `--target win32` path now correctly prepends all Win32 forward declarations
to user C before compilation. Fixed a bug where the declarations were built into
a shadowed variable and never written to the output file.

**New canonical binary names.**
`haki-desktop`, `haki-server`, and `haki-browser` are now first-class `[[bin]]`
entries in Cargo.toml (previously only the deprecated `haki-gtk`, `haki-dom`,
`haki-web` aliases existed). Old names still work but emit a deprecation warning.

**Windows install script (`install.ps1`).**
```
irm https://raw.githubusercontent.com/iceman5508/haki-lang/main/install.ps1 | iex
```
Downloads the latest `haki-windows.zip`, extracts to `%LOCALAPPDATA%\haki\bin`,
and adds it to the user PATH — no admin rights required.

**Winget package manifest (`manifests/h/Haki/Haki/Haki.yaml`).**
Skeleton manifest ready to submit to `microsoft/winget-pkgs`. Update
`InstallerSha256` after building the release zip.

**CI matrix extended to `windows-latest`.**
`.github/workflows/ci.yml` now runs a full Windows job: unit tests (no LLVM),
`--emit-c` smoke test, `haki-desktop --version` check, bootstrap self-check,
and packages `haki-windows.zip` as a workflow artifact on every run.

**Full forward declarations for Win32 backend.**
Added missing `haki_gtk_create_text_field`, `haki_gtk_create_checkbox`,
`haki_gtk_create_dropdown`, `haki_gtk_create_image`, `haki_gtk_set_callback`,
`haki_gtk_set_padding`, `haki_gtk_set_spacing`, `haki_gtk_set_alignment`,
`haki_gtk_peek_next_id`, `haki_gtk_get_label_id`, `haki_app_run`, and
`haki_set_rerender_callback` to the win32 forward-declaration block.

### Bug fixes

- **`^0.x.y` semver caret** — fixed: when major is 0, caret now acts like tilde
  (locks major.minor, allows patch >= req.patch), matching the npm/Cargo spec.
- **`haki_map_entry_key` double-definition** — wrapped first definition in
  `RUNTIME_C_SOURCE` with `#ifndef HAKI_MAP_ENTRY_DEFINED` guard.
- **`-DHAKI_HTTP_TYPES_DEFINED` in LLVM path** — removed; the flag is only valid
  for unity (C emit) builds. In the LLVM path, `runtime.c` is a separate TU and
  must define its own `HttpRequest`/`HttpResponse` structs.
- **`semver.rs` doctest false positive** — file-level comment block converted from
  `///` to `//` to prevent rustdoc from treating the example table as runnable code.
- **`test_mixed_numeric_types_error`** — updated to accept `TypeMismatch` as well
  as `InvalidBinary` (typechecker now infers `f64` for `1 + 3.14`, reports the
  mismatch at the return site instead of the binary op).

### QA

14/14 QA programs pass. Unit tests: 56/56.

---

## v4.2.0 — Package Registry

## v2.1.0 — Bootstrap Complete: Type Inference Threading

**The Haki compiler is now fully self-hosting.** Stage 0 (Rust) and Stage 1 (Haki-compiled) produce identical output for all test programs including integer arithmetic, string operations, and complex control flow.

### New: Type inference pass for the bootstrap compiler

`haki_bootstrap/haki_typeinfer.haki` — a new type inference module that threads type information through the bootstrap C emitter:

- `TypedExpr { kind, ty }` — every expression now carries a resolved type tag
- `Scope` — parallel name/type arrays for variable lookup during expression walking
- `inferExprTy` — resolves `"int" | "float" | "bool" | "string" | "void" | "void*"` for any expression
- `binaryOpC(op, ty)` — returns the correct C operator: `+` for integers, `haki_string_concat` for strings, `strcmp` for string equality
- `buildFnScope` — pre-walks a function body to populate scope from parameters and let bindings

### Fixed: Bootstrap C emitter type correctness

- **Binary operators** — `i + 1` where `i: int` now emits `(i + ((int64_t)1LL))` instead of `haki_string_concat(i, 1LL)`
- **Variable declarations** — `let i = 2` now emits `int64_t i = 2LL` instead of `void* i = (void*)(2LL)`
- **Comparison operators** — integer `==` and `!=` now emit `==` and `!=` instead of `strcmp`

### Fixed: Bootstrap import resolution

- Import paths in `IImport` items omit the `.haki` extension — `resolveImports` now appends it before calling `readFile`
- Added `alreadyResolved(visited, path)` deduplication to prevent processing the same module multiple times

### Bootstrap proof

Three programs, all producing identical output from Stage 0 (Rust) and Stage 1 (Haki-compiled):

| Test | Stage 0 | Stage 1 |
|------|---------|---------|
| Fibonacci | `0 1 1 2 3 5 8 13 21 34` | `0 1 1 2 3 5 8 13 21 34` ✓ |
| Strings | `Hello, World!` / `Hello, Haki!` | `Hello, World!` / `Hello, Haki!` ✓ |
| Primes | `2 3 5 7 11 13 17 19 23 29` | `2 3 5 7 11 13 17 19 23 29` ✓ |

### Other changes

- `haki_typeck.haki`: added `semTyToStr(ty: SemTy) -> string` helper
- `haki_mono.haki`: added `mergeProgramWithAlias(dst, src, alias)` for import resolution
- `haki_cemit.haki`: all emit functions now take `sc: tinfer.Scope, sym: typeck.SymTable`
- Version bumped to 2.1.0

---

## v2.0.0 — Bootstrap: Stage 1

**The Haki compiler compiles itself.** A Haki-compiled binary (`hakic_s1`) correctly runs `check` and `--emit-c` on Haki source files.

### C emitter fixes

- **Array append lvalue** — non-addressable expressions emitted as temp vars before `haki_array_append_val`
- **Enum variant semicolon** — trailing `;` added inside GNU statement expression: `__ev;` not `__ev`
- **`haki_read_file` wrapper** — returns `(string, Error?)` tuple via raw `void**` pairs
- **`haki_write_file` wrapper** — forward declaration added to runtime
- **`Map()` constructor** → `haki_map_new(sizeof(void*))`
- **`Map__getOrDefault`** → `haki_map_get_or_default(map, key, default)`
- **Array element size** — `sizeof(void*)` for pointer types, not `sizeof(void)`
- **Multi-field enum binding extraction** — extra dereference: `*({bt}*)((void**)__mpayload)[{bi}]`

---

## v1.9.1 — Binary Aliases

Five binaries from one `src/main.rs` via `argv[0]` dispatch:

| Binary | Equivalent | Use case |
|--------|-----------|---------|
| `haki` | `hakic` | run any .haki file |
| `haki-gtk` | `hakic --target gtk` | compile + run GTK desktop app |
| `haki-dom` | `hakic --emit-wasm` | compile to .wasm for browser |
| `haki-web` | `hakic --target so` | compile to .so for Apache/nginx |
| `hakic` | (canonical) | tooling, CI, scripts |

---

## v1.9.0 — haki_ui: Native UI with GTK + DOM backends

- `State<T>` — reactive state with explicit `.set()`, `StateBase` protocol for type-erased dirty checking
- `Element` enum — 17 variants: Text, Button, Column, Row, Stack, ScrollView, Image, ViewEmbed, etc.
- `View` protocol — `fn body() -> Element`
- `App.run()` — serializes Element tree to JSON, dispatches to platform
- `haki_ui_gtk.c` — GTK 3 backend with minimal JSON parser
- `haki_ui_dom.js` — DOM backend with CSS flexbox layout
- `examples/counter.haki` — reference app, identical source for both targets

---

## v1.8.0 — C FFI: @attr, extern "c", @link, haki_db

- `@attr(args)` syntax — attributes on declarations: `@link("libname")`, `@inline`, `@deprecated`
- `extern "c"` — C function declarations that compile to forward declarations in the C backend
- `@link("libname")` — auto-injects `-lname` at link time, no Makefile required
- `stdlib/postgres.haki` — PostgreSQL bindings via libpq
- `stdlib/sqlite.haki` — SQLite 3 bindings

---

## v1.7.0 — WebAssembly: extern "js", haki_dom

- `extern "js"` FFI — declare and call JavaScript functions from Haki
- `haki_runtime.js` — browser/Node.js runtime shim: memory helpers, DOM bindings, element handle table
- `stdlib/dom.haki` — `getElementById`, `setText`, `onClick`, `fetch`, etc.
- `haki` binary alias alongside `hakic`
- Wasm short-circuit before LLVM codegen

---

## v1.6.0 — Web Deployment

- `hakic --target so` — compile to shared library with `haki_handle_request` ABI
- `mod_haki/mod_haki.c` — Apache 2.4 module: lazy dlopen, ABI version check, full request/response bridge
- `mod_haki/haki_fastcgi.c` — FastCGI adapter for nginx, Caddy, lighttpd (thread-per-connection)
- `mod_haki/test_harness.c` — standalone dlopen test (8/8 pass)
- `haki_abi.h` — stable C ABI contract: `HakiRequest`, `HakiResponse`, `haki_handle_request`

---

## v1.5.0 — LSP + VS Code Extension

- `hakic lsp` — hand-rolled JSON-RPC Language Server Protocol daemon (no tower-lsp dependency)
- Parser error recovery — `parse_recovery()` returns partial AST on syntax errors
- `textDocument/hover` — expression type display + full function/struct/enum signatures
- `textDocument/definition` — go-to-definition for local variables, parameters, top-level declarations
- `textDocument/publishDiagnostics` — parse + type errors with accurate line/col
- VS Code extension (`haki-language-0.1.0.vsix`) — syntax highlighting, squiggles, hover, F12

---

## v1.4.0 — Integer and String Match

- `match` on integer literals: LLVM switch / C switch
- `match` on string literals: strcmp if-else chain
- Wildcard `_` required and enforced at compile time
- `MatchPattern` enum: `Ident`, `Int`, `String`

---

## v1.3.0 — Standard Library Expansion

- `std/path` — join, dir, base, ext, stem, isAbsolute, isSafe
- `std/env` — get, set, cwd, chdir, home, args
- `std/time` — nowMs, nowSec, sleepMs, sleep, format
- `std/process` — run, exec, shell, exit
- `std/regex` — matches, find, replaceAll, split
- `std/json` — encode, decode, string, int, bool, null, decodeGet

---

## v1.2.0 — Package Manager + Windows

- `hakic pkg` — init, add, install, update, remove, list
- `haki.json` + `haki.lock` — manifest and reproducible lock file
- Git dependency support with `#tag` fragments
- Cache: `~/.haki/pkg/<alias>@<short-commit>/`
- Windows build (C backend only, no LLVM requirement)

---

## v1.1.0 — Developer Experience

- `haki hello.haki` bare-path execution (always uses C backend, temp dir)
- `hakic run` auto-fallback to `--emit-c` when LLVM not available
- `.length` on Array fixed in C backend

---

## v1.0.0 — Initial Release

- Full compiler pipeline: lex → parse → typeck → mono → LLVM codegen + C emit
- ARC memory management (retain/release injection in codegen)
- Closures with explicit capture lists
- ADTs (enums with payloads)
- Async/await + Task<T> + Mutex<T>
- HTTP server (HttpServer, Router, HttpRequest, HttpResponse)
- Native UI via GTK 3 (App, View, Element)
- Modules and import system
- `hakic check`, `hakic test`, `hakic fmt`, `hakic doc`
- Homebrew distribution
- macOS + Linux + Windows CI
