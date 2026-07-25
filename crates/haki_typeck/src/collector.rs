/// collector.rs — Pass 1: symbol table construction.
///
/// Scans all top-level items and registers:
///   - Type definitions (struct, class) with their fields and methods.
///   - Protocol definitions with their required method signatures.
///   - Protocol implementations (impl blocks).
///   - Top-level function definitions.
///
/// This pass does NOT enter function bodies. That happens in the inference pass.
/// All names must be resolved against this table during inference.

use std::collections::HashMap;
use haki_ast::*;
use crate::error::{TypeError, TypeResult};
use crate::typed_ast::SemTy;

// ── Type descriptor ───────────────────────────────────────────────────────────

/// Describes a user-defined type (struct or class).
#[derive(Debug, Clone)]
pub struct TypeDef {
    pub kind: TypeKind,
    pub name: String,
    pub type_params: Vec<String>,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<FnInfo>,
    /// For classes: name of superclass if any.
    pub superclass: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Struct, // value type — stack allocated
    Class,  // reference type — ARC heap allocated
}

/// A field as seen by the symbol table.
#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    pub ty: Ty,   // syntactic — resolved during inference
    pub mutability: Mut,
    pub is_weak: bool,
    pub span: Span,
}

/// A method/function signature as seen by the symbol table.
#[derive(Debug, Clone)]
pub struct FnInfo {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    pub return_ty: Option<ReturnTy>,
    pub span: Span,
}

impl FnInfo {
    fn from_fn_def(f: &FnDef) -> Self {
        Self {
            name: f.name.name.clone(),
            type_params: f.type_params.clone(),
            params: f.params.clone(),
            return_ty: f.return_ty.clone(),
            span: f.span,
        }
    }

    fn from_fn_sig(s: &FnSig) -> Self {
        Self {
            name: s.name.name.clone(),
            type_params: s.type_params.clone(),
            params: s.params.clone(),
            return_ty: s.return_ty.clone(),
            span: s.span,
        }
    }
}

// ── Protocol descriptor ───────────────────────────────────────────────────────

/// A protocol definition with optional default method implementations.
#[derive(Debug, Clone)]
pub struct ProtocolInfo {
    pub name: String,
    /// Required signatures — must be provided by implementors.
    pub methods: Vec<FnInfo>,
    /// Default method bodies — used when the impl block omits them.
    /// Stored as untyped AST; typechecked against the concrete type at inject time.
    pub default_methods: HashMap<String, FnDef>,
    pub span: Span,
}

// ── Impl record ───────────────────────────────────────────────────────────────

/// A record that type `target` implements protocol `protocol`.
#[derive(Debug, Clone)]
pub struct ImplRecord {
    pub protocol: String,
    pub target: String,
    pub methods: Vec<FnInfo>,
    pub span: Span,
}

// ── Symbol table ──────────────────────────────────────────────────────────────

/// The global symbol table built during Pass 1.
///
/// Lookup methods are used heavily by the inference pass.
#[derive(Debug, Default)]
pub struct SymbolTable {
    /// All named types (struct + class), keyed by type name.
    pub types: HashMap<String, TypeDef>,

    /// All enum definitions, keyed by enum name.
    pub enum_defs: HashMap<String, EnumDef>,

    /// All protocol definitions, keyed by protocol name.
    pub protocols: HashMap<String, ProtocolInfo>,

    /// All `impl Protocol for Type` records.
    /// Outer key: type name. Inner key: protocol name.
    pub impls: HashMap<String, HashMap<String, ImplRecord>>,

    /// Top-level functions, keyed by function name.
    pub functions: HashMap<String, FnInfo>,

    /// In-scope type parameter → protocol bounds.
    pub type_param_bounds: HashMap<String, Vec<String>>,

    /// Module registry: alias → per-module symbol table.
    pub modules: HashMap<String, ModuleSymbols>,
}

/// The symbols exported by a single imported module.
#[derive(Debug, Clone, Default)]
pub struct ModuleSymbols {
    pub functions: HashMap<String, FnInfo>,
    pub types:     HashMap<String, TypeDef>,
    pub enum_defs: HashMap<String, EnumDef>,
}

impl SymbolTable {
    pub fn new() -> Self {
        let mut sym = Self::default();
        sym.register_builtins();
        sym
    }

    // ── Built-in types ────────────────────────────────────────────────────

    /// Register the built-in types and protocols that every Haki program has.
    fn register_builtins(&mut self) {
        // `Error` class — base error type
        let error_def = TypeDef {
            kind: TypeKind::Class,
            name: "Error".into(),
            type_params: vec![],
            fields: vec![
                FieldInfo {
                    name: "message".into(),
                    ty: Ty { kind: TyKind::Named(Ident::new("string", Span::dummy())), span: Span::dummy() },
                    mutability: Mut::Const,
                    is_weak: false,
                    span: Span::dummy(),
                },
                FieldInfo {
                    name: "cause".into(),
                    ty: Ty {
                        kind: TyKind::Optional(Box::new(Ty {
                            kind: TyKind::Named(Ident::new("Error", Span::dummy())),
                            span: Span::dummy(),
                        })),
                        span: Span::dummy(),
                    },
                    mutability: Mut::Const,
                    is_weak: false,
                    span: Span::dummy(),
                },
            ],
            methods: vec![],
            superclass: None,
            span: Span::dummy(),
        };
        self.types.insert("Error".into(), error_def);

        // `Array<T>` — generic collection
        let array_def = TypeDef {
            kind: TypeKind::Class,
            name: "Array".into(),
            type_params: vec!["T".into()],
            fields: vec![
                FieldInfo {
                    name: "length".into(),
                    ty: Ty { kind: TyKind::Named(Ident::new("int", Span::dummy())), span: Span::dummy() },
                    mutability: Mut::Const,
                    is_weak: false,
                    span: Span::dummy(),
                },
            ],
            methods: vec![
                // fn append(item: T) — returns void
                FnInfo {
                    name: "append".into(),
                    type_params: vec![],
                    params: vec![Param {
                        name: Ident::new("item", Span::dummy()),
                        ty: Ty { kind: TyKind::Named(Ident::new("T", Span::dummy())), span: Span::dummy() },
                        span: Span::dummy(),
                    }],
                    return_ty: None,
                    span: Span::dummy(),
                },
                // fn removeLast() -> (T?, Error?)
                FnInfo {
                    name: "removeLast".into(),
                    type_params: vec![],
                    params: vec![],
                    return_ty: Some(ReturnTy::Tuple(vec![
                        Ty { kind: TyKind::Optional(Box::new(Ty { kind: TyKind::Named(Ident::new("T", Span::dummy())), span: Span::dummy() })), span: Span::dummy() },
                        Ty { kind: TyKind::Optional(Box::new(Ty { kind: TyKind::Named(Ident::new("Error", Span::dummy())), span: Span::dummy() })), span: Span::dummy() },
                    ])),
                    span: Span::dummy(),
                },
                // fn contains(item: T) -> bool
                FnInfo {
                    name: "contains".into(), type_params: vec![],
                    params: vec![Param { name: Ident::new("item", Span::dummy()),
                        ty: Ty { kind: TyKind::Named(Ident::new("T", Span::dummy())), span: Span::dummy() }, span: Span::dummy() }],
                    return_ty: Some(ReturnTy::Single(Ty { kind: TyKind::Named(Ident::new("bool", Span::dummy())), span: Span::dummy() })),
                    span: Span::dummy(),
                },
                // fn indexOf(item: T) -> int
                FnInfo {
                    name: "indexOf".into(), type_params: vec![],
                    params: vec![Param { name: Ident::new("item", Span::dummy()),
                        ty: Ty { kind: TyKind::Named(Ident::new("T", Span::dummy())), span: Span::dummy() }, span: Span::dummy() }],
                    return_ty: Some(ReturnTy::Single(Ty { kind: TyKind::Named(Ident::new("int", Span::dummy())), span: Span::dummy() })),
                    span: Span::dummy(),
                },
                // fn removeAt(idx: int) -> (T?, Error?)
                FnInfo {
                    name: "removeAt".into(), type_params: vec![],
                    params: vec![Param { name: Ident::new("idx", Span::dummy()),
                        ty: Ty { kind: TyKind::Named(Ident::new("int", Span::dummy())), span: Span::dummy() }, span: Span::dummy() }],
                    return_ty: Some(ReturnTy::Tuple(vec![
                        Ty { kind: TyKind::Optional(Box::new(Ty { kind: TyKind::Named(Ident::new("T", Span::dummy())), span: Span::dummy() })), span: Span::dummy() },
                        Ty { kind: TyKind::Optional(Box::new(Ty { kind: TyKind::Named(Ident::new("Error", Span::dummy())), span: Span::dummy() })), span: Span::dummy() },
                    ])),
                    span: Span::dummy(),
                },
                // fn first() -> T?
                FnInfo {
                    name: "first".into(), type_params: vec![], params: vec![],
                    return_ty: Some(ReturnTy::Single(Ty { kind: TyKind::Optional(Box::new(
                        Ty { kind: TyKind::Named(Ident::new("T", Span::dummy())), span: Span::dummy() }
                    )), span: Span::dummy() })),
                    span: Span::dummy(),
                },
                // fn last() -> T?
                FnInfo {
                    name: "last".into(), type_params: vec![], params: vec![],
                    return_ty: Some(ReturnTy::Single(Ty { kind: TyKind::Optional(Box::new(
                        Ty { kind: TyKind::Named(Ident::new("T", Span::dummy())), span: Span::dummy() }
                    )), span: Span::dummy() })),
                    span: Span::dummy(),
                },
                // fn join(sep: string) -> string  [only valid for Array<string>]
                FnInfo {
                    name: "join".into(), type_params: vec![],
                    params: vec![Param { name: Ident::new("sep", Span::dummy()),
                        ty: Ty { kind: TyKind::Named(Ident::new("string", Span::dummy())), span: Span::dummy() }, span: Span::dummy() }],
                    return_ty: Some(ReturnTy::Single(Ty { kind: TyKind::Named(Ident::new("string", Span::dummy())), span: Span::dummy() })),
                    span: Span::dummy(),
                },
            ],
            superclass: None,
            span: Span::dummy(),
        };
        self.types.insert("Array".into(), array_def);

        // `Map<K, V>` — key-value map
        let int_ty_node = Ty { kind: TyKind::Named(Ident::new("int", Span::dummy())), span: Span::dummy() };
        let bool_ty_node = Ty { kind: TyKind::Named(Ident::new("bool", Span::dummy())), span: Span::dummy() };
        let k_ty_node = Ty { kind: TyKind::Named(Ident::new("K", Span::dummy())), span: Span::dummy() };
        let v_ty_node = Ty { kind: TyKind::Named(Ident::new("V", Span::dummy())), span: Span::dummy() };
        let map_def = TypeDef {
            kind: TypeKind::Class,
            name: "Map".into(),
            type_params: vec!["K".into(), "V".into()],
            fields: vec![
                FieldInfo {
                    name: "length".into(),
                    ty: int_ty_node.clone(),
                    mutability: Mut::Const,
                    is_weak: false,
                    span: Span::dummy(),
                },
            ],
            methods: vec![
                FnInfo {
                    name: "get".into(),
                    type_params: vec![],
                    params: vec![Param {
                        name: Ident::new("key", Span::dummy()),
                        ty: k_ty_node.clone(),
                        span: Span::dummy(),
                    }],
                    return_ty: Some(ReturnTy::Single(Ty {
                        kind: TyKind::Optional(Box::new(v_ty_node.clone())),
                        span: Span::dummy(),
                    })),
                    span: Span::dummy(),
                },
                FnInfo {
                    name: "set".into(),
                    type_params: vec![],
                    params: vec![
                        Param {
                            name: Ident::new("key", Span::dummy()),
                            ty: k_ty_node.clone(),
                            span: Span::dummy(),
                        },
                        Param {
                            name: Ident::new("value", Span::dummy()),
                            ty: v_ty_node.clone(),
                            span: Span::dummy(),
                        },
                    ],
                    return_ty: None,
                    span: Span::dummy(),
                },
                FnInfo {
                    name: "has".into(),
                    type_params: vec![],
                    params: vec![Param {
                        name: Ident::new("key", Span::dummy()),
                        ty: k_ty_node.clone(),
                        span: Span::dummy(),
                    }],
                    return_ty: Some(ReturnTy::Single(bool_ty_node.clone())),
                    span: Span::dummy(),
                },
                FnInfo {
                    name: "delete".into(),
                    type_params: vec![],
                    params: vec![Param {
                        name: Ident::new("key", Span::dummy()),
                        ty: k_ty_node.clone(),
                        span: Span::dummy(),
                    }],
                    return_ty: None,
                    span: Span::dummy(),
                },
                // getOrDefault(key, default) -> V — returns default if key not found
                FnInfo {
                    name: "getOrDefault".into(),
                    type_params: vec![],
                    params: vec![
                        Param { name: Ident::new("key", Span::dummy()), ty: k_ty_node.clone(), span: Span::dummy() },
                        Param { name: Ident::new("default", Span::dummy()), ty: v_ty_node.clone(), span: Span::dummy() },
                    ],
                    return_ty: Some(ReturnTy::Single(v_ty_node.clone())),
                    span: Span::dummy(),
                },
            ],
            superclass: None,
            span: Span::dummy(),
        };
        self.types.insert("Map".into(), map_def);

        // Register built-in protocols
        let protocols = [
            ("Comparable", vec![("compareTo", vec![("other", "Self")], Some("int"))]),
            ("Equatable",  vec![("equals",    vec![("other", "Self")], Some("bool"))]),
            ("Printable",  vec![("toString",  vec![],                  Some("string"))]),
            ("Hashable",   vec![("hashCode",  vec![],                  Some("int"))]),
            ("Cloneable",  vec![("clone",     vec![],                  Some("Self"))]),
        ];

        for (proto_name, methods) in &protocols {
            let method_infos = methods.iter().map(|(mname, params, ret)| {
                FnInfo {
                    name: mname.to_string(),
                    type_params: vec![],
                    params: params.iter().map(|(pname, pty)| Param {
                        name: Ident::new(*pname, Span::dummy()),
                        ty: Ty { kind: TyKind::Named(Ident::new(*pty, Span::dummy())), span: Span::dummy() },
                        span: Span::dummy(),
                    }).collect(),
                    return_ty: ret.map(|r| ReturnTy::Single(Ty {
                        kind: TyKind::Named(Ident::new(r, Span::dummy())),
                        span: Span::dummy(),
                    })),
                    span: Span::dummy(),
                }
            }).collect();

            self.protocols.insert(proto_name.to_string(), ProtocolInfo {
                name: proto_name.to_string(),
                methods: method_infos,
                default_methods: HashMap::new(),
            span: Span::dummy(),
            });
        }

        // `Iterable<T>` — separate because it has a generic return type
        self.protocols.insert("Iterable".into(), ProtocolInfo {
            name: "Iterable".into(),
            methods: vec![FnInfo {
                name: "iterator".into(),
                type_params: vec![],
                params: vec![],
                return_ty: Some(ReturnTy::Single(Ty {
                    kind: TyKind::Generic(
                        Ident::new("Iterator", Span::dummy()),
                        vec![Ty { kind: TyKind::Named(Ident::new("T", Span::dummy())), span: Span::dummy() }],
                    ),
                    span: Span::dummy(),
                })),
                span: Span::dummy(),
            }],
            default_methods: HashMap::new(),
            span: Span::dummy(),
        });

        // `string` — builtin string type with methods.
        // Even though `string` is a primitive SemTy, we register it as a
        // TypeDef so method calls like `s.split(",")` can be resolved.
        let str_ty_node  = Ty { kind: TyKind::Named(Ident::new("string",       Span::dummy())), span: Span::dummy() };
        let int_ty_node2 = Ty { kind: TyKind::Named(Ident::new("int",          Span::dummy())), span: Span::dummy() };
        let bool_ty_node2= Ty { kind: TyKind::Named(Ident::new("bool",         Span::dummy())), span: Span::dummy() };
        let str_arr_ty   = Ty { kind: TyKind::Generic(Ident::new("Array", Span::dummy()), vec![str_ty_node.clone()]), span: Span::dummy() };

        fn str_param(name: &str) -> Param {
            Param { name: Ident::new(name, Span::dummy()), ty: Ty { kind: TyKind::Named(Ident::new("string", Span::dummy())), span: Span::dummy() }, span: Span::dummy() }
        }

        let string_def = TypeDef {
            kind: TypeKind::Struct, // value type — no ARC
            name: "string".into(),
            type_params: vec![],
            fields: vec![
                FieldInfo { name: "length".into(), ty: int_ty_node2.clone(), mutability: Mut::Const, is_weak: false, span: Span::dummy() },
            ],
            methods: vec![
                // fn split(sep: string) -> Array<string>
                FnInfo { name: "split".into(),       type_params: vec![], params: vec![str_param("sep")],    return_ty: Some(ReturnTy::Single(str_arr_ty.clone())),   span: Span::dummy() },
                // fn trim() -> string
                FnInfo { name: "trim".into(),         type_params: vec![], params: vec![],                    return_ty: Some(ReturnTy::Single(str_ty_node.clone())),  span: Span::dummy() },
                // fn trimStart() -> string
                FnInfo { name: "trimStart".into(),    type_params: vec![], params: vec![],                    return_ty: Some(ReturnTy::Single(str_ty_node.clone())),  span: Span::dummy() },
                // fn trimEnd() -> string
                FnInfo { name: "trimEnd".into(),      type_params: vec![], params: vec![],                    return_ty: Some(ReturnTy::Single(str_ty_node.clone())),  span: Span::dummy() },
                // fn toUpper() -> string
                FnInfo { name: "toUpper".into(),      type_params: vec![], params: vec![],                    return_ty: Some(ReturnTy::Single(str_ty_node.clone())),  span: Span::dummy() },
                // fn toLower() -> string
                FnInfo { name: "toLower".into(),      type_params: vec![], params: vec![],                    return_ty: Some(ReturnTy::Single(str_ty_node.clone())),  span: Span::dummy() },
                // fn contains(sub: string) -> bool
                FnInfo { name: "contains".into(),     type_params: vec![], params: vec![str_param("sub")],   return_ty: Some(ReturnTy::Single(bool_ty_node2.clone())), span: Span::dummy() },
                // fn startsWith(prefix: string) -> bool
                FnInfo { name: "startsWith".into(),   type_params: vec![], params: vec![str_param("prefix")],return_ty: Some(ReturnTy::Single(bool_ty_node2.clone())), span: Span::dummy() },
                // fn endsWith(suffix: string) -> bool
                FnInfo { name: "endsWith".into(),     type_params: vec![], params: vec![str_param("suffix")],return_ty: Some(ReturnTy::Single(bool_ty_node2.clone())), span: Span::dummy() },
                // fn replace(from: string, to: string) -> string
                FnInfo { name: "replace".into(),      type_params: vec![], params: vec![str_param("from"), str_param("to")], return_ty: Some(ReturnTy::Single(str_ty_node.clone())), span: Span::dummy() },
                // fn indexOf(sub: string) -> int   (-1 = not found)
                FnInfo { name: "indexOf".into(),      type_params: vec![], params: vec![str_param("sub")],   return_ty: Some(ReturnTy::Single(int_ty_node2.clone())), span: Span::dummy() },
                // fn substring(start: int, end: int) -> string
                FnInfo { name: "substring".into(),    type_params: vec![],
                    params: vec![
                        Param { name: Ident::new("start", Span::dummy()), ty: Ty { kind: TyKind::Named(Ident::new("int", Span::dummy())), span: Span::dummy() }, span: Span::dummy() },
                        Param { name: Ident::new("end",   Span::dummy()), ty: Ty { kind: TyKind::Named(Ident::new("int", Span::dummy())), span: Span::dummy() }, span: Span::dummy() },
                    ],
                    return_ty: Some(ReturnTy::Single(str_ty_node.clone())), span: Span::dummy() },
            ],
            superclass: None,
            span: Span::dummy(),
        };
        self.types.insert("string".into(), string_def);

        // `Thread` — OS-level thread handle (v0.1: 1:1 pthread wrapper)
        let void_ty = Ty { kind: TyKind::Named(Ident::new("void", Span::dummy())), span: Span::dummy() };

        // ── v0.4: haki_web types ──────────────────────────────────────────────

        let str_ty2 = Ty { kind: TyKind::Named(Ident::new("string", Span::dummy())), span: Span::dummy() };
        let int_ty2 = Ty { kind: TyKind::Named(Ident::new("int",    Span::dummy())), span: Span::dummy() };

        // HttpRequest
        let http_req_def = TypeDef {
            kind: TypeKind::Class, name: "HttpRequest".into(), type_params: vec![],
            fields: vec![
                FieldInfo { name: "path".into(),   ty: str_ty2.clone(), mutability: Mut::Const, is_weak: false, span: Span::dummy() },
                FieldInfo { name: "method".into(), ty: str_ty2.clone(), mutability: Mut::Const, is_weak: false, span: Span::dummy() },
                FieldInfo { name: "body".into(),   ty: str_ty2.clone(), mutability: Mut::Const, is_weak: false, span: Span::dummy() },
            ],
            methods: vec![], superclass: None, span: Span::dummy(),
        };
        self.types.insert("HttpRequest".into(), http_req_def);

        // HttpResponse
        let http_resp_def = TypeDef {
            kind: TypeKind::Class, name: "HttpResponse".into(), type_params: vec![],
            fields: vec![
                FieldInfo { name: "status".into(), ty: int_ty2.clone(), mutability: Mut::Const, is_weak: false, span: Span::dummy() },
                FieldInfo { name: "body".into(),   ty: str_ty2.clone(), mutability: Mut::Const, is_weak: false, span: Span::dummy() },
            ],
            methods: vec![], superclass: None, span: Span::dummy(),
        };
        self.types.insert("HttpResponse".into(), http_resp_def);

        // HttpServer
        let http_server_def = TypeDef {
            kind: TypeKind::Class, name: "HttpServer".into(), type_params: vec![],
            fields: vec![],
            methods: vec![
                FnInfo { name: "listen".into(), type_params: vec![], params: vec![], return_ty: None, span: Span::dummy() },
            ],
            superclass: None, span: Span::dummy(),
        };
        self.types.insert("HttpServer".into(), http_server_def);

        // HttpServer constructor as free function
        self.functions.insert("HttpServer".into(), FnInfo {
            name: "HttpServer".into(), type_params: vec![],
            params: vec![
                Param { name: Ident::new("port",    Span::dummy()), ty: int_ty2.clone(), span: Span::dummy() },
                Param { name: Ident::new("handler", Span::dummy()), ty: str_ty2.clone(), span: Span::dummy() },
            ],
            return_ty: Some(ReturnTy::Single(Ty { kind: TyKind::Named(Ident::new("HttpServer", Span::dummy())), span: Span::dummy() })),
            span: Span::dummy(),
        });

        // ── Router ──────────────────────────────────────────────────────────
        // `Router` type — pattern-matching HTTP router
        let req_ty = Ty { kind: TyKind::Named(Ident::new("HttpRequest",  Span::dummy())), span: Span::dummy() };
        let resp_ty= Ty { kind: TyKind::Named(Ident::new("HttpResponse", Span::dummy())), span: Span::dummy() };
        let handler_ty = Ty {
            kind: TyKind::Fn(vec![req_ty.clone()], Some(Box::new(resp_ty.clone()))),
            span: Span::dummy(),
        };
        let str_ty3 = Ty { kind: TyKind::Named(Ident::new("string", Span::dummy())), span: Span::dummy() };

        let router_def = TypeDef {
            kind: TypeKind::Class,
            name: "Router".into(),
            type_params: vec![],
            fields: vec![],
            methods: vec![
                FnInfo { name: "get".into(),    type_params: vec![], params: vec![
                    Param { name: Ident::new("pattern", Span::dummy()), ty: str_ty3.clone(), span: Span::dummy() },
                    Param { name: Ident::new("handler", Span::dummy()), ty: handler_ty.clone(), span: Span::dummy() },
                ], return_ty: None, span: Span::dummy() },
                FnInfo { name: "post".into(),   type_params: vec![], params: vec![
                    Param { name: Ident::new("pattern", Span::dummy()), ty: str_ty3.clone(), span: Span::dummy() },
                    Param { name: Ident::new("handler", Span::dummy()), ty: handler_ty.clone(), span: Span::dummy() },
                ], return_ty: None, span: Span::dummy() },
                FnInfo { name: "put".into(),    type_params: vec![], params: vec![
                    Param { name: Ident::new("pattern", Span::dummy()), ty: str_ty3.clone(), span: Span::dummy() },
                    Param { name: Ident::new("handler", Span::dummy()), ty: handler_ty.clone(), span: Span::dummy() },
                ], return_ty: None, span: Span::dummy() },
                FnInfo { name: "delete".into(), type_params: vec![], params: vec![
                    Param { name: Ident::new("pattern", Span::dummy()), ty: str_ty3.clone(), span: Span::dummy() },
                    Param { name: Ident::new("handler", Span::dummy()), ty: handler_ty.clone(), span: Span::dummy() },
                ], return_ty: None, span: Span::dummy() },
                FnInfo { name: "handle".into(), type_params: vec![], params: vec![
                    Param { name: Ident::new("req", Span::dummy()), ty: req_ty.clone(), span: Span::dummy() },
                ], return_ty: Some(ReturnTy::Single(resp_ty.clone())), span: Span::dummy() },
            ],
            superclass: None,
            span: Span::dummy(),
        };
        self.types.insert("Router".into(), router_def);

        // Router() free constructor
        self.functions.insert("Router".into(), FnInfo {
            name: "Router".into(), type_params: vec![], params: vec![],
            return_ty: Some(ReturnTy::Single(Ty { kind: TyKind::Named(Ident::new("Router", Span::dummy())), span: Span::dummy() })),
            span: Span::dummy(),
        });

        // requestParam(req, key) -> string — extract a route param
        self.functions.insert("requestParam".into(), FnInfo {
            name: "requestParam".into(), type_params: vec![],
            params: vec![
                Param { name: Ident::new("req", Span::dummy()), ty: req_ty.clone(), span: Span::dummy() },
                Param { name: Ident::new("key", Span::dummy()), ty: str_ty3.clone(), span: Span::dummy() },
            ],
            return_ty: Some(ReturnTy::Single(str_ty3.clone())),
            span: Span::dummy(),
        });

        // routerDispatch(router, req) -> HttpResponse — dispatch a request through a router
        self.functions.insert("routerDispatch".into(), FnInfo {
            name: "routerDispatch".into(), type_params: vec![],
            params: vec![
                Param { name: Ident::new("router", Span::dummy()),
                    ty: Ty { kind: TyKind::Named(Ident::new("Router", Span::dummy())), span: Span::dummy() },
                    span: Span::dummy() },
                Param { name: Ident::new("req", Span::dummy()), ty: req_ty.clone(), span: Span::dummy() },
            ],
            return_ty: Some(ReturnTy::Single(resp_ty.clone())),
            span: Span::dummy(),
        });

        // requestQuery(req, key) -> string — extract a query param
        self.functions.insert("requestQuery".into(), FnInfo {
            name: "requestQuery".into(), type_params: vec![],
            params: vec![
                Param { name: Ident::new("req", Span::dummy()), ty: req_ty.clone(), span: Span::dummy() },
                Param { name: Ident::new("key", Span::dummy()), ty: str_ty3.clone(), span: Span::dummy() },
            ],
            return_ty: Some(ReturnTy::Single(str_ty3.clone())),
            span: Span::dummy(),
        });

        // JSON free functions
        self.functions.insert("jsonString".into(), FnInfo {
            name: "jsonString".into(), type_params: vec![],
            params: vec![Param { name: Ident::new("v", Span::dummy()), ty: str_ty2.clone(), span: Span::dummy() }],
            return_ty: Some(ReturnTy::Single(str_ty2.clone())), span: Span::dummy(),
        });
        self.functions.insert("jsonInt".into(), FnInfo {
            name: "jsonInt".into(), type_params: vec![],
            params: vec![Param { name: Ident::new("v", Span::dummy()), ty: int_ty2.clone(), span: Span::dummy() }],
            return_ty: Some(ReturnTy::Single(str_ty2.clone())), span: Span::dummy(),
        });
        // ── haki_ui — v0.5 ─────────────────────────────────────────────────

        let view_ty  = Ty { kind: TyKind::Named(Ident::new("View",  Span::dummy())), span: Span::dummy() };
        let str_ui   = Ty { kind: TyKind::Named(Ident::new("string",Span::dummy())), span: Span::dummy() };
        let bool_ty  = Ty { kind: TyKind::Named(Ident::new("bool",  Span::dummy())), span: Span::dummy() };
        let void_fn  = Ty { kind: TyKind::Fn(vec![], None), span: Span::dummy() };
        let str_fn   = Ty { kind: TyKind::Fn(vec![str_ui.clone()], None), span: Span::dummy() };
        let arr_view = Ty {
            kind: TyKind::Generic(
                Ident::new("Array", Span::dummy()),
                vec![view_ty.clone()],
            ),
            span: Span::dummy(),
        };
        let view_ret = Some(ReturnTy::Single(view_ty.clone()));

        // Register View and Action as nominal types so they resolve in type position.
        // This allows `fn makeBody() -> View` to type-check correctly.
        let view_placeholder = TypeDef {
            kind: TypeKind::Class, name: "View".into(), type_params: vec![],
            fields: vec![], methods: vec![
                FnInfo { name: "body".into(),   type_params: vec![], params: vec![], return_ty: view_ret.clone(), span: Span::dummy() },
                FnInfo { name: "render".into(), type_params: vec![], params: vec![], return_ty: None, span: Span::dummy() },
            ],
            superclass: None, span: Span::dummy(),
        };
        self.types.insert("View".into(), view_placeholder);

        // View protocol
        let view_proto = ProtocolInfo {
            name: "View".into(),
            methods: vec![
                FnInfo { name: "body".into(),   type_params: vec![], params: vec![], return_ty: view_ret.clone(), span: Span::dummy() },
                FnInfo { name: "render".into(), type_params: vec![], params: vec![], return_ty: None, span: Span::dummy() },
            ],
            default_methods: HashMap::new(),
            span: Span::dummy(),
        };
        self.protocols.insert("View".into(), view_proto);

        // Action protocol
        let action_proto = ProtocolInfo {
            name: "Action".into(),
            methods: vec![
                FnInfo { name: "invoke".into(), type_params: vec![], params: vec![], return_ty: None, span: Span::dummy() },
            ],
            default_methods: HashMap::new(),
            span: Span::dummy(),
        };
        self.protocols.insert("Action".into(), action_proto);

        // Text(content: string)
        let text_def = TypeDef {
            kind: TypeKind::Class, name: "Text".into(), type_params: vec![],
            fields: vec![
                FieldInfo { name: "content".into(), ty: str_ui.clone(), mutability: Mut::Const, is_weak: false, span: Span::dummy() },
            ],
            methods: vec![
                FnInfo { name: "body".into(),   type_params: vec![], params: vec![], return_ty: view_ret.clone(), span: Span::dummy() },
                FnInfo { name: "render".into(), type_params: vec![], params: vec![], return_ty: None, span: Span::dummy() },
            ],
            superclass: Some("View".into()), span: Span::dummy(),
        };
        self.types.insert("Text".into(), text_def);
        self.functions.insert("Text".into(), FnInfo {
            name: "Text".into(), type_params: vec![],
            params: vec![Param { name: Ident::new("content", Span::dummy()), ty: str_ui.clone(), span: Span::dummy() }],
            return_ty: Some(ReturnTy::Single(Ty { kind: TyKind::Named(Ident::new("Text", Span::dummy())), span: Span::dummy() })),
            span: Span::dummy(),
        });

        // Button(label: string, onTap: fn() -> void)
        let button_def = TypeDef {
            kind: TypeKind::Class, name: "Button".into(), type_params: vec![],
            fields: vec![
                FieldInfo { name: "label".into(), ty: str_ui.clone(), mutability: Mut::Const, is_weak: false, span: Span::dummy() },
                FieldInfo { name: "onTap".into(), ty: void_fn.clone(), mutability: Mut::Const, is_weak: false, span: Span::dummy() },
            ],
            methods: vec![
                FnInfo { name: "body".into(),   type_params: vec![], params: vec![], return_ty: view_ret.clone(), span: Span::dummy() },
                FnInfo { name: "render".into(), type_params: vec![], params: vec![], return_ty: None, span: Span::dummy() },
            ],
            superclass: Some("View".into()), span: Span::dummy(),
        };
        self.types.insert("Button".into(), button_def);
        self.functions.insert("Button".into(), FnInfo {
            name: "Button".into(), type_params: vec![],
            params: vec![
                Param { name: Ident::new("label",  Span::dummy()), ty: str_ui.clone(),   span: Span::dummy() },
                Param { name: Ident::new("onTap",  Span::dummy()), ty: void_fn.clone(),  span: Span::dummy() },
            ],
            return_ty: Some(ReturnTy::Single(Ty { kind: TyKind::Named(Ident::new("Button", Span::dummy())), span: Span::dummy() })),
            span: Span::dummy(),
        });

        // VStack(children: Array<View>)
        let vstack_def = TypeDef {
            kind: TypeKind::Class, name: "VStack".into(), type_params: vec![],
            fields: vec![
                FieldInfo { name: "children".into(), ty: arr_view.clone(), mutability: Mut::Const, is_weak: false, span: Span::dummy() },
            ],
            methods: vec![
                FnInfo { name: "body".into(),   type_params: vec![], params: vec![], return_ty: view_ret.clone(), span: Span::dummy() },
                FnInfo { name: "render".into(), type_params: vec![], params: vec![], return_ty: None, span: Span::dummy() },
            ],
            superclass: Some("View".into()), span: Span::dummy(),
        };
        self.types.insert("VStack".into(), vstack_def);
        self.functions.insert("VStack".into(), FnInfo {
            name: "VStack".into(), type_params: vec![],
            params: vec![Param { name: Ident::new("children", Span::dummy()), ty: arr_view.clone(), span: Span::dummy() }],
            return_ty: Some(ReturnTy::Single(Ty { kind: TyKind::Named(Ident::new("VStack", Span::dummy())), span: Span::dummy() })),
            span: Span::dummy(),
        });

        // HStack(children: Array<View>)
        let hstack_def = TypeDef {
            kind: TypeKind::Class, name: "HStack".into(), type_params: vec![],
            fields: vec![
                FieldInfo { name: "children".into(), ty: arr_view.clone(), mutability: Mut::Const, is_weak: false, span: Span::dummy() },
            ],
            methods: vec![
                FnInfo { name: "body".into(),   type_params: vec![], params: vec![], return_ty: view_ret.clone(), span: Span::dummy() },
                FnInfo { name: "render".into(), type_params: vec![], params: vec![], return_ty: None, span: Span::dummy() },
            ],
            superclass: Some("View".into()), span: Span::dummy(),
        };
        self.types.insert("HStack".into(), hstack_def);
        self.functions.insert("HStack".into(), FnInfo {
            name: "HStack".into(), type_params: vec![],
            params: vec![Param { name: Ident::new("children", Span::dummy()), ty: arr_view.clone(), span: Span::dummy() }],
            return_ty: Some(ReturnTy::Single(Ty { kind: TyKind::Named(Ident::new("HStack", Span::dummy())), span: Span::dummy() })),
            span: Span::dummy(),
        });

        // Spacer()
        let spacer_def = TypeDef {
            kind: TypeKind::Class, name: "Spacer".into(), type_params: vec![],
            fields: vec![],
            methods: vec![
                FnInfo { name: "body".into(),   type_params: vec![], params: vec![], return_ty: view_ret.clone(), span: Span::dummy() },
                FnInfo { name: "render".into(), type_params: vec![], params: vec![], return_ty: None, span: Span::dummy() },
            ],
            superclass: Some("View".into()), span: Span::dummy(),
        };
        self.types.insert("Spacer".into(), spacer_def);
        self.functions.insert("Spacer".into(), FnInfo {
            name: "Spacer".into(), type_params: vec![], params: vec![],
            return_ty: Some(ReturnTy::Single(Ty { kind: TyKind::Named(Ident::new("Spacer", Span::dummy())), span: Span::dummy() })),
            span: Span::dummy(),
        });

        // TextField(value: string, onChange: fn(string) -> void)
        let tf_def = TypeDef {
            kind: TypeKind::Class, name: "TextField".into(), type_params: vec![],
            fields: vec![
                FieldInfo { name: "value".into(),    ty: str_ui.clone(),  mutability: Mut::Const, is_weak: false, span: Span::dummy() },
                FieldInfo { name: "onChange".into(), ty: str_fn.clone(),  mutability: Mut::Const, is_weak: false, span: Span::dummy() },
            ],
            methods: vec![
                FnInfo { name: "body".into(),   type_params: vec![], params: vec![], return_ty: view_ret.clone(), span: Span::dummy() },
                FnInfo { name: "render".into(), type_params: vec![], params: vec![], return_ty: None, span: Span::dummy() },
            ],
            superclass: Some("View".into()), span: Span::dummy(),
        };
        self.types.insert("TextField".into(), tf_def);
        self.functions.insert("TextField".into(), FnInfo {
            name: "TextField".into(), type_params: vec![],
            params: vec![
                Param { name: Ident::new("value",    Span::dummy()), ty: str_ui.clone(), span: Span::dummy() },
                Param { name: Ident::new("onChange", Span::dummy()), ty: str_fn.clone(), span: Span::dummy() },
            ],
            return_ty: Some(ReturnTy::Single(Ty { kind: TyKind::Named(Ident::new("TextField", Span::dummy())), span: Span::dummy() })),
            span: Span::dummy(),
        });

        // App(title: string, root: View) — root is any class implementing View.
        // The codegen resolves the concrete body method at construction time.
        let app_def = TypeDef {
            kind: TypeKind::Class, name: "App".into(), type_params: vec![],
            fields: vec![],
            methods: vec![
                FnInfo { name: "run".into(), type_params: vec![], params: vec![], return_ty: None, span: Span::dummy() },
            ],
            superclass: None, span: Span::dummy(),
        };
        self.types.insert("App".into(), app_def);
        self.functions.insert("App".into(), FnInfo {
            name: "App".into(), type_params: vec![],
            params: vec![
                Param { name: Ident::new("title", Span::dummy()), ty: str_ui.clone(),  span: Span::dummy() },
                Param { name: Ident::new("root",  Span::dummy()), ty: view_ty.clone(), span: Span::dummy() },
            ],
            return_ty: Some(ReturnTy::Single(Ty { kind: TyKind::Named(Ident::new("App", Span::dummy())), span: Span::dummy() })),
            span: Span::dummy(),
        });

        // Suppress unused variable warnings
        let _ = bool_ty;

        self.functions.insert("jsonBool".into(), FnInfo {
            name: "jsonBool".into(), type_params: vec![],
            params: vec![Param { name: Ident::new("v", Span::dummy()), ty: Ty { kind: TyKind::Named(Ident::new("bool", Span::dummy())), span: Span::dummy() }, span: Span::dummy() }],
            return_ty: Some(ReturnTy::Single(str_ty2.clone())), span: Span::dummy(),
        });

        // ── Phase 4: renderTemplate + serveFile ──────────────────────────────

        let map_str_str = Ty {
            kind: TyKind::Generic(
                Ident::new("Map", Span::dummy()),
                vec![
                    Ty { kind: TyKind::Named(Ident::new("string", Span::dummy())), span: Span::dummy() },
                    Ty { kind: TyKind::Named(Ident::new("string", Span::dummy())), span: Span::dummy() },
                ],
            ),
            span: Span::dummy(),
        };
        let err_ty = Ty { kind: TyKind::Optional(Box::new(
            Ty { kind: TyKind::Named(Ident::new("Error", Span::dummy())), span: Span::dummy() }
        )), span: Span::dummy() };
        let resp_ty2 = Ty { kind: TyKind::Named(Ident::new("HttpResponse", Span::dummy())), span: Span::dummy() };
        let str_ty4  = Ty { kind: TyKind::Named(Ident::new("string", Span::dummy())), span: Span::dummy() };

        // renderTemplate(path, data) -> (string, Error?)
        self.functions.insert("renderTemplate".into(), FnInfo {
            name: "renderTemplate".into(), type_params: vec![],
            params: vec![
                Param { name: Ident::new("path", Span::dummy()), ty: str_ty4.clone(), span: Span::dummy() },
                Param { name: Ident::new("data", Span::dummy()), ty: map_str_str.clone(), span: Span::dummy() },
            ],
            return_ty: Some(ReturnTy::Tuple(vec![str_ty4.clone(), err_ty.clone()])),
            span: Span::dummy(),
        });

        // serveFile(path) -> HttpResponse
        self.functions.insert("serveFile".into(), FnInfo {
            name: "serveFile".into(), type_params: vec![],
            params: vec![Param { name: Ident::new("path", Span::dummy()), ty: str_ty4.clone(), span: Span::dummy() }],
            return_ty: Some(ReturnTy::Single(resp_ty2.clone())),
            span: Span::dummy(),
        });

        // ── Phase 5: jsonDecode ──────────────────────────────────────────────

        // jsonDecode(s) -> (Map<string,string>, Error?)
        self.functions.insert("jsonDecode".into(), FnInfo {
            name: "jsonDecode".into(), type_params: vec![],
            params: vec![Param { name: Ident::new("s", Span::dummy()), ty: str_ty4.clone(), span: Span::dummy() }],
            return_ty: Some(ReturnTy::Tuple(vec![map_str_str.clone(), err_ty.clone()])),
            span: Span::dummy(),
        });

        // jsonDecodeGet(s, key) -> string  (convenience — gets a key from a decoded object)
        self.functions.insert("jsonDecodeGet".into(), FnInfo {
            name: "jsonDecodeGet".into(), type_params: vec![],
            params: vec![
                Param { name: Ident::new("s",   Span::dummy()), ty: str_ty4.clone(), span: Span::dummy() },
                Param { name: Ident::new("key", Span::dummy()), ty: str_ty4.clone(), span: Span::dummy() },
            ],
            return_ty: Some(ReturnTy::Single(str_ty4.clone())),
            span: Span::dummy(),
        });        let thread_def = TypeDef {
            kind: TypeKind::Class,
            name: "Thread".into(),
            type_params: vec![],
            fields: vec![],
            methods: vec![
                FnInfo {
                    name: "join".into(),
                    type_params: vec![],
                    params: vec![],
                    return_ty: None, // void
                    span: Span::dummy(),
                },
            ],
            superclass: None,
            span: Span::dummy(),
        };
        self.types.insert("Thread".into(), thread_def);

        // `thread_spawn(fn: fn() -> void) -> Thread` — free function
        let void_fn_ty = Ty {
            kind: TyKind::Fn(vec![], None),
            span: Span::dummy(),
        };
        self.functions.insert("thread_spawn".into(), FnInfo {
            name: "thread_spawn".into(),
            type_params: vec![],
            params: vec![Param {
                name: Ident::new("f", Span::dummy()),
                ty: void_fn_ty,
                span: Span::dummy(),
            }],
            return_ty: Some(ReturnTy::Single(Ty {
                kind: TyKind::Named(Ident::new("Thread", Span::dummy())),
                span: Span::dummy(),
            })),
            span: Span::dummy(),
        });

        // `Task<T>` — result of `async fn(args)`. ARC reference type.
        // .await() blocks the current thread and returns T.
        let t_ty = Ty { kind: TyKind::Named(Ident::new("T", Span::dummy())), span: Span::dummy() };
        let task_def = TypeDef {
            kind: TypeKind::Class,
            name: "Task".into(),
            type_params: vec!["T".into()],
            fields: vec![],
            methods: vec![
                FnInfo {
                    name: "await".into(),
                    type_params: vec![],
                    params: vec![],
                    return_ty: Some(ReturnTy::Single(t_ty.clone())),
                    span: Span::dummy(),
                },
            ],
            superclass: None,
            span: Span::dummy(),
        };
        self.types.insert("Task".into(), task_def);

        // `Mutex<T>` — wraps any value for safe shared mutable access.
        // .lock() returns a MutexGuard<T>; released on scope exit via ARC.
        let mutex_guard_ty = Ty {
            kind: TyKind::Generic(
                Ident::new("MutexGuard", Span::dummy()),
                vec![t_ty.clone()],
            ),
            span: Span::dummy(),
        };
        let mutex_def = TypeDef {
            kind: TypeKind::Class,
            name: "Mutex".into(),
            type_params: vec!["T".into()],
            fields: vec![],
            methods: vec![
                FnInfo {
                    name: "lock".into(),
                    type_params: vec![],
                    params: vec![],
                    return_ty: Some(ReturnTy::Single(mutex_guard_ty)),
                    span: Span::dummy(),
                },
            ],
            superclass: None,
            span: Span::dummy(),
        };
        self.types.insert("Mutex".into(), mutex_def);

        // `MutexGuard<T>` — held while mutex is locked. Has `.value: T`.
        let mutex_guard_def = TypeDef {
            kind: TypeKind::Class,
            name: "MutexGuard".into(),
            type_params: vec!["T".into()],
            fields: vec![
                FieldInfo {
                    name: "value".into(),
                    ty: t_ty,
                    mutability: Mut::Let,
                    is_weak: false,
                    span: Span::dummy(),
                },
            ],
            methods: vec![],
            superclass: None,
            span: Span::dummy(),
        };
        self.types.insert("MutexGuard".into(), mutex_guard_def);
    }

    // ── Collection pass ───────────────────────────────────────────────────

    /// Collect all top-level declarations from the parsed source file.
    pub fn collect(&mut self, file: &SourceFile) -> TypeResult<()> {
        for item in &file.items {
            self.collect_item(item)?;
        }
        Ok(())
    }

    fn collect_item(&mut self, item: &Item) -> TypeResult<()> {
        match &item.kind {
            // Imports are resolved by the compiler driver before typechecking.
            // By the time collect_item runs, imported symbols are already merged
            // into this symbol table. Nothing to do here.
            ItemKind::Import { .. } => Ok(()),
            ItemKind::Struct(s)   => self.collect_struct(s),
            ItemKind::Class(c)    => self.collect_class(c),
            ItemKind::Enum(e)     => self.collect_enum(e),
            ItemKind::Protocol(p) => self.collect_protocol(p),
            ItemKind::Impl(i)     => self.collect_impl(i),
            ItemKind::Fn(f)       => self.collect_fn(f),
        }
    }

    fn collect_enum(&mut self, e: &EnumDef) -> TypeResult<()> {
        // Register enum in enum_defs so the typechecker can look up variants.
        self.enum_defs.insert(e.name.name.clone(), e.clone());
        // Also register as a TypeDef so the type can appear in signatures.
        // Variants are NOT registered as separate types — they're looked up via enum_defs.
        self.types.insert(e.name.name.clone(), TypeDef {
            kind: TypeKind::Class, // heap-allocated tagged union
            name: e.name.name.clone(),
            type_params: e.type_params.iter().map(|p| p.name.name.clone()).collect(),
            fields: vec![],
            methods: vec![],
            superclass: None,
            span: e.span,
        });
        Ok(())
    }

    fn collect_struct(&mut self, s: &StructDef) -> TypeResult<()> {
        let name = &s.name.name;
        // User definitions always win — they override any pre-registered builtin.
        self.types.insert(name.clone(), TypeDef {
            kind: TypeKind::Struct,
            name: name.clone(),
            type_params: s.type_params.iter().map(|p| p.name.name.clone()).collect(),
            fields: s.fields.iter().map(|f| FieldInfo {
                name: f.name.name.clone(),
                ty: f.ty.clone(),
                mutability: f.mutability,
                is_weak: f.is_weak,
                span: f.span,
            }).collect(),
            methods: s.methods.iter().map(FnInfo::from_fn_def).collect(),
            superclass: None,
            span: s.span,
        });
        Ok(())
    }

    fn collect_class(&mut self, c: &ClassDef) -> TypeResult<()> {
        let name = &c.name.name;
        // User definitions always win — they override any pre-registered builtin.
        self.types.insert(name.clone(), TypeDef {
            kind: TypeKind::Class,
            name: name.clone(),
            type_params: c.type_params.iter().map(|p| p.name.name.clone()).collect(),
            fields: c.fields.iter().map(|f| FieldInfo {
                name: f.name.name.clone(),
                ty: f.ty.clone(),
                mutability: f.mutability,
                is_weak: f.is_weak,
                span: f.span,
            }).collect(),
            methods: c.methods.iter().map(FnInfo::from_fn_def).collect(),
            superclass: c.superclass.as_ref().map(|s| s.name.clone()),
            span: c.span,
        });
        Ok(())
    }

    fn collect_protocol(&mut self, p: &ProtocolDef) -> TypeResult<()> {
        let default_methods: HashMap<String, FnDef> = p.default_methods
            .iter()
            .map(|f| (f.name.name.clone(), f.clone()))
            .collect();
        // Default methods have both a signature AND a body. Include their signatures
        // in the `methods` list so `lookup_method` can resolve them for call sites.
        let default_sigs: Vec<FnInfo> = p.default_methods
            .iter()
            .map(|f| FnInfo {
                name: f.name.name.clone(),
                type_params: f.type_params.clone(),
                params: f.params.iter().map(|p| p.clone()).collect(),
                return_ty: f.return_ty.clone(),
                span: f.span,
            })
            .collect();
        let mut all_methods: Vec<FnInfo> = p.methods.iter().map(FnInfo::from_fn_sig).collect();
        all_methods.extend(default_sigs);
        self.protocols.insert(p.name.name.clone(), ProtocolInfo {
            name: p.name.name.clone(),
            methods: all_methods,
            default_methods,
            span: p.span,
        });
        Ok(())
    }

    fn collect_impl(&mut self, i: &ImplBlock) -> TypeResult<()> {
        let proto = &i.protocol.name;
        let target = &i.target.name;

        let entry = self.impls.entry(target.clone()).or_default();
        if entry.contains_key(proto) {
            return Err(TypeError::DuplicateImpl {
                protocol: proto.clone(),
                ty: target.clone(),
                span: i.span,
            });
        }
        entry.insert(proto.clone(), ImplRecord {
            protocol: proto.clone(),
            target: target.clone(),
            methods: i.methods.iter().map(FnInfo::from_fn_def).collect(),
            span: i.span,
        });
        Ok(())
    }

    fn collect_fn(&mut self, f: &FnDef) -> TypeResult<()> {
        self.functions.insert(f.name.name.clone(), FnInfo::from_fn_def(f));
        Ok(())
    }

    // ── Lookup helpers ────────────────────────────────────────────────────

    /// Look up a type by name. Checks user types only (not primitives).
    /// Primitives are handled directly in the inference pass.
    pub fn lookup_type(&self, name: &str) -> Option<&TypeDef> {
        self.types.get(name)
    }

    /// Look up a top-level function.
    pub fn lookup_fn(&self, name: &str) -> Option<&FnInfo> {
        self.functions.get(name)
    }

    /// Look up a protocol.
    pub fn lookup_protocol(&self, name: &str) -> Option<&ProtocolInfo> {
        self.protocols.get(name)
    }

    /// Look up a function in a specific module (for qualified access `alias.fn`).
    pub fn lookup_module_fn(&self, alias: &str, name: &str) -> Option<&FnInfo> {
        let m = self.modules.get(alias)?;
        // Try both the raw name and the prefixed name (alias__name).
        m.functions.get(name)
            .or_else(|| m.functions.get(&format!("{alias}__{name}")))
    }

    pub fn lookup_module_type(&self, alias: &str, name: &str) -> Option<&TypeDef> {
        let m = self.modules.get(alias)?;
        m.types.get(name)
            .or_else(|| m.types.get(&format!("{alias}__{name}")))
    }

    pub fn lookup_module_enum(&self, alias: &str, name: &str) -> Option<&EnumDef> {
        let m = self.modules.get(alias)?;
        m.enum_defs.get(name)
            .or_else(|| m.enum_defs.get(&format!("{alias}__{name}")))
    }

    pub fn is_module(&self, alias: &str) -> bool {
        self.modules.contains_key(alias)
    }

    /// Look up a field on a named type (does not resolve generic type params).
    pub fn lookup_field<'a>(&'a self, ty_name: &str, field: &str) -> Option<&'a FieldInfo> {
        let mut current = ty_name;
        loop {
            let typedef = self.types.get(current)?;
            if let Some(f) = typedef.fields.iter().find(|f| f.name == field) {
                return Some(f);
            }
            // Walk superclass chain
            match &typedef.superclass {
                Some(super_name) => current = super_name,
                None => return None,
            }
        }
    }

    /// Look up a method: first check methods defined on the type, then
    /// walk impl records, then check the superclass chain.
    /// For type parameters (e.g. `T`), check protocol bounds.
    pub fn lookup_method<'a>(&'a self, ty_name: &str, method: &str) -> Option<&'a FnInfo> {
        // Direct methods on the type definition
        if let Some(typedef) = self.types.get(ty_name) {
            if let Some(m) = typedef.methods.iter().find(|m| m.name == method) {
                return Some(m);
            }
        }

        // Methods provided via impl blocks
        if let Some(proto_impls) = self.impls.get(ty_name) {
            for impl_record in proto_impls.values() {
                if let Some(m) = impl_record.methods.iter().find(|m| m.name == method) {
                    return Some(m);
                }
            }
        }

        // Protocol default methods — the type implements the protocol so the
        // default is available even if not in the impl block.
        if let Some(proto_impls) = self.impls.get(ty_name) {
            for proto_name in proto_impls.keys() {
                if let Some(proto) = self.protocols.get(proto_name) {
                    // Default method signatures are included in proto.methods
                    if let Some(m) = proto.methods.iter().find(|m| m.name == method) {
                        return Some(m);
                    }
                }
            }
        }

        // Type parameter: look up the method in any protocol bounds.
        // e.g. `T: Comparable` → `compareTo` is available on `T`.
        if let Some(bounds) = self.type_param_bounds.get(ty_name) {
            for proto_name in bounds {
                if let Some(proto) = self.protocols.get(proto_name) {
                    if let Some(m) = proto.methods.iter().find(|m| m.name == method) {
                        return Some(m);
                    }
                }
            }
        }

        // Superclass chain
        if let Some(typedef) = self.types.get(ty_name) {
            if let Some(ref super_name) = typedef.superclass {
                return self.lookup_method(super_name, method);
            }
        }

        None
    }

    /// Does type `ty_name` conform to protocol `proto_name`?
    pub fn conforms_to(&self, ty_name: &str, proto_name: &str) -> bool {
        // Check direct impl
        if let Some(type_impls) = self.impls.get(ty_name) {
            if type_impls.contains_key(proto_name) {
                return true;
            }
        }

        // Check superclass chain
        if let Some(typedef) = self.types.get(ty_name) {
            if let Some(ref super_name) = typedef.superclass {
                return self.conforms_to(super_name, proto_name);
            }
        }

        false
    }

    /// Is `ty_name` a class (reference type)?
    pub fn is_class(&self, ty_name: &str) -> bool {
        self.types.get(ty_name).map_or(false, |t| t.kind == TypeKind::Class)
    }

    /// Is `ty_name` a struct (value type)?
    pub fn is_struct(&self, ty_name: &str) -> bool {
        self.types.get(ty_name).map_or(false, |t| t.kind == TypeKind::Struct)
    }

    /// Resolve a syntactic `Ty` to a `SemTy`, given a map of in-scope
    /// type parameters (e.g. `{"T" => SemTy::Named("int")}`).
    pub fn resolve_ty(
        &self,
        ty: &Ty,
        type_args: &HashMap<String, SemTy>,
    ) -> TypeResult<SemTy> {
        match &ty.kind {
            TyKind::Named(ident) => {
                let name = &ident.name;
                // Type parameter substitution
                if let Some(sub) = type_args.get(name) {
                    return Ok(sub.clone());
                }
                // Primitive types
                match name.as_str() {
                    "int"    => Ok(SemTy::Int),
                    "f64"    => Ok(SemTy::Float),
                    "float"  => Ok(SemTy::Float),
                    "bool"   => Ok(SemTy::Bool),
                    "string" => Ok(SemTy::String),
                    "Self"   => Ok(SemTy::Named("Self".into())),
                    "void"   => Ok(SemTy::Void),
                    _ => {
                        if self.types.contains_key(name.as_str()) {
                            Ok(SemTy::Named(name.clone()))
                        } else {
                            Err(TypeError::UnknownType { name: name.clone(), span: ty.span })
                        }
                    }
                }
            }
            TyKind::Generic(ident, args) => {
                let name = &ident.name;
                let resolved_args: TypeResult<Vec<SemTy>> = args.iter()
                    .map(|a| self.resolve_ty(a, type_args))
                    .collect();
                Ok(SemTy::Generic(name.clone(), resolved_args?))
            }
            TyKind::Optional(inner) => {
                Ok(SemTy::Optional(Box::new(self.resolve_ty(inner, type_args)?)))
            }
            TyKind::Tuple(tys) => {
                let resolved: TypeResult<Vec<SemTy>> = tys.iter()
                    .map(|t| self.resolve_ty(t, type_args))
                    .collect();
                Ok(SemTy::Tuple(resolved?))
            }
            TyKind::Fn(params, ret) => {
                let resolved_params: TypeResult<Vec<SemTy>> = params.iter()
                    .map(|p| self.resolve_ty(p, type_args))
                    .collect();
                let resolved_ret = match ret {
                    Some(r) => self.resolve_ty(r, type_args)?,
                    None    => SemTy::Void,
                };
                Ok(SemTy::Fn(resolved_params?, Box::new(resolved_ret)))
            }
        }
    }

    /// Resolve a `ReturnTy` to a `SemTy`.
    pub fn resolve_return_ty(
        &self,
        ret: &Option<ReturnTy>,
        type_args: &HashMap<String, SemTy>,
    ) -> TypeResult<SemTy> {
        match ret {
            None => Ok(SemTy::Void),
            Some(ReturnTy::Single(ty)) => self.resolve_ty(ty, type_args),
            Some(ReturnTy::Tuple(tys)) => {
                let resolved: TypeResult<Vec<SemTy>> = tys.iter()
                    .map(|t| self.resolve_ty(t, type_args))
                    .collect();
                Ok(SemTy::Tuple(resolved?))
            }
        }
    }
}
