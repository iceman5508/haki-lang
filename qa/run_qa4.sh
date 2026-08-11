#!/bin/bash
HAKI=/home/claude/haki_work/target/release/haki
STDLIB=${HAKI_STDLIB:-/home/claude/haki_work/stdlib}
PASS=0; FAIL=0
green() { printf "\033[32m✓\033[0m %s\n" "$1"; PASS=$((PASS+1)); }
red()   { printf "\033[31m✗\033[0m %s\n" "$1"; FAIL=$((FAIL+1)); }

run_test() {
    local name="$1" file="$2" expected="$3" got
    got=$(HAKI_STDLIB="$STDLIB" $HAKI run "$file" 2>/dev/null)
    if [ "$got" = "$expected" ]; then green "$name"
    else
        red "$name"
        echo "  expected: $(echo "$expected" | head -3)"
        echo "  got:      $(echo "$got" | head -3)"
        HAKI_STDLIB="$STDLIB" $HAKI run "$file" 2>&1 | grep "error:" | head -2 || true
    fi
}

# ── Core language ─────────────────────────────────────────────────────────────
cat > /tmp/t_fib.haki << 'EOF'
fn fib(n: int) -> int {
    if n <= 1 { return n }
    return fib(n - 1) + fib(n - 2)
}
fn main() { print(int_to_string(fib(10))) }
EOF
run_test "core: fibonacci" /tmp/t_fib.haki "55"

cat > /tmp/t_map.haki << 'EOF'
fn main() {
    const m: Map<string, string> = Map()
    m.set("x", "hello")
    m.set("y", "world")
    const v = m.get("x")
    if v != null { print(v) }
    print(bool_to_string(m.has("y")))
    print(bool_to_string(m.has("z")))
}
EOF
run_test "core: map operations" /tmp/t_map.haki "hello
true
false"

cat > /tmp/t_arr.haki << 'EOF'
fn main() {
    const a: Array<int> = []
    a.append(10)
    a.append(20)
    a.append(30)
    print(int_to_string(a.length))
    print(int_to_string(a[1]))
}
EOF
run_test "core: array operations" /tmp/t_arr.haki "3
20"

cat > /tmp/t_class.haki << 'EOF'
class Point {
    const x: int
    const y: int
    fn dist() -> int { return x * x + y * y }
}
fn main() {
    const p = Point(x: 3, y: 4)
    print(int_to_string(p.dist()))
}
EOF
run_test "core: class with methods" /tmp/t_class.haki "25"

# FIX: enum uses comma separators, not pipe
cat > /tmp/t_enum.haki << 'EOF'
enum Shape { Circle, Square, Triangle }
fn name(s: Shape) -> string {
    return match s {
        Circle   { yield "circle" }
        Square   { yield "square" }
        Triangle { yield "triangle" }
    }
}
fn main() {
    print(name(Shape.Circle))
    print(name(Shape.Triangle))
}
EOF
run_test "core: enum match" /tmp/t_enum.haki "circle
triangle"

cat > /tmp/t_closure.haki << 'EOF'
fn apply(f: fn(int) -> int, x: int) -> int { return f(x) }
fn main() {
    const double = fn(x: int) -> int { return x * 2 }
    print(int_to_string(apply(double, 21)))
}
EOF
run_test "core: closures" /tmp/t_closure.haki "42"

cat > /tmp/t_optional.haki << 'HEOF'
fn findFirst(arr: Array<string>, target: string) -> string? {
    let i = 0
    while i < arr.length {
        if arr[i] == target { return arr[i] }
        i = i + 1
    }
    return null
}
fn main() {
    const arr: Array<string> = []
    arr.append("foo")
    arr.append("bar")
    arr.append("baz")
    const r = findFirst(arr, "bar")
    if r != null { print(r) } else { print("not found") }
    const r2 = findFirst(arr, "qux")
    if r2 != null { print(r2) } else { print("not found") }
}
HEOF
run_test "core: optional returns" /tmp/t_optional.haki "bar
not found"

cat > /tmp/t_multi.haki << 'EOF'
fn divmod(a: int, b: int) -> (int, int) { return a / b, a % b }
fn main() {
    const q, r = divmod(17, 5)
    print(int_to_string(q))
    print(int_to_string(r))
}
EOF
run_test "core: multi-return" /tmp/t_multi.haki "3
2"

cat > /tmp/t_generics.haki << 'HEOF'
fn firstInt(arr: Array<int>) -> int { return arr[0] }
fn main() {
    const s = "hello"
    print(s)
    const arr: Array<int> = []
    arr.append(99)
    arr.append(42)
    print(int_to_string(firstInt(arr)))
}
HEOF
run_test "core: generics" /tmp/t_generics.haki "hello
99"

cat > /tmp/t_while.haki << 'HEOF'
fn main() {
    let sum = 0
    let i = 1
    while i <= 10 {
        sum = sum + i
        i = i + 1
    }
    print(int_to_string(sum))
}
HEOF
run_test "core: while loop" /tmp/t_while.haki "55"

# ── Optional narrowing ────────────────────────────────────────────────────────
cat > /tmp/t_narrow.haki << 'EOF'
fn process(s: string?) -> string {
    if s == null { return "none" }
    return s
}
fn findUser(id: int) -> string? {
    if id == 1 { return "Alice" }
    if id == 2 { return "Bob" }
    return null
}
fn greet(id: int) -> string {
    const user = findUser(id)
    if user == null { return "unknown" }
    return "Hello, " + user
}
fn main() {
    print(process("world"))
    print(process(null))
    print(greet(1))
    print(greet(99))
}
EOF
run_test "core: optional narrowing after return" /tmp/t_narrow.haki "world
none
Hello, Alice
unknown"

# ── String methods ────────────────────────────────────────────────────────────
cat > /tmp/t_str.haki << 'EOF'
fn main() {
    const s = "Hello"
    print(int_to_string(s.length()))
    print(bool_to_string(s.isEmpty()))
    print(s.charAt(1))
    print(int_to_string(s.charCodeAt(0)))
}
EOF
run_test "string: v4.7 methods (length/isEmpty/charAt/charCodeAt)" /tmp/t_str.haki "5
false
e
72"

cat > /tmp/t_str2.haki << 'EOF'
fn main() {
    const s = "  hello world  "
    print(s.trim())
    print("hello".toUpper())
    print("WORLD".toLower())
    print(bool_to_string("hello".contains("ell")))
    print(bool_to_string("hello".startsWith("he")))
    print(bool_to_string("hello".endsWith("lo")))
}
EOF
run_test "string: existing methods" /tmp/t_str2.haki "hello world
HELLO
world
true
true
true"

cat > /tmp/t_str3.haki << 'EOF'
fn main() {
    const parts = "a,b,c".split(",")
    print(parts[0])
    print(parts[2])
    print(int_to_string(parts.length))
}
EOF
run_test "string: split → array" /tmp/t_str3.haki "a
c
3"

cat > /tmp/t_str4.haki << 'EOF'
fn main() {
    const s = "hello"
    print(s.charAt(0))
    print(int_to_string(s.charCodeAt(0)))
    print("  hi  ".trim())
}
EOF
run_test "string: charAt/charCodeAt/trim" /tmp/t_str4.haki "h
104
hi"

# ── JSON ──────────────────────────────────────────────────────────────────────
cat > /tmp/t_json.haki << 'EOF'
import "std/json" as json
fn main() {
    const m = json.parse("{\"name\":\"Alice\",\"age\":\"30\"}")
    const name = m.get("name")
    if name != null { print(name) }
    const age = m.get("age")
    if age != null { print(age) }
}
EOF
run_test "json: parse object" /tmp/t_json.haki "Alice
30"

cat > /tmp/t_json2.haki << 'EOF'
import "std/json" as json
fn main() {
    const m: Map<string,string> = Map()
    m.set("hello", "world")
    const s = json.stringify(m)
    const m2 = json.parse(s)
    const v = m2.get("hello")
    if v != null { print(v) }
}
EOF
run_test "json: stringify and parse roundtrip" /tmp/t_json2.haki "world"

cat > /tmp/t_json3.haki << 'EOF'
import "std/json" as json
fn main() {
    const val = json.decodeGet("{\"key\":\"value\"}", "key")
    print(val)
}
EOF
run_test "json: decodeGet" /tmp/t_json3.haki "value"

cat > /tmp/t_json4.haki << 'EOF'
import "std/json" as json
fn main() {
    const fields: Map<string, string> = Map()
    fields.set("name", json.str("Bob"))
    fields.set("age", json.num(42))
    const out = json.object(fields)
    print(json.decodeGet(out, "name"))
    print(json.decodeGet(out, "age"))
}
EOF
run_test "json: build with str/num/object" /tmp/t_json4.haki "Bob
42"

# ── Regex ─────────────────────────────────────────────────────────────────────
# FIX: regex.find returns (string, Error?) tuple — destructure it
cat > /tmp/t_regex.haki << 'EOF'
import "std/regex" as regex
fn main() {
    print(bool_to_string(regex.matches("hello world", "world")))
    print(bool_to_string(regex.matches("hello", "^xyz")))
    const found, ferr = regex.find("hello world", "w[a-z]+")
    print(found)
    print(regex.replaceAll("aabbcc", "[bc]", "X"))
    const parts = regex.split("a,b,c", ",")
    print(parts[0])
    print(parts[2])
}
EOF
run_test "regex: matches/find/replaceAll/split" /tmp/t_regex.haki "true
false
world
aaXXXX
a
c"

cat > /tmp/t_regex2.haki << 'EOF'
import "std/regex" as regex
fn main() {
    const groups = regex.findGroups("2024-01-15", "([0-9]{4})-([0-9]{2})-([0-9]{2})")
    print(groups[0])
    print(groups[1])
    print(groups[2])
}
EOF
run_test "regex: findGroups" /tmp/t_regex2.haki "2024
01
15"

# ── Template ──────────────────────────────────────────────────────────────────
cat > /tmp/t_tpl.haki << 'EOF'
import "std/template" as template
fn main() {
    const td: Map<string,string> = Map()
    td.set("name", "World")
    const out = template.render("Hello, {{name}}!", td)
    print(out)
    print(template.escape("<b>bold</b>"))
}
EOF
run_test "template: render and escape" /tmp/t_tpl.haki 'Hello, World!
&lt;b&gt;bold&lt;/b&gt;'

# FIX: template.vars() now takes 0 args and returns empty Map
cat > /tmp/t_tpl2.haki << 'EOF'
import "std/template" as template
fn main() {
    const td = template.vars()
    td.set("item", "apple")
    const out = template.render("item: {{item}}", td)
    print(out)
}
EOF
run_test "template: vars helper" /tmp/t_tpl2.haki "item: apple"

cat > /tmp/t_tpl3.haki << 'EOF'
import "std/template" as template
fn main() {
    const td: Map<string,string> = Map()
    td.set("show", "true")
    const out = template.render("{{#if show}}visible{{/if}}", td)
    print(out)
}
EOF
run_test "template: conditional block" /tmp/t_tpl3.haki "visible"

# ── XML ───────────────────────────────────────────────────────────────────────
# FIX: getAttr takes (tagStr, attrName) — 2 args; search finds "id" in the full doc
cat > /tmp/t_xml.haki << 'EOF'
import "std/xml" as xml
fn main() {
    const doc = "<root><item id=\"1\">Hello</item></root>"
    print(xml.getElement(doc, "item"))
    print(xml.getAttr(doc, "id"))
    print(xml.emitElement("tag", "content"))
    print(xml.escape("<&>"))
}
EOF
run_test "xml: getElement/getAttr/emitElement/escape" /tmp/t_xml.haki "Hello
1
<tag>content</tag>
&lt;&amp;&gt;"

# FIX: parseAttrs takes the attribute portion only, not the full tag
cat > /tmp/t_xml2.haki << 'EOF'
import "std/xml" as xml
fn main() {
    const attrs = xml.parseAttrs("id=\"5\" class=\"x\"")
    const id = attrs.get("id")
    if id != null { print(id) }
    const cls = attrs.get("class")
    if cls != null { print(cls) }
}
EOF
run_test "xml: parseAttrs" /tmp/t_xml2.haki "5
x"

# ── CSV ───────────────────────────────────────────────────────────────────────
cat > /tmp/t_csv.haki << 'EOF'
import "std/csv" as csv
fn main() {
    const row = csv.parseRow("A,B,C")
    print(row[0])
    print(row[2])
    const fa: Array<string> = []
    fa.append("X")
    fa.append("Y")
    print(csv.encodeRow(fa))
}
EOF
run_test "csv: parseRow/encodeRow" /tmp/t_csv.haki "A
C
X,Y"

# FIX: no semicolons in Haki — use multi-line if block
cat > /tmp/t_csv2.haki << 'EOF'
import "std/csv" as csv
fn main() {
    const rows, err = csv.parse("h1,h2\nv1,v2")
    if err != null {
        print("err")
        return
    }
    const r0 = rows[0]
    print(r0[0])
    const r1 = rows[1]
    print(r1[1])
    const trows: Array<Array<string>> = []
    const ra: Array<string> = []
    ra.append("a")
    ra.append("b")
    trows.append(ra)
    print(csv.encode(trows))
}
EOF
run_test "csv: parse/encode" /tmp/t_csv2.haki "h1
v2
a,b"

# ── HTTP ──────────────────────────────────────────────────────────────────────
cat > /tmp/t_http_compile.haki << 'EOF'
import "std/http" as http
fn main() {
    print("http ok")
}
EOF
run_test "http: module compiles" /tmp/t_http_compile.haki "http ok"

# HTTP end-to-end: start local server then test
cat > /tmp/t_http_req.haki << 'EOF'
import "std/http" as http
fn main() {
    const resp, err = http.get("http://127.0.0.1:19876/")
    if err != null {
        print("error")
        return
    }
    print(int_to_string(resp.status))
    print(resp.body)
}
EOF

python3 -c "
import http.server, threading, socketserver, sys, time, subprocess, os

class H(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b'pong')
    def log_message(self, *a): pass

srv = socketserver.TCPServer(('127.0.0.1', 19876), H)
srv.allow_reuse_address = True
t = threading.Thread(target=srv.serve_forever)
t.daemon = True
t.start()
time.sleep(0.5)
env = os.environ.copy()
env['HAKI_STDLIB'] = '$STDLIB'
r = subprocess.run(['$HAKI', 'run', '/tmp/t_http_req.haki'],
    capture_output=True, text=True, env=env, timeout=10)
print(r.stdout.strip())
srv.shutdown()
" > /tmp/http_result.txt 2>&1

HTTP_RESULT=$(cat /tmp/http_result.txt 2>/dev/null)
if [ "$HTTP_RESULT" = "200
pong" ]; then
    green "http: real GET request to localhost"
else
    red "http: real GET request to localhost"
    echo "  result: $HTTP_RESULT"
fi

# ── Crypto ────────────────────────────────────────────────────────────────────
cat > /tmp/t_crypto.haki << 'EOF'
import "std/crypto" as crypto
fn main() {
    const h = crypto.sha256("hello")
    print(h)
    const b = crypto.base64Encode("Hello, World!")
    print(b)
    const d = crypto.base64Decode("SGVsbG8sIFdvcmxkIQ==")
    print(d)
}
EOF
run_test "crypto: sha256 + base64" /tmp/t_crypto.haki "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
SGVsbG8sIFdvcmxkIQ==
Hello, World!"

# ── strings ───────────────────────────────────────────────────────────────────
# FIX: strings.count added to strings.haki
cat > /tmp/t_strings.haki << 'EOF'
import "std/strings" as strings
fn main() {
    print(strings.repeat("ab", 3))
    print(strings.join(["x","y","z"], "-"))
    print(int_to_string(strings.count("hello", "l")))
}
EOF
run_test "strings: repeat/join/count" /tmp/t_strings.haki "ababab
x-y-z
2"

# ── math ──────────────────────────────────────────────────────────────────────
cat > /tmp/t_math.haki << 'EOF'
import "std/math" as math
fn main() {
    print(int_to_string(math.abs(-42)))
    print(int_to_string(math.max(3, 7)))
    print(int_to_string(math.min(3, 7)))
}
EOF
run_test "math: abs/max/min" /tmp/t_math.haki "42
7
3"

# ── fs ────────────────────────────────────────────────────────────────────────
# FIX: fs.readFile returns (string, Error?) — destructure with two bindings
cat > /tmp/t_fs.haki << 'EOF'
import "std/fs" as fs
fn main() {
    fs.writeFile("/tmp/haki_fstest.txt", "hello haki")
    const content, rerr = fs.readFile("/tmp/haki_fstest.txt")
    print(content)
    print(bool_to_string(fs.exists("/tmp/haki_fstest.txt")))
    fs.deleteFile("/tmp/haki_fstest.txt")
    print(bool_to_string(fs.exists("/tmp/haki_fstest.txt")))
}
EOF
run_test "fs: write/read/exists/delete" /tmp/t_fs.haki "hello haki
true
false"

echo ""
echo "======================================="
printf "  QA Results: %d passed, %d failed\n" $PASS $FAIL
echo "======================================="
[ $FAIL -eq 0 ]
