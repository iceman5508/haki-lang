/// emitter.rs — Wasm binary emitter for Haki v0.1.
///
/// Produces a valid Wasm module binary using wasm-encoder.
/// The host environment (browser/Node) must provide imports:
///
///   (import "env" "print"       (func (param i32)))
///   (import "env" "print_int"   (func (param i64)))
///   (import "env" "print_float" (func (param f64)))
///   (import "env" "print_bool"  (func (param i32)))

use std::collections::HashMap;
use wasm_encoder::{
    CodeSection, EntityType, ExportKind, ExportSection,
    Function, FunctionSection, ImportSection, Instruction,
    Module, TypeSection, ValType,
};
use haki_ast::{BinaryOp, Mut, UnaryOp};
use haki_typeck::typed_ast::SemTy;
use haki_mono::mono_ast::*;
use crate::error::{WasmError, WasmResult};
use crate::types::{sem_to_result, sem_to_val};

// ── Function index tracking ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct FnEntry {
    index:     u32,
    is_import: bool,
}

// ── Local variable slot ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct LocalSlot {
    index: u32,
    ty:    SemTy,
}

// ── WasmEmitter ───────────────────────────────────────────────────────────────

pub struct WasmEmitter {
    _module_name: String,
    types:        TypeSection,
    imports:      ImportSection,
    functions:    FunctionSection,
    exports:      ExportSection,
    codes:        CodeSection,
    fn_map:       HashMap<String, FnEntry>,
    type_cache:   Vec<(Vec<ValType>, Vec<ValType>)>, // (params, results) → type_idx
    next_fn_idx:  u32,
}

impl WasmEmitter {
    pub fn new(module_name: &str) -> Self {
        Self {
            _module_name: module_name.to_string(),
            types:        TypeSection::new(),
            imports:      ImportSection::new(),
            functions:    FunctionSection::new(),
            exports:      ExportSection::new(),
            codes:        CodeSection::new(),
            fn_map:       HashMap::new(),
            type_cache:   Vec::new(),
            next_fn_idx:  0,
        }
    }

    // ── Type section helpers ──────────────────────────────────────────────

    fn intern_type(&mut self, params: Vec<ValType>, results: Vec<ValType>) -> u32 {
        if let Some(idx) = self.type_cache.iter().position(|(p, r)| p == &params && r == &results) {
            return idx as u32;
        }
        let idx = self.type_cache.len() as u32;
        self.types.function(params.clone(), results.clone());
        self.type_cache.push((params, results));
        idx
    }

    // ── Import section ────────────────────────────────────────────────────

    fn declare_imports(&mut self) {
        // Stdlib imports the host must provide.
        let imports: &[(&str, &str, &[ValType], &[ValType])] = &[
            ("env", "print",        &[ValType::I32], &[]),
            ("env", "print_int",    &[ValType::I64], &[]),
            ("env", "print_float",  &[ValType::F64], &[]),
            ("env", "print_bool",   &[ValType::I32], &[]),
            ("env", "string_concat",&[ValType::I32, ValType::I32], &[ValType::I32]),
            ("env", "string_length",&[ValType::I32], &[ValType::I32]),
        ];

        for (module, name, params, results) in imports {
            let type_idx = self.intern_type(params.to_vec(), results.to_vec());
            self.imports.import(module, *name, EntityType::Function(type_idx));
            self.fn_map.insert(name.to_string(), FnEntry {
                index: self.next_fn_idx,
                is_import: true,
            });
            self.next_fn_idx += 1;
        }
    }

    /// Declare `extern "js"` functions as Wasm imports from module "env".
    /// Each `extern "js" fn name(params) -> RetTy` in Haki becomes:
    ///   (import "env" "name" (func (param ...) (result ...)))
    fn declare_extern_fns(&mut self, extern_fns: &[haki_ast::ExternFnDef]) {
        use crate::types::{ast_ty_to_val, ast_return_to_val};
        for f in extern_fns {
            // Skip if already registered (e.g. duplicate extern declarations)
            if self.fn_map.contains_key(&f.name.name) { continue; }

            let params: Vec<ValType> = f.params.iter()
                .map(|p| ast_ty_to_val(&p.ty))
                .collect();
            let results: Vec<ValType> = ast_return_to_val(&f.return_ty)
                .map(|v| vec![v])
                .unwrap_or_default();

            let type_idx = self.intern_type(params, results);
            // Always import from "env" regardless of ABI string —
            // "env" is the universal Wasm import convention for host functions.
            self.imports.import("env", f.name.name.as_str(), EntityType::Function(type_idx));
            self.fn_map.insert(f.name.name.clone(), FnEntry {
                index: self.next_fn_idx,
                is_import: true,
            });
            self.next_fn_idx += 1;
        }
    }

    // ── Declaration pass ──────────────────────────────────────────────────

    fn declare_fn(&mut self, f: &MonoFn) -> WasmResult<()> {
        if self.fn_map.contains_key(&f.name) { return Ok(()); }

        let params: Vec<ValType> = f.params.iter()
            .filter_map(|p| sem_to_val(&p.ty).ok())
            .collect();
        let results: Vec<ValType> = sem_to_result(&f.return_ty)
            .map(|v| vec![v])
            .unwrap_or_default();

        let type_idx = self.intern_type(params, results);
        self.functions.function(type_idx);
        self.fn_map.insert(f.name.clone(), FnEntry {
            index: self.next_fn_idx,
            is_import: false,
        });
        self.next_fn_idx += 1;
        Ok(())
    }

    // ── Emit ──────────────────────────────────────────────────────────────

    pub fn emit(&mut self, program: &MonoProgram) -> WasmResult<()> {
        self.declare_imports();
        // Declare extern "js" functions as Wasm imports BEFORE regular functions.
        // Import indices must come before function indices in Wasm.
        self.declare_extern_fns(&program.extern_fns);

        // Declare all functions first.
        let all_fns: Vec<MonoFn> = {
            let mut v = program.fns.clone();
            for s in &program.structs { v.extend(s.methods.clone()); }
            for c in &program.classes { v.extend(c.methods.clone()); }
            for i in &program.impls   { v.extend(i.methods.clone()); }
            v
        };
        for f in &all_fns { self.declare_fn(f)?; }

        // Emit function bodies.
        for f in &all_fns {
            self.emit_fn(f, program)?;
        }

        // Export `main` if present.
        if let Some(entry) = self.fn_map.get("main") {
            self.exports.export("main", ExportKind::Func, entry.index);
        }

        Ok(())
    }

    fn emit_fn(&mut self, f: &MonoFn, program: &MonoProgram) -> WasmResult<()> {
        // Build a local variable map: param slots first, then let-bindings.
        let mut locals: HashMap<String, LocalSlot> = HashMap::new();
        let mut local_decls: Vec<(u32, ValType)> = Vec::new();
        let mut next_local: u32 = 0;

        for param in &f.params {
            if let Ok(vt) = sem_to_val(&param.ty) {
                locals.insert(param.name.clone(), LocalSlot {
                    index: next_local,
                    ty: param.ty.clone(),
                });
                local_decls.push((1, vt));
                next_local += 1;
            }
        }

        // Pre-allocate locals for all let-bindings in the body.
        self.collect_locals(&f.body, &mut locals, &mut local_decls, &mut next_local);

        let mut func = Function::new(local_decls);
        self.emit_block_instrs(&f.body, &locals, &mut func, program, &f.return_ty)?;

        // Ensure there's always an end.
        func.instruction(&Instruction::End);
        self.codes.function(&func);
        Ok(())
    }

    /// Walk a block and pre-allocate Wasm locals for every let-binding.
    fn collect_locals(
        &self,
        block: &MonoBlock,
        locals: &mut HashMap<String, LocalSlot>,
        decls: &mut Vec<(u32, ValType)>,
        next: &mut u32,
    ) {
        for stmt in &block.stmts {
            match &stmt.kind {
                MonoStmtKind::Let(l) => {
                    for (binding, ty) in &l.bindings {
                        if let haki_ast::Binding::Name(id) = binding {
                            if let Ok(vt) = sem_to_val(ty) {
                                if !locals.contains_key(&id.name) {
                                    locals.insert(id.name.clone(), LocalSlot {
                                        index: *next,
                                        ty: ty.clone(),
                                    });
                                    decls.push((1, vt));
                                    *next += 1;
                                }
                            }
                        }
                    }
                }
                MonoStmtKind::If(i) => {
                    self.collect_locals(&i.then_block, locals, decls, next);
                    if let Some(else_br) = &i.else_branch {
                        if let MonoElse::Block(b) = else_br {
                            self.collect_locals(b, locals, decls, next);
                        }
                    }
                }
                MonoStmtKind::While(w) => self.collect_locals(&w.body, locals, decls, next),
                MonoStmtKind::For(f)   => self.collect_locals(&f.body, locals, decls, next),
                _ => {}
            }
        }
    }

    // ── Block instructions ────────────────────────────────────────────────

    fn emit_block_instrs(
        &self,
        block: &MonoBlock,
        locals: &HashMap<String, LocalSlot>,
        func: &mut Function,
        program: &MonoProgram,
        ret_ty: &SemTy,
    ) -> WasmResult<()> {
        for stmt in &block.stmts {
            self.emit_stmt(stmt, locals, func, program, ret_ty)?;
        }
        Ok(())
    }

    // ── Statements ────────────────────────────────────────────────────────

    fn emit_stmt(
        &self,
        stmt: &MonoStmt,
        locals: &HashMap<String, LocalSlot>,
        func: &mut Function,
        program: &MonoProgram,
        ret_ty: &SemTy,
    ) -> WasmResult<()> {
        match &stmt.kind {
            MonoStmtKind::Let(l) => self.emit_let(l, locals, func, program),
            MonoStmtKind::Return(r) => self.emit_return(r, locals, func, program),
            MonoStmtKind::Expr(e) => {
                self.emit_expr(e, locals, func, program)?;
                // Drop return value if non-void expression used as statement.
                if sem_to_result(&e.ty).is_some() {
                    func.instruction(&Instruction::Drop);
                }
                Ok(())
            }
            MonoStmtKind::If(i) => self.emit_if_stmt(i, locals, func, program, ret_ty),
            MonoStmtKind::While(w) => self.emit_while(w, locals, func, program, ret_ty),
            MonoStmtKind::Select(_) => Ok(()),  // select is C-backend only
            MonoStmtKind::Panic(msg) => {
                // Call env.print with a simple message, then unreachable.
                self.emit_expr(msg, locals, func, program)?;
                if let Some(entry) = self.fn_map.get("print") {
                    func.instruction(&Instruction::Call(entry.index));
                }
                func.instruction(&Instruction::Unreachable);
                Ok(())
            }
            MonoStmtKind::Yield(_) | MonoStmtKind::Defer(_) | MonoStmtKind::Continue | MonoStmtKind::Break | MonoStmtKind::For(_) | MonoStmtKind::Match(_) => {
                // For/match/yield are complex — emit a nop for now.
                func.instruction(&Instruction::Nop);
                Ok(())
            }
        }
    }

    fn emit_let(
        &self,
        l: &MonoLetStmt,
        locals: &HashMap<String, LocalSlot>,
        func: &mut Function,
        program: &MonoProgram,
    ) -> WasmResult<()> {
        let init_val = self.emit_expr(&l.init, locals, func, program)?;
        match l.bindings.as_slice() {
            [(binding, _ty)] => {
                if !init_val { return Ok(()); }
                if let haki_ast::Binding::Name(id) = binding {
                    if let Some(slot) = locals.get(&id.name) {
                        func.instruction(&Instruction::LocalSet(slot.index));
                    }
                } else {
                    func.instruction(&Instruction::Drop);
                }
            }
            _ => {
                // Multi-binding: simplified — drop for now.
                if init_val { func.instruction(&Instruction::Drop); }
            }
        }
        Ok(())
    }

    fn emit_return(
        &self,
        r: &MonoReturnStmt,
        locals: &HashMap<String, LocalSlot>,
        func: &mut Function,
        program: &MonoProgram,
    ) -> WasmResult<()> {
        match r.values.as_slice() {
            [] => {}
            [single] => { self.emit_expr(single, locals, func, program)?; }
            multi => { self.emit_expr(&multi[0], locals, func, program)?; }
        }
        func.instruction(&Instruction::Return);
        Ok(())
    }

    fn emit_if_stmt(
        &self,
        i: &MonoIf,
        locals: &HashMap<String, LocalSlot>,
        func: &mut Function,
        program: &MonoProgram,
        ret_ty: &SemTy,
    ) -> WasmResult<()> {
        self.emit_expr(&i.cond, locals, func, program)?;
        // Wasm if block type — void for statements, value type for expressions.
        func.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
        self.emit_block_instrs(&i.then_block, locals, func, program, ret_ty)?;
        if let Some(else_br) = &i.else_branch {
            func.instruction(&Instruction::Else);
            match else_br {
                MonoElse::Block(b) => self.emit_block_instrs(b, locals, func, program, ret_ty)?,
                MonoElse::If(inner) => self.emit_if_stmt(inner, locals, func, program, ret_ty)?,
            }
        }
        func.instruction(&Instruction::End);
        Ok(())
    }

    fn emit_while(
        &self,
        w: &MonoWhile,
        locals: &HashMap<String, LocalSlot>,
        func: &mut Function,
        program: &MonoProgram,
        ret_ty: &SemTy,
    ) -> WasmResult<()> {
        // Wasm while: block { loop { cond; br_if_false exit; body; br loop } }
        func.instruction(&Instruction::Block(wasm_encoder::BlockType::Empty));
        func.instruction(&Instruction::Loop(wasm_encoder::BlockType::Empty));

        // Condition — if false, break out.
        self.emit_expr(&w.cond, locals, func, program)?;
        func.instruction(&Instruction::I32Eqz);
        func.instruction(&Instruction::BrIf(1)); // break outer block

        // Body
        self.emit_block_instrs(&w.body, locals, func, program, ret_ty)?;

        // Loop back.
        func.instruction(&Instruction::Br(0)); // continue inner loop
        func.instruction(&Instruction::End); // end loop
        func.instruction(&Instruction::End); // end block
        Ok(())
    }

    // ── Expressions ───────────────────────────────────────────────────────

    /// Returns true if an i/f/etc. value was pushed onto the stack.
    fn emit_expr(
        &self,
        expr: &MonoExpr,
        locals: &HashMap<String, LocalSlot>,
        func: &mut Function,
        program: &MonoProgram,
    ) -> WasmResult<bool> {
        match &expr.kind {
            MonoExprKind::Int(n) => {
                func.instruction(&Instruction::I64Const(*n));
                Ok(true)
            }
            MonoExprKind::Float(f) => {
                func.instruction(&Instruction::F64Const(*f));
                Ok(true)
            }
            MonoExprKind::Bool(b) => {
                func.instruction(&Instruction::I32Const(*b as i32));
                Ok(true)
            }
            MonoExprKind::Null => {
                func.instruction(&Instruction::I32Const(0));
                Ok(true)
            }
            MonoExprKind::String(_s) => {
                // v0.1: string literals are just a null pointer in Wasm.
                // Full string support requires linear memory setup.
                func.instruction(&Instruction::I32Const(0));
                Ok(true)
            }
            MonoExprKind::Var(name) => {
                if let Some(slot) = locals.get(name) {
                    func.instruction(&Instruction::LocalGet(slot.index));
                    Ok(true)
                } else {
                    // May be a global function name — not a value in Wasm.
                    Ok(false)
                }
            }
            MonoExprKind::Unary(op, operand) => {
                self.emit_expr(operand, locals, func, program)?;
                match (op, &operand.ty) {
                    (UnaryOp::Neg, SemTy::Int) => {
                        func.instruction(&Instruction::I64Const(-1));
                        func.instruction(&Instruction::I64Mul);
                    }
                    (UnaryOp::Neg, SemTy::Float) => {
                        func.instruction(&Instruction::F64Neg);
                    }
                    (UnaryOp::Not, _) => {
                        func.instruction(&Instruction::I32Eqz);
                    }
                    _ => {}
                }
                Ok(true)
            }
            MonoExprKind::Binary(op, lhs, rhs) => {
                self.emit_binary(*op, lhs, rhs, &expr.ty, locals, func, program)
            }
            MonoExprKind::Call(name, args) => {
                self.emit_call_expr(name, args, locals, func, program, &expr.ty)
            }
            MonoExprKind::If(i) => {
                self.emit_if_expr(i, locals, func, program, &expr.ty)
            }
            MonoExprKind::Assign(target, value) => {
                self.emit_expr(value, locals, func, program)?;
                if let MonoExprKind::Var(name) = &target.kind {
                    if let Some(slot) = locals.get(name.as_str()) {
                        func.instruction(&Instruction::LocalSet(slot.index));
                    }
                }
                Ok(false)
            }
            // Unsupported in v0.1 Wasm — emit nop and return false.
            MonoExprKind::Field(_, _)
            | MonoExprKind::Construct(_, _)
            | MonoExprKind::Index(_, _)
            | MonoExprKind::Array(_)
            | MonoExprKind::Async(_)
            | MonoExprKind::Match(_)
            | MonoExprKind::Block(_) => {
                Ok(false)
            }
        }
    }

    fn emit_binary(
        &self,
        op: BinaryOp,
        lhs: &MonoExpr,
        rhs: &MonoExpr,
        _ty: &SemTy,
        locals: &HashMap<String, LocalSlot>,
        func: &mut Function,
        program: &MonoProgram,
    ) -> WasmResult<bool> {
        let is_float = lhs.ty == SemTy::Float;
        self.emit_expr(lhs, locals, func, program)?;
        self.emit_expr(rhs, locals, func, program)?;

        let instr: Instruction = match (op, is_float) {
            (BinaryOp::Add, false) => Instruction::I64Add,
            (BinaryOp::Add, true)  => Instruction::F64Add,
            (BinaryOp::Sub, false) => Instruction::I64Sub,
            (BinaryOp::Sub, true)  => Instruction::F64Sub,
            (BinaryOp::Mul, false) => Instruction::I64Mul,
            (BinaryOp::Mul, true)  => Instruction::F64Mul,
            (BinaryOp::Div, false) => Instruction::I64DivS,
            (BinaryOp::Div, true)  => Instruction::F64Div,
            (BinaryOp::Mod, false) => Instruction::I64RemS,
            (BinaryOp::Eq,  false) => Instruction::I64Eq,
            (BinaryOp::Eq,  true)  => Instruction::F64Eq,
            (BinaryOp::Ne,  false) => Instruction::I64Ne,
            (BinaryOp::Ne,  true)  => Instruction::F64Ne,
            (BinaryOp::Lt,  false) => Instruction::I64LtS,
            (BinaryOp::Lt,  true)  => Instruction::F64Lt,
            (BinaryOp::Le,  false) => Instruction::I64LeS,
            (BinaryOp::Le,  true)  => Instruction::F64Le,
            (BinaryOp::Gt,  false) => Instruction::I64GtS,
            (BinaryOp::Gt,  true)  => Instruction::F64Gt,
            (BinaryOp::Ge,  false) => Instruction::I64GeS,
            (BinaryOp::Ge,  true)  => Instruction::F64Ge,
            (BinaryOp::And, _)     => Instruction::I32And,
            (BinaryOp::Or,  _)     => Instruction::I32Or,
            _ => return Err(WasmError::UnsupportedOp(format!("{op:?}"))),
        };
        func.instruction(&instr);
        Ok(true)
    }

    fn emit_call_expr(
        &self,
        name: &str,
        args: &[MonoExpr],
        locals: &HashMap<String, LocalSlot>,
        func: &mut Function,
        program: &MonoProgram,
        _ret_ty: &SemTy,
    ) -> WasmResult<bool> {
        // Push all arguments.
        for arg in args {
            self.emit_expr(arg, locals, func, program)?;
        }

        if let Some(entry) = self.fn_map.get(name) {
            func.instruction(&Instruction::Call(entry.index));
            Ok(true)
        } else {
            // Unknown function — drop args and push a zero.
            for _ in args { func.instruction(&Instruction::Drop); }
            Ok(false)
        }
    }

    fn emit_if_expr(
        &self,
        i: &MonoIf,
        locals: &HashMap<String, LocalSlot>,
        func: &mut Function,
        program: &MonoProgram,
        ty: &SemTy,
    ) -> WasmResult<bool> {
        self.emit_expr(&i.cond, locals, func, program)?;
        let block_ty = match sem_to_result(ty) {
            Some(vt) => wasm_encoder::BlockType::Result(vt),
            None     => wasm_encoder::BlockType::Empty,
        };
        func.instruction(&Instruction::If(block_ty));

        // Then: emit yield or last expr.
        for stmt in &i.then_block.stmts {
            if let MonoStmtKind::Yield(e) = &stmt.kind {
                self.emit_expr(e, locals, func, program)?;
            } else {
                self.emit_stmt(stmt, locals, func, program, ty)?;
            }
        }

        if let Some(else_br) = &i.else_branch {
            func.instruction(&Instruction::Else);
            match else_br {
                MonoElse::Block(b) => {
                    for stmt in &b.stmts {
                        if let MonoStmtKind::Yield(e) = &stmt.kind {
                            self.emit_expr(e, locals, func, program)?;
                        } else {
                            self.emit_stmt(stmt, locals, func, program, ty)?;
                        }
                    }
                }
                MonoElse::If(inner) => { self.emit_if_expr(inner, locals, func, program, ty)?; }
            }
        }
        func.instruction(&Instruction::End);
        Ok(sem_to_result(ty).is_some())
    }

    // ── Finish ────────────────────────────────────────────────────────────

    pub fn finish(self) -> Vec<u8> {
        let mut module = Module::new();
        module.section(&self.types);
        module.section(&self.imports);
        module.section(&self.functions);
        module.section(&self.exports);
        module.section(&self.codes);
        module.finish()
    }
}
