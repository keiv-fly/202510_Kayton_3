use crate::diagnostics::Diagnostic;
use crate::span::{SourceId, Span};
use smol_str::SmolStr;

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Identifier(SmolStr),
    Int(SmolStr),
    String(SmolStr),
    Keyword(Keyword),
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Dot,
    Question,
    DoubleColon,
    Arrow,
    FatArrow,
    Equal,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    EqEq,
    BangEq,
    Lt,
    Gt,
    Le,
    Ge,
    Newline,
    Indent,
    Dedent,
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Fn,
    Let,
    Return,
    If,
    Elif,
    Else,
    While,
    True,
    False,
    Use,
    Struct,
    Enum,
    For,
    In,
    Break,
    Continue,
    As,
    Where,
    Yield,
}

pub fn lex(source: &str, source_id: SourceId) -> (Vec<Token>, Vec<Diagnostic>) {
    let mut lexer = Lexer::new(source, source_id);
    lexer.lex()
}

struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
    source_id: SourceId,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
    indent_stack: Vec<usize>,
    pending_dedents: Vec<Token>,
    line_start: bool,
    nesting: usize,
    follower: bool,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str, source_id: SourceId) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
            source_id,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
            indent_stack: vec![0],
            pending_dedents: Vec::new(),
            line_start: true,
            nesting: 0,
            follower: false,
        }
    }

    fn lex(&mut self) -> (Vec<Token>, Vec<Diagnostic>) {
        while self.pos < self.bytes.len() || !self.pending_dedents.is_empty() {
            if let Some(token) = self.pending_dedents.pop() {
                self.tokens.push(token);
                continue;
            }

            if self.line_start {
                if let Some(token) = self.consume_indentation() {
                    self.tokens.push(token);
                    continue;
                }
            }

            if self.pos >= self.bytes.len() {
                break;
            }

            let ch = self.current_char();
            if ch == '\n' {
                let start = self.pos;
                self.pos += 1;
                self.line_start = true;
                if self.nesting > 0 || self.follower {
                    self.follower = false;
                    continue;
                }
                let span = Span::new(self.source_id, start, self.pos);
                self.tokens.push(Token {
                    kind: TokenKind::Newline,
                    span,
                });
                continue;
            }

            if ch == '\r' {
                self.pos += 1;
                continue;
            }

            if self.line_start && ch == '#' {
                self.skip_comment();
                continue;
            }

            if ch.is_ascii_whitespace() {
                self.pos += 1;
                continue;
            }

            if ch == '#' {
                self.skip_comment();
                continue;
            }

            if let Some(token) = self.lex_number_or_identifier() {
                self.tokens.push(token);
                continue;
            }

            if ch == '"' {
                match self.lex_string() {
                    Some(token) => {
                        self.tokens.push(token);
                    }
                    None => {}
                }
                continue;
            }

            let start = self.pos;
            let token = match ch {
                '(' => {
                    self.pos += 1;
                    self.nesting += 1;
                    TokenKind::LParen
                }
                ')' => {
                    self.pos += 1;
                    if self.nesting > 0 {
                        self.nesting -= 1;
                    }
                    TokenKind::RParen
                }
                '{' => {
                    self.pos += 1;
                    self.nesting += 1;
                    TokenKind::LBrace
                }
                '}' => {
                    self.pos += 1;
                    if self.nesting > 0 {
                        self.nesting -= 1;
                    }
                    TokenKind::RBrace
                }
                '[' => {
                    self.pos += 1;
                    self.nesting += 1;
                    TokenKind::LBracket
                }
                ']' => {
                    self.pos += 1;
                    if self.nesting > 0 {
                        self.nesting -= 1;
                    }
                    TokenKind::RBracket
                }
                ':' => {
                    self.pos += 1;
                    if self.peek_char() == Some(':') {
                        self.pos += 1;
                        TokenKind::DoubleColon
                    } else {
                        TokenKind::Colon
                    }
                }
                '.' => {
                    self.pos += 1;
                    TokenKind::Dot
                }
                '?' => {
                    self.pos += 1;
                    TokenKind::Question
                }
                ',' => {
                    self.pos += 1;
                    TokenKind::Comma
                }
                '=' => {
                    self.pos += 1;
                    if self.peek_char() == Some('=') {
                        self.pos += 1;
                        TokenKind::EqEq
                    } else if self.peek_char() == Some('>') {
                        self.pos += 1;
                        TokenKind::FatArrow
                    } else {
                        TokenKind::Equal
                    }
                }
                '-' => {
                    self.pos += 1;
                    if self.peek_char() == Some('>') {
                        self.pos += 1;
                        TokenKind::Arrow
                    } else {
                        TokenKind::Minus
                    }
                }
                '+' => {
                    self.pos += 1;
                    TokenKind::Plus
                }
                '*' => {
                    self.pos += 1;
                    TokenKind::Star
                }
                '/' => {
                    self.pos += 1;
                    TokenKind::Slash
                }
                '%' => {
                    self.pos += 1;
                    TokenKind::Percent
                }
                '!' => {
                    self.pos += 1;
                    if self.peek_char() == Some('=') {
                        self.pos += 1;
                        TokenKind::BangEq
                    } else {
                        TokenKind::Bang
                    }
                }
                '<' => {
                    self.pos += 1;
                    if self.peek_char() == Some('=') {
                        self.pos += 1;
                        TokenKind::Le
                    } else {
                        TokenKind::Lt
                    }
                }
                '>' => {
                    self.pos += 1;
                    if self.peek_char() == Some('=') {
                        self.pos += 1;
                        TokenKind::Ge
                    } else {
                        TokenKind::Gt
                    }
                }
                _ => {
                    self.pos += 1;
                    let span = Span::new(self.source_id, start, self.pos);
                    self.diagnostics
                        .push(Diagnostic::error("unexpected character", span));
                    continue;
                }
            };
            let span = Span::new(self.source_id, start, self.pos);
            self.follower = is_follower(&token);
            self.tokens.push(Token { kind: token, span });
        }

        while self.indent_stack.len() > 1 {
            self.indent_stack.pop();
            let span = Span::new(self.source_id, self.src.len(), self.src.len());
            self.tokens.push(Token {
                kind: TokenKind::Dedent,
                span,
            });
        }

        let eof_span = Span::new(self.source_id, self.src.len(), self.src.len());
        self.tokens.push(Token {
            kind: TokenKind::Eof,
            span: eof_span,
        });

        (
            std::mem::take(&mut self.tokens),
            std::mem::take(&mut self.diagnostics),
        )
    }

    fn consume_indentation(&mut self) -> Option<Token> {
        let mut idx = self.pos;
        let mut indent = 0usize;
        while idx < self.bytes.len() {
            match self.bytes[idx] as char {
                ' ' => {
                    indent += 1;
                    idx += 1;
                }
                '\t' => {
                    let span = Span::new(self.source_id, idx, idx + 1);
                    self.diagnostics.push(Diagnostic::error(
                        "tabs are not allowed for indentation",
                        span,
                    ));
                    indent += 4;
                    idx += 1;
                }
                '\r' => {
                    idx += 1;
                }
                _ => break,
            }
        }

        if idx >= self.bytes.len() {
            self.pos = idx;
            self.line_start = false;
            return None;
        }

        match self.bytes[idx] as char {
            '\n' => {
                self.pos = idx;
                return None;
            }
            '#' => {
                self.pos = idx;
                self.skip_comment();
                return None;
            }
            _ => {}
        }

        self.pos = idx;
        self.line_start = false;
        let current = *self.indent_stack.last().unwrap();
        if indent > current {
            self.indent_stack.push(indent);
            let span = Span::new(self.source_id, self.pos, self.pos);
            return Some(Token {
                kind: TokenKind::Indent,
                span,
            });
        }
        if indent < current {
            while indent < *self.indent_stack.last().unwrap() {
                self.indent_stack.pop();
                let span = Span::new(self.source_id, self.pos, self.pos);
                self.pending_dedents.push(Token {
                    kind: TokenKind::Dedent,
                    span,
                });
            }
            if indent != *self.indent_stack.last().unwrap() {
                let span = Span::new(self.source_id, self.pos, self.pos);
                self.diagnostics.push(
                    Diagnostic::error("invalid indentation level", span)
                        .with_note("indentation must match a previous level"),
                );
                self.indent_stack.truncate(1);
            }
            if let Some(token) = self.pending_dedents.pop() {
                return Some(token);
            }
        }
        None
    }

    fn lex_number_or_identifier(&mut self) -> Option<Token> {
        let ch = self.current_char();
        if is_ident_start(ch) {
            return Some(self.lex_identifier());
        }
        if ch.is_ascii_digit() {
            return Some(self.lex_number());
        }
        None
    }

    fn lex_identifier(&mut self) -> Token {
        let start = self.pos;
        self.pos += 1;
        while self.pos < self.bytes.len() {
            let ch = self.bytes[self.pos] as char;
            if is_ident_continue(ch) {
                self.pos += 1;
            } else {
                break;
            }
        }
        let slice = &self.src[start..self.pos];
        let span = Span::new(self.source_id, start, self.pos);
        if let Some(keyword) = keyword(slice) {
            let token_kind = TokenKind::Keyword(keyword);
            self.follower = is_keyword_follower(keyword);
            Token {
                kind: token_kind,
                span,
            }
        } else {
            self.follower = false;
            Token {
                kind: TokenKind::Identifier(SmolStr::new(slice)),
                span,
            }
        }
    }

    fn lex_number(&mut self) -> Token {
        let start = self.pos;
        self.pos += 1;
        while self.pos < self.bytes.len() {
            let ch = self.bytes[self.pos] as char;
            if ch.is_ascii_digit() || ch == '_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let slice = &self.src[start..self.pos];
        let span = Span::new(self.source_id, start, self.pos);
        self.follower = false;
        Token {
            kind: TokenKind::Int(SmolStr::new(slice)),
            span,
        }
    }

    fn lex_string(&mut self) -> Option<Token> {
        let start = self.pos;
        self.pos += 1;
        let mut value = String::new();
        while self.pos < self.bytes.len() {
            let ch = self.bytes[self.pos] as char;
            if ch == '"' {
                self.pos += 1;
                let span = Span::new(self.source_id, start, self.pos);
                self.follower = false;
                return Some(Token {
                    kind: TokenKind::String(SmolStr::from(value)),
                    span,
                });
            }
            if ch == '\\' {
                self.pos += 1;
                if self.pos >= self.bytes.len() {
                    break;
                }
                let escape = self.bytes[self.pos] as char;
                value.push(match escape {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    '"' => '"',
                    '\\' => '\\',
                    other => other,
                });
                self.pos += 1;
                continue;
            }
            value.push(ch);
            self.pos += 1;
        }
        let span = Span::new(self.source_id, start, self.pos);
        self.diagnostics
            .push(Diagnostic::error("unterminated string literal", span));
        None
    }

    fn skip_comment(&mut self) {
        while self.pos < self.bytes.len() {
            let ch = self.bytes[self.pos] as char;
            self.pos += 1;
            if ch == '\n' {
                self.line_start = true;
                if self.nesting == 0 && !self.follower {
                    let span = Span::new(self.source_id, self.pos - 1, self.pos);
                    self.tokens.push(Token {
                        kind: TokenKind::Newline,
                        span,
                    });
                }
                break;
            }
        }
    }

    fn current_char(&self) -> char {
        self.bytes[self.pos] as char
    }

    fn peek_char(&self) -> Option<char> {
        self.bytes.get(self.pos + 1).map(|b| *b as char)
    }
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit()
}

fn keyword(text: &str) -> Option<Keyword> {
    match text {
        "fn" => Some(Keyword::Fn),
        "let" => Some(Keyword::Let),
        "return" => Some(Keyword::Return),
        "if" => Some(Keyword::If),
        "elif" => Some(Keyword::Elif),
        "else" => Some(Keyword::Else),
        "while" => Some(Keyword::While),
        "true" => Some(Keyword::True),
        "false" => Some(Keyword::False),
        "use" => Some(Keyword::Use),
        "struct" => Some(Keyword::Struct),
        "enum" => Some(Keyword::Enum),
        "for" => Some(Keyword::For),
        "in" => Some(Keyword::In),
        "break" => Some(Keyword::Break),
        "continue" => Some(Keyword::Continue),
        "as" => Some(Keyword::As),
        "where" => Some(Keyword::Where),
        "yield" => Some(Keyword::Yield),
        _ => None,
    }
}

fn is_follower(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Dot
            | TokenKind::Question
            | TokenKind::DoubleColon
            | TokenKind::Equal
            | TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Percent
            | TokenKind::Arrow
            | TokenKind::FatArrow
    )
}

fn is_keyword_follower(keyword: Keyword) -> bool {
    matches!(
        keyword,
        Keyword::Return
            | Keyword::Break
            | Keyword::Continue
            | Keyword::Yield
            | Keyword::As
            | Keyword::Where
    )
}
