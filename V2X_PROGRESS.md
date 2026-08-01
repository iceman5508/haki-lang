# Haki v2.x — Pre-release Development Track

This directory is the active development build for v2.x features.
None of this runs on hardware until v3.0.

## Status

### v2.2 — Self-hosting loop ✅ COMPLETE (in main branch)
- diff hakic_s2.c hakic_s3.c = 0
- 222/222 tests passing

### v2.3 — Channels + Structured Concurrency ✅ COMPLETE

**Done:**
- [x] C runtime: `HakiChan` (bounded + unbounded ring buffer/linked list)
- [x] C runtime: `haki_select` (deadlock-free, starvation-resistant)
- [x] C runtime: `HakiTaskGroup` (dynamic realloc, awaitAll, cancel)
- [x] All four validated against standalone C test suite (4/4 PASS)

**Todo:**
- [x] Parser: `Chan<T>` generic type, `select { }` statement, `for msg in ch` desugaring
- [x] Typechecker: Chan<T> methods, select arm inference, for-in channel
- [x] Mono: Chan<T> instantiation, select lowering
- [x] C emitter: haki_chan_send/receive, select block, taskgroup emit (integration tested)
- [x] stdlib: `std/sync` module (chan, unbounded, group, collect, feed, fanOut, pipe)

### v2.4 — Language Ergonomics ✅ COMPLETE
- [x] Compound assignment (`+=`, `-=`, `*=`, `/=`, `%=`) — desugared at parse time
- [x] Ternary (`cond ? then : else`) — parsed as infix, lowers to ExprKind::If
- [x] F-strings (`f"Hello {name}, count: {n}"`) — lexer segments, parser concatenates, typechecker coerces
- [ ] `for` destructuring (`for (k, v) in map`) — v2.5
- [ ] `where` clauses on generics — v2.5

### v2.5 — Stdlib expansion ✅ COMPLETE
- [x] `std/test` — assertTrue, assertEq, assertContains, assertLen etc (191 lines)
- [x] `std/fmt` — hex, bin, oct, padLeft/Right, center, table, duration, bytes (203 lines)
- [x] `std/net` — TcpStream, TcpListener, tcpConnect, tcpListen (114 lines)
- [x] `std/crypto` — sha256, hmacSha256, base64Encode/Decode, base64UrlEncode (74 lines)
- [x] C runtime: haki_net_* (POSIX sockets), haki_crypto_sha256, hmac_sha256, base64 (self-contained)
- [x] All crypto vectors verified against Python hashlib (sha256 empty/abc/hello world, RFC 4648 base64)

### v2.6 — Database ✅ COMPLETE
- [x] `PgPool` — `Chan<int>`-backed connection pool (acquire/release/withConn/close)
- [x] `pgPool(connString, size)` — opens N connections, fills channel at startup
- [x] `QueryBuilder` — fluent SELECT/INSERT/UPDATE with parameterized `$1` placeholders
- [x] `Migrator` — tracks applied migrations in `_haki_migrations`, applyOne/rollbackOne/status
- [x] Convenience: `fetchAll`, `fetchOne`, `fetchScalar`, `begin`/`commit`/`rollback`, `transaction`
- [x] Injection-safe by construction — user values always in params[], never interpolated into SQL
- [x] SQL generation logic validated (select+where+order+limit, insert, update, injection prevention)

### v2.7 — DX ✅ COMPLETE
- [x] `hakic watch <file>` — 100ms stat polling, kills/restarts child on change, portable (no inotify/kqueue)
- [x] `hakic repl` — sentinel-based incremental eval, decl vs stmt classification, :quit/:clear/:show/:help
- [x] REPL state model: decls persist (fn/class/import), stmts accumulate in main(), error = no commit
- [x] REPL logic validated (sentinel split, decl-only, multi-stmt isolation, classification)
- [ ] Error messages with source context — v2.8

### v2.8 — Debug + Profiling ✅ COMPLETE
- [x] Rich source-context errors — Rust/Elm-style: error + --> file:line:col + source line + caret
- [x] `format_error` rewritten to extract Span offsets and render full diagnostic block
- [x] `#line` directive injection — cemit emits `/* haki span:N */` markers, driver replaces
      them with `#line L "source.haki"` so gcc embeds DWARF pointing to .haki source
- [x] `-g -O0` flags passed to gcc when debugging so DWARF is fully populated
- [x] Both features validated: caret lands on correct token, span→line mapping correct
- [ ] `hakic profile` (perf/instruments integration) — post-v3.0
- [ ] VS Code DAP adapter — post-v3.0

### v2.x — haki_ui (Virtual Tree renderer) ✅ COMPLETE
- [x] `Element` enum — developer-facing API (Text/Button/Column/Row/Spacer/...)
- [x] `VNode` class — ARC-managed virtual tree node, GTK never sees VNode pointers
- [x] `Mutation` enum — SetText/InsertChild/RemoveChild/CreateLabel/CreateButton/CreateBox
- [x] `buildVtree(elem, counter)` — converts Element → VNode with stable node_ids
- [x] `diff(old, new, mutations)` — produces minimal Array<Mutation> between two VNode trees
- [x] `appendCreateMutations` — handles new subtree creation in diff output
- [x] `applyMutation/applyMutations` — dispatches mutations to C platform layer
- [x] `State<T>.__notify` — closure called by set() to trigger App re-render cycle
- [x] `haki_ui_gtk.c` rewritten — JSON bridge gone, pure surgical integer mutation API:
      haki_gtk_create_window/label/button/box + haki_gtk_set_text/insert/remove_child
- [x] `haki_set_callback_dispatcher` — single C function pointer Haki registers pre-gtk_main
- [x] node_id boundary validated: integer-only FFI, no Haki memory crosses to GTK
- [x] Diff algorithm validated: 5/5 cases (text change, no-op, add, remove, boundary)

**v3.0 gate: all v2.x milestones complete. Ready for hardware testing.**

---

## The v3.0 gate

v3.0 ships when ALL of the following are true:
- Full test suite passes on macOS arm64, Linux x86_64, Linux arm64
- haki_ui counter app opens a real GTK window
- `haki` and `hakic` install cleanly via Homebrew with no manual steps
- Language spec locked (no breaking syntax changes after v3.0)
- stdlib API locked

**No hardware testing until v3.0.**
