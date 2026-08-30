//! Jett's typed, backend-neutral high-level intermediate representation.
//!
//! The initial lowering covers ordinary and generic functions plus core
//! structured control flow. Unsupported source constructs fail explicitly;
//! they never survive as embedded AST nodes.

use std::collections::HashMap;

use jett_common::{FileId, SourceOrigin, Span};
use jett_parser::ast::{self, Expr, Item, Module, Stmt};
use jett_resolve::{DefId, DefKind, ResolveResult};
use jett_typecheck::{
    CheckResult, CheckedCallArgumentOrder, CheckedGenericCall, CheckedGenericFunctionInstantiation,
    CheckedMethodCall, CheckedMethodDefinition, CheckedStructConstruction,
};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FieldId(u32);

impl FieldId {
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
    Method,
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
    pub source_definition: Option<DefId>,
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
        /// Parameter indexes in lexical source evaluation order.
        evaluation_order: Vec<usize>,
    },
    StructConstruct {
        struct_type: TypeId,
        fields: Vec<Expression>,
        /// Field indexes in lexical source evaluation order.
        evaluation_order: Vec<usize>,
        validates_refinements: bool,
    },
    ListConstruct {
        elements: Vec<Expression>,
    },
    MapConstruct {
        entries: Vec<MapEntry>,
    },
    Field {
        base: Box<Expression>,
        owner_type: TypeId,
        field: FieldId,
    },
    View(Box<Expression>),
    Clone(Box<Expression>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapEntry {
    pub key: Expression,
    pub value: Expression,
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
    definition: Option<DefId>,
    function: &'a ast::FunctionDef,
    instantiation: Option<CheckedGenericFunctionInstantiation>,
    method: Option<CheckedMethodDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum FunctionKey {
    Definition {
        definition: DefId,
        concrete_args: Vec<TypeId>,
    },
    Method {
        source_span: Span,
    },
}

struct Lowerer<'a> {
    module: &'a Module,
    resolve: &'a ResolveResult,
    check: &'a CheckResult,
    origins: &'a HashMap<FileId, SourceOrigin>,
    functions: Vec<FunctionSource<'a>>,
    function_ids: HashMap<FunctionKey, FunctionId>,
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
        let mut generic_templates = HashMap::new();
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
                    self.function_ids.insert(
                        FunctionKey::Definition {
                            definition,
                            concrete_args: Vec::new(),
                        },
                        id,
                    );
                    self.functions.push(FunctionSource {
                        id,
                        definition: Some(definition),
                        function,
                        instantiation: None,
                        method: None,
                    });
                }
                Item::Function(function) => {
                    if let Some(definition) =
                        self.definition_at(function.name.span, DefKind::Function)
                    {
                        generic_templates.insert(definition, function);
                    }
                }
                _ => {}
            }
        }

        for instantiation in self.check.generic_function_instantiations.clone() {
            let Some(function) = generic_templates.get(&instantiation.definition).copied() else {
                let span = self.resolve.scope_table.def(instantiation.definition).span;
                self.error(
                    span,
                    "generic instantiation has no top-level function template",
                );
                continue;
            };
            let id = FunctionId(self.functions.len() as u32);
            self.function_ids.insert(
                FunctionKey::Definition {
                    definition: instantiation.definition,
                    concrete_args: instantiation.concrete_args.clone(),
                },
                id,
            );
            self.functions.push(FunctionSource {
                id,
                definition: Some(instantiation.definition),
                function,
                instantiation: Some(instantiation),
                method: None,
            });
        }

        for method in self.check.method_definitions.clone() {
            let Some(function) = self.method_at(method.source_span) else {
                self.error(method.source_span, "checked method has no source body");
                continue;
            };
            let id = FunctionId(self.functions.len() as u32);
            self.function_ids.insert(
                FunctionKey::Method {
                    source_span: method.source_span,
                },
                id,
            );
            self.functions.push(FunctionSource {
                id,
                definition: None,
                function,
                instantiation: None,
                method: Some(method),
            });
        }
    }

    fn method_at(&self, span: Span) -> Option<&'a ast::FunctionDef> {
        self.module.items.iter().find_map(|item| match item {
            Item::Struct(definition) => {
                definition.methods.iter().find(|method| method.span == span)
            }
            Item::Implement(block) => block.methods.iter().find(|method| method.span == span),
            _ => None,
        })
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
        let (
            parameter_types,
            return_type,
            expression_types,
            generic_calls,
            call_argument_orders,
            method_calls,
            struct_constructions,
            concrete_args,
        ) = if let Some(instantiation) = &source.instantiation {
            (
                instantiation.parameter_types.clone(),
                instantiation.return_type,
                instantiation.type_map.clone(),
                instantiation.generic_calls.clone(),
                instantiation.call_argument_orders.clone(),
                instantiation.method_calls.clone(),
                instantiation.struct_constructions.clone(),
                instantiation.concrete_args.clone(),
            )
        } else if let Some(method) = &source.method {
            (
                method.parameter_types.clone(),
                method.return_type,
                self.check.type_map.clone(),
                self.check.generic_calls.clone(),
                self.check.call_argument_orders.clone(),
                self.check.method_calls.clone(),
                self.check.struct_constructions.clone(),
                Vec::new(),
            )
        } else {
            let Some(definition) = source.definition else {
                self.error(
                    source.function.span,
                    "function source has no checked identity",
                );
                return None;
            };
            let function_ty = match self.check.definition_types.get(&definition) {
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
            (
                parameter_types,
                return_type,
                self.check.type_map.clone(),
                self.check.generic_calls.clone(),
                self.check.call_argument_orders.clone(),
                self.check.method_calls.clone(),
                self.check.struct_constructions.clone(),
                Vec::new(),
            )
        };
        if parameter_types.len() != source.function.params.len() {
            self.error(
                source.function.span,
                "checked function parameter count changed",
            );
            return None;
        }

        let function_ids = self.function_ids.clone();
        let mut body_lowerer = BodyLowerer::new(
            self,
            &function_ids,
            &expression_types,
            &generic_calls,
            &call_argument_orders,
            &method_calls,
            &struct_constructions,
        );
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
        let (namespace, name, kind) = if let Some(method) = &source.method {
            if let Some(interface) = &method.interface_name {
                (
                    String::new(),
                    format!(
                        "{} as {interface}.{}",
                        method.owner_name, source.function.name.name
                    ),
                    DeclarationKind::Method,
                )
            } else {
                let (namespace, owner) = method
                    .owner_name
                    .rsplit_once('.')
                    .map(|(namespace, owner)| (namespace.to_string(), owner.to_string()))
                    .unwrap_or_else(|| (String::new(), method.owner_name.clone()));
                (
                    namespace,
                    format!("{owner}.{}", source.function.name.name),
                    DeclarationKind::Method,
                )
            }
        } else {
            let definition = self.resolve.scope_table.def(
                source
                    .definition
                    .expect("ordinary function definition checked above"),
            );
            (
                definition.namespace.clone().unwrap_or_default(),
                source.function.name.name.clone(),
                DeclarationKind::Function,
            )
        };

        Some(Function {
            id: source.id,
            identity: FunctionIdentity {
                declaration: DeclarationId {
                    origin,
                    namespace,
                    name,
                    kind,
                },
                type_arguments: concrete_args,
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
    function_ids: &'lowerer HashMap<FunctionKey, FunctionId>,
    expression_types: &'lowerer HashMap<Span, TypeId>,
    generic_calls: &'lowerer HashMap<Span, CheckedGenericCall>,
    call_argument_orders: &'lowerer HashMap<Span, CheckedCallArgumentOrder>,
    method_calls: &'lowerer HashMap<Span, CheckedMethodCall>,
    struct_constructions: &'lowerer HashMap<Span, CheckedStructConstruction>,
    local_ids: HashMap<DefId, LocalId>,
    locals: Vec<Local>,
}

impl<'lowerer, 'program> BodyLowerer<'lowerer, 'program> {
    fn new(
        parent: &'lowerer mut Lowerer<'program>,
        function_ids: &'lowerer HashMap<FunctionKey, FunctionId>,
        expression_types: &'lowerer HashMap<Span, TypeId>,
        generic_calls: &'lowerer HashMap<Span, CheckedGenericCall>,
        call_argument_orders: &'lowerer HashMap<Span, CheckedCallArgumentOrder>,
        method_calls: &'lowerer HashMap<Span, CheckedMethodCall>,
        struct_constructions: &'lowerer HashMap<Span, CheckedStructConstruction>,
    ) -> Self {
        Self {
            parent,
            function_ids,
            expression_types,
            generic_calls,
            call_argument_orders,
            method_calls,
            struct_constructions,
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
                    .expression_types
                    .get(&decl.value.span())
                    .copied()
                    .or_else(|| self.parent.check.definition_types.get(&definition).copied())
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
            Expr::FieldAccess(base, field, _) => self.lower_field(base, field)?,
            Expr::Call(callee, args, _) | Expr::GenericCall(callee, _, args, _) => {
                self.lower_call(callee, args, span)?
            }
            Expr::ListConstruct(elements, _) => ExpressionKind::ListConstruct {
                elements: elements
                    .iter()
                    .map(|element| self.lower_expression(element))
                    .collect::<Option<Vec<_>>>()?,
            },
            Expr::MapConstruct(entries, _) => ExpressionKind::MapConstruct {
                entries: entries
                    .iter()
                    .map(|(key, value)| {
                        Some(MapEntry {
                            key: self.lower_expression(key)?,
                            value: self.lower_expression(value)?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?,
            },
            Expr::Pipeline(initial, steps, _) => {
                return self.lower_pipeline(initial, steps, ty, span);
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

    fn lower_call(
        &mut self,
        callee: &Expr,
        args: &[ast::CallArg],
        call_span: Span,
    ) -> Option<ExpressionKind> {
        if let Some(construction) = self.struct_constructions.get(&call_span) {
            let (fields, evaluation_order) =
                self.lower_arguments_in_parameter_order(args, call_span)?;
            return Some(ExpressionKind::StructConstruct {
                struct_type: construction.struct_type,
                fields,
                evaluation_order,
                validates_refinements: construction.validates_refinements,
            });
        }

        let function = self.resolve_user_call_target(callee, call_span)?;
        let (lowered_args, evaluation_order) =
            self.lower_arguments_in_parameter_order(args, call_span)?;
        Some(ExpressionKind::Call {
            function,
            args: lowered_args,
            evaluation_order,
        })
    }

    fn resolve_user_call_target(&mut self, callee: &Expr, call_span: Span) -> Option<FunctionId> {
        if let Some(method) = self.method_calls.get(&call_span) {
            let key = FunctionKey::Method {
                source_span: method.source_span,
            };
            let Some(function) = self.function_ids.get(&key).copied() else {
                self.parent.error(
                    call_span,
                    "method call target has no checked concrete HIR function",
                );
                return None;
            };
            return Some(function);
        }

        let Expr::Ident(ident) = callee else {
            self.parent.error(
                callee.span(),
                "only direct user-function calls are in the current HIR subset",
            );
            return None;
        };
        let Some(definition) = self.parent.resolve.resolutions.get(&ident.span).copied() else {
            self.parent
                .error(ident.span, "call target has no resolved definition");
            return None;
        };
        let key = if let Some(generic) = self.generic_calls.get(&call_span) {
            if generic.definition != definition {
                self.parent.error(
                    call_span,
                    "checked generic call target disagrees with name resolution",
                );
                return None;
            }
            FunctionKey::Definition {
                definition,
                concrete_args: generic.concrete_args.clone(),
            }
        } else {
            FunctionKey::Definition {
                definition,
                concrete_args: Vec::new(),
            }
        };
        let Some(function) = self.function_ids.get(&key).copied() else {
            self.parent.error(
                ident.span,
                "call target has no checked concrete HIR function",
            );
            return None;
        };
        Some(function)
    }

    fn lower_pipeline(
        &mut self,
        initial: &Expr,
        steps: &[ast::PipelineStep],
        pipeline_type: TypeId,
        pipeline_span: Span,
    ) -> Option<Expression> {
        if let Some(step) = steps.iter().find(|step| step.handle.is_some()) {
            self.parent.error(
                step.span,
                "handled pipeline steps require the staged HIR handle lowering slice",
            );
            return None;
        }

        let mut current = self.lower_expression(initial)?;
        for (index, step) in steps.iter().enumerate() {
            let output_type = if let Some(next_step) = steps.get(index + 1) {
                let Some(ty) = self.expression_types.get(&next_step.span).copied() else {
                    self.parent
                        .error(next_step.span, "pipeline step has no checked input type");
                    return None;
                };
                ty
            } else {
                pipeline_type
            };
            current = self.lower_pipeline_step(current, step, output_type)?;
        }
        current.ty = pipeline_type;
        current.span = pipeline_span;
        Some(current)
    }

    fn lower_pipeline_step(
        &mut self,
        piped: Expression,
        step: &ast::PipelineStep,
        output_type: TypeId,
    ) -> Option<Expression> {
        let (callee, extra_args, piped_as_view) = Self::pipeline_step_call_parts(step);
        let function = self.resolve_user_call_target(callee, step.span)?;
        let piped = if piped_as_view {
            Expression {
                ty: piped.ty,
                span: step.function.span(),
                kind: ExpressionKind::View(Box::new(piped)),
            }
        } else {
            piped
        };
        let (args, evaluation_order) =
            self.lower_pipeline_arguments(piped, extra_args, step.span)?;
        Some(Expression {
            kind: ExpressionKind::Call {
                function,
                args,
                evaluation_order,
            },
            ty: output_type,
            span: step.span,
        })
    }

    fn pipeline_step_call_parts(step: &ast::PipelineStep) -> (&Expr, &[ast::CallArg], bool) {
        let (function, piped_as_view) = match &step.function {
            Expr::View(inner, _) => (inner.as_ref(), true),
            _ => (&step.function, false),
        };
        match function {
            Expr::Call(callee, args, _) => (callee, args, piped_as_view),
            Expr::GenericCall(callee, _, args, _) => (callee, args, piped_as_view),
            _ => (function, &step.extra_args, piped_as_view),
        }
    }

    fn lower_pipeline_arguments(
        &mut self,
        piped: Expression,
        extra_args: &[ast::CallArg],
        step_span: Span,
    ) -> Option<(Vec<Expression>, Vec<usize>)> {
        let argument_count = extra_args.len() + 1;
        let source_indices = if let Some(order) = self.call_argument_orders.get(&step_span) {
            order.source_indices.clone()
        } else if extra_args.iter().all(|arg| arg.name.is_none()) {
            (0..argument_count).collect()
        } else {
            self.parent.error(
                step_span,
                "named pipeline arguments have no checked parameter order",
            );
            return None;
        };
        let mut seen = vec![false; argument_count];
        let invalid_permutation = source_indices.iter().any(|&index| {
            if index >= argument_count || seen[index] {
                true
            } else {
                seen[index] = true;
                false
            }
        });
        if source_indices.len() != argument_count || invalid_permutation {
            self.parent
                .error(step_span, "checked pipeline argument order is invalid");
            return None;
        }
        let evaluation_order = (0..argument_count)
            .map(|source_index| {
                source_indices
                    .iter()
                    .position(|&candidate| candidate == source_index)
                    .expect("validated pipeline order must be a permutation")
            })
            .collect();

        let mut piped = Some(piped);
        let args = source_indices
            .into_iter()
            .map(|source_index| {
                if source_index == 0 {
                    piped.take()
                } else {
                    self.lower_expression(&extra_args[source_index - 1].value)
                }
            })
            .collect::<Option<Vec<_>>>()?;
        Some((args, evaluation_order))
    }

    fn lower_arguments_in_parameter_order(
        &mut self,
        args: &[ast::CallArg],
        call_span: Span,
    ) -> Option<(Vec<Expression>, Vec<usize>)> {
        let source_indices = if let Some(order) = self.call_argument_orders.get(&call_span) {
            order.source_indices.clone()
        } else if args.iter().all(|arg| arg.name.is_none()) {
            (0..args.len()).collect()
        } else {
            self.parent.error(
                call_span,
                "named call arguments have no checked parameter order",
            );
            return None;
        };
        let mut seen = vec![false; args.len()];
        let invalid_permutation = source_indices.iter().any(|&index| {
            if index >= args.len() || seen[index] {
                true
            } else {
                seen[index] = true;
                false
            }
        });
        if source_indices.len() != args.len() || invalid_permutation {
            self.parent
                .error(call_span, "checked call argument order is invalid");
            return None;
        }
        let evaluation_order = (0..args.len())
            .map(|source_index| {
                source_indices
                    .iter()
                    .position(|&candidate| candidate == source_index)
                    .expect("validated call order must be a permutation")
            })
            .collect();
        let lowered = source_indices
            .into_iter()
            .map(|index| self.lower_expression(&args[index].value))
            .collect::<Option<Vec<_>>>()?;
        Some((lowered, evaluation_order))
    }

    fn lower_field(&mut self, base: &Expr, field: &ast::Ident) -> Option<ExpressionKind> {
        let base = self.lower_expression(base)?;
        let owner_type = match self.parent.check.interner.resolve(base.ty) {
            Type::Struct(_) => base.ty,
            Type::Secret(inner)
                if matches!(self.parent.check.interner.resolve(*inner), Type::Struct(_)) =>
            {
                *inner
            }
            _ => {
                self.parent.error(
                    field.span,
                    "only checked struct fields are in the current HIR lowering subset",
                );
                return None;
            }
        };
        let Type::Struct(struct_id) = *self.parent.check.interner.resolve(owner_type) else {
            unreachable!("owner type was checked as a struct")
        };
        let Some(field_index) = self
            .parent
            .check
            .interner
            .resolve_struct(struct_id)
            .fields
            .iter()
            .position(|(name, _)| name == &field.name)
        else {
            self.parent
                .error(field.span, "checked struct field has no canonical index");
            return None;
        };
        Some(ExpressionKind::Field {
            base: Box::new(base),
            owner_type,
            field: FieldId(field_index as u32),
        })
    }

    fn expression_type(&mut self, expression: &Expr) -> Option<TypeId> {
        if let Some(ty) = self.expression_types.get(&expression.span()).copied() {
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

    #[test]
    fn lowers_nested_generic_instantiations_and_calls() {
        let source = r#"namespace app
function inner[T](value: T) returns T:
    return value
function outer[T](value: T) returns T:
    return inner[T](value)
function main() returns int64:
    return outer[int64](42)
"#;
        let program = lower_source(source);
        assert_eq!(program.functions.len(), 3);

        let main = &program.functions[0];
        let outer = &program.functions[1];
        let inner = &program.functions[2];
        assert!(main.identity.type_arguments.is_empty());
        assert_eq!(outer.identity.type_arguments.len(), 1);
        assert_eq!(inner.identity.type_arguments.len(), 1);
        assert_eq!(outer.params[0].ty, outer.identity.type_arguments[0]);
        assert_eq!(inner.params[0].ty, inner.identity.type_arguments[0]);

        let StatementKind::Return(Some(Expression {
            kind: ExpressionKind::Call { function, .. },
            ..
        })) = &main.body.statements[0].kind
        else {
            panic!("expected main to call outer[int64]");
        };
        assert_eq!(*function, outer.id);

        let StatementKind::Return(Some(Expression {
            kind: ExpressionKind::Call { function, .. },
            ..
        })) = &outer.body.statements[0].kind
        else {
            panic!("expected outer[int64] to call inner[int64]");
        };
        assert_eq!(*function, inner.id);
    }

    #[test]
    fn keeps_inferred_instantiation_type_facts_separate() {
        let source = r#"namespace app
function identity[T](value: T) returns T:
    return value
function main() returns nothing:
    int64 number = identity(1)
    string text = identity("jett")
"#;
        let program = lower_source(source);
        assert_eq!(program.functions.len(), 3);
        let main = &program.functions[0];
        let integer_identity = &program.functions[1];
        let string_identity = &program.functions[2];

        assert_ne!(
            integer_identity.identity.type_arguments,
            string_identity.identity.type_arguments
        );
        assert_eq!(integer_identity.params[0].ty, integer_identity.return_type);
        assert_eq!(string_identity.params[0].ty, string_identity.return_type);

        let StatementKind::Let { value, .. } = &main.body.statements[0].kind else {
            panic!("expected first inferred generic call");
        };
        let ExpressionKind::Call { function, .. } = value.kind else {
            panic!("expected first inferred generic call target");
        };
        assert_eq!(function, integer_identity.id);

        let StatementKind::Let { value, .. } = &main.body.statements[1].kind else {
            panic!("expected second inferred generic call");
        };
        let ExpressionKind::Call { function, .. } = value.kind else {
            panic!("expected second inferred generic call target");
        };
        assert_eq!(function, string_identity.id);
    }

    #[test]
    fn lowers_recursive_generic_calls_to_the_reserved_identity() {
        let source = r#"namespace app
function repeat[T](value: T, remaining: int64) returns T:
    if remaining == 0:
        return value
    return repeat[T](value, remaining - 1)
function main() returns int64:
    return repeat[int64](7, 2)
"#;
        let program = lower_source(source);
        assert_eq!(program.functions.len(), 2);
        let repeat = &program.functions[1];
        let StatementKind::Return(Some(Expression {
            kind: ExpressionKind::Call { function, .. },
            ..
        })) = &repeat.body.statements[1].kind
        else {
            panic!("expected recursive concrete call");
        };
        assert_eq!(*function, repeat.id);
    }

    #[test]
    fn lowers_struct_construction_fields_and_named_calls_to_canonical_order() {
        let source = r#"namespace app
struct Point:
    x: int64
    y: int64
function difference(left: int64, right: int64) returns int64:
    return left - right
function main() returns int64:
    Point point = Point(y: 2, x: 7)
    return difference(right: point.y, left: point.x)
"#;
        let program = lower_source(source);
        let difference = &program.functions[0];
        let main = &program.functions[1];

        let StatementKind::Let { value, .. } = &main.body.statements[0].kind else {
            panic!("expected point construction");
        };
        let ExpressionKind::StructConstruct { fields, .. } = &value.kind else {
            panic!("expected canonical struct construction");
        };
        assert!(matches!(fields[0].kind, ExpressionKind::Int(7)));
        assert!(matches!(fields[1].kind, ExpressionKind::Int(2)));

        let StatementKind::Return(Some(Expression {
            kind:
                ExpressionKind::Call {
                    function,
                    args,
                    evaluation_order,
                },
            ..
        })) = &main.body.statements[1].kind
        else {
            panic!("expected named function call");
        };
        assert_eq!(*function, difference.id);
        assert_eq!(evaluation_order, &[1, 0]);
        let ExpressionKind::Field { field, .. } = args[0].kind else {
            panic!("expected left field argument");
        };
        assert_eq!(field.index(), 0);
        let ExpressionKind::Field { field, .. } = args[1].kind else {
            panic!("expected right field argument");
        };
        assert_eq!(field.index(), 1);
    }

    #[test]
    fn lowers_interface_calls_to_the_concrete_method_body() {
        let source = r#"namespace app
interface Scored:
    function score(view self: Scored, bonus: int64) returns int64
struct User:
    points: int64
implement Scored for User:
    function score(view self: User, bonus: int64) returns int64:
        return self.points + bonus
function main() returns int64:
    User user = User(points: 7)
    return Scored.score(bonus: 5, self: view user)
"#;
        let program = lower_source(source);
        assert_eq!(program.functions.len(), 2);
        let main = &program.functions[0];
        let method = &program.functions[1];
        assert_eq!(method.identity.declaration.kind, DeclarationKind::Method);
        assert_eq!(
            method.identity.declaration.name,
            "app.User as app.Scored.score"
        );

        let StatementKind::Return(Some(Expression {
            kind:
                ExpressionKind::Call {
                    function,
                    args,
                    evaluation_order,
                },
            ..
        })) = &main.body.statements[1].kind
        else {
            panic!("expected concrete method call");
        };
        assert_eq!(*function, method.id);
        assert_eq!(evaluation_order, &[1, 0]);
        assert!(matches!(args[0].kind, ExpressionKind::View(_)));
        assert!(matches!(args[1].kind, ExpressionKind::Int(5)));
    }

    #[test]
    fn keeps_named_argument_orders_inside_generic_instantiations() {
        let source = r#"namespace app
function inner[T](first: T, second: T) returns T:
    return first
function outer[T](first: T, second: T) returns T:
    return inner[T](second: second, first: first)
function main() returns int64:
    return outer[int64](second: 2, first: 7)
"#;
        let program = lower_source(source);
        let main = &program.functions[0];
        let outer = &program.functions[1];

        for function in [main, outer] {
            let StatementKind::Return(Some(Expression {
                kind:
                    ExpressionKind::Call {
                        args,
                        evaluation_order,
                        ..
                    },
                ..
            })) = &function.body.statements[0].kind
            else {
                panic!("expected named generic call");
            };
            assert_eq!(evaluation_order, &[1, 0]);
            assert!(matches!(
                args[0].kind,
                ExpressionKind::Int(7) | ExpressionKind::Local(_)
            ));
        }
    }

    #[test]
    fn lowers_list_and_map_construction_in_lexical_order() {
        let source = r#"namespace app
function main() returns map[string, list[int64]]:
    return map("odds": list(1, 3), "evens": list(2, 4))
"#;
        let program = lower_source(source);
        let main = &program.functions[0];
        let StatementKind::Return(Some(Expression {
            kind: ExpressionKind::MapConstruct { entries },
            ..
        })) = &main.body.statements[0].kind
        else {
            panic!("expected map construction");
        };
        assert_eq!(entries.len(), 2);
        assert!(
            matches!(entries[0].key.kind, ExpressionKind::String(ref value) if value == "odds")
        );
        let ExpressionKind::ListConstruct { elements } = &entries[0].value.kind else {
            panic!("expected nested list construction");
        };
        assert!(matches!(elements[0].kind, ExpressionKind::Int(1)));
        assert!(matches!(elements[1].kind, ExpressionKind::Int(3)));
    }

    #[test]
    fn desugars_pipeline_steps_to_checked_hir_calls() {
        let source = r#"namespace app
struct Score:
    value: int64
    function add(view self: Score, bonus: int64) returns int64:
        return self.value + bonus
function choose[T](first: T, second: T, third: T) returns T:
    return first
function increment(value: int64) returns int64:
    return value + 1
function main() returns int64:
    Score score = Score(value: 7)
    int64 chosen = 1 into choose[int64](third: 3, second: 2)
    return score into view Score.add(bonus: chosen) into increment
"#;
        let program = lower_source(source);
        let main = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "main")
            .expect("main function");
        let choose = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "choose")
            .expect("concrete choose function");
        let method = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "Score.add")
            .expect("score method");
        let increment = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "increment")
            .expect("increment function");

        let StatementKind::Let { value, .. } = &main.body.statements[1].kind else {
            panic!("expected chosen local");
        };
        let ExpressionKind::Call {
            function,
            args,
            evaluation_order,
        } = &value.kind
        else {
            panic!("expected lowered generic pipeline step");
        };
        assert_eq!(*function, choose.id);
        assert_eq!(evaluation_order, &[0, 2, 1]);
        assert!(matches!(args[0].kind, ExpressionKind::Int(1)));
        assert!(matches!(args[1].kind, ExpressionKind::Int(2)));
        assert!(matches!(args[2].kind, ExpressionKind::Int(3)));

        let StatementKind::Return(Some(Expression {
            kind:
                ExpressionKind::Call {
                    function,
                    args,
                    evaluation_order,
                },
            ..
        })) = &main.body.statements[2].kind
        else {
            panic!("expected final pipeline call");
        };
        assert_eq!(*function, increment.id);
        assert_eq!(evaluation_order, &[0]);
        let ExpressionKind::Call {
            function,
            args,
            evaluation_order,
        } = &args[0].kind
        else {
            panic!("expected concrete method pipeline step");
        };
        assert_eq!(*function, method.id);
        assert_eq!(evaluation_order, &[0, 1]);
        assert!(matches!(args[0].kind, ExpressionKind::View(_)));
    }

    #[test]
    fn stages_handled_pipeline_steps_explicitly() {
        let source = r#"namespace app
function parse(value: string) returns result[int64, string]:
    return fail("invalid")
function main() returns int64:
    return "x" into parse handle error:
        default 0
"#;
        let file = FileId::new(0);
        let parsed = jett_parser::parse(source, file);
        assert!(
            parsed.errors.is_empty(),
            "parse errors: {:?}",
            parsed.errors
        );
        let resolved = jett_resolve::resolve(&parsed.module);
        let checked = jett_typecheck::check(&parsed.module, &resolved);
        assert!(
            checked
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != Severity::Error),
            "type errors: {:?}",
            checked.diagnostics
        );
        let origins = HashMap::from([(file, SourceOrigin::Project)]);
        let errors = lower(&parsed.module, &resolved, &checked, &origins)
            .expect_err("handled pipeline must remain staged");
        assert!(errors.iter().any(|error| {
            error
                .message
                .contains("handled pipeline steps require the staged HIR handle")
        }));
    }
}
