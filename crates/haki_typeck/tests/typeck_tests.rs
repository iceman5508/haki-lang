/// Integration tests for `haki_typeck`.
///
/// Each test parses a snippet, runs the type checker, and asserts on
/// the outcome. Tests are grouped by the rule they exercise.

use haki_typeck::{typecheck, TypeError};
use haki_parser::parse;


// ── Helpers ───────────────────────────────────────────────────────────────────

fn check_ok(src: &str) -> haki_typeck::TypedSourceFile {
    let ast = parse(src).unwrap_or_else(|e| panic!("parse failed: {e}"));
    typecheck(&ast).unwrap_or_else(|e| panic!("typecheck failed: {e}"))
}

fn check_err(src: &str) -> TypeError {
    let ast = parse(src).unwrap_or_else(|e| panic!("parse failed: {e}"));
    typecheck(&ast).expect_err("expected a type error but check passed")
}

fn is_unknown_var(e: &TypeError) -> bool {
    matches!(e, TypeError::UnknownVar { .. })
}
fn is_type_mismatch(e: &TypeError) -> bool {
    matches!(e, TypeError::TypeMismatch { .. })
}
fn is_invalid_binary(e: &TypeError) -> bool {
    matches!(e, TypeError::InvalidBinary { .. })
}
fn is_assign_to_const(e: &TypeError) -> bool {
    matches!(e, TypeError::AssignToConst { .. })
}
fn is_binding_count_mismatch(e: &TypeError) -> bool {
    matches!(e, TypeError::BindingCountMismatch { .. })
}
#[allow(dead_code)]
fn is_missing_protocol_method(e: &TypeError) -> bool {
    matches!(e, TypeError::MissingProtocolMethod { .. })
}
fn is_wrong_arg_count(e: &TypeError) -> bool {
    matches!(e, TypeError::WrongArgCount { .. })
}
fn is_no_such_field(e: &TypeError) -> bool {
    matches!(e, TypeError::NoSuchField { .. })
}
fn is_no_such_method(e: &TypeError) -> bool {
    matches!(e, TypeError::NoSuchMethod { .. })
}
fn is_null_to_non_optional(e: &TypeError) -> bool {
    matches!(e, TypeError::NullToNonOptional { .. })
}
fn is_weak_on_value_type(e: &TypeError) -> bool {
    matches!(e, TypeError::WeakOnValueType { .. })
}
fn is_weak_field_must_be_optional(e: &TypeError) -> bool {
    matches!(e, TypeError::WeakFieldMustBeOptional { .. })
}
fn is_duplicate_impl(e: &TypeError) -> bool {
    matches!(e, TypeError::DuplicateImpl { .. })
}
fn is_assign_to_const_field(e: &TypeError) -> bool {
    matches!(e, TypeError::AssignToConstField { .. })
}

// ── Literals and primitives ────────────────────────────────────────────────────

#[test]
fn test_int_literal() {
    check_ok("fn f() { const x: int = 42 }");
}

#[test]
fn test_float_literal() {
    check_ok("fn f() { const x: f64 = 3.14 }");
}

#[test]
fn test_string_literal() {
    check_ok(r#"fn f() { const x: string = "hello" }"#);
}

#[test]
fn test_bool_literal() {
    check_ok("fn f() { const x: bool = true }");
}

#[test]
fn test_null_to_optional() {
    check_ok("fn f() -> (string?, Error?) { return null, null }");
}

// ── Variable scoping ──────────────────────────────────────────────────────────

#[test]
fn test_undefined_variable_error() {
    let e = check_err("fn f() { const x = y }");
    assert!(is_unknown_var(&e), "got: {e}");
}

#[test]
fn test_variable_shadowing_in_nested_scope() {
    check_ok(r#"
        fn f() {
            const x: int = 1
            if true {
                const x: int = 2
                return
            }
        }
    "#);
}

#[test]
fn test_let_binding_is_mutable() {
    check_ok(r#"
        fn f() {
            let x: int = 1
            x = 2
        }
    "#);
}

// ── Type annotations and inference ───────────────────────────────────────────

#[test]
fn test_annotation_mismatch_error() {
    let e = check_err(r#"fn f() { const x: int = "hello" }"#);
    assert!(is_type_mismatch(&e), "got: {e}");
}

#[test]
fn test_inferred_type_propagates() {
    // y gets the type of x (int), then we can use it in arithmetic.
    check_ok("fn f() { const x: int = 1 const y = x }");
}

// ── Binary operators ──────────────────────────────────────────────────────────

#[test]
fn test_int_arithmetic() {
    check_ok("fn f() -> int { return 1 + 2 * 3 }");
}

#[test]
fn test_string_concat() {
    check_ok(r#"fn f() -> string { return "a" + "b" }"#);
}

#[test]
fn test_mixed_numeric_types_error() {
    let e = check_err("fn f() -> int { return 1 + 3.14 }");
    assert!(is_invalid_binary(&e), "got: {e}");
}

#[test]
fn test_bool_comparison() {
    check_ok("fn f() -> bool { return 1 == 1 }");
}

#[test]
fn test_null_comparison() {
    check_ok(r#"
        fn f(err: Error?) -> bool {
            return err != null
        }
    "#);
}

#[test]
fn test_logical_operators() {
    check_ok("fn f() -> bool { return true && false || true }");
}

#[test]
fn test_logical_on_non_bool_error() {
    let e = check_err("fn f() { const x = 1 && 2 }");
    assert!(is_invalid_binary(&e), "got: {e}");
}

// ── Mutability ────────────────────────────────────────────────────────────────

#[test]
fn test_const_reassign_error() {
    let e = check_err("fn f() { const x: int = 1 x = 2 }");
    assert!(is_assign_to_const(&e), "got: {e}");
}

#[test]
fn test_const_field_assign_error() {
    let e = check_err(r#"
        struct Point { const x: f64 }
        fn f(p: Point) { p.x = 1.0 }
    "#);
    assert!(is_assign_to_const_field(&e), "got: {e}");
}

// ── Functions and return types ────────────────────────────────────────────────

#[test]
fn test_fn_return_type_check() {
    check_ok("fn add(a: int, b: int) -> int { return a }");
}

#[test]
fn test_fn_return_mismatch_error() {
    let e = check_err(r#"fn f() -> int { return "hello" }"#);
    assert!(is_type_mismatch(&e), "got: {e}");
}

#[test]
fn test_fn_call() {
    check_ok(r#"
        fn add(a: int, b: int) -> int { return a }
        fn f() -> int { return add(1, 2) }
    "#);
}

#[test]
fn test_fn_call_wrong_arg_count() {
    let e = check_err(r#"
        fn add(a: int, b: int) -> int { return a }
        fn f() -> int { return add(1) }
    "#);
    assert!(is_wrong_arg_count(&e), "got: {e}");
}

#[test]
fn test_fn_call_arg_type_mismatch() {
    let e = check_err(r#"
        fn add(a: int, b: int) -> int { return a }
        fn f() -> int { return add(1, true) }
    "#);
    assert!(is_type_mismatch(&e), "got: {e}");
}

// ── Multi-return / tuple bindings ────────────────────────────────────────────

#[test]
fn test_multi_return_binding() {
    check_ok(r#"
        fn readFile(path: string) -> (string, Error?) { return "", null }
        fn f() {
            const data, err = readFile("f.txt")
            if err != null { return }
        }
    "#);
}

#[test]
fn test_discard_binding() {
    check_ok(r#"
        fn readFile(path: string) -> (string, Error?) { return "", null }
        fn f() {
            const data, _ = readFile("f.txt")
        }
    "#);
}

#[test]
fn test_discard_all() {
    check_ok(r#"
        fn logEvent(msg: string) -> bool { return true }
        fn f() { _ = logEvent("ping") }
    "#);
}

#[test]
fn test_binding_count_mismatch_error() {
    let e = check_err(r#"
        fn readFile(path: string) -> (string, Error?) { return "", null }
        fn f() {
            const data, err, extra = readFile("f.txt")
        }
    "#);
    assert!(is_binding_count_mismatch(&e), "got: {e}");
}

// ── Structs ───────────────────────────────────────────────────────────────────

#[test]
fn test_struct_field_access() {
    check_ok(r#"
        struct Point { const x: f64 const y: f64 }
        fn f(p: Point) -> f64 { return p.x }
    "#);
}

#[test]
fn test_struct_no_such_field_error() {
    let e = check_err(r#"
        struct Point { const x: f64 }
        fn f(p: Point) -> f64 { return p.z }
    "#);
    assert!(is_no_such_field(&e), "got: {e}");
}

#[test]
fn test_struct_method_call() {
    check_ok(r#"
        struct Point {
            const x: f64
            fn getX() -> f64 { return 0.0 }
        }
        fn f(p: Point) -> f64 { return p.getX() }
    "#);
}

#[test]
fn test_struct_no_such_method_error() {
    let e = check_err(r#"
        struct Point { const x: f64 }
        fn f(p: Point) -> f64 { return p.missing() }
    "#);
    assert!(is_no_such_method(&e), "got: {e}");
}

// ── Classes ───────────────────────────────────────────────────────────────────

#[test]
fn test_class_field_access() {
    check_ok(r#"
        class User { const name: string let score: int }
        fn f(u: User) -> string { return u.name }
    "#);
}

#[test]
fn test_class_superclass_field() {
    check_ok(r#"
        class Error { const message: string }
        class NetworkError extends Error { const statusCode: int }
        fn f(e: NetworkError) -> string { return e.message }
    "#);
}

#[test]
fn test_class_mutable_field_assign() {
    check_ok(r#"
        class User { const name: string let score: int }
        fn f(u: User) { u.score = 10 }
    "#);
}

// ── Protocols and impl ────────────────────────────────────────────────────────

#[test]
fn test_protocol_method_callable_via_impl() {
    check_ok(r#"
        struct Temperature { const value: f64 }
        impl Comparable for Temperature {
            fn compareTo(other: Temperature) -> int { return 0 }
        }
        fn f(a: Temperature, b: Temperature) -> int {
            return a.compareTo(b)
        }
    "#);
}

#[test]
fn test_protocol_conformance_check() {
    // impl block is complete — should pass.
    check_ok(r#"
        struct Temperature { const value: f64 }
        impl Comparable for Temperature {
            fn compareTo(other: Temperature) -> int { return 0 }
        }
    "#);
}

#[test]
fn test_duplicate_impl_error() {
    let e = check_err(r#"
        struct Temperature { const value: f64 }
        impl Comparable for Temperature {
            fn compareTo(other: Temperature) -> int { return 0 }
        }
        impl Comparable for Temperature {
            fn compareTo(other: Temperature) -> int { return 1 }
        }
    "#);
    assert!(is_duplicate_impl(&e), "got: {e}");
}

// ── Weak references ───────────────────────────────────────────────────────────

#[test]
fn test_weak_field_on_class_ok() {
    check_ok(r#"
        class Node {
            let value: int
            weak next: Node?
        }
    "#);
}

#[test]
fn test_weak_field_on_struct_error() {
    let e = check_err(r#"
        struct Node {
            let value: int
            weak next: Node?
        }
    "#);
    assert!(is_weak_on_value_type(&e), "got: {e}");
}

#[test]
fn test_weak_field_must_be_optional_error() {
    // `weak` field without `?` is a type error.
    let e = check_err(r#"
        class Node {
            let value: int
            weak next: Node
        }
    "#);
    assert!(is_weak_field_must_be_optional(&e), "got: {e}");
}

// ── Null safety ───────────────────────────────────────────────────────────────

#[test]
fn test_null_to_non_optional_error() {
    let e = check_err("fn f() -> int { return null }");
    // null can't satisfy a non-optional return type
    assert!(
        is_null_to_non_optional(&e) || is_type_mismatch(&e),
        "got: {e}"
    );
}

// ── If / block expressions ────────────────────────────────────────────────────

#[test]
fn test_if_as_expression() {
    check_ok(r#"
        fn f(x: int) -> string {
            const label: string = if x > 0 {
                yield "positive"
            } else {
                yield "non-positive"
            }
            return label
        }
    "#);
}

#[test]
fn test_if_condition_must_be_bool() {
    let e = check_err("fn f() { if 42 { return } }");
    assert!(is_type_mismatch(&e), "got: {e}");
}

// ── While / for loops ─────────────────────────────────────────────────────────

#[test]
fn test_while_loop() {
    check_ok("fn f() { let x: int = 0 while x == 0 { x = 1 } }");
}

#[test]
fn test_while_condition_must_be_bool() {
    let e = check_err("fn f() { while 1 { return } }");
    assert!(is_type_mismatch(&e), "got: {e}");
}

#[test]
fn test_for_loop_over_array() {
    check_ok(r#"
        fn sum(items: Array<int>) -> int {
            let total: int = 0
            for item in items {
                total = total + item
            }
            return total
        }
    "#);
}

// ── Panic ─────────────────────────────────────────────────────────────────────

#[test]
fn test_panic_in_fn() {
    check_ok(r#"
        fn f(x: int) -> int {
            if x < 0 {
                panic("negative!")
            }
            return x
        }
    "#);
}

// ── Array literals ────────────────────────────────────────────────────────────

#[test]
fn test_array_literal() {
    check_ok("fn f() -> Array<int> { return [1, 2, 3] }");
}

// ── Named construction ────────────────────────────────────────────────────────

#[test]
fn test_named_construction() {
    check_ok(r#"
        class User { const name: string let score: int }
        fn f() -> User { return User(name: "Alice", score: 0) }
    "#);
}

// ── Generic functions ─────────────────────────────────────────────────────────

#[test]
fn test_generic_fn_definition() {
    check_ok(r#"
        fn identity<T>(x: T) -> T { return x }
    "#);
}

#[test]
fn test_generic_fn_with_bound() {
    check_ok(r#"
        struct Temperature { const value: f64 }
        impl Comparable for Temperature {
            fn compareTo(other: Temperature) -> int { return 0 }
        }
        fn best<T: Comparable>(a: T, b: T) -> T {
            return if a.compareTo(b) >= 0 {
                yield a
            } else {
                yield b
            }
        }
    "#);
}

// ── Match expression ──────────────────────────────────────────────────────────

#[test]
fn test_match_on_error() {
    check_ok(r#"
        class NetworkError extends Error { const statusCode: int }
        fn handleErr(err: Error?) {
            if err != null {
                match err {
                    NetworkError e { return }
                    Error e        { return }
                }
            }
        }
    "#);
}

// ── Full program from spec ────────────────────────────────────────────────────

#[test]
fn test_full_spec_stack() {
    check_ok(r#"
        struct Stack<T> {
            let items: Array<T>

            fn push(item: T) {
                _ = items.append(item)
            }

            fn pop() -> (T?, Error?) {
                if items.length == 0 {
                    return null, Error(message: "Stack is empty")
                }
                return null, null
            }
        }
    "#);
}

#[test]
fn test_full_spec_user_class() {
    check_ok(r#"
        class User {
            const name: string
            let score: int

            fn greet() -> string {
                return "Hello, "
            }
        }

        impl Printable for User {
            fn toString() -> string {
                return "User"
            }
        }
    "#);
}

#[test]
fn test_full_spec_error_handling() {
    check_ok(r#"
        fn readFile(path: string) -> (string, Error?) { return "", null }
        fn processFile(path: string) -> (string, Error?) {
            const data, err = readFile(path)
            if err != null { return "", err }
            return data, null
        }
    "#);
}
