//! Jett's typed, backend-neutral high-level intermediate representation.
//!
//! The initial lowering slice deliberately covers ordinary monomorphic
//! functions and core structured control flow. Unsupported source constructs
//! fail explicitly; they never survive as embedded AST nodes.

use std::collections::HashMap;

use jett_common::{FileId, SourceOrigin, Span};
use jett_parser::ast::{self, Expr, Item, Module, Stmt};
use jett_resolve::{DefId, DefKind, ResolveResult};
use jett_typecheck::CheckResult;
use jett_types::{Type, TypeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId(u32);

impl FunctionId {
    pub fn index(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(u32);

impl LocalId {
    pub fn index(self) -> u32 {
        self.0
    }
}

/// Canonical declaration identity. Resolver `DefId`s are not part of it
/// because they are allocated afresh in each compiler session.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeclarationId {
    pub origin: SourceOrigin,
    pub namespace: String,
    pub name: String,
    pub kind: DeclarationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeclarationKind {
    Function,
}

/// One concrete in-memory function identity. Type arguments are empty for an
/// ordinary function and form part of identity after monomorphization. Raw
/// `TypeId`s are session-local; persistent artifacts must encode their
/// canonical structural type identities instead.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionIdentity {
    pub declaration: DeclarationId,
    pub type_arguments: Vec<TypeId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub id: FunctionId,
    pub identity: FunctionIdentity,
    pub source_definition: DefId,
    pub params: Vec<Param>,
    pub return_type: TypeId,
    pub locals: Vec<Local>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub local: LocalId,
    pub name: String,
    pub ty: TypeId,
    pub mode: ParamMode,
    pub mutable: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamMode {
    Owned,
    View,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Local {
    pub id: LocalId,
    pub name: String,
    pub ty: TypeId,
    pub mutable: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatementKind {
    Let {
        local: LocalId,
        value: Expression,
    },
    Assign {
        target: Expression,
        value: Expression,
    },
    Return(Option<Expression>),
    Expression(Expression),
    If {
        condition: Expression,
        then_block: Block,
        else_block: Option<Block>,
    },
    While {
        condition: Expression,
        body: Block,
    },
    Break,
    Continue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expression {
    pub kind: ExpressionKind,
    pub ty: TypeId,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExpressionKind {
    Int(i128),
    Float(f64),
    String(String),
    Bool(bool),
    Nothing,
    Local(LocalId),
    Binary {
        left: Box<Expression>,
        op: BinaryOp,
        right: Box<Expression>,
    },
    Unary {
        op: UnaryOp,
        value: Box<Expression>,
    },
    Call {
        function: FunctionId,
        args: Vec<Expression>,
    },
    View(Box<Expression>),
    Clone(Box<Expression>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Equal,
    NotEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Negate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LowerError {
    pub span: Span,
    pub message: String,
}

/// Lower the currently supported checked source subset into HIR.
///
/// `origins` is mandatory: inferring authority from numeric `FileId` ranges
/// would allow source provenance to change compiler policy.
pub fn lower(
    module: &Module,
    resolve: &ResolveResult,
    check: &CheckResult,
    origins: &HashMap<FileId, SourceOrigin>,
) -> Result<Program, Vec<LowerError>> {
    Lowerer::new(module, resolve, check, origins).lower()
}

struct FunctionSource<'a> {
    id: FunctionId,
    definition: DefId,
    function: &'a ast::FunctionDef,
}

struct Lowerer<'a> {
    module: &'a Module,
    resolve: &'a ResolveResult,
    check: &'a CheckResult,
    origins: &'a HashMap<FileId, SourceOrigin>,
    functions: Vec<FunctionSource<'a>>,
    function_ids: HashMap<DefId, FunctionId>,
    errors: Vec<LowerError>,
}

impl<'a> Lowerer<'a> {
    fn new(
        module: &'a Module,
        resolve: &'a ResolveResult,
        check: &'a CheckResult,
        origins: &'a HashMap<FileId, SourceOrigin>,
    ) -> Self {
        Self {
            module,
            resolve,
            check,
            origins,
            functions: Vec::new(),
            function_ids: HashMap::new(),
            errors: Vec::new(),
        }
    }

    fn lower(mut self) -> Result<Program, Vec<LowerError>> {
        self.collect_functions();
        let sources = std::mem::take(&mut self.functions);
        let mut functions = Vec::with_capacity(sources.len());
        for source in sources {
            if let Some(function) = self.lower_function(source) {
                functions.push(function);
            }
        }
        if self.errors.is_empty() {
            Ok(Program { functions })
        } else {
            Err(self.errors)
        }
    }

    fn collect_functions(&mut self) {
        for item in &self.module.items {
            match item {
                Item::Function(function) if function.type_params.is_empty() => {
                    let Some(definition) =
                        self.definition_at(function.name.span, DefKind::Function)
                    else {
                        self.error(function.name.span, "function has no resolved definition");
                        continue;
                    };
                    let id = FunctionId(self.functions.len() as u32);
                    self.function_ids.insert(definition, id);
                    self.functions.push(FunctionSource {
                        id,
                        definition,
                        function,
                    });
                }
                _ => {}
            }
        }
    }

    fn lower_function(&mut self, source: FunctionSource<'a>) -> Option<Function> {
        let origin = match self.origins.get(&source.function.span.file) {
            Some(origin) => origin.clone(),
            None => {
                self.error(
                    source.function.span,
                    "source origin is missing for function",
                );
                return None;
            }
        };
        let function_ty = match self.check.definition_types.get(&source.definition) {
            Some(ty) => *ty,
            None => {
                self.error(source.function.name.span, "function has no checked type");
                return None;
            }
        };
        let (parameter_types, return_type) = match self.check.interner.resolve(function_ty) {
            Type::Function {
                params,
                return_type,
            } => (params.clone(), *return_type),
            _ => {
                self.error(
                    source.function.name.span,
                    "definition is not a checked function",
                );
                return None;
            }
        };
        if parameter_types.len() != source.function.params.len() {
            self.error(
                source.function.span,
                "checked function parameter count changed",
            );
            return None;
        }

        let function_ids = self.function_ids.clone();
        let mut body_lowerer = BodyLowerer::new(self, &function_ids);
        let mut params = Vec::with_capacity(source.function.params.len());
        for (param, ty) in source.function.params.iter().zip(parameter_types) {
            let Some(definition) = body_lowerer
                .parent
                .definition_at(param.name.span, DefKind::Param)
            else {
                body_lowerer
                    .parent
                    .error(param.name.span, "parameter has no resolved definition");
                continue;
            };
            let local = body_lowerer.allocate_local(
                definition,
                &param.name.name,
                ty,
                param.mutable,
                param.span,
            );
            params.push(Param {
                local,
                name: param.name.name.clone(),
                ty,
                mode: if param.view {
                    ParamMode::View
                } else {
                    ParamMode::Owned
                },
                mutable: param.mutable,
                span: param.span,
            });
        }
        let body = body_lowerer.lower_block(&source.function.body);
        let locals = body_lowerer.locals;
        let definition = self.resolve.scope_table.def(source.definition);

        Some(Function {
            id: source.id,
            identity: FunctionIdentity {
                declaration: DeclarationId {
                    origin,
                    namespace: definition.namespace.clone().unwrap_or_default(),
                    name: source.function.name.name.clone(),
                    kind: DeclarationKind::Function,
                },
                type_arguments: Vec::new(),
            },
            source_definition: source.definition,
            params,
            return_type,
            locals,
            body,
            span: source.function.span,
        })
    }

    fn definition_at(&self, span: Span, kind: DefKind) -> Option<DefId> {
        self.resolve
            .scope_table
            .definitions
            .iter()
            .find(|definition| definition.span == span && definition.kind == kind)
            .map(|definition| definition.id)
    }

    fn error(&mut self, span: Span, message: impl Into<String>) {
        self.errors.push(LowerError {
            span,
            message: message.into(),
        });
    }
}

struct BodyLowerer<'lowerer, 'program> {
    parent: &'lowerer mut Lowerer<'program>,
    function_ids: &'lowerer HashMap<DefId, FunctionId>,
    local_ids: HashMap<DefId, LocalId>,
    locals: Vec<Local>,
}

impl<'lowerer, 'program> BodyLowerer<'lowerer, 'program> {
    fn new(
        parent: &'lowerer mut Lowerer<'program>,
        function_ids: &'lowerer HashMap<DefId, FunctionId>,
    ) -> Self {
        Self {
            parent,
            function_ids,
            local_ids: HashMap::new(),
            locals: Vec::new(),
        }
    }

    fn allocate_local(
        &mut self,
        definition: DefId,
        name: &str,
        ty: TypeId,
        mutable: bool,
        span: Span,
    ) -> LocalId {
        let id = LocalId(self.locals.len() as u32);
        self.local_ids.insert(definition, id);
        self.locals.push(Local {
            id,
            name: name.to_string(),
            ty,
            mutable,
            span,
        });
        id
    }

    fn lower_block(&mut self, block: &ast::Block) -> Block {
        let mut statements = Vec::new();
        for statement in &block.stmts {
            if let Some(statement) = self.lower_statement(statement) {
                statements.push(statement);
            }
        }
        Block {
            statements,
            span: block.span,
        }
    }

    fn lower_statement(&mut self, statement: &Stmt) -> Option<Statement> {
        let (kind, span) = match statement {
            Stmt::VarDecl(decl) => {
                let Some(definition) = self.parent.definition_at(decl.name.span, DefKind::Variable)
                else {
                    self.parent
                        .error(decl.name.span, "local has no resolved definition");
                    return None;
                };
                let Some(ty) = self
                    .parent
                    .check
                    .definition_types
                    .get(&definition)
                    .copied()
                    .or_else(|| self.parent.check.type_map.get(&decl.value.span()).copied())
                else {
                    self.parent
                        .error(decl.name.span, "local has no checked type");
                    return None;
                };
                let local =
                    self.allocate_local(definition, &decl.name.name, ty, decl.mutable, decl.span);
                let value = self.lower_expression(&decl.value)?;
                (StatementKind::Let { local, value }, decl.span)
            }
            Stmt::Assign(assign) => (
                StatementKind::Assign {
                    target: self.lower_expression(&assign.target)?,
                    value: self.lower_expression(&assign.value)?,
                },
                assign.span,
            ),
            Stmt::Return(ret) => {
                let value = match &ret.value {
                    Some(value) => Some(self.lower_expression(value)?),
                    None => None,
                };
                (StatementKind::Return(value), ret.span)
            }
            Stmt::Expr(expr) => (
                StatementKind::Expression(self.lower_expression(&expr.expr)?),
                expr.span,
            ),
            Stmt::If(branch) => {
                let condition = self.lower_expression(&branch.condition)?;
                let then_block = self.lower_block(&branch.then_block);
                let else_block =
                    self.lower_else_chain(&branch.else_ifs, branch.else_block.as_ref());
                (
                    StatementKind::If {
                        condition,
                        then_block,
                        else_block,
                    },
                    branch.span,
                )
            }
            Stmt::While(loop_stmt) => (
                StatementKind::While {
                    condition: self.lower_expression(&loop_stmt.condition)?,
                    body: self.lower_block(&loop_stmt.body),
                },
                loop_stmt.span,
            ),
            Stmt::Break(span) => (StatementKind::Break, *span),
            Stmt::Continue(span) => (StatementKind::Continue, *span),
            Stmt::Use(_) => return None,
            unsupported => {
                self.parent.error(
                    statement_span(unsupported),
                    "source statement is not in the initial HIR lowering subset",
                );
                return None;
            }
        };
        Some(Statement { kind, span })
    }

    fn lower_else_chain(
        &mut self,
        else_ifs: &[(Expr, ast::Block)],
        final_else: Option<&ast::Block>,
    ) -> Option<Block> {
        let mut tail = final_else.map(|block| self.lower_block(block));
        for (condition, block) in else_ifs.iter().rev() {
            let condition = self.lower_expression(condition)?;
            let then_block = self.lower_block(block);
            let span = condition.span.merge(block.span);
            tail = Some(Block {
                statements: vec![Statement {
                    kind: StatementKind::If {
                        condition,
                        then_block,
                        else_block: tail,
                    },
                    span,
                }],
                span,
            });
        }
        tail
    }

    fn lower_expression(&mut self, expression: &Expr) -> Option<Expression> {
        let span = expression.span();
        let ty = self.expression_type(expression)?;
        let kind = match expression {
            Expr::IntLiteral(value, _) => ExpressionKind::Int(*value),
            Expr::FloatLiteral(value, _) => ExpressionKind::Float(*value),
            Expr::StringLiteral(value, _) => ExpressionKind::String(value.clone()),
            Expr::BoolLiteral(value, _) => ExpressionKind::Bool(*value),
            Expr::Nothing(_) => ExpressionKind::Nothing,
            Expr::Ident(ident) => {
                let Some(definition) = self.parent.resolve.resolutions.get(&ident.span).copied()
                else {
                    self.parent
                        .error(ident.span, "identifier has no resolved definition");
                    return None;
                };
                let Some(local) = self.local_ids.get(&definition).copied() else {
                    self.parent.error(
                        span,
                        "identifier is not a local value in the initial HIR subset",
                    );
                    return None;
                };
                ExpressionKind::Local(local)
            }
            Expr::Binary(left, op, right, _) => ExpressionKind::Binary {
                left: Box::new(self.lower_expression(left)?),
                op: lower_binary_op(*op),
                right: Box::new(self.lower_expression(right)?),
            },
            Expr::Unary(op, value, _) => ExpressionKind::Unary {
                op: lower_unary_op(*op),
                value: Box::new(self.lower_expression(value)?),
            },
            Expr::Call(callee, args, _) => {
                let Expr::Ident(ident) = callee.as_ref() else {
                    self.parent.error(
                        callee.span(),
                        "only direct user-function calls are in the initial HIR subset",
                    );
                    return None;
                };
                let Some(definition) = self.parent.resolve.resolutions.get(&ident.span).copied()
                else {
                    self.parent
                        .error(ident.span, "call target has no resolved definition");
                    return None;
                };
                let Some(function) = self.function_ids.get(&definition).copied() else {
                    self.parent
                        .error(ident.span, "call target is not an initial HIR function");
                    return None;
                };
                let mut lowered_args = Vec::with_capacity(args.len());
                for arg in args {
                    if arg.name.is_some() {
                        self.parent.error(
                            arg.span,
                            "named arguments are not yet lowered to positional HIR arguments",
                        );
                        return None;
                    }
                    lowered_args.push(self.lower_expression(&arg.value)?);
                }
                ExpressionKind::Call {
                    function,
                    args: lowered_args,
                }
            }
            Expr::Paren(inner, _) => {
                return self.lower_expression(inner).map(|mut lowered| {
                    lowered.span = span;
                    lowered.ty = ty;
                    lowered
                });
            }
            Expr::View(value, _) => ExpressionKind::View(Box::new(self.lower_expression(value)?)),
            Expr::Clone(value, _) => ExpressionKind::Clone(Box::new(self.lower_expression(value)?)),
            unsupported => {
                self.parent.error(
                    unsupported.span(),
                    "source expression is not in the initial HIR lowering subset",
                );
                return None;
            }
        };
        Some(Expression { kind, ty, span })
    }

    fn expression_type(&mut self, expression: &Expr) -> Option<TypeId> {
        if let Some(ty) = self.parent.check.type_map.get(&expression.span()).copied() {
            return Some(ty);
        }
        if let Expr::Ident(ident) = expression
            && let Some(definition) = self.parent.resolve.resolutions.get(&ident.span)
            && let Some(ty) = self.parent.check.definition_types.get(definition)
        {
            return Some(*ty);
        }
        self.parent
            .error(expression.span(), "expression has no checked type");
        None
    }
}

fn lower_binary_op(op: ast::BinOp) -> BinaryOp {
    match op {
        ast::BinOp::Add => BinaryOp::Add,
        ast::BinOp::Sub => BinaryOp::Subtract,
        ast::BinOp::Mul => BinaryOp::Multiply,
        ast::BinOp::Div => BinaryOp::Divide,
        ast::BinOp::Modulo => BinaryOp::Modulo,
        ast::BinOp::Eq => BinaryOp::Equal,
        ast::BinOp::NotEq => BinaryOp::NotEqual,
        ast::BinOp::Lt => BinaryOp::Less,
        ast::BinOp::Gt => BinaryOp::Greater,
        ast::BinOp::LtEq => BinaryOp::LessEqual,
        ast::BinOp::GtEq => BinaryOp::GreaterEqual,
        ast::BinOp::And => BinaryOp::And,
        ast::BinOp::Or => BinaryOp::Or,
    }
}

fn lower_unary_op(op: ast::UnaryOp) -> UnaryOp {
    match op {
        ast::UnaryOp::Not => UnaryOp::Not,
        ast::UnaryOp::Neg => UnaryOp::Negate,
    }
}

fn statement_span(statement: &Stmt) -> Span {
    match statement {
        Stmt::VarDecl(value) => value.span,
        Stmt::Assign(value) => value.span,
        Stmt::Return(value) => value.span,
        Stmt::Respond(value) => value.span,
        Stmt::ComptimeTypeBind(value) => value.span,
        Stmt::If(value) => value.span,
        Stmt::For(value) => value.span,
        Stmt::While(value) => value.span,
        Stmt::Match(value) => value.span,
        Stmt::Expr(value) => value.span,
        Stmt::Use(value) => value.span,
        Stmt::Assert(value) => value.span,
        Stmt::Trace(value) => value.span,
        Stmt::Breakpoint(value) => value.span,
        Stmt::Break(span) | Stmt::Continue(span) => *span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jett_common::FileId;
    use jett_diagnostics::Severity;

    fn lower_source(source: &str) -> Program {
        let file = FileId::new(0);
        let parsed = jett_parser::parse(source, file);
        assert!(
            parsed.errors.is_empty(),
            "parse errors: {:?}",
            parsed.errors
        );
        let resolved = jett_resolve::resolve(&parsed.module);
        assert!(
            resolved
                .diagnostics
                .iter()
                .all(|d| d.severity != Severity::Error),
            "resolve errors: {:?}",
            resolved.diagnostics
        );
        let checked = jett_typecheck::check(&parsed.module, &resolved);
        assert!(
            checked
                .diagnostics
                .iter()
                .all(|d| d.severity != Severity::Error),
            "type errors: {:?}",
            checked.diagnostics
        );
        let origins = HashMap::from([(file, SourceOrigin::Project)]);
        lower(&parsed.module, &resolved, &checked, &origins).expect("HIR lowering failed")
    }

    #[test]
    fn lowers_typed_functions_calls_and_control_flow_deterministically() {
        let source = r#"namespace app
function add(a: int64, b: int64) returns int64:
    return a + b
function choose(flag: bool, a: int64, b: int64) returns int64:
    if flag:
        return add(a, b)
    else:
        return b
"#;
        let first = lower_source(source);
        let second = lower_source(source);
        assert_eq!(first, second);
        assert_eq!(first.functions.len(), 2);

        let add = &first.functions[0];
        assert_eq!(add.id.index(), 0);
        assert_eq!(add.identity.declaration.namespace, "app");
        assert_eq!(add.identity.declaration.name, "add");
        assert_eq!(add.identity.declaration.origin, SourceOrigin::Project);
        assert_eq!(add.params.len(), 2);
        assert_eq!(add.locals.len(), 2);

        let choose = &first.functions[1];
        assert_eq!(choose.id.index(), 1);
        assert_eq!(choose.params.len(), 3);
        let StatementKind::If { then_block, .. } = &choose.body.statements[0].kind else {
            panic!("expected lowered if statement");
        };
        let StatementKind::Return(Some(Expression {
            kind: ExpressionKind::Call { function, .. },
            ..
        })) = &then_block.statements[0].kind
        else {
            panic!("expected lowered direct call");
        };
        assert_eq!(*function, add.id);
    }

    #[test]
    fn requires_explicit_source_origin() {
        let source = "function main() returns nothing:\n    return\n";
        let file = FileId::new(0);
        let parsed = jett_parser::parse(source, file);
        let resolved = jett_resolve::resolve(&parsed.module);
        let checked = jett_typecheck::check(&parsed.module, &resolved);
        let errors = lower(&parsed.module, &resolved, &checked, &HashMap::new())
            .expect_err("missing authority must fail");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("source origin"))
        );
    }

    #[test]
    fn lowers_mutable_locals_assignments_and_loops() {
        let source = r#"namespace app
function count_to(limit: int64) returns int64:
    mutable int64 current = 0
    while current < limit:
        current = current + 1
    return current
"#;
        let program = lower_source(source);
        let function = &program.functions[0];
        assert_eq!(function.locals.len(), 2);
        assert!(function.locals[1].mutable);
        assert!(matches!(
            function.body.statements[0].kind,
            StatementKind::Let { .. }
        ));
        let StatementKind::While { body, .. } = &function.body.statements[1].kind else {
            panic!("expected lowered while statement");
        };
        assert!(matches!(
            body.statements[0].kind,
            StatementKind::Assign { .. }
        ));
    }
}
