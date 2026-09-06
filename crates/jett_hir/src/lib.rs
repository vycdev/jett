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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VariantId(u32);

impl VariantId {
    pub fn index(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StateId(u32);

impl StateId {
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
    /// Yield a fallback value to the nearest enclosing handle expression.
    HandleDefault(Expression),
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
    For {
        key: LocalId,
        value: Option<LocalId>,
        by_view: bool,
        iterable: Expression,
        body: Block,
    },
    Match {
        scrutinee: Expression,
        arms: Vec<MatchArm>,
    },
    Break,
    Continue,
    Assert {
        condition: Expression,
        message: Option<Expression>,
    },
    Trace(LocalId),
    Breakpoint(Option<Expression>),
    Respond(Expression),
    Scope(Block),
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
    Intrinsic {
        canonical_name: String,
        args: Vec<Expression>,
        evaluation_order: Vec<usize>,
    },
    IndirectCall {
        callee: Box<Expression>,
        args: Vec<Expression>,
        evaluation_order: Vec<usize>,
    },
    StructConstruct {
        struct_type: TypeId,
        fields: Vec<Expression>,
        /// Field indexes in lexical source evaluation order.
        evaluation_order: Vec<usize>,
        validates_refinements: bool,
    },
    BitfieldConstruct {
        bitfield_type: TypeId,
        fields: Vec<Expression>,
        evaluation_order: Vec<usize>,
        validates_widths: bool,
    },
    MachineConstruct {
        state_type: TypeId,
        state: StateId,
        payloads: Vec<Expression>,
    },
    MachineTransition {
        source: Box<Expression>,
        state_type: TypeId,
        target: StateId,
        payloads: Vec<Expression>,
    },
    ListConstruct {
        elements: Vec<Expression>,
    },
    MapConstruct {
        entries: Vec<MapEntry>,
    },
    ResultOk(Box<Expression>),
    ResultFail(Box<Expression>),
    OptionalSome(Box<Expression>),
    OptionalNone,
    Handle {
        target: Box<Expression>,
        kind: HandleKind,
        error_local: Option<LocalId>,
        failure: Block,
    },
    EnumConstruct {
        enum_type: TypeId,
        variant: VariantId,
        payloads: Vec<Expression>,
    },
    StringInterpolation(Vec<StringSegment>),
    Comptime(Box<Expression>),
    Declassify(Box<Expression>),
    Coarsen(Box<Expression>),
    StateIs {
        value: Box<Expression>,
        state: StateId,
    },
    Run(Box<Expression>),
    Join(Box<Expression>),
    Cancel(Box<Expression>),
    InlineFunction {
        params: Vec<LocalId>,
        body: Block,
    },
    ActorSpawn {
        actor_type: String,
        args: Vec<Expression>,
        evaluation_order: Vec<usize>,
    },
    ActorMessage {
        actor: Box<Expression>,
        message: String,
        args: Vec<Expression>,
        /// Parameter indexes in lexical source evaluation order.
        evaluation_order: Vec<usize>,
        kind: ActorMessageKind,
    },
    Field {
        base: Box<Expression>,
        owner_type: TypeId,
        field: FieldId,
    },
    View(Box<Expression>),
    Clone(Box<Expression>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleKind {
    Result,
    Optional,
    Refinement { refined_type: TypeId },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub variant: Option<VariantId>,
    pub bindings: Vec<LocalId>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StringSegment {
    Text(String),
    Value(Expression),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorMessageKind {
    Send,
    Ask,
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
pub struct ValidationError {
    pub span: Span,
    pub message: String,
}

/// Validate backend-facing HIR invariants after lowering and before MIR.
pub fn validate(program: &Program) -> Result<(), Vec<ValidationError>> {
    let mut validator = Validator {
        program,
        errors: Vec::new(),
        local_count: 0,
        loop_depth: 0,
        handle_depth: 0,
    };
    for (index, function) in program.functions.iter().enumerate() {
        if function.id.index() as usize != index {
            validator.error(function.span, "function IDs must be dense and ordered");
        }
        validator.local_count = function.locals.len();
        for (local_index, local) in function.locals.iter().enumerate() {
            if local.id.index() as usize != local_index {
                validator.error(local.span, "local IDs must be dense and ordered");
            }
        }
        for param in &function.params {
            validator.check_local(param.local, param.span);
        }
        validator.block(&function.body);
    }
    if validator.errors.is_empty() {
        Ok(())
    } else {
        Err(validator.errors)
    }
}

struct Validator<'a> {
    program: &'a Program,
    errors: Vec<ValidationError>,
    local_count: usize,
    loop_depth: usize,
    handle_depth: usize,
}

impl Validator<'_> {
    fn error(&mut self, span: Span, message: impl Into<String>) {
        self.errors.push(ValidationError {
            span,
            message: message.into(),
        });
    }

    fn check_local(&mut self, local: LocalId, span: Span) {
        if local.index() as usize >= self.local_count {
            self.error(span, "HIR references a local outside its function");
        }
    }

    fn check_evaluation_order(&mut self, order: &[usize], argument_count: usize, span: Span) {
        let mut seen = vec![false; argument_count];
        let valid = order.len() == argument_count
            && order
                .iter()
                .all(|&index| index < argument_count && !std::mem::replace(&mut seen[index], true));
        if !valid {
            self.error(
                span,
                "argument evaluation order must be a permutation of the argument indexes",
            );
        }
    }

    fn block(&mut self, block: &Block) {
        for statement in &block.statements {
            self.statement(statement);
        }
    }

    fn statement(&mut self, statement: &Statement) {
        match &statement.kind {
            StatementKind::Let { local, value } => {
                self.check_local(*local, statement.span);
                self.expression(value);
            }
            StatementKind::Assign { target, value } => {
                self.expression(target);
                self.expression(value);
            }
            StatementKind::Return(value) => {
                if let Some(value) = value {
                    self.expression(value);
                }
            }
            StatementKind::HandleDefault(value) => {
                if self.handle_depth == 0 {
                    self.error(statement.span, "handle default is outside a failure block");
                }
                self.expression(value);
            }
            StatementKind::Expression(value) => self.expression(value),
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                self.expression(condition);
                self.block(then_block);
                if let Some(block) = else_block {
                    self.block(block);
                }
            }
            StatementKind::While { condition, body } => {
                self.expression(condition);
                self.loop_depth += 1;
                self.block(body);
                self.loop_depth -= 1;
            }
            StatementKind::For {
                key,
                value,
                iterable,
                body,
                ..
            } => {
                self.check_local(*key, statement.span);
                if let Some(value) = value {
                    self.check_local(*value, statement.span);
                }
                self.expression(iterable);
                self.loop_depth += 1;
                self.block(body);
                self.loop_depth -= 1;
            }
            StatementKind::Match { scrutinee, arms } => {
                self.expression(scrutinee);
                let mut variants = std::collections::HashSet::new();
                let mut catch_all = false;
                for arm in arms {
                    if let Some(variant) = arm.variant {
                        if !variants.insert(variant) {
                            self.error(arm.span, "match contains a duplicate variant arm");
                        }
                    } else if std::mem::replace(&mut catch_all, true) {
                        self.error(arm.span, "match contains multiple catch-all arms");
                    }
                    for binding in &arm.bindings {
                        self.check_local(*binding, arm.span);
                    }
                    self.block(&arm.body);
                }
            }
            StatementKind::Break | StatementKind::Continue if self.loop_depth == 0 => {
                self.error(statement.span, "loop control is outside a loop");
            }
            StatementKind::Break | StatementKind::Continue => {}
            StatementKind::Assert { condition, message } => {
                self.expression(condition);
                if let Some(message) = message {
                    self.expression(message);
                }
            }
            StatementKind::Trace(local) => self.check_local(*local, statement.span),
            StatementKind::Breakpoint(condition) => {
                if let Some(condition) = condition {
                    self.expression(condition);
                }
            }
            StatementKind::Respond(value) => self.expression(value),
            StatementKind::Scope(block) => self.block(block),
        }
    }

    fn expression(&mut self, expression: &Expression) {
        match &expression.kind {
            ExpressionKind::Local(local) => self.check_local(*local, expression.span),
            ExpressionKind::Binary { left, right, .. } => {
                self.expression(left);
                self.expression(right);
            }
            ExpressionKind::Unary { value, .. }
            | ExpressionKind::ResultOk(value)
            | ExpressionKind::ResultFail(value)
            | ExpressionKind::OptionalSome(value)
            | ExpressionKind::View(value)
            | ExpressionKind::Clone(value)
            | ExpressionKind::Comptime(value)
            | ExpressionKind::Declassify(value)
            | ExpressionKind::Coarsen(value)
            | ExpressionKind::Run(value)
            | ExpressionKind::Join(value)
            | ExpressionKind::Cancel(value) => self.expression(value),
            ExpressionKind::Call {
                function,
                args,
                evaluation_order,
            } => {
                if function.index() as usize >= self.program.functions.len() {
                    self.error(expression.span, "call references an unknown HIR function");
                }
                self.check_evaluation_order(evaluation_order, args.len(), expression.span);
                for argument in args {
                    self.expression(argument);
                }
            }
            ExpressionKind::Intrinsic {
                args,
                evaluation_order,
                ..
            } => {
                self.check_evaluation_order(evaluation_order, args.len(), expression.span);
                for argument in args {
                    self.expression(argument);
                }
            }
            ExpressionKind::IndirectCall {
                callee,
                args,
                evaluation_order,
            } => {
                self.expression(callee);
                self.check_evaluation_order(evaluation_order, args.len(), expression.span);
                for argument in args {
                    self.expression(argument);
                }
            }
            ExpressionKind::StructConstruct {
                fields,
                evaluation_order,
                ..
            }
            | ExpressionKind::BitfieldConstruct {
                fields,
                evaluation_order,
                ..
            } => {
                self.check_evaluation_order(evaluation_order, fields.len(), expression.span);
                for field in fields {
                    self.expression(field);
                }
            }
            ExpressionKind::MachineConstruct { payloads, .. } => {
                for payload in payloads {
                    self.expression(payload);
                }
            }
            ExpressionKind::MachineTransition {
                source, payloads, ..
            } => {
                self.expression(source);
                for payload in payloads {
                    self.expression(payload);
                }
            }
            ExpressionKind::ListConstruct { elements } => {
                for element in elements {
                    self.expression(element);
                }
            }
            ExpressionKind::MapConstruct { entries } => {
                for entry in entries {
                    self.expression(&entry.key);
                    self.expression(&entry.value);
                }
            }
            ExpressionKind::Handle {
                target,
                error_local,
                failure,
                ..
            } => {
                self.expression(target);
                if let Some(local) = error_local {
                    self.check_local(*local, expression.span);
                }
                self.handle_depth += 1;
                self.block(failure);
                self.handle_depth -= 1;
            }
            ExpressionKind::EnumConstruct { payloads, .. } => {
                for payload in payloads {
                    self.expression(payload);
                }
            }
            ExpressionKind::StringInterpolation(parts) => {
                for part in parts {
                    if let StringSegment::Value(value) = part {
                        self.expression(value);
                    }
                }
            }
            ExpressionKind::StateIs { value, .. } => self.expression(value),
            ExpressionKind::InlineFunction { params, body } => {
                for param in params {
                    self.check_local(*param, expression.span);
                }
                self.block(body);
            }
            ExpressionKind::ActorSpawn {
                args,
                evaluation_order,
                ..
            } => {
                self.check_evaluation_order(evaluation_order, args.len(), expression.span);
                for argument in args {
                    self.expression(argument);
                }
            }
            ExpressionKind::ActorMessage {
                actor,
                args,
                evaluation_order,
                ..
            } => {
                self.expression(actor);
                self.check_evaluation_order(evaluation_order, args.len(), expression.span);
                for argument in args {
                    self.expression(argument);
                }
            }
            ExpressionKind::Field { base, .. } => self.expression(base),
            ExpressionKind::Int(_)
            | ExpressionKind::Float(_)
            | ExpressionKind::String(_)
            | ExpressionKind::Bool(_)
            | ExpressionKind::Nothing
            | ExpressionKind::OptionalNone => {}
        }
    }
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
            let program = Program { functions };
            validate(&program).map_err(|errors| {
                errors
                    .into_iter()
                    .map(|error| LowerError {
                        span: error.span,
                        message: error.message,
                    })
                    .collect::<Vec<_>>()
            })?;
            Ok(program)
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
            pipeline_step_call_types,
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
                instantiation.pipeline_step_call_types.clone(),
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
                self.check.pipeline_step_call_types.clone(),
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
                self.check.pipeline_step_call_types.clone(),
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
            &pipeline_step_call_types,
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
    pipeline_step_call_types: &'lowerer HashMap<Span, TypeId>,
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
        pipeline_step_call_types: &'lowerer HashMap<Span, TypeId>,
    ) -> Self {
        Self {
            parent,
            function_ids,
            expression_types,
            generic_calls,
            call_argument_orders,
            method_calls,
            struct_constructions,
            pipeline_step_call_types,
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
            Stmt::Expr(expr) => {
                let kind = if let Expr::Default(value, _) = &expr.expr {
                    StatementKind::HandleDefault(self.lower_expression(value)?)
                } else {
                    StatementKind::Expression(self.lower_expression(&expr.expr)?)
                };
                (kind, expr.span)
            }
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
            Stmt::For(loop_stmt) => {
                let key = self.allocate_declared_local(&loop_stmt.variable, false)?;
                let value = match &loop_stmt.value_variable {
                    Some(binding) => Some(self.allocate_declared_local(binding, false)?),
                    None => None,
                };
                (
                    StatementKind::For {
                        key,
                        value,
                        by_view: loop_stmt.view,
                        iterable: self.lower_expression(&loop_stmt.iterable)?,
                        body: self.lower_block(&loop_stmt.body),
                    },
                    loop_stmt.span,
                )
            }
            Stmt::Match(match_stmt) => (self.lower_match(match_stmt)?, match_stmt.span),
            Stmt::Break(span) => (StatementKind::Break, *span),
            Stmt::Continue(span) => (StatementKind::Continue, *span),
            Stmt::Assert(assertion) => (
                StatementKind::Assert {
                    condition: self.lower_expression(&assertion.condition)?,
                    message: match &assertion.message {
                        Some(value) => Some(self.lower_expression(value)?),
                        None => None,
                    },
                },
                assertion.span,
            ),
            Stmt::Trace(trace) => {
                let Some(definition) = self.parent.resolve.resolutions.get(&trace.name.span) else {
                    self.parent.error(trace.span, "trace target is unresolved");
                    return None;
                };
                let Some(local) = self.local_ids.get(definition).copied() else {
                    self.parent.error(trace.span, "trace target is not a local");
                    return None;
                };
                (StatementKind::Trace(local), trace.span)
            }
            Stmt::Breakpoint(point) => (
                StatementKind::Breakpoint(match &point.condition {
                    Some(value) => Some(self.lower_expression(value)?),
                    None => None,
                }),
                point.span,
            ),
            Stmt::Respond(response) => (
                StatementKind::Respond(self.lower_expression(&response.value)?),
                response.span,
            ),
            Stmt::ComptimeTypeBind(binding) => (
                StatementKind::Scope(self.lower_block(&binding.body)),
                binding.span,
            ),
            Stmt::Use(_) => return None,
        };
        Some(Statement { kind, span })
    }

    fn allocate_declared_local(&mut self, name: &ast::Ident, mutable: bool) -> Option<LocalId> {
        let Some(definition) = self.parent.definition_at(name.span, DefKind::Variable) else {
            self.parent
                .error(name.span, "binding has no resolved definition");
            return None;
        };
        let Some(ty) = self.parent.check.definition_types.get(&definition).copied() else {
            self.parent.error(name.span, "binding has no checked type");
            return None;
        };
        Some(self.allocate_local(definition, &name.name, ty, mutable, name.span))
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
            Expr::FieldAccess(base, field, _) => {
                if self.enum_variant_index(ty, field).is_some() {
                    self.lower_enum_construct(ty, field, &[], span)?
                } else {
                    self.lower_field(base, field)?
                }
            }
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
            Expr::Ok(value, _) => ExpressionKind::ResultOk(Box::new(self.lower_expression(value)?)),
            Expr::Fail(error, _) => {
                ExpressionKind::ResultFail(Box::new(self.lower_expression(error)?))
            }
            Expr::Some(value, _) => {
                ExpressionKind::OptionalSome(Box::new(self.lower_expression(value)?))
            }
            Expr::None(_) => ExpressionKind::OptionalNone,
            Expr::StringInterpolation(parts, _) => ExpressionKind::StringInterpolation(
                parts
                    .iter()
                    .map(|part| match part {
                        ast::StringPart::Literal(text) => Some(StringSegment::Text(text.clone())),
                        ast::StringPart::Expr(value) => {
                            Some(StringSegment::Value(self.lower_expression(value)?))
                        }
                    })
                    .collect::<Option<Vec<_>>>()?,
            ),
            Expr::EnumVariant(_, variant, _) => {
                self.lower_enum_construct(ty, variant, &[], span)?
            }
            Expr::Handle(target, error_name, failure, _) => {
                let target = self.lower_expression(target)?;
                return self.lower_handle(target, error_name.as_ref(), failure, ty, span);
            }
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
            Expr::Comptime(value, _) => {
                ExpressionKind::Comptime(Box::new(self.lower_expression(value)?))
            }
            Expr::Declassify(value, _) => {
                ExpressionKind::Declassify(Box::new(self.lower_expression(value)?))
            }
            Expr::Coarsen(value, _) => {
                ExpressionKind::Coarsen(Box::new(self.lower_expression(value)?))
            }
            Expr::At(value, state, _) => {
                let value = self.lower_expression(value)?;
                let machine = match self.parent.check.interner.resolve(value.ty) {
                    Type::Machine(machine) | Type::MachineState { machine, .. } => *machine,
                    _ => {
                        self.parent
                            .error(span, "state check target is not a machine");
                        return None;
                    }
                };
                let Some(state_id) = self
                    .parent
                    .check
                    .interner
                    .resolve_machine(machine)
                    .state_id(&state.name)
                else {
                    self.parent
                        .error(state.span, "checked machine state is missing");
                    return None;
                };
                ExpressionKind::StateIs {
                    value: Box::new(value),
                    state: StateId(state_id.index()),
                }
            }
            Expr::Run(value, _) => ExpressionKind::Run(Box::new(self.lower_expression(value)?)),
            Expr::Join(value, _) => ExpressionKind::Join(Box::new(self.lower_expression(value)?)),
            Expr::Cancel(value, _) => {
                ExpressionKind::Cancel(Box::new(self.lower_expression(value)?))
            }
            Expr::InlineFn(params, _, body, _) => {
                let mut lowered_params = Vec::with_capacity(params.len());
                for param in params {
                    let Some(definition) =
                        self.parent.definition_at(param.name.span, DefKind::Param)
                    else {
                        self.parent
                            .error(param.name.span, "closure parameter is unresolved");
                        return None;
                    };
                    let Some(param_type) =
                        self.parent.check.definition_types.get(&definition).copied()
                    else {
                        self.parent
                            .error(param.name.span, "closure parameter has no checked type");
                        return None;
                    };
                    lowered_params.push(self.allocate_local(
                        definition,
                        &param.name.name,
                        param_type,
                        param.mutable,
                        param.span,
                    ));
                }
                ExpressionKind::InlineFunction {
                    params: lowered_params,
                    body: self.lower_block(body),
                }
            }
            Expr::Spawn(inner, _) => self.lower_actor_spawn(inner)?,
            Expr::Send(inner, _) => self.lower_actor_message(inner, ActorMessageKind::Send)?,
            Expr::Ask(inner, _) => self.lower_actor_message(inner, ActorMessageKind::Ask)?,
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

    fn lower_actor_spawn(&mut self, inner: &Expr) -> Option<ExpressionKind> {
        let (callee, args, call_span) = match inner {
            Expr::Call(callee, args, span) => (callee.as_ref(), args.as_slice(), *span),
            _ => (inner, &[][..], inner.span()),
        };
        let actor_type = self.canonical_call_name(callee)?;
        let (args, evaluation_order) = self.lower_arguments_in_parameter_order(args, call_span)?;
        Some(ExpressionKind::ActorSpawn {
            actor_type,
            args,
            evaluation_order,
        })
    }

    fn lower_actor_message(
        &mut self,
        inner: &Expr,
        kind: ActorMessageKind,
    ) -> Option<ExpressionKind> {
        let (callee, args, call_span) = match inner {
            Expr::Call(callee, args, span) => (callee.as_ref(), args.as_slice(), *span),
            _ => (inner, &[][..], inner.span()),
        };
        let Expr::FieldAccess(actor, message, _) = callee else {
            self.parent
                .error(callee.span(), "actor message target has no message member");
            return None;
        };
        let actor = Box::new(self.lower_expression(actor)?);
        let (args, evaluation_order) = self.lower_arguments_in_parameter_order(args, call_span)?;
        Some(ExpressionKind::ActorMessage {
            actor,
            message: message.name.clone(),
            args,
            evaluation_order,
            kind,
        })
    }

    fn lower_handle(
        &mut self,
        target: Expression,
        error_name: Option<&ast::Ident>,
        failure: &ast::Block,
        output_type: TypeId,
        span: Span,
    ) -> Option<Expression> {
        let kind = match self.parent.check.interner.resolve(target.ty) {
            Type::Result(_, _) => HandleKind::Result,
            Type::Optional(_) => HandleKind::Optional,
            _ if matches!(
                self.parent.check.interner.resolve(output_type),
                Type::Refinement { .. }
            ) =>
            {
                HandleKind::Refinement {
                    refined_type: output_type,
                }
            }
            _ => {
                self.parent.error(
                    span,
                    "checked handle target is neither result, optional, nor refinement",
                );
                return None;
            }
        };
        let error_local = if let Some(name) = error_name {
            let Some(definition) = self.parent.definition_at(name.span, DefKind::Variable) else {
                self.parent
                    .error(name.span, "handle error binding has no resolved definition");
                return None;
            };
            let Some(ty) = self.parent.check.definition_types.get(&definition).copied() else {
                self.parent
                    .error(name.span, "handle error binding has no checked type");
                return None;
            };
            Some(self.allocate_local(definition, &name.name, ty, false, name.span))
        } else {
            None
        };
        let failure = self.lower_block(failure);
        Some(Expression {
            kind: ExpressionKind::Handle {
                target: Box::new(target),
                kind,
                error_local,
                failure,
            },
            ty: output_type,
            span,
        })
    }

    fn lower_call(
        &mut self,
        callee: &Expr,
        args: &[ast::CallArg],
        call_span: Span,
    ) -> Option<ExpressionKind> {
        let enum_variant = match callee {
            Expr::EnumVariant(_, variant, _) => Some(variant),
            Expr::FieldAccess(_, field, _)
                if self
                    .expression_types
                    .get(&call_span)
                    .is_some_and(|ty| self.enum_variant_index(*ty, field).is_some()) =>
            {
                Some(field)
            }
            _ => None,
        };
        if let Some(variant) = enum_variant {
            let ty = self.expression_types.get(&call_span).copied().or_else(|| {
                self.parent
                    .error(call_span, "enum construction has no checked type");
                None
            })?;
            return self.lower_enum_construct(ty, variant, args, call_span);
        }
        if let Expr::Ident(name) = callee
            && self.resolved_kind(name) == Some(DefKind::Bitfield)
        {
            return self.lower_bitfield_construct(args, call_span);
        }
        if let Expr::Ident(name) = callee
            && self.resolved_kind(name) == Some(DefKind::Machine)
        {
            return self.lower_machine_construct(args, call_span);
        }
        if let Expr::FieldAccess(base, member, _) = callee
            && member.name == "transition"
            && matches!(base.as_ref(), Expr::Ident(name) if self.resolved_kind(name) == Some(DefKind::Machine))
        {
            return self.lower_machine_transition(args, call_span);
        }
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

        let (lowered_args, evaluation_order) =
            self.lower_arguments_in_parameter_order(args, call_span)?;
        if matches!(
            callee,
            Expr::Ident(ident)
                if matches!(self.resolved_kind(ident), Some(DefKind::Variable | DefKind::Param))
        ) {
            return Some(ExpressionKind::IndirectCall {
                callee: Box::new(self.lower_expression(callee)?),
                args: lowered_args,
                evaluation_order,
            });
        }
        if self.is_source_call(callee, call_span) {
            Some(ExpressionKind::Call {
                function: self.resolve_user_call_target(callee, call_span)?,
                args: lowered_args,
                evaluation_order,
            })
        } else {
            Some(ExpressionKind::Intrinsic {
                canonical_name: self.canonical_call_name(callee)?,
                args: lowered_args,
                evaluation_order,
            })
        }
    }

    fn lower_enum_construct(
        &mut self,
        enum_type: TypeId,
        variant: &ast::Ident,
        args: &[ast::CallArg],
        span: Span,
    ) -> Option<ExpressionKind> {
        let Some(index) = self.enum_variant_index(enum_type, variant) else {
            self.parent
                .error(span, "checked enum construction has no matching variant");
            return None;
        };
        let payloads = args
            .iter()
            .map(|arg| self.lower_expression(&arg.value))
            .collect::<Option<Vec<_>>>()?;
        Some(ExpressionKind::EnumConstruct {
            enum_type,
            variant: VariantId(index as u32),
            payloads,
        })
    }

    fn enum_variant_index(&self, enum_type: TypeId, variant: &ast::Ident) -> Option<usize> {
        let Type::Enum(enum_id) = *self.parent.check.interner.resolve(enum_type) else {
            return None;
        };
        self.parent
            .check
            .interner
            .resolve_enum(enum_id)
            .variants
            .iter()
            .position(|candidate| candidate.name == variant.name)
    }

    fn resolved_kind(&self, ident: &ast::Ident) -> Option<DefKind> {
        self.parent
            .resolve
            .resolutions
            .get(&ident.span)
            .map(|definition| self.parent.resolve.scope_table.def(*definition).kind)
    }

    fn is_source_call(&self, callee: &Expr, span: Span) -> bool {
        if self.method_calls.contains_key(&span) {
            return true;
        }
        let Expr::Ident(ident) = callee else {
            return false;
        };
        self.resolved_kind(ident) == Some(DefKind::Function)
    }

    fn canonical_call_name(&mut self, callee: &Expr) -> Option<String> {
        fn dotted(expression: &Expr) -> Option<String> {
            match expression {
                Expr::Ident(ident) => Some(ident.name.clone()),
                Expr::FieldAccess(base, field, _) => {
                    Some(format!("{}.{}", dotted(base)?, field.name))
                }
                _ => None,
            }
        }
        dotted(callee).or_else(|| {
            self.parent.error(
                callee.span(),
                "checked intrinsic has no canonical call name",
            );
            None
        })
    }

    fn lower_bitfield_construct(
        &mut self,
        args: &[ast::CallArg],
        span: Span,
    ) -> Option<ExpressionKind> {
        let checked_type = self.expression_types.get(&span).copied()?;
        let (bitfield_type, validates_widths) =
            match self.parent.check.interner.resolve(checked_type) {
                Type::Bitfield(_) => (checked_type, false),
                Type::Result(ok, _)
                    if matches!(self.parent.check.interner.resolve(*ok), Type::Bitfield(_)) =>
                {
                    (*ok, true)
                }
                _ => {
                    self.parent
                        .error(span, "checked bitfield construction has an invalid type");
                    return None;
                }
            };
        let Type::Bitfield(bitfield_id) = *self.parent.check.interner.resolve(bitfield_type) else {
            unreachable!()
        };
        let definition = self.parent.check.interner.resolve_bitfield(bitfield_id);
        let mut source_indices = vec![usize::MAX; definition.fields.len()];
        for (source_index, arg) in args.iter().enumerate() {
            let field_index = if let Some(name) = &arg.name {
                definition
                    .fields
                    .iter()
                    .position(|field| field.name == name.name)?
            } else {
                source_indices
                    .iter()
                    .position(|index| *index == usize::MAX)?
            };
            source_indices[field_index] = source_index;
        }
        let evaluation_order = (0..args.len())
            .map(|source| {
                source_indices
                    .iter()
                    .position(|candidate| *candidate == source)
                    .unwrap()
            })
            .collect();
        let fields = source_indices
            .into_iter()
            .map(|index| self.lower_expression(&args[index].value))
            .collect::<Option<Vec<_>>>()?;
        Some(ExpressionKind::BitfieldConstruct {
            bitfield_type,
            fields,
            evaluation_order,
            validates_widths,
        })
    }

    fn lower_machine_construct(
        &mut self,
        args: &[ast::CallArg],
        span: Span,
    ) -> Option<ExpressionKind> {
        let state_type = self.expression_types.get(&span).copied()?;
        let Type::MachineState { state, .. } = self.parent.check.interner.resolve(state_type)
        else {
            self.parent
                .error(span, "machine construction lacks a checked state type");
            return None;
        };
        let payloads = args[1..]
            .iter()
            .map(|arg| self.lower_expression(&arg.value))
            .collect::<Option<Vec<_>>>()?;
        Some(ExpressionKind::MachineConstruct {
            state_type,
            state: StateId(state.index()),
            payloads,
        })
    }

    fn lower_machine_transition(
        &mut self,
        args: &[ast::CallArg],
        span: Span,
    ) -> Option<ExpressionKind> {
        let state_type = self.expression_types.get(&span).copied()?;
        let Type::MachineState { state, .. } = self.parent.check.interner.resolve(state_type)
        else {
            self.parent
                .error(span, "machine transition lacks a checked target state");
            return None;
        };
        let source = Box::new(self.lower_expression(&args[0].value)?);
        let payloads = args[2..]
            .iter()
            .map(|arg| self.lower_expression(&arg.value))
            .collect::<Option<Vec<_>>>()?;
        Some(ExpressionKind::MachineTransition {
            source,
            state_type,
            target: StateId(state.index()),
            payloads,
        })
    }

    fn lower_match(&mut self, match_stmt: &ast::MatchStmt) -> Option<StatementKind> {
        let scrutinee = self.lower_expression(&match_stmt.expr)?;
        let Type::Enum(enum_id) = *self.parent.check.interner.resolve(scrutinee.ty) else {
            self.parent.error(
                match_stmt.expr.span(),
                "checked match scrutinee is not an enum",
            );
            return None;
        };
        let enum_definition = self.parent.check.interner.resolve_enum(enum_id).clone();
        let mut arms = Vec::with_capacity(match_stmt.arms.len());
        for arm in &match_stmt.arms {
            let (variant, source_bindings, field_types) = match &arm.pattern {
                ast::Pattern::Ident(name) => {
                    let Some(index) = enum_definition
                        .variants
                        .iter()
                        .position(|candidate| candidate.name == name.name)
                    else {
                        self.parent
                            .error(name.span, "checked match variant is missing");
                        return None;
                    };
                    (Some(VariantId(index as u32)), &[][..], &[][..])
                }
                ast::Pattern::Variant(name, bindings) => {
                    let Some(index) = enum_definition
                        .variants
                        .iter()
                        .position(|candidate| candidate.name == name.name)
                    else {
                        self.parent
                            .error(name.span, "checked match variant is missing");
                        return None;
                    };
                    let fields = &enum_definition.variants[index].fields;
                    (
                        Some(VariantId(index as u32)),
                        bindings.as_slice(),
                        fields.as_slice(),
                    )
                }
                ast::Pattern::Other(_) => (None, &[][..], &[][..]),
            };
            let mut bindings = Vec::with_capacity(source_bindings.len());
            for (binding, (_, field_type)) in source_bindings.iter().zip(field_types) {
                let Some(definition) = self.parent.definition_at(binding.span, DefKind::Variable)
                else {
                    self.parent
                        .error(binding.span, "match binding has no resolved definition");
                    return None;
                };
                bindings.push(self.allocate_local(
                    definition,
                    &binding.name,
                    *field_type,
                    false,
                    binding.span,
                ));
            }
            let body = self.lower_block(&arm.body);
            arms.push(MatchArm {
                variant,
                bindings,
                body,
                span: arm.span,
            });
        }
        Some(StatementKind::Match { scrutinee, arms })
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
            let Some(call_type) = self.pipeline_step_call_types.get(&step.span).copied() else {
                self.parent
                    .error(step.span, "pipeline step has no checked call-result type");
                return None;
            };
            current = self.lower_pipeline_step(current, step, call_type)?;
            if let Some(handle) = &step.handle {
                current = self.lower_handle(
                    current,
                    handle.error_name.as_ref(),
                    &handle.body,
                    output_type,
                    handle.span,
                )?;
            }
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
        let kind = if self.is_source_call(callee, step.span) {
            ExpressionKind::Call {
                function: self.resolve_user_call_target(callee, step.span)?,
                args,
                evaluation_order,
            }
        } else {
            ExpressionKind::Intrinsic {
                canonical_name: self.canonical_call_name(callee)?,
                args,
                evaluation_order,
            }
        };
        Some(Expression {
            kind,
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
            Type::Struct(_) | Type::Bitfield(_) | Type::MachineState { .. } => base.ty,
            Type::Secret(inner)
                if matches!(
                    self.parent.check.interner.resolve(*inner),
                    Type::Struct(_) | Type::Bitfield(_) | Type::MachineState { .. }
                ) =>
            {
                *inner
            }
            _ => {
                self.parent.error(
                    field.span,
                    "checked field owner has no HIR aggregate layout",
                );
                return None;
            }
        };
        let field_index = match *self.parent.check.interner.resolve(owner_type) {
            Type::Struct(struct_id) => self
                .parent
                .check
                .interner
                .resolve_struct(struct_id)
                .fields
                .iter()
                .position(|(name, _)| name == &field.name),
            Type::Bitfield(bitfield_id) => self
                .parent
                .check
                .interner
                .resolve_bitfield(bitfield_id)
                .fields
                .iter()
                .position(|candidate| candidate.name == field.name),
            Type::MachineState { machine, state } => self
                .parent
                .check
                .interner
                .resolve_machine(machine)
                .state(state)
                .and_then(|state| {
                    state
                        .fields
                        .iter()
                        .position(|(name, _)| name == &field.name)
                }),
            _ => unreachable!(),
        };
        let Some(field_index) = field_index else {
            self.parent
                .error(field.span, "checked aggregate field has no canonical index");
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
    fn snapshots_core_hir() {
        let program = lower_source(
            r#"namespace app
function add(left: int64, right: int64) returns int64:
    return left + right
function main() returns int64:
    int64 value = add(right: 2, left: 1)
    if value > 0:
        return value
    return 0
"#,
        );
        insta::assert_debug_snapshot!("hir_core", program);
    }

    #[test]
    fn snapshots_enum_and_handle_hir() {
        let program = lower_source(
            r#"namespace app
enum Value:
    number(value: int64)
    missing
function parse(raw: string) returns result[Value, string]:
    return ok(Value.number(1))
function main() returns int64:
    Value value = parse("x") handle error:
        default Value.missing
    match value:
        number(number):
            return number
        missing:
            return 0
"#,
        );
        insta::assert_debug_snapshot!("hir_enum_handle", program);
    }

    #[test]
    fn validator_rejects_unknown_function_targets() {
        let mut program = lower_source(
            r#"namespace app
function identity(value: int64) returns int64:
    return value
function main() returns int64:
    return identity(1)
"#,
        );
        let StatementKind::Return(Some(Expression {
            kind: ExpressionKind::Call { function, .. },
            ..
        })) = &mut program.functions[1].body.statements[0].kind
        else {
            panic!("expected call");
        };
        *function = FunctionId(99);
        let errors = validate(&program).expect_err("invalid target must fail validation");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("unknown HIR function"))
        );
    }

    #[test]
    fn validator_rejects_invalid_argument_evaluation_orders() {
        let mut program = lower_source(
            r#"namespace app
function difference(left: int64, right: int64) returns int64:
    return left - right
function main() returns int64:
    return difference(right: 2, left: 7)
"#,
        );
        let StatementKind::Return(Some(Expression {
            kind: ExpressionKind::Call {
                evaluation_order, ..
            },
            ..
        })) = &mut program.functions[1].body.statements[0].kind
        else {
            panic!("expected call");
        };
        *evaluation_order = vec![0, 0];

        let errors = validate(&program).expect_err("invalid evaluation order must fail validation");
        assert!(errors.iter().any(|error| {
            error
                .message
                .contains("argument evaluation order must be a permutation")
        }));
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
    fn lowers_for_debug_comptime_and_interpolation_forms() {
        let source = r#"namespace app
function render(view values: list[int64]) returns string:
    mutable string output = comptime "values"
    for value in view values:
        breakpoint value > 10
        trace value
        output = "{output}:{value}"
    return output
"#;
        let program = lower_source(source);
        let function = &program.functions[0];
        assert!(matches!(
            function.body.statements[0].kind,
            StatementKind::Let {
                value: Expression {
                    kind: ExpressionKind::Comptime(_),
                    ..
                },
                ..
            }
        ));
        let StatementKind::For { body, .. } = &function.body.statements[1].kind else {
            panic!("expected explicit for loop");
        };
        assert!(matches!(
            body.statements[0].kind,
            StatementKind::Breakpoint(Some(_))
        ));
        assert!(matches!(body.statements[1].kind, StatementKind::Trace(_)));
        let StatementKind::Assign { value, .. } = &body.statements[2].kind else {
            panic!("expected interpolation assignment");
        };
        assert!(matches!(value.kind, ExpressionKind::StringInterpolation(_)));
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
    fn lowers_wrappers_and_general_handles_with_scoped_control_flow() {
        let source = r#"namespace app
function parse(value: string) returns result[int64, string]:
    return ok(7)
function first() returns optional[int64]:
    return some(3)
function fallback() returns optional[int64]:
    return none
function main() returns int64:
    int64 parsed = parse("x") handle error:
        string message = error
        default 0
    int64 present = first() handle:
        default 1
    int64 absent = fallback() handle:
        return 2
    return parsed + present + absent
"#;
        let program = lower_source(source);
        let parse = &program.functions[0];
        let first = &program.functions[1];
        let fallback = &program.functions[2];
        let main = &program.functions[3];

        let StatementKind::Return(Some(Expression {
            kind: ExpressionKind::ResultOk(_),
            ..
        })) = &parse.body.statements[0].kind
        else {
            panic!("expected explicit result-ok construction");
        };
        assert!(matches!(
            first.body.statements[0].kind,
            StatementKind::Return(Some(Expression {
                kind: ExpressionKind::OptionalSome(_),
                ..
            }))
        ));
        assert!(matches!(
            fallback.body.statements[0].kind,
            StatementKind::Return(Some(Expression {
                kind: ExpressionKind::OptionalNone,
                ..
            }))
        ));

        let StatementKind::Let { value, .. } = &main.body.statements[0].kind else {
            panic!("expected handled result");
        };
        let ExpressionKind::Handle {
            kind: HandleKind::Result,
            error_local: Some(error_local),
            failure,
            ..
        } = &value.kind
        else {
            panic!("expected result handle with an error binding");
        };
        assert_eq!(main.locals[error_local.index() as usize].name, "error");
        assert!(matches!(
            failure.statements[1].kind,
            StatementKind::HandleDefault(_)
        ));

        let StatementKind::Let { value, .. } = &main.body.statements[1].kind else {
            panic!("expected handled optional");
        };
        assert!(matches!(
            value.kind,
            ExpressionKind::Handle {
                kind: HandleKind::Optional,
                error_local: None,
                ..
            }
        ));

        let StatementKind::Let { value, .. } = &main.body.statements[2].kind else {
            panic!("expected return-terminated handle");
        };
        let ExpressionKind::Handle { failure, .. } = &value.kind else {
            panic!("expected optional handle");
        };
        assert!(matches!(
            failure.statements[0].kind,
            StatementKind::Return(Some(_))
        ));
    }

    #[test]
    fn lowers_enum_construction_and_exhaustive_match() {
        let source = r#"namespace app
enum Shape:
    point
    circle(radius: int64)
    rect(width: int64, height: int64)
function area(shape: Shape) returns int64:
    match shape:
        point:
            return 0
        circle(radius):
            return radius * radius
        rect(width, height):
            return width * height
function main() returns int64:
    Shape shape = Shape.circle(3)
    return area(shape)
"#;
        let program = lower_source(source);
        let area = &program.functions[0];
        let main = &program.functions[1];
        let StatementKind::Match { arms, .. } = &area.body.statements[0].kind else {
            panic!("expected lowered match");
        };
        assert_eq!(arms.len(), 3);
        assert_eq!(arms[0].variant.expect("point variant").index(), 0);
        assert_eq!(arms[1].variant.expect("circle variant").index(), 1);
        assert_eq!(arms[1].bindings.len(), 1);
        assert_eq!(arms[2].bindings.len(), 2);

        let StatementKind::Let { value, .. } = &main.body.statements[0].kind else {
            panic!("expected enum local");
        };
        let ExpressionKind::EnumConstruct {
            variant, payloads, ..
        } = &value.kind
        else {
            panic!("expected enum construction");
        };
        assert_eq!(variant.index(), 1);
        assert!(matches!(payloads[0].kind, ExpressionKind::Int(3)));
    }

    #[test]
    fn lowers_refinement_boundary_handles_explicitly() {
        let source = r#"namespace app
type Positive = int64 where value > 0
function positive_or_one(raw: int64, fallback: Positive) returns Positive:
    Positive value = raw handle error:
        default fallback
    return value
"#;
        let program = lower_source(source);
        let function = &program.functions[0];
        let StatementKind::Let { value, .. } = &function.body.statements[0].kind else {
            panic!("expected refinement local");
        };
        let ExpressionKind::Handle {
            kind: HandleKind::Refinement { refined_type },
            error_local: Some(error_local),
            ..
        } = value.kind
        else {
            panic!("expected refinement boundary handle");
        };
        assert_eq!(refined_type, value.ty);
        assert_eq!(function.locals[error_local.index() as usize].name, "error");
    }

    #[test]
    fn lowers_bitfield_and_machine_operations() {
        let source = r#"namespace app
bitfield Header:
    version: 4 bits
    length: 4 bits
machine Session:
    states:
        guest
        logged_in(user_id: string)
    transitions:
        guest to logged_in
function header() returns int64:
    Header value = Header(length: 5, version: 4)
    return value.version
function login(session: Session at guest) returns string:
    Session at logged_in next = Session.transition(session, logged_in, "user-1")
    return next.user_id
function guest() returns Session at guest:
    return Session(guest)
"#;
        let program = lower_source(source);
        let header = &program.functions[0];
        let StatementKind::Let { value, .. } = &header.body.statements[0].kind else {
            panic!("expected bitfield local");
        };
        let ExpressionKind::BitfieldConstruct {
            fields,
            evaluation_order,
            ..
        } = &value.kind
        else {
            panic!("expected bitfield construction");
        };
        assert!(matches!(fields[0].kind, ExpressionKind::Int(4)));
        assert_eq!(evaluation_order, &[1, 0]);

        let login = &program.functions[1];
        let StatementKind::Let { value, .. } = &login.body.statements[0].kind else {
            panic!("expected transition local");
        };
        assert!(matches!(
            value.kind,
            ExpressionKind::MachineTransition {
                target: StateId(1),
                ..
            }
        ));
        let guest = &program.functions[2];
        assert!(matches!(
            guest.body.statements[0].kind,
            StatementKind::Return(Some(Expression {
                kind: ExpressionKind::MachineConstruct {
                    state: StateId(0),
                    ..
                },
                ..
            }))
        ));
    }

    #[test]
    fn lowers_checked_compiler_calls_as_intrinsics() {
        let source = r#"namespace app
function absolute(value: int64) returns int64:
    return math.abs(value)
"#;
        let program = lower_source(source);
        let StatementKind::Return(Some(Expression {
            kind:
                ExpressionKind::Intrinsic {
                    canonical_name,
                    args,
                    ..
                },
            ..
        })) = &program.functions[0].body.statements[0].kind
        else {
            panic!("expected compiler intrinsic");
        };
        assert_eq!(canonical_name, "math.abs");
        assert_eq!(args.len(), 1);
    }

    #[test]
    fn lowers_actor_operations() {
        let source = r#"namespace app
actor Counter:
    mutable int64 count = 0
    receive add(amount: int64):
        count = count + amount
    receive current responds int64:
        respond count
function main() returns int64:
    Counter counter = spawn Counter()
    send counter.add(2)
    return ask counter.current
"#;
        let program = lower_source(source);
        let main = &program.functions[0];
        let StatementKind::Let { value, .. } = &main.body.statements[0].kind else {
            panic!("expected actor local");
        };
        assert!(matches!(value.kind, ExpressionKind::ActorSpawn { .. }));
        assert!(matches!(
            main.body.statements[1].kind,
            StatementKind::Expression(Expression {
                kind: ExpressionKind::ActorMessage {
                    kind: ActorMessageKind::Send,
                    ..
                },
                ..
            })
        ));
        assert!(matches!(
            main.body.statements[2].kind,
            StatementKind::Return(Some(Expression {
                kind: ExpressionKind::ActorMessage {
                    kind: ActorMessageKind::Ask,
                    ..
                },
                ..
            }))
        ));
    }

    #[test]
    fn lowers_actor_named_arguments_to_parameter_order() {
        let source = r#"namespace app
actor Counter(seed: int64, step: int64):
    mutable int64 count = seed
    receive update(first: int64, second: int64):
        count = first + second
function main() returns nothing:
    Counter counter = spawn Counter(step: 2, seed: 1)
    send counter.update(second: 4, first: 3)
    return nothing
"#;
        let program = lower_source(source);
        let main = &program.functions[0];

        let StatementKind::Let { value, .. } = &main.body.statements[0].kind else {
            panic!("expected actor local");
        };
        let ExpressionKind::ActorSpawn {
            args,
            evaluation_order,
            ..
        } = &value.kind
        else {
            panic!("expected actor spawn");
        };
        assert!(matches!(args[0].kind, ExpressionKind::Int(1)));
        assert!(matches!(args[1].kind, ExpressionKind::Int(2)));
        assert_eq!(evaluation_order, &[1, 0]);

        let StatementKind::Expression(Expression {
            kind:
                ExpressionKind::ActorMessage {
                    args,
                    evaluation_order,
                    ..
                },
            ..
        }) = &main.body.statements[1].kind
        else {
            panic!("expected actor message");
        };
        assert!(matches!(args[0].kind, ExpressionKind::Int(3)));
        assert!(matches!(args[1].kind, ExpressionKind::Int(4)));
        assert_eq!(evaluation_order, &[1, 0]);
    }

    #[test]
    fn validator_rejects_invalid_actor_message_evaluation_orders() {
        let mut program = lower_source(
            r#"namespace app
actor Counter:
    receive update(first: int64, second: int64):
        return nothing
function main() returns nothing:
    Counter counter = spawn Counter()
    send counter.update(second: 4, first: 3)
    return nothing
"#,
        );
        for invalid_order in [vec![0, 0], vec![2, 0], vec![0], vec![]] {
            let StatementKind::Expression(Expression {
                kind:
                    ExpressionKind::ActorMessage {
                        evaluation_order, ..
                    },
                ..
            }) = &mut program.functions[0].body.statements[1].kind
            else {
                panic!("expected actor message");
            };
            *evaluation_order = invalid_order;
            let errors = validate(&program).expect_err("invalid actor evaluation order");
            assert!(errors.iter().any(|error| {
                error
                    .message
                    .contains("argument evaluation order must be a permutation")
            }));
        }
    }

    #[test]
    fn lowers_inline_functions_and_indirect_calls() {
        let source = r#"namespace app
function main() returns int64:
    function(int64) returns int64 double = function(value: int64) returns int64: return value * 2
    return double(4)
"#;
        let program = lower_source(source);
        let main = &program.functions[0];
        let StatementKind::Let { value, .. } = &main.body.statements[0].kind else {
            panic!("expected closure local");
        };
        assert!(matches!(value.kind, ExpressionKind::InlineFunction { .. }));
        assert!(matches!(
            main.body.statements[1].kind,
            StatementKind::Return(Some(Expression {
                kind: ExpressionKind::IndirectCall { .. },
                ..
            }))
        ));
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
    fn lowers_handled_pipeline_steps_and_continues_with_the_unwrapped_value() {
        let source = r#"namespace app
function parse(value: string) returns result[int64, string]:
    return fail("invalid")
function plus_one(value: int64) returns int64:
    return value + 1
function main() returns int64:
    return "x"
        into parse handle error:
            string message = error
            default 0
        into plus_one
"#;
        let program = lower_source(source);
        let main = &program.functions[2];
        let StatementKind::Return(Some(Expression {
            kind: ExpressionKind::Call { args, .. },
            ..
        })) = &main.body.statements[0].kind
        else {
            panic!("expected final pipeline call");
        };
        let ExpressionKind::Handle {
            target,
            kind: HandleKind::Result,
            error_local: Some(_),
            failure,
        } = &args[0].kind
        else {
            panic!("expected handled pipeline call as the next step input");
        };
        assert!(matches!(target.kind, ExpressionKind::Call { .. }));
        assert!(matches!(
            failure.statements[1].kind,
            StatementKind::HandleDefault(_)
        ));
    }

    #[test]
    fn keeps_handled_pipeline_types_inside_generic_instantiations() {
        let source = r#"namespace app
function wrap[T](value: T) returns result[T, string]:
    return ok(value)
function recover[T](value: T, fallback: T) returns T:
    return value
        into wrap[T]() handle error:
            default fallback
function main() returns int64:
    return recover[int64](7, 0)
"#;
        let program = lower_source(source);
        let recover = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "recover")
            .expect("concrete recover function");
        let StatementKind::Return(Some(Expression {
            kind:
                ExpressionKind::Handle {
                    target,
                    kind: HandleKind::Result,
                    ..
                },
            ..
        })) = &recover.body.statements[0].kind
        else {
            panic!("expected handled generic pipeline");
        };
        assert!(matches!(target.kind, ExpressionKind::Call { .. }));
        assert!(matches!(
            recover
                .identity
                .type_arguments
                .first()
                .map(|ty| program.functions.iter().any(|function| {
                    function.identity.declaration.name == "wrap"
                        && function.identity.type_arguments.first() == Some(ty)
                })),
            Some(true)
        ));
    }
}
