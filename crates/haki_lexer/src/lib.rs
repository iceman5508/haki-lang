/// haki_lexer — Hand-written tokenizer for the Haki programming language.
///
/// Design:
///   - Single-pass, character-by-character.
///   - Produces a flat `Vec<Token>` including an `Eof` sentinel.
///   - Comments (`//` to end-of-line) are skipped.
///   - Whitespace (including newlines) is skipped — Haki has no
///     statement-terminating significance for newlines.
///   - String literals support `\"` and `\\` escape sequences.

use haki_ast::Span;
use thiserror::Error;

// ── Token kinds ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // ── Literals ──────────────────────────────────────────────
    Int(i64),
    Float(f64),
    String(String),

    // ── Identifiers & keywords ────────────────────────────────
    Ident(String),

    // Keywords
    Fn,
    Struct,
    Class,
    Protocol,
    Impl,
    For,
    Extends,

    Const,
    Let,
    Weak,

    If,
    Else,
    While,
    In,
    Return,
    Yield,
    Match,
    Panic,
    Async,
    Await,
    As,
    Import,
    Defer,
    Extern,
    Enum,
    Try,
    Continue,
    Break,

    True,
    False,
    Null,

    // ── Punctuation ───────────────────────────────────────────
    LParen,   // (
    RParen,   // )
    LBrace,   // {
    RBrace,   // }
    LBracket, // [
    RBracket, // ]
    Lt,       // <
    Gt,       // >
    Comma,    // ,
    Colon,    // :
    Dot,      // .
    Question, // ?
    Under,    // _
    At,       // @  (attribute prefix: @link, @deprecated, @inline ...)

    // ── Operators ─────────────────────────────────────────────
    Plus,     // +
    Minus,    // -
    Star,     // *
    Slash,    // /
    Percent,  // %
    Bang,     // !
    Eq,       // =
    EqEq,     // ==
    BangEq,   // !=
    LtEq,     // <=
    GtEq,     // >=
    AndAnd,   // &&
    OrOr,     // ||
    Arrow,    // ->
    Amp,      // &   (used in protocol bounds: T: A & B)

    // ── End of file ───────────────────────────────────────────
    Eof,
}

impl TokenKind {
    /// Map an identifier string to a keyword token, or return `None` if it is
    /// a plain identifier.
    fn from_keyword(s: &str) -> Option<TokenKind> {
        match s {
            "fn"       => Some(TokenKind::Fn),
            "struct"   => Some(TokenKind::Struct),
            "class"    => Some(TokenKind::Class),
            "protocol" => Some(TokenKind::Protocol),
            "impl"     => Some(TokenKind::Impl),
            "for"      => Some(TokenKind::For),
            "extends"  => Some(TokenKind::Extends),
            "const"    => Some(TokenKind::Const),
            "let"      => Some(TokenKind::Let),
            "weak"     => Some(TokenKind::Weak),
            "if"       => Some(TokenKind::If),
            "else"     => Some(TokenKind::Else),
            "while"    => Some(TokenKind::While),
            "in"       => Some(TokenKind::In),
            "return"   => Some(TokenKind::Return),
            "yield"    => Some(TokenKind::Yield),
            "match"    => Some(TokenKind::Match),
            "panic"    => Some(TokenKind::Panic),
            "async"    => Some(TokenKind::Async),
            "await"    => Some(TokenKind::Await),
            "as"       => Some(TokenKind::As),
            "import"   => Some(TokenKind::Import),
            "defer"    => Some(TokenKind::Defer),
            "extern"   => Some(TokenKind::Extern),
            "enum"     => Some(TokenKind::Enum),
            "try"      => Some(TokenKind::Try),
            "continue" => Some(TokenKind::Continue),
            "break"    => Some(TokenKind::Break),
            "true"     => Some(TokenKind::True),
            "false"    => Some(TokenKind::False),
            "null"     => Some(TokenKind::Null),
            _          => None,
        }
    }
}

// ── Token ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    fn new(kind: TokenKind, lo: u32, hi: u32) -> Self {
        Self { kind, span: Span::new(lo, hi) }
    }
}

// ── Lex error ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum LexError {
    #[error("{0}:{1}: unexpected character '{2}'")]
    UnexpectedChar(u32, u32, char),

    #[error("{0}:{1}: unterminated string literal")]
    UnterminatedString(u32, u32),

    #[error("{0}:{1}: invalid numeric literal")]
    InvalidNumber(u32, u32),
}

// ── Lexer ─────────────────────────────────────────────────────────────────────

pub struct Lexer<'src> {
    src: &'src [u8],
    pos: usize,
}

impl<'src> Lexer<'src> {
    pub fn new(src: &'src str) -> Self {
        Self { src: src.as_bytes(), pos: 0 }
    }

    // ── Internal helpers ───────────────────────────────────────────────────

    fn at_end(&self) -> bool {
        self.pos >= self.src.len()
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn peek2(&self) -> Option<u8> {
        self.src.get(self.pos + 1).copied()
    }

    #[allow(dead_code)]
    fn advance(&mut self) -> u8 {
        let b = self.src[self.pos];
        self.pos += 1;
        b
    }

    fn skip_while(&mut self, f: impl Fn(u8) -> bool) {
        while self.peek().map_or(false, |b| f(b)) {
            self.pos += 1;
        }
    }

    fn slice(&self, lo: usize, hi: usize) -> &str {
        // Safety: we only ever produce slices from validated UTF-8 input.
        std::str::from_utf8(&self.src[lo..hi]).unwrap()
    }

    // ── Skip whitespace and line comments ─────────────────────────────────

    fn skip_trivia(&mut self) {
        loop {
            // Whitespace
            self.skip_while(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'));

            // Line comment: // ...
            if self.peek() == Some(b'/') && self.peek2() == Some(b'/') {
                self.pos += 2;
                self.skip_while(|b| b != b'\n');
            } else {
                break;
            }
        }
    }

    // ── Number literal ────────────────────────────────────────────────────

    fn lex_number(&mut self, lo: usize) -> Result<Token, LexError> {
        self.skip_while(|b| b.is_ascii_digit());

        let is_float = self.peek() == Some(b'.') && self.peek2().map_or(false, |b| b.is_ascii_digit());
        if is_float {
            self.pos += 1; // consume '.'
            self.skip_while(|b| b.is_ascii_digit());
        }

        let text = self.slice(lo, self.pos);
        let hi = self.pos as u32;
        let lo32 = lo as u32;

        if is_float {
            let val: f64 = text.parse().map_err(|_| LexError::InvalidNumber(lo32, hi))?;
            Ok(Token::new(TokenKind::Float(val), lo32, hi))
        } else {
            let val: i64 = text.parse().map_err(|_| LexError::InvalidNumber(lo32, hi))?;
            Ok(Token::new(TokenKind::Int(val), lo32, hi))
        }
    }

    // ── String literal ────────────────────────────────────────────────────

    fn lex_string(&mut self, lo: usize) -> Result<Token, LexError> {
        let mut buf = String::new();
        loop {
            match self.peek() {
                None | Some(b'\n') => {
                    return Err(LexError::UnterminatedString(lo as u32, self.pos as u32));
                }
                Some(b'"') => {
                    self.pos += 1;
                    break;
                }
                Some(b'\\') => {
                    self.pos += 1;
                    match self.peek() {
                        Some(b'"')  => { self.pos += 1; buf.push('"');  }
                        Some(b'\\') => { self.pos += 1; buf.push('\\'); }
                        Some(b'n')  => { self.pos += 1; buf.push('\n'); }
                        Some(b't')  => { self.pos += 1; buf.push('\t'); }
                        Some(b'r')  => { self.pos += 1; buf.push('\r'); }
                        _ => {
                            // Unknown escape — emit as-is.
                            if let Some(b) = self.peek() {
                                self.pos += 1;
                                buf.push(b as char);
                            }
                        }
                    }
                }
                Some(b) => {
                    self.pos += 1;
                    buf.push(b as char);
                }
            }
        }
        Ok(Token::new(TokenKind::String(buf), lo as u32, self.pos as u32))
    }

    // ── Identifier / keyword ──────────────────────────────────────────────

    fn lex_ident(&mut self, lo: usize) -> Token {
        self.skip_while(|b| b.is_ascii_alphanumeric() || b == b'_');
        let text = self.slice(lo, self.pos);
        let hi = self.pos as u32;
        let lo32 = lo as u32;

        let kind = TokenKind::from_keyword(text)
            .unwrap_or_else(|| TokenKind::Ident(text.to_owned()));
        Token::new(kind, lo32, hi)
    }

    // ── Main tokenise entry point ─────────────────────────────────────────

    /// Tokenise the entire source and return a `Vec<Token>` terminated by `Eof`.
    pub fn tokenise(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_trivia();
            if self.at_end() {
                tokens.push(Token::new(TokenKind::Eof, self.pos as u32, self.pos as u32));
                break;
            }

            let lo = self.pos;
            let b = self.peek().unwrap();

            let tok = match b {
                // Single-char punctuation
                b'(' => { self.pos += 1; Token::new(TokenKind::LParen,   lo as u32, self.pos as u32) }
                b')' => { self.pos += 1; Token::new(TokenKind::RParen,   lo as u32, self.pos as u32) }
                b'{' => { self.pos += 1; Token::new(TokenKind::LBrace,   lo as u32, self.pos as u32) }
                b'}' => { self.pos += 1; Token::new(TokenKind::RBrace,   lo as u32, self.pos as u32) }
                b'[' => { self.pos += 1; Token::new(TokenKind::LBracket, lo as u32, self.pos as u32) }
                b']' => { self.pos += 1; Token::new(TokenKind::RBracket, lo as u32, self.pos as u32) }
                b',' => { self.pos += 1; Token::new(TokenKind::Comma,    lo as u32, self.pos as u32) }
                b':' => { self.pos += 1; Token::new(TokenKind::Colon,    lo as u32, self.pos as u32) }
                b'.' => { self.pos += 1; Token::new(TokenKind::Dot,      lo as u32, self.pos as u32) }
                b'?' => { self.pos += 1; Token::new(TokenKind::Question, lo as u32, self.pos as u32) }
                b'%' => { self.pos += 1; Token::new(TokenKind::Percent,  lo as u32, self.pos as u32) }
                b'*' => { self.pos += 1; Token::new(TokenKind::Star,     lo as u32, self.pos as u32) }
                b'+' => { self.pos += 1; Token::new(TokenKind::Plus,     lo as u32, self.pos as u32) }
                b'@' => { self.pos += 1; Token::new(TokenKind::At,       lo as u32, self.pos as u32) }

                // `_` — wildcard / discard
                b'_' => {
                    self.pos += 1;
                    // If the next char is alphanumeric, it's an identifier starting with `_`.
                    if self.peek().map_or(false, |b| b.is_ascii_alphanumeric() || b == b'_') {
                        self.lex_ident(lo)
                    } else {
                        Token::new(TokenKind::Under, lo as u32, self.pos as u32)
                    }
                }

                // `-` or `->`
                b'-' => {
                    self.pos += 1;
                    if self.peek() == Some(b'>') {
                        self.pos += 1;
                        Token::new(TokenKind::Arrow, lo as u32, self.pos as u32)
                    } else {
                        Token::new(TokenKind::Minus, lo as u32, self.pos as u32)
                    }
                }

                // `=` or `==`
                b'=' => {
                    self.pos += 1;
                    if self.peek() == Some(b'=') {
                        self.pos += 1;
                        Token::new(TokenKind::EqEq, lo as u32, self.pos as u32)
                    } else {
                        Token::new(TokenKind::Eq, lo as u32, self.pos as u32)
                    }
                }

                // `!` or `!=`
                b'!' => {
                    self.pos += 1;
                    if self.peek() == Some(b'=') {
                        self.pos += 1;
                        Token::new(TokenKind::BangEq, lo as u32, self.pos as u32)
                    } else {
                        Token::new(TokenKind::Bang, lo as u32, self.pos as u32)
                    }
                }

                // `<` or `<=`
                b'<' => {
                    self.pos += 1;
                    if self.peek() == Some(b'=') {
                        self.pos += 1;
                        Token::new(TokenKind::LtEq, lo as u32, self.pos as u32)
                    } else {
                        Token::new(TokenKind::Lt, lo as u32, self.pos as u32)
                    }
                }

                // `>` or `>=`
                b'>' => {
                    self.pos += 1;
                    if self.peek() == Some(b'=') {
                        self.pos += 1;
                        Token::new(TokenKind::GtEq, lo as u32, self.pos as u32)
                    } else {
                        Token::new(TokenKind::Gt, lo as u32, self.pos as u32)
                    }
                }

                // `&&`
                b'&' => {
                    self.pos += 1;
                    if self.peek() == Some(b'&') {
                        self.pos += 1;
                        Token::new(TokenKind::AndAnd, lo as u32, self.pos as u32)
                    } else {
                        Token::new(TokenKind::Amp, lo as u32, self.pos as u32)
                    }
                }

                // `||`
                b'|' => {
                    self.pos += 1;
                    if self.peek() == Some(b'|') {
                        self.pos += 1;
                        Token::new(TokenKind::OrOr, lo as u32, self.pos as u32)
                    } else {
                        return Err(LexError::UnexpectedChar(lo as u32, self.pos as u32, '|'));
                    }
                }

                // `/` (not `//` — that's already handled in skip_trivia)
                b'/' => {
                    self.pos += 1;
                    Token::new(TokenKind::Slash, lo as u32, self.pos as u32)
                }

                // String literal
                b'"' => {
                    self.pos += 1; // consume opening quote
                    self.lex_string(lo)?
                }

                // Number literal
                b if b.is_ascii_digit() => self.lex_number(lo)?,

                // Identifier or keyword
                b if b.is_ascii_alphabetic() || b == b'_' => self.lex_ident(lo),

                other => {
                    self.pos += 1;
                    return Err(LexError::UnexpectedChar(
                        lo as u32, self.pos as u32, other as char,
                    ));
                }
            };

            tokens.push(tok);
        }
        Ok(tokens)
    }
}

/// Convenience function: lex a source string and return all tokens.
pub fn lex(src: &str) -> Result<Vec<Token>, LexError> {
    Lexer::new(src).tokenise()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        lex(src).unwrap().into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn test_keywords() {
        let toks = kinds("fn struct class protocol impl for extends const let weak if else while in return yield match panic true false null");
        assert_eq!(toks[0],  TokenKind::Fn);
        assert_eq!(toks[1],  TokenKind::Struct);
        assert_eq!(toks[2],  TokenKind::Class);
        assert_eq!(toks[3],  TokenKind::Protocol);
        assert_eq!(toks[4],  TokenKind::Impl);
        assert_eq!(toks[5],  TokenKind::For);
        assert_eq!(toks[6],  TokenKind::Extends);
        assert_eq!(toks[7],  TokenKind::Const);
        assert_eq!(toks[8],  TokenKind::Let);
        assert_eq!(toks[9],  TokenKind::Weak);
        assert_eq!(toks[10], TokenKind::If);
        assert_eq!(toks[11], TokenKind::Else);
        assert_eq!(toks[12], TokenKind::While);
        assert_eq!(toks[13], TokenKind::In);
        assert_eq!(toks[14], TokenKind::Return);
        assert_eq!(toks[15], TokenKind::Yield);
        assert_eq!(toks[16], TokenKind::Match);
        assert_eq!(toks[17], TokenKind::Panic);
        assert_eq!(toks[18], TokenKind::True);
        assert_eq!(toks[19], TokenKind::False);
        assert_eq!(toks[20], TokenKind::Null);
    }

    #[test]
    fn test_operators() {
        let toks = kinds("== != <= >= -> && ||");
        assert_eq!(toks[0], TokenKind::EqEq);
        assert_eq!(toks[1], TokenKind::BangEq);
        assert_eq!(toks[2], TokenKind::LtEq);
        assert_eq!(toks[3], TokenKind::GtEq);
        assert_eq!(toks[4], TokenKind::Arrow);
        assert_eq!(toks[5], TokenKind::AndAnd);
        assert_eq!(toks[6], TokenKind::OrOr);
    }

    #[test]
    fn test_number_literals() {
        let toks = kinds("42 3.14");
        assert_eq!(toks[0], TokenKind::Int(42));
        assert_eq!(toks[1], TokenKind::Float(3.14));
    }

    #[test]
    fn test_string_literal() {
        let toks = kinds(r#""hello world""#);
        assert_eq!(toks[0], TokenKind::String("hello world".into()));
    }

    #[test]
    fn test_string_escape() {
        let toks = kinds(r#""say \"hi\"\\n""#);
        assert_eq!(toks[0], TokenKind::String(r#"say "hi"\n"#.into()));
    }

    #[test]
    fn test_comment_skipped() {
        let toks = kinds("42 // this is a comment\n99");
        assert_eq!(toks[0], TokenKind::Int(42));
        assert_eq!(toks[1], TokenKind::Int(99));
        assert_eq!(toks[2], TokenKind::Eof);
    }

    #[test]
    fn test_underscore_wildcard() {
        let toks = kinds("_ _foo");
        assert_eq!(toks[0], TokenKind::Under);
        assert_eq!(toks[1], TokenKind::Ident("_foo".into()));
    }

    #[test]
    fn test_arrow_vs_minus() {
        let toks = kinds("-> -");
        assert_eq!(toks[0], TokenKind::Arrow);
        assert_eq!(toks[1], TokenKind::Minus);
    }

    #[test]
    fn test_span() {
        let toks = lex("fn foo").unwrap();
        assert_eq!(toks[0].span.lo, 0);
        assert_eq!(toks[0].span.hi, 2);
        assert_eq!(toks[1].span.lo, 3);
        assert_eq!(toks[1].span.hi, 6);
    }
}
