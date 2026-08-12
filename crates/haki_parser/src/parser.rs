/// parser.rs — Full recursive descent parser for Haki v0.1.
///
/// Grammar summary (informal, in priority order):
///
///   file        ::= item*
///   item        ::= struct_def | class_def | protocol_def | impl_block | fn_def
///
///   struct_def  ::= 'struct' IDENT type_params? '{' (field_def | fn_def)* '}'
///   class_def   ::= 'class' IDENT type_params? ('extends' IDENT)? '{' (field_def | fn_def)* '}'
///   protocol    ::= 'protocol' IDENT '{' fn_sig* '}'
///   impl_block  ::= 'impl' IDENT 'for' IDENT type_params? '{' fn_def* '}'
///   fn_def      ::= 'fn' IDENT type_params? '(' params ')' ('->' return_ty)? block
///
///   field_def   ::= ('weak'? ('const'|'let') IDENT ':' ty)
///
///   type_params ::= '<' type_param (',' type_param)* '>'
///   type_param  ::= IDENT (':' IDENT ('&' IDENT)*)?
///
///   params      ::= (param (',' param)*)?
///   param       ::= IDENT ':' ty
///
///   return_ty   ::= ty | '(' ty (',' ty)* ')'
///
///   ty          ::= named_ty '?'?
///   named_ty    ::= IDENT ('<' ty (',' ty)* '>')? | '(' ty (',' ty)* ')'
///
///   block       ::= '{' stmt* '}'
///
///   stmt        ::= let_stmt
///                 | return_stmt
///                 | yield_stmt
///                 | if_stmt
///                 | while_stmt
///                 | for_stmt
///                 | match_stmt
///                 | panic_stmt
///                 | expr_stmt
///
///   Expressions use Pratt precedence climbing.

use haki_ast::*;
use haki_lexer::{Token, TokenKind};
use crate::{ParseError, token_description};

// ── Precedence levels (Pratt) ─────────────────────────────────────────────────

/// Binding power for binary operators.
/// Returns `(left_bp, right_bp)` for left-associative, or `None` if not a
/// binary infix operator at this token.
fn infix_bp(kind: &TokenKind) -> Option<(u8, u8)> {
    match kind {
        // Ternary gets the lowest bp (1,2) — parsed inline in parse_expr
        TokenKind::Question                  => Some((1, 2)),
        TokenKind::OrOr                      => Some((3, 4)),
        TokenKind::AndAnd                    => Some((5, 6)),
        TokenKind::EqEq | TokenKind::BangEq => Some((7, 8)),
        TokenKind::Lt | TokenKind::Gt |
        TokenKind::LtEq | TokenKind::GtEq   => Some((9, 10)),
        TokenKind::Plus | TokenKind::Minus   => Some((11, 12)),
        TokenKind::Star | TokenKind::Slash |
        TokenKind::Percent                   => Some((13, 14)),
        _ => None,
    }
}

fn token_to_binop(kind: &TokenKind) -> Option<BinaryOp> {
    match kind {
        TokenKind::Plus    => Some(BinaryOp::Add),
        TokenKind::Minus   => Some(BinaryOp::Sub),
        TokenKind::Star    => Some(BinaryOp::Mul),
        TokenKind::Slash   => Some(BinaryOp::Div),
        TokenKind::Percent => Some(BinaryOp::Mod),
        TokenKind::EqEq    => Some(BinaryOp::Eq),
        TokenKind::BangEq  => Some(BinaryOp::Ne),
        TokenKind::Lt      => Some(BinaryOp::Lt),
        TokenKind::LtEq    => Some(BinaryOp::Le),
        TokenKind::Gt      => Some(BinaryOp::Gt),
        TokenKind::GtEq    => Some(BinaryOp::Ge),
        TokenKind::AndAnd  => Some(BinaryOp::And),
        TokenKind::OrOr    => Some(BinaryOp::Or),
        _ => None,
    }
}

// ── Parser struct ─────────────────────────────────────────────────────────────

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    // ── Cursor helpers ────────────────────────────────────────────────────

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn peek2_kind(&self) -> &TokenKind {
        let next = (self.pos + 1).min(self.tokens.len() - 1);
        &self.tokens[next].kind
    }

    fn current_span(&self) -> Span {
        self.peek().span
    }

    /// Advance past the current token and return it.
    fn advance(&mut self) -> &Token {
        let t = &self.tokens[self.pos];
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        t
    }

    /// Expect the current token to match `expected`, advance, and return its span.
    fn expect(&mut self, expected: &TokenKind) -> Result<Span, ParseError> {
        if self.peek_kind() == expected {
            Ok(self.advance().span)
        } else {
            Err(ParseError::expected(
                token_description(expected),
                self.peek_kind(),
                self.current_span(),
            ))
        }
    }

    /// Consume the current token if it matches; return whether it matched.
    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.peek_kind() == kind {
            self.advance();
            true
        } else {
            false
        }
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    // ── Identifier ────────────────────────────────────────────────────────

    fn expect_ident(&mut self) -> Result<Ident, ParseError> {
        let span = self.current_span();
        match self.peek_kind().clone() {
            TokenKind::Ident(name) => {
                self.advance();
                Ok(Ident::new(name, span))
            }
            // Allow `Self` as an identifier in protocol / impl contexts.
            _ => Err(ParseError::expected("identifier", self.peek_kind(), span)),
        }
    }

    /// Like `expect_ident` but also accepts `_` as a discard identifier.
    fn expect_ident_or_underscore(&mut self) -> Result<Ident, ParseError> {
        let span = self.current_span();
        match self.peek_kind().clone() {
            TokenKind::Ident(name) => {
                self.advance();
                Ok(Ident::new(name, span))
            }
            TokenKind::Under => {
                self.advance();
                Ok(Ident::new("_", span))
            }
            _ => Err(ParseError::expected("identifier or _", self.peek_kind(), span)),
        }
    }

    /// Like `expect_ident` but also accepts `Self` as a keyword-identifier.
    #[allow(dead_code)]
    fn expect_ident_or_self(&mut self) -> Result<Ident, ParseError> {
        let span = self.current_span();
        match self.peek_kind().clone() {
            TokenKind::Ident(name) => { self.advance(); Ok(Ident::new(name, span)) }
            // `Self` is used in protocol signatures — treat it as an identifier.
            _ => Err(ParseError::expected("identifier or `Self`", self.peek_kind(), span)),
        }
    }

    // ── Top-level file ────────────────────────────────────────────────────

    pub fn parse_file(&mut self) -> Result<SourceFile, ParseError> {
        let lo = self.current_span().lo;
        let mut items = Vec::new();
        while !self.at_eof() {
            items.push(self.parse_item()?);
        }
        let hi = self.current_span().hi;
        Ok(SourceFile { items, span: Span::new(lo, hi) })
    }

    /// Error-recovering file parse for the LSP.
    ///
    /// On error, records the error, calls `synchronize()` to skip to the next
    /// safe top-level keyword, and continues parsing. Always returns a (partial)
    /// SourceFile; invalid items are simply absent from the AST.
    pub fn parse_file_recovery(&mut self) -> crate::ParseResult {
        let lo = self.current_span().lo;
        let mut items  = Vec::new();
        let mut errors = Vec::new();

        while !self.at_eof() {
            match self.parse_item() {
                Ok(item)  => items.push(item),
                Err(e) => {
                    errors.push(e);
                    self.synchronize();
                }
            }
        }

        let hi = self.current_span().hi;
        crate::ParseResult {
            ast: SourceFile { items, span: Span::new(lo, hi) },
            errors,
        }
    }

    /// Advance past tokens until we reach a top-level synchronization point.
    ///
    /// Strategy: skip until brace depth is 0 AND the next token is a top-level
    /// keyword (`fn`, `struct`, `class`, `enum`, `protocol`, `impl`, `import`).
    /// This correctly handles errors inside function bodies — we skip the rest of
    /// the body before looking for the next top-level item.
    fn synchronize(&mut self) {
        let mut depth: i32 = 0;
        loop {
            match self.peek_kind() {
                TokenKind::Eof => return,
                TokenKind::LBrace => {
                    depth += 1;
                    self.advance();
                }
                TokenKind::RBrace => {
                    if depth == 0 {
                        // Stray `}` at top level — consume and continue scanning
                        self.advance();
                    } else {
                        depth -= 1;
                        self.advance();
                        // After closing the last open brace, check if we're back
                        // at top level and the next token is a sync point
                        if depth == 0 {
                            match self.peek_kind() {
                                TokenKind::Fn
                                | TokenKind::Struct
                                | TokenKind::Class
                                | TokenKind::Enum
                                | TokenKind::Protocol
                | TokenKind::Annotation
                                | TokenKind::Impl
                                | TokenKind::Import
                                | TokenKind::Eof => return,
                                _ => {}
                            }
                        }
                    }
                }
                TokenKind::Fn
                | TokenKind::Struct
                | TokenKind::Class
                | TokenKind::Enum
                | TokenKind::Protocol
                | TokenKind::Impl
                | TokenKind::Import
                | TokenKind::Extern
                    if depth == 0 =>
                {
                    return;
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    // ── Items ─────────────────────────────────────────────────────────────

    fn parse_item(&mut self) -> Result<Item, ParseError> {
        let lo = self.current_span().lo;

        // Collect zero or more leading `@attr(args)` annotations.
        let attributes = self.parse_attributes()?;

        let kind = match self.peek_kind() {
            TokenKind::Import   => self.parse_import()?,
            TokenKind::Struct   => ItemKind::Struct(self.parse_struct_with_attrs(attributes)?),
            TokenKind::Class    => ItemKind::Class(self.parse_class()?),
            TokenKind::Enum     => ItemKind::Enum(self.parse_enum()?),
            TokenKind::Protocol => ItemKind::Protocol(self.parse_protocol()?),
            TokenKind::Impl     => ItemKind::Impl(self.parse_impl()?),
            TokenKind::Fn       => ItemKind::Fn(self.parse_fn_def_with_attrs(attributes)?),
            TokenKind::Extern   => ItemKind::ExternFn(self.parse_extern_fn_with_attrs(attributes)?),
            TokenKind::Annotation => ItemKind::AnnotationDef(self.parse_annotation_def()?),
            TokenKind::Const | TokenKind::Let => {
                // Top-level const/let declaration: `const VERSION = "4.0.0"`
                self.advance(); // consume `const` or `let`
                let name = self.expect_ident()?;
                let ty = if matches!(self.peek_kind(), TokenKind::Colon) {
                    self.advance(); // consume `:`
                    Some(self.parse_ty()?)
                } else {
                    None
                };
                self.expect(&TokenKind::Eq)?;
                let value = self.parse_expr(0)?;
                ItemKind::GlobalConst { name, ty, value }
            }
            TokenKind::Ident(ref s) if s == "type" => {
                // `type Alias = ExistingType` — transparent type alias
                self.advance(); // consume `type`
                let name = self.expect_ident()?;
                self.expect(&TokenKind::Eq)?;
                let ty = self.parse_ty()?;
                ItemKind::TypeAlias { name, ty, span: Span::new(lo, self.current_span().lo) }
            }
            _ => {
                if !attributes.is_empty() {
                    return Err(ParseError::expected(
                        "fn, struct, class, or extern after attribute",
                        self.peek_kind(), self.current_span()
                    ));
                }
                return Err(ParseError::unexpected(self.peek_kind(), self.current_span()))
            }
        };
        let hi = self.current_span().lo;
        Ok(Item { kind, span: Span::new(lo, hi) })
    }

    /// Parse zero or more `@name` or `@name("arg1", "arg2")` attribute declarations.
    fn parse_attributes(&mut self) -> Result<Vec<Attribute>, ParseError> {
        let mut attrs = Vec::new();
        while matches!(self.peek_kind(), TokenKind::At) {
            let lo = self.current_span().lo;
            self.advance(); // consume `@`

            // Attribute name: identifier or keyword (e.g. @timeout, @skip, @link).
            // Keywords are valid annotation names — extract their text.
            let name = match self.peek_kind().clone() {
                TokenKind::Ident(s) => { self.advance(); s }
                // Storage modifiers / control flow keywords used as annotation names:
                TokenKind::Timeout  => { self.advance(); "timeout".into() }
                TokenKind::Async    => { self.advance(); "async".into() }
                TokenKind::Await    => { self.advance(); "await".into() }
                TokenKind::Select   => { self.advance(); "select".into() }
                TokenKind::Const    => { self.advance(); "const".into() }
                TokenKind::Let      => { self.advance(); "let".into() }
                TokenKind::Weak     => { self.advance(); "weak".into() }
                // Declaration keywords that might be used as annotations:
                TokenKind::Extern   => { self.advance(); "extern".into() }
                _ => return Err(ParseError::expected("annotation name", self.peek_kind(), self.current_span())),
            };

            // Optional argument list: `("arg1", "arg2")`, `(ms: 100)`, `(100)`,
            // or expression args like `(x > 0)`.
            // Also accept bare string literal without parens: `@error "msg"`.
            let mut args = Vec::new();
            if matches!(self.peek_kind(), TokenKind::String(_)) {
                // `@name "string"` — bare string literal shorthand
                if let TokenKind::String(s) = self.peek_kind().clone() {
                    self.advance();
                    args.push(s);
                }
            } else if matches!(self.peek_kind(), TokenKind::LParen) {
                self.advance(); // consume `(`
                // Collect all tokens up to matching `)`, building each comma-separated
                // argument as a raw source string. This supports expressions like `x > 0`.
                let mut depth = 0i32;
                let mut current_arg = String::new();
                loop {
                    match self.peek_kind().clone() {
                        TokenKind::Eof => break,
                        TokenKind::RParen if depth == 0 => {
                            self.advance();
                            let trimmed = current_arg.trim().to_string();
                            if !trimmed.is_empty() { args.push(trimmed); }
                            break;
                        }
                        TokenKind::Comma if depth == 0 => {
                            self.advance();
                            let trimmed = current_arg.trim().to_string();
                            if !trimmed.is_empty() { args.push(trimmed); }
                            current_arg.clear();
                        }
                        tok => {
                            // Track nested parens depth
                            match &tok {
                                TokenKind::LParen => depth += 1,
                                TokenKind::RParen => depth -= 1,
                                _ => {}
                            }
                            // Convert token back to source text
                            let tok_text = match tok {
                                TokenKind::Ident(s) => s,
                                TokenKind::String(s) => format!("\"{}\"", s),
                                TokenKind::Int(n) => n.to_string(),
                                TokenKind::Float(f) => f.to_string(),
                                TokenKind::True => "true".into(),
                                TokenKind::False => "false".into(),
                                TokenKind::Plus => "+".into(),
                                TokenKind::Minus => "-".into(),
                                TokenKind::Star => "*".into(),
                                TokenKind::Slash => "/".into(),
                                TokenKind::Percent => "%".into(),
                                TokenKind::Lt => "<".into(),
                                TokenKind::Gt => ">".into(),
                                TokenKind::LtEq => "<=".into(),
                                TokenKind::GtEq => ">=".into(),
                                TokenKind::EqEq => "==".into(),
                                TokenKind::BangEq => "!=".into(),
                                TokenKind::Amp => "&".into(),
                                TokenKind::AndAnd => "&&".into(),
                                TokenKind::OrOr => "||".into(),
                                TokenKind::Bang => "!".into(),
                                TokenKind::Dot => ".".into(),
                                TokenKind::LParen => "(".into(),
                                TokenKind::RParen => ")".into(),
                                TokenKind::Colon => ": ".into(),
                                TokenKind::Null => "NULL".into(),
                                _ => String::new(), // skip unrecognised tokens
                            };
                            if !current_arg.is_empty() && !tok_text.starts_with(':') {
                                current_arg.push(' ');
                            }
                            current_arg.push_str(&tok_text);
                            self.advance();
                        }
                    }
                }
            }

            let hi = self.current_span().lo;
            attrs.push(Attribute { name, args, span: Span::new(lo, hi) });
        }
        Ok(attrs)
    }

    /// Wrapper: parse a `fn` definition and attach pre-parsed attributes.
    fn parse_fn_def_with_attrs(&mut self, attributes: Vec<Attribute>) -> Result<FnDef, ParseError> {
        let mut f = self.parse_fn_def()?;
        f.attributes = attributes;
        Ok(f)
    }

    /// Wrapper: parse a `struct` definition and attach pre-parsed attributes.
    fn parse_struct_with_attrs(&mut self, attributes: Vec<Attribute>) -> Result<StructDef, ParseError> {
        let mut s = self.parse_struct()?;
        s.attributes = attributes;
        Ok(s)
    }

    /// Wrapper: parse an `extern` declaration and attach pre-parsed attributes.
    fn parse_extern_fn_with_attrs(&mut self, attributes: Vec<Attribute>) -> Result<ExternFnDef, ParseError> {
        let mut f = self.parse_extern_fn()?;
        f.attributes = attributes;
        Ok(f)
    }

    /// Desugar `try f()` into a block expression:
    ///
    /// ```haki
    /// {
    ///     const __val, __err = f()
    ///     if __err != null { return null, __err }
    ///     yield __val
    /// }
    /// ```
    ///
    /// This desugaring happens entirely in the parser — the typechecker and codegen
    /// see only existing constructs (multi-return let, null check, early return, yield).
    /// Therefore defer cleanup, ARC release, and scope cleanup all work for free.
    fn desugar_try(&self, call_expr: Expr, sp: Span) -> Expr {
        let d = Span::dummy();
        let mk = |kind: ExprKind| Expr { kind, span: d };
        let ident = |n: &str| Ident::new(n, d);

        // `const __val, __err = call_expr`
        let let_stmt = Stmt {
            kind: StmtKind::Let(LetStmt {
                mutability: Mut::Const,
                bindings: vec![
                    Binding::Name(ident("__try_val")),
                    Binding::Name(ident("__try_err")),
                ],
                ty: None,
                init: Box::new(call_expr),
                span: d,
            }),
            span: d,
        };

        // `if __err != null { return __val, __err }`
        // Using __val (zero value from the failed call) instead of literal null
        // avoids the null-assignability problem with concrete return types.
        let err_check = Stmt {
            kind: StmtKind::If(IfExpr {
                cond: Box::new(mk(ExprKind::Binary(
                    BinaryOp::Ne,
                    Box::new(mk(ExprKind::Ident(ident("__try_err")))),
                    Box::new(mk(ExprKind::Null)),
                ))),
                then_block: Block {
                    stmts: vec![Stmt {
                        kind: StmtKind::Return(ReturnStmt {
                            values: vec![
                                mk(ExprKind::Ident(ident("__try_val"))),
                                mk(ExprKind::Ident(ident("__try_err"))),
                            ],
                            span: d,
                        }),
                        span: d,
                    }],
                    span: d,
                },
                else_branch: None,
                span: d,
            }),
            span: d,
        };

        // `yield __val`
        let yield_stmt = Stmt {
            kind: StmtKind::Yield(Box::new(mk(ExprKind::Ident(ident("__try_val"))))),
            span: d,
        };

        Expr {
            kind: ExprKind::Block(Box::new(Block {
                stmts: vec![let_stmt, err_check, yield_stmt],
                span: d,
            })),
            span: sp,
        }
    }

    /// `enum Name<T> { Variant, Variant(Type), Variant(T, U) }`
    fn parse_enum(&mut self) -> Result<EnumDef, ParseError> {
        let lo = self.current_span().lo;
        self.expect(&TokenKind::Enum)?;
        let name = self.expect_ident()?;
        let type_params = self.parse_type_params()?;
        self.expect(&TokenKind::LBrace)?;

        let mut variants = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
            let vlo = self.current_span().lo;
            let vname = self.expect_ident()?;
            // Optional payload: `Variant(T, U, ...)`
            let fields = if matches!(self.peek_kind(), TokenKind::LParen) {
                self.advance(); // consume `(`
                let mut tys = Vec::new();
                while !matches!(self.peek_kind(), TokenKind::RParen | TokenKind::Eof) {
                    tys.push(self.parse_ty()?);
                    if !matches!(self.peek_kind(), TokenKind::RParen) {
                        self.expect(&TokenKind::Comma)?;
                    }
                }
                self.expect(&TokenKind::RParen)?;
                tys
            } else {
                vec![]
            };
            // Optional comma between variants
            self.eat(&TokenKind::Comma);
            let vhi = self.current_span().lo;
            variants.push(EnumVariant { name: vname, fields, span: Span::new(vlo, vhi) });
        }
        let hi = self.current_span().hi;
        self.expect(&TokenKind::RBrace)?;
        Ok(EnumDef { name, type_params, variants, span: Span::new(lo, hi) })
    }

    /// `extern "js" fn name(params) -> RetTy`
    /// Parses an extern function declaration with no body.
    fn parse_extern_fn(&mut self) -> Result<ExternFnDef, ParseError> {
        let lo = self.current_span().lo;
        self.expect(&TokenKind::Extern)?;

        // ABI string: currently only "js" is supported
        let abi = match self.peek_kind().clone() {
            TokenKind::String(s) => { self.advance(); s }
            _ => return Err(ParseError::expected(r#""js""#, self.peek_kind(), self.current_span())),
        };

        self.expect(&TokenKind::Fn)?;
        let name = self.expect_ident()?;

        // Parameter list
        self.expect(&TokenKind::LParen)?;
        let mut params = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::RParen | TokenKind::Eof) {
            let p_name = self.expect_ident()?;
            self.expect(&TokenKind::Colon)?;
            let p_ty = self.parse_ty()?;
            let p_span = p_name.span;
            params.push(Param { name: p_name, ty: p_ty, span: p_span });
            if !matches!(self.peek_kind(), TokenKind::RParen) {
                self.expect(&TokenKind::Comma)?;
            }
        }
        self.expect(&TokenKind::RParen)?;

        // Optional return type
        let return_ty = if matches!(self.peek_kind(), TokenKind::Arrow) {
            self.advance();
            Some(self.parse_return_ty()?)
        } else {
            None
        };

        let hi = self.current_span().lo;
        Ok(ExternFnDef { abi, name, params, return_ty, attributes: vec![], span: Span::new(lo, hi) })
    }

    /// `import "path"` or `import "path" as alias`
    fn parse_import(&mut self) -> Result<ItemKind, ParseError> {
        let lo = self.current_span().lo;
        self.expect(&TokenKind::Import)?;

        // Path must be a string literal: `"utils/math"` 
        let path = match self.peek_kind().clone() {
            TokenKind::String(s) => {
                self.advance();
                s
            }
            _ => return Err(ParseError::unexpected(self.peek_kind(), self.current_span())),
        };

        let hi = self.current_span().lo;

        // Optional alias: `as identifier`
        let alias = if matches!(self.peek_kind(), TokenKind::As) {
            self.advance(); // consume `as`
            Some(self.expect_ident()?.name)
        } else {
            None
        };

        Ok(ItemKind::Import { path, alias, span: Span::new(lo, hi) })
    }

    // ── Struct definition ─────────────────────────────────────────────────

    fn parse_struct(&mut self) -> Result<StructDef, ParseError> {
        let lo = self.current_span().lo;
        self.expect(&TokenKind::Struct)?;
        let name = self.expect_ident()?;
        let type_params = self.parse_type_params()?;
        self.expect(&TokenKind::LBrace)?;

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
            match self.peek_kind() {
                TokenKind::Fn => methods.push(self.parse_fn_def()?),
                _ => fields.push(self.parse_field_def()?),
            }
        }

        let hi = self.current_span().hi;
        self.expect(&TokenKind::RBrace)?;
        Ok(StructDef { name, type_params, fields, methods, attributes: vec![], span: Span::new(lo, hi) })
    }

    // ── Class definition ──────────────────────────────────────────────────

    fn parse_class(&mut self) -> Result<ClassDef, ParseError> {
        let lo = self.current_span().lo;
        self.expect(&TokenKind::Class)?;
        let name = self.expect_ident()?;
        let type_params = self.parse_type_params()?;

        let superclass = if self.eat(&TokenKind::Extends) {
            Some(self.expect_ident()?)
        } else {
            None
        };

        self.expect(&TokenKind::LBrace)?;

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
            match self.peek_kind() {
                TokenKind::Fn => methods.push(self.parse_fn_def()?),
                _ => fields.push(self.parse_field_def()?),
            }
        }

        let hi = self.current_span().hi;
        self.expect(&TokenKind::RBrace)?;
        Ok(ClassDef { name, type_params, superclass, fields, methods, span: Span::new(lo, hi) })
    }

    // ── Protocol definition ───────────────────────────────────────────────

    fn parse_protocol(&mut self) -> Result<ProtocolDef, ParseError> {
        let lo = self.current_span().lo;
        self.expect(&TokenKind::Protocol)?;
        let name = self.expect_ident()?;
        self.expect(&TokenKind::LBrace)?;

        let mut methods = Vec::new();
        let mut default_methods = Vec::new();

        while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
            // Parse the signature first, then check if a body `{` follows.
            let sig = self.parse_fn_sig()?;
            if matches!(self.peek_kind(), TokenKind::LBrace) {
                // Has a default body — parse as a full FnDef.
                let body = self.parse_block()?;
                default_methods.push(FnDef {
                    name:      sig.name,
                    type_params: sig.type_params,
                    params:    sig.params,
                    return_ty: sig.return_ty,
                    attributes: vec![],
                    body,
                    span:      sig.span,
                });
            } else {
                methods.push(sig);
            }
        }

        let hi = self.current_span().hi;
        self.expect(&TokenKind::RBrace)?;
        Ok(ProtocolDef { name, methods, default_methods, span: Span::new(lo, hi) })
    }

    /// Parse a function *signature only* (no body) — used inside protocols.
    fn parse_fn_sig(&mut self) -> Result<FnSig, ParseError> {
        let lo = self.current_span().lo;
        self.expect(&TokenKind::Fn)?;
        let name = self.expect_ident()?;
        let type_params = self.parse_type_params()?;
        self.expect(&TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RParen)?;
        let return_ty = if self.eat(&TokenKind::Arrow) {
            Some(self.parse_return_ty()?)
        } else {
            None
        };
        let hi = self.current_span().lo;
        Ok(FnSig { name, type_params, params, return_ty, span: Span::new(lo, hi) })
    }

    // ── Impl block ────────────────────────────────────────────────────────

    fn parse_impl(&mut self) -> Result<ImplBlock, ParseError> {
        let lo = self.current_span().lo;
        self.expect(&TokenKind::Impl)?;
        // Protocol name may be dotted: `impl view.View for T`
        // Parse the first ident, then consume any `.ident` segments,
        // combining them into a single mangled name `view__View`.
        let mut protocol = self.expect_ident()?;
        while matches!(self.peek_kind(), TokenKind::Dot) {
            self.advance(); // consume `.`
            let part = self.expect_ident()?;
            let combined = format!("{}__{}", protocol.name, part.name);
            let span = Span::new(protocol.span.lo, part.span.hi);
            protocol = Ident::new(combined, span);
        }
        self.expect(&TokenKind::For)?;
        let target = self.expect_ident()?;
        let type_params = self.parse_type_params()?;
        self.expect(&TokenKind::LBrace)?;

        let mut methods = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
            methods.push(self.parse_fn_def()?);
        }

        let hi = self.current_span().hi;
        self.expect(&TokenKind::RBrace)?;
        Ok(ImplBlock { protocol, target, type_params, methods, span: Span::new(lo, hi) })
    }

    // ── Function definition ───────────────────────────────────────────────

    pub fn parse_fn_def(&mut self) -> Result<FnDef, ParseError> {
        let lo = self.current_span().lo;
        self.expect(&TokenKind::Fn)?;
        let name = self.expect_ident()?;
        let type_params = self.parse_type_params()?;
        self.expect(&TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RParen)?;
        let return_ty = if self.eat(&TokenKind::Arrow) {
            Some(self.parse_return_ty()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        let hi = self.current_span().lo;
        Ok(FnDef { name, type_params, params, return_ty, body, attributes: vec![], span: Span::new(lo, hi) })
    }

    // ── Type parameters ───────────────────────────────────────────────────

    /// `<T: Comparable & Printable, U>` — returns empty vec if no `<`.
    fn parse_type_params(&mut self) -> Result<Vec<TypeParam>, ParseError> {
        if !matches!(self.peek_kind(), TokenKind::Lt) {
            return Ok(vec![]);
        }
        self.advance(); // consume `<`
        let mut params = Vec::new();
        loop {
            let lo = self.current_span().lo;
            let name = self.expect_ident()?;
            let mut bounds = Vec::new();
            if self.eat(&TokenKind::Colon) {
                bounds.push(self.expect_ident()?);
                while self.eat(&TokenKind::Amp) {
                    bounds.push(self.expect_ident()?);
                }
            }
            let hi = self.current_span().lo;
            params.push(TypeParam { name, bounds, span: Span::new(lo, hi) });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::Gt)?;
        Ok(params)
    }

    // ── Function parameters ───────────────────────────────────────────────

    fn parse_params(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::RParen | TokenKind::Eof) {
            let lo = self.current_span().lo;
            let name = self.expect_ident()?;
            self.expect(&TokenKind::Colon)?;
            let ty = self.parse_ty()?;
            let hi = self.current_span().lo;
            params.push(Param { name, ty, span: Span::new(lo, hi) });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        Ok(params)
    }

    // ── Return type ───────────────────────────────────────────────────────

    /// After consuming `->`, parse either a single type or a tuple `(T, U)`.
    fn parse_return_ty(&mut self) -> Result<ReturnTy, ParseError> {
        if matches!(self.peek_kind(), TokenKind::LParen) {
            // Tuple return: `(string, Error?)`
            self.advance(); // consume `(`
            let mut tys = Vec::new();
            while !matches!(self.peek_kind(), TokenKind::RParen | TokenKind::Eof) {
                tys.push(self.parse_ty()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::RParen)?;
            if tys.len() == 1 {
                // `(T)` is just `T`
                Ok(ReturnTy::Single(tys.remove(0)))
            } else {
                Ok(ReturnTy::Tuple(tys))
            }
        } else {
            Ok(ReturnTy::Single(self.parse_ty()?))
        }
    }

    // ── Type expressions ──────────────────────────────────────────────────

    fn parse_ty(&mut self) -> Result<Ty, ParseError> {
        let lo = self.current_span().lo;
        let mut ty = self.parse_ty_primary()?;

        // Postfix `?` — optional / nullable
        if self.eat(&TokenKind::Question) {
            let hi = self.current_span().lo;
            ty = Ty { kind: TyKind::Optional(Box::new(ty)), span: Span::new(lo, hi) };
        }

        Ok(ty)
    }

    fn parse_ty_primary(&mut self) -> Result<Ty, ParseError> {
        let lo = self.current_span().lo;

        // Tuple type: `(string, Error?)`
        if matches!(self.peek_kind(), TokenKind::LParen) {
            self.advance();
            let mut tys = Vec::new();
            while !matches!(self.peek_kind(), TokenKind::RParen | TokenKind::Eof) {
                tys.push(self.parse_ty()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            let hi = self.current_span().hi;
            self.expect(&TokenKind::RParen)?;
            return Ok(Ty { kind: TyKind::Tuple(tys), span: Span::new(lo, hi) });
        }

        // Function type: `fn(A, B) -> R`
        // The `fn` keyword in type position introduces a function type.
        if matches!(self.peek_kind(), TokenKind::Fn) {
            self.advance(); // consume `fn`
            self.expect(&TokenKind::LParen)?;
            let mut params = Vec::new();
            while !matches!(self.peek_kind(), TokenKind::RParen | TokenKind::Eof) {
                params.push(self.parse_ty()?);
                if !self.eat(&TokenKind::Comma) { break; }
            }
            self.expect(&TokenKind::RParen)?;
            let ret = if self.eat(&TokenKind::Arrow) {
                Some(Box::new(self.parse_ty()?))
            } else {
                None
            };
            let hi = self.current_span().lo;
            return Ok(Ty {
                kind: TyKind::Fn(params, ret),
                span: Span::new(lo, hi),
            });
        }

        let mut name = self.expect_ident()?;

        // Qualified type name: `lex.Token`, `auth.User` — module.Type in type position.
        // Consumes `Ident . Ident` and produces a single `alias__TypeName` identifier
        // matching the module rename convention used by the driver.
        if matches!(self.peek_kind(), TokenKind::Dot) {
            if matches!(self.peek2_kind(), TokenKind::Ident(_)) {
                self.advance(); // consume `.`
                let type_name = self.expect_ident()?;
                let qualified = format!("{}__{}", name.name, type_name.name);
                name = Ident::new(&qualified, Span::new(name.span.lo, type_name.span.hi));
            }
        }

        // Generic application: `Array<T>`, `Map<string, int>`, `Array<lex.Token>`
        if matches!(self.peek_kind(), TokenKind::Lt) {
            self.advance(); // consume `<`
            let mut args = Vec::new();
            while !matches!(self.peek_kind(), TokenKind::Gt | TokenKind::Eof) {
                args.push(self.parse_ty()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            let hi = self.current_span().hi;
            self.expect(&TokenKind::Gt)?;
            return Ok(Ty { kind: TyKind::Generic(name, args), span: Span::new(lo, hi) });
        }

        let hi = name.span.hi;
        Ok(Ty { kind: TyKind::Named(name), span: Span::new(lo, hi) })
    }

    // ── Field definitions ─────────────────────────────────────────────────

    /// `weak? (const|let)? IDENT : ty`
    ///
    /// Per the spec, `weak` fields may omit the `const`/`let` keyword — they
    /// are implicitly `let`:
    ///   `weak next: Node?`   → is_weak=true, mutability=Let
    ///   `weak team: Team?`   → is_weak=true, mutability=Let
    /// Non-weak fields always require an explicit `const` or `let`.
    fn parse_field_def(&mut self) -> Result<FieldDef, ParseError> {
        let lo = self.current_span().lo;
        let is_weak = self.eat(&TokenKind::Weak);

        let mutability = match self.peek_kind() {
            TokenKind::Const => { self.advance(); Mut::Const }
            TokenKind::Let   => { self.advance(); Mut::Let }
            // A `weak` field may omit the mutability keyword — implicitly `let`.
            _ if is_weak => Mut::Let,
            _ => return Err(ParseError::expected(
                "`const` or `let`", self.peek_kind(), self.current_span(),
            )),
        };

        let name = self.expect_ident()?;
        self.expect(&TokenKind::Colon)?;
        let ty = self.parse_ty()?;
        let hi = self.current_span().lo;
        Ok(FieldDef { mutability, is_weak, name, ty, span: Span::new(lo, hi) })
    }

    // ── Block ─────────────────────────────────────────────────────────────

    pub fn parse_block(&mut self) -> Result<Block, ParseError> {
        let lo = self.current_span().lo;
        self.expect(&TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        let hi = self.current_span().hi;
        self.expect(&TokenKind::RBrace)?;
        Ok(Block { stmts, span: Span::new(lo, hi) })
    }

    // ── Statements ────────────────────────────────────────────────────────

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        let lo = self.current_span().lo;
        let kind = match self.peek_kind() {
            TokenKind::Const | TokenKind::Let => self.parse_let_stmt()?,
            TokenKind::Return  => self.parse_return_stmt()?,
            TokenKind::Yield   => {
                self.advance();
                let expr = self.parse_expr(0)?;
                StmtKind::Yield(Box::new(expr))
            }
            TokenKind::Defer   => {
                self.advance();
                let expr = self.parse_expr(0)?;
                StmtKind::Defer(Box::new(expr))
            }
            TokenKind::Continue => { self.advance(); StmtKind::Continue }
            TokenKind::Break    => { self.advance(); StmtKind::Break }
            TokenKind::If      => StmtKind::If(self.parse_if_expr()?),
            TokenKind::While   => self.parse_while_stmt()?,
            TokenKind::For     => self.parse_for_stmt()?,
            TokenKind::Match   => StmtKind::Match(self.parse_match_expr()?),
            TokenKind::Select  => self.parse_select_stmt()?,
            TokenKind::Panic   => {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let msg = self.parse_expr(0)?;
                self.expect(&TokenKind::RParen)?;
                StmtKind::Panic(Box::new(msg))
            }
            // `_ = expr` — discard assignment
            TokenKind::Under => self.parse_discard_or_expr_stmt()?,
            _ => self.parse_expr_stmt()?,
        };
        let hi = self.current_span().lo;
        Ok(Stmt { kind, span: Span::new(lo, hi) })
    }

    /// `const`/`let` binding statement.
    ///
    /// Forms:
    ///   `const x: T = expr`
    ///   `let x = expr`
    ///   `const data, err = readFile("f")`    // multi-binding
    ///   `const data, _ = readFile("f")`      // with discard
    fn parse_let_stmt(&mut self) -> Result<StmtKind, ParseError> {
        let lo = self.current_span().lo;
        let mutability = match self.peek_kind() {
            TokenKind::Const => { self.advance(); Mut::Const }
            TokenKind::Let   => { self.advance(); Mut::Let }
            _ => unreachable!(),
        };

        // Parse the binding list: may be `x`, `x, y`, `x, _`
        let mut bindings = vec![self.parse_binding()?];
        while self.eat(&TokenKind::Comma) {
            bindings.push(self.parse_binding()?);
        }

        // Optional type annotation — only on single bindings
        let ty = if bindings.len() == 1 && self.eat(&TokenKind::Colon) {
            Some(self.parse_ty()?)
        } else {
            None
        };

        self.expect(&TokenKind::Eq)?;
        let init = self.parse_expr(0)?;
        let hi = self.current_span().lo;

        Ok(StmtKind::Let(LetStmt {
            mutability,
            bindings,
            ty,
            init: Box::new(init),
            span: Span::new(lo, hi),
        }))
    }

    fn parse_binding(&mut self) -> Result<Binding, ParseError> {
        match self.peek_kind() {
            TokenKind::Under => {
                let span = self.current_span();
                self.advance();
                Ok(Binding::Discard(span))
            }
            TokenKind::Ident(_) => Ok(Binding::Name(self.expect_ident()?)),
            _ => Err(ParseError::expected("identifier or `_`", self.peek_kind(), self.current_span())),
        }
    }

    fn parse_return_stmt(&mut self) -> Result<StmtKind, ParseError> {
        let lo = self.current_span().lo;
        self.expect(&TokenKind::Return)?;

        // `return` at end of block or before `}` means void return
        let mut values = Vec::new();
        if !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
            values.push(self.parse_expr(0)?);
            // Multi-return: `return null, err`
            while self.eat(&TokenKind::Comma) {
                values.push(self.parse_expr(0)?);
            }
        }

        let hi = self.current_span().lo;
        Ok(StmtKind::Return(ReturnStmt { values, span: Span::new(lo, hi) }))
    }

    /// Parse an f-string into a chain of string concatenations.
    ///
    /// After `FStringStart` is consumed, we process:
    ///   FStringLit(s)         → ExprKind::String(s)
    ///   FStringInterpStart    → parse_expr(0) until FStringInterpEnd
    ///                           (typechecker coerces each to string)
    ///   FStringEnd            → done
    ///
    /// Result: Binary(Add, Binary(Add, ...), ...) chain.
    /// An empty f-string f"" emits ExprKind::String("".to_string()).
    fn parse_fstring(&mut self, lo: u32) -> Result<Expr, ParseError> {
        let mut parts: Vec<Expr> = Vec::new();

        loop {
            match self.peek_kind() {
                TokenKind::FStringEnd => {
                    self.advance();
                    break;
                }
                TokenKind::FStringLit(s) => {
                    let s = s.clone();
                    let span = self.current_span();
                    self.advance();
                    parts.push(Expr { kind: ExprKind::String(s), span });
                }
                TokenKind::FStringInterpStart => {
                    self.advance(); // consume {
                    let interp_lo = self.current_span().lo;
                    let inner = self.parse_expr(0)?;
                    let interp_hi = self.current_span().hi;
                    let inner_span = inner.span;
                    // Wrap in __fstr_coerce marker — typechecker converts to string
                    // by inserting int_to_string / float_to_string as needed.
                    // We use a NamedCall with the special name __fstr so the
                    // typechecker knows to coerce, not just typecheck.
                    let coerced = Expr {
                        kind: ExprKind::NamedCall(
                            Box::new(Expr {
                                kind: ExprKind::Ident(Ident::new("__fstr", inner_span)),
                                span: inner_span,
                            }),
                            vec![NamedArg { name: Ident::new("val", inner_span), value: inner, span: inner_span }],
                        ),
                        span: Span::new(interp_lo, interp_hi),
                    };
                    parts.push(coerced);
                    self.expect(&TokenKind::FStringInterpEnd)?;
                }
                TokenKind::Eof => {
                    return Err(ParseError::expected("end of f-string", &TokenKind::Eof, self.current_span()));
                }
                _ => {
                    return Err(ParseError::expected("end of f-string", self.peek_kind(), self.current_span()));
                }
            }
        }

        let hi = self.current_span().lo;
        let full_span = Span::new(lo, hi);

        if parts.is_empty() {
            return Ok(Expr { kind: ExprKind::String(String::new()), span: full_span });
        }

        // Fold parts into left-associative string concatenation:
        // ["Hello ", name, "!"] → Binary(Add, Binary(Add, "Hello ", name), "!")
        let mut result = parts.remove(0);
        for part in parts {
            let span = Span::new(result.span.lo, part.span.hi);
            result = Expr {
                kind: ExprKind::Binary(BinaryOp::Add, Box::new(result), Box::new(part)),
                span,
            };
        }
        Ok(result)
    }

    fn parse_while_stmt(&mut self) -> Result<StmtKind, ParseError> {
        self.expect(&TokenKind::While)?;
        let cond = self.parse_expr(0)?;
        let body = self.parse_block()?;
        Ok(StmtKind::While(WhileStmt { cond: Box::new(cond), body, span: Span::dummy() }))
    }

    fn parse_for_stmt(&mut self) -> Result<StmtKind, ParseError> {
        self.expect(&TokenKind::For)?;
        let first = self.expect_ident_or_underscore()?;
        // `for i, v in` — two variables: index then element
        let (index_var, var) = if matches!(self.peek_kind(), TokenKind::Comma) {
            self.advance(); // consume `,`
            let v = self.expect_ident_or_underscore()?;
            (Some(first), v)
        } else {
            (None, first)
        };
        self.expect(&TokenKind::In)?;
        let iter = self.parse_expr(0)?;
        let body = self.parse_block()?;
        Ok(StmtKind::For(ForStmt { index_var, var, iter: Box::new(iter), body, span: Span::dummy() }))
    }

    /// `select { binding = chan.receive() { body } timeout(ms) { body } }`
    fn parse_select_stmt(&mut self) -> Result<StmtKind, ParseError> {
        let lo = self.current_span().lo;
        self.expect(&TokenKind::Select)?;
        self.expect(&TokenKind::LBrace)?;

        let mut arms: Vec<haki_ast::SelectArm> = Vec::new();
        let mut timeout: Option<(Box<haki_ast::Expr>, haki_ast::Block)> = None;

        while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
            let arm_lo = self.current_span().lo;

            // timeout(expr) { body }
            if matches!(self.peek_kind(), TokenKind::Timeout) {
                self.advance();
                self.expect(&TokenKind::LParen)?;
                let ms_expr = self.parse_expr(0)?;
                self.expect(&TokenKind::RParen)?;
                let body = self.parse_block()?;
                timeout = Some((Box::new(ms_expr), body));
                continue;
            }

            // binding = chan.receive() { body }
            let binding = self.expect_ident()?;
            self.expect(&TokenKind::Eq)?;
            let channel = self.parse_expr(0)?;
            let body = self.parse_block()?;
            let arm_hi = self.current_span().lo;
            arms.push(haki_ast::SelectArm {
                binding,
                channel: Box::new(channel),
                body,
                span: Span::new(arm_lo, arm_hi),
            });
        }

        let hi = self.current_span().hi;
        self.expect(&TokenKind::RBrace)?;
        Ok(StmtKind::Select(haki_ast::SelectStmt { arms, timeout, span: Span::new(lo, hi) }))
    }

    /// `_ = expr` — the discard-all form from the spec.
    fn parse_discard_or_expr_stmt(&mut self) -> Result<StmtKind, ParseError> {
        let lo = self.current_span().lo;
        // Peek: is this `_ =`?
        if matches!(self.peek_kind(), TokenKind::Under) && matches!(self.peek2_kind(), TokenKind::Eq) {
            self.advance(); // consume `_`
            self.advance(); // consume `=`
            let init = self.parse_expr(0)?;
            let hi = self.current_span().lo;
            // Represent as `const _ = expr` (special discard let)
            return Ok(StmtKind::Let(LetStmt {
                mutability: Mut::Const,
                bindings: vec![Binding::Discard(Span::new(lo, lo + 1))],
                ty: None,
                init: Box::new(init),
                span: Span::new(lo, hi),
            }));
        }
        // Otherwise fall through to expression statement
        self.parse_expr_stmt()
    }

    fn parse_expr_stmt(&mut self) -> Result<StmtKind, ParseError> {
        let expr = self.parse_expr(0)?;
        Ok(StmtKind::Expr(Box::new(expr)))
    }

    // ── If expression ─────────────────────────────────────────────────────

    fn parse_if_expr(&mut self) -> Result<IfExpr, ParseError> {
        let lo = self.current_span().lo;
        self.expect(&TokenKind::If)?;
        let cond = self.parse_expr(0)?;
        let then_block = self.parse_block()?;

        let else_branch = if self.eat(&TokenKind::Else) {
            if matches!(self.peek_kind(), TokenKind::If) {
                Some(ElseBranch::If(Box::new(self.parse_if_expr()?)))
            } else {
                Some(ElseBranch::Block(self.parse_block()?))
            }
        } else {
            None
        };

        let hi = self.current_span().lo;
        Ok(IfExpr {
            cond: Box::new(cond),
            then_block,
            else_branch,
            span: Span::new(lo, hi),
        })
    }

    // ── Match expression ──────────────────────────────────────────────────

    fn parse_match_expr(&mut self) -> Result<MatchExpr, ParseError> {
        let lo = self.current_span().lo;
        self.expect(&TokenKind::Match)?;
        let scrutinee = self.parse_expr(0)?;
        self.expect(&TokenKind::LBrace)?;

        let mut arms = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::RBrace | TokenKind::Eof) {
            let arm_lo = self.current_span().lo;

            // Pattern: identifier/wildcard, integer literal, or string literal
            let pattern = match self.peek_kind().clone() {
                // Wildcard: `_`
                TokenKind::Under => {
                    self.advance();
                    MatchPattern::Ident(Ident::new("_", self.current_span()))
                }
                // Integer literal: `0`, `42`, `404`
                TokenKind::Int(n) => {
                    let n = n;
                    self.advance();
                    MatchPattern::Int(n)
                }
                // Negative integer literal: `-1`, `-42`
                TokenKind::Minus => {
                    self.advance();
                    if let TokenKind::Int(n) = self.peek_kind().clone() {
                        self.advance();
                        MatchPattern::Int(-n)
                    } else {
                        return Err(ParseError::unexpected(self.peek_kind(), self.current_span()));
                    }
                }
                // String literal: `"GET"`, `"error"`
                TokenKind::String(s) => {
                    let s = s.clone();
                    self.advance();
                    MatchPattern::String(s)
                }
                // Named pattern: variant name, class name
                _ => {
                    MatchPattern::Ident(self.expect_ident()?)
                }
            };

            // Bindings (only valid for Ident patterns, not literals)
            let bindings = match &pattern {
                MatchPattern::Int(_) | MatchPattern::String(_) => vec![],
                MatchPattern::Ident(ident) if ident.name == "_" => vec![],
                MatchPattern::Ident(_) => {
                    if matches!(self.peek_kind(), TokenKind::LParen) {
                        // `Variant(x, y, ...)` — enum payload destructuring
                        self.advance(); // consume `(`
                        let mut names = Vec::new();
                        while !matches!(self.peek_kind(), TokenKind::RParen | TokenKind::Eof) {
                            names.push(self.expect_ident()?);
                            if !matches!(self.peek_kind(), TokenKind::RParen) {
                                self.expect(&TokenKind::Comma)?;
                            }
                        }
                        self.expect(&TokenKind::RParen)?;
                        names
                    } else if matches!(self.peek_kind(), TokenKind::LBrace | TokenKind::RBrace | TokenKind::Eof) {
                        // `Variant { body }` — unit variant, no binding
                        vec![]
                    } else {
                        // `TypeName binding { body }` — class hierarchy match
                        vec![self.expect_ident()?]
                    }
                }
            };

            // Optional guard: `if condition` before the body block
            let guard = if matches!(self.peek_kind(), TokenKind::If) {
                self.advance(); // consume `if`
                Some(self.parse_expr(0)?)
            } else {
                None
            };

            let body = self.parse_block()?;
            let arm_hi = self.current_span().lo;
            arms.push(MatchArm { pattern, bindings, guard, body, span: Span::new(arm_lo, arm_hi) });
        }

        let hi = self.current_span().hi;
        self.expect(&TokenKind::RBrace)?;
        Ok(MatchExpr { scrutinee: Box::new(scrutinee), arms, span: Span::new(lo, hi) })
    }

    // ── Expressions — Pratt precedence climbing ───────────────────────────

    /// Parse an expression with minimum left binding power `min_bp`.
    pub fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let lo = self.current_span().lo;
        let mut lhs = self.parse_expr_prefix()?;

        loop {
                        let kind = self.peek_kind().clone();  // Assignment: `x = expr` — right-associative, lowest precedence.
            // Compound assignment desugars at parse time:
            //   x += rhs  →  x = x + rhs
            // Note: LHS is duplicated in the AST — see V2X_PROGRESS.md for
            // the known double-evaluation caveat (acceptable for v2.4).
            if matches!(kind, TokenKind::Eq
                | TokenKind::PlusEq | TokenKind::MinusEq
                | TokenKind::StarEq | TokenKind::SlashEq | TokenKind::PercentEq)
                && min_bp == 0
            {
                self.advance();
                let rhs = self.parse_expr(0)?;
                let hi  = rhs.span.hi;

                // Desugar compound assignment to plain assignment
                let rhs_desugared = match kind {
                    TokenKind::PlusEq    => Expr { span: Span::new(lo, hi), kind: ExprKind::Binary(BinaryOp::Add, Box::new(lhs.clone()), Box::new(rhs)) },
                    TokenKind::MinusEq   => Expr { span: Span::new(lo, hi), kind: ExprKind::Binary(BinaryOp::Sub, Box::new(lhs.clone()), Box::new(rhs)) },
                    TokenKind::StarEq    => Expr { span: Span::new(lo, hi), kind: ExprKind::Binary(BinaryOp::Mul, Box::new(lhs.clone()), Box::new(rhs)) },
                    TokenKind::SlashEq   => Expr { span: Span::new(lo, hi), kind: ExprKind::Binary(BinaryOp::Div, Box::new(lhs.clone()), Box::new(rhs)) },
                    TokenKind::PercentEq => Expr { span: Span::new(lo, hi), kind: ExprKind::Binary(BinaryOp::Mod, Box::new(lhs.clone()), Box::new(rhs)) },
                    _                    => rhs,  // plain Eq
                };

                lhs = Expr {
                    kind: ExprKind::Assign(Box::new(lhs), Box::new(rhs_desugared)),
                    span: Span::new(lo, hi),
                };
                break; // assignment is not chained
            }

            // Ternary operator: cond ? then : else
            // Lowers to ExprKind::If with a mandatory else branch.
            // NOTE: `?.` is optional chaining (handled in parse_expr_postfix), NOT ternary.
            if matches!(kind, TokenKind::Question)
                && !matches!(self.tokens.get(self.pos + 1).map(|t| &t.kind), Some(TokenKind::Dot))
            {
                let (l_bp, _r_bp) = infix_bp(&kind).unwrap();
                if l_bp < min_bp { break; }
                self.advance(); // consume `?`
                let then_expr = self.parse_expr(0)?;
                self.expect(&TokenKind::Colon)?;
                let else_expr = self.parse_expr(0)?;
                let hi = else_expr.span.hi;
                // Lower to IfExpr with yield in both branches
                let then_block = Block {
                    stmts: vec![Stmt {
                        kind: StmtKind::Yield(Box::new(then_expr)),
                        span: Span::dummy(),
                    }],
                    span: Span::dummy(),
                };
                let else_block = Block {
                    stmts: vec![Stmt {
                        kind: StmtKind::Yield(Box::new(else_expr)),
                        span: Span::dummy(),
                    }],
                    span: Span::dummy(),
                };
                lhs = Expr {
                    kind: ExprKind::If(Box::new(IfExpr {
                        cond: Box::new(lhs),
                        then_block,
                        else_branch: Some(ElseBranch::Block(else_block)),
                        span: Span::new(lo, hi),
                    })),
                    span: Span::new(lo, hi),
                };
                continue;
            }

            // Binary operator?
            // Skip `?` when it's optional chaining `?.` — that's handled below in postfix.
            let is_optional_chain = matches!(kind, TokenKind::Question)
                && matches!(self.tokens.get(self.pos + 1).map(|t| &t.kind), Some(TokenKind::Dot));
            if !is_optional_chain {
            if let Some((l_bp, r_bp)) = infix_bp(&kind) {
                if l_bp < min_bp {
                    break;
                }
                let op_kind = kind.clone();
                self.advance();
                let rhs = self.parse_expr(r_bp)?;
                let hi = rhs.span.hi;
                let op = token_to_binop(&op_kind).unwrap();
                lhs = Expr {
                    kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)),
                    span: Span::new(lo, hi),
                };
                continue;
            }
            } // end !is_optional_chain guard

            // Postfix: field access / method call / index / call
            match self.peek_kind() {
                TokenKind::Dot => {
                    self.advance(); // consume `.`
                    // After a dot, certain keywords are valid as method names.
                    // `await` and `join` are contextual — only keywords at call site.
                    let field = match self.peek_kind().clone() {
                        TokenKind::Ident(name) => {
                            let span = self.current_span();
                            self.advance();
                            Ident::new(name, span)
                        }
                        // Contextual keywords valid as method names after `.`
                        TokenKind::Await => {
                            let span = self.current_span();
                            self.advance();
                            Ident::new("await", span)
                        }
                        _ => self.expect_ident()?,
                    };
                    // Is it a method call or named-arg constructor call?
                    // `obj.method(a, b)`        → MethodCall
                    // `obj.method<T>(a, b)`     → MethodCall with type-param encoded in field name
                    // `module.Type(field: val)` → NamedCall on Field expr
                    if matches!(self.peek_kind(), TokenKind::Lt) {
                        // Speculatively try to parse `<type_args>` before `(args)`
                        let saved_pos = self.pos;
                        if let Ok(type_args) = self.try_parse_type_args_for_constructor() {
                            if matches!(self.peek_kind(), TokenKind::LParen) {
                                self.advance(); // consume `(`
                                let (named, positional) = self.parse_call_args_mixed()?;
                                self.expect(&TokenKind::RParen)?;
                                let hi = self.current_span().lo;
                                // Encode the type args in the field name: `chan<string>`
                                let generic_field_name = format!("{}<{}>",
                                    field.name,
                                    type_args.iter().map(|t| t.as_str()).collect::<Vec<_>>().join(","));
                                let generic_field = Ident::new(&generic_field_name, field.span);
                                lhs = Expr {
                                    kind: ExprKind::MethodCall(Box::new(lhs), generic_field, positional),
                                    span: Span::new(lo, hi),
                                };
                            } else {
                                self.pos = saved_pos;
                                let hi = field.span.hi;
                                lhs = Expr {
                                    kind: ExprKind::Field(Box::new(lhs), field),
                                    span: Span::new(lo, hi),
                                };
                            }
                        } else {
                            self.pos = saved_pos;
                            let hi = field.span.hi;
                            lhs = Expr {
                                kind: ExprKind::Field(Box::new(lhs), field),
                                span: Span::new(lo, hi),
                            };
                        }
                    } else if matches!(self.peek_kind(), TokenKind::LParen) {
                        self.advance(); // consume `(`
                        let (named, positional) = self.parse_call_args_mixed()?;
                        self.expect(&TokenKind::RParen)?;
                        let hi = self.current_span().lo;
                        if named.is_empty() {
                            lhs = Expr {
                                kind: ExprKind::MethodCall(Box::new(lhs), field, positional),
                                span: Span::new(lo, hi),
                            };
                        } else {
                            // Named args on a dotted path: `module.Type(field: val)`
                            // Represent as NamedCall on a Field expression.
                            let field_expr = Expr {
                                kind: ExprKind::Field(Box::new(lhs), field),
                                span: Span::new(lo, hi),
                            };
                            lhs = Expr {
                                kind: ExprKind::NamedCall(Box::new(field_expr), named),
                                span: Span::new(lo, hi),
                            };
                        }
                    } else {
                        let hi = field.span.hi;
                        lhs = Expr {
                            kind: ExprKind::Field(Box::new(lhs), field),
                            span: Span::new(lo, hi),
                        };
                    }
                    continue;
                }
                TokenKind::LBracket => {
                    self.advance(); // consume `[`
                    let idx = self.parse_expr(0)?;
                    self.expect(&TokenKind::RBracket)?;
                    let hi = self.current_span().lo;
                    lhs = Expr {
                        kind: ExprKind::Index(Box::new(lhs), Box::new(idx)),
                        span: Span::new(lo, hi),
                    };
                    continue;
                }
                // Postfix call: `Stack<int>(items: arr)` or `expr(args)`
                // This handles generic type construction like Stack<int>() or Stack<int>(items: x)
                TokenKind::LParen => {
                    self.advance(); // consume `(`
                    let (named, positional) = self.parse_call_args_mixed()?;
                    self.expect(&TokenKind::RParen)?;
                    let hi = self.current_span().lo;
                    if named.is_empty() {
                        lhs = Expr {
                            kind: ExprKind::Call(Box::new(lhs), positional),
                            span: Span::new(lo, hi),
                        };
                    } else {
                        lhs = Expr {
                            kind: ExprKind::NamedCall(Box::new(lhs), named),
                            span: Span::new(lo, hi),
                        };
                    }
                    continue;
                }
                // Optional chaining: `expr?.field` or `expr?.method(args)`
                TokenKind::Question if matches!(self.tokens.get(self.pos + 1).map(|t| &t.kind), Some(TokenKind::Dot)) => {
                    self.advance(); // consume `?`
                    self.advance(); // consume `.`
                    let field = match self.peek_kind().clone() {
                        TokenKind::Ident(name) => { let sp = self.current_span(); self.advance(); Ident::new(name, sp) }
                        TokenKind::Await => { let sp = self.current_span(); self.advance(); Ident::new("await", sp) }
                        _ => return Err(ParseError::expected("field or method name after `?.`", self.peek_kind(), self.current_span())),
                    };
                    let hi = self.current_span().lo;
                    // Check for generic type args before method call parens
                    let saved_pos = self.pos;
                    let has_generic = if matches!(self.peek_kind(), TokenKind::Lt) {
                        self.try_parse_type_args_for_constructor().is_ok()
                            && matches!(self.peek_kind(), TokenKind::LParen)
                    } else { false };
                    self.pos = saved_pos;
                    if matches!(self.peek_kind(), TokenKind::LParen) {
                        self.advance(); // consume `(`
                        let (_, positional) = self.parse_call_args_mixed()?;
                        self.expect(&TokenKind::RParen)?;
                        let hi = self.current_span().lo;
                        lhs = Expr { kind: ExprKind::OptionalMethodCall(Box::new(lhs), field, positional), span: Span::new(lo, hi) };
                    } else {
                        lhs = Expr { kind: ExprKind::OptionalField(Box::new(lhs), field), span: Span::new(lo, hi) };
                    }
                    continue;
                }
                _ => break,
            }
        }

        Ok(lhs)
    }

    /// Parse prefix (unary) expressions and primary atoms.
    fn parse_expr_prefix(&mut self) -> Result<Expr, ParseError> {
        let lo = self.current_span().lo;

        match self.peek_kind().clone() {
            // Unary `-`
            TokenKind::Minus => {
                self.advance();
                let operand = self.parse_expr_prefix()?;
                let hi = operand.span.hi;
                return Ok(Expr {
                    kind: ExprKind::Unary(UnaryOp::Neg, Box::new(operand)),
                    span: Span::new(lo, hi),
                });
            }

            // Unary `!`
            TokenKind::Bang => {
                self.advance();
                let operand = self.parse_expr_prefix()?;
                let hi = operand.span.hi;
                return Ok(Expr {
                    kind: ExprKind::Unary(UnaryOp::Not, Box::new(operand)),
                    span: Span::new(lo, hi),
                });
            }

            // `try f()` — desugar to:
            //   { const __val, __err = f(); if __err != null { return null, __err }; yield __val }
            // The typechecker and codegen see only explicit multi-return destructuring
            // and an early return, so defer/ARC/scope cleanup all work correctly.
            TokenKind::Try => {
                self.advance();
                let call_expr = self.parse_expr(0)?;
                let sp = Span::new(lo, call_expr.span.hi);
                let desugar = self.desugar_try(call_expr, sp);
                return Ok(desugar);
            }

            _ => {}
        }

        self.parse_expr_atom()
    }

    /// Parse a primary atom (literal, identifier, grouped expr, if, match, block).
    fn parse_expr_atom(&mut self) -> Result<Expr, ParseError> {
        let lo = self.current_span().lo;

        match self.peek_kind().clone() {
            // ── Literals ──────────────────────────────────────────────
            TokenKind::Int(n) => {
                self.advance();
                Ok(Expr { kind: ExprKind::Int(n), span: Span::new(lo, self.current_span().lo) })
            }
            TokenKind::Float(f) => {
                self.advance();
                Ok(Expr { kind: ExprKind::Float(f), span: Span::new(lo, self.current_span().lo) })
            }
            TokenKind::String(s) => {
                self.advance();
                Ok(Expr { kind: ExprKind::String(s), span: Span::new(lo, self.current_span().lo) })
            }
            // F-string: f"Hello {name}!"
            // Assembled into a chain of string concatenations at parse time.
            // Each {expr} is coerced to string in the typechecker.
            TokenKind::FStringStart => {
                self.advance(); // consume FStringStart
                self.parse_fstring(lo)
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr { kind: ExprKind::Bool(true), span: Span::new(lo, self.current_span().lo) })
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr { kind: ExprKind::Bool(false), span: Span::new(lo, self.current_span().lo) })
            }
            TokenKind::Null => {
                self.advance();
                Ok(Expr { kind: ExprKind::Null, span: Span::new(lo, self.current_span().lo) })
            }

            // ── Array literal: `[expr, expr, ...]` ────────────────────
            TokenKind::LBracket => {
                self.advance();
                let mut elems = Vec::new();
                while !matches!(self.peek_kind(), TokenKind::RBracket | TokenKind::Eof) {
                    elems.push(self.parse_expr(0)?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                let hi = self.current_span().hi;
                self.expect(&TokenKind::RBracket)?;
                Ok(Expr { kind: ExprKind::Array(elems), span: Span::new(lo, hi) })
            }

            // ── Grouped expression or tuple: `(expr)` ─────────────────
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_expr(0)?;
                self.expect(&TokenKind::RParen)?;
                // Just return the inner expression — parentheses don't produce a node.
                Ok(expr)
            }

            // ── If expression ──────────────────────────────────────────
            TokenKind::If => {
                let if_expr = self.parse_if_expr()?;
                let span = if_expr.span;
                Ok(Expr { kind: ExprKind::If(Box::new(if_expr)), span })
            }

            // ── Match expression ───────────────────────────────────────
            TokenKind::Match => {
                let m = self.parse_match_expr()?;
                let span = m.span;
                Ok(Expr { kind: ExprKind::Match(Box::new(m)), span })
            }

            // ── Block expression: `{ stmts... }` ──────────────────────
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                let span = block.span;
                Ok(Expr { kind: ExprKind::Block(Box::new(block)), span })
            }

            // ── Anonymous function literal (with optional capture list) ───
            // `fn(x: A) -> R { body }`           — no captures (function pointer)
            // `fn[self, count](x: A) -> R { body }` — closure with captures
            TokenKind::Fn if matches!(self.peek2_kind(),
                TokenKind::LParen | TokenKind::LBracket) => {
                self.advance(); // consume `fn`

                // Parse optional capture list: `[weak self, count, ...]`
                let captures = if matches!(self.peek_kind(), TokenKind::LBracket) {
                    self.advance(); // consume `[`
                    let mut caps = Vec::new();
                    while !matches!(self.peek_kind(), TokenKind::RBracket | TokenKind::Eof) {
                        let cap_lo = self.current_span().lo;
                        let weak = if matches!(self.peek_kind(), TokenKind::Weak) {
                            self.advance();
                            true
                        } else { false };
                        let name = self.expect_ident()?;
                        let cap_hi = self.current_span().lo;
                        caps.push(Capture { name, weak, mutable: false, span: Span::new(cap_lo, cap_hi) });
                        self.eat(&TokenKind::Comma);
                    }
                    self.expect(&TokenKind::RBracket)?;
                    caps
                } else {
                    vec![]
                };

                self.expect(&TokenKind::LParen)?;
                let params = self.parse_params()?;
                self.expect(&TokenKind::RParen)?;
                let return_ty = if self.eat(&TokenKind::Arrow) {
                    Some(self.parse_return_ty()?)
                } else {
                    None
                };
                let body = self.parse_block()?;
                let hi = body.span.hi;
                Ok(Expr {
                    kind: ExprKind::FnLiteral {
                        captures,
                        params,
                        return_ty,
                        body: Box::new(body),
                    },
                    span: Span::new(lo, hi),
                })
            }

            // ── Async expression: `async expr` ─────────────────────────
            // `async` is a call-site modifier. `async fetchUser(42)` wraps
            // the call expression in an Async node, producing a `Task<T>`.
            TokenKind::Async => {
                self.advance(); // consume `async`
                let inner = self.parse_expr(0)?;
                let hi = inner.span.hi;
                Ok(Expr {
                    kind: ExprKind::Async(Box::new(inner)),
                    span: Span::new(lo, hi),
                })
            }

            // ── Identifier or call ─────────────────────────────────────
            TokenKind::Ident(name) => {
                let ident = Ident::new(name, Span::new(lo, self.current_span().hi));
                self.advance();

                // Generic constructor: `Type<T>(field: val)` or `Type<T>(val)`
                // Disambiguate: `<` followed by type name followed by `>` then `(`
                if matches!(self.peek_kind(), TokenKind::Lt) {
                    // Speculatively try to parse type args
                    let saved_pos = self.pos;
                    if let Ok(type_args) = self.try_parse_type_args_for_constructor() {
                        if matches!(self.peek_kind(), TokenKind::LParen) {
                            self.advance(); // consume `(`
                            let (named, positional) = self.parse_call_args_mixed()?;
                            self.expect(&TokenKind::RParen)?;
                            let hi = self.current_span().lo;
                            // Build a GenericType ident to carry the type args
                            let generic_name = format!("{}<{}>", ident.name,
                                type_args.iter().map(|t| t.as_str()).collect::<Vec<_>>().join(","));
                            let callee = Expr {
                                kind: ExprKind::Ident(Ident::new(&generic_name, ident.span)),
                                span: Span::new(lo, hi)
                            };
                            return if named.is_empty() {
                                Ok(Expr { kind: ExprKind::Call(Box::new(callee), positional), span: Span::new(lo, hi) })
                            } else {
                                Ok(Expr { kind: ExprKind::NamedCall(Box::new(callee), named), span: Span::new(lo, hi) })
                            };
                        } else {
                            // Wasn't a constructor call, restore position
                            self.pos = saved_pos;
                        }
                    } else {
                        self.pos = saved_pos;
                    }
                }

                // Is this a call?
                if matches!(self.peek_kind(), TokenKind::LParen) {
                    self.advance(); // consume `(`
                    let (named, positional) = self.parse_call_args_mixed()?;
                    self.expect(&TokenKind::RParen)?;
                    let hi = self.current_span().lo;
                    let callee = Expr { kind: ExprKind::Ident(ident), span: Span::new(lo, hi) };
                    if named.is_empty() {
                        Ok(Expr { kind: ExprKind::Call(Box::new(callee), positional), span: Span::new(lo, hi) })
                    } else {
                        Ok(Expr { kind: ExprKind::NamedCall(Box::new(callee), named), span: Span::new(lo, hi) })
                    }
                } else {
                    let hi = self.current_span().lo;
                    Ok(Expr { kind: ExprKind::Ident(ident), span: Span::new(lo, hi) })
                }
            }

            _ => Err(ParseError::unexpected(self.peek_kind(), self.current_span())),
        }
    }

    // ── Annotation definition ────────────────────────────────────────────────

    fn parse_annotation_def(&mut self) -> Result<haki_ast::AnnotationDef, ParseError> {
        let lo = self.current_span().lo;
        self.expect(&TokenKind::Annotation)?;
        self.expect(&TokenKind::At)?;
        let name = self.expect_ident()?.name;
        let params = if matches!(self.peek_kind(), TokenKind::LParen) {
            self.advance();
            let mut ps = Vec::new();
            while !matches!(self.peek_kind(), TokenKind::RParen | TokenKind::Eof) {
                let pname = self.expect_ident()?;
                self.expect(&TokenKind::Colon)?;
                let pty = self.parse_ty()?;
                ps.push(haki_ast::Param { name: pname, ty: pty, span: haki_ast::Span::dummy() });
                if !self.eat(&TokenKind::Comma) { break; }
            }
            self.expect(&TokenKind::RParen)?;
            ps
        } else { vec![] };
        let body = self.parse_block()?;
        let hi = self.current_span().hi;
        Ok(haki_ast::AnnotationDef { name, params, body, span: haki_ast::Span::new(lo, hi) })
    }

    // ── Generic constructor type args ─────────────────────────────────────

    /// Try to parse `<TypeArg, ...>` for a generic constructor call.
    /// Returns Ok(type_args) if successful, Err if this isn't a type arg list.
    /// Uses a simple heuristic: `<` IDENT (`,` IDENT)* `>` followed by `(`
    fn try_parse_type_args_for_constructor(&mut self) -> Result<Vec<String>, ParseError> {
        // Must start with `<`
        if !matches!(self.peek_kind(), TokenKind::Lt) {
            return Err(ParseError::unexpected(self.peek_kind(), self.current_span()));
        }
        self.advance(); // consume `<`
        let mut args = Vec::new();
        loop {
            match self.peek_kind() {
                TokenKind::Ident(name) => {
                    args.push(name.clone());
                    self.advance();
                    if self.eat(&TokenKind::Comma) { continue; }
                    // Expect `>`
                    if matches!(self.peek_kind(), TokenKind::Gt) {
                        self.advance(); // consume `>`
                        return Ok(args);
                    }
                    return Err(ParseError::unexpected(self.peek_kind(), self.current_span()));
                }
                _ => return Err(ParseError::unexpected(self.peek_kind(), self.current_span())),
            }
        }
    }

    // ── Call argument helpers ─────────────────────────────────────────────

    /// Parse purely positional arguments (used in method calls).
    fn parse_call_args(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        while !matches!(self.peek_kind(), TokenKind::RParen | TokenKind::Eof) {
            args.push(self.parse_expr(0)?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        Ok(args)
    }

    /// Parse arguments that might be labelled (`name: expr`) or positional.
    /// Returns `(named_args, positional_args)`.
    /// If any argument is named, ALL must be named (enforced by typeck, not here).
    fn parse_call_args_mixed(
        &mut self,
    ) -> Result<(Vec<NamedArg>, Vec<Expr>), ParseError> {
        let mut named = Vec::new();
        let mut positional = Vec::new();

        while !matches!(self.peek_kind(), TokenKind::RParen | TokenKind::Eof) {
            let lo = self.current_span().lo;

            // Peek ahead: `ident :` → named argument
            let is_named = matches!(self.peek_kind(), TokenKind::Ident(_))
                && matches!(self.peek2_kind(), TokenKind::Colon);

            if is_named {
                let name = self.expect_ident()?;
                self.expect(&TokenKind::Colon)?;
                let value = self.parse_expr(0)?;
                let hi = self.current_span().lo;
                named.push(NamedArg { name, value, span: Span::new(lo, hi) });
            } else {
                positional.push(self.parse_expr(0)?);
            }

            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }

        Ok((named, positional))
    }
}
