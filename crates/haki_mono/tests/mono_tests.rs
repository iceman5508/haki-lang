/// Integration tests for haki_mono.
///
/// Each test exercises the full pipeline: parse → typecheck → monomorphize,
/// then asserts on the shape of the MonoProgram output.

use haki_mono::{monomorphize, mono_ast::*};
use haki_typeck::typecheck;
use haki_parser::parse;
use haki_typeck::typed_ast::SemTy;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn mono_ok(src: &str) -> MonoProgram {
    let ast = parse(src).unwrap_or_else(|e| panic!("parse failed: {e}"));
    let typed = typecheck(&ast).unwrap_or_else(|e| panic!("typecheck failed: {e}"));
    monomorphize(&typed).unwrap_or_else(|e| panic!("mono failed: {e}"))
}

fn find_fn<'a>(prog: &'a MonoProgram, name: &str) -> Option<&'a MonoFn> {
    prog.fns.iter().find(|f| f.name == name)
}

#[allow(dead_code)]
fn find_fn_prefix<'a>(prog: &'a MonoProgram, prefix: &str) -> Option<&'a MonoFn> {
    prog.fns.iter().find(|f| f.name.starts_with(prefix))
}

fn find_struct<'a>(prog: &'a MonoProgram, name: &str) -> Option<&'a MonoStruct> {
    prog.structs.iter().find(|s| s.name == name)
}

fn find_class<'a>(prog: &'a MonoProgram, name: &str) -> Option<&'a MonoClass> {
    prog.classes.iter().find(|c| c.name == name)
}

fn find_impl<'a>(prog: &'a MonoProgram, proto: &str, target: &str) -> Option<&'a MonoImpl> {
    prog.impls.iter().find(|i| i.protocol == proto && i.target == target)
}

// ── Non-generic items pass through unchanged ──────────────────────────────────

#[test]
fn test_simple_fn_emitted() {
    let prog = mono_ok("fn add(a: int, b: int) -> int { return a }");
    let f = find_fn(&prog, "add").expect("add not emitted");
    assert_eq!(f.params.len(), 2);
    assert_eq!(f.params[0].name, "a");
    assert_eq!(f.params[0].ty, SemTy::Int);
    assert_eq!(f.params[1].name, "b");
    assert_eq!(f.params[1].ty, SemTy::Int);
    assert_eq!(f.return_ty, SemTy::Int);
}

#[test]
fn test_void_fn_emitted() {
    let prog = mono_ok("fn doWork() { return }");
    let f = find_fn(&prog, "doWork").expect("doWork not emitted");
    assert_eq!(f.params.len(), 0);
    assert_eq!(f.return_ty, SemTy::Void);
}

#[test]
fn test_struct_emitted() {
    let prog = mono_ok("struct Point { const x: f64 const y: f64 }");
    let s = find_struct(&prog, "Point").expect("Point not emitted");
    assert_eq!(s.fields.len(), 2);
    assert_eq!(s.fields[0].name, "x");
    assert_eq!(s.fields[0].ty, SemTy::Float);
    assert_eq!(s.fields[1].name, "y");
    assert_eq!(s.fields[1].ty, SemTy::Float);
}

#[test]
fn test_class_emitted() {
    let prog = mono_ok("class User { const name: string let score: int }");
    let c = find_class(&prog, "User").expect("User not emitted");
    assert_eq!(c.fields.len(), 2);
    assert_eq!(c.fields[0].name, "name");
    assert_eq!(c.fields[0].ty, SemTy::String);
    assert_eq!(c.fields[1].name, "score");
    assert_eq!(c.fields[1].ty, SemTy::Int);
    assert!(c.superclass.is_none());
}

#[test]
fn test_class_with_superclass() {
    let prog = mono_ok(r#"
        class NetworkError extends Error { const statusCode: int }
    "#);
    let c = find_class(&prog, "NetworkError").expect("NetworkError not emitted");
    assert_eq!(c.superclass.as_deref(), Some("Error"));
}

#[test]
fn test_impl_emitted() {
    let prog = mono_ok(r#"
        struct Temperature { const value: f64 }
        impl Comparable for Temperature {
            fn compareTo(other: Temperature) -> int { return 0 }
        }
    "#);
    let i = find_impl(&prog, "Comparable", "Temperature")
        .expect("impl not emitted");
    assert_eq!(i.methods.len(), 1);
    assert_eq!(i.methods[0].name, "Temperature__compareTo");
}

// ── Return type preservation ──────────────────────────────────────────────────

#[test]
fn test_tuple_return_ty() {
    let prog = mono_ok(
        "fn readFile(path: string) -> (string, Error?) { return \"\", null }"
    );
    let f = find_fn(&prog, "readFile").expect("readFile not found");
    assert!(
        matches!(&f.return_ty, SemTy::Tuple(tys) if tys.len() == 2),
        "expected tuple return, got {:?}", f.return_ty
    );
}

#[test]
fn test_optional_return_ty() {
    let prog = mono_ok("fn maybeInt() -> int? { return null }");
    let f = find_fn(&prog, "maybeInt").expect("not found");
    assert!(matches!(&f.return_ty, SemTy::Optional(inner) if **inner == SemTy::Int));
}

// ── Statement lowering ────────────────────────────────────────────────────────

#[test]
fn test_let_stmt_in_body() {
    let prog = mono_ok("fn f() { const x: int = 42 }");
    let f = find_fn(&prog, "f").unwrap();
    assert_eq!(f.body.stmts.len(), 1);
    assert!(matches!(f.body.stmts[0].kind, MonoStmtKind::Let(_)));
}

#[test]
fn test_multi_binding_stmt() {
    let prog = mono_ok(r#"
        fn readFile(path: string) -> (string, Error?) { return "", null }
        fn f() { const data, err = readFile("x.txt") }
    "#);
    let f = find_fn(&prog, "f").unwrap();
    let MonoStmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    assert_eq!(l.bindings.len(), 2);
}

#[test]
fn test_return_stmt_lowered() {
    let prog = mono_ok("fn f() -> int { return 42 }");
    let f = find_fn(&prog, "f").unwrap();
    let MonoStmtKind::Return(r) = &f.body.stmts[0].kind else { panic!() };
    assert_eq!(r.values.len(), 1);
    assert!(matches!(r.values[0].kind, MonoExprKind::Int(42)));
}

#[test]
fn test_panic_stmt_lowered() {
    let prog = mono_ok(r#"fn f() { panic("bad") }"#);
    let f = find_fn(&prog, "f").unwrap();
    assert!(matches!(f.body.stmts[0].kind, MonoStmtKind::Panic(_)));
}

#[test]
fn test_while_lowered() {
    let prog = mono_ok("fn f() { let x: int = 0 while x == 0 { x = 1 } }");
    let f = find_fn(&prog, "f").unwrap();
    assert!(matches!(f.body.stmts[1].kind, MonoStmtKind::While(_)));
}

#[test]
fn test_for_lowered() {
    let prog = mono_ok(r#"
        fn sum(items: Array<int>) -> int {
            let total: int = 0
            for item in items { total = total + item }
            return total
        }
    "#);
    let f = find_fn(&prog, "sum").unwrap();
    assert!(matches!(f.body.stmts[1].kind, MonoStmtKind::For(_)));
    if let MonoStmtKind::For(fr) = &f.body.stmts[1].kind {
        assert_eq!(fr.var.name, "item");
        assert_eq!(fr.var_ty, SemTy::Int);
    }
}

#[test]
fn test_if_lowered() {
    let prog = mono_ok(r#"
        fn f(x: int) -> string {
            const label: string = if x > 0 {
                yield "pos"
            } else {
                yield "neg"
            }
            return label
        }
    "#);
    let f = find_fn(&prog, "f").unwrap();
    let MonoStmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    assert!(matches!(l.init.kind, MonoExprKind::If(_)));
}

#[test]
fn test_match_lowered() {
    let prog = mono_ok(r#"
        class NetworkError extends Error { const statusCode: int }
        fn f(err: Error?) {
            if err != null {
                match err {
                    NetworkError e { return }
                    Error e { return }
                }
            }
        }
    "#);
    let f = find_fn(&prog, "f").unwrap();
    // First stmt is the if, inside it there's the match.
    assert!(matches!(f.body.stmts[0].kind, MonoStmtKind::If(_)));
}

// ── Expression lowering ───────────────────────────────────────────────────────

#[test]
fn test_literals_lowered() {
    let prog = mono_ok(r#"fn f() {
        const a = 42
        const b = 3.14
        const c = "hi"
        const d = true
        const e = null
    }"#);
    let f = find_fn(&prog, "f").unwrap();
    fn init(stmt: &MonoStmt) -> &MonoExprKind {
        let MonoStmtKind::Let(l) = &stmt.kind else { panic!() };
        &l.init.kind
    }
    assert!(matches!(init(&f.body.stmts[0]), MonoExprKind::Int(42)));
    assert!(matches!(init(&f.body.stmts[1]), MonoExprKind::Float(_)));
    assert!(matches!(init(&f.body.stmts[2]), MonoExprKind::String(_)));
    assert!(matches!(init(&f.body.stmts[3]), MonoExprKind::Bool(true)));
    assert!(matches!(init(&f.body.stmts[4]), MonoExprKind::Null));
}

#[test]
fn test_binary_expr_lowered() {
    let prog = mono_ok("fn f() -> int { return 1 + 2 }");
    let f = find_fn(&prog, "f").unwrap();
    let MonoStmtKind::Return(r) = &f.body.stmts[0].kind else { panic!() };
    assert!(matches!(r.values[0].kind, MonoExprKind::Binary(_, _, _)));
}

#[test]
fn test_unary_expr_lowered() {
    let prog = mono_ok("fn f() -> int { return -1 }");
    let f = find_fn(&prog, "f").unwrap();
    let MonoStmtKind::Return(r) = &f.body.stmts[0].kind else { panic!() };
    assert!(matches!(r.values[0].kind, MonoExprKind::Unary(_, _)));
}

#[test]
fn test_field_access_lowered() {
    let prog = mono_ok(r#"
        struct Point { const x: f64 }
        fn f(p: Point) -> f64 { return p.x }
    "#);
    let f = find_fn(&prog, "f").unwrap();
    let MonoStmtKind::Return(r) = &f.body.stmts[0].kind else { panic!() };
    assert!(matches!(&r.values[0].kind, MonoExprKind::Field(_, field) if field == "x"));
}

#[test]
fn test_fn_call_lowered() {
    let prog = mono_ok(r#"
        fn add(a: int, b: int) -> int { return a }
        fn f() -> int { return add(1, 2) }
    "#);
    let f = find_fn(&prog, "f").unwrap();
    let MonoStmtKind::Return(r) = &f.body.stmts[0].kind else { panic!() };
    assert!(matches!(&r.values[0].kind, MonoExprKind::Call(name, _) if name == "add"));
}

#[test]
fn test_named_call_lowered() {
    let prog = mono_ok(r#"
        class User { const name: string let score: int }
        fn f() -> User { return User(name: "Alice", score: 0) }
    "#);
    let f = find_fn(&prog, "f").unwrap();
    let MonoStmtKind::Return(r) = &f.body.stmts[0].kind else { panic!() };
    assert!(matches!(
        &r.values[0].kind,
        MonoExprKind::Construct(name, args) if name == "User" && args.len() == 2
    ));
}

#[test]
fn test_method_call_lowered() {
    let prog = mono_ok(r#"
        struct Temperature { const value: f64 }
        impl Comparable for Temperature {
            fn compareTo(other: Temperature) -> int { return 0 }
        }
        fn f(a: Temperature, b: Temperature) -> int {
            return a.compareTo(b)
        }
    "#);
    let f = find_fn(&prog, "f").unwrap();
    let MonoStmtKind::Return(r) = &f.body.stmts[0].kind else { panic!() };
    // Method call becomes a plain Call with the receiver as first arg.
    assert!(matches!(
        &r.values[0].kind,
        MonoExprKind::Call(name, args) if name.contains("compareTo") && args.len() == 2
    ));
}

#[test]
fn test_array_literal_lowered() {
    let prog = mono_ok("fn f() -> Array<int> { return [1, 2, 3] }");
    let f = find_fn(&prog, "f").unwrap();
    let MonoStmtKind::Return(r) = &f.body.stmts[0].kind else { panic!() };
    assert!(matches!(&r.values[0].kind, MonoExprKind::Array(elems) if elems.len() == 3));
}

#[test]
fn test_assign_lowered() {
    let prog = mono_ok("fn f() { let x: int = 0 x = 42 }");
    let f = find_fn(&prog, "f").unwrap();
    assert!(matches!(f.body.stmts[1].kind, MonoStmtKind::Expr(_)));
    let MonoStmtKind::Expr(e) = &f.body.stmts[1].kind else { panic!() };
    assert!(matches!(e.kind, MonoExprKind::Assign(_, _)));
}

// ── Type substitution in mono output ─────────────────────────────────────────

#[test]
fn test_subst_propagates_through_block() {
    // A non-generic function that uses optional types.
    let prog = mono_ok(r#"
        fn f(x: int?) -> int? {
            if x != null { return x }
            return null
        }
    "#);
    let f = find_fn(&prog, "f").unwrap();
    assert!(matches!(
        &f.return_ty,
        SemTy::Optional(inner) if **inner == SemTy::Int
    ));
    assert_eq!(f.params[0].ty, SemTy::Optional(Box::new(SemTy::Int)));
}

// ── Generic function NOT emitted until instantiated ───────────────────────────

#[test]
fn test_generic_fn_not_emitted_without_call() {
    let prog = mono_ok("fn identity<T>(x: T) -> T { return x }");
    // No instantiation means no concrete version in the program.
    assert!(
        find_fn(&prog, "identity").is_none(),
        "generic fn should not be emitted without instantiation"
    );
}

#[test]
fn test_generic_struct_not_emitted_without_use() {
    let prog = mono_ok("struct Box<T> { const value: T }");
    assert!(
        find_struct(&prog, "Box").is_none(),
        "generic struct should not be emitted without use"
    );
}

// ── Name mangling ─────────────────────────────────────────────────────────────

#[test]
fn test_mangle_subst_unit() {
    // Direct unit test of the mangle helper.
    use haki_mono::subst::mangle;
    assert_eq!(mangle("foo", &[]), "foo");
    assert_eq!(mangle("identity", &[SemTy::Int]), "identity__int");
    assert_eq!(mangle("best", &[SemTy::Named("Temperature".into())]), "best__Temperature");
}

// ── Full program round-trips ──────────────────────────────────────────────────

#[test]
fn test_full_non_generic_program() {
    let prog = mono_ok(r#"
        class User {
            const name: string
            let score: int
        }
        impl Printable for User {
            fn toString() -> string { return "User" }
        }
        fn greet(u: User) -> string {
            return u.name
        }
    "#);
    assert!(find_class(&prog, "User").is_some());
    assert!(find_impl(&prog, "Printable", "User").is_some());
    assert!(find_fn(&prog, "greet").is_some());
}

#[test]
fn test_full_error_handling_program() {
    let prog = mono_ok(r#"
        fn readFile(path: string) -> (string, Error?) { return "", null }
        fn process(path: string) -> (string, Error?) {
            const data, err = readFile(path)
            if err != null { return "", err }
            return data, null
        }
    "#);
    assert!(find_fn(&prog, "readFile").is_some());
    assert!(find_fn(&prog, "process").is_some());

    // process's return type is a Tuple.
    let f = find_fn(&prog, "process").unwrap();
    assert!(matches!(&f.return_ty, SemTy::Tuple(tys) if tys.len() == 2));
}

#[test]
fn test_struct_with_method() {
    let prog = mono_ok(r#"
        struct Point {
            const x: f64
            const y: f64
            fn getX() -> f64 { return 0.0 }
        }
    "#);
    let s = find_struct(&prog, "Point").unwrap();
    assert_eq!(s.fields.len(), 2);
    assert_eq!(s.methods.len(), 1);
    assert_eq!(s.methods[0].name, "Point__getX");
    assert_eq!(s.methods[0].return_ty, SemTy::Float);
}

#[test]
fn test_discard_binding_in_mono() {
    let prog = mono_ok(r#"
        fn logEvent(msg: string) -> bool { return true }
        fn f() { _ = logEvent("ping") }
    "#);
    let f = find_fn(&prog, "f").unwrap();
    // The `_ = expr` becomes a Let with a Discard binding.
    assert!(matches!(f.body.stmts[0].kind, MonoStmtKind::Let(_)));
}

#[test]
fn test_yield_in_block_expr() {
    let prog = mono_ok(r#"
        fn f(x: int) -> string {
            const label: string = if x > 0 {
                yield "positive"
            } else {
                yield "non-positive"
            }
            return label
        }
    "#);
    let f = find_fn(&prog, "f").unwrap();
    assert_eq!(f.body.stmts.len(), 2);
    let MonoStmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    // If-expr produces a string
    assert_eq!(l.init.ty, SemTy::String);
}

#[test]
fn test_multiple_items_emitted() {
    let prog = mono_ok(r#"
        struct Point { const x: f64 const y: f64 }
        class User { const name: string }
        fn add(a: int, b: int) -> int { return a }
        fn sub(a: int, b: int) -> int { return a }
    "#);
    assert_eq!(prog.structs.len(), 1);
    assert_eq!(prog.classes.len(), 1);
    assert_eq!(prog.fns.len(), 2);
}
