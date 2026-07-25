/// layout.rs — Pre-registers struct and class LLVM struct types.
///
/// ARC header (class field 0):  { i64 refcount, ptr destructor }
/// Struct: plain fields in order.
/// Class:  ARC header + user fields.

use std::collections::HashMap;
use inkwell::context::Context;
use inkwell::types::StructType;
use haki_mono::mono_ast::{MonoField, MonoProgram};
use haki_typeck::typed_ast::SemTy;
use crate::types::{ptr_ty, TypeMap};
use crate::error::CodeGenResult;

pub struct Layout<'ctx> {
    pub structs:  HashMap<String, StructType<'ctx>>,
    pub is_class: HashMap<String, bool>,
}

impl<'ctx> Layout<'ctx> {
    pub fn build(ctx: &'ctx Context, program: &MonoProgram) -> CodeGenResult<Self> {
        let tmap = TypeMap::new(ctx);
        let mut structs  = HashMap::new();
        let mut is_class = HashMap::new();

        // Phase 1: forward-declare all opaque named structs.
        for s in &program.structs {
            structs.insert(s.name.clone(), ctx.opaque_struct_type(&s.name));
            is_class.insert(s.name.clone(), false);
        }
        for c in &program.classes {
            structs.insert(c.name.clone(), ctx.opaque_struct_type(&c.name));
            is_class.insert(c.name.clone(), true);
        }

        // ARC header: { i64, ptr }
        let arc_header = ctx.struct_type(
            &[ctx.i64_type().into(), ptr_ty(ctx).into()],
            false,
        );

        // Phase 2: set struct bodies.
        for s in &program.structs {
            let field_tys = fields_to_ll(&s.fields, &tmap)?;
            structs[&s.name].set_body(&field_tys, false);
        }
        for c in &program.classes {
            let mut field_tys = vec![arc_header.into()];
            field_tys.extend(fields_to_ll(&c.fields, &tmap)?);
            structs[&c.name].set_body(&field_tys, false);
        }

        Ok(Self { structs, is_class })
    }

    pub fn get(&self, name: &str) -> Option<StructType<'ctx>> {
        self.structs.get(name).copied()
    }

    /// LLVM field index, accounting for the ARC header on classes.
    pub fn field_index(&self, type_name: &str, user_pos: usize) -> u32 {
        let offset = if *self.is_class.get(type_name).unwrap_or(&false) { 1 } else { 0 };
        (user_pos + offset) as u32
    }
}

fn fields_to_ll<'ctx>(
    fields: &[MonoField],
    tmap: &TypeMap<'ctx>,
) -> CodeGenResult<Vec<inkwell::types::BasicTypeEnum<'ctx>>> {
    fields.iter().map(|f| match &f.ty {
        SemTy::Void | SemTy::Never => Ok(tmap.ctx.i8_type().into()),
        ty => tmap.basic(ty),
    }).collect()
}
