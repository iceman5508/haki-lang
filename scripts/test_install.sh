#!/bin/bash
# test_install.sh — End-to-end installation verification for Haki v2.1.0
# Run after `brew install haki` to verify the installation is correct.
# Usage: bash scripts/test_install.sh
# Expected: all tests pass and script exits 0

set -e

PASS=0
FAIL=0
TMPDIR_TEST=$(mktemp -d)
trap "rm -rf $TMPDIR_TEST" EXIT

green() { printf "\033[32m✓\033[0m %s\n" "$1"; }
red()   { printf "\033[31m✗\033[0m %s\n" "$1"; FAIL=$((FAIL + 1)); }

check() {
    local name="$1"
    local result="$2"
    local expected="$3"
    if [ "$result" = "$expected" ]; then
        green "$name"
        PASS=$((PASS + 1))
    else
        red "$name"
        echo "    got:      '$result'"
        echo "    expected: '$expected'"
    fi
}

echo "Haki v2.1.0 Installation Test"
echo "=============================="
echo ""

# 1. Version
echo "Binary:"
VER=$(haki --version 2>/dev/null | head -1)
check "  haki --version reports 2.1.0" "$VER" "haki 2.1.0 — Haki compiler"

echo ""
echo "Compilation:"

# 2. Hello world
cat > $TMPDIR_TEST/hello.haki << 'EOF'
fn main() { print("Hello from Haki!") }
EOF
OUT=$(haki $TMPDIR_TEST/hello.haki 2>/dev/null)
check "  hello world" "$OUT" "Hello from Haki!"

# 3. Integer arithmetic (fibonacci)
cat > $TMPDIR_TEST/fib.haki << 'EOF'
fn fibonacci(n: int) -> int {
    if n <= 1 { return n }
    return fibonacci(n - 1) + fibonacci(n - 2)
}
fn main() {
    let i = 0
    while i < 8 { print_int(fibonacci(i))  i = i + 1 }
}
EOF
OUT=$(haki $TMPDIR_TEST/fib.haki 2>/dev/null | tr '\n' ' ' | sed 's/ $//')
check "  fibonacci (int arithmetic + recursion)" "$OUT" "0 1 1 2 3 5 8 13"

# 4. String concatenation
cat > $TMPDIR_TEST/strings.haki << 'EOF'
fn greet(name: string) -> string { return "Hello, " + name + "!" }
fn main() { print(greet("World"))  print(greet("Haki")) }
EOF
OUT=$(haki $TMPDIR_TEST/strings.haki 2>/dev/null | tr '\n' '|')
check "  string concatenation" "$OUT" "Hello, World!|Hello, Haki!|"

# 5. Structs
cat > $TMPDIR_TEST/struct.haki << 'EOF'
struct Point { const x: float  const y: float }
fn dist2(p: Point) -> float { return p.x * p.x + p.y * p.y }
fn main() { const p = Point(x: 3.0, y: 4.0)  print_float(dist2(p)) }
EOF
OUT=$(haki $TMPDIR_TEST/struct.haki 2>/dev/null)
check "  structs and field access" "$OUT" "25"

# 6. Arrays
cat > $TMPDIR_TEST/array.haki << 'EOF'
fn main() {
    let nums: Array<int> = []
    let i = 0
    while i < 5 { nums.append(i * i)  i = i + 1 }
    for n in nums { print_int(n) }
}
EOF
OUT=$(haki $TMPDIR_TEST/array.haki 2>/dev/null | tr '\n' ' ' | sed 's/ $//')
check "  arrays (append + for loop)" "$OUT" "0 1 4 9 16"

# 7. Multi-return + errors
cat > $TMPDIR_TEST/errors.haki << 'EOF'
fn safeDivide(a: int, b: int) -> (int, Error?) {
    if b == 0 { return 0, Error(message: "division by zero") }
    return a / b, null
}
fn main() {
    const r1, e1 = safeDivide(10, 2)
    if e1 != null { print("error") } else { print_int(r1) }
    const r2, e2 = safeDivide(10, 0)
    if e2 != null { print(e2.message) } else { print_int(r2) }
}
EOF
OUT=$(haki $TMPDIR_TEST/errors.haki 2>/dev/null | tr '\n' '|')
check "  multi-return + error handling" "$OUT" "5|division by zero|"

# 8. Match
cat > $TMPDIR_TEST/match.haki << 'EOF'
fn classify(n: int) -> string {
    return match n {
        1 { yield "one" }  2 { yield "two" }  _ { yield "other" }
    }
}
fn main() { print(classify(1))  print(classify(2))  print(classify(5)) }
EOF
OUT=$(haki $TMPDIR_TEST/match.haki 2>/dev/null | tr '\n' '|')
check "  match expression (integer)" "$OUT" "one|two|other|"

echo ""
echo "Tooling:"

# 9. hakic check
hakic check $TMPDIR_TEST/hello.haki > /dev/null 2>&1
check "  hakic check" "0" "0"

# 10. Stdlib import
cat > $TMPDIR_TEST/stdlib.haki << 'EOF'
import "std/math" as math
fn main() { print_int(math.abs(-42))  print_int(math.max(3, 7)) }
EOF
OUT=$(haki $TMPDIR_TEST/stdlib.haki 2>/dev/null | tr '\n' ' ' | sed 's/ $//')
check "  std/math import" "$OUT" "42 7"

# 11. C FFI + @link
cat > $TMPDIR_TEST/ffi.haki << 'EOF'
@link("m")
extern "c" fn sqrt(x: float) -> float
fn main() { print_float(sqrt(9.0)) }
EOF
OUT=$(haki $TMPDIR_TEST/ffi.haki 2>/dev/null)
check "  extern \"c\" + @link" "$OUT" "3"

# 12. emit-c
haki $TMPDIR_TEST/hello.haki --emit-c -o $TMPDIR_TEST/hello_out > /dev/null 2>&1
if [ -f "$TMPDIR_TEST/hello_out" ]; then
    green "  --emit-c produces binary"
    PASS=$((PASS + 1))
else
    red "  --emit-c produces binary"
fi

# Summary
echo ""
echo "══════════════════════════════════════"
TOTAL=$((PASS + FAIL))
if [ $FAIL -eq 0 ]; then
    printf "\033[32mAll $PASS/$TOTAL tests passed\033[0m\n"
    printf "Haki v2.1.0 is correctly installed.\n"
    exit 0
else
    printf "\033[31m$FAIL/$TOTAL tests failed\033[0m ($PASS passed)\n"
    exit 1
fi
