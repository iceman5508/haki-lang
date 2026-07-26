## Haki v2.1.0 — Bootstrap Complete

The Haki compiler is now fully self-hosting. A Haki-compiled binary produces identical output to the Rust-compiled binary for all programs including integer arithmetic, recursion, string operations, and complex control flow.

### Installation

**macOS (Homebrew)**
```
brew tap iceman5508/tap
brew install haki
```

**Linux / macOS (curl)**
```
curl -fsSL https://raw.githubusercontent.com/iceman5508/haki-lang/main/install.sh | sh
```

**Windows**
```
winget install haki-lang.haki
```

### What's new in v2.1

- **Bootstrap proof** — Stage 0 (Rust) and Stage 1 (Haki-compiled) produce identical output for fibonacci, string operations, and primes sieve
- **Type inference pass** — `haki_typeinfer.haki` threads type information through the bootstrap C emitter; `i + 1` now emits integer add, not string concat
- **Typed variable declarations** — `let i = 2` now emits `int64_t i` not `void* i`
- **Import resolution fix** — bootstrap correctly loads `.haki` extension files with cycle detection

### Requirements

- gcc or clang (pre-installed on macOS/Linux; included in MinGW on Windows)
- No LLVM or Rust required for end users

### Binary aliases

| Command | Use case |
|---------|---------|
| `haki hello.haki` | Run a Haki script immediately |
| `haki-gtk app.haki` | Compile and run a native GTK desktop app |
| `haki-dom app.haki` | Compile to WebAssembly for browser |
| `haki-web api.haki` | Compile to .so for Apache/nginx deployment |
| `hakic check src.haki` | Fast typecheck (no compilation) |

### Quick start

```haki
fn main() {
    print("Hello from Haki " + "v2.1!")
}
```

```bash
haki hello.haki
# Hello from Haki v2.1!
```

See the [CHANGELOG](CHANGELOG.md) for the full history and [README](README.md) for language documentation.
