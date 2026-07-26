# Haki

**A statically-typed, ARC-managed, general-purpose language.**

```haki
fn fibonacci(n: int) -> int {
    if n <= 1 { return n }
    return fibonacci(n - 1) + fibonacci(n - 2)
}

fn main() {
    let i = 0
    while i < 10 {
        print_int(fibonacci(i))
        i = i + 1
    }
}
```

```bash
haki fib.haki
# 0 1 1 2 3 5 8 13 21 34
```

## What Haki is

Haki is a compiled language with C-style syntax, no semicolons, and no garbage collector. It targets native binaries (via LLVM), portable C source, and WebAssembly. ARC handles memory for heap-allocated types; structs live on the stack. The compiler is written in Haki — the bootstrap is complete as of v2.1.

## Install

```bash
brew tap iceman5508/tap
brew install haki
```

Or build from source:

```bash
LLVM_SYS_170_PREFIX=$(brew --prefix llvm@17) cargo build --release --bin hakic
```

Requires: Rust 1.75+, LLVM 17 (macOS/Linux), gcc or clang.

## Quick start

```bash
haki hello.haki          # compile and run immediately
hakic check hello.haki   # typecheck only (fast)
hakic test hello.haki    # run fn test_*() functions
hakic --emit-c hello.haki  # emit portable C source
```

## Language overview

### Types

```haki
int   float   bool   string   void
T?                     // optional (null allowed)
Array<T>               // heap array, ARC-managed
Map<string, V>         // string-keyed hash map
fn(A, B) -> R          // first-class function type
Task<T>                // async result
Mutex<T>               // shared mutable state
```

### Variables, functions, control flow

```haki
const x = 42                          // immutable
let count = 0                         // mutable
const data, err = readFile("f.json")  // multi-return destructuring
const _, err2 = riskyOp()            // explicit discard

fn add(a: int, b: int) -> int { return a + b }
fn divmod(a: int, b: int) -> (int, int) { return a/b, a%b }
fn best<T: Comparable>(a: T, b: T) -> T { ... }

if x > 0 { ... } else { ... }
while i < 10 { i = i + 1 }
for item in items { ... }
defer closeFile(f)    // runs before every return path
```

### Match

```haki
// Enum variants
const msg = match result {
    Ok(v)  { yield "got: " + v }
    Err(e) { yield "error: " + e }
}

// Integers and strings (wildcard required)
match code {
    200 { print("OK") }
    404 { print("Not Found") }
    _   { print("unknown") }
}
```

### Structs, classes, ARC

```haki
struct Point { const x: float   const y: float }   // stack, copied

class User {                                         // heap, ARC
    const name: string
    let score: int
    weak team: Team?    // weak reference — breaks retain cycles
}

impl Printable for User {
    fn toString() -> string { return "User(" + name + ")" }
}
```

### Error handling

```haki
fn divide(a: int, b: int) -> (int, Error?) {
    if b == 0 { return 0, Error(message: "div by zero") }
    return a / b, null
}

const result, err = divide(10, 0)
if err != null { panic(err.message) }
```

### Concurrency

```haki
// async is a call-site modifier — any function can be called async
const task = async fetchUser(42)
const user, err = task.await()
_ = async logEvent("ping")    // detached (explicit discard)

const counter: Mutex<int> = Mutex(0)
const guard = counter.lock()
guard.value = guard.value + 1
```

### C FFI

```haki
@link("m")
extern "c" fn sqrt(x: float) -> float

@link("sqlite3")
extern "c" fn sqlite3_open(path: string, db: int) -> int

fn main() {
    const root = sqrt(2.0)
    print(float_to_string(root))   // 1.4142135...
}
```

No Makefile. No build scripts. `haki myfile.haki` handles `-lm` automatically from `@link`.

### Web deployment

```haki
// handler.haki — compile to .so, deploy behind Apache or nginx
fn handle(req: HttpRequest) -> HttpResponse {
    if req.path == "/health" {
        return HttpResponse(status: 200, body: "ok")
    }
    return HttpResponse(status: 404, body: "not found")
}
```

```bash
haki-web handler.haki -o handler.so   # compile to shared library
# Drop handler.so behind mod_haki (Apache) or haki_fastcgi (nginx)
```

### Native UI

```haki
// counter.haki — identical source for desktop and browser
class CounterApp {
    const count = makeState(0)

    fn body() -> Element {
        return column([
            text(f"Count: {count.value}"),
            button("Increment", fn() { count.set(count.value + 1) })
        ])
    }
}

fn main() {
    App(root: CounterApp(), title: "Counter", width: 400, height: 300).run()
}
```

```bash
haki-gtk counter.haki    # native desktop (GTK 3)
haki-dom counter.haki    # WebAssembly for browser
```

## Modules and packages

```haki
import "std/path"    as path
import "std/json"    as json
import "std/time"    as time
import "./mymodule"  as mod
import "pkg/utils"   as utils    // package dependency
```

```bash
hakic pkg init myapp       # create haki.json
hakic pkg add <github-url> # add dependency
hakic pkg install          # install from haki.json + haki.lock
```

## Standard library

| Module | Contents |
|--------|----------|
| `std/path` | path.join, dir, base, ext, isAbsolute |
| `std/env` | get, set, cwd, home, args |
| `std/time` | nowMs, nowSec, sleepMs, format |
| `std/process` | run, exec, shell, exit |
| `std/regex` | matches, find, replaceAll, split |
| `std/json` | encode, decode, string, int, bool |
| `std/math` | abs, max, min, clamp, pow |
| `std/strings` | repeat, join, padLeft, padRight |
| `std/db` | postgres and sqlite bindings |

## Bootstrap status

As of v2.1, the Haki compiler is written in Haki. The bootstrap is proven: Stage 0 (Rust compiler) and Stage 1 (Haki-compiled compiler) produce identical output for the same source programs. The Rust codebase remains as reference; the active compiler is self-hosted.

```
Stage 0:  Rust hakic compiles haki_bootstrap/ → hakic_s1 binary
Stage 1:  hakic_s1 compiles haki programs correctly
Proof:    fibonacci / strings / primes — Stage 0 == Stage 1 output ✓
```

## Design principles

| Decision | Rationale |
|----------|-----------|
| No semicolons | Noise with no semantic value |
| No parens on control flow | Redundant given `{}` |
| Explicit `return` | Prevents accidental return bugs |
| Multi-return tuples | Type signature shows what can fail |
| No error propagation `?` | Friction around errors is intentional |
| ARC, no GC | Predictable latency; works in Wasm |
| `async` at call site | Any function is callable async; no ecosystem split |
| Explicit closure captures | `fn[x, weak self](args)` — no hidden captures |

## Building from source

```bash
git clone https://github.com/iceman5508/haki-lang
cd haki-lang

# macOS (requires LLVM 17 via Homebrew)
brew install llvm@17
LLVM_SYS_170_PREFIX=$(brew --prefix llvm@17) cargo build --release --bin hakic

# Linux (requires LLVM 17)
apt install llvm-17-dev
LLVM_SYS_170_PREFIX=/usr/lib/llvm-17 cargo build --release --bin hakic

# Windows (C backend only, no LLVM required)
cargo build --release --bin hakic

# Run tests
LLVM_SYS_170_PREFIX=$(brew --prefix llvm@17) cargo test --workspace
```

## License

MIT
