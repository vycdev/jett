use jett_common::Span;
use jett_diagnostics::Diagnostic;
use jett_lexer::{Token, TokenKind};

use crate::ast::*;

// ---------------------------------------------------------------------------
// Parse result
// ---------------------------------------------------------------------------

/// Result of parsing a source file.
pub struct ParseResult {
    pub module: Module,
    pub errors: Vec<Diagnostic>,
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

pub struct Parser<'src> {
    source: &'src str,
    tokens: Vec<Token>,
    pos: usize,
    errors: Vec<Diagnostic>,
}

impl<'src> Parser<'src> {
    pub fn new(source: &'src str, tokens: Vec<Token>) -> Self {
        Self {
            source,
            tokens,
            pos: 0,
            errors: Vec::new(),
        }
    }

    // =======================================================================
    // Token helpers
    // =======================================================================

    fn peek(&self) -> TokenKind {
        self.tokens.get(self.pos).map_or(TokenKind::Eof, |t| t.kind)
    }

    fn peek_token(&self) -> &Token {
        static EOF_TOKEN: std::sync::LazyLock<Token> = std::sync::LazyLock::new(|| Token {
            kind: TokenKind::Eof,
            span: Span::new(jett_common::FileId::new(0), 0, 0),
        });
        self.tokens.get(self.pos).unwrap_or(&EOF_TOKEN)
    }

    fn peek_nth(&self, n: usize) -> TokenKind {
        self.tokens
            .get(self.pos + n)
            .map_or(TokenKind::Eof, |t| t.kind)
    }

    fn advance(&mut self) -> Token {
        let tok = self.peek_token().clone();
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn eat(&mut self, kind: TokenKind) -> Option<Token> {
        if self.peek() == kind {
            Some(self.advance())
        } else {
            None
        }
    }

    fn expect(&mut self, kind: TokenKind) -> Token {
        if self.peek() == kind {
            self.advance()
        } else {
            let tok = self.peek_token().clone();
            self.error(
                format!("expected {:?}, found {:?}", kind, tok.kind),
                tok.span,
            );
            tok
        }
    }

    fn token_text(&self, token: &Token) -> &'src str {
        let start = token.span.start as usize;
        let end = token.span.end as usize;
        if end <= self.source.len() {
            &self.source[start..end]
        } else {
            ""
        }
    }

    fn skip_newlines(&mut self) {
        while self.peek() == TokenKind::Newline {
            self.advance();
        }
    }

    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.errors
            .push(Diagnostic::error(1000, message, span));
    }

    /// Skip tokens until we reach one of the recovery points (Newline, Dedent, Eof).
    fn synchronize(&mut self) {
        loop {
            match self.peek() {
                TokenKind::Newline => {
                    self.advance();
                    return;
                }
                TokenKind::Dedent | TokenKind::Eof => return,
                _ => {
                    self.advance();
                }
            }
        }
    }

    // =======================================================================
    // Entry point
    // =======================================================================

    pub fn parse(mut self) -> ParseResult {
        let start_span = self.peek_token().span;
        self.skip_newlines();
        let mut items = Vec::new();

        while self.peek() != TokenKind::Eof {
            match self.parse_item() {
                Some(item) => items.push(item),
                None => {
                    // Error recovery: skip to next line
                    self.synchronize();
                }
            }
            self.skip_newlines();
        }

        let end_span = if let Some(last) = self.tokens.last() {
            last.span
        } else {
            start_span
        };

        ParseResult {
            module: Module {
                span: start_span.merge(end_span),
                items,
            },
            errors: self.errors,
        }
    }

    // =======================================================================
    // Items
    // =======================================================================

    fn parse_item(&mut self) -> Option<Item> {
        self.skip_newlines();
        match self.peek() {
            TokenKind::Namespace => Some(Item::Namespace(self.parse_namespace())),
            TokenKind::Function => Some(Item::Function(self.parse_function())),
            TokenKind::Struct => Some(Item::Struct(self.parse_struct())),
            TokenKind::Enum => Some(Item::Enum(self.parse_enum())),
            TokenKind::Verify => Some(Item::Verify(self.parse_verify_block())),
            TokenKind::Mutable => Some(Item::VarDecl(self.parse_var_decl())),
            // Could be a variable declaration: `Type name = expr`
            _ if self.looks_like_var_decl() => Some(Item::VarDecl(self.parse_var_decl())),
            _ => {
                let tok = self.peek_token().clone();
                self.error(
                    format!("expected item (namespace, function, struct, enum, or variable), found {:?}", tok.kind),
                    tok.span,
                );
                None
            }
        }
    }

    fn parse_namespace(&mut self) -> NamespaceDecl {
        let kw = self.expect(TokenKind::Namespace);
        let name = self.parse_ident();
        NamespaceDecl {
            span: kw.span.merge(name.span),
            name,
        }
    }

    fn parse_verify_block(&mut self) -> VerifyBlock {
        let kw = self.expect(TokenKind::Verify);
        let name = self.parse_ident();
        self.expect(TokenKind::Colon);
        let body = self.parse_block();
        let end_span = body.span;
        VerifyBlock {
            span: kw.span.merge(end_span),
            name,
            body,
        }
    }

    fn parse_function(&mut self) -> FunctionDef {
        let kw = self.expect(TokenKind::Function);
        let name = self.parse_ident();
        self.expect(TokenKind::LParen);
        let params = self.parse_params();
        self.expect(TokenKind::RParen);

        let return_type = if self.eat(TokenKind::Returns).is_some() {
            Some(self.parse_type())
        } else {
            None
        };

        self.expect(TokenKind::Colon);
        let body = self.parse_block();
        let end_span = body.span;

        FunctionDef {
            span: kw.span.merge(end_span),
            name,
            params,
            return_type,
            body,
        }
    }

    fn parse_params(&mut self) -> Vec<Param> {
        let mut params = Vec::new();
        if self.peek() == TokenKind::RParen {
            return params;
        }
        params.push(self.parse_param());
        while self.eat(TokenKind::Comma).is_some() {
            if self.peek() == TokenKind::RParen {
                break;
            }
            params.push(self.parse_param());
        }
        params
    }

    fn parse_param(&mut self) -> Param {
        let start = self.peek_token().span;
        let view = self.eat(TokenKind::View).is_some();
        let mutable = self.eat(TokenKind::Mutable).is_some();
        let name = self.parse_ident();
        self.expect(TokenKind::Colon);
        let ty = self.parse_type();
        let end = ty.span();
        Param {
            view,
            mutable,
            name,
            ty,
            span: start.merge(end),
        }
    }

    fn parse_struct(&mut self) -> StructDef {
        let kw = self.expect(TokenKind::Struct);
        let name = self.parse_ident();
        self.expect(TokenKind::Colon);

        // Expect an indented block of fields and methods
        self.skip_newlines();
        let indent_tok = self.expect(TokenKind::Indent);
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut last_span = indent_tok.span;

        self.skip_newlines();
        while self.peek() != TokenKind::Dedent && self.peek() != TokenKind::Eof {
            if self.peek() == TokenKind::Function {
                let func = self.parse_function();
                last_span = func.span;
                methods.push(func);
            } else {
                // Field: `name: Type`
                let field = self.parse_field_def();
                last_span = field.span;
                fields.push(field);
            }
            self.skip_newlines();
        }
        if self.peek() == TokenKind::Dedent {
            last_span = self.advance().span;
        }

        StructDef {
            span: kw.span.merge(last_span),
            name,
            fields,
            methods,
        }
    }

    fn parse_field_def(&mut self) -> FieldDef {
        let name = self.parse_ident();
        self.expect(TokenKind::Colon);
        let ty = self.parse_type();
        FieldDef {
            span: name.span.merge(ty.span()),
            name,
            ty,
        }
    }

    fn parse_enum(&mut self) -> EnumDef {
        let kw = self.expect(TokenKind::Enum);
        let name = self.parse_ident();
        self.expect(TokenKind::Colon);

        self.skip_newlines();
        let indent_tok = self.expect(TokenKind::Indent);
        let mut variants = Vec::new();
        let mut last_span = indent_tok.span;

        self.skip_newlines();
        while self.peek() != TokenKind::Dedent && self.peek() != TokenKind::Eof {
            let variant = self.parse_variant();
            last_span = variant.span;
            variants.push(variant);
            self.skip_newlines();
        }
        if self.peek() == TokenKind::Dedent {
            last_span = self.advance().span;
        }

        EnumDef {
            span: kw.span.merge(last_span),
            name,
            variants,
        }
    }

    fn parse_variant(&mut self) -> Variant {
        let name = self.parse_ident();
        let mut fields = Vec::new();
        let mut end_span = name.span;

        if self.eat(TokenKind::LParen).is_some() {
            if self.peek() != TokenKind::RParen {
                fields.push(self.parse_field_def());
                while self.eat(TokenKind::Comma).is_some() {
                    if self.peek() == TokenKind::RParen {
                        break;
                    }
                    fields.push(self.parse_field_def());
                }
            }
            end_span = self.expect(TokenKind::RParen).span;
        }

        Variant {
            span: name.span.merge(end_span),
            name,
            fields,
        }
    }

    // =======================================================================
    // Types
    // =======================================================================

    fn parse_type(&mut self) -> TypeExpr {
        if self.eat(TokenKind::View).is_some() {
            let start = self.tokens[self.pos - 1].span;
            let inner = self.parse_type();
            let span = start.merge(inner.span());
            return TypeExpr::View(Box::new(inner), span);
        }

        let ident = self.parse_type_ident();

        // Check for generic parameters: `list[string]`, `map[string, int64]`
        if self.peek() == TokenKind::LBracket {
            let start = ident.span;
            self.advance(); // consume `[`
            let mut args = Vec::new();
            if self.peek() != TokenKind::RBracket {
                args.push(self.parse_type());
                while self.eat(TokenKind::Comma).is_some() {
                    args.push(self.parse_type());
                }
            }
            let end = self.expect(TokenKind::RBracket).span;
            TypeExpr::Generic(ident, args, start.merge(end))
        } else {
            TypeExpr::Named(ident)
        }
    }

    /// Parse a type name — could be a keyword type (`int64`, `string`, etc.) or an identifier.
    fn parse_type_ident(&mut self) -> Ident {
        let tok = self.peek_token().clone();
        match tok.kind {
            // Built-in type keywords
            TokenKind::Int8
            | TokenKind::Int16
            | TokenKind::Int32
            | TokenKind::Int64
            | TokenKind::Uint8
            | TokenKind::Uint16
            | TokenKind::Uint32
            | TokenKind::Uint64
            | TokenKind::Float32
            | TokenKind::Float64
            | TokenKind::String_
            | TokenKind::Bool_
            | TokenKind::Bytes_
            | TokenKind::List_
            | TokenKind::Map_
            | TokenKind::Set_
            | TokenKind::Nothing
            | TokenKind::Result
            | TokenKind::Optional => {
                self.advance();
                Ident {
                    name: self.token_text(&tok).to_string(),
                    span: tok.span,
                }
            }
            TokenKind::Ident => self.parse_ident(),
            _ => {
                self.error(format!("expected type, found {:?}", tok.kind), tok.span);
                self.advance();
                Ident {
                    name: "<error>".to_string(),
                    span: tok.span,
                }
            }
        }
    }

    // =======================================================================
    // Blocks and Statements
    // =======================================================================

    fn parse_block(&mut self) -> Block {
        self.skip_newlines();
        let indent_tok = self.expect(TokenKind::Indent);
        let start = indent_tok.span;
        let mut stmts = Vec::new();
        let mut last_span = start;

        self.skip_newlines();
        while self.peek() != TokenKind::Dedent && self.peek() != TokenKind::Eof {
            match self.parse_stmt() {
                Some(stmt) => {
                    last_span = stmt_span(&stmt);
                    stmts.push(stmt);
                }
                None => {
                    self.synchronize();
                }
            }
            self.skip_newlines();
        }
        if self.peek() == TokenKind::Dedent {
            last_span = self.advance().span;
        }

        Block {
            stmts,
            span: start.merge(last_span),
        }
    }

    fn parse_stmt(&mut self) -> Option<Stmt> {
        self.skip_newlines();
        match self.peek() {
            TokenKind::Return => Some(self.parse_return_stmt()),
            TokenKind::If => Some(self.parse_if_stmt()),
            TokenKind::For => Some(self.parse_for_stmt()),
            TokenKind::While => Some(self.parse_while_stmt()),
            TokenKind::Match => Some(self.parse_match_stmt()),
            TokenKind::Use => Some(self.parse_use_stmt()),
            TokenKind::Assert => Some(self.parse_assert_stmt()),
            TokenKind::Break => {
                let tok = self.advance();
                Some(Stmt::Break(tok.span))
            }
            TokenKind::Continue => {
                let tok = self.advance();
                Some(Stmt::Continue(tok.span))
            }
            TokenKind::Mutable => Some(Stmt::VarDecl(self.parse_var_decl())),
            _ if self.looks_like_var_decl() => Some(Stmt::VarDecl(self.parse_var_decl())),
            TokenKind::Eof => None,
            TokenKind::Dedent => None,
            _ => {
                // Try to parse as expression statement (could also be an assignment).
                let expr = self.parse_expr();
                if self.eat(TokenKind::Eq).is_some() {
                    // Assignment: `target = value`
                    let value = self.parse_expr();
                    let span = expr.span().merge(value.span());
                    Some(Stmt::Assign(AssignStmt {
                        target: expr,
                        value,
                        span,
                    }))
                } else {
                    // Check for handle block attached to expression
                    let expr = self.maybe_parse_handle(expr);
                    let span = expr.span();
                    Some(Stmt::Expr(ExprStmt { expr, span }))
                }
            }
        }
    }

    fn parse_return_stmt(&mut self) -> Stmt {
        let kw = self.expect(TokenKind::Return);
        // Check if there's an expression on the same line
        let value = if self.peek() == TokenKind::Newline
            || self.peek() == TokenKind::Dedent
            || self.peek() == TokenKind::Eof
        {
            None
        } else {
            Some(self.parse_expr())
        };
        let end = value.as_ref().map_or(kw.span, |e| e.span());
        Stmt::Return(ReturnStmt {
            value,
            span: kw.span.merge(end),
        })
    }

    fn parse_if_stmt(&mut self) -> Stmt {
        let kw = self.expect(TokenKind::If);
        let condition = self.parse_expr();
        self.expect(TokenKind::Colon);
        let then_block = self.parse_block();

        let mut else_ifs = Vec::new();
        let mut else_block = None;
        let mut end_span = then_block.span;

        // Parse `else if` and `else` chains
        self.skip_newlines();
        while self.peek() == TokenKind::Else {
            self.advance(); // consume `else`
            if self.eat(TokenKind::If).is_some() {
                let cond = self.parse_expr();
                self.expect(TokenKind::Colon);
                let block = self.parse_block();
                end_span = block.span;
                else_ifs.push((cond, block));
                self.skip_newlines();
            } else {
                self.expect(TokenKind::Colon);
                let block = self.parse_block();
                end_span = block.span;
                else_block = Some(block);
                break;
            }
        }

        Stmt::If(IfStmt {
            condition,
            then_block,
            else_ifs,
            else_block,
            span: kw.span.merge(end_span),
        })
    }

    fn parse_for_stmt(&mut self) -> Stmt {
        let kw = self.expect(TokenKind::For);
        let variable = self.parse_ident();
        self.expect(TokenKind::In);

        // Check for `view` keyword before iterable
        let view = self.eat(TokenKind::View).is_some();
        let iterable = self.parse_expr();
        self.expect(TokenKind::Colon);
        let body = self.parse_block();

        Stmt::For(ForStmt {
            span: kw.span.merge(body.span),
            variable,
            view,
            iterable,
            body,
        })
    }

    fn parse_while_stmt(&mut self) -> Stmt {
        let kw = self.expect(TokenKind::While);
        let condition = self.parse_expr();
        self.expect(TokenKind::Colon);
        let body = self.parse_block();

        Stmt::While(WhileStmt {
            condition,
            body: body.clone(),
            span: kw.span.merge(body.span),
        })
    }

    fn parse_match_stmt(&mut self) -> Stmt {
        let kw = self.expect(TokenKind::Match);
        let expr = self.parse_expr();
        self.expect(TokenKind::Colon);

        self.skip_newlines();
        let indent_tok = self.expect(TokenKind::Indent);
        let mut arms = Vec::new();
        let mut last_span = indent_tok.span;

        self.skip_newlines();
        while self.peek() != TokenKind::Dedent && self.peek() != TokenKind::Eof {
            let arm = self.parse_match_arm();
            last_span = arm.span;
            arms.push(arm);
            self.skip_newlines();
        }
        if self.peek() == TokenKind::Dedent {
            last_span = self.advance().span;
        }

        Stmt::Match(MatchStmt {
            expr,
            arms,
            span: kw.span.merge(last_span),
        })
    }

    fn parse_match_arm(&mut self) -> MatchArm {
        let start = self.peek_token().span;

        let pattern = if self.peek() == TokenKind::Other {
            let tok = self.advance();
            // Check if this is `other` as catch-all (followed by `:`) or
            // `other` as an identifier pattern
            if self.peek() == TokenKind::Colon {
                Pattern::Other(tok.span)
            } else if self.peek() == TokenKind::LParen {
                // other(bindings) — destructuring with name "other"
                let name = Ident {
                    name: self.token_text(&tok).to_string(),
                    span: tok.span,
                };
                self.advance(); // consume `(`
                let mut bindings = Vec::new();
                if self.peek() != TokenKind::RParen {
                    bindings.push(self.parse_ident());
                    while self.eat(TokenKind::Comma).is_some() {
                        if self.peek() == TokenKind::RParen {
                            break;
                        }
                        bindings.push(self.parse_ident());
                    }
                }
                self.expect(TokenKind::RParen);
                Pattern::Variant(name, bindings)
            } else {
                Pattern::Other(tok.span)
            }
        } else {
            let name = self.parse_ident();
            if self.peek() == TokenKind::LParen {
                // Destructuring: `variant(a, b)`
                self.advance(); // consume `(`
                let mut bindings = Vec::new();
                if self.peek() != TokenKind::RParen {
                    bindings.push(self.parse_ident());
                    while self.eat(TokenKind::Comma).is_some() {
                        if self.peek() == TokenKind::RParen {
                            break;
                        }
                        bindings.push(self.parse_ident());
                    }
                }
                self.expect(TokenKind::RParen);
                Pattern::Variant(name, bindings)
            } else {
                Pattern::Ident(name)
            }
        };

        self.expect(TokenKind::Colon);
        let body = self.parse_block();

        MatchArm {
            span: start.merge(body.span),
            pattern,
            body,
        }
    }

    fn parse_use_stmt(&mut self) -> Stmt {
        let kw = self.expect(TokenKind::Use);
        let path = self.parse_ident();
        let alias = if self.eat(TokenKind::As).is_some() {
            Some(self.parse_ident())
        } else {
            None
        };
        let end = alias.as_ref().map_or(path.span, |a| a.span);
        Stmt::Use(UseDecl {
            path,
            alias,
            span: kw.span.merge(end),
        })
    }

    fn parse_assert_stmt(&mut self) -> Stmt {
        let kw = self.expect(TokenKind::Assert);
        let condition = self.parse_expr();
        // Optional message (string literal)
        let message = if self.peek() == TokenKind::StringLiteral
            || self.peek() == TokenKind::StringStart
        {
            Some(self.parse_expr())
        } else {
            None
        };
        let end = message
            .as_ref()
            .map_or(condition.span(), |m| m.span());
        Stmt::Assert(AssertStmt {
            condition,
            message,
            span: kw.span.merge(end),
        })
    }

    fn parse_var_decl(&mut self) -> VarDecl {
        let start = self.peek_token().span;
        let mutable = self.eat(TokenKind::Mutable).is_some();
        let ty = self.parse_type();
        let name = self.parse_ident();
        self.expect(TokenKind::Eq);
        let value = self.parse_expr();
        // Check for handle block after the expression
        let value = self.maybe_parse_handle(value);
        let end = value.span();
        VarDecl {
            mutable,
            ty,
            name,
            value,
            span: start.merge(end),
        }
    }

    /// Try to determine if the current position looks like a variable declaration.
    /// A var decl starts with a type followed by an identifier followed by `=`.
    /// We look ahead to check this pattern.
    fn looks_like_var_decl(&self) -> bool {
        // Must start with something that could be a type
        if !self.is_type_start(self.peek()) {
            return false;
        }

        // Walk forward to find the pattern: Type [GenericArgs] Ident =
        let mut lookahead = 0;

        // Skip the type name
        let first = self.peek_nth(lookahead);
        if !self.is_type_start(first) {
            return false;
        }
        lookahead += 1;

        // Handle dotted paths for types: e.g. module.Type — not needed for MVP
        // but handle generic args: Type[...]
        if self.peek_nth(lookahead) == TokenKind::LBracket {
            lookahead += 1;
            let mut depth = 1;
            while depth > 0 && lookahead < 20 {
                match self.peek_nth(lookahead) {
                    TokenKind::LBracket => depth += 1,
                    TokenKind::RBracket => depth -= 1,
                    TokenKind::Eof => return false,
                    _ => {}
                }
                lookahead += 1;
            }
        }

        // Now we should see an identifier (or contextual keyword) followed by `=`
        let name_kind = self.peek_nth(lookahead);
        if (name_kind == TokenKind::Ident || self.is_contextual_ident(name_kind))
            && self.peek_nth(lookahead + 1) == TokenKind::Eq
        {
            return true;
        }

        false
    }

    fn is_type_start(&self, kind: TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::Int8
                | TokenKind::Int16
                | TokenKind::Int32
                | TokenKind::Int64
                | TokenKind::Uint8
                | TokenKind::Uint16
                | TokenKind::Uint32
                | TokenKind::Uint64
                | TokenKind::Float32
                | TokenKind::Float64
                | TokenKind::String_
                | TokenKind::Bool_
                | TokenKind::Bytes_
                | TokenKind::List_
                | TokenKind::Map_
                | TokenKind::Set_
                | TokenKind::Nothing
                | TokenKind::Result
                | TokenKind::Optional
                | TokenKind::Ident
        )
    }

    // =======================================================================
    // Handle blocks
    // =======================================================================

    fn maybe_parse_handle(&mut self, expr: Expr) -> Expr {
        if self.peek() != TokenKind::Handle {
            return expr;
        }
        self.advance(); // consume `handle`

        // `handle error:` or `handle:`
        let error_name = if self.peek() == TokenKind::Error {
            let tok = self.advance();
            Some(Ident {
                name: self.token_text(&tok).to_string(),
                span: tok.span,
            })
        } else {
            None
        };

        self.expect(TokenKind::Colon);
        let block = self.parse_block();
        let span = expr.span().merge(block.span);
        Expr::Handle(Box::new(expr), error_name, block, span)
    }

    // =======================================================================
    // Expressions — Pratt parser
    // =======================================================================

    fn parse_expr(&mut self) -> Expr {
        self.parse_expr_bp(0)
    }

    /// Pratt parser: parse expression with minimum binding power.
    fn parse_expr_bp(&mut self, min_bp: u8) -> Expr {
        let mut lhs = self.parse_prefix();

        loop {
            // Check for postfix/infix operators
            let kind = self.peek();

            // Postfix: field access `.`, function call `(`
            match kind {
                TokenKind::Dot => {
                    let (l_bp, _) = (14, 15); // high precedence
                    if l_bp < min_bp {
                        break;
                    }
                    self.advance(); // consume `.`
                    let field = self.parse_ident();
                    let span = lhs.span().merge(field.span);
                    lhs = Expr::FieldAccess(Box::new(lhs), field, span);
                    // Check for method call: `expr.method(args)`
                    if self.peek() == TokenKind::LParen {
                        lhs = self.parse_call_args(lhs);
                    }
                    continue;
                }
                TokenKind::LParen => {
                    let (l_bp, _) = (14, 15);
                    if l_bp < min_bp {
                        break;
                    }
                    lhs = self.parse_call_args(lhs);
                    continue;
                }
                TokenKind::LBracket => {
                    // Generic call: `name[Type](args)`
                    // But only if followed by something that looks like a type
                    // and eventually `](` — this is tricky. For now, handle the
                    // common case.
                    let (l_bp, _) = (14, 15);
                    if l_bp < min_bp {
                        break;
                    }
                    if self.looks_like_generic_args() {
                        lhs = self.parse_generic_call(lhs);
                        continue;
                    }
                    break;
                }
                _ => {}
            }

            // Handle block as postfix
            if kind == TokenKind::Handle {
                let (l_bp, _) = (1, 2);
                if l_bp < min_bp {
                    break;
                }
                lhs = self.maybe_parse_handle(lhs);
                continue;
            }

            // Infix binary operators
            if let Some((l_bp, r_bp)) = infix_binding_power(kind) {
                if l_bp < min_bp {
                    break;
                }
                let op_tok = self.advance();
                let op = token_to_binop(op_tok.kind);
                let rhs = self.parse_expr_bp(r_bp);
                let span = lhs.span().merge(rhs.span());
                lhs = Expr::Binary(Box::new(lhs), op, Box::new(rhs), span);
                continue;
            }

            break;
        }

        lhs
    }

    fn parse_prefix(&mut self) -> Expr {
        let tok = self.peek_token().clone();
        match tok.kind {
            TokenKind::IntLiteral => {
                self.advance();
                let text = self.token_text(&tok);
                let value = text.parse::<i64>().unwrap_or(0);
                Expr::IntLiteral(value, tok.span)
            }
            TokenKind::FloatLiteral => {
                self.advance();
                let text = self.token_text(&tok);
                let value = text.parse::<f64>().unwrap_or(0.0);
                Expr::FloatLiteral(value, tok.span)
            }
            TokenKind::StringLiteral => {
                self.advance();
                let text = self.token_text(&tok);
                // Strip the surrounding quotes
                let inner = if text.len() >= 2 {
                    text[1..text.len() - 1].to_string()
                } else {
                    text.to_string()
                };
                Expr::StringLiteral(inner, tok.span)
            }
            TokenKind::StringStart => {
                // String interpolation: StringStart, tokens..., StringMid/StringEnd
                // For simplicity, represent as a single string literal with placeholders
                self.advance();
                let text = self.token_text(&tok);
                let start_span = tok.span;
                let mut full = text.to_string();
                // Consume tokens until StringEnd
                loop {
                    match self.peek() {
                        TokenKind::StringEnd => {
                            let end_tok = self.advance();
                            full.push_str(self.token_text(&end_tok));
                            let span = start_span.merge(end_tok.span);
                            return Expr::StringLiteral(full, span);
                        }
                        TokenKind::StringMid => {
                            let mid_tok = self.advance();
                            full.push_str(self.token_text(&mid_tok));
                        }
                        TokenKind::Eof => {
                            return Expr::StringLiteral(full, start_span);
                        }
                        _ => {
                            // Interpolated expression token — skip for now
                            let inner_tok = self.advance();
                            full.push_str(self.token_text(&inner_tok));
                        }
                    }
                }
            }
            TokenKind::True => {
                self.advance();
                Expr::BoolLiteral(true, tok.span)
            }
            TokenKind::False => {
                self.advance();
                Expr::BoolLiteral(false, tok.span)
            }
            TokenKind::Nothing => {
                self.advance();
                Expr::Nothing(tok.span)
            }
            TokenKind::None => {
                self.advance();
                Expr::None(tok.span)
            }
            TokenKind::Not | TokenKind::Bang => {
                self.advance();
                let operand = self.parse_expr_bp(13); // unary has high precedence
                let span = tok.span.merge(operand.span());
                Expr::Unary(UnaryOp::Not, Box::new(operand), span)
            }
            TokenKind::Minus => {
                self.advance();
                let operand = self.parse_expr_bp(13);
                let span = tok.span.merge(operand.span());
                Expr::Unary(UnaryOp::Neg, Box::new(operand), span)
            }
            TokenKind::LParen => {
                self.advance();
                let inner = self.parse_expr();
                let close = self.expect(TokenKind::RParen);
                Expr::Paren(Box::new(inner), tok.span.merge(close.span))
            }
            TokenKind::View => {
                self.advance();
                let inner = self.parse_expr_bp(13);
                let span = tok.span.merge(inner.span());
                Expr::View(Box::new(inner), span)
            }
            TokenKind::Ok => {
                self.advance();
                self.expect(TokenKind::LParen);
                let inner = self.parse_expr();
                let close = self.expect(TokenKind::RParen);
                Expr::Ok(Box::new(inner), tok.span.merge(close.span))
            }
            TokenKind::Fail => {
                self.advance();
                self.expect(TokenKind::LParen);
                let inner = self.parse_expr();
                let close = self.expect(TokenKind::RParen);
                Expr::Fail(Box::new(inner), tok.span.merge(close.span))
            }
            TokenKind::Some => {
                self.advance();
                self.expect(TokenKind::LParen);
                let inner = self.parse_expr();
                let close = self.expect(TokenKind::RParen);
                Expr::Some(Box::new(inner), tok.span.merge(close.span))
            }
            TokenKind::Default => {
                self.advance();
                let inner = self.parse_expr();
                let span = tok.span.merge(inner.span());
                Expr::Default(Box::new(inner), span)
            }
            TokenKind::List_ => {
                self.advance();
                self.expect(TokenKind::LParen);
                let mut items = Vec::new();
                if self.peek() != TokenKind::RParen {
                    items.push(self.parse_expr());
                    while self.eat(TokenKind::Comma).is_some() {
                        if self.peek() == TokenKind::RParen {
                            break;
                        }
                        items.push(self.parse_expr());
                    }
                }
                let close = self.expect(TokenKind::RParen);
                Expr::ListConstruct(items, tok.span.merge(close.span))
            }
            TokenKind::Map_ => {
                self.advance();
                self.expect(TokenKind::LParen);
                let mut pairs = Vec::new();
                if self.peek() != TokenKind::RParen {
                    let key = self.parse_expr();
                    self.expect(TokenKind::Colon);
                    let val = self.parse_expr();
                    pairs.push((key, val));
                    while self.eat(TokenKind::Comma).is_some() {
                        if self.peek() == TokenKind::RParen {
                            break;
                        }
                        let key = self.parse_expr();
                        self.expect(TokenKind::Colon);
                        let val = self.parse_expr();
                        pairs.push((key, val));
                    }
                }
                let close = self.expect(TokenKind::RParen);
                Expr::MapConstruct(pairs, tok.span.merge(close.span))
            }
            TokenKind::Self_ => {
                self.advance();
                Expr::Ident(Ident {
                    name: "self".to_string(),
                    span: tok.span,
                })
            }
            TokenKind::Ident => {
                let ident = self.parse_ident();
                Expr::Ident(ident)
            }
            // Type keywords that can also appear as expressions (for Type.method calls)
            kind if self.is_type_start(kind) => {
                let ident = self.parse_type_ident();
                Expr::Ident(ident)
            }
            _ => {
                self.error(
                    format!("expected expression, found {:?}", tok.kind),
                    tok.span,
                );
                self.advance();
                Expr::Error(tok.span)
            }
        }
    }

    fn parse_call_args(&mut self, callee: Expr) -> Expr {
        let start = callee.span();
        self.expect(TokenKind::LParen);
        let mut args = Vec::new();

        if self.peek() != TokenKind::RParen {
            args.push(self.parse_call_arg());
            while self.eat(TokenKind::Comma).is_some() {
                if self.peek() == TokenKind::RParen {
                    break;
                }
                args.push(self.parse_call_arg());
            }
        }

        let close = self.expect(TokenKind::RParen);
        Expr::Call(Box::new(callee), args, start.merge(close.span))
    }

    fn parse_call_arg(&mut self) -> CallArg {
        // Check for named argument: `name: expr`
        // But be careful — `name: expr` could also be `view name` or just `expr`.
        // Named args: Ident Colon Expr
        if (self.peek() == TokenKind::Ident || self.is_contextual_ident(self.peek()))
            && self.peek_nth(1) == TokenKind::Colon
        {
            let name_tok = self.peek_token().clone();
            let name_text = self.token_text(&name_tok).to_string();
            self.advance(); // ident
            self.advance(); // colon
            let value = self.parse_expr();
            let span = name_tok.span.merge(value.span());
            return CallArg {
                name: Some(Ident {
                    name: name_text,
                    span: name_tok.span,
                }),
                value,
                span,
            };
        }

        let value = self.parse_expr();
        let span = value.span();
        CallArg {
            name: None,
            value,
            span,
        }
    }

    fn looks_like_generic_args(&self) -> bool {
        // Quick check: `[` followed by a type name and eventually `](`
        if self.peek() != TokenKind::LBracket {
            return false;
        }
        let mut i = 1;
        let mut depth = 1;
        while depth > 0 && i < 20 {
            match self.peek_nth(i) {
                TokenKind::LBracket => depth += 1,
                TokenKind::RBracket => depth -= 1,
                TokenKind::Eof | TokenKind::Newline => return false,
                _ => {}
            }
            i += 1;
        }
        // After `]`, we should see `(`
        self.peek_nth(i) == TokenKind::LParen || depth == 0 && self.peek_nth(i - 1 + 1) == TokenKind::LParen
    }

    fn parse_generic_call(&mut self, callee: Expr) -> Expr {
        let start = callee.span();
        self.expect(TokenKind::LBracket);
        let mut type_args = Vec::new();
        if self.peek() != TokenKind::RBracket {
            type_args.push(self.parse_type());
            while self.eat(TokenKind::Comma).is_some() {
                type_args.push(self.parse_type());
            }
        }
        self.expect(TokenKind::RBracket);

        self.expect(TokenKind::LParen);
        let mut args = Vec::new();
        if self.peek() != TokenKind::RParen {
            args.push(self.parse_call_arg());
            while self.eat(TokenKind::Comma).is_some() {
                if self.peek() == TokenKind::RParen {
                    break;
                }
                args.push(self.parse_call_arg());
            }
        }
        let close = self.expect(TokenKind::RParen);
        Expr::GenericCall(Box::new(callee), type_args, args, start.merge(close.span))
    }

    // =======================================================================
    // Identifier
    // =======================================================================

    fn parse_ident(&mut self) -> Ident {
        let tok = self.peek_token().clone();
        if tok.kind == TokenKind::Ident || self.is_contextual_ident(tok.kind) {
            self.advance();
            Ident {
                name: self.token_text(&tok).to_string(),
                span: tok.span,
            }
        } else {
            self.error(
                format!("expected identifier, found {:?}", tok.kind),
                tok.span,
            );
            // Don't advance — let the caller decide what to do
            Ident {
                name: "<error>".to_string(),
                span: tok.span,
            }
        }
    }

    /// Keywords that can appear as identifiers in certain contexts
    /// (e.g., parameter names, variable names, use aliases, field names).
    fn is_contextual_ident(&self, kind: TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::Self_
                | TokenKind::Other
                | TokenKind::Error
                | TokenKind::Value
                | TokenKind::Network
                | TokenKind::Default
                | TokenKind::Ok
                | TokenKind::Fail
                | TokenKind::Clone
                | TokenKind::Send
                | TokenKind::Run
                | TokenKind::Join
                | TokenKind::Cancel
                | TokenKind::Trace
                | TokenKind::Transition
                | TokenKind::Bit
                | TokenKind::Bits
                | TokenKind::States
        )
    }
}

// ===========================================================================
// Operator precedence tables
// ===========================================================================

/// Returns (left_binding_power, right_binding_power) for infix operators.
/// Higher numbers = tighter binding.
fn infix_binding_power(kind: TokenKind) -> Option<(u8, u8)> {
    match kind {
        TokenKind::PipePipe => Some((1, 2)),
        TokenKind::AmpAmp => Some((3, 4)),
        TokenKind::EqEq | TokenKind::NotEq => Some((5, 6)),
        TokenKind::Lt | TokenKind::Gt | TokenKind::LtEq | TokenKind::GtEq => Some((7, 8)),
        TokenKind::Plus | TokenKind::Minus => Some((9, 10)),
        TokenKind::Star | TokenKind::Slash | TokenKind::Modulo => Some((11, 12)),
        _ => None,
    }
}

fn token_to_binop(kind: TokenKind) -> BinOp {
    match kind {
        TokenKind::Plus => BinOp::Add,
        TokenKind::Minus => BinOp::Sub,
        TokenKind::Star => BinOp::Mul,
        TokenKind::Slash => BinOp::Div,
        TokenKind::Modulo => BinOp::Modulo,
        TokenKind::EqEq => BinOp::Eq,
        TokenKind::NotEq => BinOp::NotEq,
        TokenKind::Lt => BinOp::Lt,
        TokenKind::Gt => BinOp::Gt,
        TokenKind::LtEq => BinOp::LtEq,
        TokenKind::GtEq => BinOp::GtEq,
        TokenKind::AmpAmp => BinOp::And,
        TokenKind::PipePipe => BinOp::Or,
        _ => unreachable!("not a binary operator: {:?}", kind),
    }
}

fn stmt_span(stmt: &Stmt) -> Span {
    match stmt {
        Stmt::VarDecl(v) => v.span,
        Stmt::Assign(a) => a.span,
        Stmt::Return(r) => r.span,
        Stmt::If(i) => i.span,
        Stmt::For(f) => f.span,
        Stmt::While(w) => w.span,
        Stmt::Match(m) => m.span,
        Stmt::Expr(e) => e.span,
        Stmt::Use(u) => u.span,
        Stmt::Assert(a) => a.span,
        Stmt::Break(s) | Stmt::Continue(s) => *s,
    }
}

// ===========================================================================
// Public convenience function
// ===========================================================================

/// Parse source text into an AST. Lexes first, then parses.
pub fn parse(source: &str, file: jett_common::FileId) -> ParseResult {
    let lex_result = jett_lexer::tokenize(source, file);

    // Convert lex errors to diagnostics
    let mut diagnostics: Vec<Diagnostic> = lex_result
        .errors
        .iter()
        .map(|e| Diagnostic::error(999, &e.message, e.span))
        .collect();

    let parser = Parser::new(source, lex_result.tokens);
    let mut result = parser.parse();
    diagnostics.append(&mut result.errors);
    result.errors = diagnostics;
    result
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use jett_common::FileId;

    fn parse_str(source: &str) -> ParseResult {
        parse(source, FileId::new(0))
    }

    // -----------------------------------------------------------------------
    // Parsing a simple function
    // -----------------------------------------------------------------------

    #[test]
    fn parse_simple_function() {
        let src = "\
function add(a: int64, b: int64) returns int64:
    return a + b
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert_eq!(result.module.items.len(), 1);
        match &result.module.items[0] {
            Item::Function(f) => {
                assert_eq!(f.name.name, "add");
                assert_eq!(f.params.len(), 2);
                assert_eq!(f.params[0].name.name, "a");
                assert_eq!(f.params[1].name.name, "b");
                assert!(f.return_type.is_some());
                assert_eq!(f.body.stmts.len(), 1);
            }
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn parse_function_returns_nothing() {
        let src = "\
function greet(view stdout: Stdout, name: string) returns nothing:
    return nothing
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => {
                assert_eq!(f.name.name, "greet");
                assert_eq!(f.params.len(), 2);
                assert!(f.params[0].view);
            }
            other => panic!("expected Function, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Parsing variable declarations
    // -----------------------------------------------------------------------

    #[test]
    fn parse_var_decl_immutable() {
        let src = "\
function main() returns nothing:
    int64 x = 42
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => {
                assert_eq!(f.body.stmts.len(), 1);
                match &f.body.stmts[0] {
                    Stmt::VarDecl(v) => {
                        assert!(!v.mutable);
                        assert_eq!(v.name.name, "x");
                        matches!(&v.value, Expr::IntLiteral(42, _));
                    }
                    other => panic!("expected VarDecl, got {:?}", other),
                }
            }
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn parse_var_decl_mutable() {
        let src = "\
function main() returns nothing:
    mutable int64 counter = 0
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => match &f.body.stmts[0] {
                Stmt::VarDecl(v) => {
                    assert!(v.mutable);
                    assert_eq!(v.name.name, "counter");
                }
                other => panic!("expected VarDecl, got {:?}", other),
            },
            other => panic!("expected Function, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Parsing if/else
    // -----------------------------------------------------------------------

    #[test]
    fn parse_if_else() {
        let src = "\
function classify(x: int64) returns string:
    if x > 0:
        return \"positive\"
    else if x == 0:
        return \"zero\"
    else:
        return \"negative\"
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => {
                assert_eq!(f.body.stmts.len(), 1);
                match &f.body.stmts[0] {
                    Stmt::If(if_stmt) => {
                        assert_eq!(if_stmt.else_ifs.len(), 1);
                        assert!(if_stmt.else_block.is_some());
                    }
                    other => panic!("expected If, got {:?}", other),
                }
            }
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn parse_if_no_else() {
        let src = "\
function check(x: int64) returns nothing:
    if x > 0:
        return nothing
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => match &f.body.stmts[0] {
                Stmt::If(if_stmt) => {
                    assert!(if_stmt.else_ifs.is_empty());
                    assert!(if_stmt.else_block.is_none());
                }
                other => panic!("expected If, got {:?}", other),
            },
            other => panic!("expected Function, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Parsing for/while loops
    // -----------------------------------------------------------------------

    #[test]
    fn parse_for_loop() {
        let src = "\
function process(items: list[string]) returns nothing:
    for item in items:
        break
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => match &f.body.stmts[0] {
                Stmt::For(for_stmt) => {
                    assert_eq!(for_stmt.variable.name, "item");
                    assert!(!for_stmt.view);
                }
                other => panic!("expected For, got {:?}", other),
            },
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn parse_for_loop_with_view() {
        let src = "\
function process(items: list[string]) returns nothing:
    for item in view items:
        continue
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => match &f.body.stmts[0] {
                Stmt::For(for_stmt) => {
                    assert!(for_stmt.view);
                }
                other => panic!("expected For, got {:?}", other),
            },
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn parse_while_loop() {
        let src = "\
function countdown(mutable count: int64) returns nothing:
    while count > 0:
        count = count - 1
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => {
                assert_eq!(f.body.stmts.len(), 1);
                match &f.body.stmts[0] {
                    Stmt::While(w) => {
                        assert_eq!(w.body.stmts.len(), 1);
                    }
                    other => panic!("expected While, got {:?}", other),
                }
            }
            other => panic!("expected Function, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Parsing struct definitions
    // -----------------------------------------------------------------------

    #[test]
    fn parse_struct_with_fields() {
        let src = "\
struct Point:
    x: float64
    y: float64
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Struct(s) => {
                assert_eq!(s.name.name, "Point");
                assert_eq!(s.fields.len(), 2);
                assert_eq!(s.fields[0].name.name, "x");
                assert_eq!(s.fields[1].name.name, "y");
                assert!(s.methods.is_empty());
            }
            other => panic!("expected Struct, got {:?}", other),
        }
    }

    #[test]
    fn parse_struct_with_methods() {
        let src = "\
struct Point:
    x: float64
    y: float64

    function distance(view self: Point, view other: Point) returns float64:
        return 0.0
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Struct(s) => {
                assert_eq!(s.fields.len(), 2);
                assert_eq!(s.methods.len(), 1);
                assert_eq!(s.methods[0].name.name, "distance");
            }
            other => panic!("expected Struct, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Parsing enum definitions
    // -----------------------------------------------------------------------

    #[test]
    fn parse_simple_enum() {
        let src = "\
enum Color:
    red
    green
    blue
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Enum(e) => {
                assert_eq!(e.name.name, "Color");
                assert_eq!(e.variants.len(), 3);
                assert_eq!(e.variants[0].name.name, "red");
                assert_eq!(e.variants[1].name.name, "green");
                assert_eq!(e.variants[2].name.name, "blue");
            }
            other => panic!("expected Enum, got {:?}", other),
        }
    }

    #[test]
    fn parse_enum_with_data() {
        let src = "\
enum Shape:
    circle(radius: float64)
    rect(width: float64, height: float64)
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Enum(e) => {
                assert_eq!(e.variants.len(), 2);
                assert_eq!(e.variants[0].fields.len(), 1);
                assert_eq!(e.variants[1].fields.len(), 2);
            }
            other => panic!("expected Enum, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Parsing expressions with operator precedence
    // -----------------------------------------------------------------------

    #[test]
    fn parse_arithmetic_precedence() {
        // `a + b * c` should parse as `a + (b * c)`
        let src = "\
function f() returns int64:
    return a + b * c
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => match &f.body.stmts[0] {
                Stmt::Return(r) => {
                    let val = r.value.as_ref().unwrap();
                    // Should be Binary(a, Add, Binary(b, Mul, c))
                    match val {
                        Expr::Binary(lhs, BinOp::Add, rhs, _) => {
                            assert!(matches!(lhs.as_ref(), Expr::Ident(_)));
                            assert!(matches!(rhs.as_ref(), Expr::Binary(_, BinOp::Mul, _, _)));
                        }
                        other => panic!("expected Binary Add, got {:?}", other),
                    }
                }
                other => panic!("expected Return, got {:?}", other),
            },
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn parse_comparison_and_logic() {
        // `x > 0 && y < 10` should parse as `(x > 0) && (y < 10)`
        let src = "\
function f() returns bool:
    return x > 0 && y < 10
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => match &f.body.stmts[0] {
                Stmt::Return(r) => {
                    let val = r.value.as_ref().unwrap();
                    match val {
                        Expr::Binary(_, BinOp::And, _, _) => { /* correct */ }
                        other => panic!("expected Binary And, got {:?}", other),
                    }
                }
                other => panic!("expected Return, got {:?}", other),
            },
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn parse_unary_not() {
        let src = "\
function f() returns bool:
    return not x
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => match &f.body.stmts[0] {
                Stmt::Return(r) => {
                    let val = r.value.as_ref().unwrap();
                    assert!(matches!(val, Expr::Unary(UnaryOp::Not, _, _)));
                }
                other => panic!("expected Return, got {:?}", other),
            },
            other => panic!("expected Function, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Parsing function calls and field access
    // -----------------------------------------------------------------------

    #[test]
    fn parse_function_call() {
        let src = "\
function f() returns nothing:
    foo(1, 2, 3)
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => match &f.body.stmts[0] {
                Stmt::Expr(e) => match &e.expr {
                    Expr::Call(callee, args, _) => {
                        assert!(matches!(callee.as_ref(), Expr::Ident(_)));
                        assert_eq!(args.len(), 3);
                    }
                    other => panic!("expected Call, got {:?}", other),
                },
                other => panic!("expected Expr, got {:?}", other),
            },
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn parse_method_call() {
        let src = "\
function f() returns nothing:
    Point.distance(view p1, view p2)
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => match &f.body.stmts[0] {
                Stmt::Expr(e) => match &e.expr {
                    Expr::Call(callee, args, _) => {
                        // callee should be FieldAccess(Ident("Point"), "distance")
                        assert!(matches!(callee.as_ref(), Expr::FieldAccess(_, _, _)));
                        assert_eq!(args.len(), 2);
                    }
                    other => panic!("expected Call, got {:?}", other),
                },
                other => panic!("expected Expr, got {:?}", other),
            },
            other => panic!("expected Function, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Namespace
    // -----------------------------------------------------------------------

    #[test]
    fn parse_namespace() {
        let src = "namespace myapp\n";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert_eq!(result.module.items.len(), 1);
        match &result.module.items[0] {
            Item::Namespace(ns) => {
                assert_eq!(ns.name.name, "myapp");
            }
            other => panic!("expected Namespace, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Use declarations
    // -----------------------------------------------------------------------

    #[test]
    fn parse_use_with_alias() {
        let src = "\
function f() returns nothing:
    use math
    use net as network
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => {
                assert_eq!(f.body.stmts.len(), 2);
                match &f.body.stmts[0] {
                    Stmt::Use(u) => {
                        assert_eq!(u.path.name, "math");
                        assert!(u.alias.is_none());
                    }
                    other => panic!("expected Use, got {:?}", other),
                }
                match &f.body.stmts[1] {
                    Stmt::Use(u) => {
                        assert_eq!(u.path.name, "net");
                        assert_eq!(u.alias.as_ref().unwrap().name, "network");
                    }
                    other => panic!("expected Use, got {:?}", other),
                }
            }
            other => panic!("expected Function, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Assert
    // -----------------------------------------------------------------------

    #[test]
    fn parse_assert() {
        let src = "\
function f() returns nothing:
    assert x > 0
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => match &f.body.stmts[0] {
                Stmt::Assert(a) => {
                    assert!(a.message.is_none());
                }
                other => panic!("expected Assert, got {:?}", other),
            },
            other => panic!("expected Function, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Error recovery
    // -----------------------------------------------------------------------

    #[test]
    fn error_recovery_continues_after_bad_statement() {
        let src = "\
function f() returns nothing:
    ??? bad
    int64 x = 42
";
        let result = parse_str(src);
        // Should have at least one error from the lexer or parser
        assert!(!result.errors.is_empty());
        // But should still have parsed the function with at least some content
        assert_eq!(result.module.items.len(), 1);
        match &result.module.items[0] {
            Item::Function(f) => {
                // We recovered and parsed the valid var decl
                let has_var_decl = f.body.stmts.iter().any(|s| matches!(s, Stmt::VarDecl(_)));
                assert!(has_var_decl, "should have recovered and parsed var decl, stmts: {:?}", f.body.stmts);
            }
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn error_recovery_multiple_items() {
        // Even if first item fails, second should parse
        let src = "\
namespace myapp

function f() returns nothing:
    return nothing
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert_eq!(result.module.items.len(), 2);
    }

    // -----------------------------------------------------------------------
    // List and map construction
    // -----------------------------------------------------------------------

    #[test]
    fn parse_list_construction() {
        let src = "\
function f() returns nothing:
    list[string] names = list(\"alice\", \"bob\")
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => match &f.body.stmts[0] {
                Stmt::VarDecl(v) => match &v.value {
                    Expr::ListConstruct(items, _) => {
                        assert_eq!(items.len(), 2);
                    }
                    other => panic!("expected ListConstruct, got {:?}", other),
                },
                other => panic!("expected VarDecl, got {:?}", other),
            },
            other => panic!("expected Function, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Handle blocks
    // -----------------------------------------------------------------------

    #[test]
    fn parse_handle_error_block() {
        let src = "\
function f() returns nothing:
    string content = read_file(path) handle error:
        return nothing
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => match &f.body.stmts[0] {
                Stmt::VarDecl(v) => match &v.value {
                    Expr::Handle(_, error_name, _, _) => {
                        assert!(error_name.is_some());
                        assert_eq!(error_name.as_ref().unwrap().name, "error");
                    }
                    other => panic!("expected Handle, got {:?}", other),
                },
                other => panic!("expected VarDecl, got {:?}", other),
            },
            other => panic!("expected Function, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Global variable declarations
    // -----------------------------------------------------------------------

    #[test]
    fn parse_global_constants() {
        let src = "\
namespace config

int64 MAX_RETRIES = 5
string DEFAULT_HOST = \"localhost\"
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert_eq!(result.module.items.len(), 3);
        match &result.module.items[1] {
            Item::VarDecl(v) => {
                assert_eq!(v.name.name, "MAX_RETRIES");
            }
            other => panic!("expected VarDecl, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Assignment
    // -----------------------------------------------------------------------

    #[test]
    fn parse_assignment() {
        let src = "\
function f(mutable x: int64) returns nothing:
    x = x + 1
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => match &f.body.stmts[0] {
                Stmt::Assign(a) => {
                    assert!(matches!(&a.target, Expr::Ident(_)));
                }
                other => panic!("expected Assign, got {:?}", other),
            },
            other => panic!("expected Function, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Match statements
    // -----------------------------------------------------------------------

    #[test]
    fn parse_match_simple_patterns() {
        let src = "\
function f(color: Color) returns string:
    match color:
        red:
            return \"red\"
        green:
            return \"green\"
        blue:
            return \"blue\"
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => {
                assert_eq!(f.body.stmts.len(), 1);
                match &f.body.stmts[0] {
                    Stmt::Match(m) => {
                        assert_eq!(m.arms.len(), 3);
                        assert!(matches!(&m.arms[0].pattern, Pattern::Ident(id) if id.name == "red"));
                        assert!(matches!(&m.arms[1].pattern, Pattern::Ident(id) if id.name == "green"));
                        assert!(matches!(&m.arms[2].pattern, Pattern::Ident(id) if id.name == "blue"));
                        // Each arm body has one return statement
                        assert_eq!(m.arms[0].body.stmts.len(), 1);
                    }
                    other => panic!("expected Match, got {:?}", other),
                }
            }
            other => panic!("expected Function, got {:?}", other),
        }
    }

    #[test]
    fn parse_match_with_destructuring() {
        let src = "\
function f(shape: Shape) returns nothing:
    match shape:
        circle(r):
            return r
        rect(w, h):
            return w
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        match &result.module.items[0] {
            Item::Function(f) => match &f.body.stmts[0] {
                Stmt::Match(m) => {
                    assert_eq!(m.arms.len(), 2);
                    match &m.arms[0].pattern {
                        Pattern::Variant(name, bindings) => {
                            assert_eq!(name.name, "circle");
                            assert_eq!(bindings.len(), 1);
                            assert_eq!(bindings[0].name, "r");
                        }
                        other => panic!("expected Variant pattern, got {:?}", other),
                    }
                    match &m.arms[1].pattern {
                        Pattern::Variant(name, bindings) => {
                            assert_eq!(name.name, "rect");
                            assert_eq!(bindings.len(), 2);
                            assert_eq!(bindings[0].name, "w");
                            assert_eq!(bindings[1].name, "h");
                        }
                        other => panic!("expected Variant pattern, got {:?}", other),
                    }
                }
                other => panic!("expected Match, got {:?}", other),
            },
            other => panic!("expected Function, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Verify blocks
    // -----------------------------------------------------------------------

    #[test]
    fn parse_function_with_verify_block() {
        let src = "\
function add(a: int64, b: int64) returns int64:
    return a + b

verify add:
    assert add(2, 3) == 5
    assert add(0, 0) == 0
";
        let result = parse_str(src);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert_eq!(result.module.items.len(), 2);

        // First item is the function
        match &result.module.items[0] {
            Item::Function(f) => {
                assert_eq!(f.name.name, "add");
                assert_eq!(f.params.len(), 2);
            }
            other => panic!("expected Function, got {:?}", other),
        }

        // Second item is the verify block
        match &result.module.items[1] {
            Item::Verify(vb) => {
                assert_eq!(vb.name.name, "add");
                assert_eq!(vb.body.stmts.len(), 2);
                assert!(matches!(&vb.body.stmts[0], Stmt::Assert(_)));
                assert!(matches!(&vb.body.stmts[1], Stmt::Assert(_)));
            }
            other => panic!("expected Verify, got {:?}", other),
        }
    }
}
