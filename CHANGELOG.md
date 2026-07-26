# Changelog

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
