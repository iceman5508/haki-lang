/// haki_parser — Recursive descent parser for the Haki programming language.
///
/// Architecture:
///   - `Parser` wraps the flat token stream from `haki_lexer` with a cursor.
///   - Every `parse_*` method corresponds to a grammar production.
///   - Error recovery is "stop at first error" for v0.1 — the type-checker and
///     later passes need a coherent AST, so we don't attempt to recover.
///   - Operator precedence for expressions is handled via a Pratt-style
///     precedence-climbing loop in `parse_expr`.

mod parser;

pub use parser::Parser;

use thiserror::Error;
use haki_ast::Span;
use haki_lexer::TokenKind;

// ── Parse error ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
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

/// Parse a complete source file.
pub fn parse(src: &str) -> Result<haki_ast::SourceFile, ParseError> {
    let tokens = haki_lexer::lex(src)
        .map_err(|e| ParseError::Other {
            message: e.to_string(),
            span: Span::dummy(),
        })?;
    Parser::new(tokens).parse_file()
}
