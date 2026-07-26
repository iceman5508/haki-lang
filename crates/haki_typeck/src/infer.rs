/// infer.rs — Pass 2: type inference.
///
/// The `Inferer` walks the AST in declaration order, inferring the type of
/// every expression. It maintains an explicit scope stack for local bindings
/// and tracks the expected return type of the current function.
///
/// Key design decisions (from CLAUDE.md):
///   - `null` is assignable to any `T?` type.
///   - `panic(...)` has type `Never` which satisfies any expected type.
///   - Multi-return is modelled as `SemTy::Tuple`.
///   - Binding count must match return value count exactly (or use `_`).
///   - `const` bindings cannot be reassigned.
///   - Generic type parameters are substituted when a call site is resolved.

use std::collections::{HashMap, HashSet};
use haki_ast::*;
use crate::collector::SymbolTable;
use crate::error::{TypeError, TypeResult};
use crate::typed_ast::*;

// ── Scope ─────────────────────────────────────────────────────────────────────

/// A single scope frame — local bindings visible in a block.
#[derive(Debug, Clone)]
struct Scope {
    /// Variable name → (type, mutability).
    vars: HashMap<String, (SemTy, Mut)>,
}

impl Scope {
    fn new() -> Self {
        Self { vars: HashMap::new() }
    }
}

// ── Inferer ───────────────────────────────────────────────────────────────────

pub struct Inferer {
    sym: SymbolTable,
    /// Stack of scope frames; innermost is last.
    scopes: Vec<Scope>,
    /// Expected return type of the function we're currently inside.
    /// `None` at top level.
    current_return_ty: Option<SemTy>,
    /// The type of `self` in the current method, if any.
    self_ty: Option<SemTy>,
}

impl Inferer {
    pub fn new(sym: SymbolTable) -> Self {
        Self {
            sym,
            scopes: vec![Scope::new()], // global scope
            current_return_ty: None,
            self_ty: None,
        }
    }

    // ── Scope management ──────────────────────────────────────────────────

    /// Walk the superclass chain to check if `child` inherits from `ancestor`.
    fn is_subclass_of(&self, child: &str, ancestor: &str) -> bool {
        if child == ancestor { return true; }
        let mut current = child.to_string();
        for _ in 0..32 { // depth limit avoids infinite loops
            let parent = match self.sym.types.get(&current) {
                Some(td) => match &td.superclass {
                    Some(p) => p.clone(),
                    None    => return false,
                },
                None => return false,
            };
            if parent == ancestor { return true; }
            current = parent;
        }
        false
    }

    fn push_scope(&mut self) {        self.scopes.push(Scope::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define(&mut self, name: &str, ty: SemTy, mutability: Mut) {
        self.scopes.last_mut().unwrap().vars.insert(name.to_owned(), (ty, mutability));
    }

    fn lookup_var(&self, name: &str) -> Option<&(SemTy, Mut)> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.vars.get(name) {
                return Some(v);
            }
        }
        None
    }

    // ── Entry point ───────────────────────────────────────────────────────

    pub fn infer_file(&mut self, file: &SourceFile) -> TypeResult<TypedSourceFile> {
        let mut items = Vec::new();
        for item in &file.items {
            items.push(self.infer_item(item)?);
        }
        Ok(TypedSourceFile { items, span: file.span })
    }

    // ── Items ─────────────────────────────────────────────────────────────

    fn infer_item(&mut self, item: &Item) -> TypeResult<TypedItem> {
        let kind = match &item.kind {
            // Import nodes are fully resolved before infer runs.
            // Carry them through so the mono engine can filter them out.
            ItemKind::Import { path, alias, .. } => {
                let resolved_alias = alias.clone().unwrap_or_else(|| {
                    std::path::Path::new(path)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(path)
                        .to_string()
                });
                TypedItemKind::Import {
                    path: path.clone(),
                    alias: resolved_alias,
                }
            }
            ItemKind::Struct(s)   => TypedItemKind::Struct(self.infer_struct(s)?),
            ItemKind::Class(c)    => TypedItemKind::Class(self.infer_class(c)?),
            ItemKind::Enum(e)     => TypedItemKind::Enum(e.clone()),
            ItemKind::Protocol(p) => TypedItemKind::Protocol(p.clone()),
            ItemKind::Impl(i)     => TypedItemKind::Impl(self.infer_impl(i)?),
            ItemKind::Fn(f)       => TypedItemKind::Fn(self.infer_fn_def(f, &HashMap::new())?),
            // ExternFn: no body to infer — pass through as-is.
            // The function is already registered in sym by collect_item.
            ItemKind::ExternFn(f) => TypedItemKind::ExternFn(f.clone()),
        };
        Ok(TypedItem { kind, span: item.span })
    }

    fn infer_struct(&mut self, s: &StructDef) -> TypeResult<TypedStructDef> {
        let self_ty = SemTy::Named(s.name.name.clone());
        let type_args = self.make_type_param_map(&s.type_params);

        let methods = s.methods.iter()
            .map(|m| {
                self.self_ty = Some(self_ty.clone());
                let r = self.infer_fn_def(m, &type_args);
                self.self_ty = None;
                r
            })
            .collect::<TypeResult<Vec<_>>>()?;

        Ok(TypedStructDef {
            name: s.name.clone(),
            type_params: s.type_params.clone(),
            fields: s.fields.clone(),
            methods,
            span: s.span,
        })
    }

    fn infer_class(&mut self, c: &ClassDef) -> TypeResult<TypedClassDef> {
        let self_ty = SemTy::Named(c.name.name.clone());
        let type_args = self.make_type_param_map(&c.type_params);

        let methods = c.methods.iter()
            .map(|m| {
                self.self_ty = Some(self_ty.clone());
                let r = self.infer_fn_def(m, &type_args);
                self.self_ty = None;
                r
            })
            .collect::<TypeResult<Vec<_>>>()?;

        Ok(TypedClassDef {
            name: c.name.clone(),
            type_params: c.type_params.clone(),
            superclass: c.superclass.clone(),
            fields: c.fields.clone(),
            methods,
            span: c.span,
        })
    }

    fn infer_impl(&mut self, i: &ImplBlock) -> TypeResult<TypedImplBlock> {
        let self_ty = SemTy::Named(i.target.name.clone());
        let type_args = self.make_type_param_map(&i.type_params);

        let mut methods = i.methods.iter()
            .map(|m| {
                self.self_ty = Some(self_ty.clone());
                let r = self.infer_fn_def(m, &type_args);
                self.self_ty = None;
                r
            })
            .collect::<TypeResult<Vec<_>>>()?;

        // ── Inject protocol default methods ───────────────────────────────
        // If the protocol has default method bodies and this impl omits them,
        // clone the default AST, substitute `Self` → concrete type, and typecheck.
        let proto_name = &i.protocol.name;
        if let Some(proto_info) = self.sym.protocols.get(proto_name).cloned() {
            let provided: std::collections::HashSet<String> =
                methods.iter().map(|m| m.name.name.clone()).collect();

            for (method_name, default_fn) in &proto_info.default_methods {
                if !provided.contains(method_name) {
                    // Clone and substitute Self → concrete type name.
                    let mut concrete_fn = default_fn.clone();
                    substitute_self(&mut concrete_fn, &i.target.name);

                    self.self_ty = Some(self_ty.clone());
                    let typed = self.infer_fn_def(&concrete_fn, &type_args)?;
                    self.self_ty = None;
                    methods.push(typed);
                }
            }
        }

        Ok(TypedImplBlock {
            protocol: i.protocol.clone(),
            target: i.target.clone(),
            type_params: i.type_params.clone(),
            methods,
            span: i.span,
        })
    }

    // ── Function definitions ──────────────────────────────────────────────

    pub fn infer_fn_def(
        &mut self,
        f: &FnDef,
        outer_type_args: &HashMap<String, SemTy>,
    ) -> TypeResult<TypedFnDef> {
        // Build type-arg map for this function's own type parameters.
        let mut type_args = outer_type_args.clone();
        for tp in &f.type_params {
            // During inference of the definition body we treat each type param
            // as an opaque named type (the monomorphizer will specialise later).
            type_args.insert(tp.name.name.clone(), SemTy::Named(tp.name.name.clone()));
            // Register protocol bounds so method lookups on `T` work.
            self.sym.type_param_bounds.insert(
                tp.name.name.clone(),
                tp.bounds.iter().map(|b| b.name.clone()).collect(),
            );
        }

        let return_ty = self.sym.resolve_return_ty(&f.return_ty, &type_args)?;

        // New scope for function body.
        self.push_scope();
        let prev_return_ty = self.current_return_ty.replace(return_ty.clone());

        // Bind parameters into scope.
        for param in &f.params {
            let ty = self.sym.resolve_ty(&param.ty, &type_args)?;
            self.define(&param.name.name, ty, Mut::Const);
        }

        // Bind `self` and inject all its fields as bare names into scope.
        // In Haki, inside a method `score` refers to `self.score`.
        if let Some(ref st) = self.self_ty.clone() {
            self.define("self", st.clone(), Mut::Const);

            // Inject fields of the enclosing type as local bindings.
            // Walk the full superclass chain so inherited fields are visible.
            if let SemTy::Named(ref type_name) = st {
                let field_bindings: Vec<(String, SemTy, Mut)> = {
                    let mut bindings = vec![];
                    let mut current = type_name.clone();
                    loop {
                        if let Some(typedef) = self.sym.types.get(&current) {
                            for fi in &typedef.fields {
                                let sem_ty = self.sym.resolve_ty(&fi.ty, &type_args)
                                    .unwrap_or(SemTy::Void);
                                bindings.push((fi.name.clone(), sem_ty, fi.mutability));
                            }
                            match &typedef.superclass {
                                Some(parent) => current = parent.clone(),
                                None => break,
                            }
                        } else {
                            break;
                        }
                    }
                    bindings
                };
                for (name, ty, mutability) in field_bindings {
                    self.define(&name, ty, mutability);
                }
            }
        }

        let body = self.infer_block(&f.body, &type_args)?;

        self.current_return_ty = prev_return_ty;
        // Clear the type-param bounds we registered for this function.
        for tp in &f.type_params {
            self.sym.type_param_bounds.remove(&tp.name.name);
        }
        self.pop_scope();

        Ok(TypedFnDef {
            name: f.name.clone(),
            type_params: f.type_params.clone(),
            params: f.params.clone(),
            return_ty,
            body,
            attributes: f.attributes.clone(),
            span: f.span,
        })
    }

    // ── Block ─────────────────────────────────────────────────────────────

    fn infer_block(
        &mut self,
        block: &Block,
        type_args: &HashMap<String, SemTy>,
    ) -> TypeResult<TypedBlock> {
        self.push_scope();
        let mut stmts = Vec::new();
        for stmt in &block.stmts {
            stmts.push(self.infer_stmt(stmt, type_args)?);
        }
        self.pop_scope();
        Ok(TypedBlock { stmts, span: block.span })
    }

    // ── Statements ────────────────────────────────────────────────────────

    fn infer_stmt(
        &mut self,
        stmt: &Stmt,
        type_args: &HashMap<String, SemTy>,
    ) -> TypeResult<TypedStmt> {
        let kind = match &stmt.kind {
            StmtKind::Let(l)    => TypedStmtKind::Let(self.infer_let(l, type_args)?),
            StmtKind::Return(r) => TypedStmtKind::Return(self.infer_return(r, type_args, stmt.span)?),
            StmtKind::Yield(e)  => TypedStmtKind::Yield(Box::new(self.infer_expr(e, type_args)?)),
            StmtKind::Defer(e)  => TypedStmtKind::Defer(Box::new(self.infer_expr(e, type_args)?)),
            StmtKind::Continue  => TypedStmtKind::Continue,
            StmtKind::Break     => TypedStmtKind::Break,
            StmtKind::If(i)     => TypedStmtKind::If(self.infer_if(i, type_args)?),
            StmtKind::While(w)  => TypedStmtKind::While(self.infer_while(w, type_args)?),
            StmtKind::For(f)    => TypedStmtKind::For(self.infer_for(f, type_args)?),
            StmtKind::Match(m)  => TypedStmtKind::Match(self.infer_match(m, type_args)?),
            StmtKind::Panic(e)  => {
                let typed = self.infer_expr(e, type_args)?;
                // panic arg must be a string
                self.expect_ty(&typed.ty, &SemTy::String, typed.span)?;
                TypedStmtKind::Panic(Box::new(typed))
            }
            StmtKind::Expr(e)   => TypedStmtKind::Expr(Box::new(self.infer_expr(e, type_args)?)),
        };
        Ok(TypedStmt { kind, span: stmt.span })
    }

    fn infer_let(
        &mut self,
        l: &LetStmt,
        type_args: &HashMap<String, SemTy>,
    ) -> TypeResult<TypedLetStmt> {
        let init = self.infer_expr(&l.init, type_args)?;
        let init_ty = init.ty.clone();

        // If there's an explicit annotation, check it matches and use it as
        // the binding type. This is important for generic constructors like
        // `Mutex(0)` or `Map()` where the constructor returns a bare `Named`
        // type but the annotation carries the concrete type args.
        // Also handles empty array literals: `[]` infers as `Array<void>` but
        // `let x: Array<int> = []` should bind `x` as `Array<int>`.
        let binding_ty = if let Some(ref ann) = l.ty {
            let ann_ty = self.sym.resolve_ty(ann, type_args)?;
            // Special case: empty array `Array<void>` is assignable to any `Array<T>`
            let is_empty_array = matches!(&init_ty,
                SemTy::Generic(n, args) if n == "Array" && args.len() == 1 && args[0] == SemTy::Void);
            if is_empty_array {
                if let SemTy::Generic(ref n, _) = ann_ty {
                    if n == "Array" {
                        // Use annotation type directly — don't check assignability
                        // for empty literal since Array<void> wouldn't pass.
                        let typed_bindings = self.destructure_bindings(&l.bindings, &ann_ty, l.span)?;
                        for (binding, ty) in &typed_bindings {
                            if let Binding::Name(ident) = binding {
                                self.define(&ident.name, ty.clone(), l.mutability);
                            }
                        }
                        return Ok(TypedLetStmt {
                            mutability: l.mutability,
                            bindings: typed_bindings,
                            init: Box::new(init),
                            span: l.span,
                        });
                    }
                }
            }
            self.expect_assignable(&ann_ty, &init_ty, l.init.span)?;
            ann_ty
        } else {
            init_ty.clone()
        };

        // Destructure: match bindings against the resolved binding type.
        let typed_bindings = self.destructure_bindings(&l.bindings, &binding_ty, l.span)?;

        // Define all named bindings in scope.
        for (binding, ty) in &typed_bindings {
            if let Binding::Name(ident) = binding {
                self.define(&ident.name, ty.clone(), l.mutability);
            }
        }

        Ok(TypedLetStmt {
            mutability: l.mutability,
            bindings: typed_bindings,
            init: Box::new(init),
            span: l.span,
        })
    }

    /// Match binding targets against the init type, returning (Binding, SemTy) pairs.
    fn destructure_bindings(
        &self,
        bindings: &[Binding],
        init_ty: &SemTy,
        span: Span,
    ) -> TypeResult<Vec<(Binding, SemTy)>> {
        match bindings.len() {
            0 => unreachable!("parser never produces 0 bindings"),

            1 => {
                // Single binding — the full init type flows to it.
                // For a Tuple init_ty, this is also fine (the single binding
                // receives the tuple; typeck enforces use at access time).
                Ok(vec![(bindings[0].clone(), init_ty.clone())])
            }

            n => {
                // Multi-binding: init_ty must be a Tuple of exactly n elements.
                match init_ty {
                    SemTy::Tuple(tys) if tys.len() == n => {
                        Ok(bindings.iter().zip(tys.iter())
                            .map(|(b, t)| (b.clone(), t.clone()))
                            .collect())
                    }
                    SemTy::Tuple(tys) => Err(TypeError::BindingCountMismatch {
                        bindings: n,
                        values: tys.len(),
                        span,
                    }),
                    // Single-value init with multiple bindings
                    _ => Err(TypeError::BindingCountMismatch {
                        bindings: n,
                        values: 1,
                        span,
                    }),
                }
            }
        }
    }

    fn infer_return(
        &mut self,
        r: &ReturnStmt,
        type_args: &HashMap<String, SemTy>,
        span: Span,
    ) -> TypeResult<TypedReturnStmt> {
        if self.current_return_ty.is_none() {
            return Err(TypeError::ReturnOutsideFunction { span });
        }

        let typed_values: Vec<TypedExpr> = r.values.iter()
            .map(|v| self.infer_expr(v, type_args))
            .collect::<TypeResult<_>>()?;

        // Check against declared return type.
        let actual_ty = match typed_values.as_slice() {
            [] => SemTy::Void,
            [single] => single.ty.clone(),
            many => SemTy::Tuple(many.iter().map(|e| e.ty.clone()).collect()),
        };

        let expected = self.current_return_ty.clone().unwrap();
        self.expect_assignable(&expected, &actual_ty, span)?;

        Ok(TypedReturnStmt { values: typed_values, span: r.span })
    }

    fn infer_while(
        &mut self,
        w: &WhileStmt,
        type_args: &HashMap<String, SemTy>,
    ) -> TypeResult<TypedWhileStmt> {
        let cond = self.infer_expr(&w.cond, type_args)?;
        self.expect_ty(&cond.ty, &SemTy::Bool, cond.span)?;
        let body = self.infer_block(&w.body, type_args)?;
        Ok(TypedWhileStmt { cond: Box::new(cond), body, span: w.span })
    }

    fn infer_for(
        &mut self,
        f: &ForStmt,
        type_args: &HashMap<String, SemTy>,
    ) -> TypeResult<TypedForStmt> {
        let iter = self.infer_expr(&f.iter, type_args)?;

        // The element type depends on what we're iterating over.
        // `Array<T>` → element type is `T`.
        let elem_ty = match &iter.ty {
            SemTy::Generic(name, args) if name == "Array" && args.len() == 1 => {
                args[0].clone()
            }
            // For unknown or opaque iterables, fall back to Named("_Elem")
            // The monomorphizer will resolve this properly.
            other => other.clone(),
        };

        self.push_scope();
        // Bind index variable as int if present.
        if let Some(idx) = &f.index_var {
            self.define(&idx.name, SemTy::Int, Mut::Const);
        }
        self.define(&f.var.name, elem_ty.clone(), Mut::Const);
        let body = self.infer_block(&f.body, type_args)?;
        self.pop_scope();

        Ok(TypedForStmt {
            index_var: f.index_var.clone(),
            var: f.var.clone(),
            var_ty: elem_ty,
            iter: Box::new(iter),
            body,
            span: f.span,
        })
    }

    // ── If / Match ────────────────────────────────────────────────────────

    fn infer_if(
        &mut self,
        i: &IfExpr,
        type_args: &HashMap<String, SemTy>,
    ) -> TypeResult<TypedIfExpr> {
        let cond = self.infer_expr(&i.cond, type_args)?;
        self.expect_ty(&cond.ty, &SemTy::Bool, cond.span)?;
        let then_block = self.infer_block(&i.then_block, type_args)?;

        // Determine the yield type of the then branch.
        let then_yield = block_yield_ty(&then_block);

        let (else_branch, ty) = match &i.else_branch {
            None => (None, SemTy::Void),
            Some(ElseBranch::Block(b)) => {
                let typed_else = self.infer_block(b, type_args)?;
                let else_yield = block_yield_ty(&typed_else);
                // If both branches yield, types must match.
                let ty = unify_yields(then_yield, else_yield, i.span)?;
                (Some(TypedElseBranch::Block(typed_else)), ty)
            }
            Some(ElseBranch::If(inner)) => {
                let typed_inner = self.infer_if(inner, type_args)?;
                let inner_ty = typed_inner.ty.clone();
                let ty = unify_yields(then_yield, Some(inner_ty), i.span)?;
                (Some(TypedElseBranch::If(Box::new(typed_inner))), ty)
            }
        };

        Ok(TypedIfExpr {
            cond: Box::new(cond),
            then_block,
            else_branch,
            ty,
            span: i.span,
        })
    }

    fn infer_match(
        &mut self,
        m: &MatchExpr,
        type_args: &HashMap<String, SemTy>,
    ) -> TypeResult<TypedMatchExpr> {
        let scrutinee = self.infer_expr(&m.scrutinee, type_args)?;
        let scrutinee_ty = scrutinee.ty.clone();

        let mut arms = Vec::new();
        let mut arm_yield: Option<SemTy> = None;
        let mut has_wildcard = false;

        // Detect primitive match (int or string scrutinee with literal patterns)
        let is_primitive_match = matches!(scrutinee_ty, SemTy::Int | SemTy::String);

        for arm in &m.arms {
            self.push_scope();

            let binding_tys_computed: Vec<SemTy> = match &arm.pattern {
                // ── Wildcard ──────────────────────────────────────────────
                MatchPattern::Ident(ident) if ident.name == "_" => {
                    has_wildcard = true;
                    vec![]
                }

                // ── Integer literal pattern ───────────────────────────────
                MatchPattern::Int(_) => {
                    if !matches!(scrutinee_ty, SemTy::Int) {
                        return Err(TypeError::TypeMismatch {
                            expected: "int".into(),
                            found: format!("{:?}", scrutinee_ty),
                            span: arm.span,
                        });
                    }
                    vec![]
                }

                // ── String literal pattern ────────────────────────────────
                MatchPattern::String(_) => {
                    if !matches!(scrutinee_ty, SemTy::String) {
                        return Err(TypeError::TypeMismatch {
                            expected: "string".into(),
                            found: format!("{:?}", scrutinee_ty),
                            span: arm.span,
                        });
                    }
                    vec![]
                }

                // ── Named pattern (enum variant / class / wildcard ident) ──
                MatchPattern::Ident(ident) => {
                    if is_primitive_match {
                        return Err(TypeError::TypeMismatch {
                            expected: "integer or string literal pattern".into(),
                            found: format!("identifier '{}'", ident.name),
                            span: arm.span,
                        });
                    }
                    match &scrutinee_ty {
                        SemTy::Named(enum_name) => {
                            if let Some(enum_def) = self.sym.enum_defs.get(enum_name).cloned() {
                                let variant_opt = enum_def.variants.iter()
                                    .find(|v| v.name.name == ident.name)
                                    .or_else(|| {
                                        enum_def.variants.iter().find(|v| {
                                            if let Some(bare) = v.name.name.split("__").last() {
                                                bare == ident.name
                                            } else { false }
                                        })
                                    });
                                if let Some(variant) = variant_opt {
                                    let payload_tys: Vec<SemTy> = variant.fields.iter()
                                        .map(|f| self.sym.resolve_ty(f, type_args).unwrap_or(SemTy::Void))
                                        .collect();
                                    for (binding, bty) in arm.bindings.iter().zip(payload_tys.iter()) {
                                        self.define(&binding.name, bty.clone(), Mut::Const);
                                    }
                                    payload_tys
                                } else {
                                    if self.sym.lookup_type(&ident.name).is_none() {
                                        return Err(TypeError::UnknownType {
                                            name: ident.name.clone(),
                                            span: ident.span,
                                        });
                                    }
                                    let pat_ty = SemTy::Named(ident.name.clone());
                                    if let Some(b) = arm.bindings.first() {
                                        self.define(&b.name, pat_ty.clone(), Mut::Const);
                                    }
                                    vec![SemTy::Named(ident.name.clone())]
                                }
                            } else {
                                if self.sym.lookup_type(&ident.name).is_none() {
                                    return Err(TypeError::UnknownType {
                                        name: ident.name.clone(),
                                        span: ident.span,
                                    });
                                }
                                let pat_ty = SemTy::Named(ident.name.clone());
                                if let Some(b) = arm.bindings.first() {
                                    self.define(&b.name, pat_ty.clone(), Mut::Const);
                                }
                                vec![SemTy::Named(ident.name.clone())]
                            }
                        }
                        _ => {
                            if self.sym.lookup_type(&ident.name).is_none() {
                                return Err(TypeError::UnknownType {
                                    name: ident.name.clone(),
                                    span: ident.span,
                                });
                            }
                            let pat_ty = SemTy::Named(ident.name.clone());
                            if let Some(b) = arm.bindings.first() {
                                self.define(&b.name, pat_ty.clone(), Mut::Const);
                            }
                            vec![SemTy::Named(ident.name.clone())]
                        }
                    }
                }
            };

            let body = self.infer_block(&arm.body, type_args)?;
            self.pop_scope();

            let arm_ty = block_yield_ty(&body);
            arm_yield = match (arm_yield, arm_ty) {
                (None, t) => t,
                (Some(prev), Some(curr)) if sem_ty_eq(&prev, &curr) => Some(prev),
                (Some(_), Some(curr)) => Some(curr),
                (Some(prev), None) => Some(prev),
            };

            arms.push(TypedMatchArm {
                pattern: arm.pattern.clone(),
                bindings: arm.bindings.clone(),
                binding_tys: binding_tys_computed,
                body,
                span: arm.span,
            });
        }

        // Primitive matches must have a wildcard arm
        if is_primitive_match && !has_wildcard {
            return Err(TypeError::TypeMismatch {
                expected: "wildcard arm `_` required for integer/string match".into(),
                found: "no wildcard arm".into(),
                span: m.span,
            });
        }

        Ok(TypedMatchExpr {
            scrutinee: Box::new(scrutinee),
            arms,
            ty: arm_yield.unwrap_or(SemTy::Void),
            span: m.span,
        })
    }

    // ── Expressions ───────────────────────────────────────────────────────

    pub fn infer_expr(
        &mut self,
        expr: &Expr,
        type_args: &HashMap<String, SemTy>,
    ) -> TypeResult<TypedExpr> {
        match &expr.kind {
            ExprKind::Int(n)    => Ok(TypedExpr { kind: TypedExprKind::Int(*n),    ty: SemTy::Int,    span: expr.span }),
            ExprKind::Float(f)  => Ok(TypedExpr { kind: TypedExprKind::Float(*f),  ty: SemTy::Float,  span: expr.span }),
            ExprKind::String(s) => Ok(TypedExpr { kind: TypedExprKind::String(s.clone()), ty: SemTy::String, span: expr.span }),
            ExprKind::Bool(b)   => Ok(TypedExpr { kind: TypedExprKind::Bool(*b),   ty: SemTy::Bool,   span: expr.span }),

            // `null` — we can't infer its type alone; use Optional(Void) as a
            // placeholder. Assignment checks will coerce it to the target T?.
            ExprKind::Null => Ok(TypedExpr {
                kind: TypedExprKind::Null,
                ty: SemTy::Optional(Box::new(SemTy::Void)),
                span: expr.span,
            }),

            ExprKind::Ident(ident) => self.infer_ident(ident, expr.span),

            ExprKind::Unary(op, operand) => self.infer_unary(*op, operand, type_args, expr.span),

            ExprKind::Binary(op, lhs, rhs) => self.infer_binary(*op, lhs, rhs, type_args, expr.span),

            ExprKind::Field(recv, field) => self.infer_field(recv, field, type_args, expr.span),

            ExprKind::MethodCall(recv, method, args) =>
                self.infer_method_call(recv, method, args, type_args, expr.span),

            ExprKind::Call(callee, args) =>
                self.infer_call(callee, args, type_args, expr.span),

            ExprKind::NamedCall(callee, args) =>
                self.infer_named_call(callee, args, type_args, expr.span),

            ExprKind::Index(recv, idx) => self.infer_index(recv, idx, type_args, expr.span),

            ExprKind::If(i) => {
                let typed = self.infer_if(i, type_args)?;
                let ty = typed.ty.clone();
                Ok(TypedExpr { kind: TypedExprKind::If(Box::new(typed)), ty, span: expr.span })
            }

            ExprKind::Match(m) => {
                let typed = self.infer_match(m, type_args)?;
                let ty = typed.ty.clone();
                Ok(TypedExpr { kind: TypedExprKind::Match(Box::new(typed)), ty, span: expr.span })
            }

            ExprKind::Block(block) => {
                let typed = self.infer_block(block, type_args)?;
                let ty = block_yield_ty(&typed).unwrap_or(SemTy::Void);
                Ok(TypedExpr { kind: TypedExprKind::Block(Box::new(typed)), ty, span: expr.span })
            }

            ExprKind::Array(elems) => self.infer_array(elems, type_args, expr.span),

            ExprKind::Assign(target, value) =>
                self.infer_assign(target, value, type_args, expr.span),

            ExprKind::Async(inner) => {
                // `async expr` — the inner expression must be a call.
                // The result type is `Task<T>` where `T` is the return type of the call.
                let typed_inner = self.infer_expr(inner, type_args)?;
                let inner_ty = typed_inner.ty.clone();

                // Wrap in Task<T>
                let task_ty = SemTy::Generic("Task".into(), vec![inner_ty]);

                Ok(TypedExpr {
                    kind: TypedExprKind::Async(Box::new(typed_inner)),
                    ty: task_ty,
                    span: expr.span,
                })
            }

            ExprKind::FnLiteral { captures, params, return_ty, body } => {
                let param_sem_tys: Vec<SemTy> = params.iter()
                    .map(|p| self.sym.resolve_ty(&p.ty, type_args).unwrap_or(SemTy::Void))
                    .collect();
                let ret_sem_ty = self.sym.resolve_return_ty(return_ty, type_args)
                    .unwrap_or(SemTy::Void);

                // Resolve captured variable types from the current scope.
                let typed_captures: Vec<(Ident, SemTy, bool)> = captures.iter()
                    .map(|c| {
                        let ty = if let Some((t, _)) = self.lookup_var(&c.name.name) {
                            t.clone()
                        } else if c.name.name == "self" {
                            self.self_ty.clone().unwrap_or(SemTy::Void)
                        } else {
                            SemTy::Void
                        };
                        (c.name.clone(), ty, c.weak)
                    })
                    .collect();

                // Type-check the body in a scope that includes params AND captures.
                self.push_scope();
                for c in &typed_captures {
                    self.define(&c.0.name, c.1.clone(), haki_ast::Mut::Const);
                }
                for p in params {
                    let pty = self.sym.resolve_ty(&p.ty, type_args).unwrap_or(SemTy::Void);
                    self.define(&p.name.name, pty, haki_ast::Mut::Const);
                }
                let prev_return_ty = self.current_return_ty.replace(ret_sem_ty.clone());
                let typed_body = self.infer_block(body, type_args)?;
                self.current_return_ty = prev_return_ty;
                self.pop_scope();

                let fn_def = TypedFnDef {
                    name:        Ident::new("__fn_literal", expr.span),
                    type_params: vec![],
                    params:      params.clone(),
                    return_ty:   ret_sem_ty.clone(),
                    body:        typed_body,
                    attributes:  vec![],   // fn literals have no attribute syntax
                    span:        expr.span,
                };

                let fn_ty = if typed_captures.is_empty() {
                    SemTy::Fn(param_sem_tys, Box::new(ret_sem_ty))
                } else {
                    SemTy::Closure(param_sem_tys, Box::new(ret_sem_ty))
                };
                Ok(TypedExpr {
                    kind: TypedExprKind::FnLiteral(fn_def, typed_captures),
                    ty:   fn_ty,
                    span: expr.span,
                })
            }
        }
    }

    fn infer_ident(&self, ident: &Ident, span: Span) -> TypeResult<TypedExpr> {
        // Unit enum variant: `Point`, `Pending` — zero-payload variant used as a value.
        // Check before locals so variants shadow nothing (they're always uppercase).
        {
            let unit_variant = self.sym.enum_defs.iter().find_map(|(ename, edef)| {
                edef.variants.iter()
                    .find(|v| v.name.name == ident.name && v.fields.is_empty())
                    .map(|_| ename.clone())
            });
            if let Some(enum_name) = unit_variant {
                return Ok(TypedExpr {
                    kind: TypedExprKind::Ident(ident.clone()),
                    ty: SemTy::Named(enum_name),
                    span,
                });
            }
        }

        // Check local scope first.
        if let Some((ty, _)) = self.lookup_var(&ident.name) {
            return Ok(TypedExpr {
                kind: TypedExprKind::Ident(ident.clone()),
                ty: ty.clone(),
                span,
            });
        }
        // Top-level function reference (first-class fn reference).
        // Give it `SemTy::Fn(param_tys, ret_ty)` so it can be stored in
        // variables and called indirectly.
        if let Some(fn_info) = self.sym.lookup_fn(&ident.name).cloned() {
            let param_tys: Vec<SemTy> = fn_info.params.iter()
                .map(|p| self.sym.resolve_ty(&p.ty, &HashMap::new()).unwrap_or(SemTy::Void))
                .collect();
            let ret_ty = self.sym.resolve_return_ty(&fn_info.return_ty, &HashMap::new())
                .unwrap_or(SemTy::Void);
            let fn_ty = SemTy::Fn(param_tys, Box::new(ret_ty));
            return Ok(TypedExpr {
                kind: TypedExprKind::Ident(ident.clone()),
                ty: fn_ty,
                span,
            });
        }
        // Type names used as constructors are resolved at call sites.
        if self.sym.lookup_type(&ident.name).is_some() {
            return Ok(TypedExpr {
                kind: TypedExprKind::Ident(ident.clone()),
                ty: SemTy::Named(ident.name.clone()),
                span,
            });
        }
        // Method reference on `self` — `onTap: increment` inside a class method.
        // Walk the self type's method list and return a SemTy::Fn if found.
        if let Some(ref st) = self.self_ty.clone() {
            if let SemTy::Named(ref type_name) = st {
                let method = {
                    let mut found = None;
                    let mut cur = type_name.clone();
                    'outer: loop {
                        if let Some(td) = self.sym.types.get(&cur) {
                            for m in &td.methods {
                                if m.name == ident.name {
                                    found = Some(m.clone());
                                    break 'outer;
                                }
                            }
                            match &td.superclass {
                                Some(p) => cur = p.clone(),
                                None    => break,
                            }
                        } else { break; }
                    }
                    found
                };
                if let Some(m) = method {
                    let param_tys: Vec<SemTy> = m.params.iter()
                        .map(|p| self.sym.resolve_ty(&p.ty, &HashMap::new()).unwrap_or(SemTy::Void))
                        .collect();
                    let ret_ty = self.sym.resolve_return_ty(&m.return_ty, &HashMap::new())
                        .unwrap_or(SemTy::Void);
                    return Ok(TypedExpr {
                        kind: TypedExprKind::Ident(ident.clone()),
                        ty: SemTy::Fn(param_tys, Box::new(ret_ty)),
                        span,
                    });
                }
            }
        }
        Err(TypeError::UnknownVar { name: ident.name.clone(), span })
    }

    fn infer_unary(
        &mut self,
        op: UnaryOp,
        operand: &Expr,
        type_args: &HashMap<String, SemTy>,
        span: Span,
    ) -> TypeResult<TypedExpr> {
        let typed = self.infer_expr(operand, type_args)?;
        let ty = match op {
            UnaryOp::Neg => {
                match &typed.ty {
                    SemTy::Int   => SemTy::Int,
                    SemTy::Float => SemTy::Float,
                    other => return Err(TypeError::InvalidUnary {
                        op: "-".into(),
                        ty: other.display(),
                        span,
                    }),
                }
            }
            UnaryOp::Not => {
                match &typed.ty {
                    SemTy::Bool => SemTy::Bool,
                    other => return Err(TypeError::InvalidUnary {
                        op: "!".into(),
                        ty: other.display(),
                        span,
                    }),
                }
            }
        };
        Ok(TypedExpr { kind: TypedExprKind::Unary(op, Box::new(typed)), ty, span })
    }

    fn infer_binary(
        &mut self,
        op: BinaryOp,
        lhs: &Expr,
        rhs: &Expr,
        type_args: &HashMap<String, SemTy>,
        span: Span,
    ) -> TypeResult<TypedExpr> {
        let typed_lhs = self.infer_expr(lhs, type_args)?;
        let typed_rhs = self.infer_expr(rhs, type_args)?;

        let lty = &typed_lhs.ty;
        let rty = &typed_rhs.ty;

        let result_ty = match op {
            // Arithmetic: both sides must be numeric, result same type.
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul |
            BinaryOp::Div | BinaryOp::Mod => {
                match (lty, rty) {
                    (SemTy::Int, SemTy::Int)     => SemTy::Int,
                    (SemTy::Float, SemTy::Float) => SemTy::Float,
                    // String concatenation via `+`
                    (SemTy::String, SemTy::String) if op == BinaryOp::Add => SemTy::String,
                    // Allow Never on either side (panic)
                    (SemTy::Never, _) | (_, SemTy::Never) => SemTy::Never,
                    _ => return Err(TypeError::InvalidBinary {
                        op: binop_str(op).into(),
                        lhs: lty.display(),
                        rhs: rty.display(),
                        span,
                    }),
                }
            }
            // Comparisons: both sides same type, result bool.
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                match (lty, rty) {
                    (SemTy::Int, SemTy::Int)     => SemTy::Bool,
                    (SemTy::Float, SemTy::Float) => SemTy::Bool,
                    (SemTy::Never, _) | (_, SemTy::Never) => SemTy::Bool,
                    _ => return Err(TypeError::InvalidBinary {
                        op: binop_str(op).into(),
                        lhs: lty.display(),
                        rhs: rty.display(),
                        span,
                    }),
                }
            }
            // Equality: works on matching types (including optional/null).
            BinaryOp::Eq | BinaryOp::Ne => {
                if types_eq_or_null_compat(lty, rty) {
                    SemTy::Bool
                } else {
                    return Err(TypeError::InvalidBinary {
                        op: binop_str(op).into(),
                        lhs: lty.display(),
                        rhs: rty.display(),
                        span,
                    });
                }
            }
            // Logical: both must be bool.
            BinaryOp::And | BinaryOp::Or => {
                match (lty, rty) {
                    (SemTy::Bool, SemTy::Bool) => SemTy::Bool,
                    (SemTy::Never, _) | (_, SemTy::Never) => SemTy::Bool,
                    _ => return Err(TypeError::InvalidBinary {
                        op: binop_str(op).into(),
                        lhs: lty.display(),
                        rhs: rty.display(),
                        span,
                    }),
                }
            }
        };

        Ok(TypedExpr {
            kind: TypedExprKind::Binary(op, Box::new(typed_lhs), Box::new(typed_rhs)),
            ty: result_ty,
            span,
        })
    }

    fn infer_field(
        &mut self,
        recv: &Expr,
        field: &Ident,
        type_args: &HashMap<String, SemTy>,
        span: Span,
    ) -> TypeResult<TypedExpr> {
        // Module-qualified access: `math.SomeType` or `math.someField`.
        // When the receiver is a bare identifier that is a module alias,
        // resolve the field against the module's exported symbols.
        if let ExprKind::Ident(recv_ident) = &recv.kind {
            if self.sym.is_module(&recv_ident.name) {
                let alias = &recv_ident.name;
                // Is it a type in the module?
                if let Some(td) = self.sym.lookup_module_type(alias, &field.name).cloned() {
                    let typed_recv = TypedExpr {
                        kind: TypedExprKind::Ident(recv_ident.clone()),
                        ty:   SemTy::Named(format!("module:{alias}")),
                        span: recv.span,
                    };
                    return Ok(TypedExpr {
                        kind: TypedExprKind::Field(Box::new(typed_recv), field.clone()),
                        ty:   SemTy::Named(td.name.clone()),
                        span,
                    });
                }
                // Is it a function in the module? Return as SemTy::Fn.
                if let Some(fi) = self.sym.lookup_module_fn(alias, &field.name).cloned() {
                    let param_tys = fi.params.iter()
                        .map(|p| self.sym.resolve_ty(&p.ty, type_args).unwrap_or(SemTy::Void))
                        .collect();
                    let ret = self.sym.resolve_return_ty(&fi.return_ty, type_args)
                        .unwrap_or(SemTy::Void);
                    let typed_recv = TypedExpr {
                        kind: TypedExprKind::Ident(recv_ident.clone()),
                        ty:   SemTy::Named(format!("module:{alias}")),
                        span: recv.span,
                    };
                    return Ok(TypedExpr {
                        kind: TypedExprKind::Field(Box::new(typed_recv), field.clone()),
                        ty:   SemTy::Fn(param_tys, Box::new(ret)),
                        span,
                    });
                }
                return Err(TypeError::NoSuchField {
                    ty: format!("module:{alias}"),
                    field: field.name.clone(),
                    span,
                });
            }
        }

        let typed_recv = self.infer_expr(recv, type_args)?;
        let ty_name = sem_ty_name(&typed_recv.ty);

        // Build type_args with receiver's generic params substituted in.
        // e.g. MutexGuard<int>.value: T → int needs {"T" -> int} in scope.
        let mut field_type_args = type_args.clone();
        if let SemTy::Generic(_, conc_args) = &typed_recv.ty {
            if let Some(type_def) = self.sym.types.get(&ty_name) {
                for (param_name, conc_ty) in type_def.type_params.iter().zip(conc_args.iter()) {
                    field_type_args.insert(param_name.clone(), conc_ty.clone());
                }
            }
        }

        let field_info = self.sym.lookup_field(&ty_name, &field.name)
            .ok_or_else(|| TypeError::NoSuchField {
                ty: ty_name.clone(),
                field: field.name.clone(),
                span,
            })?;

        // Resolve the field's type with generic substitutions applied.
        let field_ty = self.sym.resolve_ty(&field_info.ty.clone(), &field_type_args)?;

        Ok(TypedExpr {
            kind: TypedExprKind::Field(Box::new(typed_recv), field.clone()),
            ty: field_ty,
            span,
        })
    }

    fn infer_method_call(
        &mut self,
        recv: &Expr,
        method: &Ident,
        args: &[Expr],
        type_args: &HashMap<String, SemTy>,
        span: Span,
    ) -> TypeResult<TypedExpr> {
        // Module-qualified call: `math.add(1, 2)`.
        // Receiver is a bare identifier that is a module alias.
        if let ExprKind::Ident(recv_ident) = &recv.kind {
            if self.sym.is_module(&recv_ident.name) {
                let alias = recv_ident.name.clone();
                if let Some(fi) = self.sym.lookup_module_fn(&alias, &method.name).cloned() {
                    // Type-check the arguments.
                    let typed_args = args.iter()
                        .map(|a| self.infer_expr(a, type_args))
                        .collect::<TypeResult<Vec<_>>>()?;
                    let ret = self.sym.resolve_return_ty(&fi.return_ty, type_args)
                        .unwrap_or(SemTy::Void);
                    // Emit as a Call with a mangled name `alias__method`.
                    let mangled = format!("{alias}__{}", method.name);
                    let callee = TypedExpr {
                        kind: TypedExprKind::Ident(Ident::new(&mangled, method.span)),
                        ty:   SemTy::Fn(vec![], Box::new(ret.clone())),
                        span: method.span,
                    };
                    return Ok(TypedExpr {
                        kind: TypedExprKind::Call(Box::new(callee), typed_args),
                        ty:   ret,
                        span,
                    });
                }
                return Err(TypeError::NoSuchMethod {
                    ty: format!("module:{alias}"),
                    method: method.name.clone(),
                    span,
                });
            }
        }

        let typed_recv = self.infer_expr(recv, type_args)?;
        let ty_name = sem_ty_name(&typed_recv.ty);

        let method_info = self.sym.lookup_method(&ty_name, &method.name)
            .cloned()
            .ok_or_else(|| TypeError::NoSuchMethod {
                ty: ty_name.clone(),
                method: method.name.clone(),
                span,
            })?;

        // Build a type_args map that includes the receiver's concrete type
        // parameters. E.g. for Array<int>.append, add {"T" -> int} so that
        // the method signature's `T` resolves correctly.
        let mut method_type_args = type_args.clone();
        match &typed_recv.ty {
            SemTy::Generic(_, conc_args) => {
                // Receiver is already specialized: Array<int>, Map<string,int>
                if let Some(type_def) = self.sym.types.get(&ty_name) {
                    for (param_name, conc_ty) in type_def.type_params.iter().zip(conc_args.iter()) {
                        method_type_args.insert(param_name.clone(), conc_ty.clone());
                    }
                }
            }
            SemTy::Named(_) => {
                // Bare name (e.g. Map from Map() constructor). If the type def
                // has type params, fill them with sensible defaults so method
                // signatures don't fail. The caller's annotation will enforce
                // the real types.
                if let Some(type_def) = self.sym.types.get(&ty_name) {
                    for param_name in &type_def.type_params {
                        // Default: treat unknown type params as Named(param_name).
                        // They'll be treated as opaque pointers in codegen.
                        method_type_args.entry(param_name.clone())
                            .or_insert_with(|| SemTy::Named(param_name.clone()));
                    }
                }
            }
            _ => {}
        }

        let typed_args = self.infer_call_args(&method_info.params, args, &method.name, &method_type_args, span)?;
        let ret_ty = self.sym.resolve_return_ty(&method_info.return_ty, &method_type_args)?;

        Ok(TypedExpr {
            kind: TypedExprKind::MethodCall(Box::new(typed_recv), method.clone(), typed_args),
            ty: ret_ty,
            span,
        })
    }

    fn infer_call(
        &mut self,
        callee: &Expr,
        args: &[Expr],
        type_args: &HashMap<String, SemTy>,
        span: Span,
    ) -> TypeResult<TypedExpr> {
        // Get the callee name for lookup.
        let callee_name = match &callee.kind {
            ExprKind::Ident(id) => id.name.clone(),
            _ => {
                // Complex callee expression — infer as best we can.
                let typed_callee = self.infer_expr(callee, type_args)?;
                let typed_args = args.iter()
                    .map(|a| self.infer_expr(a, type_args))
                    .collect::<TypeResult<Vec<_>>>()?;
                return Ok(TypedExpr {
                    kind: TypedExprKind::Call(Box::new(typed_callee), typed_args),
                    ty: SemTy::Void, // unknown callee type
                    span,
                });
            }
        };

        // Enum variant construction: `Ok(value)`, `Circle(5)`, `Point`.
        // If the callee name is a variant of any known enum, type it as that enum.
        {
            let enum_match = self.sym.enum_defs.iter().find_map(|(ename, edef)| {
                edef.variants.iter().find(|v| v.name.name == callee_name)
                    .map(|v| (ename.clone(), v.clone()))
            });
            if let Some((enum_name, variant)) = enum_match {
                // Typecheck each payload argument.
                let typed_args = args.iter()
                    .map(|a| self.infer_expr(a, type_args))
                    .collect::<TypeResult<Vec<_>>>()?;
                // Build a Call expression that the codegen will intercept.
                let callee_expr = TypedExpr {
                    kind: TypedExprKind::Ident(Ident::new(&callee_name, span)),
                    ty: SemTy::Named(enum_name.clone()),
                    span,
                };
                let _ = variant;
                return Ok(TypedExpr {
                    kind: TypedExprKind::Call(Box::new(callee_expr), typed_args),
                    ty: SemTy::Named(enum_name),
                    span,
                });
            }
        }

        // Unified print(any): accept any single argument type — the codegen
        // dispatches to haki_print, haki_print_int, etc. based on the arg type.
        if callee_name == "print" && args.len() == 1 {            let typed_arg = self.infer_expr(&args[0], type_args)?;
            let callee_expr = TypedExpr {
                kind: TypedExprKind::Ident(Ident::new("print", span)),
                ty: SemTy::Fn(vec![typed_arg.ty.clone()], Box::new(SemTy::Void)),
                span,
            };
            return Ok(TypedExpr {
                kind: TypedExprKind::Call(Box::new(callee_expr), vec![typed_arg]),
                ty: SemTy::Void,
                span,
            });
        }

        // Is it a local variable holding a function or closure value?
        let fn_var_ty = self.lookup_var(&callee_name)
            .and_then(|(ty, _)| {
                if matches!(ty, SemTy::Fn(_, _) | SemTy::Closure(_, _)) {
                    Some(ty.clone())
                } else { None }
            });

        if let Some(ref vty) = fn_var_ty {
            let (param_tys, ret) = match vty {
                SemTy::Fn(p, r) | SemTy::Closure(p, r) => (p.clone(), *r.clone()),
                _ => unreachable!(),
            };

            let mut all_args = Vec::new();
            for (i, arg) in args.iter().enumerate() {
                let _ = param_tys.get(i);
                all_args.push(self.infer_expr(arg, type_args)?);
            }

            let typed_callee = TypedExpr {
                kind: TypedExprKind::Ident(Ident::new(&callee_name, callee.span)),
                ty: vty.clone(),
                span: callee.span,
            };
            return Ok(TypedExpr {
                kind: TypedExprKind::Call(Box::new(typed_callee), all_args),
                ty: ret,
                span,
            });
        }

        // Is it a top-level function?
        if let Some(fn_info) = self.sym.lookup_fn(&callee_name).cloned() {
            let typed_args = self.infer_call_args(&fn_info.params, args, &callee_name, type_args, span)?;
            let ret_ty = self.sym.resolve_return_ty(&fn_info.return_ty, type_args)?;
            let param_tys: Vec<SemTy> = fn_info.params.iter()
                .map(|p| self.sym.resolve_ty(&p.ty, type_args).unwrap_or(SemTy::Void))
                .collect();
            // Callee type must be SemTy::Fn so collect_type_args_from_call
            // returns empty (no generic specialization for concrete functions).
            let typed_callee = TypedExpr {
                kind: TypedExprKind::Ident(Ident::new(&callee_name, callee.span)),
                ty: SemTy::Fn(param_tys, Box::new(ret_ty.clone())),
                span: callee.span,
            };
            return Ok(TypedExpr {
                kind: TypedExprKind::Call(Box::new(typed_callee), typed_args),
                ty: ret_ty,
                span,
            });
        }

        // Is it a type constructor? e.g. `Error(message: "oops")` without named args
        // is actually a positional constructor call.
        if self.sym.lookup_type(&callee_name).is_some() {
            let typed_args = args.iter()
                .map(|a| self.infer_expr(a, type_args))
                .collect::<TypeResult<Vec<_>>>()?;
            let ret_ty = SemTy::Named(callee_name.clone());
            let typed_callee = TypedExpr {
                kind: TypedExprKind::Ident(Ident::new(&callee_name, callee.span)),
                ty: ret_ty.clone(),
                span: callee.span,
            };
            return Ok(TypedExpr {
                kind: TypedExprKind::Call(Box::new(typed_callee), typed_args),
                ty: ret_ty,
                span,
            });
        }

        // Last chance: bare method call inside a method body.
        // `label()` inside `describe()` resolves to `self.label()` when
        // `label` is a method on the current self type.
        if let Some(ref st) = self.self_ty.clone() {
            let method = {
                let ty_name = match st { SemTy::Named(n) => Some(n.clone()), _ => None };
                ty_name.and_then(|n| self.sym.lookup_method(&n, &callee_name).cloned())
            };
            if let Some(method_info) = method {
                // Desugar to a method call on implicit self.
                let self_expr = Expr {
                    kind: ExprKind::Ident(Ident::new("self", span)),
                    span,
                };
                let typed_args = args.iter()
                    .map(|a| self.infer_expr(a, type_args))
                    .collect::<TypeResult<Vec<_>>>()?;
                let self_ty_expr = self.infer_expr(&self_expr, type_args)?;
                let ret = self.sym.resolve_return_ty(&method_info.return_ty, type_args)
                    .unwrap_or(SemTy::Void);
                return Ok(TypedExpr {
                    kind: TypedExprKind::MethodCall(
                        Box::new(self_ty_expr),
                        Ident::new(&callee_name, span),
                        typed_args,
                    ),
                    ty: ret,
                    span,
                });
            }
        }

        Err(TypeError::UnknownFn { name: callee_name, span })
    }

    fn infer_named_call(
        &mut self,
        callee: &Expr,
        args: &[NamedArg],
        type_args: &HashMap<String, SemTy>,
        span: Span,
    ) -> TypeResult<TypedExpr> {
        let callee_name = match &callee.kind {
            ExprKind::Ident(id) => id.name.clone(),
            _ => return Err(TypeError::UnknownFn { name: "complex_callee".into(), span }),
        };

        // Named calls are used for struct/class construction.
        let ret_ty = if self.sym.lookup_type(&callee_name).is_some() {
            SemTy::Named(callee_name.clone())
        } else if let Some(fn_info) = self.sym.lookup_fn(&callee_name).cloned() {
            self.sym.resolve_return_ty(&fn_info.return_ty, type_args)?
        } else {
            return Err(TypeError::UnknownFn { name: callee_name.clone(), span });
        };

        let typed_args = args.iter()
            .map(|a| {
                let typed_val = self.infer_expr(&a.value, type_args)?;
                Ok(TypedNamedArg {
                    name: a.name.clone(),
                    value: typed_val,
                    span: a.span,
                })
            })
            .collect::<TypeResult<Vec<_>>>()?;

        let typed_callee = TypedExpr {
            kind: TypedExprKind::Ident(Ident::new(&callee_name, callee.span)),
            ty: ret_ty.clone(),
            span: callee.span,
        };

        Ok(TypedExpr {
            kind: TypedExprKind::NamedCall(Box::new(typed_callee), typed_args),
            ty: ret_ty,
            span,
        })
    }

    fn infer_index(
        &mut self,
        recv: &Expr,
        idx: &Expr,
        type_args: &HashMap<String, SemTy>,
        span: Span,
    ) -> TypeResult<TypedExpr> {
        let typed_recv = self.infer_expr(recv, type_args)?;
        let typed_idx  = self.infer_expr(idx, type_args)?;

        // Index must be an int.
        self.expect_ty(&typed_idx.ty, &SemTy::Int, typed_idx.span)?;

        // Result type: Array<T>[int] → T
        // Out-of-bounds panics at runtime (like C arrays). No optional needed.
        let elem_ty = match &typed_recv.ty {
            SemTy::Generic(name, args) if name == "Array" && args.len() == 1 => {
                args[0].clone()
            }
            _ => SemTy::Void,
        };

        Ok(TypedExpr {
            kind: TypedExprKind::Index(Box::new(typed_recv), Box::new(typed_idx)),
            ty: elem_ty,
            span,
        })
    }

    fn infer_array(
        &mut self,
        elems: &[Expr],
        type_args: &HashMap<String, SemTy>,
        span: Span,
    ) -> TypeResult<TypedExpr> {
        let typed_elems = elems.iter()
            .map(|e| self.infer_expr(e, type_args))
            .collect::<TypeResult<Vec<_>>>()?;

        // Element type: first element's type (others must match — checked in pass 3).
        let elem_ty = typed_elems.first().map(|e| e.ty.clone()).unwrap_or(SemTy::Void);
        let arr_ty = SemTy::Generic("Array".into(), vec![elem_ty]);

        Ok(TypedExpr {
            kind: TypedExprKind::Array(typed_elems),
            ty: arr_ty,
            span,
        })
    }

    fn infer_assign(
        &mut self,
        target: &Expr,
        value: &Expr,
        type_args: &HashMap<String, SemTy>,
        span: Span,
    ) -> TypeResult<TypedExpr> {
        let typed_value = self.infer_expr(value, type_args)?;

        // Check the target is a `let` binding (not `const`).
        match &target.kind {
            ExprKind::Ident(id) => {
                match self.lookup_var(&id.name) {
                    Some((ty, Mut::Const)) => {
                        let ty = ty.clone();
                        self.expect_assignable(&ty, &typed_value.ty, span)?;
                        return Err(TypeError::AssignToConst { name: id.name.clone(), span });
                    }
                    Some((ty, Mut::Let)) => {
                        let ty = ty.clone();
                        self.expect_assignable(&ty, &typed_value.ty, span)?;
                    }
                    None => {
                        return Err(TypeError::UnknownVar { name: id.name.clone(), span });
                    }
                }
            }
            ExprKind::Field(recv, field_id) => {
                let typed_recv = self.infer_expr(recv, type_args)?;
                let ty_name = sem_ty_name(&typed_recv.ty);
                if let Some(fi) = self.sym.lookup_field(&ty_name, &field_id.name) {
                    if fi.mutability == Mut::Const {
                        return Err(TypeError::AssignToConstField {
                            field: field_id.name.clone(),
                            span,
                        });
                    }
                }
            }
            _ => {} // other lvalue forms — pass through for now
        }

        let typed_target = self.infer_expr(target, type_args)?;

        Ok(TypedExpr {
            kind: TypedExprKind::Assign(Box::new(typed_target), Box::new(typed_value)),
            ty: SemTy::Void, // assignment produces no value
            span,
        })
    }

    // ── Call argument type-checking ───────────────────────────────────────

    fn infer_call_args(
        &mut self,
        params: &[Param],
        args: &[Expr],
        fn_name: &str,
        type_args: &HashMap<String, SemTy>,
        span: Span,
    ) -> TypeResult<Vec<TypedExpr>> {
        if params.len() != args.len() {
            return Err(TypeError::WrongArgCount {
                name: fn_name.to_string(),
                expected: params.len(),
                found: args.len(),
                span,
            });
        }

        params.iter().zip(args.iter())
            .map(|(param, arg)| {
                let typed_arg = self.infer_expr(arg, type_args)?;
                let param_ty = self.sym.resolve_ty(&param.ty, type_args)?;
                // `Self` in protocol signatures is resolved by the monomorphizer.
                // At the type-check level we accept any type in that position.
                if param_ty == SemTy::Named("Self".into()) {
                    return Ok(typed_arg);
                }
                self.expect_assignable(&param_ty, &typed_arg.ty, arg.span)?;
                Ok(typed_arg)
            })
            .collect()
    }

    // ── Type assertion helpers ────────────────────────────────────────────

    fn expect_ty(&self, found: &SemTy, expected: &SemTy, span: Span) -> TypeResult<()> {
        if sem_ty_eq(found, expected) || *found == SemTy::Never {
            Ok(())
        } else {
            Err(TypeError::TypeMismatch {
                expected: expected.display(),
                found: found.display(),
                span,
            })
        }
    }

    fn expect_assignable(&self, expected: &SemTy, found: &SemTy, span: Span) -> TypeResult<()> {
        if expected.is_assignable_from(found) || *found == SemTy::Never {
            return Ok(());
        }
        // Unresolved type parameters (single uppercase names like T, K, V)
        // act as wildcards — they accept any concrete type.
        if let SemTy::Named(name) = expected {
            let is_type_param = name.len() == 1 && name.chars().next().map_or(false, |c| c.is_uppercase());
            if is_type_param {
                return Ok(());
            }
            // Subclass assignability: `Dog` is assignable to `Animal` if Dog
            // has Animal in its superclass chain. Also covers widget → View.
            if let SemTy::Named(found_name) = found {
                if self.is_subclass_of(found_name, name) {
                    return Ok(());
                }
            }
        }
        // Special case: null → T?
        if found.is_optional() && matches!(found, SemTy::Optional(inner) if **inner == SemTy::Void) {
            if expected.is_optional() {
                return Ok(());
            }
            return Err(TypeError::NullToNonOptional {
                ty: expected.display(),
                span,
            });
        }
        Err(TypeError::TypeMismatch {
            expected: expected.display(),
            found: found.display(),
            span,
        })
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    fn make_type_param_map(&self, params: &[TypeParam]) -> HashMap<String, SemTy> {
        params.iter()
            .map(|tp| (tp.name.name.clone(), SemTy::Named(tp.name.name.clone())))
            .collect()
    }
}

// ── Free functions ────────────────────────────────────────────────────────────

/// Extract the type produced by a `yield` in a block, if present.
fn block_yield_ty(block: &TypedBlock) -> Option<SemTy> {
    for stmt in &block.stmts {
        if let TypedStmtKind::Yield(e) = &stmt.kind {
            return Some(e.ty.clone());
        }
        if let TypedStmtKind::If(i) = &stmt.kind {
            if i.ty != SemTy::Void {
                return Some(i.ty.clone());
            }
        }
    }
    None
}

/// Unify the yield types of the then/else branches.
fn unify_yields(
    then: Option<SemTy>,
    else_: Option<SemTy>,
    span: Span,
) -> TypeResult<SemTy> {
    match (then, else_) {
        (None, None)   => Ok(SemTy::Void),
        (Some(t), None) => Ok(t),
        (None, Some(e)) => Ok(e),
        (Some(t), Some(e)) => {
            if sem_ty_eq(&t, &e) {
                Ok(t)
            } else if t == SemTy::Never {
                Ok(e)
            } else if e == SemTy::Never {
                Ok(t)
            } else {
                Err(TypeError::IfBranchTypeMismatch {
                    then_ty: t.display(),
                    else_ty: e.display(),
                    span,
                })
            }
        }
    }
}

/// Get the base type name of a SemTy for field/method lookup.
fn sem_ty_name(ty: &SemTy) -> String {
    match ty {
        SemTy::Named(n)      => n.clone(),
        SemTy::Generic(n, _) => n.clone(),
        SemTy::Optional(inner) => sem_ty_name(inner),
        SemTy::String        => "string".into(),
        _                    => String::new(),
    }
}

/// Structural equality for SemTy (ignores spans).
fn sem_ty_eq(a: &SemTy, b: &SemTy) -> bool {
    a == b
}

/// True if `a` and `b` are compatible for equality comparison
/// (including null/optional comparisons like `err != null`).
fn types_eq_or_null_compat(a: &SemTy, b: &SemTy) -> bool {
    if sem_ty_eq(a, b) { return true; }
    // T? == null
    if a.is_optional() && matches!(b, SemTy::Optional(inner) if **inner == SemTy::Void) {
        return true;
    }
    if b.is_optional() && matches!(a, SemTy::Optional(inner) if **inner == SemTy::Void) {
        return true;
    }
    // Never
    if *a == SemTy::Never || *b == SemTy::Never { return true; }
    false
}

fn binop_str(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+", BinaryOp::Sub => "-", BinaryOp::Mul => "*",
        BinaryOp::Div => "/", BinaryOp::Mod => "%",
        BinaryOp::Eq  => "==", BinaryOp::Ne => "!=",
        BinaryOp::Lt  => "<",  BinaryOp::Le => "<=",
        BinaryOp::Gt  => ">",  BinaryOp::Ge => ">=",
        BinaryOp::And => "&&", BinaryOp::Or => "||",
    }
}

// ── Protocol default method helpers ──────────────────────────────────────────

/// Walk a cloned `FnDef` (a protocol default method) and replace every
/// occurrence of `Self` with the concrete type name.
///
/// Covers:
///   - Parameter type annotations: `fn f(other: Self)`
///   - Return type:                `fn f() -> Self`
///   - Body expressions:           `Self(x: 1)` constructor calls
pub fn substitute_self(f: &mut FnDef, concrete: &str) {
    // Return type
    if let Some(ret) = &mut f.return_ty {
        subst_return_ty(ret, concrete);
    }
    // Parameter types
    for p in &mut f.params {
        subst_ty(&mut p.ty, concrete);
    }
    // Body
    subst_block(&mut f.body, concrete);
}

fn subst_return_ty(ret: &mut ReturnTy, concrete: &str) {
    match ret {
        ReturnTy::Single(ty) => subst_ty(ty, concrete),
        ReturnTy::Tuple(tys) => {
            for ty in tys { subst_ty(ty, concrete); }
        }
    }
}

fn subst_ty(ty: &mut Ty, concrete: &str) {
    match &mut ty.kind {
        TyKind::Named(id) => {
            if id.name == "Self" {
                id.name = concrete.to_string();
            }
        }
        TyKind::Generic(id, args) => {
            if id.name == "Self" { id.name = concrete.to_string(); }
            for a in args { subst_ty(a, concrete); }
        }
        TyKind::Optional(inner) => subst_ty(inner, concrete),
        TyKind::Fn(params, ret) => {
            for p in params { subst_ty(p, concrete); }
            if let Some(r) = ret { subst_ty(r, concrete); }
        }
        TyKind::Tuple(tys) => {
            for t in tys { subst_ty(t, concrete); }
        }
    }
}

fn subst_block(block: &mut Block, concrete: &str) {
    for stmt in &mut block.stmts {
        subst_stmt(stmt, concrete);
    }
}

fn subst_stmt(stmt: &mut Stmt, concrete: &str) {
    match &mut stmt.kind {
        StmtKind::Let(l)    => subst_expr(&mut l.init, concrete),
        StmtKind::Return(r) => { for e in &mut r.values { subst_expr(e, concrete); } }
        StmtKind::Yield(e)  => subst_expr(e, concrete),
        StmtKind::Defer(e)  => subst_expr(e, concrete),
        StmtKind::Continue | StmtKind::Break => {}
        StmtKind::Expr(e)   => subst_expr(e, concrete),
        StmtKind::Panic(e)  => subst_expr(e, concrete),
        StmtKind::If(i) => {
            subst_expr(&mut i.cond, concrete);
            subst_block(&mut i.then_block, concrete);
            if let Some(els) = &mut i.else_branch {
                match els {
                    ElseBranch::Block(b)  => subst_block(b, concrete),
                    ElseBranch::If(inner) => {
                        subst_expr(&mut inner.cond, concrete);
                        subst_block(&mut inner.then_block, concrete);
                    }
                }
            }
        }
        StmtKind::For(f)   => { subst_expr(&mut f.iter, concrete); subst_block(&mut f.body, concrete); }
        StmtKind::While(w) => { subst_expr(&mut w.cond, concrete); subst_block(&mut w.body, concrete); }
        StmtKind::Match(m) => {
            subst_expr(&mut m.scrutinee, concrete);
            for arm in &mut m.arms { subst_block(&mut arm.body, concrete); }
        }
    }
}

fn subst_expr(expr: &mut Expr, concrete: &str) {
    match &mut expr.kind {
        // The key case: `Self(...)` constructor or `Self` type reference
        ExprKind::Ident(id) => {
            if id.name == "Self" { id.name = concrete.to_string(); }
        }
        ExprKind::Call(callee, args) => {
            subst_expr(callee, concrete);
            for a in args { subst_expr(a, concrete); }
        }
        ExprKind::NamedCall(callee, args) => {
            subst_expr(callee, concrete);
            for a in args { subst_expr(&mut a.value, concrete); }
        }
        ExprKind::MethodCall(recv, _, args) => {
            subst_expr(recv, concrete);
            for a in args { subst_expr(a, concrete); }
        }
        ExprKind::Field(recv, _)     => subst_expr(recv, concrete),
        ExprKind::Binary(_, l, r)    => { subst_expr(l, concrete); subst_expr(r, concrete); }
        ExprKind::Unary(_, e)        => subst_expr(e, concrete),
        ExprKind::Assign(t, v)       => { subst_expr(t, concrete); subst_expr(v, concrete); }
        ExprKind::If(i) => {
            subst_expr(&mut i.cond, concrete);
            subst_block(&mut i.then_block, concrete);
            if let Some(els) = &mut i.else_branch {
                match els {
                    ElseBranch::Block(b)  => subst_block(b, concrete),
                    ElseBranch::If(inner) => {
                        subst_expr(&mut inner.cond, concrete);
                        subst_block(&mut inner.then_block, concrete);
                    }
                }
            }
        }
        ExprKind::Block(b)   => subst_block(b, concrete),
        ExprKind::Match(m)   => {
            subst_expr(&mut m.scrutinee, concrete);
            for arm in &mut m.arms { subst_block(&mut arm.body, concrete); }
        }
        ExprKind::Array(es)  => { for e in es { subst_expr(e, concrete); } }
        ExprKind::Index(a,i) => { subst_expr(a, concrete); subst_expr(i, concrete); }
        ExprKind::Async(e)   => subst_expr(e, concrete),
        ExprKind::FnLiteral { body, .. } => subst_block(body, concrete),
        ExprKind::Int(_) | ExprKind::Float(_) | ExprKind::String(_)
        | ExprKind::Bool(_) | ExprKind::Null => {}
    }
}
