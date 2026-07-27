/// engine.rs — Monomorphization worklist engine.
///
/// The engine walks the typed AST, emits concrete items, and discovers
/// generic instantiations as it goes. Each instantiation is added to a
/// worklist; the loop continues until the worklist is empty.
///
/// Key invariants:
///   - A specialization key `(name, type_args)` is processed at most once
///     (tracked in `seen`).
///   - Non-generic items are emitted directly.
///   - Generic items are only emitted when instantiated with concrete types.
///   - The output `MonoProgram` is order-stable: non-generic items first,
///     then specializations in discovery order.

use std::collections::{HashMap, HashSet, VecDeque};
use haki_ast::MatchPattern;
use haki_typeck::typed_ast::*;
use crate::mono_ast::*;
use crate::subst::{mangle, Subst};
use crate::{MonoError, MonoResult};

// ── Specialization key ────────────────────────────────────────────────────────

/// Uniquely identifies a monomorphization instance.
/// `("best", [SemTy::Int])` → the `best<int>` specialization.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SpecKey {
    name: String,
    type_args: Vec<String>, // stringified for hashing; SemTy doesn't impl Hash
}

impl SpecKey {
    fn new(name: &str, type_args: &[SemTy]) -> Self {
        Self {
            name: name.to_owned(),
            type_args: type_args.iter().map(|t| t.display()).collect(),
        }
    }
}

// ── Engine ────────────────────────────────────────────────────────────────────

pub struct MonoEngine<'src> {
    file: &'src TypedSourceFile,
    /// Registry of all generic functions, keyed by original name.
    generic_fns: HashMap<String, &'src TypedFnDef>,
    /// Registry of all generic structs, keyed by original name.
    generic_structs: HashMap<String, &'src TypedStructDef>,
    /// Registry of all generic classes, keyed by original name.
    generic_classes: HashMap<String, &'src TypedClassDef>,
    /// Registry of all impl blocks, keyed by target type name.
    #[allow(dead_code)]
    impl_blocks: Vec<&'src TypedImplBlock>,
    /// Specializations already emitted or queued — prevents duplicates.
    seen: HashSet<SpecKey>,
    /// Pending work: generic items waiting to be specialized.
    worklist: VecDeque<(SpecKey, Vec<SemTy>)>,
    /// Output program being assembled.
    program: MonoProgram,
}

impl<'src> MonoEngine<'src> {
    pub fn new(file: &'src TypedSourceFile) -> Self {
        let mut generic_fns = HashMap::new();
        let mut generic_structs = HashMap::new();
        let mut generic_classes = HashMap::new();
        let mut impl_blocks = Vec::new();

        // Index all items.
        for item in &file.items {
            match &item.kind {
                TypedItemKind::Fn(f) if !f.type_params.is_empty() => {
                    generic_fns.insert(f.name.name.clone(), f);
                }
                TypedItemKind::Struct(s) if !s.type_params.is_empty() => {
                    generic_structs.insert(s.name.name.clone(), s);
                }
                TypedItemKind::Class(c) if !c.type_params.is_empty() => {
                    generic_classes.insert(c.name.name.clone(), c);
                }
                TypedItemKind::Impl(i) => {
                    impl_blocks.push(i);
                }
                _ => {}
            }
        }

        Self {
            file,
            generic_fns,
            generic_structs,
            generic_classes,
            impl_blocks,
            seen: HashSet::new(),
            worklist: VecDeque::new(),
            program: MonoProgram::new(),
        }
    }

    // ── Entry point ───────────────────────────────────────────────────────

    pub fn run(mut self) -> MonoResult<MonoProgram> {
        // Pass 1: emit all non-generic items and collect instantiation seeds.
        for item in self.file.items.clone().iter() {
            self.process_top_level_item(item)?;
        }

        // Pass 2: drain the worklist.
        while let Some((key, type_args)) = self.worklist.pop_front() {
            self.specialize(&key.name.clone(), &type_args)?;
        }

        Ok(self.program)
    }

    // ── Top-level item dispatch ───────────────────────────────────────────

    fn process_top_level_item(&mut self, item: &TypedItem) -> MonoResult<()> {
        match &item.kind {
            TypedItemKind::Fn(f) => {
                if f.type_params.is_empty() {
                    // Non-generic: emit immediately.
                    let mono_fn = self.lower_fn(f, &Subst::new())?;
                    self.program.fns.push(mono_fn);
                }
                // Generic functions are only emitted when instantiated.
            }
            TypedItemKind::Struct(s) => {
                if s.type_params.is_empty() {
                    let mono_struct = self.lower_struct(s, &Subst::new())?;
                    self.program.structs.push(mono_struct);
                }
            }
            TypedItemKind::Class(c) => {
                if c.type_params.is_empty() {
                    let mono_class = self.lower_class(c, &Subst::new())?;
                    self.program.classes.push(mono_class);
                }
            }
            TypedItemKind::Impl(i) => {
                // Impl blocks without type params on the target are emitted now.
                if i.type_params.is_empty() {
                    let mono_impl = self.lower_impl(i, &Subst::new())?;
                    self.program.impls.push(mono_impl);
                }
            }
            TypedItemKind::Protocol(_) => {
                // Protocols are erased — they leave no runtime artifact.
            }
            TypedItemKind::Enum(e) => {
                // Enums are erased at the item level — variant construction
                // and match discrimination are handled at call/expression sites.
                // But carry the definition so codegen can look up discriminants.
                self.program.enum_defs.insert(e.name.name.clone(), e.clone());
            }
            TypedItemKind::Import { .. } => {
                // Imports are resolved by the driver — nothing to emit.
            }
            TypedItemKind::ExternFn(f) => {
                // Extern fns are passed through to MonoProgram for the Wasm
                // backend to emit as imports. Other backends ignore them.
                self.program.extern_fns.push(f.clone());
            }
        }
        Ok(())
    }

    // ── Specialization ────────────────────────────────────────────────────

    /// Specialize a generic item (fn, struct, or class) with concrete type args.
    fn specialize(&mut self, name: &str, type_args: &[SemTy]) -> MonoResult<()> {
        // Build the substitution map from type-param positions.
        if let Some(&fn_def) = self.generic_fns.get(name) {
            let subst = build_subst(&fn_def.type_params, type_args, name)?;
            let mut mono_fn = self.lower_fn(fn_def, &subst)?;
            mono_fn.name = mangle(name, type_args);
            self.program.fns.push(mono_fn);
        } else if let Some(&struct_def) = self.generic_structs.get(name) {
            let subst = build_subst(&struct_def.type_params, type_args, name)?;
            let mut mono_struct = self.lower_struct(struct_def, &subst)?;
            mono_struct.name = mangle(name, type_args);
            self.program.structs.push(mono_struct);
        } else if let Some(&class_def) = self.generic_classes.get(name) {
            let subst = build_subst(&class_def.type_params, type_args, name)?;
            let mut mono_class = self.lower_class(class_def, &subst)?;
            mono_class.name = mangle(name, type_args);
            self.program.classes.push(mono_class);
        }
        // If nothing found, the item may have been non-generic to begin with.
        Ok(())
    }

    /// Schedule a generic instantiation if not already seen.
    fn enqueue(&mut self, name: &str, type_args: Vec<SemTy>) {
        let key = SpecKey::new(name, &type_args);
        if self.seen.insert(key.clone()) {
            self.worklist.push_back((key, type_args));
        }
    }

    // ── Lowering: fn ─────────────────────────────────────────────────────

    fn lower_fn(&mut self, f: &TypedFnDef, subst: &Subst) -> MonoResult<MonoFn> {
        let params = f.params.iter().map(|p| {
            MonoParam {
                name: p.name.name.clone(),
                ty: subst.apply_ty(&{
                    // Re-resolve the param type through the substitution.
                    // The typed AST has `SemTy` on expressions; for params
                    // we reconstruct from the field type in the body scope.
                    // Here we use the return type's subst as a proxy — for
                    // params we infer from the function signature.
                    // Since `TypedFnDef` stores `Param` (syntactic), we need
                    // to apply the subst to whatever type the param had.
                    // We store that in the binding type of the first let that
                    // uses it — instead, we store param types in `TypedFnDef`
                    // directly via `return_ty` subst. For this pass we use a
                    // simplified approach: param types come from `f.params`
                    // resolved syntactically, which is already done at typeck.
                    // We placeholder with `Void` and fix via the body walk.
                    SemTy::Void // placeholder — overridden below
                }),
                span: p.span,
            }
        }).collect::<Vec<_>>();

        // Actually resolve param types by walking the scope the typeck built.
        // The cleanest approach: we stored the SemTy in the typed fn's body
        // via the binding information. For the mono pass we need to re-resolve
        // the syntactic `Param.ty` through the substitution.
        // We accomplish this by using the same `resolve_ty` logic inline here.
        let resolved_params: Vec<MonoParam> = f.params.iter().map(|p| {
            // Treat the param's syntactic type as a SemTy name.
            let raw_ty = synth_ty_from_param(p);
            MonoParam {
                name: p.name.name.clone(),
                ty: subst.apply_ty(&raw_ty),
                span: p.span,
            }
        }).collect();
        let _ = params; // discard placeholder

        let return_ty = subst.apply_ty(&f.return_ty);
        let body = self.lower_block(&f.body, subst)?;

        Ok(MonoFn {
            name: f.name.name.clone(),
            params: resolved_params,
            return_ty,
            body,
            span: f.span,
            captures: vec![],
            attributes: f.attributes.clone(),
        })
    }

    // ── Lowering: struct ──────────────────────────────────────────────────

    fn lower_struct(&mut self, s: &TypedStructDef, subst: &Subst) -> MonoResult<MonoStruct> {
        let fields = lower_fields(&s.fields, subst);
        let type_name = s.name.name.clone();
        let self_ty = SemTy::Named(type_name.clone());
        let methods = s.methods.iter()
            .map(|m| {
                let mut mono = self.lower_fn(m, subst)?;
                mono.name = format!("{type_name}__{}", m.name.name);
                // Prepend implicit `self` parameter (pointer to the struct).
                let self_param = MonoParam {
                    name: "self".into(),
                    ty: self_ty.clone(),
                    span: m.span,
                };
                mono.params.insert(0, self_param);
                Ok(mono)
            })
            .collect::<MonoResult<Vec<_>>>()?;
        Ok(MonoStruct {
            name: s.name.name.clone(),
            fields,
            methods,
            span: s.span,
        })
    }

    // ── Lowering: class ───────────────────────────────────────────────────

    fn lower_class(&mut self, c: &TypedClassDef, subst: &Subst) -> MonoResult<MonoClass> {
        let fields = lower_fields(&c.fields, subst);
        let type_name = c.name.name.clone();
        let self_ty = SemTy::Named(type_name.clone());

        // Lower own methods.
        // Method names carry the module alias prefix from rename_item
        // (e.g. "state__set"). Strip it before composing the mono name so
        // we get "state__State__int__set" not "state__State__int__state__set".
        // The alias prefix is everything up to and including the first "__"
        // that matches the class name's own prefix.
        let class_alias_prefix = type_name.find("__")
            .map(|i| &type_name[..=i+1])  // e.g. "state__"
            .unwrap_or("");

        let mut methods: Vec<MonoFn> = c.methods.iter()
            .map(|m| {
                let mut mono = self.lower_fn(m, subst)?;
                // Strip the alias prefix from the method name if present
                let bare_method = if !class_alias_prefix.is_empty()
                    && m.name.name.starts_with(class_alias_prefix) {
                    &m.name.name[class_alias_prefix.len()..]
                } else {
                    &m.name.name
                };
                mono.name = format!("{type_name}__{bare_method}");
                let self_param = MonoParam {
                    name: "self".into(),
                    ty: self_ty.clone(),
                    span: m.span,
                };
                mono.params.insert(0, self_param);
                Ok(mono)
            })
            .collect::<MonoResult<Vec<_>>>()?;

        // Collect own method names so we don't duplicate overridden methods.
        let own_method_names: std::collections::HashSet<String> = c.methods.iter()
            .map(|m| m.name.name.clone())
            .collect();

        // Walk the superclass chain and emit inherited methods under the
        // subclass name so `Dog__speak` resolves to Animal's `speak` body
        // with `self` typed as `Dog`.
        if let Some(ref super_ident) = c.superclass {
            let mut cur = super_ident.name.clone();
            loop {
                let parent_class = self.file.items.iter().find_map(|item| {
                    if let haki_typeck::typed_ast::TypedItemKind::Class(pc) = &item.kind {
                        if pc.name.name == cur { return Some(pc.clone()); }
                    }
                    None
                });

                let (parent_methods, parent_super) = match parent_class {
                    Some(pc) => {
                        let sup = pc.superclass.as_ref().map(|s| s.name.clone());
                        (pc.methods.clone(), sup)
                    }
                    None => break,
                };

                for m in &parent_methods {
                    if own_method_names.contains(&m.name.name) { continue; }
                    if methods.iter().any(|em| em.name == format!("{type_name}__{}", m.name.name)) { continue; }

                    if let Ok(mut mono) = self.lower_fn(m, subst) {
                        let bare_m = if !class_alias_prefix.is_empty()
                            && m.name.name.starts_with(class_alias_prefix) {
                            &m.name.name[class_alias_prefix.len()..]
                        } else { &m.name.name };
                        mono.name = format!("{type_name}__{bare_m}");
                        let self_param = MonoParam {
                            name: "self".into(),
                            ty: self_ty.clone(),
                            span: m.span,
                        };
                        mono.params.insert(0, self_param);
                        methods.push(mono);
                    }
                }

                match parent_super {
                    Some(p) => cur = p,
                    None => break,
                }
            }
        }

        Ok(MonoClass {
            name: c.name.name.clone(),
            superclass: c.superclass.as_ref().map(|s| s.name.clone()),
            fields,
            methods,
            span: c.span,
        })
    }

    // ── Lowering: impl ────────────────────────────────────────────────────

    fn lower_impl(&mut self, i: &TypedImplBlock, subst: &Subst) -> MonoResult<MonoImpl> {
        let target_name = &i.target.name;
        let self_ty = SemTy::Named(target_name.clone());
        let methods = i.methods.iter()
            .map(|m| {
                let mut mono = self.lower_fn(m, subst)?;
                mono.name = format!("{target_name}__{}", m.name.name);
                // Inject self if the method body uses it (i.e. references self
                // or was derived from a protocol default). Always injecting is
                // safe — codegen only uses self if it appears in the body.
                let already_has_self = mono.params.first()
                    .map(|p| p.name == "self").unwrap_or(false);
                if !already_has_self {
                    let self_param = MonoParam {
                        name: "self".into(),
                        ty: self_ty.clone(),
                        span: m.span,
                    };
                    mono.params.insert(0, self_param);
                }
                Ok(mono)
            })
            .collect::<MonoResult<Vec<_>>>()?;
        Ok(MonoImpl {
            protocol: i.protocol.name.clone(),
            target: i.target.name.clone(),
            methods,
            span: i.span,
        })
    }

    // ── Lowering: block ───────────────────────────────────────────────────

    fn lower_block(&mut self, block: &TypedBlock, subst: &Subst) -> MonoResult<MonoBlock> {
        let stmts = block.stmts.iter()
            .map(|s| self.lower_stmt(s, subst))
            .collect::<MonoResult<Vec<_>>>()?;
        Ok(MonoBlock { stmts, span: block.span })
    }

    // ── Lowering: statement ───────────────────────────────────────────────

    fn lower_stmt(&mut self, stmt: &TypedStmt, subst: &Subst) -> MonoResult<MonoStmt> {
        let kind = match &stmt.kind {
            TypedStmtKind::Let(l) => {
                let init = self.lower_expr(&l.init, subst)?;
                let bindings = l.bindings.iter().map(|(b, ty)| {
                    (b.clone(), subst.apply_ty(ty))
                }).collect();
                MonoStmtKind::Let(MonoLetStmt {
                    mutability: l.mutability,
                    bindings,
                    init: Box::new(init),
                    span: l.span,
                })
            }
            TypedStmtKind::Return(r) => {
                let values = r.values.iter()
                    .map(|v| self.lower_expr(v, subst))
                    .collect::<MonoResult<Vec<_>>>()?;
                MonoStmtKind::Return(MonoReturnStmt { values, span: r.span })
            }
            TypedStmtKind::Yield(e) => {
                MonoStmtKind::Yield(Box::new(self.lower_expr(e, subst)?))
            }
            TypedStmtKind::Defer(e) => {
                MonoStmtKind::Defer(Box::new(self.lower_expr(e, subst)?))
            }
            TypedStmtKind::Continue => MonoStmtKind::Continue,
            TypedStmtKind::Break    => MonoStmtKind::Break,
            TypedStmtKind::If(i) => {
                MonoStmtKind::If(self.lower_if(i, subst)?)
            }
            TypedStmtKind::For(f) => {
                let iter = self.lower_expr(&f.iter, subst)?;
                let body = self.lower_block(&f.body, subst)?;
                MonoStmtKind::For(MonoFor {
                    index_var: f.index_var.clone(),
                    var: f.var.clone(),
                    var_ty: subst.apply_ty(&f.var_ty),
                    iter: Box::new(iter),
                    body,
                    span: f.span,
                })
            }
            TypedStmtKind::While(w) => {
                let cond = self.lower_expr(&w.cond, subst)?;
                let body = self.lower_block(&w.body, subst)?;
                MonoStmtKind::While(MonoWhile {
                    cond: Box::new(cond),
                    body,
                    span: w.span,
                })
            }
            TypedStmtKind::Match(m) => {
                MonoStmtKind::Match(self.lower_match(m, subst)?)
            }
            TypedStmtKind::Panic(e) => {
                MonoStmtKind::Panic(Box::new(self.lower_expr(e, subst)?))
            }
            TypedStmtKind::Expr(e) => {
                MonoStmtKind::Expr(Box::new(self.lower_expr(e, subst)?))
            }
        };
        Ok(MonoStmt { kind, span: stmt.span })
    }

    // ── Lowering: if / match ──────────────────────────────────────────────

    fn lower_if(&mut self, i: &TypedIfExpr, subst: &Subst) -> MonoResult<MonoIf> {
        let cond = self.lower_expr(&i.cond, subst)?;
        let then_block = self.lower_block(&i.then_block, subst)?;
        let else_branch = match &i.else_branch {
            None => None,
            Some(TypedElseBranch::Block(b)) => {
                Some(MonoElse::Block(self.lower_block(b, subst)?))
            }
            Some(TypedElseBranch::If(inner)) => {
                Some(MonoElse::If(Box::new(self.lower_if(inner, subst)?)))
            }
        };
        Ok(MonoIf {
            cond: Box::new(cond),
            then_block,
            else_branch,
            ty: subst.apply_ty(&i.ty),
            span: i.span,
        })
    }

    fn lower_match(&mut self, m: &TypedMatchExpr, subst: &Subst) -> MonoResult<MonoMatch> {
        let scrutinee = self.lower_expr(&m.scrutinee, subst)?;

        // Determine match kind from scrutinee type and first non-wildcard arm pattern
        let kind = match &scrutinee.ty {
            ConcrTy::Int => MonoMatchKind::Int,
            ConcrTy::String => MonoMatchKind::String,
            _ => {
                // Check if first non-wildcard arm is a literal (shouldn't happen after typeck)
                let has_literal = m.arms.iter().any(|arm| {
                    matches!(&arm.pattern, MatchPattern::Int(_) | MatchPattern::String(_))
                });
                if has_literal {
                    MonoMatchKind::Int // fallback — typeck should have caught mismatches
                } else {
                    // Enum or class — distinguished by whether variant lookup worked
                    // (typeck already verified, codegen checks via find_variant)
                    MonoMatchKind::Enum
                }
            }
        };

        let arms = m.arms.iter().map(|arm| {
            let body = self.lower_block(&arm.body, subst)?;
            let pattern = match &arm.pattern {
                MatchPattern::Ident(ident) => MonoPattern::Named(ident.name.clone()),
                MatchPattern::Int(n)       => MonoPattern::Int(*n),
                MatchPattern::String(s)    => MonoPattern::String(s.clone()),
            };
            Ok(MonoArm {
                pattern,
                bindings: arm.bindings.clone(),
                binding_tys: arm.binding_tys.iter().map(|t| subst.apply_ty(t)).collect(),
                body,
                span: arm.span,
            })
        }).collect::<MonoResult<Vec<_>>>()?;

        Ok(MonoMatch {
            scrutinee: Box::new(scrutinee),
            arms,
            ty: subst.apply_ty(&m.ty),
            span: m.span,
            kind,
        })
    }

    // ── Lowering: expression ──────────────────────────────────────────────

    fn lower_expr(&mut self, expr: &TypedExpr, subst: &Subst) -> MonoResult<MonoExpr> {
        let ty = subst.apply_ty(&expr.ty);
        let kind = match &expr.kind {
            TypedExprKind::Int(n)    => MonoExprKind::Int(*n),
            TypedExprKind::Float(f)  => MonoExprKind::Float(*f),
            TypedExprKind::String(s) => MonoExprKind::String(s.clone()),
            TypedExprKind::Bool(b)   => MonoExprKind::Bool(*b),
            TypedExprKind::Null      => MonoExprKind::Null,
            TypedExprKind::Ident(id) => {
                // If this is a method reference on self (type SemTy::Fn and name
                // matches a method in the typed program), mangle it so codegen finds
                // TypeName__method in the function table.
                let mangled = if matches!(&expr.ty, SemTy::Fn(_, _)) {
                    // Look for a class that has a method with this name.
                    self.file.items.iter().find_map(|item| {
                        if let haki_typeck::typed_ast::TypedItemKind::Class(c) = &item.kind {
                            if c.methods.iter().any(|m| m.name.name == id.name) {
                                return Some(format!("{}__{}", c.name.name, id.name));
                            }
                        }
                        None
                    })
                } else {
                    None
                };
                MonoExprKind::Var(mangled.unwrap_or_else(|| id.name.clone()))
            }

            TypedExprKind::Unary(op, operand) => {
                MonoExprKind::Unary(*op, Box::new(self.lower_expr(operand, subst)?))
            }

            TypedExprKind::Binary(op, lhs, rhs) => {
                MonoExprKind::Binary(
                    *op,
                    Box::new(self.lower_expr(lhs, subst)?),
                    Box::new(self.lower_expr(rhs, subst)?),
                )
            }

            TypedExprKind::Field(recv, field) => {
                MonoExprKind::Field(
                    Box::new(self.lower_expr(recv, subst)?),
                    field.name.clone(),
                )
            }

            // Method calls become plain function calls in the mono AST,
            // with the receiver passed as the first argument.
            TypedExprKind::MethodCall(recv, method, args) => {
                let mono_recv = self.lower_expr(recv, subst)?;
                let recv_ty = mono_recv.ty.clone();

                // Determine the concrete function name.
                let call_name = method_call_name(&recv_ty, &method.name);

                // For Array<T> and Map<K,V> built-in methods, just pass the
                // args through without trying to lower a non-existent body.
                // The codegen intercepts these calls by name and routes them
                // to the C runtime (haki_array_append etc.).
                let mut mono_args = vec![mono_recv];
                for a in args {
                    mono_args.push(self.lower_expr(a, subst)?);
                }
                MonoExprKind::Call(call_name, mono_args)
            }

            TypedExprKind::Call(callee, args) => {
                let callee_name = match &callee.kind {
                    TypedExprKind::Ident(id) => id.name.clone(),
                    _ => "?".into(),
                };

                // Discover if this call instantiates a generic function.
                // We look at the type args embedded in the expr type.
                let type_args = collect_type_args_from_call(callee, subst);
                let mangled = if type_args.is_empty() {
                    callee_name.clone()
                } else {
                    let name = mangle(&callee_name, &type_args);
                    self.enqueue(&callee_name, type_args);
                    name
                };

                let mono_args = args.iter()
                    .map(|a| self.lower_expr(a, subst))
                    .collect::<MonoResult<Vec<_>>>()?;
                MonoExprKind::Call(mangled, mono_args)
            }

            TypedExprKind::NamedCall(callee, args) => {
                let callee_name = match &callee.kind {
                    TypedExprKind::Ident(id) => id.name.clone(),
                    _ => "?".into(),
                };
                // Check if we need to specialize the callee type.
                let constr_name = subst.apply_ty(&callee.ty);
                let final_name = match &constr_name {
                    SemTy::Generic(base, targs) => {
                        self.enqueue(base, targs.clone());
                        mangle(base, targs)
                    }
                    _ => callee_name,
                };
                let mono_args = args.iter().map(|a| {
                    Ok(MonoNamedArg {
                        name: a.name.name.clone(),
                        value: self.lower_expr(&a.value, subst)?,
                        span: a.span,
                    })
                }).collect::<MonoResult<Vec<_>>>()?;
                MonoExprKind::Construct(final_name, mono_args)
            }

            TypedExprKind::Index(recv, idx) => {
                MonoExprKind::Index(
                    Box::new(self.lower_expr(recv, subst)?),
                    Box::new(self.lower_expr(idx, subst)?),
                )
            }

            TypedExprKind::If(i) => {
                MonoExprKind::If(Box::new(self.lower_if(i, subst)?))
            }

            TypedExprKind::Match(m) => {
                MonoExprKind::Match(Box::new(self.lower_match(m, subst)?))
            }

            TypedExprKind::Block(b) => {
                MonoExprKind::Block(Box::new(self.lower_block(b, subst)?))
            }

            TypedExprKind::Array(elems) => {
                let mono_elems = elems.iter()
                    .map(|e| self.lower_expr(e, subst))
                    .collect::<MonoResult<Vec<_>>>()?;
                // If the array type uses generic type args, enqueue the element type.
                if let SemTy::Generic(ref name, ref targs) = ty {
                    self.enqueue(name, targs.clone());
                }
                MonoExprKind::Array(mono_elems)
            }

            TypedExprKind::Assign(target, value) => {
                MonoExprKind::Assign(
                    Box::new(self.lower_expr(target, subst)?),
                    Box::new(self.lower_expr(value, subst)?),
                )
            }

            TypedExprKind::Async(inner) => {
                MonoExprKind::Async(Box::new(self.lower_expr(inner, subst)?))
            }

            TypedExprKind::FnLiteral(fn_def, typed_captures) => {
                let name = format!("__fn_lit_{}", expr.span.lo);
                let mut mono_fn = self.lower_fn(fn_def, subst)?;
                mono_fn.name = name.clone();

                let mono_captures: Vec<(String, ConcrTy, bool)> = typed_captures.iter()
                    .map(|(id, ty, weak)| (id.name.clone(), subst.apply_ty(ty), *weak))
                    .collect();
                mono_fn.captures = mono_captures;

                // Always prepend __env: ptr — uniform calling convention.
                // Non-capturing fn_lits receive null and ignore it.
                // Capturing closures receive the env struct pointer.
                let env_param = MonoParam {
                    name: "__env".into(),
                    ty: SemTy::Named("__env_ptr".into()),
                    span: fn_def.span,
                };
                mono_fn.params.insert(0, env_param);

                self.program.fns.push(mono_fn);
                MonoExprKind::Var(name)
            }
        };

        Ok(MonoExpr { kind, ty, span: expr.span })
    }
}

// ── Free helpers ──────────────────────────────────────────────────────────────

/// Build a `Subst` from a type-param list and concrete type args.
fn build_subst(
    type_params: &[haki_ast::TypeParam],
    type_args: &[SemTy],
    name: &str,
) -> MonoResult<Subst> {
    if type_params.len() != type_args.len() {
        return Err(MonoError::TypeArgCountMismatch {
            name: name.to_owned(),
            expected: type_params.len(),
            found: type_args.len(),
        });
    }
    Ok(Subst::from_pairs(
        type_params.iter().zip(type_args.iter())
            .map(|(tp, ta)| (tp.name.name.clone(), ta.clone())),
    ))
}

/// Lower the fields of a struct/class through a substitution.
fn lower_fields(fields: &[haki_ast::FieldDef], subst: &Subst) -> Vec<MonoField> {
    fields.iter().map(|f| {
        // Re-resolve the syntactic field type through the substitution.
        let raw = synth_ty_from_ast_ty(&f.ty);
        MonoField {
            name: f.name.name.clone(),
            ty: subst.apply_ty(&raw),
            mutability: f.mutability,
            is_weak: f.is_weak,
            span: f.span,
        }
    }).collect()
}

/// Convert a syntactic `haki_ast::Ty` to a `SemTy` naively (name-only,
/// no symbol-table resolution). Good enough for the mono pass since
/// the typeck pass already validated everything.
fn synth_ty_from_ast_ty(ty: &haki_ast::Ty) -> SemTy {
    use haki_ast::TyKind;
    match &ty.kind {
        TyKind::Named(id) => match id.name.as_str() {
            "int"    => SemTy::Int,
            "f64" | "float" => SemTy::Float,
            "bool"   => SemTy::Bool,
            "string" => SemTy::String,
            "void"   => SemTy::Void,
            n        => SemTy::Named(n.to_owned()),
        },
        TyKind::Generic(id, args) => {
            SemTy::Generic(id.name.clone(), args.iter().map(synth_ty_from_ast_ty).collect())
        }
        TyKind::Optional(inner) => SemTy::Optional(Box::new(synth_ty_from_ast_ty(inner))),
        TyKind::Tuple(tys) => SemTy::Tuple(tys.iter().map(synth_ty_from_ast_ty).collect()),
        TyKind::Fn(params, ret) => {
            let p = params.iter().map(synth_ty_from_ast_ty).collect();
            let r = ret.as_ref().map(|r| synth_ty_from_ast_ty(r)).unwrap_or(SemTy::Void);
            SemTy::Fn(p, Box::new(r))
        }
    }
}

/// Same but for a function parameter's syntactic type.
fn synth_ty_from_param(p: &haki_ast::Param) -> SemTy {
    synth_ty_from_ast_ty(&p.ty)
}

/// Produce a concrete method call name given the receiver type.
/// E.g. `Stack__int.push` → `Stack__int__push` (for LLVM naming).
/// For simple non-generic receivers, just returns `method`.
fn method_call_name(recv_ty: &SemTy, method: &str) -> String {
    match recv_ty {
        SemTy::Named(n)        => format!("{n}__{method}"),
        SemTy::Generic(n, args) => format!("{}__{method}", mangle(n, args)),
        SemTy::String           => format!("string__{method}"),
        SemTy::Int              => format!("int__{method}"),
        SemTy::Float            => format!("float__{method}"),
        SemTy::Bool             => format!("bool__{method}"),
        _                       => method.to_owned(),
    }
}

/// Extract concrete type arguments embedded in the type of a generic call
/// expression. This covers cases like `identity::<int>(x)` where the
/// callee's `ty` field carries the inferred return type.
///
/// For v0.1 we use a simplified heuristic: if the callee's type is a
/// `Named` that matches a known generic function, we look at the call's
/// return type to infer what the type args were.
fn collect_type_args_from_call(callee: &TypedExpr, subst: &Subst) -> Vec<SemTy> {
    // Extract concrete type arguments so the mono engine knows how to specialize
    // a generic function call. The callee carries its type as SemTy::Fn(params, ret).
    // We look at the return type: if it's Generic, its args are the type params.
    match &callee.ty {
        // Direct generic value (rare)
        SemTy::Generic(_, args) => args.iter().map(|a| subst.apply_ty(a)).collect(),
        // Function returning a generic type: e.g. makeState<T> -> State<T>
        // The concrete T is embedded in the return type after typechecking.
        SemTy::Fn(_, ret) => match ret.as_ref() {
            SemTy::Generic(_, args) => args.iter().map(|a| subst.apply_ty(a)).collect(),
            _ => vec![],
        },
        _ => vec![],
    }
}
