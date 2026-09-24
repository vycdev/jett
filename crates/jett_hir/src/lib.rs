//! Jett's typed, backend-neutral high-level intermediate representation.
//!
//! The initial lowering covers ordinary and generic functions plus core
//! structured control flow. Unsupported source constructs fail explicitly;
//! they never survive as embedded AST nodes.

use std::collections::{HashMap, HashSet};

use jett_common::{FileId, SourceOrigin, Span};
pub use jett_intrinsics::IntrinsicId;
use jett_parser::ast::{self, Expr, Item, Module, Stmt};
use jett_resolve::{DefId, DefKind, ResolveResult};
use jett_typecheck::{
    CheckResult, CheckedBodyFacts, CheckedCallArgumentOrder, CheckedComptimeTypeBinding,
    CheckedComptimeTypeSelection, CheckedGenericCall, CheckedGenericFunctionInstantiation,
    CheckedGenericSpecialization, CheckedMethodCall, CheckedMethodDefinition,
    CheckedStaticSelection, CheckedStructConstruction,
};
use jett_types::{
    ReflectionBitfieldFieldInfo, ReflectionBitfieldInfo, ReflectionFieldInfo,
    ReflectionMachineInfo, ReflectionMachineStateInfo, ReflectionMachineTransitionInfo,
    ReflectionTypeInfo, ReflectionVariantInfo, Type, TypeId, TypeInterner,
};

mod inline_functions;
mod type_validation;

pub use type_validation::validate_backend_types;

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
    /// Allocate a canonical local identity during backend-neutral lowering.
    pub fn new(index: u32) -> Self {
        Self(index)
    }

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
    ActorHandler,
    RefinementPredicate,
}

/// One concrete in-memory function identity. Type arguments and checked
/// reflection-visible specialization facts form identity after
/// monomorphization. Both are empty for an ordinary function. Raw `TypeId`s
/// are session-local; persistent artifacts must encode their canonical
/// structural type identities and this specialization discriminator instead.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionIdentity {
    pub declaration: DeclarationId,
    pub type_arguments: Vec<TypeId>,
    pub specialization: CheckedGenericSpecialization,
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
    Breakpoint {
        condition: Option<Expression>,
        bindings: Vec<LocalId>,
    },
    Respond(Expression),
    Scope(Block),
    /// Execute the checker-specialized body whose concrete bound type matches
    /// the runtime `TypeInfo` value produced by a trusted reflection loop.
    ///
    /// This is compiler-owned control flow. Source cannot construct it, and a
    /// backend must compare canonical reflected type identity rather than
    /// executing every arm for every loop element.
    ReflectedTypeDispatch {
        type_info: Expression,
        arms: Vec<ReflectedTypeArm>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReflectedTypeArm {
    /// Canonical position exported by the checker. It is retained for stable
    /// diagnostics and for backends that can dispatch on loop ordinals.
    pub iteration_index: usize,
    pub bound_type: TypeId,
    /// Canonical identity of the checker-selected bound type, including
    /// nested type arguments while treating source aliases transparently.
    pub canonical_identity: String,
    pub body: Block,
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
    /// A checked, concrete source function used as a first-class value.
    FunctionRef(FunctionId),
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
        intrinsic: IntrinsicId,
        /// Concrete checked generic operands in source order. An intrinsic
        /// with no type operands carries an empty vector.
        type_arguments: Vec<TypeId>,
        /// Checker-owned source identity for reflection operands, including
        /// aliases that share a canonical type ID.
        reflection_arguments: Vec<ReflectionTypeInfo>,
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
    /// Compiler-owned conversion after every required refinement predicate passed.
    RefinementValidated(Box<Expression>),
    StateIs {
        value: Box<Expression>,
        state: StateId,
    },
    Run(Box<Expression>),
    Join(Box<Expression>),
    Cancel(Box<Expression>),
    InlineFunction {
        params: Vec<LocalId>,
        view_params: Vec<LocalId>,
        /// First local allocated inside this closure. Earlier locals are captures.
        local_floor: u32,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandleKind {
    Result,
    Optional,
    Refinement {
        refined_type: TypeId,
        predicates: Vec<RefinementPredicate>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefinementPredicate {
    pub refined_type: TypeId,
    pub type_name: String,
    pub function: FunctionId,
    pub input_type: TypeId,
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
            StatementKind::Breakpoint {
                condition,
                bindings,
            } => {
                if let Some(condition) = condition {
                    self.expression(condition);
                }
                for binding in bindings {
                    self.check_local(*binding, statement.span);
                }
            }
            StatementKind::Respond(value) => self.expression(value),
            StatementKind::Scope(block) => self.block(block),
            StatementKind::ReflectedTypeDispatch { type_info, arms } => {
                self.expression(type_info);
                let mut iteration_indexes = std::collections::HashSet::new();
                for arm in arms {
                    if !iteration_indexes.insert(arm.iteration_index) {
                        self.error(
                            statement.span,
                            "reflected type dispatch contains a duplicate iteration index",
                        );
                    }
                    self.block(&arm.body);
                }
                if arms.is_empty() {
                    self.error(statement.span, "reflected type dispatch has no arms");
                }
            }
        }
    }

    fn expression(&mut self, expression: &Expression) {
        match &expression.kind {
            ExpressionKind::Local(local) => self.check_local(*local, expression.span),
            ExpressionKind::FunctionRef(function) => {
                if function.index() as usize >= self.program.functions.len() {
                    self.error(
                        expression.span,
                        "function value references an unknown HIR function",
                    );
                }
            }
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
            | ExpressionKind::RefinementValidated(value)
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
                kind,
                error_local,
                failure,
                ..
            } => {
                self.expression(target);
                if let HandleKind::Refinement { predicates, .. } = kind {
                    for predicate in predicates {
                        if predicate.function.index() as usize >= self.program.functions.len() {
                            self.error(
                                expression.span,
                                "refinement predicate is outside HIR function table",
                            );
                        }
                    }
                }
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
            ExpressionKind::InlineFunction {
                params,
                view_params,
                local_floor,
                body,
            } => {
                if *local_floor as usize > self.local_count {
                    self.error(
                        expression.span,
                        "inline function local floor is out of range",
                    );
                }
                for param in params {
                    self.check_local(*param, expression.span);
                    if param.index() < *local_floor {
                        self.error(
                            expression.span,
                            "inline function parameter precedes its local floor",
                        );
                    }
                }
                for view_param in view_params {
                    if !params.contains(view_param) {
                        self.error(
                            expression.span,
                            "inline function view parameter is not a parameter",
                        );
                    }
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

struct ActorHandlerSource<'a> {
    id: FunctionId,
    actor_definition: DefId,
    actor: &'a ast::ActorDef,
    handler: &'a ast::ReceiveHandler,
    message_index: usize,
}

struct RefinementSource<'a> {
    id: FunctionId,
    definition: DefId,
    alias: &'a ast::TypeAlias,
    ty: TypeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum FunctionKey {
    Definition {
        definition: DefId,
        concrete_args: Vec<TypeId>,
        specialization: CheckedGenericSpecialization,
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
    actor_handlers: Vec<ActorHandlerSource<'a>>,
    refinement_sources: Vec<RefinementSource<'a>>,
    refinement_function_ids: HashMap<TypeId, FunctionId>,
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
            actor_handlers: Vec::new(),
            refinement_sources: Vec::new(),
            refinement_function_ids: HashMap::new(),
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
        let actor_handlers = std::mem::take(&mut self.actor_handlers);
        for source in actor_handlers {
            if let Some(function) = self.lower_actor_handler(source) {
                functions.push(function);
            }
        }
        let refinement_sources = std::mem::take(&mut self.refinement_sources);
        for source in refinement_sources {
            if let Some(function) = self.lower_refinement_predicate(source) {
                functions.push(function);
            }
        }
        if self.errors.is_empty() {
            inline_functions::extract_capture_free(&mut functions, &self.check.interner);
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
            validate_backend_types(&program, &self.check.interner).map_err(|errors| {
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
                        self.error(
                            function.name.span,
                            format!(
                                "function `{}` has no resolved definition",
                                function.name.name
                            ),
                        );
                        continue;
                    };
                    let id = FunctionId(self.functions.len() as u32);
                    self.function_ids.insert(
                        FunctionKey::Definition {
                            definition,
                            concrete_args: Vec::new(),
                            specialization: CheckedGenericSpecialization::default(),
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
                    specialization: instantiation.specialization.clone(),
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

        for item in &self.module.items {
            let Item::Actor(actor) = item else {
                continue;
            };
            let Some(actor_definition) = self.definition_at(actor.name.span, DefKind::Actor) else {
                self.error(actor.name.span, "actor has no resolved definition");
                continue;
            };
            for (message_index, handler) in actor.handlers.iter().enumerate() {
                let id = FunctionId((self.functions.len() + self.actor_handlers.len()) as u32);
                self.actor_handlers.push(ActorHandlerSource {
                    id,
                    actor_definition,
                    actor,
                    handler,
                    message_index,
                });
            }
        }
        for item in &self.module.items {
            let Item::TypeAlias(alias) = item else {
                continue;
            };
            if alias.constraint.is_none() {
                continue;
            }
            let Some(definition) = self.definition_at(alias.name.span, DefKind::Type) else {
                self.error(alias.name.span, "refinement has no resolved definition");
                continue;
            };
            let declared = self.resolve.scope_table.def(definition);
            let canonical = match &declared.namespace {
                Some(namespace) if !namespace.is_empty() => {
                    format!("{namespace}.{}", alias.name.name)
                }
                _ => alias.name.name.clone(),
            };
            let Some(ty) = self.check.interner.type_ids().find(|id| {
                matches!(self.check.interner.resolve(*id), Type::Refinement { name, .. } if name == &canonical)
            }) else {
                self.error(alias.name.span, "checked refinement type is missing");
                continue;
            };
            let id = FunctionId(
                (self.functions.len() + self.actor_handlers.len() + self.refinement_sources.len())
                    as u32,
            );
            self.refinement_function_ids.insert(ty, id);
            self.refinement_sources.push(RefinementSource {
                id,
                definition,
                alias,
                ty,
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
            intrinsic_ids,
            intrinsic_type_arguments,
            intrinsic_reflection_arguments,
            call_argument_orders,
            method_calls,
            struct_constructions,
            pipeline_step_call_types,
            static_selections,
            comptime_type_bindings,
            concrete_args,
            specialization,
        ) = if let Some(instantiation) = &source.instantiation {
            (
                instantiation.parameter_types.clone(),
                instantiation.return_type,
                instantiation.type_map.clone(),
                instantiation.generic_calls.clone(),
                instantiation.intrinsic_ids.clone(),
                instantiation.intrinsic_type_arguments.clone(),
                instantiation.intrinsic_reflection_arguments.clone(),
                instantiation.call_argument_orders.clone(),
                instantiation.method_calls.clone(),
                instantiation.struct_constructions.clone(),
                instantiation.pipeline_step_call_types.clone(),
                instantiation.static_selections.clone(),
                instantiation.comptime_type_bindings.clone(),
                instantiation.concrete_args.clone(),
                instantiation.specialization.clone(),
            )
        } else if let Some(method) = &source.method {
            (
                method.parameter_types.clone(),
                method.return_type,
                self.check.type_map.clone(),
                self.check.generic_calls.clone(),
                self.check.intrinsic_ids.clone(),
                self.check.intrinsic_type_arguments.clone(),
                self.check.intrinsic_reflection_arguments.clone(),
                self.check.call_argument_orders.clone(),
                self.check.method_calls.clone(),
                self.check.struct_constructions.clone(),
                self.check.pipeline_step_call_types.clone(),
                HashMap::new(),
                facts_in_span(&self.check.comptime_type_bindings, source.function.span),
                Vec::new(),
                CheckedGenericSpecialization::default(),
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
                self.check.intrinsic_ids.clone(),
                self.check.intrinsic_type_arguments.clone(),
                self.check.intrinsic_reflection_arguments.clone(),
                self.check.call_argument_orders.clone(),
                self.check.method_calls.clone(),
                self.check.struct_constructions.clone(),
                self.check.pipeline_step_call_types.clone(),
                HashMap::new(),
                facts_in_span(&self.check.comptime_type_bindings, source.function.span),
                Vec::new(),
                CheckedGenericSpecialization::default(),
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
            expression_types,
            generic_calls,
            intrinsic_ids,
            intrinsic_type_arguments,
            intrinsic_reflection_arguments,
            call_argument_orders,
            method_calls,
            struct_constructions,
            pipeline_step_call_types,
            static_selections,
            comptime_type_bindings,
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
        body_lowerer.reject_unconsumed_static_selections();
        body_lowerer.reject_unconsumed_comptime_type_bindings();
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
                specialization,
            },
            source_definition: source.definition,
            params,
            return_type,
            locals,
            body,
            span: source.function.span,
        })
    }

    fn lower_actor_handler(&mut self, source: ActorHandlerSource<'a>) -> Option<Function> {
        let origin = match self.origins.get(&source.actor.span.file) {
            Some(origin) => origin.clone(),
            None => {
                self.error(
                    source.actor.span,
                    "source origin is missing for actor handler",
                );
                return None;
            }
        };
        let actor_type = match self.check.definition_types.get(&source.actor_definition) {
            Some(ty) => *ty,
            None => {
                self.error(source.actor.name.span, "actor has no checked type");
                return None;
            }
        };
        let actor_definition = match self.check.interner.resolve(actor_type) {
            Type::Actor(id) => self.check.interner.resolve_actor(*id).clone(),
            _ => {
                self.error(
                    source.actor.name.span,
                    "checked actor definition is not an actor",
                );
                return None;
            }
        };
        let message = match actor_definition.messages.get(source.message_index) {
            Some(message) => message.clone(),
            None => {
                self.error(
                    source.handler.span,
                    "actor handler has no checked message definition",
                );
                return None;
            }
        };
        if source.actor.capability_params.len() != actor_definition.capability_params.len()
            || source.actor.state_fields.len() != actor_definition.state_fields.len()
            || source.handler.params.len() != message.params.len()
        {
            self.error(
                source.handler.span,
                "checked actor shape changed during HIR lowering",
            );
            return None;
        }

        let function_ids = self.function_ids.clone();
        let expression_types = self.check.type_map.clone();
        let generic_calls = self.check.generic_calls.clone();
        let intrinsic_ids = self.check.intrinsic_ids.clone();
        let intrinsic_type_arguments = self.check.intrinsic_type_arguments.clone();
        let intrinsic_reflection_arguments = self.check.intrinsic_reflection_arguments.clone();
        let call_argument_orders = self.check.call_argument_orders.clone();
        let method_calls = self.check.method_calls.clone();
        let struct_constructions = self.check.struct_constructions.clone();
        let pipeline_step_call_types = self.check.pipeline_step_call_types.clone();
        let static_selections = HashMap::new();
        let comptime_type_bindings =
            facts_in_span(&self.check.comptime_type_bindings, source.handler.span);
        let mut body_lowerer = BodyLowerer::new(
            self,
            &function_ids,
            expression_types,
            generic_calls,
            intrinsic_ids,
            intrinsic_type_arguments,
            intrinsic_reflection_arguments,
            call_argument_orders,
            method_calls,
            struct_constructions,
            pipeline_step_call_types,
            static_selections,
            comptime_type_bindings,
        );

        for (param, (_, ty)) in source
            .actor
            .capability_params
            .iter()
            .zip(actor_definition.capability_params.iter())
        {
            let Some(definition) = body_lowerer
                .parent
                .definition_at(param.name.span, DefKind::Param)
            else {
                body_lowerer
                    .parent
                    .error(param.name.span, "actor capability parameter is unresolved");
                return None;
            };
            body_lowerer.allocate_local(
                definition,
                &param.name.name,
                *ty,
                param.mutable,
                param.span,
            );
        }
        for (field, (_, ty)) in source
            .actor
            .state_fields
            .iter()
            .zip(actor_definition.state_fields.iter())
        {
            let Some(definition) = body_lowerer
                .parent
                .definition_at(field.name.span, DefKind::Variable)
            else {
                body_lowerer
                    .parent
                    .error(field.name.span, "actor state field is unresolved");
                return None;
            };
            body_lowerer.allocate_local(
                definition,
                &field.name.name,
                *ty,
                field.mutable,
                field.span,
            );
        }
        let mut params = Vec::with_capacity(source.handler.params.len());
        for (param, (_, ty)) in source.handler.params.iter().zip(message.params.iter()) {
            let Some(definition) = body_lowerer
                .parent
                .definition_at(param.name.span, DefKind::Param)
            else {
                body_lowerer
                    .parent
                    .error(param.name.span, "actor message parameter is unresolved");
                return None;
            };
            let local = body_lowerer.allocate_local(
                definition,
                &param.name.name,
                *ty,
                param.mutable,
                param.span,
            );
            params.push(Param {
                local,
                name: param.name.name.clone(),
                ty: *ty,
                mode: if param.view {
                    ParamMode::View
                } else {
                    ParamMode::Owned
                },
                mutable: param.mutable,
                span: param.span,
            });
        }
        let body = body_lowerer.lower_block(&source.handler.body);
        body_lowerer.reject_unconsumed_static_selections();
        body_lowerer.reject_unconsumed_comptime_type_bindings();
        let locals = body_lowerer.locals;
        let actor_def = self.resolve.scope_table.def(source.actor_definition);

        Some(Function {
            id: source.id,
            identity: FunctionIdentity {
                declaration: DeclarationId {
                    origin,
                    namespace: actor_def.namespace.clone().unwrap_or_default(),
                    name: format!("{}.{}", source.actor.name.name, source.handler.name.name),
                    kind: DeclarationKind::ActorHandler,
                },
                type_arguments: Vec::new(),
                specialization: CheckedGenericSpecialization::default(),
            },
            source_definition: None,
            params,
            return_type: message.responds,
            locals,
            body,
            span: source.handler.span,
        })
    }

    fn lower_refinement_predicate(&mut self, source: RefinementSource<'a>) -> Option<Function> {
        let constraint = source.alias.constraint.as_ref()?;
        let origin = match self.origins.get(&source.alias.span.file) {
            Some(origin) => origin.clone(),
            None => {
                self.error(source.alias.span, "source origin is missing for refinement");
                return None;
            }
        };
        let Some(value_definition) = self.definition_at(constraint.span(), DefKind::Variable)
        else {
            self.error(
                constraint.span(),
                "refinement value has no resolved definition",
            );
            return None;
        };
        let mut input_type = source.ty;
        while let Type::Refinement { base, .. } = self.check.interner.resolve(input_type) {
            input_type = *base;
        }
        if let Type::Secret(inner) = self.check.interner.resolve(input_type) {
            input_type = *inner;
        }
        let function_ids = self.function_ids.clone();
        let mut lowerer = BodyLowerer::new(
            self,
            &function_ids,
            self.check.type_map.clone(),
            self.check.generic_calls.clone(),
            self.check.intrinsic_ids.clone(),
            self.check.intrinsic_type_arguments.clone(),
            self.check.intrinsic_reflection_arguments.clone(),
            self.check.call_argument_orders.clone(),
            self.check.method_calls.clone(),
            self.check.struct_constructions.clone(),
            self.check.pipeline_step_call_types.clone(),
            HashMap::new(),
            facts_in_span(&self.check.comptime_type_bindings, source.alias.span),
        );
        let local = lowerer.allocate_local(
            value_definition,
            "value",
            input_type,
            false,
            constraint.span(),
        );
        let result = lowerer.lower_expression(constraint)?;
        lowerer.reject_unconsumed_comptime_type_bindings();
        let locals = lowerer.locals;
        let declared = self.resolve.scope_table.def(source.definition);
        Some(Function {
            id: source.id,
            identity: FunctionIdentity {
                declaration: DeclarationId {
                    origin,
                    namespace: declared.namespace.clone().unwrap_or_default(),
                    name: source.alias.name.name.clone(),
                    kind: DeclarationKind::RefinementPredicate,
                },
                type_arguments: Vec::new(),
                specialization: CheckedGenericSpecialization::default(),
            },
            source_definition: Some(source.definition),
            params: vec![Param {
                local,
                name: "value".to_string(),
                ty: input_type,
                mode: ParamMode::Owned,
                mutable: false,
                span: constraint.span(),
            }],
            return_type: TypeInterner::BOOL,
            locals,
            body: Block {
                statements: vec![Statement {
                    kind: StatementKind::Return(Some(result)),
                    span: constraint.span(),
                }],
                span: source.alias.span,
            },
            span: source.alias.span,
        })
    }

    fn definition_at(&self, span: Span, kind: DefKind) -> Option<DefId> {
        self.resolve
            .resolutions
            .get(&span)
            .copied()
            .filter(|definition| self.resolve.scope_table.def(*definition).kind == kind)
            .or_else(|| {
                self.resolve
                    .scope_table
                    .definitions
                    .iter()
                    .find(|definition| definition.span == span && definition.kind == kind)
                    .map(|definition| definition.id)
            })
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
    expression_types: HashMap<Span, TypeId>,
    generic_calls: HashMap<Span, CheckedGenericCall>,
    intrinsic_ids: HashMap<Span, IntrinsicId>,
    intrinsic_type_arguments: HashMap<Span, Vec<TypeId>>,
    intrinsic_reflection_arguments: HashMap<Span, Vec<ReflectionTypeInfo>>,
    call_argument_orders: HashMap<Span, CheckedCallArgumentOrder>,
    method_calls: HashMap<Span, CheckedMethodCall>,
    struct_constructions: HashMap<Span, CheckedStructConstruction>,
    pipeline_step_call_types: HashMap<Span, TypeId>,
    static_selections: HashMap<Span, CheckedStaticSelection>,
    comptime_type_bindings: HashMap<Span, Vec<CheckedComptimeTypeBinding>>,
    consumed_static_selections: HashSet<Span>,
    consumed_comptime_type_bindings: HashSet<Span>,
    local_ids: HashMap<DefId, LocalId>,
    locals: Vec<Local>,
    visible_bindings: Vec<HashMap<String, LocalId>>,
}

impl<'lowerer, 'program> BodyLowerer<'lowerer, 'program> {
    fn new(
        parent: &'lowerer mut Lowerer<'program>,
        function_ids: &'lowerer HashMap<FunctionKey, FunctionId>,
        expression_types: HashMap<Span, TypeId>,
        generic_calls: HashMap<Span, CheckedGenericCall>,
        intrinsic_ids: HashMap<Span, IntrinsicId>,
        intrinsic_type_arguments: HashMap<Span, Vec<TypeId>>,
        intrinsic_reflection_arguments: HashMap<Span, Vec<ReflectionTypeInfo>>,
        call_argument_orders: HashMap<Span, CheckedCallArgumentOrder>,
        method_calls: HashMap<Span, CheckedMethodCall>,
        struct_constructions: HashMap<Span, CheckedStructConstruction>,
        pipeline_step_call_types: HashMap<Span, TypeId>,
        static_selections: HashMap<Span, CheckedStaticSelection>,
        comptime_type_bindings: HashMap<Span, Vec<CheckedComptimeTypeBinding>>,
    ) -> Self {
        Self {
            parent,
            function_ids,
            expression_types,
            generic_calls,
            intrinsic_ids,
            intrinsic_type_arguments,
            intrinsic_reflection_arguments,
            call_argument_orders,
            method_calls,
            struct_constructions,
            pipeline_step_call_types,
            static_selections,
            comptime_type_bindings,
            consumed_static_selections: HashSet::new(),
            consumed_comptime_type_bindings: HashSet::new(),
            local_ids: HashMap::new(),
            locals: Vec::new(),
            visible_bindings: vec![HashMap::new()],
        }
    }

    fn static_selection(&mut self, span: Span) -> Option<CheckedStaticSelection> {
        let selection = self.static_selections.get(&span).copied()?;
        self.consumed_static_selections.insert(span);
        Some(selection)
    }

    fn reject_unconsumed_static_selections(&mut self) {
        let mut spans = self
            .static_selections
            .keys()
            .copied()
            .filter(|span| !self.consumed_static_selections.contains(span))
            .collect::<Vec<_>>();
        spans.sort_by_key(|span| (span.file.index(), span.start, span.end));
        for span in spans {
            self.parent.error(
                span,
                "checked static selection does not match a lowered statement",
            );
        }
    }

    fn reject_unconsumed_comptime_type_bindings(&mut self) {
        let mut spans = self
            .comptime_type_bindings
            .keys()
            .copied()
            .filter(|span| !self.consumed_comptime_type_bindings.contains(span))
            .collect::<Vec<_>>();
        spans.sort_by_key(|span| (span.file.index(), span.start, span.end));
        for span in spans {
            self.parent.error(
                span,
                "checked comptime type binding does not match a lowered statement",
            );
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
        if let Some(scope) = self.visible_bindings.last_mut() {
            scope.insert(name.to_string(), id);
        }
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
        self.visible_bindings.push(HashMap::new());
        let mut statements = Vec::new();
        for statement in &block.stmts {
            if let Some(statement) = self.lower_statement(statement) {
                statements.push(statement);
            }
        }
        self.visible_bindings.pop();
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
                if let Some(selection) = self.static_selection(branch.span) {
                    let selected = match selection {
                        CheckedStaticSelection::IfThen => Some(&branch.then_block),
                        CheckedStaticSelection::IfElseIf(index) => {
                            match branch.else_ifs.get(index) {
                                Some((_, block)) => Some(block),
                                None => {
                                    self.parent.error(
                                        branch.span,
                                        "checked static else-if selection is out of range",
                                    );
                                    return None;
                                }
                            }
                        }
                        CheckedStaticSelection::IfElse => match branch.else_block.as_ref() {
                            Some(block) => Some(block),
                            None => {
                                self.parent.error(
                                    branch.span,
                                    "checked static else selection has no source branch",
                                );
                                return None;
                            }
                        },
                        CheckedStaticSelection::IfNoBranch => {
                            if branch.else_block.is_some() {
                                self.parent.error(
                                    branch.span,
                                    "checked static no-branch selection disagrees with source else",
                                );
                                return None;
                            }
                            None
                        }
                        CheckedStaticSelection::MatchArm(_) => {
                            self.parent.error(
                                branch.span,
                                "checked static match selection attached to an if statement",
                            );
                            return None;
                        }
                    };
                    let Some(selected) = selected else {
                        return None;
                    };
                    (
                        StatementKind::Scope(self.lower_block(selected)),
                        branch.span,
                    )
                } else {
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
            }
            Stmt::While(loop_stmt) => (
                StatementKind::While {
                    condition: self.lower_expression(&loop_stmt.condition)?,
                    body: self.lower_block(&loop_stmt.body),
                },
                loop_stmt.span,
            ),
            Stmt::For(loop_stmt) => {
                let iterable = self.lower_expression(&loop_stmt.iterable)?;
                self.visible_bindings.push(HashMap::new());
                let key = self.allocate_declared_local(&loop_stmt.variable, false)?;
                let value = match &loop_stmt.value_variable {
                    Some(binding) => Some(self.allocate_declared_local(binding, false)?),
                    None => None,
                };
                let body = self.lower_block(&loop_stmt.body);
                self.visible_bindings.pop();
                (
                    StatementKind::For {
                        key,
                        value,
                        by_view: loop_stmt.view,
                        iterable,
                        body,
                    },
                    loop_stmt.span,
                )
            }
            Stmt::Match(match_stmt) => {
                if let Some(selection) = self.static_selection(match_stmt.span) {
                    let CheckedStaticSelection::MatchArm(index) = selection else {
                        self.parent.error(
                            match_stmt.span,
                            "checked static if selection attached to a match statement",
                        );
                        return None;
                    };
                    let Some(arm) = match_stmt.arms.get(index) else {
                        self.parent.error(
                            match_stmt.span,
                            "checked static match-arm selection is out of range",
                        );
                        return None;
                    };
                    if matches!(&arm.pattern, ast::Pattern::Variant(_, bindings) if !bindings.is_empty())
                    {
                        self.parent.error(
                            arm.span,
                            "checked static match arm unexpectedly binds runtime payloads",
                        );
                        return None;
                    }
                    (
                        StatementKind::Scope(self.lower_block(&arm.body)),
                        match_stmt.span,
                    )
                } else {
                    (self.lower_match(match_stmt)?, match_stmt.span)
                }
            }
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
            Stmt::Breakpoint(point) => {
                let condition = match &point.condition {
                    Some(value) => Some(self.lower_expression(value)?),
                    None => None,
                };
                let mut visible = std::collections::BTreeMap::new();
                for scope in &self.visible_bindings {
                    visible.extend(scope.iter().map(|(name, local)| (name, *local)));
                }
                (
                    StatementKind::Breakpoint {
                        condition,
                        bindings: visible.into_values().collect(),
                    },
                    point.span,
                )
            }
            Stmt::Respond(response) => (
                StatementKind::Respond(self.lower_expression(&response.value)?),
                response.span,
            ),
            Stmt::ComptimeTypeBind(binding) => {
                (self.lower_comptime_type_bind(binding)?, binding.span)
            }
            Stmt::Use(_) => return None,
        };
        Some(Statement { kind, span })
    }

    fn lower_comptime_type_bind(
        &mut self,
        binding: &ast::ComptimeTypeBindStmt,
    ) -> Option<StatementKind> {
        let Some(bindings) = self.comptime_type_bindings.get(&binding.span).cloned() else {
            self.parent.error(
                binding.span,
                "comptime type statement has no checked concrete binding",
            );
            return None;
        };
        self.consumed_comptime_type_bindings.insert(binding.span);
        // The checker retains the last recursively checked expansion in its
        // legacy flat maps for the interpreter. Those descendant entries are
        // owned by `CheckedBodyFacts` and are validated while lowering each
        // specialized body, not as siblings in the enclosing function.
        self.consumed_comptime_type_bindings.extend(
            self.comptime_type_bindings.keys().copied().filter(|span| {
                span.file == binding.body.span.file
                    && span.start >= binding.body.span.start
                    && span.end <= binding.body.span.end
            }),
        );

        if bindings.len() == 1
            && bindings[0].selection == CheckedComptimeTypeSelection::Unconditional
        {
            let checked = bindings
                .into_iter()
                .next()
                .expect("single checked binding exists");
            return Some(StatementKind::Scope(
                self.lower_block_with_checked_facts(&binding.body, checked.body),
            ));
        }

        if bindings.is_empty()
            || bindings.iter().any(|checked| {
                !matches!(
                    checked.selection,
                    CheckedComptimeTypeSelection::ReflectedIteration(_)
                )
            })
        {
            self.parent.error(
                binding.span,
                "checked comptime type binding has inconsistent selection semantics",
            );
            return None;
        }

        // The initializer is a trusted `TypeInfo` value belonging to the
        // current reflection-loop element. It is evaluated once, then used to
        // select the one checker-specialized body with matching canonical
        // reflected type identity.
        let type_info = self.lower_expression(&binding.value)?;
        let is_type_info = match self.parent.check.interner.resolve(type_info.ty) {
            Type::Struct(id) => self.parent.check.interner.resolve_struct(*id).name == "TypeInfo",
            _ => false,
        };
        if !is_type_info {
            self.parent.error(
                binding.value.span(),
                "reflected comptime type dispatch selector is not TypeInfo",
            );
            return None;
        }
        let mut arms = Vec::with_capacity(bindings.len());
        let mut bound_types = HashSet::new();
        for checked in bindings {
            let CheckedComptimeTypeSelection::ReflectedIteration(iteration_index) =
                checked.selection
            else {
                unreachable!("selection kind checked above");
            };
            // Several reflected elements may have the same concrete type.
            // Their source body is specialized by bound type, so one arm is
            // canonical and is selected independently for every matching
            // runtime element.
            if !bound_types.insert(checked.bound_type) {
                continue;
            }
            let Some(info) = self
                .parent
                .check
                .reflection_metadata
                .get_type_info_for_id(checked.bound_type)
            else {
                self.parent.error(
                    binding.span,
                    "reflected type dispatch has no checked bound-type metadata",
                );
                return None;
            };
            arms.push(ReflectedTypeArm {
                iteration_index,
                bound_type: checked.bound_type,
                canonical_identity: info.canonical_identity(),
                body: self.lower_block_with_checked_facts(&binding.body, checked.body),
            });
        }
        Some(StatementKind::ReflectedTypeDispatch { type_info, arms })
    }

    fn lower_block_with_checked_facts(
        &mut self,
        block: &ast::Block,
        facts: CheckedBodyFacts,
    ) -> Block {
        let CheckedBodyFacts {
            type_map,
            generic_calls,
            intrinsic_ids,
            intrinsic_type_arguments,
            intrinsic_reflection_arguments,
            call_argument_orders,
            method_calls,
            struct_constructions,
            pipeline_step_call_types,
            static_selections,
            comptime_type_bindings,
        } = facts;

        let saved_expression_types = std::mem::replace(&mut self.expression_types, type_map);
        let saved_generic_calls = std::mem::replace(&mut self.generic_calls, generic_calls);
        let saved_intrinsic_ids = std::mem::replace(&mut self.intrinsic_ids, intrinsic_ids);
        let saved_intrinsic_type_arguments =
            std::mem::replace(&mut self.intrinsic_type_arguments, intrinsic_type_arguments);
        let saved_intrinsic_reflection_arguments = std::mem::replace(
            &mut self.intrinsic_reflection_arguments,
            intrinsic_reflection_arguments,
        );
        let saved_call_argument_orders =
            std::mem::replace(&mut self.call_argument_orders, call_argument_orders);
        let saved_method_calls = std::mem::replace(&mut self.method_calls, method_calls);
        let saved_struct_constructions =
            std::mem::replace(&mut self.struct_constructions, struct_constructions);
        let saved_pipeline_step_call_types =
            std::mem::replace(&mut self.pipeline_step_call_types, pipeline_step_call_types);
        let saved_static_selections =
            std::mem::replace(&mut self.static_selections, static_selections);
        let saved_comptime_type_bindings =
            std::mem::replace(&mut self.comptime_type_bindings, comptime_type_bindings);
        let saved_consumed_static = std::mem::take(&mut self.consumed_static_selections);
        let saved_consumed_comptime = std::mem::take(&mut self.consumed_comptime_type_bindings);
        let saved_local_ids = self.local_ids.clone();

        let lowered = self.lower_block(block);
        self.reject_unconsumed_static_selections();
        self.reject_unconsumed_comptime_type_bindings();

        self.local_ids = saved_local_ids;
        self.expression_types = saved_expression_types;
        self.generic_calls = saved_generic_calls;
        self.intrinsic_ids = saved_intrinsic_ids;
        self.intrinsic_type_arguments = saved_intrinsic_type_arguments;
        self.intrinsic_reflection_arguments = saved_intrinsic_reflection_arguments;
        self.call_argument_orders = saved_call_argument_orders;
        self.method_calls = saved_method_calls;
        self.struct_constructions = saved_struct_constructions;
        self.pipeline_step_call_types = saved_pipeline_step_call_types;
        self.static_selections = saved_static_selections;
        self.comptime_type_bindings = saved_comptime_type_bindings;
        self.consumed_static_selections = saved_consumed_static;
        self.consumed_comptime_type_bindings = saved_consumed_comptime;
        lowered
    }

    fn allocate_declared_local(&mut self, name: &ast::Ident, mutable: bool) -> Option<LocalId> {
        let Some(definition) = self.parent.definition_at(name.span, DefKind::Variable) else {
            self.parent
                .error(name.span, "binding has no resolved definition");
            return None;
        };
        let Some(ty) = self
            .expression_types
            .get(&name.span)
            .copied()
            .or_else(|| self.parent.check.definition_types.get(&definition).copied())
        else {
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
                if let Some(local) = self.local_ids.get(&definition).copied() {
                    ExpressionKind::Local(local)
                } else if self.parent.resolve.scope_table.def(definition).kind == DefKind::Function
                {
                    ExpressionKind::FunctionRef(self.resolve_function_value_target(expression)?)
                } else {
                    self.parent.error(
                        span,
                        "identifier is neither a local nor a checked concrete function value",
                    );
                    return None;
                }
            }
            Expr::Binary(left, op @ (ast::BinOp::Eq | ast::BinOp::NotEq), right, _)
                if self.method_calls.contains_key(&span) =>
            {
                let function = self.resolve_user_call_target(left, span)?;
                let call = ExpressionKind::Call {
                    function,
                    args: vec![self.lower_expression(left)?, self.lower_expression(right)?],
                    evaluation_order: vec![0, 1],
                };
                if *op == ast::BinOp::NotEq {
                    ExpressionKind::Unary {
                        op: UnaryOp::Not,
                        value: Box::new(Expression {
                            kind: call,
                            ty,
                            span,
                        }),
                    }
                } else {
                    call
                }
            }
            Expr::Binary(left, op, right, _) => ExpressionKind::Binary {
                left: Box::new(self.lower_expression(left)?),
                op: lower_binary_op(*op),
                right: Box::new(self.lower_expression(right)?),
            },
            Expr::Unary(ast::UnaryOp::Neg, value, _) => match value.as_ref() {
                Expr::IntLiteral(value, _) => {
                    let Some(value) = value.checked_neg() else {
                        self.parent
                            .error(span, "negated integer literal exceeds the HIR value range");
                        return None;
                    };
                    ExpressionKind::Int(value)
                }
                Expr::FloatLiteral(value, _) => ExpressionKind::Float(-value),
                _ => ExpressionKind::Unary {
                    op: UnaryOp::Negate,
                    value: Box::new(self.lower_expression(value)?),
                },
            },
            Expr::Unary(op, value, _) => ExpressionKind::Unary {
                op: lower_unary_op(*op),
                value: Box::new(self.lower_expression(value)?),
            },
            Expr::FieldAccess(base, field, _) => {
                if self.resolved_expression_kind(expression) == Some(DefKind::Function) {
                    ExpressionKind::FunctionRef(self.resolve_function_value_target(expression)?)
                } else if self.enum_variant_index(ty, field).is_some() {
                    self.lower_enum_construct(ty, field, &[], span)?
                } else {
                    self.lower_field(base, field)?
                }
            }
            Expr::Call(callee, args, _) => self.lower_call(callee, args, span, false)?,
            Expr::GenericCall(callee, type_args, args, _) => {
                self.lower_call(callee, args, span, !type_args.is_empty())?
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
                let local_floor = self.locals.len() as u32;
                self.visible_bindings.push(HashMap::new());
                let mut lowered_params = Vec::with_capacity(params.len());
                let mut view_params = Vec::new();
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
                    let local = self.allocate_local(
                        definition,
                        &param.name.name,
                        param_type,
                        param.mutable,
                        param.span,
                    );
                    lowered_params.push(local);
                    if param.view {
                        view_params.push(local);
                    }
                }
                let body = self.lower_block(body);
                self.visible_bindings.pop();
                ExpressionKind::InlineFunction {
                    params: lowered_params,
                    view_params,
                    local_floor,
                    body,
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
        let actor_type = self.dotted_expression_name(callee)?;
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
                let mut predicates = Vec::new();
                let mut current = output_type;
                while let Type::Refinement { name, base } =
                    self.parent.check.interner.resolve(current)
                {
                    let Some(function) = self.parent.refinement_function_ids.get(&current).copied()
                    else {
                        self.parent
                            .error(span, "refinement has no checked predicate function");
                        return None;
                    };
                    predicates.push(RefinementPredicate {
                        refined_type: current,
                        type_name: name.clone(),
                        function,
                        input_type: {
                            let mut input = current;
                            while let Type::Refinement { base, .. } =
                                self.parent.check.interner.resolve(input)
                            {
                                input = *base;
                            }
                            if let Type::Secret(inner) = self.parent.check.interner.resolve(input) {
                                *inner
                            } else {
                                input
                            }
                        },
                    });
                    current = *base;
                }
                predicates.reverse();
                HandleKind::Refinement {
                    refined_type: output_type,
                    predicates,
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
        self.visible_bindings.push(HashMap::new());
        let error_local = if let Some(name) = error_name {
            let Some(definition) = self.parent.definition_at(name.span, DefKind::Variable) else {
                self.parent
                    .error(name.span, "handle error binding has no resolved definition");
                return None;
            };
            let ty = match self.parent.check.interner.resolve(target.ty) {
                Type::Result(_, error) => Some(*error),
                _ => self.parent.check.definition_types.get(&definition).copied(),
            };
            let Some(ty) = ty else {
                self.parent
                    .error(name.span, "handle error binding has no checked type");
                return None;
            };
            Some(self.allocate_local(definition, &name.name, ty, false, name.span))
        } else {
            None
        };
        let failure = self.lower_block(failure);
        self.visible_bindings.pop();
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
        has_explicit_type_arguments: bool,
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
        if self.is_declaration_reference(callee, DefKind::Bitfield) {
            return self.lower_bitfield_construct(args, call_span);
        }
        if let Expr::FieldAccess(base, member, _) = callee
            && member.name == "transition"
            && (self.resolved_expression_kind(base) == Some(DefKind::Machine)
                || self.resolved_expression_kind(callee) == Some(DefKind::Machine))
        {
            return self.lower_machine_transition(args, call_span);
        }
        if self.is_declaration_reference(callee, DefKind::Machine) {
            return self.lower_machine_construct(args, call_span);
        }
        if let Some(construction) = self.struct_constructions.get(&call_span).cloned() {
            let (fields, evaluation_order) =
                self.lower_arguments_in_parameter_order(args, call_span)?;
            return Some(ExpressionKind::StructConstruct {
                struct_type: construction.struct_type,
                fields,
                evaluation_order,
                validates_refinements: construction.validates_refinements,
            });
        }

        let (mut lowered_args, mut evaluation_order) =
            self.lower_arguments_in_parameter_order(args, call_span)?;
        if matches!(
            self.resolved_expression_kind(callee),
            Some(DefKind::Variable | DefKind::Param)
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
            let intrinsic = self.checked_intrinsic_id(call_span)?;
            let type_arguments =
                self.checked_intrinsic_type_arguments(call_span, has_explicit_type_arguments)?;
            let reflection_arguments = self
                .intrinsic_reflection_arguments
                .get(&call_span)
                .cloned()
                .unwrap_or_default();
            let raw_json_source = match intrinsic {
                IntrinsicId::JsonSerialize | IntrinsicId::JsonSerializePublic => {
                    Some("serialize_raw")
                }
                IntrinsicId::JsonParse | IntrinsicId::JsonParseExact => Some("parse_raw"),
                _ => None,
            };
            if let Some(name) = raw_json_source
                && type_arguments.len() == 1
                && lowered_args.len() == 1
                && self.is_raw_json_tree(type_arguments[0])
            {
                let Some(function) = self.trusted_stdlib_function("json", name) else {
                    self.parent
                        .error(call_span, "trusted raw JSON source is missing");
                    return None;
                };
                return Some(ExpressionKind::Call {
                    function,
                    args: lowered_args,
                    evaluation_order,
                });
            }
            if matches!(
                intrinsic,
                IntrinsicId::JsonParse | IntrinsicId::JsonParseExact
            ) && type_arguments.len() == 1
                && lowered_args.len() == 1
                && let Some(name) = self.native_json_primitive_parser(type_arguments[0])
            {
                let Some(function) = self.trusted_stdlib_function("json", name) else {
                    self.parent
                        .error(call_span, "trusted primitive JSON parser is missing");
                    return None;
                };
                return Some(ExpressionKind::Call {
                    function,
                    args: lowered_args,
                    evaluation_order,
                });
            }
            if matches!(
                intrinsic,
                IntrinsicId::JsonSerialize | IntrinsicId::JsonSerializePublic
            ) && type_arguments.len() == 1
                && lowered_args.len() == 1
                && let Some(kind) = self.lower_primitive_json_serialization(
                    type_arguments[0],
                    &lowered_args[0],
                    call_span,
                )
            {
                return Some(kind);
            }
            if matches!(
                intrinsic,
                IntrinsicId::TypeInfo
                    | IntrinsicId::TypeKindTag
                    | IntrinsicId::TypePrimitiveTag
                    | IntrinsicId::TypeFields
                    | IntrinsicId::TypeBitfieldFields
                    | IntrinsicId::TypeBitfieldLayout
                    | IntrinsicId::TypeMachineLayout
                    | IntrinsicId::TypeMachineStates
                    | IntrinsicId::TypeMachineTransitions
                    | IntrinsicId::TypeVariants
            ) {
                if !lowered_args.is_empty()
                    || type_arguments.len() != 1
                    || reflection_arguments.len() != 1
                {
                    self.parent.error(
                        call_span,
                        "reflection intrinsic has no checked type operand",
                    );
                    return None;
                }
                let ty = self.expression_types.get(&call_span).copied()?;
                let info = &reflection_arguments[0];
                return match intrinsic {
                    IntrinsicId::TypeInfo => self.lower_reflection_type_info(info, ty, call_span),
                    IntrinsicId::TypeKindTag => self
                        .reflected_enum_value(
                            ty,
                            ReflectionTypeInfo::kind_tag_variant(&info.kind),
                            call_span,
                        )
                        .map(|value| value.kind),
                    IntrinsicId::TypePrimitiveTag => self.lower_reflection_primitive_tag(
                        info.primitive_tag.as_deref(),
                        ty,
                        call_span,
                    ),
                    IntrinsicId::TypeFields => self.lower_reflection_type_fields(
                        type_arguments[0],
                        &info.type_name,
                        &info.kind,
                        ty,
                        call_span,
                    ),
                    IntrinsicId::TypeBitfieldFields => self.lower_reflection_bitfield_fields(
                        type_arguments[0],
                        &info.kind,
                        ty,
                        call_span,
                    ),
                    IntrinsicId::TypeBitfieldLayout => self.lower_reflection_bitfield_layout(
                        type_arguments[0],
                        &info.kind,
                        ty,
                        call_span,
                    ),
                    IntrinsicId::TypeVariants => self.lower_reflection_type_variants(
                        type_arguments[0],
                        &info.type_name,
                        &info.kind,
                        ty,
                        call_span,
                    ),
                    IntrinsicId::TypeMachineLayout => self.lower_reflection_machine_layout(
                        type_arguments[0],
                        &info.type_name,
                        &info.kind,
                        ty,
                        call_span,
                    ),
                    IntrinsicId::TypeMachineStates => self.lower_reflection_machine_states(
                        type_arguments[0],
                        &info.type_name,
                        &info.kind,
                        ty,
                        call_span,
                    ),
                    IntrinsicId::TypeMachineTransitions => self
                        .lower_reflection_machine_transitions(
                            type_arguments[0],
                            &info.kind,
                            ty,
                            call_span,
                        ),
                    _ => unreachable!(),
                };
            }
            if intrinsic == IntrinsicId::TypeVariantValue
                && type_arguments.len() == 1
                && lowered_args.len() == 1
                && reflection_arguments
                    .first()
                    .is_some_and(|info| info.kind == "enum")
            {
                let result_ty = self.expression_types.get(&call_span).copied()?;
                let info = &reflection_arguments[0];
                let variants = self
                    .parent
                    .check
                    .reflection_metadata
                    .get_type_variants_for_id(type_arguments[0])
                    .map(<[_]>::to_vec);
                let Some(variants) = variants else {
                    self.parent.error(
                        call_span,
                        "type.variant_value has no checked variant metadata",
                    );
                    return None;
                };
                for variant in &variants {
                    let kind = self.lower_reflection_type_variant(
                        variant,
                        &info.type_name,
                        result_ty,
                        call_span,
                    )?;
                    evaluation_order.push(lowered_args.len());
                    lowered_args.push(Expression {
                        kind,
                        ty: result_ty,
                        span: call_span,
                    });
                }
            }
            if intrinsic == IntrinsicId::TypeMachineStateValue
                && type_arguments.len() == 1
                && lowered_args.len() == 1
                && reflection_arguments
                    .first()
                    .is_some_and(|info| matches!(info.kind.as_str(), "machine" | "machine_state"))
            {
                let result_ty = self.expression_types.get(&call_span).copied()?;
                let machine = self.checked_reflection_machine(type_arguments[0], call_span)?;
                let owner_name = reflection_arguments[0]
                    .type_name
                    .split_once(" at ")
                    .map_or(reflection_arguments[0].type_name.as_str(), |(base, _)| base);
                for state in &machine.states {
                    let kind = self
                        .lower_reflection_machine_state(state, owner_name, result_ty, call_span)?;
                    evaluation_order.push(lowered_args.len());
                    lowered_args.push(Expression {
                        kind,
                        ty: result_ty,
                        span: call_span,
                    });
                }
            }
            if intrinsic == IntrinsicId::TypeMachineFieldValue
                && type_arguments.len() == 2
                && lowered_args.len() == 2
                && reflection_arguments
                    .first()
                    .is_some_and(|info| matches!(info.kind.as_str(), "machine" | "machine_state"))
            {
                let machine = self.checked_reflection_machine(type_arguments[0], call_span)?;
                let owner_name = reflection_arguments[0]
                    .type_name
                    .split_once(" at ")
                    .map_or(reflection_arguments[0].type_name.as_str(), |(base, _)| base);
                let field_ty = lowered_args[1].ty;
                for state in &machine.states {
                    for field in &state.fields {
                        let kind = self.lower_reflection_type_field(
                            field,
                            owner_name,
                            Some(&state.name),
                            field_ty,
                            call_span,
                        )?;
                        evaluation_order.push(lowered_args.len());
                        lowered_args.push(Expression {
                            kind,
                            ty: field_ty,
                            span: call_span,
                        });
                    }
                }
            }
            if intrinsic == IntrinsicId::TypeArg
                && type_arguments.len() == 1
                && lowered_args.len() == 1
                && reflection_arguments.len() == 1
            {
                let result_ty = self.expression_types.get(&call_span).copied()?;
                for arg_info in &reflection_arguments[0].args {
                    let kind = self.lower_reflection_type_info(arg_info, result_ty, call_span)?;
                    evaluation_order.push(lowered_args.len());
                    lowered_args.push(Expression {
                        kind,
                        ty: result_ty,
                        span: call_span,
                    });
                }
            }
            if intrinsic == IntrinsicId::TypeFieldValue
                && type_arguments.len() == 2
                && lowered_args.len() == 2
                && reflection_arguments
                    .first()
                    .is_some_and(|info| matches!(info.kind.as_str(), "struct" | "bitfield"))
            {
                let info = &reflection_arguments[0];
                let fields = self
                    .parent
                    .check
                    .reflection_metadata
                    .get_type_fields_for_id(type_arguments[0])
                    .map(<[_]>::to_vec);
                let Some(fields) = fields else {
                    self.parent
                        .error(call_span, "type.field_value has no checked field metadata");
                    return None;
                };
                let field_ty = lowered_args[1].ty;
                for field in &fields {
                    let kind = self.lower_reflection_type_field(
                        field,
                        &info.type_name,
                        None,
                        field_ty,
                        call_span,
                    )?;
                    evaluation_order.push(lowered_args.len());
                    lowered_args.push(Expression {
                        kind,
                        ty: field_ty,
                        span: call_span,
                    });
                }
            }
            if intrinsic == IntrinsicId::TypeVariantFieldValue
                && type_arguments.len() == 2
                && lowered_args.len() == 2
                && reflection_arguments
                    .first()
                    .is_some_and(|info| info.kind == "enum")
            {
                let info = &reflection_arguments[0];
                let variants = self
                    .parent
                    .check
                    .reflection_metadata
                    .get_type_variants_for_id(type_arguments[0])
                    .map(<[_]>::to_vec);
                let Some(variants) = variants else {
                    self.parent.error(
                        call_span,
                        "type.variant_field_value has no checked variant metadata",
                    );
                    return None;
                };
                let field_ty = lowered_args[1].ty;
                for variant in &variants {
                    for field in &variant.fields {
                        let kind = self.lower_reflection_type_field(
                            field,
                            &info.type_name,
                            Some(&variant.name),
                            field_ty,
                            call_span,
                        )?;
                        evaluation_order.push(lowered_args.len());
                        lowered_args.push(Expression {
                            kind,
                            ty: field_ty,
                            span: call_span,
                        });
                    }
                }
            }
            Some(ExpressionKind::Intrinsic {
                intrinsic,
                type_arguments,
                reflection_arguments,
                args: lowered_args,
                evaluation_order,
            })
        }
    }

    fn reflected_enum_value(
        &mut self,
        ty: TypeId,
        variant: &str,
        span: Span,
    ) -> Option<Expression> {
        let Type::Enum(enum_id) = self.parent.check.interner.resolve(ty) else {
            self.parent
                .error(span, "reflected metadata tag is not an enum");
            return None;
        };
        let Some(index) = self
            .parent
            .check
            .interner
            .resolve_enum(*enum_id)
            .variants
            .iter()
            .position(|candidate| candidate.name == variant && candidate.fields.is_empty())
        else {
            self.parent
                .error(span, "reflected metadata tag has no checked variant");
            return None;
        };
        Some(Expression {
            kind: ExpressionKind::EnumConstruct {
                enum_type: ty,
                variant: VariantId(index as u32),
                payloads: Vec::new(),
            },
            ty,
            span,
        })
    }

    fn lower_reflection_primitive_tag(
        &mut self,
        variant: Option<&str>,
        optional_ty: TypeId,
        span: Span,
    ) -> Option<ExpressionKind> {
        let Type::Optional(primitive_ty) = self.parent.check.interner.resolve(optional_ty) else {
            self.parent
                .error(span, "reflected primitive tag is not optional");
            return None;
        };
        let primitive_ty = *primitive_ty;
        match variant {
            Some(variant) => Some(ExpressionKind::OptionalSome(Box::new(
                self.reflected_enum_value(primitive_ty, variant, span)?,
            ))),
            None => Some(ExpressionKind::OptionalNone),
        }
    }

    fn lower_reflection_type_info(
        &mut self,
        info: &ReflectionTypeInfo,
        ty: TypeId,
        span: Span,
    ) -> Option<ExpressionKind> {
        let Type::Struct(struct_id) = self.parent.check.interner.resolve(ty) else {
            self.parent.error(span, "type.info result is not TypeInfo");
            return None;
        };
        let definition = self.parent.check.interner.resolve_struct(*struct_id);
        let expected = [
            "type_name",
            "kind",
            "kind_tag",
            "primitive_tag",
            "has_secret",
            "args",
        ];
        if definition.name != "TypeInfo"
            || !definition
                .fields
                .iter()
                .map(|(name, _)| name.as_str())
                .eq(expected)
        {
            self.parent
                .error(span, "type.info has no checked TypeInfo layout");
            return None;
        }
        let field_types = definition
            .fields
            .iter()
            .map(|(_, field_ty)| *field_ty)
            .collect::<Vec<_>>();
        let Type::List(arg_type) = self.parent.check.interner.resolve(field_types[5]) else {
            self.parent.error(span, "TypeInfo arguments are not a list");
            return None;
        };
        let arg_type = *arg_type;
        if arg_type != ty {
            self.parent
                .error(span, "TypeInfo argument type is inconsistent");
            return None;
        }
        let string_field = |value: &str| Expression {
            kind: ExpressionKind::String(value.to_string()),
            ty: field_types[0],
            span,
        };
        let kind_tag = self.reflected_enum_value(
            field_types[2],
            ReflectionTypeInfo::kind_tag_variant(&info.kind),
            span,
        )?;
        let primitive_tag = self.lower_reflection_primitive_tag(
            info.primitive_tag.as_deref(),
            field_types[3],
            span,
        )?;
        let args = info
            .args
            .iter()
            .map(|arg| {
                Some(Expression {
                    kind: self.lower_reflection_type_info(arg, arg_type, span)?,
                    ty: arg_type,
                    span,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let fields = vec![
            string_field(&info.type_name),
            Expression {
                kind: ExpressionKind::String(info.kind.clone()),
                ty: field_types[1],
                span,
            },
            kind_tag,
            Expression {
                kind: primitive_tag,
                ty: field_types[3],
                span,
            },
            Expression {
                kind: ExpressionKind::Bool(info.has_secret),
                ty: field_types[4],
                span,
            },
            Expression {
                kind: ExpressionKind::ListConstruct { elements: args },
                ty: field_types[5],
                span,
            },
        ];
        Some(ExpressionKind::StructConstruct {
            struct_type: ty,
            fields,
            evaluation_order: (0..expected.len()).collect(),
            validates_refinements: false,
        })
    }

    fn lower_reflection_type_fields(
        &mut self,
        owner_ty: TypeId,
        owner_name: &str,
        owner_kind: &str,
        list_ty: TypeId,
        span: Span,
    ) -> Option<ExpressionKind> {
        let Type::List(field_ty) = self.parent.check.interner.resolve(list_ty) else {
            self.parent.error(span, "type.fields result is not a list");
            return None;
        };
        let field_ty = *field_ty;
        if !matches!(owner_kind, "struct" | "bitfield") {
            return Some(ExpressionKind::ListConstruct {
                elements: Vec::new(),
            });
        }
        let fields = self
            .parent
            .check
            .reflection_metadata
            .get_type_fields_for_id(owner_ty)
            .map(<[_]>::to_vec);
        let fields = match fields {
            Some(fields) => fields,
            None if matches!(
                self.parent.check.interner.resolve(owner_ty),
                Type::Struct(_) | Type::Bitfield(_)
            ) =>
            {
                self.parent
                    .error(span, "type.fields has no checked field metadata");
                return None;
            }
            None => Vec::new(),
        };
        let elements = fields
            .iter()
            .map(|field| {
                Some(Expression {
                    kind: self
                        .lower_reflection_type_field(field, owner_name, None, field_ty, span)?,
                    ty: field_ty,
                    span,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(ExpressionKind::ListConstruct { elements })
    }

    fn lower_reflection_type_field(
        &mut self,
        field: &ReflectionFieldInfo,
        owner_name: &str,
        owner_member: Option<&str>,
        ty: TypeId,
        span: Span,
    ) -> Option<ExpressionKind> {
        let Type::Struct(struct_id) = self.parent.check.interner.resolve(ty) else {
            self.parent
                .error(span, "type.fields element is not TypeField");
            return None;
        };
        let definition = self.parent.check.interner.resolve_struct(*struct_id);
        let expected = [
            "index",
            "owner_type",
            "owner_member",
            "name",
            "type_name",
            "kind",
            "kind_tag",
            "serialize_name",
            "has_secret",
            "type_info",
        ];
        if definition.name != "TypeField"
            || !definition
                .fields
                .iter()
                .map(|(name, _)| name.as_str())
                .eq(expected)
        {
            self.parent
                .error(span, "type.fields has no checked TypeField layout");
            return None;
        }
        let field_types = definition
            .fields
            .iter()
            .map(|(_, field_ty)| *field_ty)
            .collect::<Vec<_>>();
        let Ok(index) = i128::try_from(field.index) else {
            self.parent
                .error(span, "reflected field index is too large");
            return None;
        };
        let string_field = |value: &str, ty| Expression {
            kind: ExpressionKind::String(value.to_string()),
            ty,
            span,
        };
        let kind_tag = self.reflected_enum_value(
            field_types[6],
            ReflectionTypeInfo::kind_tag_variant(&field.kind),
            span,
        )?;
        let type_info = self.lower_reflection_type_info(&field.type_info, field_types[9], span)?;
        let owner_member = match owner_member {
            Some(member) => {
                let Type::Optional(string_ty) = self.parent.check.interner.resolve(field_types[2])
                else {
                    self.parent
                        .error(span, "reflected owner member is not optional string");
                    return None;
                };
                ExpressionKind::OptionalSome(Box::new(string_field(member, *string_ty)))
            }
            None => ExpressionKind::OptionalNone,
        };
        let fields = vec![
            Expression {
                kind: ExpressionKind::Int(index),
                ty: field_types[0],
                span,
            },
            string_field(owner_name, field_types[1]),
            Expression {
                kind: owner_member,
                ty: field_types[2],
                span,
            },
            string_field(&field.name, field_types[3]),
            string_field(&field.type_name, field_types[4]),
            string_field(&field.kind, field_types[5]),
            kind_tag,
            string_field(&field.serialize_name, field_types[7]),
            Expression {
                kind: ExpressionKind::Bool(field.has_secret),
                ty: field_types[8],
                span,
            },
            Expression {
                kind: type_info,
                ty: field_types[9],
                span,
            },
        ];
        Some(ExpressionKind::StructConstruct {
            struct_type: ty,
            fields,
            evaluation_order: (0..expected.len()).collect(),
            validates_refinements: false,
        })
    }

    fn lower_reflection_type_variants(
        &mut self,
        owner_ty: TypeId,
        owner_name: &str,
        owner_kind: &str,
        list_ty: TypeId,
        span: Span,
    ) -> Option<ExpressionKind> {
        let Type::List(variant_ty) = self.parent.check.interner.resolve(list_ty) else {
            self.parent
                .error(span, "type.variants result is not a list");
            return None;
        };
        let variant_ty = *variant_ty;
        if owner_kind != "enum" {
            return Some(ExpressionKind::ListConstruct {
                elements: Vec::new(),
            });
        }
        let Some(variants) = self
            .parent
            .check
            .reflection_metadata
            .get_type_variants_for_id(owner_ty)
            .map(<[_]>::to_vec)
        else {
            self.parent
                .error(span, "type.variants has no checked variant metadata");
            return None;
        };
        let elements = variants
            .iter()
            .map(|variant| {
                Some(Expression {
                    kind: self
                        .lower_reflection_type_variant(variant, owner_name, variant_ty, span)?,
                    ty: variant_ty,
                    span,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(ExpressionKind::ListConstruct { elements })
    }

    fn lower_reflection_type_variant(
        &mut self,
        variant: &ReflectionVariantInfo,
        owner_name: &str,
        ty: TypeId,
        span: Span,
    ) -> Option<ExpressionKind> {
        let Type::Struct(struct_id) = self.parent.check.interner.resolve(ty) else {
            self.parent
                .error(span, "type.variants element is not TypeVariant");
            return None;
        };
        let definition = self.parent.check.interner.resolve_struct(*struct_id);
        let expected = [
            "index",
            "owner_type",
            "name",
            "discriminant",
            "has_secret",
            "fields",
        ];
        if definition.name != "TypeVariant"
            || !definition
                .fields
                .iter()
                .map(|(name, _)| name.as_str())
                .eq(expected)
        {
            self.parent
                .error(span, "type.variants has no checked TypeVariant layout");
            return None;
        }
        let field_types = definition
            .fields
            .iter()
            .map(|(_, field_ty)| *field_ty)
            .collect::<Vec<_>>();
        let Type::List(field_ty) = self.parent.check.interner.resolve(field_types[5]) else {
            self.parent.error(span, "TypeVariant fields are not a list");
            return None;
        };
        let field_ty = *field_ty;
        let Ok(index) = i128::try_from(variant.index) else {
            self.parent
                .error(span, "reflected variant index is too large");
            return None;
        };
        let field_values = variant
            .fields
            .iter()
            .map(|field| {
                Some(Expression {
                    kind: self.lower_reflection_type_field(
                        field,
                        owner_name,
                        Some(&variant.name),
                        field_ty,
                        span,
                    )?,
                    ty: field_ty,
                    span,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let fields = vec![
            Expression {
                kind: ExpressionKind::Int(index),
                ty: field_types[0],
                span,
            },
            Expression {
                kind: ExpressionKind::String(owner_name.to_string()),
                ty: field_types[1],
                span,
            },
            Expression {
                kind: ExpressionKind::String(variant.name.clone()),
                ty: field_types[2],
                span,
            },
            Expression {
                kind: ExpressionKind::Int(variant.discriminant.into()),
                ty: field_types[3],
                span,
            },
            Expression {
                kind: ExpressionKind::Bool(variant.has_secret),
                ty: field_types[4],
                span,
            },
            Expression {
                kind: ExpressionKind::ListConstruct {
                    elements: field_values,
                },
                ty: field_types[5],
                span,
            },
        ];
        Some(ExpressionKind::StructConstruct {
            struct_type: ty,
            fields,
            evaluation_order: (0..expected.len()).collect(),
            validates_refinements: false,
        })
    }

    fn checked_reflection_machine(
        &mut self,
        owner_ty: TypeId,
        span: Span,
    ) -> Option<ReflectionMachineInfo> {
        let machine_ty = match self.parent.check.interner.resolve(owner_ty) {
            Type::Machine(_) => Some(owner_ty),
            Type::MachineState { machine, .. } => self
                .parent
                .check
                .interner
                .type_ids()
                .find(|ty| matches!(self.parent.check.interner.resolve(*ty), Type::Machine(id) if id == machine)),
            _ => None,
        };
        machine_ty
            .and_then(|machine_ty| {
                self.parent
                    .check
                    .reflection_metadata
                    .get_machine_for_id(machine_ty)
                    .cloned()
            })
            .or_else(|| {
                self.parent
                    .error(span, "machine reflection has no checked metadata");
                None
            })
    }

    fn reflected_machine_info(
        &mut self,
        owner_ty: TypeId,
        owner_kind: &str,
        span: Span,
    ) -> Option<ReflectionMachineInfo> {
        if matches!(owner_kind, "machine" | "machine_state") {
            self.checked_reflection_machine(owner_ty, span)
        } else {
            Some(ReflectionMachineInfo::new(Vec::new(), Vec::new()))
        }
    }

    fn lower_reflection_machine_layout(
        &mut self,
        owner_ty: TypeId,
        owner_name: &str,
        owner_kind: &str,
        ty: TypeId,
        span: Span,
    ) -> Option<ExpressionKind> {
        let Type::Struct(struct_id) = self.parent.check.interner.resolve(ty) else {
            self.parent
                .error(span, "type.machine_layout result is not TypeMachine");
            return None;
        };
        let definition = self.parent.check.interner.resolve_struct(*struct_id);
        let expected = ["states", "edges"];
        if definition.name != "TypeMachine"
            || !definition
                .fields
                .iter()
                .map(|(name, _)| name.as_str())
                .eq(expected)
        {
            self.parent.error(
                span,
                "type.machine_layout has no checked TypeMachine layout",
            );
            return None;
        }
        let states_ty = definition.fields[0].1;
        let edges_ty = definition.fields[1].1;
        let fields = vec![
            Expression {
                kind: self.lower_reflection_machine_states(
                    owner_ty, owner_name, owner_kind, states_ty, span,
                )?,
                ty: states_ty,
                span,
            },
            Expression {
                kind: self
                    .lower_reflection_machine_transitions(owner_ty, owner_kind, edges_ty, span)?,
                ty: edges_ty,
                span,
            },
        ];
        Some(ExpressionKind::StructConstruct {
            struct_type: ty,
            fields,
            evaluation_order: vec![0, 1],
            validates_refinements: false,
        })
    }

    fn lower_reflection_machine_states(
        &mut self,
        owner_ty: TypeId,
        owner_name: &str,
        owner_kind: &str,
        list_ty: TypeId,
        span: Span,
    ) -> Option<ExpressionKind> {
        let Type::List(state_ty) = self.parent.check.interner.resolve(list_ty) else {
            self.parent
                .error(span, "type.machine_states result is not a list");
            return None;
        };
        let state_ty = *state_ty;
        let machine = self.reflected_machine_info(owner_ty, owner_kind, span)?;
        let owner_name = owner_name
            .split_once(" at ")
            .map_or(owner_name, |(base, _)| base);
        let elements = machine
            .states
            .iter()
            .map(|state| {
                Some(Expression {
                    kind: self.lower_reflection_machine_state(state, owner_name, state_ty, span)?,
                    ty: state_ty,
                    span,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(ExpressionKind::ListConstruct { elements })
    }

    fn lower_reflection_machine_transitions(
        &mut self,
        owner_ty: TypeId,
        owner_kind: &str,
        list_ty: TypeId,
        span: Span,
    ) -> Option<ExpressionKind> {
        let Type::List(edge_ty) = self.parent.check.interner.resolve(list_ty) else {
            self.parent
                .error(span, "type.machine_transitions result is not a list");
            return None;
        };
        let edge_ty = *edge_ty;
        let machine = self.reflected_machine_info(owner_ty, owner_kind, span)?;
        let elements = machine
            .edges
            .iter()
            .map(|edge| {
                Some(Expression {
                    kind: self.lower_reflection_machine_transition(edge, edge_ty, span)?,
                    ty: edge_ty,
                    span,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(ExpressionKind::ListConstruct { elements })
    }

    fn lower_reflection_machine_transition(
        &mut self,
        edge: &ReflectionMachineTransitionInfo,
        ty: TypeId,
        span: Span,
    ) -> Option<ExpressionKind> {
        let Type::Struct(struct_id) = self.parent.check.interner.resolve(ty) else {
            self.parent.error(
                span,
                "type.machine_transitions element is not TypeMachineTransition",
            );
            return None;
        };
        let definition = self.parent.check.interner.resolve_struct(*struct_id);
        let expected = ["index", "source_index", "source", "target_index", "target"];
        if definition.name != "TypeMachineTransition"
            || !definition
                .fields
                .iter()
                .map(|(name, _)| name.as_str())
                .eq(expected)
        {
            self.parent.error(
                span,
                "type.machine_transitions has no checked TypeMachineTransition layout",
            );
            return None;
        }
        let mut fields = Vec::with_capacity(5);
        for (position, value) in [
            (0, edge.index),
            (1, edge.source_index),
            (3, edge.target_index),
        ] {
            let Ok(value) = i128::try_from(value) else {
                self.parent
                    .error(span, "reflected machine transition index is too large");
                return None;
            };
            fields.push((
                position,
                Expression {
                    kind: ExpressionKind::Int(value),
                    ty: definition.fields[position].1,
                    span,
                },
            ));
        }
        for (position, value) in [(2, &edge.source), (4, &edge.target)] {
            fields.push((
                position,
                Expression {
                    kind: ExpressionKind::String(value.clone()),
                    ty: definition.fields[position].1,
                    span,
                },
            ));
        }
        fields.sort_by_key(|(position, _)| *position);
        Some(ExpressionKind::StructConstruct {
            struct_type: ty,
            fields: fields.into_iter().map(|(_, field)| field).collect(),
            evaluation_order: (0..expected.len()).collect(),
            validates_refinements: false,
        })
    }

    fn lower_reflection_machine_state(
        &mut self,
        state: &ReflectionMachineStateInfo,
        owner_name: &str,
        ty: TypeId,
        span: Span,
    ) -> Option<ExpressionKind> {
        let Type::Struct(struct_id) = self.parent.check.interner.resolve(ty) else {
            self.parent.error(
                span,
                "type.machine_state_value result is not TypeMachineState",
            );
            return None;
        };
        let definition = self.parent.check.interner.resolve_struct(*struct_id);
        let expected = ["index", "owner_type", "name", "has_secret", "fields"];
        if definition.name != "TypeMachineState"
            || !definition
                .fields
                .iter()
                .map(|(name, _)| name.as_str())
                .eq(expected)
        {
            self.parent.error(
                span,
                "type.machine_state_value has no checked TypeMachineState layout",
            );
            return None;
        }
        let field_types = definition
            .fields
            .iter()
            .map(|(_, field_ty)| *field_ty)
            .collect::<Vec<_>>();
        let Type::List(field_ty) = self.parent.check.interner.resolve(field_types[4]) else {
            self.parent
                .error(span, "TypeMachineState fields are not a list");
            return None;
        };
        let field_ty = *field_ty;
        let Ok(index) = i128::try_from(state.index) else {
            self.parent
                .error(span, "reflected machine state index is too large");
            return None;
        };
        let field_values = state
            .fields
            .iter()
            .map(|field| {
                Some(Expression {
                    kind: self.lower_reflection_type_field(
                        field,
                        owner_name,
                        Some(&state.name),
                        field_ty,
                        span,
                    )?,
                    ty: field_ty,
                    span,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let fields = vec![
            Expression {
                kind: ExpressionKind::Int(index),
                ty: field_types[0],
                span,
            },
            Expression {
                kind: ExpressionKind::String(owner_name.to_string()),
                ty: field_types[1],
                span,
            },
            Expression {
                kind: ExpressionKind::String(state.name.clone()),
                ty: field_types[2],
                span,
            },
            Expression {
                kind: ExpressionKind::Bool(state.has_secret),
                ty: field_types[3],
                span,
            },
            Expression {
                kind: ExpressionKind::ListConstruct {
                    elements: field_values,
                },
                ty: field_types[4],
                span,
            },
        ];
        Some(ExpressionKind::StructConstruct {
            struct_type: ty,
            fields,
            evaluation_order: (0..expected.len()).collect(),
            validates_refinements: false,
        })
    }

    fn checked_reflection_bitfield(
        &mut self,
        owner_ty: TypeId,
        owner_kind: &str,
        span: Span,
    ) -> Option<ReflectionBitfieldInfo> {
        if owner_kind != "bitfield" {
            return Some(ReflectionBitfieldInfo::new(false, Vec::new()));
        }
        self.parent
            .check
            .reflection_metadata
            .get_bitfield_for_id(owner_ty)
            .cloned()
            .or_else(|| {
                self.parent
                    .error(span, "bitfield reflection has no checked layout metadata");
                None
            })
    }

    fn lower_reflection_bitfield_fields(
        &mut self,
        owner_ty: TypeId,
        owner_kind: &str,
        list_ty: TypeId,
        span: Span,
    ) -> Option<ExpressionKind> {
        let Type::List(field_ty) = self.parent.check.interner.resolve(list_ty) else {
            self.parent
                .error(span, "type.bitfield_fields result is not a list");
            return None;
        };
        let field_ty = *field_ty;
        let bitfield = self.checked_reflection_bitfield(owner_ty, owner_kind, span)?;
        let elements = bitfield
            .fields
            .iter()
            .map(|field| {
                Some(Expression {
                    kind: self.lower_reflection_bitfield_field(field, field_ty, span)?,
                    ty: field_ty,
                    span,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(ExpressionKind::ListConstruct { elements })
    }

    fn lower_reflection_bitfield_layout(
        &mut self,
        owner_ty: TypeId,
        owner_kind: &str,
        ty: TypeId,
        span: Span,
    ) -> Option<ExpressionKind> {
        let Type::Struct(struct_id) = self.parent.check.interner.resolve(ty) else {
            self.parent
                .error(span, "type.bitfield_layout result is not TypeBitfield");
            return None;
        };
        let definition = self.parent.check.interner.resolve_struct(*struct_id);
        let expected = ["network_order", "fields"];
        if definition.name != "TypeBitfield"
            || !definition
                .fields
                .iter()
                .map(|(name, _)| name.as_str())
                .eq(expected)
        {
            self.parent.error(
                span,
                "type.bitfield_layout has no checked TypeBitfield layout",
            );
            return None;
        }
        let field_types = definition
            .fields
            .iter()
            .map(|(_, field_ty)| *field_ty)
            .collect::<Vec<_>>();
        let bitfield = self.checked_reflection_bitfield(owner_ty, owner_kind, span)?;
        let fields = vec![
            Expression {
                kind: ExpressionKind::Bool(bitfield.network_order),
                ty: field_types[0],
                span,
            },
            Expression {
                kind: self.lower_reflection_bitfield_fields(
                    owner_ty,
                    owner_kind,
                    field_types[1],
                    span,
                )?,
                ty: field_types[1],
                span,
            },
        ];
        Some(ExpressionKind::StructConstruct {
            struct_type: ty,
            fields,
            evaluation_order: (0..expected.len()).collect(),
            validates_refinements: false,
        })
    }

    fn lower_reflection_bitfield_field(
        &mut self,
        field: &ReflectionBitfieldFieldInfo,
        ty: TypeId,
        span: Span,
    ) -> Option<ExpressionKind> {
        let Type::Struct(struct_id) = self.parent.check.interner.resolve(ty) else {
            self.parent
                .error(span, "bitfield reflection element is not TypeBitfieldField");
            return None;
        };
        let definition = self.parent.check.interner.resolve_struct(*struct_id);
        let expected = [
            "index",
            "name",
            "shape",
            "shape_tag",
            "width",
            "type_info",
            "enum_type",
        ];
        if definition.name != "TypeBitfieldField"
            || !definition
                .fields
                .iter()
                .map(|(name, _)| name.as_str())
                .eq(expected)
        {
            self.parent.error(
                span,
                "bitfield reflection has no checked TypeBitfieldField layout",
            );
            return None;
        }
        let field_types = definition
            .fields
            .iter()
            .map(|(_, field_ty)| *field_ty)
            .collect::<Vec<_>>();
        let Ok(index) = i128::try_from(field.index) else {
            self.parent
                .error(span, "reflected bitfield field index is too large");
            return None;
        };
        let shape_variant = if field.shape == "payload" {
            "payload_field"
        } else {
            "bits_field"
        };
        let shape_tag = self.reflected_enum_value(field_types[3], shape_variant, span)?;
        let type_info = self.lower_reflection_type_info(&field.type_info, field_types[5], span)?;
        let enum_type = match &field.enum_type {
            Some(info) => {
                let Type::Optional(info_ty) = self.parent.check.interner.resolve(field_types[6])
                else {
                    self.parent
                        .error(span, "bitfield enum metadata is not optional TypeInfo");
                    return None;
                };
                let info_ty = *info_ty;
                let value = Expression {
                    kind: self.lower_reflection_type_info(info, info_ty, span)?,
                    ty: info_ty,
                    span,
                };
                ExpressionKind::OptionalSome(Box::new(value))
            }
            None => ExpressionKind::OptionalNone,
        };
        let fields = vec![
            Expression {
                kind: ExpressionKind::Int(index),
                ty: field_types[0],
                span,
            },
            Expression {
                kind: ExpressionKind::String(field.name.clone()),
                ty: field_types[1],
                span,
            },
            Expression {
                kind: ExpressionKind::String(field.shape.clone()),
                ty: field_types[2],
                span,
            },
            shape_tag,
            Expression {
                kind: ExpressionKind::Int(field.width.into()),
                ty: field_types[4],
                span,
            },
            Expression {
                kind: type_info,
                ty: field_types[5],
                span,
            },
            Expression {
                kind: enum_type,
                ty: field_types[6],
                span,
            },
        ];
        Some(ExpressionKind::StructConstruct {
            struct_type: ty,
            fields,
            evaluation_order: (0..expected.len()).collect(),
            validates_refinements: false,
        })
    }

    fn checked_intrinsic_type_arguments(
        &mut self,
        span: Span,
        required: bool,
    ) -> Option<Vec<TypeId>> {
        if let Some(arguments) = self.intrinsic_type_arguments.get(&span) {
            return Some(arguments.clone());
        }
        if required {
            self.parent.error(
                span,
                "generic compiler intrinsic has no checked concrete type operands",
            );
            None
        } else {
            Some(Vec::new())
        }
    }

    fn checked_intrinsic_id(&mut self, span: Span) -> Option<IntrinsicId> {
        self.intrinsic_ids.get(&span).copied().or_else(|| {
            self.parent.error(
                span,
                "checked compiler intrinsic has no closed intrinsic identity",
            );
            None
        })
    }

    fn dotted_expression_name(&mut self, expression: &Expr) -> Option<String> {
        fn dotted(expression: &Expr) -> Option<String> {
            match expression {
                Expr::Ident(ident) => Some(ident.name.clone()),
                Expr::FieldAccess(base, field, _) => {
                    Some(format!("{}.{}", dotted(base)?, field.name))
                }
                _ => None,
            }
        }
        dotted(expression).or_else(|| {
            self.parent
                .error(expression.span(), "expression has no canonical dotted name");
            None
        })
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

    fn resolved_definition(&self, expression: &Expr) -> Option<DefId> {
        self.parent
            .resolve
            .resolutions
            .get(&expression.span())
            .copied()
    }

    fn resolved_expression_kind(&self, expression: &Expr) -> Option<DefKind> {
        self.resolved_definition(expression)
            .map(|definition| self.parent.resolve.scope_table.def(definition).kind)
    }

    fn is_declaration_reference(&self, expression: &Expr, kind: DefKind) -> bool {
        let Some(definition) = self.resolved_definition(expression) else {
            return false;
        };
        let info = self.parent.resolve.scope_table.def(definition);
        if info.kind != kind {
            return false;
        }
        match expression {
            Expr::Ident(_) => true,
            Expr::FieldAccess(_, member, _) => info
                .name
                .rsplit('.')
                .next()
                .is_some_and(|name| name == member.name),
            _ => false,
        }
    }

    fn is_source_call(&self, callee: &Expr, span: Span) -> bool {
        if self.method_calls.contains_key(&span) {
            return true;
        }
        let Some(definition) = self.resolved_definition(callee) else {
            return false;
        };
        if self.parent.resolve.scope_table.def(definition).kind != DefKind::Function {
            return false;
        }
        if self
            .generic_calls
            .get(&span)
            .is_some_and(|generic| generic.definition != definition)
        {
            return true;
        }
        let key = self.generic_calls.get(&span).map_or_else(
            || FunctionKey::Definition {
                definition,
                concrete_args: Vec::new(),
                specialization: CheckedGenericSpecialization::default(),
            },
            |generic| FunctionKey::Definition {
                definition,
                concrete_args: generic.concrete_args.clone(),
                specialization: generic.specialization.clone(),
            },
        );
        self.function_ids.contains_key(&key) || !self.is_trusted_stdlib_intrinsic(definition, span)
    }

    fn is_trusted_stdlib_intrinsic(&self, definition: DefId, span: Span) -> bool {
        let info = self.parent.resolve.scope_table.def(definition);
        self.parent.origins.get(&info.span.file) == Some(&SourceOrigin::Stdlib)
            && self.intrinsic_ids.contains_key(&span)
    }

    fn is_raw_json_tree(&self, ty: TypeId) -> bool {
        matches!(self.parent.check.interner.resolve(ty), Type::Enum(id) if self.parent.check.interner.resolve_enum(*id).name == "json.JsonTree")
    }

    fn native_json_primitive_parser(&self, ty: TypeId) -> Option<&'static str> {
        match self.parent.check.interner.resolve(ty) {
            Type::String => Some("json_parse_native_string"),
            Type::Bool => Some("json_parse_native_bool"),
            Type::Int64 => Some("json_parse_native_int64"),
            Type::Uint64 => Some("json_parse_native_uint64"),
            Type::Float64 => Some("json_parse_native_float64"),
            Type::Bytes => Some("json_parse_native_bytes"),
            Type::Nothing => Some("json_parse_native_nothing"),
            _ => None,
        }
    }

    fn lower_primitive_json_serialization(
        &mut self,
        ty: TypeId,
        value: &Expression,
        span: Span,
    ) -> Option<ExpressionKind> {
        let (variant_name, payload) = match self.parent.check.interner.resolve(ty) {
            Type::String => (
                "string_value",
                Some(Expression {
                    kind: ExpressionKind::Clone(Box::new(value.clone())),
                    ty,
                    span,
                }),
            ),
            Type::Bool => ("bool_value", Some(value.clone())),
            Type::Nothing if matches!(value.kind, ExpressionKind::Nothing) => ("null", None),
            Type::Int8
            | Type::Int16
            | Type::Int32
            | Type::Int64
            | Type::Uint8
            | Type::Uint16
            | Type::Uint32
            | Type::Uint64
            | Type::Float32
            | Type::Float64 => (
                "number_value",
                Some(Expression {
                    kind: ExpressionKind::StringInterpolation(vec![StringSegment::Value(
                        value.clone(),
                    )]),
                    ty: TypeInterner::STRING,
                    span,
                }),
            ),
            _ => return None,
        };
        let tree_type = self
            .parent
            .check
            .interner
            .type_ids()
            .find(|candidate| self.is_raw_json_tree(*candidate))?;
        let Type::Enum(enum_id) = self.parent.check.interner.resolve(tree_type) else {
            return None;
        };
        let variant = self
            .parent
            .check
            .interner
            .resolve_enum(*enum_id)
            .variants
            .iter()
            .position(|candidate| candidate.name == variant_name)?;
        let function = self.trusted_stdlib_function("json", "serialize_raw")?;
        let tree = Expression {
            kind: ExpressionKind::EnumConstruct {
                enum_type: tree_type,
                variant: VariantId(variant as u32),
                payloads: payload.into_iter().collect(),
            },
            ty: tree_type,
            span,
        };
        Some(ExpressionKind::Call {
            function,
            args: vec![Expression {
                kind: ExpressionKind::View(Box::new(tree)),
                ty: tree_type,
                span,
            }],
            evaluation_order: vec![0],
        })
    }

    fn trusted_stdlib_function(&self, namespace: &str, name: &str) -> Option<FunctionId> {
        self.parent.module.items.iter().find_map(|item| {
            let Item::Function(function) = item else {
                return None;
            };
            if function.name.name != name || !function.type_params.is_empty() {
                return None;
            }
            let definition = self
                .parent
                .definition_at(function.name.span, DefKind::Function)?;
            let declared = self.parent.resolve.scope_table.def(definition);
            if declared.namespace.as_deref() != Some(namespace)
                || self.parent.origins.get(&function.name.span.file) != Some(&SourceOrigin::Stdlib)
            {
                return None;
            }
            self.function_ids
                .get(&FunctionKey::Definition {
                    definition,
                    concrete_args: Vec::new(),
                    specialization: CheckedGenericSpecialization::default(),
                })
                .copied()
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
            self.visible_bindings.push(HashMap::new());
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
            self.visible_bindings.pop();
            arms.push(MatchArm {
                variant,
                bindings,
                body,
                span: arm.span,
            });
        }
        Some(StatementKind::Match { scrutinee, arms })
    }

    fn resolve_function_value_target(&mut self, expression: &Expr) -> Option<FunctionId> {
        let Some(definition) = self.resolved_definition(expression) else {
            self.parent.error(
                expression.span(),
                "function value has no resolved definition",
            );
            return None;
        };
        let key = FunctionKey::Definition {
            definition,
            concrete_args: Vec::new(),
            specialization: CheckedGenericSpecialization::default(),
        };
        self.function_ids.get(&key).copied().or_else(|| {
            self.parent.error(
                expression.span(),
                "function value has no checked concrete HIR function",
            );
            None
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

        let Some(definition) = self.resolved_definition(callee) else {
            self.parent
                .error(callee.span(), "call target has no resolved definition");
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
                specialization: generic.specialization.clone(),
            }
        } else {
            FunctionKey::Definition {
                definition,
                concrete_args: Vec::new(),
                specialization: CheckedGenericSpecialization::default(),
            }
        };
        let Some(function) = self.function_ids.get(&key).copied() else {
            self.parent.error(
                callee.span(),
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
            let has_explicit_type_arguments = Self::has_explicit_type_arguments(&step.function);
            ExpressionKind::Intrinsic {
                intrinsic: self.checked_intrinsic_id(step.span)?,
                type_arguments: self
                    .checked_intrinsic_type_arguments(step.span, has_explicit_type_arguments)?,
                reflection_arguments: self
                    .intrinsic_reflection_arguments
                    .get(&step.span)
                    .cloned()
                    .unwrap_or_default(),
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

    fn has_explicit_type_arguments(expression: &Expr) -> bool {
        match expression {
            Expr::GenericCall(_, arguments, _, _) => !arguments.is_empty(),
            Expr::View(inner, _) | Expr::Paren(inner, _) => {
                Self::has_explicit_type_arguments(inner)
            }
            _ => false,
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

fn facts_in_span<V: Clone>(facts: &HashMap<Span, V>, owner: Span) -> HashMap<Span, V> {
    facts
        .iter()
        .filter(|(span, _)| {
            span.file == owner.file && span.start >= owner.start && span.end <= owner.end
        })
        .map(|(span, value)| (*span, value.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use jett_common::FileId;
    use jett_diagnostics::Severity;
    use jett_types::{CapabilityKind, TypeInterner};

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
    fn lowers_capability_parameters_with_nominal_types() {
        let program = lower_source(
            r#"namespace app
function main(stdout: Stdout, stderr: Stderr, stdin: Stdin, filesystem: Filesystem, network: Network, clock: Clock, random: Random, process: Process, environment: Environment, log: Log, graphics: Graphics) returns nothing:
    return nothing
"#,
        );
        let actual = program.functions[0]
            .params
            .iter()
            .map(|param| param.ty)
            .collect::<Vec<_>>();
        let expected = CapabilityKind::ALL.map(TypeInterner::capability).to_vec();

        assert_eq!(actual, expected);
        assert!(actual.iter().all(|type_id| *type_id != TypeInterner::ERROR));
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
    fn lowers_mutual_function_bodies_through_declaration_resolutions() {
        let program = lower_source(
            r#"namespace app
mutual:
    function is_even(value: int64) returns bool
    function is_odd(value: int64) returns bool
function is_even(value: int64) returns bool:
    if value == 0:
        return true
    return is_odd(value - 1)
function is_odd(value: int64) returns bool:
    if value == 0:
        return false
    return is_even(value - 1)
"#,
        );

        assert_eq!(program.functions.len(), 2);
        assert!(
            program
                .functions
                .iter()
                .all(|function| function.source_definition.is_some())
        );
    }

    #[test]
    fn canonicalizes_contextual_negated_literals_at_signed_minimum() {
        let program = lower_source("function minimum() returns int8:\n    return -128\n");
        let StatementKind::Return(Some(value)) = &program.functions[0].body.statements[0].kind
        else {
            panic!("expected a returned literal");
        };
        assert_eq!(value.ty, jett_types::TypeInterner::INT8);
        assert_eq!(value.kind, ExpressionKind::Int(-128));
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
            StatementKind::Breakpoint {
                condition: Some(_),
                ..
            }
        ));
        assert!(matches!(body.statements[1].kind, StatementKind::Trace(_)));
        let StatementKind::Assign { value, .. } = &body.statements[2].kind else {
            panic!("expected interpolation assignment");
        };
        assert!(matches!(value.kind, ExpressionKind::StringInterpolation(_)));
    }

    #[test]
    fn lowers_actor_receive_handlers_as_backend_functions() {
        let program = lower_source(
            r#"namespace app
actor Counter:
    mutable int64 count = 0
    receive add(amount: int64) responds int64:
        count = count + amount
        respond count
"#,
        );

        assert_eq!(program.functions.len(), 1);
        let handler = &program.functions[0];
        assert_eq!(handler.identity.declaration.namespace, "app");
        assert_eq!(handler.identity.declaration.name, "Counter.add");
        assert_eq!(handler.params.len(), 1);
        assert_eq!(handler.params[0].name, "amount");
        assert!(handler.locals.iter().any(|local| local.name == "count"));
        assert!(matches!(
            handler.body.statements.as_slice(),
            [
                Statement {
                    kind: StatementKind::Assign { .. },
                    ..
                },
                Statement {
                    kind: StatementKind::Respond(_),
                    ..
                }
            ]
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
    fn prunes_checker_selected_generic_if_and_match_control_flow() {
        let program = lower_source(
            r#"namespace app
function list_length_or_zero[T](value: T) returns int64:
    if type.kind_tag[T]() == TypeKind.list_type:
        list[int64] items = value
        return 1
    return 0
function primitive_string_or_other[T](value: T) returns string:
    TypePrimitive primitive = type.primitive_tag[T]() handle:
        default TypePrimitive.unknown_type
    match primitive:
        string_type:
            string text = value
            return text
        other:
            return "other"
function main() returns string:
    list[int64] values = list(1, 2)
    int64 present = list_length_or_zero[list[int64]](values)
    int64 absent = list_length_or_zero[string]("Ada")
    string text = primitive_string_or_other[string]("Ada")
    string other = primitive_string_or_other[int64](7)
    return "{present}:{absent}:{text}:{other}"
"#,
        );

        let list_instantiations = program
            .functions
            .iter()
            .filter(|function| function.identity.declaration.name == "list_length_or_zero")
            .collect::<Vec<_>>();
        assert_eq!(list_instantiations.len(), 2);
        assert!(list_instantiations.iter().any(|function| {
            matches!(
                function.body.statements.as_slice(),
                [
                    Statement {
                        kind: StatementKind::Scope(_),
                        ..
                    },
                    Statement {
                        kind: StatementKind::Return(_),
                        ..
                    }
                ]
            )
        }));
        assert!(list_instantiations.iter().any(|function| {
            matches!(
                function.body.statements.as_slice(),
                [Statement {
                    kind: StatementKind::Return(_),
                    ..
                }]
            )
        }));

        let match_instantiations = program
            .functions
            .iter()
            .filter(|function| function.identity.declaration.name == "primitive_string_or_other")
            .collect::<Vec<_>>();
        assert_eq!(match_instantiations.len(), 2);
        assert!(match_instantiations.iter().all(|function| {
            function
                .body
                .statements
                .iter()
                .all(|statement| !matches!(statement.kind, StatementKind::Match { .. }))
                && function
                    .body
                    .statements
                    .iter()
                    .any(|statement| matches!(statement.kind, StatementKind::Scope(_)))
        }));
    }

    #[test]
    fn alias_and_underlying_type_lower_to_distinct_branch_and_match_functions() {
        let program = lower_source(
            r#"namespace app
type Names = list[string]
function classify_branch[T]() returns string:
    if type.kind_tag[T]() == TypeKind.alias_type:
        return "alias"
    else:
        return "list"
function classify_match[T]() returns string:
    match type.kind_tag[T]():
        alias_type:
            return "alias"
        other:
            return "list"
function main() returns nothing:
    string branch_alias = classify_branch[Names]()
    string branch_list = classify_branch[list[string]]()
    string match_alias = classify_match[Names]()
    string match_list = classify_match[list[string]]()
"#,
        );

        for name in ["classify_branch", "classify_match"] {
            let instantiations = program
                .functions
                .iter()
                .filter(|function| function.identity.declaration.name == name)
                .collect::<Vec<_>>();
            assert_eq!(instantiations.len(), 2);
            assert_eq!(
                instantiations[0].identity.type_arguments,
                instantiations[1].identity.type_arguments
            );
            assert_ne!(
                instantiations[0].identity.specialization,
                instantiations[1].identity.specialization
            );
            assert!(instantiations.iter().all(|function| {
                function.body.statements.iter().all(|statement| {
                    !matches!(
                        statement.kind,
                        StatementKind::If { .. } | StatementKind::Match { .. }
                    )
                })
            }));
        }

        let main = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "main")
            .expect("main should lower");
        let call_targets = main
            .body
            .statements
            .iter()
            .filter_map(|statement| match &statement.kind {
                StatementKind::Let {
                    value:
                        Expression {
                            kind: ExpressionKind::Call { function, .. },
                            ..
                        },
                    ..
                } => Some(*function),
                _ => None,
            })
            .collect::<HashSet<_>>();
        assert_eq!(call_targets.len(), 4);
    }

    #[test]
    fn rejects_static_selection_attached_to_the_wrong_statement_kind() {
        let source = r#"function choose[T]() returns int64:
    if type.kind_tag[T]() == TypeKind.list_type:
        return 1
    return 0
function main() returns int64:
    return choose[list[int64]]()
"#;
        let file = FileId::new(0);
        let parsed = jett_parser::parse(source, file);
        let resolved = jett_resolve::resolve(&parsed.module);
        let mut checked = jett_typecheck::check(&parsed.module, &resolved);
        assert!(
            checked
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != Severity::Error),
            "type errors: {:?}",
            checked.diagnostics
        );
        let instantiation = checked
            .generic_function_instantiations
            .first_mut()
            .expect("choose[list[int64]] should be instantiated");
        let span = *instantiation
            .static_selections
            .keys()
            .next()
            .expect("static if selection should be exported");
        instantiation
            .static_selections
            .insert(span, CheckedStaticSelection::MatchArm(0));

        let origins = HashMap::from([(file, SourceOrigin::Project)]);
        let errors = lower(&parsed.module, &resolved, &checked, &origins)
            .expect_err("mismatched static selection must fail lowering");
        assert!(errors.iter().any(|error| {
            error
                .message
                .contains("static match selection attached to an if")
        }));
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
    fn keeps_generic_loop_binding_types_separate() {
        let program = lower_source(
            r#"namespace app
function count[T](items: list[T]) returns int64:
    mutable int64 total = 0
    for item in view items:
        total = total + 1
    return total
function main() returns nothing:
    int64 numbers = count[int64](list(1, 2))
    int64 words = count[string](list("one", "two"))
"#,
        );
        let mut binding_types = program
            .functions
            .iter()
            .filter(|function| function.identity.declaration.name == "count")
            .map(|function| {
                let item = function
                    .locals
                    .iter()
                    .find(|local| local.name == "item")
                    .expect("generic loop binding should lower");
                (function.identity.type_arguments[0], item.ty)
            })
            .collect::<Vec<_>>();
        binding_types.sort_by_key(|(argument, _)| argument.index());
        assert_eq!(
            binding_types,
            [
                (TypeInterner::INT64, TypeInterner::INT64),
                (TypeInterner::STRING, TypeInterner::STRING),
            ]
        );
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
            kind:
                HandleKind::Refinement {
                    refined_type,
                    ref predicates,
                },
            error_local: Some(error_local),
            ..
        } = value.kind
        else {
            panic!("expected refinement boundary handle");
        };
        assert_eq!(refined_type, value.ty);
        assert_eq!(predicates.len(), 1);
        assert_eq!(predicates[0].type_name, "app.Positive");
        assert_eq!(function.locals[error_local.index() as usize].name, "error");
        assert_eq!(
            program.functions[1].identity.declaration.kind,
            DeclarationKind::RefinementPredicate
        );
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
            kind: ExpressionKind::Intrinsic {
                intrinsic, args, ..
            },
            ..
        })) = &program.functions[0].body.statements[0].kind
        else {
            panic!("expected compiler intrinsic");
        };
        assert_eq!(*intrinsic, IntrinsicId::MathAbs);
        assert_eq!(args.len(), 1);
    }

    #[test]
    fn direct_user_functions_remain_distinct_from_intrinsics() {
        let program = lower_source(
            r#"namespace app
function identity(value: int64) returns int64:
    return value
function main() returns int64:
    return identity(7)
"#,
        );
        let main = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "main")
            .expect("main should lower");
        assert!(matches!(
            main.body.statements[0].kind,
            StatementKind::Return(Some(Expression {
                kind: ExpressionKind::Call { .. },
                ..
            }))
        ));
    }

    #[test]
    fn lowering_rejects_an_intrinsic_without_a_checked_closed_identity() {
        let source = r#"namespace app
function absolute(value: int64) returns int64:
    return math.abs(value)
"#;
        let file = FileId::new(0);
        let parsed = jett_parser::parse(source, file);
        let resolved = jett_resolve::resolve(&parsed.module);
        let mut checked = jett_typecheck::check(&parsed.module, &resolved);
        assert_eq!(
            checked.intrinsic_ids.values().copied().collect::<Vec<_>>(),
            [IntrinsicId::MathAbs]
        );
        checked.intrinsic_ids.clear();
        let origins = HashMap::from([(file, SourceOrigin::Project)]);

        let errors = lower(&parsed.module, &resolved, &checked, &origins)
            .expect_err("unclassified intrinsic must not enter HIR");

        assert!(
            errors
                .iter()
                .any(|error| { error.message.contains("has no closed intrinsic identity") })
        );
    }

    #[test]
    fn preserves_checked_intrinsic_type_operands() {
        let program = lower_source(
            r#"namespace app
function describe[T]() returns string:
    return type.name[T]()
function main() returns string:
    return describe[int64]()
"#,
        );
        let describe = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "describe")
            .expect("concrete describe function should lower");
        let StatementKind::Return(Some(Expression {
            kind:
                ExpressionKind::Intrinsic {
                    intrinsic,
                    type_arguments,
                    ..
                },
            ..
        })) = &describe.body.statements[0].kind
        else {
            panic!("expected reflected compiler intrinsic");
        };
        assert_eq!(*intrinsic, IntrinsicId::TypeName);
        assert_eq!(type_arguments, describe.identity.type_arguments.as_slice());
    }

    #[test]
    fn preserves_stdlib_kernel_type_operands_in_inferred_generic_instantiations() {
        let source = r#"namespace list
export function length[T](view items: list[T]) returns int64:
    return list.__length[T](view items)
function main() returns int64:
    list[int64] items = list()
    return list.length(view items)
"#;
        let file = FileId::new(jett_common::STDLIB_FILE_ID_START);
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
                .all(|diagnostic| diagnostic.severity != Severity::Error),
            "resolve errors: {:?}",
            resolved.diagnostics
        );
        let checked = jett_typecheck::check(&parsed.module, &resolved);
        assert!(
            checked
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != Severity::Error),
            "type errors: {:?}",
            checked.diagnostics
        );
        let origins = HashMap::from([(file, SourceOrigin::Stdlib)]);
        let program = lower(&parsed.module, &resolved, &checked, &origins)
            .expect("stdlib generic kernel call should lower");

        let length = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "length")
            .expect("concrete length function should lower");
        let StatementKind::Return(Some(Expression {
            kind:
                ExpressionKind::Intrinsic {
                    intrinsic,
                    type_arguments,
                    ..
                },
            ..
        })) = &length.body.statements[0].kind
        else {
            panic!("expected stdlib kernel intrinsic");
        };
        assert_eq!(*intrinsic, IntrinsicId::ListLength);
        assert_eq!(type_arguments, length.identity.type_arguments.as_slice());
    }

    #[test]
    fn lowers_list_stdlib_specializations_with_checked_kernel_operands() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let stdlib_file = FileId::new(jett_common::STDLIB_FILE_ID_START);
        let project_file = FileId::new(0);
        let stdlib_source = std::fs::read_to_string(root.join("stdlib/list.jett"))
            .expect("list stdlib source should be readable");
        let project_source =
            std::fs::read_to_string(root.join("tests/run_pass/list_operations.jett"))
                .expect("list fixture should be readable");
        let stdlib = jett_parser::parse(&stdlib_source, stdlib_file);
        let project = jett_parser::parse(&project_source, project_file);
        assert!(
            stdlib.errors.is_empty(),
            "stdlib parse errors: {:?}",
            stdlib.errors
        );
        assert!(
            project.errors.is_empty(),
            "project parse errors: {:?}",
            project.errors
        );
        let mut items = stdlib.module.items;
        items.extend(project.module.items);
        let module = Module {
            items,
            span: project.module.span,
        };
        let resolved = jett_resolve::resolve(&module);
        assert!(
            resolved
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != Severity::Error),
            "resolve errors: {:?}",
            resolved.diagnostics
        );
        let checked = jett_typecheck::check(&module, &resolved);
        assert!(
            checked
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != Severity::Error),
            "type errors: {:?}",
            checked.diagnostics
        );
        let origins = HashMap::from([
            (stdlib_file, SourceOrigin::Stdlib),
            (project_file, SourceOrigin::Project),
        ]);
        lower(&module, &resolved, &checked, &origins)
            .expect("list stdlib specializations should lower");
    }

    #[test]
    fn lowers_reflected_comptime_type_bodies_as_exclusive_dispatch() {
        let program = lower_source(
            r#"namespace app
struct User:
    name: string
    age: int64
    city: string
function reflected_names[T](view value: T) returns string:
    mutable string output = ""
    for field in type.fields[T]():
        comptime type Field = field.type_info:
            output = type.name[Field]()
    return output
function main() returns string:
    User user = User(name: "Ada", age: 37, city: "London")
    return reflected_names[User](view user)
"#,
        );
        let reflected = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "reflected_names")
            .expect("concrete reflected function should lower");
        let StatementKind::For { body, .. } = &reflected.body.statements[1].kind else {
            panic!("expected reflected field loop");
        };
        let StatementKind::ReflectedTypeDispatch { type_info, arms } = &body.statements[0].kind
        else {
            panic!("expected compiler-owned reflected type dispatch");
        };
        assert!(matches!(type_info.kind, ExpressionKind::Field { .. }));
        assert_eq!(arms.len(), 2);
        assert_eq!(
            arms.iter()
                .map(|arm| arm.iteration_index)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        assert_ne!(arms[0].bound_type, arms[1].bound_type);
        for arm in arms {
            let StatementKind::Assign {
                value:
                    Expression {
                        kind:
                            ExpressionKind::Intrinsic {
                                intrinsic,
                                type_arguments,
                                ..
                            },
                        ..
                    },
                ..
            } = &arm.body.statements[0].kind
            else {
                panic!("expected specialized reflected body");
            };
            assert_eq!(*intrinsic, IntrinsicId::TypeName);
            assert_eq!(type_arguments, &[arm.bound_type]);
        }
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
    fn extracts_capture_free_inline_functions_and_indirect_calls() {
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
        assert!(matches!(
            value.kind,
            ExpressionKind::FunctionRef(FunctionId(1))
        ));
        assert_eq!(program.functions[1].params.len(), 1);
        assert!(matches!(
            main.body.statements[1].kind,
            StatementKind::Return(Some(Expression {
                kind: ExpressionKind::IndirectCall { .. },
                ..
            }))
        ));
    }

    #[test]
    fn keeps_captured_inline_functions_explicit() {
        let source = r#"namespace app
function make(seed: int64) returns function(int64) returns int64:
    return function(value: int64) returns int64: return value + seed
"#;
        let program = lower_source(source);
        assert_eq!(program.functions.len(), 1);
        assert!(matches!(
            program.functions[0].body.statements[0].kind,
            StatementKind::Return(Some(Expression {
                kind: ExpressionKind::InlineFunction { .. },
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
