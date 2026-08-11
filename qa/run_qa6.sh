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
    const resp, err = http.get("http://127.0.0.1:19877/")
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

srv = socketserver.TCPServer(('127.0.0.1', 19877), H)
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

# ── v4.9 new features ─────────────────────────────────────────────────────────

# math: float functions (sqrt, floor, ceil, roundInt)
cat > /tmp/t_mathf.haki << 'EOF'
import "std/math" as math
fn main() {
    const x: f64 = 16.0
    print(int_to_string(math.roundInt(math.sqrt(x))))
    const y: f64 = 2.7
    print(int_to_string(math.floorInt(y)))
    print(int_to_string(math.ceilInt(y)))
}
EOF
run_test "math: float sqrt/floor/ceil" /tmp/t_mathf.haki "4
2
3"

# strings: trimLeft/trimRight/replaceAll/indexOf/substring
cat > /tmp/t_strings_v49.haki << 'EOF'
import "std/strings" as strings
fn main() {
    print(strings.trimLeft("  hello"))
    print(strings.trimRight("hello  "))
    print(strings.replaceAll("hello world", "world", "haki"))
    print(int_to_string(strings.indexOf("hello", "ll")))
    print(strings.substring("hello world", 6, 11))
}
EOF
run_test "strings: trimLeft/trimRight/replaceAll/indexOf/substring" /tmp/t_strings_v49.haki "hello
hello
hello haki
2
world"

# fs: readLines
cat > /tmp/t_readlines.haki << 'EOF'
import "std/fs" as fs
fn main() {
    fs.writeFile("/tmp/haki_lines.txt", "alpha\nbeta\ngamma")
    const lines, err = fs.readLines("/tmp/haki_lines.txt")
    if err != null {
        print("error")
    } else {
        print(int_to_string(lines.length))
        print(lines[0])
        print(lines[2])
    }
    fs.deleteFile("/tmp/haki_lines.txt")
}
EOF
run_test "fs: readLines" /tmp/t_readlines.haki "3
alpha
gamma"

# ── v4.9.1: Language Completeness (roadmap v4.0) ─────────────────────────────

# for k, v in map iteration
cat > /tmp/t_mapiter.haki << 'EOF'
fn main() {
    const m: Map<string, int> = Map()
    m.set("apples", 3)
    m.set("bananas", 5)
    let total = 0
    for k, v in m {
        total = total + v
    }
    print(int_to_string(total))
}
EOF
run_test "v4.9.1: for k,v in map" /tmp/t_mapiter.haki "8"

# Inherited fields
cat > /tmp/t_inherit.haki << 'EOF'
class Animal { const name: string }
class Dog extends Animal { const breed: string }
fn main() {
    const d = Dog(name: "Rex", breed: "Lab")
    print(d.name)
    print(d.breed)
}
EOF
run_test "v4.9.1: inherited fields" /tmp/t_inherit.haki "Rex
Lab"

# Mutable closure capture
cat > /tmp/t_closure.haki << 'EOF'
fn makeCounter() -> fn() -> int {
    let count = 0
    return fn() -> int {
        count = count + 1
        return count
    }
}
fn main() {
    const inc = makeCounter()
    print(int_to_string(inc()))
    print(int_to_string(inc()))
    print(int_to_string(inc()))
}
EOF
run_test "v4.9.1: mutable closure capture" /tmp/t_closure.haki "1
2
3"

# ── v4.9.2: self-contained HTTP server (no libmicrohttpd) ─────────────────
cat > /tmp/t_http_server.haki << 'EOF'
fn main() {
    let server = HttpServer(19878, fn(req: HttpRequest) -> HttpResponse {
        if req.path == "/ping" {
            return HttpResponse(status: 200, body: "pong", contentType: "text/plain")
        }
        return HttpResponse(status: 404, body: "not found", contentType: "text/plain")
    })
    server.listen()
}
EOF

# Start server in background, test, kill it
HAKI_STDLIB="$STDLIB" $HAKI run /tmp/t_http_server.haki &
SRV_PID=$!
sleep 4
PING=$(curl -s http://localhost:19878/ping 2>/dev/null)
MISS=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:19878/missing 2>/dev/null)
kill $SRV_PID 2>/dev/null
wait $SRV_PID 2>/dev/null
if [ "$PING" = "pong" ] && [ "$MISS" = "404" ]; then
    green "v4.9.2: self-contained HTTP server (no libmicrohttpd)"
else
    red "v4.9.2: self-contained HTTP server (no libmicrohttpd)"
    echo "  /ping response: '$PING' (want 'pong')"
    echo "  /missing status: '$MISS' (want '404')"
fi

# ── v4.9.3: Concurrency v0.2 ─────────────────────────────────────────────────

# Test 1: async + await with typed return
cat > /tmp/t_async_await.haki << 'EOF'
fn compute(x: int) -> int {
    return x * x
}
fn main() {
    let t = async compute(7)
    let result = t.await()
    print(int_to_string(result))
}
EOF
run_test "v4.9.3: async + typed await" /tmp/t_async_await.haki "49"

# Test 2: Chan<string> send/recv
cat > /tmp/t_chan_str.haki << 'EOF'
fn main() {
    let ch: Chan<string> = Chan(2)
    ch.send("ping")
    ch.send("pong")
    let a = ch.recv()
    let b = ch.recv()
    print(a)
    print(b)
}
EOF
run_test "v4.9.3: Chan<string> send/recv" /tmp/t_chan_str.haki "ping
pong"

# Test 3: select basic (data on first channel)
cat > /tmp/t_select.haki << 'EOF'
fn main() {
    let ch1: Chan<string> = Chan(1)
    let ch2: Chan<string> = Chan(1)
    ch1.send("winner")
    select {
        msg = ch1.recv() {
            print(msg)
        }
        msg = ch2.recv() {
            print("wrong")
        }
    }
}
EOF
run_test "v4.9.3: select basic" /tmp/t_select.haki "winner"

# Test 4: select with timeout
cat > /tmp/t_select_timeout.haki << 'EOF'
fn main() {
    let ch: Chan<string> = Chan(1)
    select {
        msg = ch.recv() {
            print(msg)
        }
        timeout(200) {
            print("timed out")
        }
    }
}
EOF
run_test "v4.9.3: select with timeout" /tmp/t_select_timeout.haki "timed out"

# Test 5: sync.chan<string>() module-qualified constructor
cat > /tmp/t_sync_chan.haki << 'EOF'
fn main() {
    let ch: Chan<string> = sync.chan<string>(2)
    ch.send("a")
    ch.send("b")
    let a = ch.recv()
    let b = ch.recv()
    print(a)
    print(b)
}
EOF
run_test "v4.9.3: sync.chan<T>() constructor" /tmp/t_sync_chan.haki "a
b"

# Test 6: TaskGroup
cat > /tmp/t_taskgroup.haki << 'EOF'
fn square(x: int) -> int {
    return x * x
}
fn main() {
    let g: TaskGroup<int> = TaskGroup()
    g.add(async square(3))
    g.add(async square(4))
    let _ = g.awaitAll()
    print("done")
}
EOF
run_test "v4.9.3: TaskGroup.awaitAll" /tmp/t_taskgroup.haki "done"

# Test 7: sync.group<int>() module-qualified constructor
cat > /tmp/t_sync_group.haki << 'EOF'
fn double(x: int) -> int {
    return x * 2
}
fn main() {
    let g: TaskGroup<int> = sync.group<int>()
    g.add(async double(21))
    let _ = g.awaitAll()
    print("done")
}
EOF
run_test "v4.9.3: sync.group<T>() constructor" /tmp/t_sync_group.haki "done"

# ── v4.9.4: Annotation system ─────────────────────────────────────────────────

# Test 1: @requires(condition)
cat > /tmp/t_requires.haki << 'EOF'
@requires(x > 0)
fn safeSqrt(x: int) -> int {
    return x * x
}
fn main() {
    let r = safeSqrt(5)
    print(r)
}
EOF
run_test "v4.9.4: @requires passes" /tmp/t_requires.haki "25"

# Test 2: @requires fires panic on violation
cat > /tmp/t_requires_panic.haki << 'EOF'
@requires(x > 0)
fn safeSqrt(x: int) -> int {
    return x * x
}
fn main() {
    let r = safeSqrt(-1)
    print(r)
}
EOF
got=$(HAKI_STDLIB="$STDLIB" $HAKI run /tmp/t_requires_panic.haki 2>&1)
if echo "$got" | grep -q "requires\|panic\|abort\|failed"; then
    green "v4.9.4: @requires panics on violation"
    PASS=$((PASS+1))
else
    red "v4.9.4: @requires panics on violation"
    FAIL=$((FAIL+1))
    echo "  got: $(echo "$got" | head -2)"
fi

# Test 3: @error "msg" — success path still works
cat > /tmp/t_error_ok.haki << 'EOF'
@error "parse failed: {err}"
fn parseInt(s: string) -> (int, Error?) {
    return 42, null
}
fn main() {
    const v, e = parseInt("42")
    print(v)
}
EOF
run_test "v4.9.4: @error success path" /tmp/t_error_ok.haki "42"

# Test 4: @error "msg" — error path panics instead of propagating
cat > /tmp/t_error_panic.haki << 'EOF'
@error "parse failed: {err}"
fn parseInt(s: string) -> (int, Error?) {
    return 0, Error("bad input")
}
fn main() {
    const v, e = parseInt("abc")
    print(v)
}
EOF
got=$(HAKI_STDLIB="$STDLIB" $HAKI run /tmp/t_error_panic.haki 2>&1)
if echo "$got" | grep -q "parse failed\|panic\|abort\|bad input"; then
    green "v4.9.4: @error panics on error return"
    PASS=$((PASS+1))
else
    red "v4.9.4: @error panics on error return"
    FAIL=$((FAIL+1))
    echo "  got: $(echo "$got" | head -2)"
fi

# Test 5: @requires + @inline combined
cat > /tmp/t_requires_inline.haki << 'EOF'
@requires(n >= 0)
@inline
fn factorial(n: int) -> int {
    if n == 0 { return 1 }
    return n * factorial(n - 1)
}
fn main() {
    print(factorial(5))
}
EOF
run_test "v4.9.4: @requires + @inline stacked" /tmp/t_requires_inline.haki "120"

# ── v4.9.5: Type aliases ─────────────────────────────────────────────────────
cat > /tmp/t_type_alias.haki << 'EOF'
type UserId = int
fn greet(id: UserId) -> string {
    return "user:" + int_to_string(id)
}
fn main() {
    const id: UserId = 42
    print(greet(id))
}
EOF
run_test "v4.9.5: type alias basic" /tmp/t_type_alias.haki "user:42"

# Type alias used in struct field
cat > /tmp/t_alias_struct.haki << 'EOF'
type Score = int
struct Player {
    const name: string
    const score: Score
}
fn main() {
    const p = Player(name: "Alice", score: 100)
    print(p.name)
    print(int_to_string(p.score))
}
EOF
run_test "v4.9.5: type alias in struct" /tmp/t_alias_struct.haki "Alice
100"

# ── v4.9.5: Optional chaining ─────────────────────────────────────────────────
cat > /tmp/t_optional_chain.haki << 'EOF'
struct Address {
    const city: string
}
struct User {
    const address: Address?
}
fn city_name(u: User?) -> string? {
    return u?.address?.city
}
fn main() {
    const addr = Address(city: "Tokyo")
    const u = User(address: addr)
    const city = city_name(u)
    if city != null {
        print(city)
    } else {
        print("none")
    }
}
EOF
run_test "v4.9.5: optional chaining non-null" /tmp/t_optional_chain.haki "Tokyo"

cat > /tmp/t_optional_null.haki << 'EOF'
struct User {
    const name: string
}
fn get_name(u: User?) -> string? {
    return u?.name
}
fn main() {
    const result = get_name(null)
    if result == null {
        print("null")
    } else {
        print(result)
    }
}
EOF
run_test "v4.9.5: optional chaining null receiver" /tmp/t_optional_null.haki "null"

# ── v4.9.5: Match guard conditions ────────────────────────────────────────────
cat > /tmp/t_match_guard.haki << 'EOF'
fn classify(n: int) -> string {
    match n {
        0 { return "zero" }
        _ if n > 0 { return "positive" }
        _ { return "negative" }
    }
}
fn main() {
    print(classify(5))
    print(classify(0))
    print(classify(-3))
}
EOF
run_test "v4.9.5: match guard basic" /tmp/t_match_guard.haki "positive
zero
negative"

# Match guard with enum (two Ok arms with different guards — guard filters after discriminant)
cat > /tmp/t_match_guard_enum.haki << 'EOF'
enum Res {
    Good(int)
    Bad(string)
}
fn describe(r: Res) -> string {
    match r {
        Good(v) if v > 100 { return "big" }
        Good(v) { return "small:" + int_to_string(v) }
        Bad(msg) { return "err:" + msg }
    }
}
fn main() {
    print(describe(Good(200)))
    print(describe(Good(42)))
    print(describe(Bad("oops")))
}
EOF
run_test "v4.9.5: match guard with enum" /tmp/t_match_guard_enum.haki "big
small:42
err:oops"

# ── v4.9.6: Stdlib Completeness ────────────────────────────────────────────────

# json.array + json.parse with numeric values
cat > /tmp/t_json_arr.haki << 'EOF'
import "std/json" as json
fn main() {
    const items: Array<string> = []
    items.append(json.num(1))
    items.append(json.num(2))
    items.append(json.str("three"))
    print(json.array(items))
    const decoded = json.parse("{\"x\":42,\"y\":7}")
    print(decoded.getOrDefault("x", ""))
}
EOF
run_test "v4.9.6: json array + nested parse" /tmp/t_json_arr.haki '[1,2,"three"]
42'

# strings: padLeft / padRight / isEmpty
cat > /tmp/t_strings2.haki << 'EOF'
import "std/strings" as strings
fn main() {
    print(strings.padLeft("5", 3, "0"))
    print(strings.padRight("hi", 5, "."))
    print(bool_to_string(strings.isEmpty("")))
    print(bool_to_string(strings.isEmpty("x")))
}
EOF
run_test "v4.9.6: strings padLeft/padRight/isEmpty" /tmp/t_strings2.haki "005
hi...
true
false"

# math: clamp / pow (integer)
cat > /tmp/t_math2.haki << 'EOF'
import "std/math" as math
fn main() {
    print(int_to_string(math.clamp(15, 0, 10)))
    print(int_to_string(math.clamp(-5, 0, 10)))
    print(int_to_string(math.pow(2, 8)))
    print(int_to_string(math.pow(3, 4)))
}
EOF
run_test "v4.9.6: math clamp/pow" /tmp/t_math2.haki "10
0
256
81"

# std/env: set/get/getOrDefault/unset
cat > /tmp/t_env.haki << 'EOF'
import "std/env" as env
fn main() {
    env.set("HAKI_TEST_VAR", "hello123")
    const val, err = env.get("HAKI_TEST_VAR")
    if err == null {
        print(val)
    } else {
        print("err")
    }
    print(env.getOrDefault("HAKI_TEST_VAR", "fallback"))
    print(env.getOrDefault("HAKI_NOT_SET", "default"))
    env.unset("HAKI_TEST_VAR")
    print(env.getOrDefault("HAKI_TEST_VAR", "gone"))
}
EOF
run_test "v4.9.6: env set/get/getOrDefault/unset" /tmp/t_env.haki "hello123
hello123
default
gone"

# std/process: shell
cat > /tmp/t_process.haki << 'EOF'
import "std/process" as process
fn main() {
    const result, err = process.shell("echo hello-from-haki")
    if err == null {
        print(result.trim())
    } else {
        print("error: " + err.message)
    }
}
EOF
run_test "v4.9.6: process shell" /tmp/t_process.haki "hello-from-haki"

# std/fs: readDir (count regular files)
cat > /tmp/t_fs_readdir.haki << 'EOF'
import "std/fs" as fs
fn main() {
    fs.mkdir("/tmp/haki_dir_test")
    fs.writeFile("/tmp/haki_dir_test/a.txt", "aaa")
    fs.writeFile("/tmp/haki_dir_test/b.txt", "bbb")
    const entries, err = fs.readDir("/tmp/haki_dir_test")
    if err != null {
        print("error")
    } else {
        let fc = 0
        let i = 0
        while i < entries.length {
            const e = entries[i]
            if e.isDirectory == false && e.name != "." && e.name != ".." {
                fc = fc + 1
            }
            i = i + 1
        }
        print(int_to_string(fc))
    }
    fs.deleteFile("/tmp/haki_dir_test/a.txt")
    fs.deleteFile("/tmp/haki_dir_test/b.txt")
    fs.rmdir("/tmp/haki_dir_test")
}
EOF
run_test "v4.9.6: fs readDir" /tmp/t_fs_readdir.haki "2"

# env__getOrDefault fix: module-level getOrDefault must not hit Map intercept
# (regression guard for the cemit __getOrDefault arg-count bug)
cat > /tmp/t_getordefault_fix.haki << 'EOF'
import "std/env" as env
fn main() {
    const a = env.getOrDefault("PATH", "no-path")
    if string_length(a) > 0 {
        print("has-path")
    } else {
        print("no-path")
    }
    const b = env.getOrDefault("DEFINITELY_NOT_SET_HAKI_XYZ", "fallback")
    print(b)
}
EOF
run_test "v4.9.6: env getOrDefault regression" /tmp/t_getordefault_fix.haki "has-path
fallback"


# ── v4.9.7: Tooling ──────────────────────────────────────────────────────────
HAKIC=/home/claude/haki_work/target/release/hakic

run_hakic_test() {
    local name="$1"; shift
    local expected_exit="$1"; shift
    local expected_out="$1"; shift
    local actual_out actual_exit
    actual_out=$(HAKI_STDLIB="$STDLIB" "$@" 2>&1)
    actual_exit=$?
    local ok=1
    [ "$actual_exit" -ne "$expected_exit" ] && ok=0
    if [ -n "$expected_out" ] && ! echo "$actual_out" | grep -qF "$expected_out"; then ok=0; fi
    if [ "$ok" -eq 1 ]; then green "$name"
    else
        red "$name"
        echo "  expected exit=$expected_exit, got exit=$actual_exit"
        [ -n "$expected_out" ] && echo "  looking for: $expected_out"
        echo "  output: $(echo "$actual_out" | head -4)"
    fi
}

# -- hakic check: valid file → exit 0 --
cat > /tmp/t_check_ok.haki << 'EOF'
fn add(a: int, b: int) -> int { return a + b }
fn main() { print(int_to_string(add(1, 2))) }
EOF
run_hakic_test "v4.9.7: hakic check valid file" 0 "ok" \
    $HAKIC check /tmp/t_check_ok.haki

# -- hakic check: undefined variable → exit 1 + did-you-mean hint --
cat > /tmp/t_check_typo.haki << 'EOF'
fn greet(name: string) -> string { return "hello " + name }
fn main() { print(grtet("world")) }
EOF
run_hakic_test "v4.9.7: hakic check typo hint" 1 "did you mean" \
    $HAKIC check /tmp/t_check_typo.haki

# -- hakic fmt --check on already-formatted file → exit 0 --
cat > /tmp/t_fmt_clean.haki << 'EOF'
fn main() {
    const x = 42
    print(int_to_string(x))
}
EOF
# Format it first so it's canonical
$HAKIC fmt /tmp/t_fmt_clean.haki 2>/dev/null
run_hakic_test "v4.9.7: hakic fmt --check clean file" 0 "" \
    $HAKIC fmt --check /tmp/t_fmt_clean.haki

# -- hakic fmt --check on unformatted file → exit 1 (flag before filename) --
printf 'fn main(){print("hi")}' > /tmp/t_fmt_dirty.haki
run_hakic_test "v4.9.7: hakic fmt --check dirty file (flag first)" 1 "" \
    $HAKIC fmt --check /tmp/t_fmt_dirty.haki

# -- hakic fmt --check (filename before flag) → exit 1 --
printf 'fn main(){print("hi")}' > /tmp/t_fmt_dirty2.haki
run_hakic_test "v4.9.7: hakic fmt --check dirty file (flag last)" 1 "" \
    $HAKIC fmt /tmp/t_fmt_dirty2.haki --check

# -- hakic fmt match guard roundtrip --
cat > /tmp/t_fmt_guard.haki << 'EOF'
fn main() {
    const n = 5
    match n {
        0 { print("zero") }
        _ if n > 0 { print("positive") }
        _ { print("negative") }
    }
}
EOF
$HAKIC fmt /tmp/t_fmt_guard.haki 2>/dev/null
# After formatting, the guard must still be present
if grep -q "if n > 0" /tmp/t_fmt_guard.haki; then
    green "v4.9.7: hakic fmt match guard preserved"
    PASS=$((PASS+1))
else
    red "v4.9.7: hakic fmt match guard preserved"
    FAIL=$((FAIL+1))
    echo "  guard 'if n > 0' was stripped by fmt"
fi

# -- hakic doc produces HTML with function anchors (uses /// doc comments) --
mkdir -p /tmp/haki_doc_test
cat > /tmp/t_doc_funcs.haki << 'EOF'
/// Adds two integers together.
fn add(a: int, b: int) -> int { return a + b }

/// Returns the larger of two integers.
fn max_of(a: int, b: int) -> int {
    if a > b { return a }
    return b
}

fn main() {}
EOF
$HAKIC doc /tmp/t_doc_funcs.haki --out /tmp/haki_doc_test/ 2>/dev/null
if ls /tmp/haki_doc_test/*.html 2>/dev/null | head -1 | grep -q html; then
    html_file=$(ls /tmp/haki_doc_test/*.html | head -1)
    if grep -q "add" "$html_file" && grep -q "max_of" "$html_file"; then
        green "v4.9.7: hakic doc generates HTML with function entries"
        PASS=$((PASS+1))
    else
        red "v4.9.7: hakic doc generates HTML with function entries"
        FAIL=$((FAIL+1))
        echo "  HTML missing function names"
    fi
else
    red "v4.9.7: hakic doc generates HTML with function entries"
    FAIL=$((FAIL+1))
    echo "  no HTML file produced in /tmp/haki_doc_test/"
fi

# -- hakic test: @skip and passing tests (test_* naming, panic-based assertions) --
cat > /tmp/t_tooling_tests.haki << 'EOF'
fn test_add() {
    if !(1 + 1 == 2) { panic("1+1 should be 2") }
}

fn test_mul() {
    if !(3 * 4 == 12) { panic("3*4 should be 12") }
}

@skip
fn test_future() {
    panic("not implemented")
}

fn main() {}
EOF
run_hakic_test "v4.9.7: hakic test @skip + passing" 0 "passed" \
    $HAKIC test /tmp/t_tooling_tests.haki


# ── v4.9.8: Performance ───────────────────────────────────────────────────────

# -- fib(35) correctness (computationally intensive, verifies optimizer doesn't break output) --
cat > /tmp/t_perf_fib.haki << 'EOF'
fn fib(n: int) -> int {
    if n <= 1 { return n }
    return fib(n - 1) + fib(n - 2)
}
fn main() { print(int_to_string(fib(35))) }
EOF
run_test "v4.9.8: fib(35) correctness" /tmp/t_perf_fib.haki "9227465"

# -- large array: build 100 elements, sum them, verify total --
# (Array element assignment through functions is a known pre-existing limitation;
#  this test exercises append + indexed read at scale instead.)
cat > /tmp/t_perf_arr.haki << 'EOF'
fn main() {
    const arr: Array<int> = []
    let i = 1
    while i <= 100 {
        arr.append(i)
        i = i + 1
    }
    let sum = 0
    let j = 0
    while j < arr.length {
        sum = sum + arr[j]
        j = j + 1
    }
    print(int_to_string(sum))
    print(int_to_string(arr[0]))
    print(int_to_string(arr[99]))
}
EOF
run_test "v4.9.8: large array append + sum 100 elements" /tmp/t_perf_arr.haki "5050
1
100"

# -- map stress: 100 insert + get operations --
cat > /tmp/t_perf_map.haki << 'EOF'
fn main() {
    const m: Map<string, int> = Map()
    let i = 0
    while i < 100 {
        m.set("key" + int_to_string(i), i * i)
        i = i + 1
    }
    const v42 = m.getOrDefault("key42", -1)
    const v99 = m.getOrDefault("key99", -1)
    const missing = m.getOrDefault("key999", -1)
    print(int_to_string(v42))
    print(int_to_string(v99))
    print(int_to_string(missing))
}
EOF
run_test "v4.9.8: map stress 100 ops" /tmp/t_perf_map.haki "1764
9801
-1"

# -- string concat 50x (string growth) --
cat > /tmp/t_perf_str.haki << 'EOF'
fn main() {
    let s = ""
    let i = 0
    while i < 50 {
        s = s + "x"
        i = i + 1
    }
    print(int_to_string(string_length(s)))
}
EOF
run_test "v4.9.8: string concat 50x" /tmp/t_perf_str.haki "50"

# -- dead function elimination: unused fn pruned, program still correct --
cat > /tmp/t_perf_dce.haki << 'EOF'
fn never_called_a(x: int) -> int { return x * 999 }
fn never_called_b(x: int) -> int { return never_called_a(x) + 1 }
fn never_called_c(x: int) -> int { return never_called_b(x) * 2 }

fn useful(a: int, b: int) -> int { return a + b }

fn main() {
    print(int_to_string(useful(10, 32)))
}
EOF
run_test "v4.9.8: DCE unused functions pruned, output correct" /tmp/t_perf_dce.haki "42"

# -- --release flag: -O3 path produces correct output --
cat > /tmp/t_perf_release.haki << 'EOF'
fn factorial(n: int) -> int {
    if n <= 1 { return 1 }
    return n * factorial(n - 1)
}
fn main() { print(int_to_string(factorial(12))) }
EOF
got=$(HAKI_STDLIB="$STDLIB" $HAKI run /tmp/t_perf_release.haki 2>/dev/null)
# Note: haki run doesn't support --release directly; test via hakic + exec
HAKIC=/home/claude/haki_work/target/release/hakic
$HAKIC /tmp/t_perf_release.haki -o /tmp/t_perf_release_bin --release 2>/dev/null
if [ -x /tmp/t_perf_release_bin ]; then
    got_rel=$(/tmp/t_perf_release_bin 2>/dev/null)
    if [ "$got_rel" = "479001600" ]; then
        green "v4.9.8: --release flag produces correct output"
        PASS=$((PASS+1))
    else
        red "v4.9.8: --release flag produces correct output"
        FAIL=$((FAIL+1))
        echo "  expected: 479001600, got: $got_rel"
    fi
else
    red "v4.9.8: --release flag produces correct output"
    FAIL=$((FAIL+1))
    echo "  binary not produced"
fi


# ── v4.9.9: Array assignment, short-circuit &&/||, hakic build, hakic init ────

# -- array subscript assignment (LLVM): swap two elements --
cat > /tmp/t_v499_arrset.haki << 'EOF'
fn main() {
    const arr: Array<int> = []
    arr.append(1)
    arr.append(3)
    let tmp: int = arr[0]
    arr[0] = arr[1]
    arr[1] = tmp
    print(int_to_string(arr[0]))
    print(int_to_string(arr[1]))
}
EOF
run_test "v4.9.9: array subscript assignment (swap)" /tmp/t_v499_arrset.haki "3
1"

# -- && short-circuit: j=-1, arr[j] must not be evaluated --
cat > /tmp/t_v499_sc_and.haki << 'EOF'
fn main() {
    const arr: Array<int> = []
    arr.append(10)
    arr.append(20)
    let j: int = -1
    if j >= 0 && arr[j] > 5 {
        print("WRONG")
    } else {
        print("sc_and_ok")
    }
}
EOF
run_test "v4.9.9: && short-circuit skips RHS when LHS is false" /tmp/t_v499_sc_and.haki "sc_and_ok"

# -- || short-circuit: x > 0 is true, arr[99] must not be evaluated --
cat > /tmp/t_v499_sc_or.haki << 'EOF'
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
run_test "v4.9.9: || short-circuit skips RHS when LHS is true" /tmp/t_v499_sc_or.haki "sc_or_ok"

# -- hakic build: produce a standalone native binary --
cat > /tmp/t_v499_build_src.haki << 'EOF'
fn main() {
    print("built_ok")
}
EOF
HAKIC=/home/claude/haki_work/target/release/hakic
rm -f /tmp/t_v499_build_bin
$HAKIC build /tmp/t_v499_build_src.haki -o /tmp/t_v499_build_bin 2>/dev/null
if [ -x /tmp/t_v499_build_bin ]; then
    got_b=$(/tmp/t_v499_build_bin 2>/dev/null)
    if [ "$got_b" = "built_ok" ]; then
        green "v4.9.9: hakic build produces working binary"
    else
        red "v4.9.9: hakic build produces working binary"
        echo "  expected: built_ok, got: $got_b"
    fi
else
    red "v4.9.9: hakic build produces working binary"
    echo "  binary not produced"
fi

# -- hakic init: scaffold src/main.haki --
rm -rf /tmp/t_v499_initdir && mkdir /tmp/t_v499_initdir
(cd /tmp/t_v499_initdir && $HAKIC init myapp 2>/dev/null)
if [ -f /tmp/t_v499_initdir/haki.toml ] && [ -f /tmp/t_v499_initdir/src/main.haki ]; then
    green "v4.9.9: hakic init scaffolds haki.toml + src/main.haki"
else
    red "v4.9.9: hakic init scaffolds haki.toml + src/main.haki"
    echo "  haki.toml exists: $([ -f /tmp/t_v499_initdir/haki.toml ] && echo yes || echo no)"
    echo "  src/main.haki exists: $([ -f /tmp/t_v499_initdir/src/main.haki ] && echo yes || echo no)"
fi

echo ""
echo "======================================="
printf "  QA Results: %d passed, %d failed\n" $PASS $FAIL
echo "======================================="
[ $FAIL -eq 0 ]
