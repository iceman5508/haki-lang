/// types.rs — Maps Haki `ConcrTy` (SemTy) to inkwell LLVM types.
///
/// Type mapping:
///   int    → i64          bool   → i1
///   f64    → double       string → ptr (i8*)
///   Named/Generic/Optional/Tuple → ptr  (all heap/stack via pointer)
///   void/Never → void (return positions only)

use inkwell::context::Context;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum, PointerType};
use inkwell::AddressSpace;
use haki_typeck::typed_ast::SemTy;
use crate::error::{CodeGenError, CodeGenResult};

/// Convenience: get the canonical opaque pointer type for this context.
pub fn ptr_ty(ctx: &Context) -> PointerType<'_> {
    ctx.i8_type().ptr_type(AddressSpace::default())
}

pub struct TypeMap<'ctx> {
    pub ctx: &'ctx Context,
}

impl<'ctx> TypeMap<'ctx> {
    pub fn new(ctx: &'ctx Context) -> Self {
        Self { ctx }
    }

    pub fn ptr(&self) -> PointerType<'ctx> {
        ptr_ty(self.ctx)
    }

    /// Map a `SemTy` to an inkwell `BasicTypeEnum`.
    pub fn basic(&self, ty: &SemTy) -> CodeGenResult<BasicTypeEnum<'ctx>> {
        match ty {
            SemTy::Int    => Ok(self.ctx.i64_type().into()),
            SemTy::Float  => Ok(self.ctx.f64_type().into()),
            SemTy::Bool   => Ok(self.ctx.bool_type().into()),
            SemTy::String | SemTy::Named(_) | SemTy::Generic(_, _)
            | SemTy::Optional(_) | SemTy::Tuple(_)
            | SemTy::Fn(_, _)       // plain function pointer
            | SemTy::Closure(_, _)  // fat pointer {fn_ptr, env_ptr} — heap pointer
                               => Ok(self.ptr().into()),
            SemTy::Void  => Err(CodeGenError::UnknownType("void in value position".into())),
            SemTy::Never => Err(CodeGenError::UnknownType("Never in value position".into())),
            // Unresolved type variables (e.g. T from builtin Array<T> signatures)
            // are treated as opaque pointers — they're erased by monomorphization
            // but builtin method stubs may still carry them.
            SemTy::Var(_) => Ok(self.ptr().into()),
        }
    }

    /// Return type for a function — None = void.
    pub fn return_type(&self, ty: &SemTy) -> CodeGenResult<Option<BasicTypeEnum<'ctx>>> {
        match ty {
            SemTy::Void | SemTy::Never => Ok(None),
            other => Ok(Some(self.basic(other)?)),
        }
    }

    /// For use in function parameter lists.
    pub fn metadata(&self, ty: &SemTy) -> CodeGenResult<BasicMetadataTypeEnum<'ctx>> {
        Ok(self.basic(ty)?.into())
    }

    /// True if this type is a class (reference type) that needs ARC.
    /// Requires the layout to distinguish classes from structs.
    pub fn is_arc_type(ty: &SemTy, layout: &crate::layout::Layout) -> bool {
        match ty {
            SemTy::Named(n) => layout.is_class.get(n.as_str()).copied().unwrap_or(false),
            SemTy::Optional(inner) => Self::is_arc_type(inner, layout),
            _ => false,
        }
    }
}
