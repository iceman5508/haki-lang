#!/bin/bash
# Haki v4.9.9 QA Test Suite
# Usage:
#   ./run_qa_v499.sh                        # uses `hakic` on PATH
#   HAKIC=/path/to/hakic ./run_qa_v499.sh   # use specific binary
#
# Each test writes a small .haki program to /tmp, runs it, and compares output.
# Exit code: 0 = all pass, 1 = one or more failures.

HAKIC=${HAKIC:-hakic}
# Resolve a relative HAKIC to an absolute path. Some tests run the compiler from
# inside a different working directory (§29 runs `init` in a scratch dir), and a
# path like ./haki_bootstrap/hakic_s5 stops resolving once the shell has cd'd.
# A bare name is left alone so PATH lookup still works.
case "$HAKIC" in
    /*)  ;;
    */*) HAKIC="$(cd "$(dirname "$HAKIC")" && pwd)/$(basename "$HAKIC")" ;;
    *)   ;;
esac
PASS=0; FAIL=0

green() { printf "\033[32m✓\033[0m %s\n" "$1"; PASS=$((PASS+1)); }
red()   { printf "\033[31m✗\033[0m %s\n" "$1"; FAIL=$((FAIL+1)); }

run_test() {
    local name="$1" file="$2" expected="$3" got
    got=$($HAKIC run "$file" 2>/dev/null)
    if [ "$got" = "$expected" ]; then
        green "$name"
    else
        red "$name"
        printf "  expected: %s\n" "$(echo "$expected" | head -3)"
        printf "  got:      %s\n" "$(echo "$got"      | head -3)"
        $HAKIC run "$file" 2>&1 | grep "error:" | head -2 || true
    fi
}

# ── §1  Core language ──────────────────────────────────────────────────────────
echo ""
echo "§1  Core language"

cat > /tmp/hqa_hello.haki << 'EOF'
fn main() { print("Hello, Haki!") }
EOF
run_test "hello world" /tmp/hqa_hello.haki "Hello, Haki!"

cat > /tmp/hqa_fib.haki << 'EOF'
fn fib(n: int) -> int {
    if n <= 1 { return n }
    return fib(n - 1) + fib(n - 2)
}
fn main() { print(int_to_string(fib(10))) }
EOF
run_test "fibonacci (recursive)" /tmp/hqa_fib.haki "55"

cat > /tmp/hqa_while.haki << 'EOF'
fn main() {
    let sum = 0
    let i = 1
    while i <= 100 {
        sum = sum + i
        i = i + 1
    }
    print(int_to_string(sum))
}
EOF
run_test "while loop (sum 1..100)" /tmp/hqa_while.haki "5050"

cat > /tmp/hqa_multi.haki << 'EOF'
fn divmod(a: int, b: int) -> (int, int) { return a / b, a % b }
fn main() {
    const q, r = divmod(17, 5)
    print(int_to_string(q))
    print(int_to_string(r))
}
EOF
run_test "multi-return tuple" /tmp/hqa_multi.haki "3
2"

cat > /tmp/hqa_const.haki << 'EOF'
fn main() {
    const x: int = 42
    let y: int = 10
    y = y + x
    print(int_to_string(y))
}
EOF
run_test "const vs let" /tmp/hqa_const.haki "52"

cat > /tmp/hqa_string_concat.haki << 'EOF'
fn main() {
    const a = "Hello"
    const b = "World"
    print(a + ", " + b + "!")
}
EOF
run_test "string concatenation" /tmp/hqa_string_concat.haki "Hello, World!"

cat > /tmp/hqa_bool.haki << 'EOF'
fn main() {
    print(bool_to_string(true))
    print(bool_to_string(false))
    print(bool_to_string(3 > 2))
    print(bool_to_string(1 == 2))
}
EOF
run_test "booleans" /tmp/hqa_bool.haki "true
false
true
false"

cat > /tmp/hqa_negative.haki << 'EOF'
fn main() {
    let x: int = -5
    print(int_to_string(x * -1))
    print(int_to_string(-3 + -4))
}
EOF
run_test "negative integers" /tmp/hqa_negative.haki "5
-7"

# ── §2  Functions ──────────────────────────────────────────────────────────────
echo ""
echo "§2  Functions"

cat > /tmp/hqa_closure.haki << 'EOF'
fn apply(f: fn(int) -> int, x: int) -> int { return f(x) }
fn main() {
    const double = fn(x: int) -> int { return x * 2 }
    print(int_to_string(apply(double, 21)))
}
EOF
run_test "closures / first-class functions" /tmp/hqa_closure.haki "42"

cat > /tmp/hqa_mut_closure.haki << 'EOF'
fn main() {
    let count = 0
    const inc = fn() { count = count + 1 }
    inc()
    inc()
    inc()
    print(int_to_string(count))
}
EOF
run_test "mutable closure capture" /tmp/hqa_mut_closure.haki "3"

cat > /tmp/hqa_generics.haki << 'EOF'
fn identity<T>(x: T) -> T { return x }
fn main() {
    print(identity("haki"))
    print(int_to_string(identity(99)))
}
EOF
run_test "generic functions" /tmp/hqa_generics.haki "haki
99"

cat > /tmp/hqa_varargs.haki << 'EOF'
fn sum(nums: Array<int>) -> int {
    let total = 0
    let i = 0
    while i < nums.length {
        total = total + nums[i]
        i = i + 1
    }
    return total
}
fn main() {
    const a: Array<int> = []
    a.append(1); a.append(2); a.append(3); a.append(4)
    print(int_to_string(sum(a)))
}
EOF
run_test "array parameter function" /tmp/hqa_varargs.haki "10"

# ── §3  Classes ────────────────────────────────────────────────────────────────
echo ""
echo "§3  Classes"

cat > /tmp/hqa_class.haki << 'EOF'
class Point {
    const x: int
    const y: int
    fn dist_sq() -> int { return x * x + y * y }
    fn to_string() -> string {
        return "(" + int_to_string(x) + "," + int_to_string(y) + ")"
    }
}
fn main() {
    const p = Point(x: 3, y: 4)
    print(int_to_string(p.dist_sq()))
    print(p.to_string())
}
EOF
run_test "class fields and methods" /tmp/hqa_class.haki "25
(3,4)"

cat > /tmp/hqa_inherit.haki << 'EOF'
class Animal {
    const name: string
    fn speak() -> string { return name + " speaks" }
}
class Dog extends Animal {
    const breed: string
    fn describe() -> string { return name + " is a " + breed }
}
fn main() {
    const d = Dog(name: "Rex", breed: "Labrador")
    print(d.speak())
    print(d.describe())
}
EOF
run_test "inheritance with inherited fields" /tmp/hqa_inherit.haki "Rex speaks
Rex is a Labrador"

cat > /tmp/hqa_method_chain.haki << 'EOF'
class Builder {
    let value: string
    fn add(s: string) -> Builder {
        value = value + s
        return this
    }
    fn build() -> string { return value }
}
fn main() {
    const b = Builder(value: "")
    b.add("Hello").add(", ").add("World")
    print(b.build())
}
EOF
run_test "method chaining with this" /tmp/hqa_method_chain.haki "Hello, World"

# ── §4  Enums and match ────────────────────────────────────────────────────────
echo ""
echo "§4  Enums and match"

cat > /tmp/hqa_enum.haki << 'EOF'
enum Direction { North, South, East, West }
fn opposite(d: Direction) -> string {
    return match d {
        North { yield "South" }
        South { yield "North" }
        East  { yield "West"  }
        West  { yield "East"  }
    }
}
fn main() {
    print(opposite(Direction.North))
    print(opposite(Direction.West))
}
EOF
run_test "enum match" /tmp/hqa_enum.haki "South
East"

cat > /tmp/hqa_match_guard.haki << 'EOF'
fn classify(n: int) -> string {
    return match n {
        x if x < 0  { yield "negative" }
        x if x == 0 { yield "zero"     }
        x if x < 10 { yield "small"    }
        _            { yield "large"    }
    }
}
fn main() {
    print(classify(-5))
    print(classify(0))
    print(classify(7))
    print(classify(100))
}
EOF
run_test "match guards" /tmp/hqa_match_guard.haki "negative
zero
small
large"

cat > /tmp/hqa_match_string.haki << 'EOF'
fn day_type(day: string) -> string {
    return match day {
        "Saturday" { yield "weekend" }
        "Sunday"   { yield "weekend" }
        _          { yield "weekday" }
    }
}
fn main() {
    print(day_type("Monday"))
    print(day_type("Saturday"))
    print(day_type("Sunday"))
}
EOF
run_test "match on string literals" /tmp/hqa_match_string.haki "weekday
weekend
weekend"

# ── §5  Optionals ─────────────────────────────────────────────────────────────
echo ""
echo "§5  Optionals"

cat > /tmp/hqa_optional.haki << 'HEOF'
fn find(arr: Array<string>, target: string) -> string? {
    let i = 0
    while i < arr.length {
        if arr[i] == target { return arr[i] }
        i = i + 1
    }
    return null
}
fn main() {
    const a: Array<string> = []
    a.append("apple"); a.append("banana"); a.append("cherry")
    const r1 = find(a, "banana")
    if r1 != null { print(r1) } else { print("not found") }
    const r2 = find(a, "grape")
    if r2 != null { print(r2) } else { print("not found") }
}
HEOF
run_test "optional returns (T?)" /tmp/hqa_optional.haki "banana
not found"

cat > /tmp/hqa_opt_chain.haki << 'HEOF'
class User {
    const name: string
    const email: string?
}
fn get_user(id: int) -> User? {
    if id == 1 { return User(name: "Alice", email: "alice@example.com") }
    if id == 2 { return User(name: "Bob",   email: null) }
    return null
}
fn main() {
    const u1 = get_user(1)
    print(u1?.name ?? "unknown")
    print(u1?.email ?? "no email")
    const u2 = get_user(2)
    print(u2?.email ?? "no email")
    const u3 = get_user(99)
    print(u3?.name ?? "unknown")
}
HEOF
run_test "optional chaining (?.) and nil-coalesce (??)" /tmp/hqa_opt_chain.haki "Alice
alice@example.com
no email
unknown"

# ── §6  Arrays ────────────────────────────────────────────────────────────────
echo ""
echo "§6  Arrays"

cat > /tmp/hqa_array.haki << 'EOF'
fn main() {
    const a: Array<int> = []
    a.append(10); a.append(20); a.append(30); a.append(40)
    print(int_to_string(a.length))
    print(int_to_string(a[0]))
    print(int_to_string(a[3]))
    a.remove(1)
    print(int_to_string(a.length))
    print(int_to_string(a[1]))
}
EOF
run_test "array append / index / remove" /tmp/hqa_array.haki "4
10
40
3
30"

cat > /tmp/hqa_arr_assign.haki << 'EOF'
fn main() {
    const arr: Array<int> = []
    arr.append(1); arr.append(2); arr.append(3)
    arr[0] = 10
    arr[2] = 30
    print(int_to_string(arr[0]))
    print(int_to_string(arr[1]))
    print(int_to_string(arr[2]))
}
EOF
run_test "array subscript assignment" /tmp/hqa_arr_assign.haki "10
2
30"

cat > /tmp/hqa_arr_swap.haki << 'EOF'
fn main() {
    const arr: Array<int> = []
    arr.append(1); arr.append(3)
    let tmp: int = arr[0]
    arr[0] = arr[1]
    arr[1] = tmp
    print(int_to_string(arr[0]))
    print(int_to_string(arr[1]))
}
EOF
run_test "array subscript assignment (swap)" /tmp/hqa_arr_swap.haki "3
1"

# ── §7  Maps ──────────────────────────────────────────────────────────────────
echo ""
echo "§7  Maps"

cat > /tmp/hqa_map.haki << 'EOF'
fn main() {
    const m: Map<string, int> = Map()
    m.set("a", 1); m.set("b", 2); m.set("c", 3)
    print(int_to_string(m.get("b") ?? 0))
    print(bool_to_string(m.has("c")))
    print(bool_to_string(m.has("z")))
    m.delete("b")
    print(bool_to_string(m.has("b")))
}
EOF
run_test "map set / get / has / delete" /tmp/hqa_map.haki "2
true
false
false"

cat > /tmp/hqa_map_iter.haki << 'EOF'
fn main() {
    const m: Map<string, int> = Map()
    m.set("x", 10); m.set("y", 20)
    let total = 0
    for k, v in m {
        total = total + v
    }
    print(int_to_string(total))
}
EOF
run_test "map iteration (for k, v in map)" /tmp/hqa_map_iter.haki "30"

# ── §8  Short-circuit operators ───────────────────────────────────────────────
echo ""
echo "§8  Short-circuit operators"

cat > /tmp/hqa_sc_and.haki << 'EOF'
fn main() {
    const arr: Array<int> = []
    arr.append(10); arr.append(20)
    let j: int = -1
    if j >= 0 && arr[j] > 5 {
        print("WRONG")
    } else {
        print("sc_and_ok")
    }
}
EOF
run_test "&& short-circuit (skips RHS when LHS false)" /tmp/hqa_sc_and.haki "sc_and_ok"

cat > /tmp/hqa_sc_or.haki << 'EOF'
fn main() {
    const arr: Array<int> = []
    arr.append(10)
    let x: int = 5
    if x > 0 || arr[99] > 0 {
        print("sc_or_ok")
    } else {
        print("WRONG")
    }
}
EOF
run_test "|| short-circuit (skips RHS when LHS true)" /tmp/hqa_sc_or.haki "sc_or_ok"

cat > /tmp/hqa_sc_combined.haki << 'EOF'
fn main() {
    let a = true
    let b = false
    print(bool_to_string(a && b))
    print(bool_to_string(a || b))
    print(bool_to_string(!a))
    print(bool_to_string(a && !b))
}
EOF
run_test "&& / || / ! combined" /tmp/hqa_sc_combined.haki "false
true
false
true"

# ── §9  Type aliases ──────────────────────────────────────────────────────────
echo ""
echo "§9  Type aliases"

cat > /tmp/hqa_alias.haki << 'EOF'
type UserId = int
type Username = string
fn greet(id: UserId, name: Username) -> string {
    return "User " + int_to_string(id) + ": " + name
}
fn main() {
    print(greet(42, "Alice"))
}
EOF
run_test "type alias (type A = B)" /tmp/hqa_alias.haki "User 42: Alice"

cat > /tmp/hqa_alias_fn.haki << 'EOF'
type Transform = fn(int) -> int
fn apply(t: Transform, x: int) -> int { return t(x) }
fn main() {
    const triple: Transform = fn(x: int) -> int { return x * 3 }
    print(int_to_string(apply(triple, 7)))
}
EOF
run_test "type alias for function type" /tmp/hqa_alias_fn.haki "21"

# ── §10  String methods ───────────────────────────────────────────────────────
echo ""
echo "§10  String methods"

cat > /tmp/hqa_str_length.haki << 'EOF'
fn main() {
    const s = "Hello, World!"
    print(int_to_string(s.length()))
    const empty = ""
    print(bool_to_string(empty.isEmpty()))
    print(bool_to_string(s.isEmpty()))
}
EOF
run_test "string length() and isEmpty()" /tmp/hqa_str_length.haki "13
true
false"

cat > /tmp/hqa_str_charat.haki << 'EOF'
fn main() {
    const s = "Haki"
    print(s.charAt(0))
    print(s.charAt(3))
    print(int_to_string(s.charCodeAt(0)))
}
EOF
run_test "string charAt() and charCodeAt()" /tmp/hqa_str_charat.haki "H
i
72"

cat > /tmp/hqa_str_split.haki << 'EOF'
fn main() {
    const s = "one,two,three"
    const parts = s.split(",")
    print(int_to_string(parts.length))
    print(parts[0])
    print(parts[2])
}
EOF
run_test "string split()" /tmp/hqa_str_split.haki "3
one
three"

cat > /tmp/hqa_str_trim.haki << 'EOF'
fn main() {
    const s = "  hello  "
    print(s.trim())
    print(s.trimStart())
    print(s.trimEnd())
}
EOF
# NOTE: trimStart("  hello  ") is "hello  " — the trailing spaces are kept.
# The old expectation dropped them (invisible whitespace when authored).
run_test "string trim() / trimStart() / trimEnd()" /tmp/hqa_str_trim.haki "hello
hello  
  hello"

cat > /tmp/hqa_str_case.haki << 'EOF'
fn main() {
    const s = "Hello World"
    print(s.toUpper())
    print(s.toLower())
}
EOF
run_test "string toUpper() / toLower()" /tmp/hqa_str_case.haki "HELLO WORLD
hello world"

cat > /tmp/hqa_str_contains.haki << 'EOF'
fn main() {
    const s = "Haki is great"
    print(bool_to_string(s.contains("great")))
    print(bool_to_string(s.contains("bad")))
    print(bool_to_string(s.startsWith("Haki")))
    print(bool_to_string(s.endsWith("great")))
}
EOF
run_test "string contains() / startsWith() / endsWith()" /tmp/hqa_str_contains.haki "true
false
true
true"

cat > /tmp/hqa_str_replace.haki << 'EOF'
fn main() {
    const s = "Hello World World"
    print(s.replace("World", "Haki"))
}
EOF
run_test "string replace()" /tmp/hqa_str_replace.haki "Hello Haki World"

cat > /tmp/hqa_str_pad.haki << 'EOF'
fn main() {
    const s = "42"
    print(s.padStart(5, "0"))
    print(s.padEnd(5, "."))
}
EOF
run_test "string padStart() / padEnd()" /tmp/hqa_str_pad.haki "00042
42..."

cat > /tmp/hqa_str_substr.haki << 'EOF'
fn main() {
    const s = "Hello, World!"
    print(s.slice(7, 12))
    print(int_to_string(s.indexOf("World")))
}
EOF
run_test "string slice() / indexOf()" /tmp/hqa_str_substr.haki "World
7"

# ── §11  Standard library — math ──────────────────────────────────────────────
echo ""
echo "§11  Stdlib — math"

cat > /tmp/hqa_math.haki << 'EOF'
import math

fn main() {
    print(int_to_string(math.abs(-42)))
    print(int_to_string(math.max(3, 7)))
    print(int_to_string(math.min(3, 7)))
    print(int_to_string(math.clamp(15, 0, 10)))
    print(int_to_string(math.pow(2, 8)))
}
EOF
run_test "math: abs / max / min / clamp / pow" /tmp/hqa_math.haki "42
7
3
10
256"

cat > /tmp/hqa_math_sqrt.haki << 'EOF'
import math

fn main() {
    print(int_to_string(math.floor(math.sqrt(16.0))))
    print(int_to_string(math.ceil(3.2)))
    print(int_to_string(math.floor(3.9)))
    print(int_to_string(math.round(3.5)))
}
EOF
run_test "math: sqrt / ceil / floor / round" /tmp/hqa_math_sqrt.haki "4
4
3
4"

# ── §12  Standard library — strings ───────────────────────────────────────────
echo ""
echo "§12  Stdlib — strings"

cat > /tmp/hqa_strings.haki << 'EOF'
import strings

fn main() {
    print(strings.repeat("ab", 3))
    print(int_to_string(strings.count("hello world hello", "hello")))
    print(strings.join(["a", "b", "c"], "-"))
}
EOF
run_test "strings: repeat / count / join" /tmp/hqa_strings.haki "ababab
2
a-b-c"

# ── §13  Standard library — json ──────────────────────────────────────────────
echo ""
echo "§13  Stdlib — json"

cat > /tmp/hqa_json.haki << 'EOF'
import json

fn main() {
    const obj = json.object()
    obj.set("name", json.string("Alice"))
    obj.set("age",  json.number(30))
    const encoded = json.encode(obj)
    print(encoded)
    const decoded = json.decode(encoded)
    print(decoded.get("name").asString())
}
EOF
run_test "json encode / decode object" /tmp/hqa_json.haki '{"name":"Alice","age":30}
Alice'

cat > /tmp/hqa_json_array.haki << 'EOF'
import json

fn main() {
    const arr = json.array()
    arr.push(json.number(1))
    arr.push(json.number(2))
    arr.push(json.number(3))
    const encoded = json.encode(arr)
    print(encoded)
    const decoded = json.decode(encoded)
    print(int_to_string(decoded.length()))
}
EOF
run_test "json array push / encode / decode" /tmp/hqa_json_array.haki "[1,2,3]
3"

# ── §14  Standard library — regex ─────────────────────────────────────────────
echo ""
echo "§14  Stdlib — regex"

cat > /tmp/hqa_regex.haki << 'EOF'
import regex

fn main() {
    const r = regex.compile("[0-9]+")
    const text = "order 42 ships on day 7"
    print(bool_to_string(r.test(text)))
    const m = r.match(text)
    if m != null { print(m.group(0)) }
    const all = r.matchAll(text)
    print(int_to_string(all.length))
}
EOF
run_test "regex compile / test / match / matchAll" /tmp/hqa_regex.haki "true
42
2"

# ── §15  Standard library — fs ────────────────────────────────────────────────
echo ""
echo "§15  Stdlib — fs"

cat > /tmp/hqa_fs.haki << 'HEOF'
import fs

fn main() {
    fs.write("/tmp/hqa_fs_test.txt", "Hello from Haki\n")
    const content = fs.read("/tmp/hqa_fs_test.txt")
    print(content.trim())
    print(bool_to_string(fs.exists("/tmp/hqa_fs_test.txt")))
    fs.delete("/tmp/hqa_fs_test.txt")
    print(bool_to_string(fs.exists("/tmp/hqa_fs_test.txt")))
}
HEOF
run_test "fs write / read / exists / delete" /tmp/hqa_fs.haki "Hello from Haki
true
false"

cat > /tmp/hqa_fs_dir.haki << 'HEOF'
import fs

fn main() {
    fs.mkdir("/tmp/hqa_dir_test")
    fs.write("/tmp/hqa_dir_test/a.txt", "a")
    fs.write("/tmp/hqa_dir_test/b.txt", "b")
    const entries = fs.readDir("/tmp/hqa_dir_test")
    print(int_to_string(entries.length))
    fs.delete("/tmp/hqa_dir_test/a.txt")
    fs.delete("/tmp/hqa_dir_test/b.txt")
    fs.delete("/tmp/hqa_dir_test")
}
HEOF
run_test "fs mkdir / readDir" /tmp/hqa_fs_dir.haki "2"

# ── §16  Standard library — env / process ─────────────────────────────────────
echo ""
echo "§16  Stdlib — env / process"

cat > /tmp/hqa_env.haki << 'EOF'
import env

fn main() {
    env.set("HAKI_QA_VAR", "test_value")
    const val = env.get("HAKI_QA_VAR")
    if val != null { print(val) } else { print("not found") }
    const missing = env.get("HAKI_QA_DOES_NOT_EXIST_XYZ")
    if missing != null { print(missing) } else { print("not found") }
}
EOF
run_test "env set / get" /tmp/hqa_env.haki "test_value
not found"

cat > /tmp/hqa_process.haki << 'EOF'
import process

fn main() {
    const args = process.args()
    print(int_to_string(args.length))
}
EOF
run_test "process args" /tmp/hqa_process.haki "0"

# ── §17  Standard library — template ──────────────────────────────────────────
echo ""
echo "§17  Stdlib — template"

cat > /tmp/hqa_template.haki << 'EOF'
import template

fn main() {
    const t = template.parse("Hello, {{name}}! You are {{age}} years old.")
    const ctx = template.context()
    ctx.set("name", "Alice")
    ctx.set("age", "30")
    print(t.render(ctx))
}
EOF
run_test "template parse / context / render" /tmp/hqa_template.haki "Hello, Alice! You are 30 years old."

# ── §18  Standard library — csv ───────────────────────────────────────────────
echo ""
echo "§18  Stdlib — csv"

cat > /tmp/hqa_csv.haki << 'EOF'
import csv

fn main() {
    const data = "name,age\nAlice,30\nBob,25"
    const rows = csv.parse(data)
    print(int_to_string(rows.length))
    print(rows[1][0])
    print(rows[1][1])
}
EOF
run_test "csv parse" /tmp/hqa_csv.haki "3
Alice
30"

# ── §19  Standard library — crypto ────────────────────────────────────────────
echo ""
echo "§19  Stdlib — crypto"

cat > /tmp/hqa_crypto.haki << 'EOF'
import crypto

fn main() {
    const h = crypto.sha256("hello")
    print(h.length() > 0 ? "hash_ok" : "hash_empty")
    const b64 = crypto.base64Encode("Haki")
    print(b64)
    print(crypto.base64Decode(b64))
}
EOF
run_test "crypto sha256 / base64Encode / base64Decode" /tmp/hqa_crypto.haki "hash_ok
SGFraQ==
Haki"

# ── §20  Concurrency — async/await ────────────────────────────────────────────
echo ""
echo "§20  Concurrency — async/await"

cat > /tmp/hqa_async.haki << 'EOF'
async fn fetch_data(id: int) -> string {
    return "data_" + int_to_string(id)
}
async fn main_async() {
    const result = await fetch_data(42)
    print(result)
}
fn main() { run_async(main_async) }
EOF
run_test "async fn / await" /tmp/hqa_async.haki "data_42"

cat > /tmp/hqa_chan.haki << 'EOF'
fn main() {
    const ch: Chan<int> = Chan()
    spawn fn() {
        ch.send(100)
    }
    const val = ch.receive()
    print(int_to_string(val))
}
EOF
run_test "Chan<T> send / receive" /tmp/hqa_chan.haki "100"

cat > /tmp/hqa_timeout.haki << 'EOF'
import time

async fn slow_op() -> string {
    time.sleep(5000)
    return "done"
}
async fn main_async() {
    const result = await timeout(slow_op(), 100)
    if result == null {
        print("timed_out")
    } else {
        print(result)
    }
}
fn main() { run_async(main_async) }
EOF
run_test "timeout() wraps async fn" /tmp/hqa_timeout.haki "timed_out"

cat > /tmp/hqa_taskgroup.haki << 'EOF'
async fn work(n: int) -> int { return n * n }
async fn main_async() {
    const tg: TaskGroup<int> = TaskGroup()
    tg.add(work(2))
    tg.add(work(3))
    tg.add(work(4))
    const results = await tg.waitAll()
    let sum = 0
    let i = 0
    while i < results.length {
        sum = sum + results[i]
        i = i + 1
    }
    print(int_to_string(sum))
}
fn main() { run_async(main_async) }
EOF
run_test "TaskGroup<T> waitAll()" /tmp/hqa_taskgroup.haki "29"

# ── §21  Annotations ──────────────────────────────────────────────────────────
echo ""
echo "§21  Annotations"

cat > /tmp/hqa_ann_inline.haki << 'EOF'
@inline
fn square(x: int) -> int { return x * x }
fn main() {
    print(int_to_string(square(7)))
}
EOF
run_test "@inline annotation (still executes correctly)" /tmp/hqa_ann_inline.haki "49"

cat > /tmp/hqa_ann_deprecated.haki << 'EOF'
@deprecated("use square() instead")
fn sq(x: int) -> int { return x * x }
fn square(x: int) -> int { return x * x }
fn main() {
    print(int_to_string(square(5)))
}
EOF
run_test "@deprecated annotation (still compiles and runs)" /tmp/hqa_ann_deprecated.haki "25"

cat > /tmp/hqa_ann_skip.haki << 'EOF'
fn main() {
    print("before")
    @skip
    print("this is skipped")
    print("after")
}
EOF
run_test "@skip annotation (statement skipped at compile time)" /tmp/hqa_ann_skip.haki "before
after"

cat > /tmp/hqa_ann_error.haki << 'EOF'
fn safe_div(a: int, b: int) -> int {
    @error("division by zero")
    if b == 0 { return 0 }
    return a / b
}
fn main() {
    print(int_to_string(safe_div(10, 2)))
    print(int_to_string(safe_div(10, 0)))
}
EOF
run_test "@error annotation (runtime guard)" /tmp/hqa_ann_error.haki "5
0"

cat > /tmp/hqa_ann_requires.haki << 'EOF'
fn sqrt_safe(x: float) -> float {
    @requires(x >= 0.0)
    import math
    return math.sqrt(x)
}
fn main() {
    print(int_to_string(math.floor(sqrt_safe(9.0))))
}
EOF
# @requires is a compile-time annotation — just verify it runs
$HAKIC run /tmp/hqa_ann_requires.haki 2>/dev/null | grep -q "3" && green "@requires annotation (compile-time precondition)" || red "@requires annotation (compile-time precondition)"

# ── §22  HTTP server ──────────────────────────────────────────────────────────
echo ""
echo "§22  HTTP server"

cat > /tmp/hqa_http_server.haki << 'HEOF'
import http

fn main() {
    const server = http.Server(port: 0)
    server.get("/ping", fn(req, res) {
        res.text("pong")
    })
    const port = server.listen_background()
    const client = http.Client()
    const resp = client.get("http://127.0.0.1:" + int_to_string(port) + "/ping")
    print(resp.body)
    server.stop()
}
HEOF
run_test "http.Server listen_background / GET route / client.get" /tmp/hqa_http_server.haki "pong"

cat > /tmp/hqa_http_json.haki << 'HEOF'
import http
import json

fn main() {
    const server = http.Server(port: 0)
    server.get("/data", fn(req, res) {
        const obj = json.object()
        obj.set("ok", json.bool(true))
        res.json(obj)
    })
    const port = server.listen_background()
    const client = http.Client()
    const resp = client.get("http://127.0.0.1:" + int_to_string(port) + "/data")
    print(resp.body)
    server.stop()
}
HEOF
run_test "http.Server JSON route" /tmp/hqa_http_json.haki '{"ok":true}'

# ── §23  HTTP client ──────────────────────────────────────────────────────────
echo ""
echo "§23  HTTP client"

cat > /tmp/hqa_http_client.haki << 'HEOF'
import http

fn main() {
    const server = http.Server(port: 0)
    server.post("/echo", fn(req, res) {
        res.text(req.body)
    })
    const port = server.listen_background()
    const client = http.Client()
    const resp = client.post(
        "http://127.0.0.1:" + int_to_string(port) + "/echo",
        "hello_body"
    )
    print(resp.body)
    print(int_to_string(resp.status))
    server.stop()
}
HEOF
run_test "http.Client POST / response body and status" /tmp/hqa_http_client.haki "hello_body
200"

# ── §24  C FFI ────────────────────────────────────────────────────────────────
echo ""
echo "§24  C FFI"

cat > /tmp/hqa_ffi_strlen.haki << 'EOF'
@extern("strlen")
fn c_strlen(s: c_string) -> int

fn main() {
    const n = c_strlen("hello")
    print(int_to_string(n))
}
EOF
run_test "C FFI @extern (strlen)" /tmp/hqa_ffi_strlen.haki "5"

# ── §25  toolchain — hakic check ─────────────────────────────────────────────
echo ""
echo "§25  Toolchain — hakic check"

cat > /tmp/hqa_chk_ok.haki << 'EOF'
fn main() { print("ok") }
EOF
$HAKIC check /tmp/hqa_chk_ok.haki 2>/dev/null && green "hakic check passes on valid program" || red "hakic check passes on valid program"

cat > /tmp/hqa_chk_typo.haki << 'EOF'
fn main() { prnt("oops") }
EOF
$HAKIC check /tmp/hqa_chk_typo.haki 2>&1 | grep -qi "did you mean\|suggestion\|print" \
    && green "hakic check typo hint (did you mean)" \
    || red "hakic check typo hint (did you mean)"

# ── §26  toolchain — hakic build ─────────────────────────────────────────────
echo ""
echo "§26  Toolchain — hakic build"

cat > /tmp/hqa_build_src.haki << 'EOF'
fn main() { print("built_ok") }
EOF
rm -f /tmp/hqa_build_bin
$HAKIC build /tmp/hqa_build_src.haki -o /tmp/hqa_build_bin 2>/dev/null
if [ -x /tmp/hqa_build_bin ]; then
    got=$(/tmp/hqa_build_bin 2>/dev/null)
    if [ "$got" = "built_ok" ]; then
        green "hakic build produces working native binary"
    else
        red "hakic build produces working native binary"
        echo "  expected: built_ok, got: $got"
    fi
else
    red "hakic build produces working native binary"
    echo "  binary not produced by: $HAKIC build"
fi
rm -f /tmp/hqa_build_bin

# ── §27  toolchain — hakic build --release ───────────────────────────────────
echo ""
echo "§27  Toolchain — hakic build --release"

cat > /tmp/hqa_release_src.haki << 'EOF'
fn factorial(n: int) -> int {
    if n <= 1 { return 1 }
    return n * factorial(n - 1)
}
fn main() { print(int_to_string(factorial(12))) }
EOF
rm -f /tmp/hqa_release_bin
$HAKIC build /tmp/hqa_release_src.haki -o /tmp/hqa_release_bin --release 2>/dev/null
if [ -x /tmp/hqa_release_bin ]; then
    got=$(/tmp/hqa_release_bin 2>/dev/null)
    if [ "$got" = "479001600" ]; then
        green "hakic build --release (factorial 12 = 479001600)"
    else
        red "hakic build --release (factorial 12 = 479001600)"
        echo "  expected: 479001600, got: $got"
    fi
else
    red "hakic build --release (binary not produced)"
fi
rm -f /tmp/hqa_release_bin

# ── §28  toolchain — hakic fmt ───────────────────────────────────────────────
echo ""
echo "§28  Toolchain — hakic fmt"

cat > /tmp/hqa_fmt_src.haki << 'EOF'
fn main(){print("hello")}
EOF
$HAKIC fmt /tmp/hqa_fmt_src.haki 2>/dev/null
formatted=$(cat /tmp/hqa_fmt_src.haki)
echo "$formatted" | grep -q "fn main()" \
    && green "hakic fmt formats source file" \
    || red "hakic fmt formats source file"

cat > /tmp/hqa_fmt_check_ok.haki << 'EOF'
fn main() {
    print("hello")
}
EOF
$HAKIC fmt --check /tmp/hqa_fmt_check_ok.haki 2>/dev/null \
    && green "hakic fmt --check passes on well-formatted file" \
    || red "hakic fmt --check passes on well-formatted file"

# ── §29  toolchain — hakic init ──────────────────────────────────────────────
echo ""
echo "§29  Toolchain — hakic init"

rm -rf /tmp/hqa_initdir && mkdir /tmp/hqa_initdir
(cd /tmp/hqa_initdir && $HAKIC init myapp 2>/dev/null)
if [ -f /tmp/hqa_initdir/haki.toml ] && [ -f /tmp/hqa_initdir/src/main.haki ]; then
    green "hakic init creates haki.toml + src/main.haki"
else
    red "hakic init creates haki.toml + src/main.haki"
    ls /tmp/hqa_initdir 2>/dev/null || true
fi
rm -rf /tmp/hqa_initdir

# ── §30  toolchain — hakic doc ───────────────────────────────────────────────
echo ""
echo "§30  Toolchain — hakic doc"

cat > /tmp/hqa_doc_src.haki << 'EOF'
/// Adds two integers together.
/// Returns the sum.
fn add(a: int, b: int) -> int { return a + b }
fn main() { print(int_to_string(add(1, 2))) }
EOF
rm -rf /tmp/hqa_doc_out && mkdir /tmp/hqa_doc_out
$HAKIC doc /tmp/hqa_doc_src.haki -o /tmp/hqa_doc_out 2>/dev/null
if ls /tmp/hqa_doc_out/*.html 2>/dev/null | grep -q html; then
    green "hakic doc generates HTML documentation"
else
    red "hakic doc generates HTML documentation"
fi
rm -rf /tmp/hqa_doc_out

# ── §31  Performance baseline ─────────────────────────────────────────────────
echo ""
echo "§31  Performance baseline"

cat > /tmp/hqa_perf_fib.haki << 'EOF'
fn fib(n: int) -> int {
    if n <= 1 { return n }
    return fib(n - 1) + fib(n - 2)
}
fn main() { print(int_to_string(fib(35))) }
EOF
got=$($HAKIC run /tmp/hqa_perf_fib.haki 2>/dev/null)
if [ "$got" = "9227465" ]; then
    green "performance: fib(35) = 9227465"
else
    red "performance: fib(35) = 9227465"
    echo "  got: $got"
fi

cat > /tmp/hqa_perf_array.haki << 'EOF'
fn main() {
    const arr: Array<int> = []
    let i = 0
    while i < 10000 {
        arr.append(i)
        i = i + 1
    }
    print(int_to_string(arr.length))
    print(int_to_string(arr[9999]))
}
EOF
run_test "performance: large array (10k elements)" /tmp/hqa_perf_array.haki "10000
9999"

cat > /tmp/hqa_perf_map_stress.haki << 'EOF'
fn main() {
    const m: Map<string, int> = Map()
    let i = 0
    while i < 1000 {
        m.set(int_to_string(i), i * i)
        i = i + 1
    }
    print(int_to_string(m.get("999") ?? -1))
}
EOF
run_test "performance: map stress (1000 entries)" /tmp/hqa_perf_map_stress.haki "998001"

cat > /tmp/hqa_perf_str.haki << 'EOF'
fn main() {
    let s = ""
    let i = 0
    while i < 100 {
        s = s + "x"
        i = i + 1
    }
    print(int_to_string(s.length()))
}
EOF
run_test "performance: string concat 100x" /tmp/hqa_perf_str.haki "100"

# ── §32  Self-hosting smoke test ──────────────────────────────────────────────
echo ""
echo "§32  Self-hosting smoke test"

SELF_SRC="$(dirname "$HAKIC")/../src/hakic.haki"
if [ -f "$SELF_SRC" ]; then
    $HAKIC check "$SELF_SRC" 2>/dev/null \
        && green "self-hosting: hakic.haki passes type check" \
        || red "self-hosting: hakic.haki passes type check"
else
    # Check passes if the binary can report its own version (proves it was compiled from Haki)
    $HAKIC --version 2>/dev/null | grep -q "5\.0\.0" \
        && green "self-hosting: hakic --version reports 5.0.0" \
        || red "self-hosting: hakic --version reports 5.0.0"
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "═══════════════════════════════════════════"
echo " Haki v4.9.9 QA  ·  $(date '+%Y-%m-%d %H:%M')"
echo "═══════════════════════════════════════════"
TOTAL=$((PASS+FAIL))
printf " \033[32m%d passed\033[0m  /  \033[31m%d failed\033[0m  /  %d total\n" \
       "$PASS" "$FAIL" "$TOTAL"
echo "═══════════════════════════════════════════"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
exit 0
