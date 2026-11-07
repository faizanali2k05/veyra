use std::fmt;
use veyra_source::Span;

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    Integer(String),
    Float(String),
    String(String),
    Let,
    Mut,
    Fn,
    Struct,
    Import,
    Pub,
    If,
    Else,
    While,
    For,
    In,
    Return,
    Policy,
    Owner,
    Export,
    Deny,
    Private,
    True,
    False,
    IntType,
    FloatType,
    BoolType,
    StringType,
    UnitType,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    Equal,
    EqualEqual,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    AndAnd,
    OrOr,
    Arrow,
    Dot,
    DotDot,
    Colon,
    Semicolon,
    Comma,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Eof,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}
impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for LexError {}

/// Converts Veyra source into a spanned token stream.
///
/// # Errors
/// Returns an error for invalid characters, strings, escapes, or numbers.
pub fn lex(input: &str) -> Result<Vec<Token>, LexError> {
    Lexer {
        input,
        offset: 0,
        tokens: Vec::new(),
    }
    .run()
}

struct Lexer<'a> {
    input: &'a str,
    offset: usize,
    tokens: Vec<Token>,
}

impl Lexer<'_> {
    fn run(mut self) -> Result<Vec<Token>, LexError> {
        while let Some(ch) = self.peek() {
            let start = self.offset;
            match ch {
                c if c.is_whitespace() => {
                    self.bump();
                }
                '/' if self.peek_next() == Some('/') => {
                    while self.peek().is_some_and(|c| c != '\n') {
                        self.bump();
                    }
                }
                '0'..='9' => self.number(start),
                '"' => self.string(start)?,
                c if c == '_' || c.is_alphabetic() => self.identifier(start),
                '+' => self.single(TokenKind::Plus),
                '*' => self.single(TokenKind::Star),
                '%' => self.single(TokenKind::Percent),
                '(' => self.single(TokenKind::LeftParen),
                ')' => self.single(TokenKind::RightParen),
                '{' => self.single(TokenKind::LeftBrace),
                '}' => self.single(TokenKind::RightBrace),
                '[' => self.single(TokenKind::LeftBracket),
                ']' => self.single(TokenKind::RightBracket),
                ':' => self.single(TokenKind::Colon),
                ';' => self.single(TokenKind::Semicolon),
                ',' => self.single(TokenKind::Comma),
                '-' if self.peek_next() == Some('>') => self.double(TokenKind::Arrow),
                '-' => self.single(TokenKind::Minus),
                '/' => self.single(TokenKind::Slash),
                '!' if self.peek_next() == Some('=') => self.double(TokenKind::BangEqual),
                '!' => self.single(TokenKind::Bang),
                '=' if self.peek_next() == Some('=') => self.double(TokenKind::EqualEqual),
                '=' => self.single(TokenKind::Equal),
                '<' if self.peek_next() == Some('=') => self.double(TokenKind::LessEqual),
                '<' => self.single(TokenKind::Less),
                '>' if self.peek_next() == Some('=') => self.double(TokenKind::GreaterEqual),
                '>' => self.single(TokenKind::Greater),
                '&' if self.peek_next() == Some('&') => self.double(TokenKind::AndAnd),
                '|' if self.peek_next() == Some('|') => self.double(TokenKind::OrOr),
                '.' if self.peek_next() == Some('.') => self.double(TokenKind::DotDot),
                '.' => self.single(TokenKind::Dot),
                _ => {
                    return Err(LexError {
                        message: format!("invalid token `{ch}`"),
                        span: Span::new(start, start + ch.len_utf8()),
                    });
                }
            }
        }
        self.tokens.push(Token {
            kind: TokenKind::Eof,
            span: Span::new(self.input.len(), self.input.len()),
        });
        Ok(self.tokens)
    }
    fn peek(&self) -> Option<char> {
        self.input[self.offset..].chars().next()
    }
    fn peek_next(&self) -> Option<char> {
        self.input[self.offset..].chars().nth(1)
    }
    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.offset += ch.len_utf8();
        Some(ch)
    }
    fn push(&mut self, kind: TokenKind, start: usize) {
        self.tokens.push(Token {
            kind,
            span: Span::new(start, self.offset),
        });
    }
    fn single(&mut self, kind: TokenKind) {
        let start = self.offset;
        self.bump();
        self.push(kind, start);
    }
    fn double(&mut self, kind: TokenKind) {
        let start = self.offset;
        self.bump();
        self.bump();
        self.push(kind, start);
    }
    fn identifier(&mut self, start: usize) {
        self.bump();
        while self.peek().is_some_and(|c| c == '_' || c.is_alphanumeric()) {
            self.bump();
        }
        let text = &self.input[start..self.offset];
        let kind = match text {
            "let" => TokenKind::Let,
            "mut" => TokenKind::Mut,
            "fn" => TokenKind::Fn,
            "struct" => TokenKind::Struct,
            "import" => TokenKind::Import,
            "pub" => TokenKind::Pub,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "return" => TokenKind::Return,
            "policy" => TokenKind::Policy,
            "owner" => TokenKind::Owner,
            "export" => TokenKind::Export,
            "deny" => TokenKind::Deny,
            "private" => TokenKind::Private,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "int" => TokenKind::IntType,
            "float" => TokenKind::FloatType,
            "bool" => TokenKind::BoolType,
            "string" => TokenKind::StringType,
            "unit" => TokenKind::UnitType,
            _ => TokenKind::Identifier(text.to_owned()),
        };
        self.push(kind, start);
    }
    fn number(&mut self, start: usize) {
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.bump();
        }
        let mut float = false;
        if self.peek() == Some('.') && self.peek_next().is_some_and(|c| c.is_ascii_digit()) {
            float = true;
            self.bump();
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.bump();
            }
        }
        let text = self.input[start..self.offset].to_owned();
        self.push(
            if float {
                TokenKind::Float(text)
            } else {
                TokenKind::Integer(text)
            },
            start,
        );
    }
    fn string(&mut self, start: usize) -> Result<(), LexError> {
        self.bump();
        let mut value = String::new();
        loop {
            match self.bump() {
                Some('"') => break,
                Some('\\') => match self.bump() {
                    Some('n') => value.push('\n'),
                    Some('r') => value.push('\r'),
                    Some('t') => value.push('\t'),
                    Some('"') => value.push('"'),
                    Some('\\') => value.push('\\'),
                    Some(other) => {
                        return Err(LexError {
                            message: format!("invalid escape `\\{other}`"),
                            span: Span::new(self.offset - other.len_utf8() - 1, self.offset),
                        });
                    }
                    None => {
                        return Err(LexError {
                            message: "unterminated string literal".into(),
                            span: Span::new(start, self.offset),
                        });
                    }
                },
                Some(ch) => value.push(ch),
                None => {
                    return Err(LexError {
                        message: "unterminated string literal".into(),
                        span: Span::new(start, self.offset),
                    });
                }
            }
        }
        self.push(TokenKind::String(value), start);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comments_and_keywords() {
        let t = lex("let mut x = true; // hi").unwrap();
        assert!(matches!(t[0].kind, TokenKind::Let));
        assert!(matches!(t[1].kind, TokenKind::Mut));
    }
    #[test]
    fn literals_and_escapes() {
        let t = lex("12 2.5 \"a\\n\"").unwrap();
        assert_eq!(t[0].kind, TokenKind::Integer("12".into()));
        assert_eq!(t[1].kind, TokenKind::Float("2.5".into()));
        assert_eq!(t[2].kind, TokenKind::String("a\n".into()));
    }
    #[test]
    fn invalid_token() {
        assert_eq!(lex("@").unwrap_err().span, Span::new(0, 1));
    }
    #[test]
    fn unterminated_string() {
        assert!(lex("\"no").unwrap_err().message.contains("unterminated"));
    }
}
