# Haki

A statically-typed, ARC-managed general-purpose scripting language.

## Status

v0.1 — bootstrap phase. Compiler written in Rust.

## Build

```bash
cargo build
```

## Run

```bash
cargo run --bin hakic -- your_file.haki
```

## Project structure

```
crates/
  haki_ast/      AST node definitions (shared)
  haki_lexer/    Lexer — source text → token stream
  haki_parser/   Parser — token stream → AST  (next)
  haki_typeck/   Type checker + inference      (stub)
  haki_mono/     Monomorphization engine       (stub)
  haki_codegen/  LLVM IR emission              (stub)
  haki_wasm/     WebAssembly backend           (stub)
  haki_stdlib/   Standard library              (stub)
hakic/           Compiler binary
```

