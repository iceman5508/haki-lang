/// Integration tests for the Haki parser.
///
/// Each test exercises a specific grammar production and checks that:
///   1. Parsing succeeds (no ParseError).
///   2. The resulting AST has the expected shape.

use haki_ast::*;
use haki_parser::parse;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_ok(src: &str) -> SourceFile {
    parse(src).unwrap_or_else(|e| panic!("parse failed: {e}"))
}

fn single_item(src: &str) -> Item {
    let file = parse_ok(src);
    assert_eq!(file.items.len(), 1, "expected exactly one top-level item");
    file.items.into_iter().next().unwrap()
}

// ── Top-level item: struct ────────────────────────────────────────────────────

#[test]
fn test_struct_empty() {
    let item = single_item("struct Foo {}");
    let ItemKind::Struct(s) = &item.kind else { panic!("expected struct") };
    assert_eq!(s.name.name, "Foo");
    assert!(s.fields.is_empty());
    assert!(s.methods.is_empty());
    assert!(s.type_params.is_empty());
}

#[test]
fn test_struct_with_fields() {
    let item = single_item("struct Point { const x: f64 const y: f64 }");
    let ItemKind::Struct(s) = &item.kind else { panic!("expected struct") };
    assert_eq!(s.fields.len(), 2);
    assert_eq!(s.fields[0].name.name, "x");
    assert_eq!(s.fields[1].name.name, "y");
    assert!(matches!(s.fields[0].mutability, Mut::Const));
    assert!(matches!(s.fields[1].mutability, Mut::Const));
}

#[test]
fn test_struct_with_let_field() {
    let item = single_item("struct S { let score: int }");
    let ItemKind::Struct(s) = &item.kind else { panic!("expected struct") };
    assert!(matches!(s.fields[0].mutability, Mut::Let));
}

#[test]
fn test_struct_with_method() {
    let item = single_item(r#"
        struct Point {
            const x: f64
            const y: f64
            fn distanceTo(other: Point) -> f64 {
                return 0.0
            }
        }
    "#);
    let ItemKind::Struct(s) = &item.kind else { panic!("expected struct") };
    assert_eq!(s.fields.len(), 2);
    assert_eq!(s.methods.len(), 1);
    assert_eq!(s.methods[0].name.name, "distanceTo");
}

#[test]
fn test_struct_generic() {
    let item = single_item("struct Stack<T> { let items: Array<T> }");
    let ItemKind::Struct(s) = &item.kind else { panic!("expected struct") };
    assert_eq!(s.type_params.len(), 1);
    assert_eq!(s.type_params[0].name.name, "T");
    assert!(s.type_params[0].bounds.is_empty());
}

#[test]
fn test_struct_generic_bounded() {
    let item = single_item("struct SortedList<T: Comparable> { let items: Array<T> }");
    let ItemKind::Struct(s) = &item.kind else { panic!("expected struct") };
    assert_eq!(s.type_params[0].bounds.len(), 1);
    assert_eq!(s.type_params[0].bounds[0].name, "Comparable");
}

// ── Top-level item: class ─────────────────────────────────────────────────────

#[test]
fn test_class_empty() {
    let item = single_item("class Foo {}");
    let ItemKind::Class(c) = &item.kind else { panic!("expected class") };
    assert_eq!(c.name.name, "Foo");
    assert!(c.superclass.is_none());
}

#[test]
fn test_class_extends() {
    let item = single_item("class NetworkError extends Error { const statusCode: int }");
    let ItemKind::Class(c) = &item.kind else { panic!("expected class") };
    assert_eq!(c.name.name, "NetworkError");
    assert_eq!(c.superclass.as_ref().unwrap().name, "Error");
    assert_eq!(c.fields.len(), 1);
    assert_eq!(c.fields[0].name.name, "statusCode");
}

#[test]
fn test_class_weak_field() {
    let item = single_item("class Node { let value: int weak next: Node? }");
    let ItemKind::Class(c) = &item.kind else { panic!("expected class") };
    assert_eq!(c.fields.len(), 2);
    assert!(!c.fields[0].is_weak);
    assert!(c.fields[1].is_weak);
    // weak field name
    assert_eq!(c.fields[1].name.name, "next");
}

#[test]
fn test_class_method() {
    let item = single_item(r#"
        class User {
            const name: string
            fn greet() -> string {
                return "hello"
            }
        }
    "#);
    let ItemKind::Class(c) = &item.kind else { panic!("expected class") };
    assert_eq!(c.methods.len(), 1);
    assert_eq!(c.methods[0].name.name, "greet");
}

// ── Top-level item: protocol ──────────────────────────────────────────────────

#[test]
fn test_protocol_empty() {
    let item = single_item("protocol Foo {}");
    let ItemKind::Protocol(p) = &item.kind else { panic!("expected protocol") };
    assert_eq!(p.name.name, "Foo");
    assert!(p.methods.is_empty());
}

#[test]
fn test_protocol_with_methods() {
    let item = single_item(r#"
        protocol Comparable {
            fn compareTo(other: Self) -> int
        }
    "#);
    let ItemKind::Protocol(p) = &item.kind else { panic!("expected protocol") };
    assert_eq!(p.methods.len(), 1);
    assert_eq!(p.methods[0].name.name, "compareTo");
    // Param: `other: Self`
    assert_eq!(p.methods[0].params[0].name.name, "other");
}

#[test]
fn test_protocol_printable() {
    let item = single_item("protocol Printable { fn toString() -> string }");
    let ItemKind::Protocol(p) = &item.kind else { panic!("expected protocol") };
    assert_eq!(p.methods[0].name.name, "toString");
    let Some(ReturnTy::Single(ty)) = &p.methods[0].return_ty else { panic!() };
    let TyKind::Named(n) = &ty.kind else { panic!() };
    assert_eq!(n.name, "string");
}

// ── Top-level item: impl block ────────────────────────────────────────────────

#[test]
fn test_impl_basic() {
    let item = single_item(r#"
        impl Printable for User {
            fn toString() -> string {
                return "User"
            }
        }
    "#);
    let ItemKind::Impl(imp) = &item.kind else { panic!("expected impl") };
    assert_eq!(imp.protocol.name, "Printable");
    assert_eq!(imp.target.name, "User");
    assert_eq!(imp.methods.len(), 1);
}

#[test]
fn test_impl_comparable() {
    let item = single_item(r#"
        impl Comparable for Temperature {
            fn compareTo(other: Self) -> int {
                return 0
            }
        }
    "#);
    let ItemKind::Impl(imp) = &item.kind else { panic!("expected impl") };
    assert_eq!(imp.protocol.name, "Comparable");
    assert_eq!(imp.target.name, "Temperature");
    assert_eq!(imp.methods[0].params[0].name.name, "other");
}

// ── Top-level item: fn ────────────────────────────────────────────────────────

#[test]
fn test_fn_no_params_no_return() {
    let item = single_item("fn doWork() {}");
    let ItemKind::Fn(f) = &item.kind else { panic!("expected fn") };
    assert_eq!(f.name.name, "doWork");
    assert!(f.params.is_empty());
    assert!(f.return_ty.is_none());
}

#[test]
fn test_fn_with_params_and_return() {
    let item = single_item("fn add(a: int, b: int) -> int { return a }");
    let ItemKind::Fn(f) = &item.kind else { panic!("expected fn") };
    assert_eq!(f.params.len(), 2);
    assert_eq!(f.params[0].name.name, "a");
    assert_eq!(f.params[1].name.name, "b");
    let Some(ReturnTy::Single(ty)) = &f.return_ty else { panic!() };
    let TyKind::Named(n) = &ty.kind else { panic!() };
    assert_eq!(n.name, "int");
}

#[test]
fn test_fn_tuple_return() {
    let item = single_item("fn readFile(path: string) -> (string, Error?) { return \"\", null }");
    let ItemKind::Fn(f) = &item.kind else { panic!("expected fn") };
    let Some(ReturnTy::Tuple(tys)) = &f.return_ty else { panic!("expected tuple return") };
    assert_eq!(tys.len(), 2);
    // Second type is `Error?` — optional
    assert!(matches!(tys[1].kind, TyKind::Optional(_)));
}

#[test]
fn test_fn_generic() {
    let item = single_item(r#"
        fn best<T: Comparable>(a: T, b: T) -> T {
            return a
        }
    "#);
    let ItemKind::Fn(f) = &item.kind else { panic!("expected fn") };
    assert_eq!(f.type_params.len(), 1);
    assert_eq!(f.type_params[0].name.name, "T");
    assert_eq!(f.type_params[0].bounds[0].name, "Comparable");
}

#[test]
fn test_fn_generic_multi_bound() {
    let item = single_item("fn foo<T: Comparable & Printable>(x: T) -> T { return x }");
    let ItemKind::Fn(f) = &item.kind else { panic!("expected fn") };
    assert_eq!(f.type_params[0].bounds.len(), 2);
    assert_eq!(f.type_params[0].bounds[0].name, "Comparable");
    assert_eq!(f.type_params[0].bounds[1].name, "Printable");
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[test]
fn test_type_optional() {
    let item = single_item("fn f() -> string? { return null }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let Some(ReturnTy::Single(ty)) = &f.return_ty else { panic!() };
    assert!(matches!(ty.kind, TyKind::Optional(_)));
}

#[test]
fn test_type_generic_nested() {
    let item = single_item("fn f() -> Map<string, Array<int>> { return null }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let Some(ReturnTy::Single(ty)) = &f.return_ty else { panic!() };
    let TyKind::Generic(name, args) = &ty.kind else { panic!() };
    assert_eq!(name.name, "Map");
    assert_eq!(args.len(), 2);
    let TyKind::Generic(inner_name, _) = &args[1].kind else { panic!() };
    assert_eq!(inner_name.name, "Array");
}

// ── Statements ────────────────────────────────────────────────────────────────

#[test]
fn test_stmt_const_binding() {
    let item = single_item("fn f() { const x: int = 42 }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let stmt = &f.body.stmts[0];
    let StmtKind::Let(l) = &stmt.kind else { panic!("expected let") };
    assert!(matches!(l.mutability, Mut::Const));
    assert_eq!(l.bindings.len(), 1);
    let Binding::Name(n) = &l.bindings[0] else { panic!() };
    assert_eq!(n.name, "x");
    // Has type annotation
    assert!(l.ty.is_some());
    // Init is int literal 42
    assert!(matches!(l.init.kind, ExprKind::Int(42)));
}

#[test]
fn test_stmt_let_no_annotation() {
    let item = single_item(r#"fn f() { let name = "Alice" }"#);
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    assert!(matches!(l.mutability, Mut::Let));
    assert!(l.ty.is_none());
}

#[test]
fn test_stmt_multi_binding() {
    let item = single_item(r#"fn f() { const data, err = readFile("f") }"#);
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    assert_eq!(l.bindings.len(), 2);
    let Binding::Name(a) = &l.bindings[0] else { panic!() };
    let Binding::Name(b) = &l.bindings[1] else { panic!() };
    assert_eq!(a.name, "data");
    assert_eq!(b.name, "err");
}

#[test]
fn test_stmt_discard_binding() {
    let item = single_item(r#"fn f() { const data, _ = readFile("f") }"#);
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    assert_eq!(l.bindings.len(), 2);
    assert!(matches!(l.bindings[1], Binding::Discard(_)));
}

#[test]
fn test_stmt_discard_all() {
    let item = single_item(r#"fn f() { _ = logEvent("ping") }"#);
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    assert_eq!(l.bindings.len(), 1);
    assert!(matches!(l.bindings[0], Binding::Discard(_)));
}

#[test]
fn test_stmt_return_void() {
    let item = single_item("fn f() { return }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Return(r) = &f.body.stmts[0].kind else { panic!() };
    assert!(r.values.is_empty());
}

#[test]
fn test_stmt_return_value() {
    let item = single_item("fn f() -> int { return 42 }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Return(r) = &f.body.stmts[0].kind else { panic!() };
    assert_eq!(r.values.len(), 1);
    assert!(matches!(r.values[0].kind, ExprKind::Int(42)));
}

#[test]
fn test_stmt_return_multi() {
    let item = single_item("fn f() { return null, null }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Return(r) = &f.body.stmts[0].kind else { panic!() };
    assert_eq!(r.values.len(), 2);
}

#[test]
fn test_stmt_panic() {
    let item = single_item(r#"fn f() { panic("oh no") }"#);
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    assert!(matches!(f.body.stmts[0].kind, StmtKind::Panic(_)));
}

#[test]
fn test_stmt_while() {
    let item = single_item("fn f() { while x { return } }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    assert!(matches!(f.body.stmts[0].kind, StmtKind::While(_)));
}

#[test]
fn test_stmt_for_in() {
    let item = single_item("fn f() { for x in items { return } }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::For(fr) = &f.body.stmts[0].kind else { panic!() };
    assert_eq!(fr.var.name, "x");
}

#[test]
fn test_stmt_yield() {
    let item = single_item(r#"fn f() { yield "hello" }"#);
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    assert!(matches!(f.body.stmts[0].kind, StmtKind::Yield(_)));
}

// ── Expressions ───────────────────────────────────────────────────────────────

#[test]
fn test_expr_literals() {
    let item = single_item(r#"fn f() { const a = 42 const b = 3.14 const c = "hi" const d = true const e = false const g = null }"#);
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    assert!(matches!(f.body.stmts[0].kind, StmtKind::Let(_)));
}

#[test]
fn test_expr_binary_arithmetic() {
    let item = single_item("fn f() { const x = 1 + 2 * 3 }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    // Should be `1 + (2 * 3)` due to precedence
    let ExprKind::Binary(BinaryOp::Add, lhs, rhs) = &l.init.kind else {
        panic!("expected addition at top level")
    };
    assert!(matches!(lhs.kind, ExprKind::Int(1)));
    assert!(matches!(rhs.kind, ExprKind::Binary(BinaryOp::Mul, _, _)));
}

#[test]
fn test_expr_comparison() {
    let item = single_item("fn f() { const x = a == b }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    assert!(matches!(l.init.kind, ExprKind::Binary(BinaryOp::Eq, _, _)));
}

#[test]
fn test_expr_unary_neg() {
    let item = single_item("fn f() { const x = -42 }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    assert!(matches!(l.init.kind, ExprKind::Unary(UnaryOp::Neg, _)));
}

#[test]
fn test_expr_unary_not() {
    let item = single_item("fn f() { const x = !flag }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    assert!(matches!(l.init.kind, ExprKind::Unary(UnaryOp::Not, _)));
}

#[test]
fn test_expr_field_access() {
    let item = single_item("fn f() { const x = user.name }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    let ExprKind::Field(_, field) = &l.init.kind else { panic!("expected field") };
    assert_eq!(field.name, "name");
}

#[test]
fn test_expr_method_call() {
    let item = single_item("fn f() { const x = a.compareTo(b) }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    let ExprKind::MethodCall(_, method, args) = &l.init.kind else { panic!("expected method call") };
    assert_eq!(method.name, "compareTo");
    assert_eq!(args.len(), 1);
}

#[test]
fn test_expr_function_call_positional() {
    let item = single_item("fn f() { const x = add(1, 2) }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    let ExprKind::Call(_, args) = &l.init.kind else { panic!("expected call") };
    assert_eq!(args.len(), 2);
}

#[test]
fn test_expr_function_call_named() {
    let item = single_item(r#"fn f() { const u = User(name: "Alice", score: 0) }"#);
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    let ExprKind::NamedCall(_, args) = &l.init.kind else { panic!("expected named call") };
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].name.name, "name");
    assert_eq!(args[1].name.name, "score");
}

#[test]
fn test_expr_index() {
    let item = single_item("fn f() { const x = arr[0] }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    assert!(matches!(l.init.kind, ExprKind::Index(_, _)));
}

#[test]
fn test_expr_array_literal() {
    let item = single_item("fn f() { const x = [1, 2, 3] }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    let ExprKind::Array(elems) = &l.init.kind else { panic!() };
    assert_eq!(elems.len(), 3);
}

#[test]
fn test_expr_if_as_value() {
    let item = single_item(r#"
        fn f() {
            const label: string = if score > 100 {
                yield "Expert"
            } else {
                yield "Beginner"
            }
        }
    "#);
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    assert!(matches!(l.init.kind, ExprKind::If(_)));
}

#[test]
fn test_expr_if_else_chain() {
    let item = single_item(r#"
        fn f() {
            if x {
                return
            } else if y {
                return
            } else {
                return
            }
        }
    "#);
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::If(if_expr) = &f.body.stmts[0].kind else { panic!() };
    let Some(ElseBranch::If(inner_if)) = &if_expr.else_branch else { panic!("expected else-if") };
    assert!(matches!(inner_if.else_branch, Some(ElseBranch::Block(_))));
}

#[test]
fn test_expr_match() {
    let item = single_item(r#"
        fn f() {
            match err {
                NetworkError e { return }
                Error e { return }
            }
        }
    "#);
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Match(m) = &f.body.stmts[0].kind else { panic!() };
    assert_eq!(m.arms.len(), 2);
    assert!(matches!(&m.arms[0].pattern, MatchPattern::Ident(id) if id.name == "NetworkError"));
    assert_eq!(m.arms[0].bindings[0].name, "e");
    assert!(matches!(&m.arms[1].pattern, MatchPattern::Ident(id) if id.name == "Error"));
}

#[test]
fn test_expr_logical_and_or() {
    let item = single_item("fn f() { const x = a && b || c }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    // `&&` binds tighter: `(a && b) || c`
    let ExprKind::Binary(BinaryOp::Or, lhs, _) = &l.init.kind else { panic!() };
    assert!(matches!(lhs.kind, ExprKind::Binary(BinaryOp::And, _, _)));
}

#[test]
fn test_expr_chained_field_method() {
    let item = single_item("fn f() { const x = user.name.toString() }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Let(l) = &f.body.stmts[0].kind else { panic!() };
    // outermost: MethodCall on (user.name)
    let ExprKind::MethodCall(recv, method, _) = &l.init.kind else { panic!() };
    assert_eq!(method.name, "toString");
    assert!(matches!(recv.kind, ExprKind::Field(_, _)));
}

#[test]
fn test_expr_assignment() {
    let item = single_item("fn f() { x = 42 }");
    let ItemKind::Fn(f) = &item.kind else { panic!() };
    let StmtKind::Expr(e) = &f.body.stmts[0].kind else { panic!() };
    assert!(matches!(e.kind, ExprKind::Assign(_, _)));
}

// ── Full program snippets from the spec ──────────────────────────────────────

#[test]
fn test_full_stack_struct() {
    let src = r#"
        struct Stack<T> {
            let items: Array<T>

            fn push(item: T) {
                _ = items.append(item)
            }

            fn pop() -> (T?, Error?) {
                if items.length == 0 {
                    return null, Error(message: "Stack is empty")
                }
                return items.removeLast(), null
            }
        }
    "#;
    let file = parse_ok(src);
    assert_eq!(file.items.len(), 1);
    let ItemKind::Struct(s) = &file.items[0].kind else { panic!() };
    assert_eq!(s.name.name, "Stack");
    assert_eq!(s.methods.len(), 2);
}

#[test]
fn test_full_user_class_with_impl() {
    let src = r#"
        class User {
            const name: string
            let score: int
            weak team: Team?

            fn greet() -> string {
                return "Hello, "
            }
        }

        impl Printable for User {
            fn toString() -> string {
                return "User"
            }
        }
    "#;
    let file = parse_ok(src);
    assert_eq!(file.items.len(), 2);
    assert!(matches!(file.items[0].kind, ItemKind::Class(_)));
    assert!(matches!(file.items[1].kind, ItemKind::Impl(_)));
}

#[test]
fn test_full_error_handling() {
    let src = r#"
        fn loadUser(id: int) -> (User?, Error?) {
            const row, err = db.query("SELECT * FROM users WHERE id = ?", id)
            if err != null { return null, err }
            return User(name: row.name, score: row.score), null
        }
    "#;
    let file = parse_ok(src);
    assert_eq!(file.items.len(), 1);
    let ItemKind::Fn(f) = &file.items[0].kind else { panic!() };
    assert_eq!(f.name.name, "loadUser");
    assert_eq!(f.body.stmts.len(), 3);
}

#[test]
fn test_full_match_on_error() {
    let src = r#"
        fn f() {
            const data, err = fetchUser(id)
            if err != null {
                match err {
                    NetworkError e { return }
                    Error e        { return }
                }
            }
        }
    "#;
    let file = parse_ok(src);
    assert_eq!(file.items.len(), 1);
}

#[test]
fn test_multiple_top_level_items() {
    let src = r#"
        protocol Comparable { fn compareTo(other: Self) -> int }
        protocol Printable  { fn toString() -> string }
        struct Point { const x: f64 const y: f64 }
        class User { const name: string let score: int }
        impl Printable for User {
            fn toString() -> string { return "User" }
        }
        fn best<T: Comparable>(a: T, b: T) -> T { return a }
    "#;
    let file = parse_ok(src);
    assert_eq!(file.items.len(), 6);
    assert!(matches!(file.items[0].kind, ItemKind::Protocol(_)));
    assert!(matches!(file.items[1].kind, ItemKind::Protocol(_)));
    assert!(matches!(file.items[2].kind, ItemKind::Struct(_)));
    assert!(matches!(file.items[3].kind, ItemKind::Class(_)));
    assert!(matches!(file.items[4].kind, ItemKind::Impl(_)));
    assert!(matches!(file.items[5].kind, ItemKind::Fn(_)));
}

// ── Error cases ───────────────────────────────────────────────────────────────

#[test]
fn test_error_missing_brace() {
    assert!(parse("struct Foo {").is_err());
}

#[test]
fn test_error_unexpected_token() {
    assert!(parse("42").is_err()); // bare literal is not a top-level item
}

#[test]
fn test_error_missing_fn_body() {
    assert!(parse("fn f()").is_err());
}

#[test]
fn test_error_unterminated_string() {
    assert!(parse(r#"fn f() { const x = "oops }"#).is_err());
}

// ── Error recovery tests ──────────────────────────────────────────────────────

#[test]
fn test_recovery_single_bad_item() {
    // One broken item followed by one good item.
    // The broken item is skipped; the good one is kept.
    // Missing function name — valid tokens, invalid syntax
    let result = haki_parser::parse_recovery(
        "fn (a: int) { }\nfn add(a: int, b: int) -> int { return a + b }\n"
    );
    assert!(!result.is_ok(), "should have at least one error");
    assert_eq!(result.ast.items.len(), 1, "good item should be in AST");
    assert!(!result.errors.is_empty());
}

#[test]
fn test_recovery_multiple_errors() {
    // Missing function name, then good, then struct missing name, then good
    let result = haki_parser::parse_recovery(
        "fn (x: int) { }\nfn good1() { }\nstruct { }\nfn good2() { }\n"
    );
    assert_eq!(result.ast.items.len(), 2, "two good items should survive");
    assert_eq!(result.errors.len(), 2, "two errors");
}

#[test]
fn test_recovery_unclosed_paren() {
    // Broken struct body (statement inside struct at top level = parse error)
    // followed by a good fn — recovery must skip the broken struct and keep fine()
    let result = haki_parser::parse_recovery(
        "struct Bad { let x = 1 + 2 }\nfn fine() -> int { return 42 }\n"
    );
    assert!(!result.errors.is_empty(), "should have errors");
    // fine() should survive after recovery skips Bad
    assert!(result.ast.items.iter().any(|item| {
        matches!(&item.kind, haki_ast::ItemKind::Fn(f) if f.name.name == "fine")
    }), "fine() not found in AST; items: {:?}", result.ast.items.len());
}

#[test]
fn test_recovery_all_valid() {
    // No errors — parse_recovery and parse agree.
    let result = haki_parser::parse_recovery(r#"
fn add(a: int, b: int) -> int { return a + b }
fn sub(a: int, b: int) -> int { return a - b }
"#);
    assert!(result.is_ok());
    assert_eq!(result.ast.items.len(), 2);
    assert!(result.errors.is_empty());
}

#[test]
fn test_recovery_error_span() {
    // Errors should carry valid spans pointing into the source.
    let src = "fn @@@bad {\nfn ok() { }\n";
    let result = haki_parser::parse_recovery(src);
    assert!(!result.errors.is_empty());
    for err in &result.errors {
        let span = err.span();
        assert!(span.lo <= span.hi, "error span lo <= hi");
    }
}
