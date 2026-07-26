/// haki_parser — Recursive descent parser for the Haki programming language.
///
/// Architecture:
///   - `Parser` wraps the flat token stream from `haki_lexer` with a cursor.
///   - Every `parse_*` method corresponds to a grammar production.
///   - `parse()` — stop-at-first-error, used by the normal compile pipeline.
///   - `parse_recovery()` — error-recovering parse for the LSP: returns a
///     partial AST plus all errors found. Broken top-level items are skipped
///     via `synchronize()` (advance to next `fn`/`struct`/`class`/etc.).
///   - Operator precedence for expressions is handled via a Pratt-style
///     precedence-climbing loop in `parse_expr`.

mod parser;

pub use parser::Parser;

use thiserror::Error;
use haki_ast::Span;
use haki_lexer::TokenKind;

// ── Parse error ───────────────────────────────────────────────────────────────

#[derive(Debug, Error, Clone)]
pub enum ParseError {
    #[error("{span:?}: expected {expected}, found {found}")]
    Expected {
        expected: String,
        found: String,
        span: Span,
    },

    #[error("{span:?}: unexpected token '{found}'")]
    Unexpected { found: String, span: Span },

    #[error("{span:?}: {message}")]
    Other { message: String, span: Span },
}

impl ParseError {
    pub fn expected(expected: impl Into<String>, found: &TokenKind, span: Span) -> Self {
        Self::Expected {
            expected: expected.into(),
            found: token_description(found),
            span,
        }
    }

    pub fn unexpected(found: &TokenKind, span: Span) -> Self {
        Self::Unexpected { found: token_description(found), span }
    }

    /// The byte span of this error in the source file.
    pub fn span(&self) -> Span {
        match self {
            Self::Expected { span, .. } => *span,
            Self::Unexpected { span, .. } => *span,
            Self::Other { span, .. } => *span,
        }
    }
}

/// The result of an error-recovering parse.
/// Always contains a (possibly partial) AST plus zero or more errors.
#[derive(Debug)]
pub struct ParseResult {
    pub ast:    haki_ast::SourceFile,
    pub errors: Vec<ParseError>,
}

impl ParseResult {
    pub fn is_ok(&self) -> bool { self.errors.is_empty() }
}

/// Human-readable description of a token kind for error messages.
pub(crate) fn token_description(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Ident(s)  => format!("identifier `{s}`"),
        TokenKind::Int(n)    => format!("integer `{n}`"),
        TokenKind::Float(f)  => format!("float `{f}`"),
        TokenKind::String(s) => format!("string `\"{s}\"`"),
        TokenKind::Fn        => "`fn`".into(),
        TokenKind::Struct    => "`struct`".into(),
        TokenKind::Class     => "`class`".into(),
        TokenKind::Protocol  => "`protocol`".into(),
        TokenKind::Impl      => "`impl`".into(),
        TokenKind::For       => "`for`".into(),
        TokenKind::Extends   => "`extends`".into(),
        TokenKind::Const     => "`const`".into(),
        TokenKind::Let       => "`let`".into(),
        TokenKind::Weak      => "`weak`".into(),
        TokenKind::If        => "`if`".into(),
        TokenKind::Else      => "`else`".into(),
        TokenKind::While     => "`while`".into(),
        TokenKind::In        => "`in`".into(),
        TokenKind::Return    => "`return`".into(),
        TokenKind::Yield     => "`yield`".into(),
        TokenKind::Match     => "`match`".into(),
        TokenKind::Panic     => "`panic`".into(),
        TokenKind::Async     => "`async`".into(),
        TokenKind::Await     => "`await`".into(),
        TokenKind::As        => "`as`".into(),
        TokenKind::Import    => "`import`".into(),
        TokenKind::Defer     => "`defer`".into(),
        TokenKind::Extern    => "`extern`".into(),
        TokenKind::Enum      => "`enum`".into(),
        TokenKind::Try       => "`try`".into(),
        TokenKind::Continue  => "`continue`".into(),
        TokenKind::Break     => "`break`".into(),
        TokenKind::True      => "`true`".into(),
        TokenKind::False     => "`false`".into(),
        TokenKind::Null      => "`null`".into(),
        TokenKind::LParen    => "`(`".into(),
        TokenKind::RParen    => "`)`".into(),
        TokenKind::LBrace    => "`{`".into(),
        TokenKind::RBrace    => "`}`".into(),
        TokenKind::LBracket  => "`[`".into(),
        TokenKind::RBracket  => "`]`".into(),
        TokenKind::Lt        => "`<`".into(),
        TokenKind::Gt        => "`>`".into(),
        TokenKind::Comma     => "`,`".into(),
        TokenKind::Colon     => "`:`".into(),
        TokenKind::Dot       => "`.`".into(),
        TokenKind::Question  => "`?`".into(),
        TokenKind::Under     => "`_`".into(),
        TokenKind::At        => "`@`".into(),
        TokenKind::Plus      => "`+`".into(),
        TokenKind::Minus     => "`-`".into(),
        TokenKind::Star      => "`*`".into(),
        TokenKind::Slash     => "`/`".into(),
        TokenKind::Percent   => "`%`".into(),
        TokenKind::Bang      => "`!`".into(),
        TokenKind::Eq        => "`=`".into(),
        TokenKind::EqEq      => "`==`".into(),
        TokenKind::BangEq    => "`!=`".into(),
        TokenKind::LtEq      => "`<=`".into(),
        TokenKind::GtEq      => "`>=`".into(),
        TokenKind::AndAnd    => "`&&`".into(),
        TokenKind::OrOr      => "`||`".into(),
        TokenKind::Arrow     => "`->`".into(),
        TokenKind::Amp       => "`&`".into(),
        TokenKind::Eof       => "end of file".into(),
    }
}

/// Parse a complete source file — stops at the first error.
/// Used by the normal compile pipeline.
pub fn parse(src: &str) -> Result<haki_ast::SourceFile, ParseError> {
    let tokens = haki_lexer::lex(src)
        .map_err(|e| ParseError::Other {
            message: e.to_string(),
            span: Span::dummy(),
        })?;
    Parser::new(tokens).parse_file()
}

/// Parse a source file with error recovery — returns a partial AST plus all
/// errors found. Broken top-level items are skipped; valid items are kept.
/// Used by the LSP to maintain a working index even when the file has errors.
pub fn parse_recovery(src: &str) -> ParseResult {
    let tokens = match haki_lexer::lex(src) {
        Ok(t) => t,
        Err(e) => {
            // Lex error — return empty AST with one error
            return ParseResult {
                ast: haki_ast::SourceFile {
                    items: vec![],
                    span: Span::new(0, src.len() as u32),
                },
                errors: vec![ParseError::Other {
                    message: e.to_string(),
                    span: Span::dummy(),
                }],
            };
        }
    };

    let mut parser = Parser::new(tokens);
    parser.parse_file_recovery()
}


