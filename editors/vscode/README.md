# Haki for VS Code

Language support for [Haki](https://github.com/iceman5508/haki-lang) — a statically-typed,
ARC-managed, general-purpose language.

## Features

- **Syntax highlighting** — keywords, types, functions, strings, comments
- **Inline diagnostics** — parse errors and type errors shown as you type
- **Hover types** — hover any expression to see its inferred type
- **Go-to-definition** — jump to function, struct, class, and enum definitions

## Requirements

`hakic` must be installed and on your `PATH`:

```bash
brew tap iceman5508/tap
brew install haki
```

Or build from source:

```bash
git clone https://github.com/iceman5508/haki-lang
cd haki-lang
LLVM_SYS_170_PREFIX=$(brew --prefix llvm@17) cargo build --release --bin hakic
# Add target/release/ to PATH
```

## Configuration

| Setting | Default | Description |
|---|---|---|
| `haki.server.path` | `"hakic"` | Path to the hakic binary if not on PATH |
| `haki.trace.server` | `"off"` | LSP trace level (`off`/`messages`/`verbose`) |

## Commands

| Command | Description |
|---|---|
| `Haki: Restart Language Server` | Restart `hakic lsp` without reloading VS Code |

## Installation (from source)

```bash
cd editors/vscode
npm install
npm run compile
# Then install via "Extensions: Install from VSIX..." in VS Code
# after running: npx vsce package
```
