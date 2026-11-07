use std::{fmt, mem::discriminant};
use veyra_ast::*;
use veyra_lexer::{Token, TokenKind};
use veyra_source::Span;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ParseError {}

/// Parses exactly one complete expression.
///
/// # Errors
/// Returns an error when the token stream does not form one expression.
pub fn parse_expression(tokens: Vec<Token>) -> Result<Expr, ParseError> {
    let mut parser = Parser::new(tokens);
    let expression = parser.expression(0)?;
    parser.expect(&TokenKind::Eof, "end of expression")?;
    Ok(expression)
}

/// Parses a complete Veyra compilation unit.
///
/// # Errors
/// Returns the first syntax error with its precise source span.
pub fn parse_program(tokens: Vec<Token>) -> Result<Program, ParseError> {
    let mut parser = Parser::new(tokens);
    let mut declarations = Vec::new();
    while !parser.at(&TokenKind::Eof) {
        declarations.push(parser.declaration()?);
    }
    Ok(Program { declarations })
}

struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    fn new(mut tokens: Vec<Token>) -> Self {
        if !tokens
            .last()
            .is_some_and(|t| same(&t.kind, &TokenKind::Eof))
        {
            let end = tokens.last().map_or(0, |t| t.span.end);
            tokens.push(Token {
                kind: TokenKind::Eof,
                span: Span::new(end, end),
            });
        }
        Self { tokens, current: 0 }
    }
    fn token(&self) -> &Token {
        &self.tokens[self.current.min(self.tokens.len() - 1)]
    }
    fn previous_span(&self) -> Span {
        self.tokens[self.current.saturating_sub(1)].span
    }
    fn at(&self, kind: &TokenKind) -> bool {
        same(&self.token().kind, kind)
    }
    fn advance(&mut self) -> Token {
        let token = self.token().clone();
        if !self.at(&TokenKind::Eof) {
            self.current += 1;
        }
        token
    }
    fn take(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, kind: &TokenKind, expected: &str) -> Result<Token, ParseError> {
        if self.at(kind) {
            Ok(self.advance())
        } else {
            Err(self.error(format!(
                "expected {expected}, found {}",
                describe(&self.token().kind)
            )))
        }
    }
    fn error(&self, message: String) -> ParseError {
        ParseError {
            message,
            span: self.token().span,
        }
    }
    fn identifier(&mut self, expected: &str) -> Result<(String, Span), ParseError> {
        let token = self.advance();
        if let TokenKind::Identifier(name) = token.kind {
            Ok((name, token.span))
        } else {
            Err(ParseError {
                message: format!("expected {expected}, found {}", describe(&token.kind)),
                span: token.span,
            })
        }
    }

    fn declaration(&mut self) -> Result<Decl, ParseError> {
        let public = self.take(&TokenKind::Pub);
        if self.take(&TokenKind::Fn) {
            return self.function(public).map(Decl::Function);
        }
        if self.take(&TokenKind::Struct) {
            return self.struct_decl(public).map(Decl::Struct);
        }
        if self.take(&TokenKind::Policy) {
            if public {
                return Err(self.error("policies cannot be declared `pub`".into()));
            }
            return self.policy_decl().map(Decl::Policy);
        }
        if public {
            return Err(self.error("`pub` is only valid on functions and structs".into()));
        }
        if self.take(&TokenKind::Import) {
            let (name, start) = self.identifier("module name")?;
            let end = self.expect(&TokenKind::Semicolon, "`;`")?.span;
            return Ok(Decl::Import(ImportDecl {
                name,
                span: start.merge(end),
            }));
        }
        if self.take(&TokenKind::Let) {
            return self.let_statement().map(Decl::Global);
        }
        Err(self.error("expected top-level declaration".into()))
    }

    fn policy_decl(&mut self) -> Result<PolicyDecl, ParseError> {
        let (name, start) = self.identifier("policy name")?;
        self.expect(&TokenKind::LeftBrace, "`{`")?;
        self.expect(&TokenKind::Owner, "`owner`")?;
        self.expect(&TokenKind::Colon, "`:`")?;
        let owner_token = self.advance();
        let TokenKind::String(owner) = owner_token.kind else {
            return Err(ParseError {
                message: "policy owner must be a string literal".into(),
                span: owner_token.span,
            });
        };
        self.expect(&TokenKind::Semicolon, "`;`")?;
        self.expect(&TokenKind::Export, "`export`")?;
        self.expect(&TokenKind::Colon, "`:`")?;
        self.expect(&TokenKind::Deny, "`deny`")?;
        self.expect(&TokenKind::Semicolon, "`;`")?;
        let end = self.expect(&TokenKind::RightBrace, "`}`")?.span;
        Ok(PolicyDecl {
            name,
            owner,
            export_denied: true,
            span: start.merge(end),
        })
    }

    fn function(&mut self, public: bool) -> Result<Function, ParseError> {
        let (name, start) = self.identifier("function name")?;
        self.expect(&TokenKind::LeftParen, "`(`")?;
        let mut params = Vec::new();
        if !self.at(&TokenKind::RightParen) {
            loop {
                let (param, span) = self.identifier("parameter name")?;
                self.expect(&TokenKind::Colon, "`:`")?;
                let ty = self.ty()?;
                params.push(Parameter {
                    name: param,
                    ty,
                    span,
                });
                if !self.take(&TokenKind::Comma) {
                    break;
                }
                if self.at(&TokenKind::RightParen) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RightParen, "`)`")?;
        let return_type = if self.take(&TokenKind::Arrow) {
            self.ty()?
        } else {
            Type::Unit
        };
        let (body, end) = self.block()?;
        Ok(Function {
            public,
            name,
            params,
            return_type,
            body,
            span: start.merge(end),
        })
    }

    fn struct_decl(&mut self, public: bool) -> Result<StructDecl, ParseError> {
        let (name, start) = self.identifier("struct name")?;
        self.expect(&TokenKind::LeftBrace, "`{`")?;
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RightBrace) {
            let (field, span) = self.identifier("field name")?;
            self.expect(&TokenKind::Colon, "`:`")?;
            fields.push(Parameter {
                name: field,
                ty: self.ty()?,
                span,
            });
            self.expect(&TokenKind::Comma, "`,`")?;
        }
        let end = self.expect(&TokenKind::RightBrace, "`}`")?.span;
        Ok(StructDecl {
            public,
            name,
            fields,
            span: start.merge(end),
        })
    }

    fn ty(&mut self) -> Result<Type, ParseError> {
        let token = self.advance();
        match token.kind {
            TokenKind::IntType => Ok(Type::Int),
            TokenKind::FloatType => Ok(Type::Float),
            TokenKind::BoolType => Ok(Type::Bool),
            TokenKind::StringType => Ok(Type::String),
            TokenKind::UnitType => Ok(Type::Unit),
            TokenKind::Identifier(name) => Ok(Type::Named(name)),
            TokenKind::LeftBracket => {
                let item = self.ty()?;
                self.expect(&TokenKind::RightBracket, "`]`")?;
                Ok(Type::Array(Box::new(item)))
            }
            _ => Err(ParseError {
                message: format!("expected type, found {}", describe(&token.kind)),
                span: token.span,
            }),
        }
    }

    fn block(&mut self) -> Result<(Block, Span), ParseError> {
        self.expect(&TokenKind::LeftBrace, "`{`")?;
        let mut statements = Vec::new();
        while !self.at(&TokenKind::RightBrace) {
            if self.at(&TokenKind::Eof) {
                return Err(self.error("expected `}`, found end of input".into()));
            }
            statements.push(self.statement()?);
        }
        let end = self.advance().span;
        Ok((statements, end))
    }

    fn statement(&mut self) -> Result<Stmt, ParseError> {
        if self.take(&TokenKind::Let) {
            return self.let_statement().map(Stmt::Let);
        }
        if self.take(&TokenKind::If) {
            return self.if_statement();
        }
        if self.take(&TokenKind::While) {
            return self.while_statement();
        }
        if self.take(&TokenKind::For) {
            return self.for_statement();
        }
        if self.take(&TokenKind::Return) {
            let start = self.previous_span();
            let value = if self.at(&TokenKind::Semicolon) {
                None
            } else {
                Some(self.expression(0)?)
            };
            let end = self.expect(&TokenKind::Semicolon, "`;`")?.span;
            return Ok(Stmt::Return {
                value,
                span: start.merge(end),
            });
        }
        let expression = self.expression(0)?;
        if self.take(&TokenKind::Equal) {
            let value = self.expression(0)?;
            let end = self.expect(&TokenKind::Semicolon, "`;`")?.span;
            let span = expression.span.merge(end);
            Ok(Stmt::Assign {
                target: expression,
                value,
                span,
            })
        } else {
            self.expect(&TokenKind::Semicolon, "`;`")?;
            Ok(Stmt::Expr(expression))
        }
    }

    fn let_statement(&mut self) -> Result<LetStmt, ParseError> {
        let start = self.previous_span();
        let mutable = self.take(&TokenKind::Mut);
        let (name, _) = self.identifier("binding name")?;
        let annotation = if self.take(&TokenKind::Colon) {
            Some(self.ty()?)
        } else {
            None
        };
        self.expect(&TokenKind::Equal, "`=`")?;
        let initializer = self.expression(0)?;
        let end = self.expect(&TokenKind::Semicolon, "`;`")?.span;
        Ok(LetStmt {
            name,
            mutable,
            annotation,
            initializer,
            span: start.merge(end),
        })
    }

    fn if_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.previous_span();
        let condition = self.expression(0)?;
        let (then_branch, mut end) = self.block()?;
        let else_branch = if self.take(&TokenKind::Else) {
            if self.take(&TokenKind::If) {
                let nested = self.if_statement()?;
                end = stmt_span(&nested);
                vec![nested]
            } else {
                let (body, close) = self.block()?;
                end = close;
                body
            }
        } else {
            Vec::new()
        };
        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
            span: start.merge(end),
        })
    }
    fn while_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.previous_span();
        let condition = self.expression(0)?;
        let (body, end) = self.block()?;
        Ok(Stmt::While {
            condition,
            body,
            span: start.merge(end),
        })
    }
    fn for_statement(&mut self) -> Result<Stmt, ParseError> {
        let start = self.previous_span();
        let (name, _) = self.identifier("loop binding")?;
        self.expect(&TokenKind::In, "`in`")?;
        let iterable = self.expression(0)?;
        let (body, end) = self.block()?;
        Ok(Stmt::For {
            name,
            iterable,
            body,
            span: start.merge(end),
        })
    }

    fn expression(&mut self, minimum: u8) -> Result<Expr, ParseError> {
        let mut left = self.prefix()?;
        loop {
            if let Some(next) = self.postfix(left.clone())? {
                left = next;
                continue;
            }
            let Some((precedence, op)) = infix(&self.token().kind) else {
                break;
            };
            if precedence < minimum {
                break;
            }
            self.advance();
            let right = self.expression(precedence + 1)?;
            let span = left.span.merge(right.span);
            left = Expr {
                kind: ExprKind::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(left)
    }

    fn prefix(&mut self) -> Result<Expr, ParseError> {
        let token = self.advance();
        let (kind, span) = match token.kind {
            TokenKind::Integer(text) => (
                ExprKind::Integer(text.parse().map_err(|_| ParseError {
                    message: format!("integer literal `{text}` is outside the int range"),
                    span: token.span,
                })?),
                token.span,
            ),
            TokenKind::Float(text) => (
                ExprKind::Float(text.parse().map_err(|_| ParseError {
                    message: format!("invalid float literal `{text}`"),
                    span: token.span,
                })?),
                token.span,
            ),
            TokenKind::String(value) => (ExprKind::String(value), token.span),
            TokenKind::True => (ExprKind::Bool(true), token.span),
            TokenKind::False => (ExprKind::Bool(false), token.span),
            TokenKind::Identifier(name) => {
                if self.at(&TokenKind::LeftBrace) && self.looks_like_struct_literal() {
                    return self.struct_literal(name, token.span);
                }
                (ExprKind::Variable(name), token.span)
            }
            TokenKind::Minus | TokenKind::Bang => {
                let op = if same(&token.kind, &TokenKind::Minus) {
                    UnaryOp::Negate
                } else {
                    UnaryOp::Not
                };
                let operand = self.expression(15)?;
                let span = token.span.merge(operand.span);
                (
                    ExprKind::Unary {
                        op,
                        operand: Box::new(operand),
                    },
                    span,
                )
            }
            TokenKind::LeftParen => {
                if self.take(&TokenKind::RightParen) {
                    return Ok(Expr {
                        kind: ExprKind::Unit,
                        span: token.span.merge(self.previous_span()),
                    });
                }
                let mut expression = self.expression(0)?;
                let close = self.expect(&TokenKind::RightParen, "`)`")?;
                expression.span = token.span.merge(close.span);
                return Ok(expression);
            }
            TokenKind::LeftBracket => {
                let mut items = Vec::new();
                if !self.at(&TokenKind::RightBracket) {
                    loop {
                        items.push(self.expression(0)?);
                        if !self.take(&TokenKind::Comma) {
                            break;
                        }
                        if self.at(&TokenKind::RightBracket) {
                            break;
                        }
                    }
                }
                let close = self.expect(&TokenKind::RightBracket, "`]`")?;
                (ExprKind::Array(items), token.span.merge(close.span))
            }
            TokenKind::Private => {
                self.expect(&TokenKind::Less, "`<`")?;
                let (policy, _) = self.identifier("policy name")?;
                self.expect(&TokenKind::Greater, "`>`")?;
                self.expect(&TokenKind::LeftParen, "`(`")?;
                let value = self.expression(0)?;
                let close = self.expect(&TokenKind::RightParen, "`)`")?;
                (
                    ExprKind::Private {
                        policy,
                        value: Box::new(value),
                    },
                    token.span.merge(close.span),
                )
            }
            other => {
                return Err(ParseError {
                    message: format!("expected expression, found {}", describe(&other)),
                    span: token.span,
                });
            }
        };
        Ok(Expr { kind, span })
    }

    fn postfix(&mut self, object: Expr) -> Result<Option<Expr>, ParseError> {
        if self.take(&TokenKind::LeftParen) {
            let mut arguments = Vec::new();
            if !self.at(&TokenKind::RightParen) {
                loop {
                    arguments.push(self.expression(0)?);
                    if !self.take(&TokenKind::Comma) {
                        break;
                    }
                }
            }
            let close = self.expect(&TokenKind::RightParen, "`)`")?;
            let span = object.span.merge(close.span);
            return Ok(Some(Expr {
                kind: ExprKind::Call {
                    callee: Box::new(object),
                    arguments,
                },
                span,
            }));
        }
        if self.take(&TokenKind::Dot) {
            let (name, end) = self.identifier("field name")?;
            let span = object.span.merge(end);
            return Ok(Some(Expr {
                kind: ExprKind::Get {
                    object: Box::new(object),
                    name,
                },
                span,
            }));
        }
        if self.take(&TokenKind::LeftBracket) {
            let index = self.expression(0)?;
            let close = self.expect(&TokenKind::RightBracket, "`]`")?;
            let span = object.span.merge(close.span);
            return Ok(Some(Expr {
                kind: ExprKind::Index {
                    object: Box::new(object),
                    index: Box::new(index),
                },
                span,
            }));
        }
        Ok(None)
    }
    fn looks_like_struct_literal(&self) -> bool {
        let first = self.tokens.get(self.current + 1);
        let second = self.tokens.get(self.current + 2);
        first.is_some_and(|t| matches!(t.kind, TokenKind::RightBrace))
            || (first.is_some_and(|t| matches!(t.kind, TokenKind::Identifier(_)))
                && second.is_some_and(|t| matches!(t.kind, TokenKind::Colon)))
    }
    fn struct_literal(&mut self, name: String, start: Span) -> Result<Expr, ParseError> {
        self.expect(&TokenKind::LeftBrace, "`{`")?;
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RightBrace) {
            let (field, _) = self.identifier("field name")?;
            self.expect(&TokenKind::Colon, "`:`")?;
            fields.push((field, self.expression(0)?));
            if !self.take(&TokenKind::Comma) {
                break;
            }
        }
        let close = self.expect(&TokenKind::RightBrace, "`}`")?;
        Ok(Expr {
            kind: ExprKind::Struct { name, fields },
            span: start.merge(close.span),
        })
    }
}

fn same(left: &TokenKind, right: &TokenKind) -> bool {
    discriminant(left) == discriminant(right)
}
fn infix(kind: &TokenKind) -> Option<(u8, BinaryOp)> {
    Some(match kind {
        TokenKind::OrOr => (1, BinaryOp::Or),
        TokenKind::AndAnd => (3, BinaryOp::And),
        TokenKind::EqualEqual => (5, BinaryOp::Equal),
        TokenKind::BangEqual => (5, BinaryOp::NotEqual),
        TokenKind::Less => (7, BinaryOp::Less),
        TokenKind::LessEqual => (7, BinaryOp::LessEqual),
        TokenKind::Greater => (7, BinaryOp::Greater),
        TokenKind::GreaterEqual => (7, BinaryOp::GreaterEqual),
        TokenKind::DotDot => (9, BinaryOp::Range),
        TokenKind::Plus => (11, BinaryOp::Add),
        TokenKind::Minus => (11, BinaryOp::Subtract),
        TokenKind::Star => (13, BinaryOp::Multiply),
        TokenKind::Slash => (13, BinaryOp::Divide),
        TokenKind::Percent => (13, BinaryOp::Remainder),
        _ => return None,
    })
}
fn describe(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Identifier(v) => format!("identifier `{v}`"),
        TokenKind::Integer(v) => format!("integer `{v}`"),
        TokenKind::Float(v) => format!("float `{v}`"),
        TokenKind::String(_) => "string".into(),
        TokenKind::Eof => "end of input".into(),
        other => format!("`{}`", token_text(other)),
    }
}
fn token_text(kind: &TokenKind) -> &'static str {
    match kind {
        TokenKind::Let => "let",
        TokenKind::Mut => "mut",
        TokenKind::Fn => "fn",
        TokenKind::Struct => "struct",
        TokenKind::Import => "import",
        TokenKind::Pub => "pub",
        TokenKind::If => "if",
        TokenKind::Else => "else",
        TokenKind::While => "while",
        TokenKind::For => "for",
        TokenKind::In => "in",
        TokenKind::Return => "return",
        TokenKind::Policy => "policy",
        TokenKind::Owner => "owner",
        TokenKind::Export => "export",
        TokenKind::Deny => "deny",
        TokenKind::Private => "private",
        TokenKind::True => "true",
        TokenKind::False => "false",
        TokenKind::IntType => "int",
        TokenKind::FloatType => "float",
        TokenKind::BoolType => "bool",
        TokenKind::StringType => "string",
        TokenKind::UnitType => "unit",
        TokenKind::Plus => "+",
        TokenKind::Minus => "-",
        TokenKind::Star => "*",
        TokenKind::Slash => "/",
        TokenKind::Percent => "%",
        TokenKind::Bang => "!",
        TokenKind::Equal => "=",
        TokenKind::EqualEqual => "==",
        TokenKind::BangEqual => "!=",
        TokenKind::Less => "<",
        TokenKind::LessEqual => "<=",
        TokenKind::Greater => ">",
        TokenKind::GreaterEqual => ">=",
        TokenKind::AndAnd => "&&",
        TokenKind::OrOr => "||",
        TokenKind::Arrow => "->",
        TokenKind::Dot => ".",
        TokenKind::DotDot => "..",
        TokenKind::Colon => ":",
        TokenKind::Semicolon => ";",
        TokenKind::Comma => ",",
        TokenKind::LeftParen => "(",
        TokenKind::RightParen => ")",
        TokenKind::LeftBrace => "{",
        TokenKind::RightBrace => "}",
        TokenKind::LeftBracket => "[",
        TokenKind::RightBracket => "]",
        _ => "token",
    }
}
fn stmt_span(stmt: &Stmt) -> Span {
    match stmt {
        Stmt::Let(v) => v.span,
        Stmt::Expr(v) => v.span,
        Stmt::Assign { span, .. }
        | Stmt::If { span, .. }
        | Stmt::While { span, .. }
        | Stmt::For { span, .. }
        | Stmt::Return { span, .. } => *span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use veyra_lexer::lex;
    #[test]
    fn arithmetic_expression() {
        assert!(parse_expression(lex("2 + 3 * 4").unwrap()).is_ok());
    }
    #[test]
    fn parses_full_program() {
        let src = "struct User { name: string, } fn main() { let mut x: int = 0; while x < 2 { x = x + 1; } println(x); }";
        let p = parse_program(lex(src).unwrap()).unwrap();
        assert_eq!(p.declarations.len(), 2);
    }
    #[test]
    fn malformed_expression() {
        assert!(parse_expression(lex("2 + * 3").unwrap()).is_err());
    }

    #[test]
    fn frontend_survives_ascii_mutations() {
        for byte in 0_u8..=127 {
            let input = format!("fn main() {{ let x = 1; {} }}", char::from(byte));
            if let Ok(tokens) = lex(&input) {
                let _ = parse_program(tokens);
            }
        }
    }

    proptest! {
        #[test]
        fn frontend_never_panics_for_arbitrary_utf8(source in any::<String>()) {
            if let Ok(tokens) = lex(&source) {
                let _ = parse_program(tokens);
            }
        }
    }

    #[test]
    fn parses_policy_and_private_value() {
        let source = "policy HospitalPrivate { owner: \"hospital-a\"; export: deny; } fn main() { let patient = private<HospitalPrivate>(100); }";
        let program = parse_program(lex(source).unwrap()).unwrap();
        assert!(matches!(program.declarations[0], Decl::Policy(_)));
    }
}
