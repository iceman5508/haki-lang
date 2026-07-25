/// subst.rs — Type substitution for monomorphization.
///
/// `Subst` maps type-parameter names to concrete `SemTy`s and applies
/// the substitution recursively through the full typed AST.

use std::collections::HashMap;
use haki_typeck::typed_ast::SemTy;

// ── Substitution map ──────────────────────────────────────────────────────────

/// A substitution from type-parameter names to concrete types.
///
/// Example: `{ "T" → SemTy::Int, "U" → SemTy::Named("User") }`
#[derive(Debug, Clone, Default)]
pub struct Subst(pub HashMap<String, SemTy>);

impl Subst {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn from_pairs(pairs: impl IntoIterator<Item = (String, SemTy)>) -> Self {
        Self(pairs.into_iter().collect())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    // ── Apply to a SemTy ──────────────────────────────────────────────────

    /// Substitute all type variables in `ty` using this map.
    pub fn apply_ty(&self, ty: &SemTy) -> SemTy {
        if self.is_empty() {
            return ty.clone();
        }
        match ty {
            // A `Named` could be a user type *or* a type parameter.
            // If it's in the substitution map, replace it.
            SemTy::Named(n) => {
                if let Some(subbed) = self.0.get(n.as_str()) {
                    subbed.clone()
                } else {
                    ty.clone()
                }
            }
            SemTy::Optional(inner) => {
                SemTy::Optional(Box::new(self.apply_ty(inner)))
            }
            SemTy::Generic(name, args) => {
                SemTy::Generic(name.clone(), args.iter().map(|a| self.apply_ty(a)).collect())
            }
            SemTy::Tuple(tys) => {
                SemTy::Tuple(tys.iter().map(|t| self.apply_ty(t)).collect())
            }
            // Primitives and Never pass through unchanged.
            other => other.clone(),
        }
    }
}

// ── Name mangling ─────────────────────────────────────────────────────────────

/// Produce a mangled name for a generic instantiation.
///
/// Examples:
///   `mangle("best", &[SemTy::Int])`           → `"best__int"`
///   `mangle("Stack", &[SemTy::Named("User")])` → `"Stack__User"`
///   `mangle("identity", &[SemTy::Int, SemTy::String])` → `"identity__int__string"`
pub fn mangle(base: &str, type_args: &[SemTy]) -> String {
    if type_args.is_empty() {
        return base.to_string();
    }
    let suffix = type_args.iter().map(mangle_ty).collect::<Vec<_>>().join("__");
    format!("{base}__{suffix}")
}

fn mangle_ty(ty: &SemTy) -> String {
    match ty {
        SemTy::Int    => "int".into(),
        SemTy::Float  => "f64".into(),
        SemTy::Bool   => "bool".into(),
        SemTy::String => "string".into(),
        SemTy::Void   => "void".into(),
        SemTy::Never  => "never".into(),
        SemTy::Named(n) => n.clone(),
        SemTy::Optional(inner) => format!("opt_{}", mangle_ty(inner)),
        SemTy::Generic(name, args) => {
            if args.is_empty() {
                name.clone()
            } else {
                let inner = args.iter().map(mangle_ty).collect::<Vec<_>>().join("_");
                format!("{name}_{inner}")
            }
        }
        SemTy::Tuple(tys) => {
            format!("tup_{}", tys.iter().map(mangle_ty).collect::<Vec<_>>().join("_"))
        }
        SemTy::Var(n) => format!("var{n}"),
        SemTy::Fn(params, ret) => {
            let ps = params.iter().map(mangle_ty).collect::<Vec<_>>().join("_");
            format!("fn_{ps}_ret_{}", mangle_ty(ret))
        }
        SemTy::Closure(params, ret) => {
            let ps = params.iter().map(mangle_ty).collect::<Vec<_>>().join("_");
            format!("clo_{ps}_ret_{}", mangle_ty(ret))
        }
    }
}

/// Extract the type arguments when a `SemTy::Generic` or a `SemTy::Named`
/// is used as an instantiation target.
///
/// Returns `(base_name, type_args)`.
pub fn decompose_generic(ty: &SemTy) -> (&str, &[SemTy]) {
    match ty {
        SemTy::Generic(name, args) => (name.as_str(), args.as_slice()),
        SemTy::Named(name) => (name.as_str(), &[]),
        _ => ("", &[]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mangle_no_args() {
        assert_eq!(mangle("foo", &[]), "foo");
    }

    #[test]
    fn test_mangle_single() {
        assert_eq!(mangle("identity", &[SemTy::Int]), "identity__int");
    }

    #[test]
    fn test_mangle_multi() {
        assert_eq!(
            mangle("best", &[SemTy::Named("Temperature".into())]),
            "best__Temperature"
        );
    }

    #[test]
    fn test_mangle_generic_nested() {
        assert_eq!(
            mangle("wrap", &[SemTy::Generic("Array".into(), vec![SemTy::Int])]),
            "wrap__Array_int"
        );
    }

    #[test]
    fn test_subst_apply() {
        let subst = Subst::from_pairs([("T".into(), SemTy::Int)]);
        assert_eq!(subst.apply_ty(&SemTy::Named("T".into())), SemTy::Int);
        assert_eq!(
            subst.apply_ty(&SemTy::Optional(Box::new(SemTy::Named("T".into())))),
            SemTy::Optional(Box::new(SemTy::Int))
        );
        // Non-type-param Named passes through.
        assert_eq!(
            subst.apply_ty(&SemTy::Named("User".into())),
            SemTy::Named("User".into())
        );
    }

    #[test]
    fn test_subst_tuple() {
        let subst = Subst::from_pairs([("T".into(), SemTy::String)]);
        let input = SemTy::Tuple(vec![SemTy::Named("T".into()), SemTy::Bool]);
        let result = subst.apply_ty(&input);
        assert_eq!(result, SemTy::Tuple(vec![SemTy::String, SemTy::Bool]));
    }

    #[test]
    fn test_subst_empty_passthrough() {
        let subst = Subst::new();
        let ty = SemTy::Generic("Array".into(), vec![SemTy::Named("T".into())]);
        assert_eq!(subst.apply_ty(&ty), ty);
    }
}
