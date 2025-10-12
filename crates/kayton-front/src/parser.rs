use crate::ast::*;
use crate::diagnostics::Diagnostic;
use crate::lexer::{Keyword, Token, TokenKind};
use crate::span::Span;
use smol_str::SmolStr;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<Diagnostic>,
    last_span: Option<Span>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>, _source: crate::span::SourceId) -> Self {
        Self {
            tokens,
            pos: 0,
            diagnostics: Vec::new(),
            last_span: None,
        }
    }

    pub fn parse_module(&mut self) -> Module {
        let start_span = self.peek_span();
        let mut module = Module {
            items: Vec::new(),
            span: start_span,
        };
        while !self.at_eof() {
            if self.eat_trivia() {
                continue;
            }
            match self.parse_item() {
                Some(item) => module.items.push(item),
                None => self.recover_to_item_boundary(),
            }
        }
        if let Some(last) = self.last_span {
            module.span = module.span.merge(last);
        }
        module
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn parse_item(&mut self) -> Option<Item> {
        match self.peek_kind() {
            TokenKind::Keyword(Keyword::Fn) => self.parse_function().map(Item::Function),
            TokenKind::Keyword(Keyword::Let) => self.parse_let_statement().map(Item::Let),
            _ => {
                let span = self.peek_span();
                self.error("expected `fn` or `let`", span);
                None
            }
        }
    }

    fn parse_function(&mut self) -> Option<Function> {
        let fn_token = self.bump();
        let (name, _name_span) = self.expect_identifier("function name")?;
        self.expect_lparen()?;
        let params = self.parse_parameters()?;
        self.expect_rparen()?;
        let body = self.parse_block("function body")?;
        let span = fn_token.span.merge(body.span);
        Some(Function {
            span,
            name,
            params,
            body,
        })
    }

    fn parse_parameters(&mut self) -> Option<Vec<Parameter>> {
        let mut params = Vec::new();
        if matches!(self.peek_kind(), TokenKind::RParen) {
            return Some(params);
        }
        loop {
            let (name, span) = self.expect_identifier("parameter")?;
            params.push(Parameter { span, name });
            if matches!(self.peek_kind(), TokenKind::Comma) {
                self.bump();
                if matches!(self.peek_kind(), TokenKind::RParen) {
                    break;
                }
                continue;
            }
            break;
        }
        Some(params)
    }

    fn parse_block(&mut self, context: &str) -> Option<Block> {
        match self.peek_kind() {
            TokenKind::LBrace => self.parse_braced_block(),
            TokenKind::Colon => self.parse_suite_block(context),
            _ => {
                let span = self.peek_span();
                self.error(format!("expected block for {}", context), span);
                None
            }
        }
    }

    fn parse_braced_block(&mut self) -> Option<Block> {
        let start = self.expect_lbrace()?.span;
        let mut statements = Vec::new();
        while !self.at_eof() && !matches!(self.peek_kind(), TokenKind::RBrace) {
            if self.eat_trivia_line() {
                continue;
            }
            if let Some(stmt) = self.parse_stmt() {
                statements.push(stmt);
            } else {
                self.recover_in_block();
            }
            if matches!(self.peek_kind(), TokenKind::RBrace) {
                break;
            }
            self.eat_newline();
        }
        let end = self.expect_rbrace()?.span;
        Some(self.finish_block(start, end, statements))
    }

    fn parse_suite_block(&mut self, context: &str) -> Option<Block> {
        let colon = self.expect_colon()?.span;
        if !self.eat_newline() {
            let span = self.peek_span();
            self.error(format!("expected newline after `:` for {}", context), span);
            return None;
        }
        self.expect_indent()?;
        self.collect_indented_block(colon)
    }

    fn collect_indented_block(&mut self, start: Span) -> Option<Block> {
        let mut statements = Vec::new();
        while !self.at_eof() && !matches!(self.peek_kind(), TokenKind::Dedent) {
            if self.eat_trivia_line() {
                continue;
            }
            if let Some(stmt) = self.parse_stmt() {
                statements.push(stmt);
            } else {
                self.recover_to_line_end();
            }
            if matches!(self.peek_kind(), TokenKind::Dedent) {
                break;
            }
            self.eat_newline();
        }
        let end = self.expect_dedent()?.span;
        Some(self.finish_block(start, end, statements))
    }

    fn parse_stmt(&mut self) -> Option<Stmt> {
        match self.peek_kind() {
            TokenKind::Keyword(Keyword::Let) => self.parse_let_statement().map(Stmt::Let),
            TokenKind::Keyword(Keyword::Return) => self.parse_return_stmt().map(Stmt::Return),
            TokenKind::Keyword(Keyword::While) => self.parse_while_stmt().map(Stmt::While),
            _ => self.parse_expr().map(Stmt::Expr),
        }
    }

    fn parse_let_statement(&mut self) -> Option<LetStatement> {
        let let_token = self.bump();
        let (name, _) = self.expect_identifier("binding name")?;
        if !matches!(self.peek_kind(), TokenKind::Equal) {
            let span = self.peek_span();
            self.error("expected `=` in let binding", span);
            return None;
        }
        self.bump();
        let value = self.parse_expr()?;
        let span = let_token.span.merge(value.span());
        Some(LetStatement { span, name, value })
    }

    fn parse_return_stmt(&mut self) -> Option<ReturnStatement> {
        let ret_token = self.bump();
        if matches!(
            self.peek_kind(),
            TokenKind::Newline | TokenKind::Dedent | TokenKind::RBrace | TokenKind::Eof
        ) {
            return Some(ReturnStatement {
                span: ret_token.span,
                value: None,
            });
        }
        let value = self.parse_expr()?;
        let span = ret_token.span.merge(value.span());
        Some(ReturnStatement {
            span,
            value: Some(value),
        })
    }

    fn parse_while_stmt(&mut self) -> Option<WhileStatement> {
        let while_token = self.bump();
        let condition = self.parse_expr()?;
        let body = self.parse_block("while body")?;
        let span = while_token.span.merge(body.span);
        Some(WhileStatement {
            span,
            condition,
            body: Box::new(body),
        })
    }

    fn parse_expr(&mut self) -> Option<Expr> {
        self.parse_if_expr()
    }

    fn parse_if_expr(&mut self) -> Option<Expr> {
        if !matches!(self.peek_kind(), TokenKind::Keyword(Keyword::If)) {
            return self.parse_binary_expr(0);
        }
        let if_token = self.bump();
        let condition = self.parse_binary_expr(0)?;
        let colon = self.expect_colon()?.span;
        let then_branch = if self.eat_newline() {
            self.expect_indent()?;
            self.collect_indented_block(colon)?
        } else {
            let expr = self.parse_expr()?;
            let span = colon.merge(expr.span());
            Block {
                span,
                statements: Vec::new(),
                tail: Some(Box::new(expr)),
            }
        };
        let else_branch = self.parse_else_branch()?;
        let span = if_token.span.merge(then_branch.span);
        Some(Expr::If(Box::new(IfExpr {
            span,
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch,
        })))
    }

    fn parse_binary_expr(&mut self, min_prec: u8) -> Option<Expr> {
        let mut left = self.parse_prefix_expr()?;
        loop {
            let (op, prec) = match self.peek_binary_op() {
                Some(info) => info,
                None => break,
            };
            if prec < min_prec {
                break;
            }
            self.bump();
            let rhs = self.parse_binary_expr(prec + 1)?;
            let span = left.span().merge(rhs.span());
            left = Expr::Binary(BinaryExpr {
                span,
                op,
                lhs: Box::new(left),
                rhs: Box::new(rhs),
            });
        }
        Some(left)
    }

    fn parse_prefix_expr(&mut self) -> Option<Expr> {
        match self.peek_kind() {
            TokenKind::Minus => {
                let op_token = self.bump();
                let operand = self.parse_prefix_expr()?;
                let span = op_token.span.merge(operand.span());
                Some(Expr::Unary(UnaryExpr {
                    span,
                    op: UnaryOp::Neg,
                    expr: Box::new(operand),
                }))
            }
            TokenKind::Bang => {
                let op_token = self.bump();
                let operand = self.parse_prefix_expr()?;
                let span = op_token.span.merge(operand.span());
                Some(Expr::Unary(UnaryExpr {
                    span,
                    op: UnaryOp::Not,
                    expr: Box::new(operand),
                }))
            }
            TokenKind::Keyword(Keyword::If) => self.parse_if_expr(),
            TokenKind::LBrace => self.parse_block_expr(),
            _ => self.parse_postfix_expr(),
        }
    }

    fn parse_block_expr(&mut self) -> Option<Expr> {
        let block = self.parse_braced_block()?;
        Some(Expr::Block(Box::new(block)))
    }

    fn parse_postfix_expr(&mut self) -> Option<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek_kind() {
                TokenKind::LParen => {
                    let start_span = expr.span();
                    self.bump();
                    let args = self.parse_argument_list()?;
                    let end = self.expect_rparen()?.span;
                    let span = start_span.merge(end);
                    expr = Expr::Call(CallExpr {
                        span,
                        callee: Box::new(expr),
                        args,
                    });
                }
                _ => break,
            }
        }
        Some(expr)
    }

    fn parse_argument_list(&mut self) -> Option<Vec<Expr>> {
        let mut args = Vec::new();
        if matches!(self.peek_kind(), TokenKind::RParen) {
            return Some(args);
        }
        loop {
            let expr = self.parse_expr()?;
            args.push(expr);
            if matches!(self.peek_kind(), TokenKind::Comma) {
                self.bump();
                if matches!(self.peek_kind(), TokenKind::RParen) {
                    break;
                }
                continue;
            }
            break;
        }
        Some(args)
    }

    fn parse_primary(&mut self) -> Option<Expr> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Identifier(name) => {
                self.bump();
                Some(Expr::Name(NameRef {
                    span: token.span,
                    name,
                }))
            }
            TokenKind::Int(value) => {
                self.bump();
                Some(Expr::Literal(Literal::Int(IntLiteral {
                    span: token.span,
                    value,
                })))
            }
            TokenKind::String(value) => {
                self.bump();
                Some(Expr::Literal(Literal::String(StringLiteral {
                    span: token.span,
                    value,
                })))
            }
            TokenKind::Keyword(Keyword::True) => {
                self.bump();
                Some(Expr::Literal(Literal::Bool(BoolLiteral {
                    span: token.span,
                    value: true,
                })))
            }
            TokenKind::Keyword(Keyword::False) => {
                self.bump();
                Some(Expr::Literal(Literal::Bool(BoolLiteral {
                    span: token.span,
                    value: false,
                })))
            }
            TokenKind::LParen => {
                let start = self.bump().span;
                if matches!(self.peek_kind(), TokenKind::RParen) {
                    let end = self.expect_rparen()?.span;
                    let span = start.merge(end);
                    return Some(Expr::Literal(Literal::Unit(UnitLiteral { span })));
                }
                let expr = self.parse_expr()?;
                self.expect_rparen()?;
                Some(Expr::Paren(Box::new(expr)))
            }
            TokenKind::LBrace => self.parse_block_expr(),
            _ => {
                self.error("expected expression", token.span);
                None
            }
        }
    }

    fn peek_binary_op(&self) -> Option<(BinaryOp, u8)> {
        match self.peek_kind() {
            TokenKind::Plus => Some((BinaryOp::Add, 10)),
            TokenKind::Minus => Some((BinaryOp::Sub, 10)),
            TokenKind::Star => Some((BinaryOp::Mul, 20)),
            TokenKind::Slash => Some((BinaryOp::Div, 20)),
            TokenKind::EqEq => Some((BinaryOp::Eq, 5)),
            TokenKind::BangEq => Some((BinaryOp::Ne, 5)),
            TokenKind::Lt => Some((BinaryOp::Lt, 6)),
            TokenKind::Gt => Some((BinaryOp::Gt, 6)),
            TokenKind::Le => Some((BinaryOp::Le, 6)),
            TokenKind::Ge => Some((BinaryOp::Ge, 6)),
            _ => None,
        }
    }

    fn parse_else_branch(&mut self) -> Option<Option<Box<Block>>> {
        if self.consume_keyword(Keyword::Elif) {
            let expr = self.parse_if_expr()?;
            let span = expr.span();
            return Some(Some(Box::new(Block {
                span,
                statements: Vec::new(),
                tail: Some(Box::new(expr)),
            })));
        }
        if self.consume_keyword(Keyword::Else) {
            let colon = self.expect_colon()?.span;
            if self.eat_newline() {
                self.expect_indent()?;
                return self
                    .collect_indented_block(colon)
                    .map(|block| Some(Box::new(block)));
            }
            let expr = self.parse_expr()?;
            let span = colon.merge(expr.span());
            return Some(Some(Box::new(Block {
                span,
                statements: Vec::new(),
                tail: Some(Box::new(expr)),
            })));
        }
        Some(None)
    }

    fn consume_keyword(&mut self, keyword: Keyword) -> bool {
        if matches!(self.peek_kind(), TokenKind::Keyword(k) if *k == keyword) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_identifier(&mut self, context: &str) -> Option<(SmolStr, Span)> {
        let token = self.peek().clone();
        if let TokenKind::Identifier(name) = token.kind {
            self.bump();
            Some((name, token.span))
        } else {
            let span = token.span;
            self.error(format!("expected identifier for {}", context), span);
            None
        }
    }

    fn expect_lparen(&mut self) -> Option<Token> {
        if matches!(self.peek_kind(), TokenKind::LParen) {
            Some(self.bump())
        } else {
            let span = self.peek_span();
            self.error("expected `(`", span);
            None
        }
    }

    fn expect_rparen(&mut self) -> Option<Token> {
        if matches!(self.peek_kind(), TokenKind::RParen) {
            Some(self.bump())
        } else {
            let span = self.peek_span();
            self.error("expected `)`", span);
            None
        }
    }

    fn expect_lbrace(&mut self) -> Option<Token> {
        if matches!(self.peek_kind(), TokenKind::LBrace) {
            Some(self.bump())
        } else {
            let span = self.peek_span();
            self.error("expected `{`", span);
            None
        }
    }

    fn expect_rbrace(&mut self) -> Option<Token> {
        if matches!(self.peek_kind(), TokenKind::RBrace) {
            Some(self.bump())
        } else {
            let span = self.peek_span();
            self.error("expected `}`", span);
            None
        }
    }

    fn expect_colon(&mut self) -> Option<Token> {
        if matches!(self.peek_kind(), TokenKind::Colon) {
            Some(self.bump())
        } else {
            let span = self.peek_span();
            self.error("expected `:`", span);
            None
        }
    }

    fn expect_indent(&mut self) -> Option<Token> {
        if matches!(self.peek_kind(), TokenKind::Indent) {
            Some(self.bump())
        } else {
            let span = self.peek_span();
            self.error("expected indentation", span);
            None
        }
    }

    fn expect_dedent(&mut self) -> Option<Token> {
        if matches!(self.peek_kind(), TokenKind::Dedent) {
            Some(self.bump())
        } else {
            let span = self.peek_span();
            self.error("expected dedent", span);
            None
        }
    }

    fn finish_block(&self, start: Span, end: Span, mut statements: Vec<Stmt>) -> Block {
        let span = start.merge(end);
        let tail = if matches!(statements.last(), Some(Stmt::Expr(_))) {
            if let Some(Stmt::Expr(expr)) = statements.pop() {
                Some(Box::new(expr))
            } else {
                None
            }
        } else {
            None
        };
        Block {
            span,
            statements,
            tail,
        }
    }

    fn eat_trivia(&mut self) -> bool {
        if matches!(self.peek_kind(), TokenKind::Newline | TokenKind::Dedent) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn eat_trivia_line(&mut self) -> bool {
        if matches!(self.peek_kind(), TokenKind::Newline) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn eat_newline(&mut self) -> bool {
        if matches!(self.peek_kind(), TokenKind::Newline) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn recover_to_item_boundary(&mut self) {
        while !self.at_eof() {
            if matches!(self.peek_kind(), TokenKind::Newline | TokenKind::Dedent) {
                break;
            }
            self.bump();
        }
    }

    fn recover_in_block(&mut self) {
        while !self.at_eof() {
            if matches!(
                self.peek_kind(),
                TokenKind::Newline | TokenKind::Dedent | TokenKind::RBrace
            ) {
                break;
            }
            self.bump();
        }
    }

    fn recover_to_line_end(&mut self) {
        while !self.at_eof() {
            if matches!(self.peek_kind(), TokenKind::Newline | TokenKind::Dedent) {
                break;
            }
            self.bump();
        }
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    fn peek_span(&self) -> Span {
        self.peek().span
    }

    fn bump(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();
        self.pos += 1;
        self.last_span = Some(token.span);
        token
    }

    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.diagnostics
            .push(Diagnostic::error(message.into(), span));
    }
}
