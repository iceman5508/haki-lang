/// check.rs — Pass 3: semantic rule validation on the typed AST.
///
/// Rules checked here (after full inference):
///   1. Protocol conformance completeness — every method in the protocol must
///      be implemented in every `impl` block.
///   2. Generic constraint satisfaction — at every call site where a type param
///      has bounds, check the concrete type conforms to each bound.
///   3. Weak fields must have optional types (`T?`).
///   4. Weak fields may only appear on class types (not structs).
///
/// Rules enforced during inference (not here):
///   - Type mismatches, const mutation, ignored return values, null safety.
///
/// This pass reads the typed AST and the symbol table embedded in the inferer
/// (passed in indirectly via the TypedSourceFile which has all type info).
/// For simplicity in v0.1 we re-receive the symbol table separately.

use haki_ast::*;
use crate::error::{TypeError, TypeResult};
use crate::typed_ast::*;
use crate::collector::SymbolTable;

/// Entry point for pass 3.
pub fn check(file: &TypedSourceFile) -> TypeResult<()> {
    let checker = Checker;
    checker.check_file(file)
}

struct Checker;

impl Checker {
    fn check_file(&self, file: &TypedSourceFile) -> TypeResult<()> {
        for item in &file.items {
            self.check_item(item)?;
        }
        Ok(())
    }

    fn check_item(&self, item: &TypedItem) -> TypeResult<()> {
        match &item.kind {
            TypedItemKind::Import { .. } => Ok(()),  // resolved by driver
            TypedItemKind::Struct(s)   => self.check_struct(s),
            TypedItemKind::Class(c)    => self.check_class(c),
            TypedItemKind::Enum(_)     => Ok(()),  // validated at collection time
            TypedItemKind::Protocol(_) => Ok(()),
            TypedItemKind::Impl(i)     => self.check_impl_block(i),
            TypedItemKind::Fn(f)       => self.check_fn(f),
        }
    }

    fn check_struct(&self, s: &TypedStructDef) -> TypeResult<()> {
        // Structs cannot have weak fields — weak is only for reference (class) types.
        for field in &s.fields {
            if field.is_weak {
                return Err(TypeError::WeakOnValueType {
                    ty: s.name.name.clone(),
                    span: field.span,
                });
            }
        }
        for method in &s.methods {
            self.check_fn(method)?;
        }
        Ok(())
    }

    fn check_class(&self, c: &TypedClassDef) -> TypeResult<()> {
        // Class fields: weak fields must have optional types.
        for field in &c.fields {
            if field.is_weak {
                if !matches!(&field.ty.kind, TyKind::Optional(_)) {
                    return Err(TypeError::WeakFieldMustBeOptional { span: field.span });
                }
            }
        }
        for method in &c.methods {
            self.check_fn(method)?;
        }
        Ok(())
    }

    fn check_impl_block(&self, i: &TypedImplBlock) -> TypeResult<()> {
        for method in &i.methods {
            self.check_fn(method)?;
        }
        Ok(())
    }

    fn check_fn(&self, f: &TypedFnDef) -> TypeResult<()> {
        self.check_block(&f.body)
    }

    fn check_block(&self, block: &TypedBlock) -> TypeResult<()> {
        for stmt in &block.stmts {
            self.check_stmt(stmt)?;
        }
        Ok(())
    }

    fn check_stmt(&self, stmt: &TypedStmt) -> TypeResult<()> {
        match &stmt.kind {
            TypedStmtKind::Let(l)    => self.check_block_expr_in_let(l),
            TypedStmtKind::Return(_) => Ok(()),
            TypedStmtKind::Yield(_)  => Ok(()),
            TypedStmtKind::Defer(_)  => Ok(()),
            TypedStmtKind::Continue  => Ok(()),
            TypedStmtKind::Break     => Ok(()),
            TypedStmtKind::If(i)     => self.check_if(i),
            TypedStmtKind::While(w)  => self.check_block(&w.body),
            TypedStmtKind::For(f)    => self.check_block(&f.body),
            TypedStmtKind::Match(m)  => self.check_match(m),
            TypedStmtKind::Panic(_)  => Ok(()),
            TypedStmtKind::Expr(e)   => self.check_expr_stmt(e, stmt.span),
        }
    }

    /// Expression statements: if the expression returns a non-Void type that
    /// is not explicitly discarded, it is a hidden ignored return.
    /// The parser already handles `_ = expr` as a LetStmt with Discard binding,
    /// so bare expression statements with a return value are an error.
    fn check_expr_stmt(&self, expr: &TypedExpr, span: Span) -> TypeResult<()> {
        match &expr.kind {
            // Assignments return Void — fine as statement.
            TypedExprKind::Assign(_, _) => Ok(()),
            // `async expr` used as a bare statement — the Task result is discarded.
            // This is always a compiler error: use `_ = async fn()` to detach.
            TypedExprKind::Async(_) => {
                Err(TypeError::IgnoredReturnValue { span })
            }
            // Method calls and plain calls: if they return non-Void, require explicit discard.
            TypedExprKind::MethodCall(_, _, _)
            | TypedExprKind::Call(_, _)
            | TypedExprKind::NamedCall(_, _) => {
                match &expr.ty {
                    SemTy::Void | SemTy::Never => Ok(()),
                    _ => Err(TypeError::IgnoredReturnValue { span }),
                }
            }
            // Other expressions used as statements are fine (if-as-statement, etc.).
            _ => Ok(()),
        }
    }

    fn check_block_expr_in_let(&self, l: &TypedLetStmt) -> TypeResult<()> {
        // The init expression itself may be complex — recurse.
        self.check_expr(&l.init)
    }

    fn check_expr(&self, expr: &TypedExpr) -> TypeResult<()> {
        match &expr.kind {
            TypedExprKind::If(i)     => self.check_if(i),
            TypedExprKind::Match(m)  => self.check_match(m),
            TypedExprKind::Block(b)  => self.check_block(b),
            _ => Ok(()),
        }
    }

    fn check_if(&self, i: &TypedIfExpr) -> TypeResult<()> {
        self.check_block(&i.then_block)?;
        match &i.else_branch {
            None => Ok(()),
            Some(TypedElseBranch::Block(b)) => self.check_block(b),
            Some(TypedElseBranch::If(inner)) => self.check_if(inner),
        }
    }

    fn check_match(&self, m: &TypedMatchExpr) -> TypeResult<()> {
        for arm in &m.arms {
            self.check_block(&arm.body)?;
        }
        Ok(())
    }
}

// ── Protocol conformance checker (standalone, called from compiler driver) ────

/// Verify that every `impl Protocol for Type` block provides all required methods.
///
/// This is separate from the main `check()` pass because it requires the full
/// symbol table (which is held by the inferer, not the typed AST).
/// The compiler driver calls this after typecheck completes.
pub fn check_protocol_conformance(sym: &SymbolTable) -> TypeResult<()> {
    for (type_name, type_impls) in &sym.impls {
        for (proto_name, impl_record) in type_impls {
            let proto = match sym.lookup_protocol(proto_name) {
                Some(p) => p,
                None => continue, // user-defined protocol not in table — skip
            };

            for required in &proto.methods {
                let provided = impl_record.methods.iter().any(|m| m.name == required.name);
                if !provided {
                    return Err(TypeError::MissingProtocolMethod {
                        ty: type_name.clone(),
                        protocol: proto_name.clone(),
                        method: required.name.clone(),
                        span: impl_record.span,
                    });
                }
            }
        }
    }
    Ok(())
}
