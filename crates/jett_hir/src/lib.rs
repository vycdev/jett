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
    CheckResult, CheckedBindingMode, CheckedBodyFacts, CheckedCallArgumentOrder,
    CheckedComptimeTypeBinding, CheckedComptimeTypeSelection, CheckedGenericCall,
    CheckedGenericFunctionInstantiation, CheckedGenericSpecialization, CheckedInterfaceCall,
    CheckedMethodCall, CheckedMethodDefinition, CheckedMethodValue, CheckedStaticSelection,
    CheckedStructConstruction, CheckedViewSource,
};
use jett_types::{
    ReflectionBitfieldFieldInfo, ReflectionBitfieldInfo, ReflectionFieldInfo,
    ReflectionMachineInfo, ReflectionMachineStateInfo, ReflectionMachineTransitionInfo,
    ReflectionTypeInfo, ReflectionVariantInfo, Type, TypeId, TypeInterner,
};

mod inline_functions;
mod interface_values;
#[cfg(test)]
mod local_view_tests;
mod local_views;
mod reflected_fields;
mod type_validation;

pub use interface_values::complete_value_conversions;
pub use local_views::{
    is_borrowed_local, local_view_root, validate_local_view_initializer, validate_local_views,
};
pub use reflected_fields::{
    ReflectedFieldAction, ReflectedFieldPlan, ReflectedFieldRequirement,
    ReflectedFieldRequirementPlan, ReflectedFieldUnsupported, ReflectedFieldValidation,
    is_reflected_field_intrinsic, refinement_predicate_declaration_matches, reflected_field_layout,
    reflected_field_requirement, valid_reflected_field_metadata_type,
    validate_reflected_field_plans,
};
pub use type_validation::validate_backend_types;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId(u32);

impl FunctionId {
    /// Allocate a compiler-managed function after checked HIR lowering.
    pub fn new(index: u32) -> Self {
        Self(index)
    }

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
    pub fn new(index: u32) -> Self {
        Self(index)
    }

    pub fn index(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VariantId(u32);

impl VariantId {
    /// Select a checked enum variant when building compiler-owned HIR.
    pub fn new(index: u32) -> Self {
        Self(index)
    }

    pub fn index(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StateId(u32);

impl StateId {
    /// Select a checked machine state when building compiler-owned HIR.
    pub fn new(index: u32) -> Self {
        Self(index)
    }

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
    Verify,
    Property,
    Method,
    ActorConstructor,
    ActorHandler,
    RefinementPredicate,
}

/// One concrete in-memory function identity. Type arguments, checked
/// reflection-visible specialization facts, and lexical comptime type bindings
/// form identity after monomorphization. All are empty for an ordinary named
/// function. Raw `TypeId`s
/// are session-local; persistent artifacts must encode their canonical
/// structural type identities and this specialization discriminator instead.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionIdentity {
    /// Lexical comptime type bindings, in enclosing-to-inner order.
    pub scoped_type_bindings: Vec<ScopedTypeBinding>,
    pub declaration: DeclarationId,
    pub type_arguments: Vec<TypeId>,
    pub specialization: CheckedGenericSpecialization,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScopedTypeBinding {
    pub name: String,
    pub ty: TypeId,
    pub reflection: jett_types::ReflectionTypeInfo,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub functions: Vec<Function>,
    pub equality_methods: HashMap<TypeId, FunctionId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub id: FunctionId,
    pub identity: FunctionIdentity,
    pub debug_kind: FunctionDebugKind,
    pub source_definition: Option<DefId>,
    pub params: Vec<Param>,
    /// Leading parameters supplied by a closure environment or actor state snapshot.
    pub capture_count: usize,
    pub return_type: TypeId,
    pub locals: Vec<Local>,
    pub body: Block,
    pub span: Span,
}

/// Source identity used when formatting a function value. Extraction can turn
/// either source form into a FunctionRef, so its origin must remain explicit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionDebugKind {
    Named(String),
    Inline,
}

impl FunctionDebugKind {
    pub fn named(namespace: &str, name: &str) -> Self {
        Self::Named(if namespace.is_empty() {
            name.to_owned()
        } else {
            format!("{namespace}.{name}")
        })
    }
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
    pub debug_ty: TypeId,
    pub debug_type_name: Option<String>,
    pub mutable: bool,
    /// Immediate stable backing local for a checked borrowed alias. Parameters
    /// use `ParamMode`; owned locals and generated snapshots have no source.
    /// Projected aliases keep their canonical typed path in the initializer.
    pub view_source: Option<LocalId>,
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
    /// backend must compare source-visible reflection identity rather than
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
    /// Source-visible reflection identity of the checker-selected bound type,
    /// including aliases inside nested type arguments.
    pub reflection_identity: String,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expression {
    pub kind: ExpressionKind,
    pub ty: TypeId,
    pub span: Span,
}

/// Compiler-generated identity of a deterministic native property trial.
#[derive(Debug, Clone, PartialEq)]
pub struct NativePropertyCase {
    pub name: String,
    /// One-based trial number.
    pub trial: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExpressionKind {
    Int(i128),
    Float(f64),
    String(String),
    Bool(bool),
    Nothing,
    Local(LocalId),
    /// A checked namespace constant materialized before MIR lowering.
    Constant {
        declaration: Span,
    },
    /// A checked, concrete source function used as a first-class value.
    FunctionRef(FunctionId),
    /// A checked inline function with an environment copied from caller locals.
    ClosureRef {
        function: FunctionId,
        captures: Vec<LocalId>,
    },
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
        /// aliases that share a canonical type ID. Construction start also
        /// carries the checked source type of each struct field in layout order.
        reflection_arguments: Vec<ReflectionTypeInfo>,
        /// Field predicate chains for reflected struct fields or flattened
        /// enum variant payloads, or machine state payloads. Empty for other
        /// intrinsics and targets without refinement fields.
        refinement_predicates: Vec<Vec<RefinementPredicate>>,
        /// Per-declared-field proof plans, consumed into CFG after selector checks.
        field_validation: Option<ReflectedFieldValidation>,
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
        /// Predicate chains by canonical field index for base-value inputs.
        refinement_predicates: Vec<Vec<RefinementPredicate>>,
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
        /// Variant payloads in declaration order.
        payloads: Vec<Expression>,
        /// Payload indices in lexical source evaluation order.
        evaluation_order: Vec<usize>,
    },
    StringInterpolation(Vec<StringSegment>),
    /// Require the string returned by an implicitly selected display method to
    /// be ready before interpolation evaluates its next segment.
    DisplayResult(Box<Expression>),
    /// Require the bool returned by implicit struct equality to be ready before
    /// using or negating it, while retaining its checked secret qualification.
    EquatableResult(Box<Expression>),
    Comptime {
        value: Box<Expression>,
        bindings: Vec<ScopedTypeBinding>,
        /// Stable lookup identity, even when parentheses widen the expression span.
        source_span: Span,
    },
    Declassify(Box<Expression>),
    Coarsen(Box<Expression>),
    /// Compiler-owned conversion after every required refinement predicate passed.
    RefinementValidated(Box<Expression>),
    /// Explicit conversion at an interface-compatible typed boundary.
    InterfaceCoerce {
        value: Box<Expression>,
        adapters: Vec<InterfaceFunctionAdapter>,
    },
    /// A checked signature adapter retaining its source function descriptor.
    FunctionAdapter {
        value: Box<Expression>,
        function: FunctionId,
    },
    /// Concrete implementation identity of an erased interface value.
    InterfaceType(Box<Expression>),
    /// Compiler-owned terminal runtime failure with an already checked result type.
    RuntimeFailure(String),
    /// Compiler-owned terminal failure borrowing an exact string error message.
    /// This has no source spelling and evaluates to nothing.
    RuntimeFailureMessage(Box<Expression>),
    /// Set or clear diagnostic context around a generated property call.
    /// This has no source-level spelling and evaluates to nothing.
    PropertyCaseContext(Option<NativePropertyCase>),
    StateIs {
        value: Box<Expression>,
        state: StateId,
    },
    Run(Box<Expression>),
    Join(Box<Expression>),
    Cancel(Box<Expression>),
    InlineFunction {
        scoped_type_bindings: Vec<ScopedTypeBinding>,
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
        /// Source spawns invoke the checked constructor. `None` is the
        /// compiler-generated allocation at the end of that constructor.
        constructor: Option<FunctionId>,
    },
    ActorMessage {
        actor: Box<Expression>,
        message: String,
        handler: FunctionId,
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

/// Generated callback conversion required inside a container conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterfaceFunctionAdapter {
    pub source: TypeId,
    pub target: TypeId,
    pub function: FunctionId,
}

impl ExpressionKind {
    pub fn interface_coerce(value: Box<Expression>) -> Self {
        Self::InterfaceCoerce {
            value,
            adapters: Vec::new(),
        }
    }
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
    pub base_type: TypeId,
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
        local_types: Vec::new(),
        loop_depth: 0,
        handle_depth: 0,
    };
    for (index, function) in program.functions.iter().enumerate() {
        if function.id.index() as usize != index {
            validator.error(function.span, "function IDs must be dense and ordered");
        }
        validator.local_count = function.locals.len();
        validator.local_types = function.locals.iter().map(|local| local.ty).collect();
        for (local_index, local) in function.locals.iter().enumerate() {
            if local.id.index() as usize != local_index {
                validator.error(local.span, "local IDs must be dense and ordered");
            }
        }
        validator
            .errors
            .extend(local_views::validate_structure(function));
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
    local_types: Vec<TypeId>,
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
            ExpressionKind::ClosureRef { function, captures } => {
                let Some(target) = self.program.functions.get(function.index() as usize) else {
                    self.error(
                        expression.span,
                        "closure references an unknown HIR function",
                    );
                    return;
                };
                if target.capture_count != captures.len()
                    || target
                        .params
                        .iter()
                        .take(target.capture_count)
                        .zip(captures)
                        .any(|(parameter, capture)| {
                            self.local_types.get(capture.index() as usize) != Some(&parameter.ty)
                        })
                {
                    self.error(
                        expression.span,
                        "closure capture list disagrees with its function",
                    );
                }
                for capture in captures {
                    self.check_local(*capture, expression.span);
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
            | ExpressionKind::Comptime { value, .. }
            | ExpressionKind::Declassify(value)
            | ExpressionKind::Coarsen(value)
            | ExpressionKind::RefinementValidated(value)
            | ExpressionKind::DisplayResult(value)
            | ExpressionKind::EquatableResult(value)
            | ExpressionKind::RuntimeFailureMessage(value)
            | ExpressionKind::InterfaceType(value)
            | ExpressionKind::Run(value)
            | ExpressionKind::Join(value)
            | ExpressionKind::Cancel(value) => self.expression(value),
            ExpressionKind::FunctionAdapter { value, function } => {
                if function.index() as usize >= self.program.functions.len() {
                    self.error(
                        expression.span,
                        "adapter references an unknown HIR function",
                    );
                }
                self.expression(value);
            }
            ExpressionKind::InterfaceCoerce { value, adapters } => {
                for entry in adapters {
                    if entry.function.index() as usize >= self.program.functions.len() {
                        self.error(
                            expression.span,
                            "container adapter references an unknown HIR function",
                        );
                    }
                }
                self.expression(value);
            }
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
                intrinsic,
                field_validation,
                args,
                evaluation_order,
                ..
            } => {
                match (is_reflected_field_intrinsic(*intrinsic), field_validation) {
                    (true, Some(ReflectedFieldValidation::Validate(plans))) => {
                        for plan in plans {
                            for predicate in plan.predicates() {
                                let valid = self
                                    .program
                                    .functions
                                    .get(predicate.function.index() as usize)
                                    .is_some_and(|function| {
                                        function.id == predicate.function
                                            && refinement_predicate_declaration_matches(
                                                &function.identity.declaration,
                                                &predicate.type_name,
                                            )
                                            && function.params.len() == 1
                                            && function.capture_count == 0
                                            && function.params[0].mode == ParamMode::Owned
                                            && function.params[0].ty == predicate.input_type
                                            && function.return_type == TypeInterner::BOOL
                                    });
                                if !valid {
                                    self.error(
                                        expression.span,
                                        "reflected validation predicate function is invalid",
                                    );
                                }
                            }
                        }
                    }
                    (false, None) => {}
                    _ => self.error(
                        expression.span,
                        "reflected read requires checked source validation plans",
                    ),
                }
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
                validates_refinements,
                refinement_predicates,
                ..
            } => {
                self.check_evaluation_order(evaluation_order, fields.len(), expression.span);
                if (*validates_refinements && refinement_predicates.len() != fields.len())
                    || (!*validates_refinements && !refinement_predicates.is_empty())
                {
                    self.error(
                        expression.span,
                        "struct refinement predicate layout is invalid",
                    );
                }
                for chain in refinement_predicates {
                    for predicate in chain {
                        if predicate.function.index() as usize >= self.program.functions.len() {
                            self.error(
                                expression.span,
                                "struct refinement predicate is outside HIR function table",
                            );
                        }
                    }
                }
                for field in fields {
                    self.expression(field);
                }
            }
            ExpressionKind::BitfieldConstruct {
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
            ExpressionKind::EnumConstruct {
                payloads,
                evaluation_order,
                ..
            } => {
                self.check_evaluation_order(evaluation_order, payloads.len(), expression.span);
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
                ..
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
                constructor,
                ..
            } => {
                self.check_evaluation_order(evaluation_order, args.len(), expression.span);
                if let Some(constructor) = constructor {
                    if self
                        .program
                        .functions
                        .get(constructor.index() as usize)
                        .is_none()
                    {
                        self.error(expression.span, "actor constructor target is absent");
                    }
                }
                for argument in args {
                    self.expression(argument);
                }
            }
            ExpressionKind::ActorMessage {
                actor,
                handler,
                args,
                evaluation_order,
                ..
            } => {
                if self
                    .program
                    .functions
                    .get(handler.index() as usize)
                    .is_none()
                {
                    self.error(expression.span, "actor handler target is absent");
                }
                self.expression(actor);
                self.check_evaluation_order(evaluation_order, args.len(), expression.span);
                for argument in args {
                    self.expression(argument);
                }
            }
            ExpressionKind::Field { base, .. } => self.expression(base),
            ExpressionKind::Int(_)
            | ExpressionKind::Constant { .. }
            | ExpressionKind::Float(_)
            | ExpressionKind::String(_)
            | ExpressionKind::Bool(_)
            | ExpressionKind::Nothing
            | ExpressionKind::PropertyCaseContext(_)
            | ExpressionKind::RuntimeFailure(_)
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
    Lowerer::new(module, resolve, check, origins, false).lower()
}

/// Lower checked verification and property bodies as callable test predicates.
/// Property `given` bindings become typed function parameters; generation stays
/// with the test runner. Production program lowering continues to use `lower`.
pub fn lower_with_test_bodies(
    module: &Module,
    resolve: &ResolveResult,
    check: &CheckResult,
    origins: &HashMap<FileId, SourceOrigin>,
) -> Result<Program, Vec<LowerError>> {
    Lowerer::new(module, resolve, check, origins, true).lower()
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

struct ActorConstructorSource<'a> {
    id: FunctionId,
    actor_definition: DefId,
    actor: &'a ast::ActorDef,
    ty: TypeId,
}

struct RefinementSource<'a> {
    id: FunctionId,
    definition: DefId,
    alias: &'a ast::TypeAlias,
    ty: TypeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum FunctionKey {
    Interface {
        owner: TypeId,
        method: usize,
    },
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
    actor_constructors: Vec<ActorConstructorSource<'a>>,
    actor_constructor_ids: HashMap<TypeId, FunctionId>,
    actor_handler_ids: HashMap<(TypeId, String), FunctionId>,
    actor_handlers: Vec<ActorHandlerSource<'a>>,
    refinement_sources: Vec<RefinementSource<'a>>,
    refinement_function_ids: HashMap<TypeId, FunctionId>,
    function_ids: HashMap<FunctionKey, FunctionId>,
    constant_definitions: HashMap<DefId, Span>,
    errors: Vec<LowerError>,
    include_test_bodies: bool,
}

impl<'a> Lowerer<'a> {
    fn new(
        module: &'a Module,
        resolve: &'a ResolveResult,
        check: &'a CheckResult,
        origins: &'a HashMap<FileId, SourceOrigin>,
        include_test_bodies: bool,
    ) -> Self {
        let constant_definitions = module
            .items
            .iter()
            .filter_map(|item| {
                let Item::VarDecl(decl) = item else {
                    return None;
                };
                if decl.mutable {
                    return None;
                }
                resolve
                    .scope_table
                    .definitions
                    .iter()
                    .find(|definition| definition.span == decl.name.span)
                    .map(|definition| (definition.id, decl.name.span))
            })
            .collect();
        Self {
            module,
            resolve,
            check,
            origins,
            functions: Vec::new(),
            actor_constructors: Vec::new(),
            actor_constructor_ids: HashMap::new(),
            actor_handler_ids: HashMap::new(),
            actor_handlers: Vec::new(),
            refinement_sources: Vec::new(),
            refinement_function_ids: HashMap::new(),
            function_ids: HashMap::new(),
            constant_definitions,
            errors: Vec::new(),
            include_test_bodies,
        }
    }

    fn lower(mut self) -> Result<Program, Vec<LowerError>> {
        self.collect_functions();
        let interface_dispatches = self.collect_interface_dispatches();
        let sources = std::mem::take(&mut self.functions);
        let mut functions = Vec::with_capacity(sources.len());
        for source in sources {
            if let Some(function) = self.lower_function(source) {
                functions.push(function);
            }
        }
        let actor_constructors = std::mem::take(&mut self.actor_constructors);
        for source in actor_constructors {
            if let Some(function) = self.lower_actor_constructor(source) {
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
        functions.extend(interface_dispatches);
        if self.include_test_bodies {
            let mut namespaces = HashMap::new();
            for item in &self.module.items {
                if let Item::Namespace(namespace) = item {
                    namespaces.insert(namespace.span.file, namespace.name.name.clone());
                    continue;
                }
                let test = match item {
                    Item::Verify(test) => Some((
                        DeclarationKind::Verify,
                        &test.name,
                        &[][..],
                        &test.body,
                        test.span,
                    )),
                    Item::Property(test) => Some((
                        DeclarationKind::Property,
                        &test.name,
                        test.givens.as_slice(),
                        &test.body,
                        test.span,
                    )),
                    _ => None,
                };
                if let Some((kind, name, givens, body, span)) = test {
                    let namespace = namespaces.get(&span.file).map_or("", String::as_str);
                    if let Some(function) = self.lower_test_body(
                        FunctionId(functions.len() as u32),
                        kind,
                        namespace,
                        name,
                        givens,
                        body,
                        span,
                    ) {
                        functions.push(function);
                    }
                }
            }
        }
        if self.errors.is_empty() {
            inline_functions::extract_inline_functions(&mut functions, &self.check.interner);
            let equality_methods = self
                .check
                .equality_methods
                .iter()
                .map(|(&owner, method)| {
                    self.function_ids
                        .get(&FunctionKey::Method {
                            source_span: method.source_span,
                        })
                        .copied()
                        .map(|target| (owner, target))
                        .ok_or_else(|| {
                            vec![LowerError {
                                span: method.source_span,
                                message:
                                    "checked equality method is absent from HIR function identities"
                                        .into(),
                            }]
                        })
                })
                .collect::<Result<HashMap<_, _>, _>>()?;
            let mut program = Program {
                functions,
                equality_methods,
            };
            interface_values::coerce_program(&mut program, &self.check.interner);
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
            let Some(&ty) = self.check.definition_types.get(&actor_definition) else {
                self.error(actor.name.span, "actor has no checked type");
                continue;
            };
            let id = FunctionId((self.functions.len() + self.actor_constructors.len()) as u32);
            self.actor_constructor_ids.insert(ty, id);
            self.actor_constructors.push(ActorConstructorSource {
                id,
                actor_definition,
                actor,
                ty,
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
            let Some(&actor_type) = self.check.definition_types.get(&actor_definition) else {
                self.error(actor.name.span, "actor handler has no checked actor type");
                continue;
            };
            for (message_index, handler) in actor.handlers.iter().enumerate() {
                let id = FunctionId(
                    (self.functions.len()
                        + self.actor_constructors.len()
                        + self.actor_handlers.len()) as u32,
                );
                self.actor_handler_ids
                    .insert((actor_type, handler.name.name.clone()), id);
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
                (self.functions.len()
                    + self.actor_constructors.len()
                    + self.actor_handlers.len()
                    + self.refinement_sources.len()) as u32,
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

    fn lower_test_body(
        &mut self,
        id: FunctionId,
        kind: DeclarationKind,
        namespace: &str,
        name: &ast::Ident,
        givens: &[ast::GivenDecl],
        source_body: &ast::Block,
        span: Span,
    ) -> Option<Function> {
        let Some(origin) = self.origins.get(&span.file).cloned() else {
            self.error(span, "test body has no source origin");
            return None;
        };
        let function_ids = self.function_ids.clone();
        let mut body_lowerer = BodyLowerer::new(
            self,
            &function_ids,
            self.check.type_map.clone(),
            self.check.debug_type_names.clone(),
            self.check.binding_modes.clone(),
            self.check.generic_calls.clone(),
            self.check.intrinsic_ids.clone(),
            self.check.intrinsic_type_arguments.clone(),
            self.check.intrinsic_reflection_arguments.clone(),
            self.check.call_argument_orders.clone(),
            self.check.method_calls.clone(),
            self.check.interface_calls.clone(),
            self.check.method_values.clone(),
            self.check.struct_constructions.clone(),
            self.check.pipeline_step_call_types.clone(),
            HashMap::new(),
            facts_in_span(&self.check.comptime_type_bindings, span),
        );
        let mut params = Vec::with_capacity(givens.len());
        for given in givens {
            let Some(definition) = body_lowerer
                .parent
                .definition_at(given.name.span, DefKind::Variable)
            else {
                body_lowerer
                    .parent
                    .error(given.span, "property input has no checked definition");
                return None;
            };
            let Some(&ty) = body_lowerer.parent.check.definition_types.get(&definition) else {
                body_lowerer
                    .parent
                    .error(given.span, "property input has no checked type");
                return None;
            };
            let local =
                body_lowerer.allocate_local(definition, &given.name.name, ty, false, given.span);
            params.push(Param {
                local,
                name: given.name.name.clone(),
                ty,
                mode: ParamMode::Owned,
                mutable: false,
                span: given.span,
            });
        }
        let body = body_lowerer.lower_block(source_body);
        body_lowerer.reject_unconsumed_static_selections();
        body_lowerer.reject_unconsumed_comptime_type_bindings();
        let locals = body_lowerer.locals;
        Some(Function {
            id,
            identity: FunctionIdentity {
                declaration: DeclarationId {
                    origin,
                    namespace: namespace.to_owned(),
                    name: format!("{}:{}", name.name, span.start),
                    kind,
                },
                scoped_type_bindings: Vec::new(),
                type_arguments: Vec::new(),
                specialization: CheckedGenericSpecialization::default(),
            },
            debug_kind: FunctionDebugKind::named(namespace, &name.name),
            source_definition: None,
            params,
            capture_count: 0,
            return_type: TypeInterner::NOTHING,
            locals,
            body,
            span,
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
            debug_type_names,
            binding_modes,
            generic_calls,
            intrinsic_ids,
            intrinsic_type_arguments,
            intrinsic_reflection_arguments,
            call_argument_orders,
            method_calls,
            interface_calls,
            method_values,
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
                instantiation.debug_type_names.clone(),
                instantiation.binding_modes.clone(),
                instantiation.generic_calls.clone(),
                instantiation.intrinsic_ids.clone(),
                instantiation.intrinsic_type_arguments.clone(),
                instantiation.intrinsic_reflection_arguments.clone(),
                instantiation.call_argument_orders.clone(),
                instantiation.method_calls.clone(),
                instantiation.interface_calls.clone(),
                instantiation.method_values.clone(),
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
                self.check.debug_type_names.clone(),
                self.check.binding_modes.clone(),
                self.check.generic_calls.clone(),
                self.check.intrinsic_ids.clone(),
                self.check.intrinsic_type_arguments.clone(),
                self.check.intrinsic_reflection_arguments.clone(),
                self.check.call_argument_orders.clone(),
                self.check.method_calls.clone(),
                self.check.interface_calls.clone(),
                self.check.method_values.clone(),
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
                    ..
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
                self.check.debug_type_names.clone(),
                self.check.binding_modes.clone(),
                self.check.generic_calls.clone(),
                self.check.intrinsic_ids.clone(),
                self.check.intrinsic_type_arguments.clone(),
                self.check.intrinsic_reflection_arguments.clone(),
                self.check.call_argument_orders.clone(),
                self.check.method_calls.clone(),
                self.check.interface_calls.clone(),
                self.check.method_values.clone(),
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
            debug_type_names,
            binding_modes,
            generic_calls,
            intrinsic_ids,
            intrinsic_type_arguments,
            intrinsic_reflection_arguments,
            call_argument_orders,
            method_calls,
            interface_calls,
            method_values,
            struct_constructions,
            pipeline_step_call_types,
            static_selections,
            comptime_type_bindings,
        );
        body_lowerer.return_type = Some(return_type);
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

        let debug_kind = match &source.method {
            Some(method) => {
                FunctionDebugKind::named(&method.owner_name, &source.function.name.name)
            }
            None => FunctionDebugKind::named(&namespace, &source.function.name.name),
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
                scoped_type_bindings: Vec::new(),
                type_arguments: concrete_args,
                specialization,
            },
            debug_kind,
            source_definition: source.definition,
            params,
            capture_count: 0,
            return_type,
            locals,
            body,
            span: source.function.span,
        })
    }

    fn lower_actor_constructor(&mut self, source: ActorConstructorSource<'a>) -> Option<Function> {
        let origin = match self.origins.get(&source.actor.span.file) {
            Some(origin) => origin.clone(),
            None => {
                self.error(
                    source.actor.span,
                    "source origin is missing for actor constructor",
                );
                return None;
            }
        };
        let Type::Actor(actor_id) = self.check.interner.resolve(source.ty) else {
            self.error(
                source.actor.name.span,
                "checked actor constructor target is not an actor",
            );
            return None;
        };
        let definition = self.check.interner.resolve_actor(*actor_id).clone();
        if source.actor.capability_params.len() != definition.capability_params.len()
            || source.actor.state_fields.len() != definition.state_fields.len()
        {
            self.error(source.actor.span, "checked actor constructor shape changed");
            return None;
        }
        let namespace = self
            .resolve
            .scope_table
            .def(source.actor_definition)
            .namespace
            .clone()
            .unwrap_or_default();
        let constructor_bindings = source
            .actor
            .state_fields
            .iter()
            .flat_map(|field| facts_in_span(&self.check.comptime_type_bindings, field.value.span()))
            .collect();

        let function_ids = self.function_ids.clone();
        let mut body_lowerer = BodyLowerer::new(
            self,
            &function_ids,
            self.check.type_map.clone(),
            self.check.debug_type_names.clone(),
            self.check.binding_modes.clone(),
            self.check.generic_calls.clone(),
            self.check.intrinsic_ids.clone(),
            self.check.intrinsic_type_arguments.clone(),
            self.check.intrinsic_reflection_arguments.clone(),
            self.check.call_argument_orders.clone(),
            self.check.method_calls.clone(),
            self.check.interface_calls.clone(),
            self.check.method_values.clone(),
            self.check.struct_constructions.clone(),
            self.check.pipeline_step_call_types.clone(),
            HashMap::new(),
            constructor_bindings,
        );
        let mut params = Vec::with_capacity(definition.capability_params.len());
        let mut fields =
            Vec::with_capacity(definition.capability_params.len() + definition.state_fields.len());
        for (param, (_, ty)) in source
            .actor
            .capability_params
            .iter()
            .zip(&definition.capability_params)
        {
            let Some(definition_id) = body_lowerer
                .parent
                .definition_at(param.name.span, DefKind::Param)
            else {
                body_lowerer
                    .parent
                    .error(param.name.span, "actor constructor parameter is unresolved");
                return None;
            };
            let local = body_lowerer.allocate_local(
                definition_id,
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
            fields.push((local, *ty, param.span));
        }
        let mut statements = Vec::with_capacity(fields.len() + source.actor.state_fields.len() + 1);
        // The interpreter retains constructor arguments in the actor and gives
        // state initializers a separate working environment. Preserve that
        // capture before any initializer consumes its parameter binding.
        for (local, ty, span) in &mut fields {
            let original = *local;
            let captured = LocalId(body_lowerer.locals.len() as u32);
            let mut metadata = body_lowerer.locals[original.index() as usize].clone();
            metadata.id = captured;
            metadata.name = format!("$actor.capture.{}", original.index());
            metadata.view_source = None;
            body_lowerer.locals.push(metadata);
            statements.push(Statement {
                kind: StatementKind::Let {
                    local: captured,
                    value: Expression {
                        kind: ExpressionKind::Clone(Box::new(Expression {
                            kind: ExpressionKind::Local(original),
                            ty: *ty,
                            span: *span,
                        })),
                        ty: *ty,
                        span: *span,
                    },
                },
                span: *span,
            });
            *local = captured;
        }
        for (field, (_, ty)) in source
            .actor
            .state_fields
            .iter()
            .zip(&definition.state_fields)
        {
            let value = body_lowerer.lower_expression(&field.value)?;
            let Some(definition_id) = body_lowerer
                .parent
                .definition_at(field.name.span, DefKind::Variable)
            else {
                body_lowerer
                    .parent
                    .error(field.name.span, "actor state field is unresolved");
                return None;
            };
            let local = body_lowerer.allocate_local(
                definition_id,
                &field.name.name,
                *ty,
                field.mutable,
                field.span,
            );
            statements.push(Statement {
                kind: StatementKind::Let { local, value },
                span: field.span,
            });
            fields.push((local, *ty, field.span));
        }
        body_lowerer.reject_unconsumed_static_selections();
        body_lowerer.reject_unconsumed_comptime_type_bindings();
        let args = fields
            .into_iter()
            .map(|(local, ty, span)| Expression {
                kind: ExpressionKind::Local(local),
                ty,
                span,
            })
            .collect::<Vec<_>>();
        let evaluation_order = (0..args.len()).collect();
        statements.push(Statement {
            kind: StatementKind::Return(Some(Expression {
                kind: ExpressionKind::ActorSpawn {
                    actor_type: definition.name,
                    args,
                    evaluation_order,
                    constructor: None,
                },
                ty: source.ty,
                span: source.actor.span,
            })),
            span: source.actor.span,
        });
        let debug_kind = FunctionDebugKind::named(&namespace, &source.actor.name.name);
        Some(Function {
            id: source.id,
            identity: FunctionIdentity {
                declaration: DeclarationId {
                    origin,
                    namespace,
                    name: source.actor.name.name.clone(),
                    kind: DeclarationKind::ActorConstructor,
                },
                scoped_type_bindings: Vec::new(),
                type_arguments: Vec::new(),
                specialization: CheckedGenericSpecialization::default(),
            },
            debug_kind,
            source_definition: None,
            params,
            capture_count: 0,
            return_type: source.ty,
            locals: body_lowerer.locals,
            body: Block {
                statements,
                span: source.actor.span,
            },
            span: source.actor.span,
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
        let debug_type_names = self.check.debug_type_names.clone();
        let binding_modes = self.check.binding_modes.clone();
        let generic_calls = self.check.generic_calls.clone();
        let intrinsic_ids = self.check.intrinsic_ids.clone();
        let intrinsic_type_arguments = self.check.intrinsic_type_arguments.clone();
        let intrinsic_reflection_arguments = self.check.intrinsic_reflection_arguments.clone();
        let call_argument_orders = self.check.call_argument_orders.clone();
        let method_calls = self.check.method_calls.clone();
        let interface_calls = self.check.interface_calls.clone();
        let method_values = self.check.method_values.clone();
        let struct_constructions = self.check.struct_constructions.clone();
        let pipeline_step_call_types = self.check.pipeline_step_call_types.clone();
        let static_selections = HashMap::new();
        let comptime_type_bindings =
            facts_in_span(&self.check.comptime_type_bindings, source.handler.span);
        let mut body_lowerer = BodyLowerer::new(
            self,
            &function_ids,
            expression_types,
            debug_type_names,
            binding_modes,
            generic_calls,
            intrinsic_ids,
            intrinsic_type_arguments,
            intrinsic_reflection_arguments,
            call_argument_orders,
            method_calls,
            interface_calls,
            method_values,
            struct_constructions,
            pipeline_step_call_types,
            static_selections,
            comptime_type_bindings,
        );

        // Handler state is an input snapshot, not an uninitialized local.
        // Captured parameters are supplied through the actor environment;
        // message parameters follow them in the ordinary call signature.
        let mut params = Vec::with_capacity(
            source.actor.capability_params.len()
                + source.actor.state_fields.len()
                + source.handler.params.len(),
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
            let local = body_lowerer.allocate_local(
                definition,
                &field.name.name,
                *ty,
                field.mutable,
                field.span,
            );
            params.push(Param {
                local,
                name: field.name.name.clone(),
                ty: *ty,
                mode: ParamMode::Owned,
                mutable: field.mutable,
                span: field.span,
            });
        }
        let capture_count = params.len();
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
                scoped_type_bindings: Vec::new(),
                type_arguments: Vec::new(),
                specialization: CheckedGenericSpecialization::default(),
            },
            debug_kind: FunctionDebugKind::named(&actor_def.name, &source.handler.name.name),
            source_definition: None,
            params,
            capture_count,
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
            self.check.debug_type_names.clone(),
            self.check.binding_modes.clone(),
            self.check.generic_calls.clone(),
            self.check.intrinsic_ids.clone(),
            self.check.intrinsic_type_arguments.clone(),
            self.check.intrinsic_reflection_arguments.clone(),
            self.check.call_argument_orders.clone(),
            self.check.method_calls.clone(),
            self.check.interface_calls.clone(),
            self.check.method_values.clone(),
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
                scoped_type_bindings: Vec::new(),
                type_arguments: Vec::new(),
                specialization: CheckedGenericSpecialization::default(),
            },
            debug_kind: FunctionDebugKind::named(
                declared.namespace.as_deref().unwrap_or_default(),
                &source.alias.name.name,
            ),
            source_definition: Some(source.definition),
            params: vec![Param {
                local,
                name: "value".to_string(),
                ty: input_type,
                mode: ParamMode::Owned,
                mutable: false,
                span: constraint.span(),
            }],
            capture_count: 0,
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
    debug_type_names: HashMap<Span, String>,
    binding_modes: HashMap<Span, CheckedBindingMode>,
    generic_calls: HashMap<Span, CheckedGenericCall>,
    intrinsic_ids: HashMap<Span, IntrinsicId>,
    intrinsic_type_arguments: HashMap<Span, Vec<TypeId>>,
    intrinsic_reflection_arguments: HashMap<Span, Vec<ReflectionTypeInfo>>,
    call_argument_orders: HashMap<Span, CheckedCallArgumentOrder>,
    method_calls: HashMap<Span, CheckedMethodCall>,
    interface_calls: HashMap<Span, CheckedInterfaceCall>,
    method_values: HashMap<Span, CheckedMethodValue>,
    struct_constructions: HashMap<Span, CheckedStructConstruction>,
    pipeline_step_call_types: HashMap<Span, TypeId>,
    static_selections: HashMap<Span, CheckedStaticSelection>,
    comptime_type_bindings: HashMap<Span, Vec<CheckedComptimeTypeBinding>>,
    consumed_static_selections: HashSet<Span>,
    consumed_comptime_type_bindings: HashSet<Span>,
    local_ids: HashMap<DefId, LocalId>,
    locals: Vec<Local>,
    visible_bindings: Vec<HashMap<String, LocalId>>,
    scoped_type_bindings: Vec<ScopedTypeBinding>,
    return_type: Option<TypeId>,
}

impl<'lowerer, 'program> BodyLowerer<'lowerer, 'program> {
    fn new(
        parent: &'lowerer mut Lowerer<'program>,
        function_ids: &'lowerer HashMap<FunctionKey, FunctionId>,
        expression_types: HashMap<Span, TypeId>,
        debug_type_names: HashMap<Span, String>,
        binding_modes: HashMap<Span, CheckedBindingMode>,
        generic_calls: HashMap<Span, CheckedGenericCall>,
        intrinsic_ids: HashMap<Span, IntrinsicId>,
        intrinsic_type_arguments: HashMap<Span, Vec<TypeId>>,
        intrinsic_reflection_arguments: HashMap<Span, Vec<ReflectionTypeInfo>>,
        call_argument_orders: HashMap<Span, CheckedCallArgumentOrder>,
        method_calls: HashMap<Span, CheckedMethodCall>,
        interface_calls: HashMap<Span, CheckedInterfaceCall>,
        method_values: HashMap<Span, CheckedMethodValue>,
        struct_constructions: HashMap<Span, CheckedStructConstruction>,
        pipeline_step_call_types: HashMap<Span, TypeId>,
        static_selections: HashMap<Span, CheckedStaticSelection>,
        comptime_type_bindings: HashMap<Span, Vec<CheckedComptimeTypeBinding>>,
    ) -> Self {
        Self {
            parent,
            function_ids,
            expression_types,
            debug_type_names,
            binding_modes,
            generic_calls,
            intrinsic_ids,
            intrinsic_type_arguments,
            intrinsic_reflection_arguments,
            call_argument_orders,
            method_calls,
            interface_calls,
            method_values,
            struct_constructions,
            pipeline_step_call_types,
            static_selections,
            comptime_type_bindings,
            consumed_static_selections: HashSet::new(),
            consumed_comptime_type_bindings: HashSet::new(),
            local_ids: HashMap::new(),
            locals: Vec::new(),
            visible_bindings: vec![HashMap::new()],
            scoped_type_bindings: Vec::new(),
            return_type: None,
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

    fn consume_omitted_body_facts(&mut self, body_span: Span) {
        let in_body = |span: &Span| {
            span.file == body_span.file
                && span.start >= body_span.start
                && span.end <= body_span.end
        };
        self.consumed_static_selections
            .extend(self.static_selections.keys().copied().filter(in_body));
        self.consumed_comptime_type_bindings
            .extend(self.comptime_type_bindings.keys().copied().filter(in_body));
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
            debug_ty: self
                .parent
                .check
                .definition_types
                .get(&definition)
                .copied()
                .unwrap_or(ty),
            debug_type_name: self
                .debug_type_names
                .get(&self.parent.resolve.scope_table.def(definition).span)
                .cloned(),
            mutable,
            view_source: None,
            span,
        });
        id
    }

    fn local_view_source(
        &mut self,
        declaration: &ast::VarDecl,
        value: &Expression,
        ty: TypeId,
    ) -> Option<Option<LocalId>> {
        let Some(mode) = self.binding_modes.get(&declaration.name.span).copied() else {
            self.parent.error(
                declaration.name.span,
                "local declaration has no checked ownership mode",
            );
            return None;
        };
        let CheckedBindingMode::View { source } = mode else {
            return Some(None);
        };
        let CheckedViewSource::Binding(definition) = source else {
            self.parent.error(declaration.value.span(), "native borrowed alias requires a stable local origin; temporary views remain unsupported");
            return None;
        };
        let Some(source) = self.local_ids.get(&definition).copied() else {
            self.parent.error(
                declaration.value.span(),
                "native borrowed alias source is not a local binding",
            );
            return None;
        };
        if declaration.mutable || !local_views::immutable_view_chain(&self.locals, source) {
            self.parent.error(
                declaration.span,
                "native borrowed alias requires immutable bindings along its stable origin",
            );
            return None;
        }
        if let Err(message) = local_views::validate_local_view_initializer(
            value,
            source,
            self.locals[source.index() as usize].ty,
            ty,
            &self.parent.check.interner,
        ) {
            self.parent.error(declaration.value.span(), message);
            return None;
        }
        Some(Some(source))
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
                let Some(ty) =
                    self.expression_types
                        .get(&decl.name.span)
                        .copied()
                        .filter(|ty| {
                            // Bare machine annotations erase precise state in
                            // the binding while its producer keeps that state.
                            // Secret qualification also preserves both sides.
                            matches!(
                                self.parent.check.interner.resolve(*ty),
                                Type::Machine(_) | Type::Secret(_)
                            ) || interface_values::contains_erased_boundary(
                                &self.parent.check.interner,
                                *ty,
                            ) || self.expression_types.get(&decl.value.span()).is_some_and(
                                |actual| {
                                    interface_values::contains_erased_boundary(
                                        &self.parent.check.interner,
                                        *actual,
                                    )
                                },
                            )
                        })
                        .or_else(|| self.expression_types.get(&decl.value.span()).copied())
                        .or_else(|| self.parent.check.definition_types.get(&definition).copied())
                else {
                    self.parent
                        .error(decl.name.span, "local has no checked type");
                    return None;
                };
                // The new binding becomes visible only after its initializer,
                // including any handled failure and nested breakpoint.
                let value = self.lower_expression(&decl.value)?;
                let view_source = self.local_view_source(decl, &value, ty)?;
                let local =
                    self.allocate_local(definition, &decl.name.name, ty, decl.mutable, decl.span);
                self.locals[local.index() as usize].view_source = view_source;
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
                    Some(value) => {
                        let value = self.lower_expression(value)?;
                        Some(self.refine_return_value(value, ret.span)?)
                    }
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
                let body = if matches!(
                    self.parent.check.interner.resolve(iterable.ty),
                    Type::List(element) if *element == TypeInterner::NEVER
                ) || matches!(
                    self.parent.check.interner.resolve(iterable.ty),
                    Type::Map(key, value)
                        if *key == TypeInterner::NEVER || *value == TypeInterner::NEVER
                ) {
                    // The frontend still checks this body. An uninhabited
                    // element type proves that no iteration can enter it;
                    // retain the iterable so MIR validates pending values.
                    self.consume_omitted_body_facts(loop_stmt.body.span);
                    Block {
                        statements: Vec::new(),
                        span: loop_stmt.body.span,
                    }
                } else {
                    self.lower_block(&loop_stmt.body)
                };
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
                if let Some(declaration) = self.parent.constant_definitions.get(definition).copied()
                {
                    if self.parent.check.release {
                        return None;
                    }
                    return self.lower_constant_trace(trace, *definition, declaration);
                }
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
                if let Some(excluded) = self.parent.check.breakpoint_exclusions.get(&point.span) {
                    visible.retain(|name, _| !excluded.contains(*name));
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
        if self.parent.check.release
            && matches!(
                kind,
                StatementKind::Trace(_) | StatementKind::Breakpoint { .. }
            )
        {
            // Lower first to consume checked nested-body facts, then discard
            // the entire observation before MIR can expand its condition.
            return None;
        }
        Some(Statement { kind, span })
    }

    fn lower_constant_trace(
        &mut self,
        trace: &ast::TraceStmt,
        definition: DefId,
        declaration: Span,
    ) -> Option<Statement> {
        let Some(ty) = self.parent.check.definition_types.get(&definition).copied() else {
            self.parent
                .error(trace.span, "constant trace target has no checked type");
            return None;
        };
        // Materialize the baked value only for this observation. The temporary
        // is not a lexical binding and must not appear in later breakpoints.
        let local = LocalId(self.locals.len() as u32);
        self.locals.push(Local {
            id: local,
            name: trace.name.name.clone(),
            ty,
            debug_ty: ty,
            debug_type_name: self
                .parent
                .check
                .debug_type_names
                .get(&declaration)
                .cloned(),
            mutable: false,
            view_source: None,
            span: trace.span,
        });
        Some(Statement {
            kind: StatementKind::Scope(Block {
                statements: vec![
                    Statement {
                        kind: StatementKind::Let {
                            local,
                            value: Expression {
                                kind: ExpressionKind::Constant { declaration },
                                ty,
                                span: trace.name.span,
                            },
                        },
                        span: trace.span,
                    },
                    Statement {
                        kind: StatementKind::Trace(local),
                        span: trace.span,
                    },
                ],
                span: trace.span,
            }),
            span: trace.span,
        })
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
        self.consumed_static_selections
            .extend(self.static_selections.keys().copied().filter(|span| {
                span.file == binding.body.span.file
                    && span.start >= binding.body.span.start
                    && span.end <= binding.body.span.end
            }));
        self.consumed_comptime_type_bindings.extend(
            self.comptime_type_bindings.keys().copied().filter(|span| {
                span.file == binding.body.span.file
                    && span.start >= binding.body.span.start
                    && span.end <= binding.body.span.end
            }),
        );

        // A trusted reflection loop over no fields has no concrete body to
        // dispatch. The runtime loop has no iterations either.
        if bindings.is_empty() {
            return Some(StatementKind::Scope(Block {
                statements: Vec::new(),
                span: binding.body.span,
            }));
        }

        if bindings.len() == 1
            && bindings[0].selection == CheckedComptimeTypeSelection::Unconditional
        {
            let checked = bindings
                .into_iter()
                .next()
                .expect("single checked binding exists");
            return Some(StatementKind::Scope(
                self.lower_bound_type_body(binding, checked),
            ));
        }

        if bindings.iter().any(|checked| {
            !matches!(
                checked.selection,
                CheckedComptimeTypeSelection::ReflectedIteration(_)
            )
        }) {
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
            // Share an arm only when both canonical type and source-visible
            // reflection agree; aliases can specialize the body differently.
            let reflection_identity = checked.reflection.reflection_identity();
            if !bound_types.insert((checked.bound_type, reflection_identity.clone())) {
                continue;
            }
            arms.push(ReflectedTypeArm {
                iteration_index,
                bound_type: checked.bound_type,
                reflection_identity,
                body: self.lower_bound_type_body(binding, checked),
            });
        }
        Some(StatementKind::ReflectedTypeDispatch { type_info, arms })
    }

    fn lower_bound_type_body(
        &mut self,
        binding: &ast::ComptimeTypeBindStmt,
        checked: CheckedComptimeTypeBinding,
    ) -> Block {
        self.scoped_type_bindings.push(ScopedTypeBinding {
            name: binding.name.name.clone(),
            ty: checked.bound_type,
            reflection: checked.reflection.clone(),
        });
        let body = self.lower_block_with_checked_facts(&binding.body, checked.body);
        self.scoped_type_bindings.pop();
        body
    }

    fn lower_block_with_checked_facts(
        &mut self,
        block: &ast::Block,
        facts: CheckedBodyFacts,
    ) -> Block {
        let CheckedBodyFacts {
            type_map,
            debug_type_names,
            binding_modes,
            generic_calls,
            intrinsic_ids,
            intrinsic_type_arguments,
            intrinsic_reflection_arguments,
            call_argument_orders,
            method_calls,
            interface_calls,
            method_values,
            struct_constructions,
            pipeline_step_call_types,
            static_selections,
            comptime_type_bindings,
        } = facts;

        let saved_expression_types = std::mem::replace(&mut self.expression_types, type_map);
        let saved_debug_type_names =
            std::mem::replace(&mut self.debug_type_names, debug_type_names);
        let saved_binding_modes = std::mem::replace(&mut self.binding_modes, binding_modes);
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
        let saved_interface_calls = std::mem::replace(&mut self.interface_calls, interface_calls);
        let saved_method_values = std::mem::replace(&mut self.method_values, method_values);
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
        self.debug_type_names = saved_debug_type_names;
        self.binding_modes = saved_binding_modes;
        self.generic_calls = saved_generic_calls;
        self.intrinsic_ids = saved_intrinsic_ids;
        self.intrinsic_type_arguments = saved_intrinsic_type_arguments;
        self.intrinsic_reflection_arguments = saved_intrinsic_reflection_arguments;
        self.call_argument_orders = saved_call_argument_orders;
        self.method_calls = saved_method_calls;
        self.interface_calls = saved_interface_calls;
        self.method_values = saved_method_values;
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

    fn lower_interpolation_value(&mut self, expression: &Expr) -> Option<Expression> {
        let value = self.lower_expression(expression)?;
        let method_span = self
            .parent
            .check
            .method_definitions
            .iter()
            .find(|method| {
                method.owner_type == value.ty
                    && method.interface_name.as_deref() == Some("Displayable")
                    && method.method_name == "display"
                    && method.parameter_types == [value.ty]
                    && method.return_type == TypeInterner::STRING
            })
            .map(|method| method.source_span);
        let Some(method_span) = method_span else {
            if matches!(
                self.parent.check.interner.resolve(value.ty),
                Type::Int8
                    | Type::Int16
                    | Type::Int32
                    | Type::Int64
                    | Type::Uint8
                    | Type::Uint16
                    | Type::Uint32
                    | Type::Uint64
                    | Type::Float32
                    | Type::Float64
                    | Type::String
                    | Type::Bool
                    | Type::Nothing
            ) {
                return Some(value);
            }
            self.parent.error(
                expression.span(),
                "displayable interpolation has no checked display method",
            );
            return None;
        };
        let Some(function) = self
            .function_ids
            .get(&FunctionKey::Method {
                source_span: method_span,
            })
            .copied()
        else {
            self.parent.error(
                expression.span(),
                "displayable interpolation has no concrete HIR method",
            );
            return None;
        };
        Some(Expression {
            kind: ExpressionKind::DisplayResult(Box::new(Expression {
                kind: ExpressionKind::Call {
                    function,
                    args: vec![value],
                    evaluation_order: vec![0],
                },
                ty: TypeInterner::STRING,
                span: expression.span(),
            })),
            ty: TypeInterner::STRING,
            span: expression.span(),
        })
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
                } else if let Some(declaration) = self.parent.constant_definitions.get(&definition)
                {
                    ExpressionKind::Constant {
                        declaration: *declaration,
                    }
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
                let call = ExpressionKind::EquatableResult(Box::new(Expression {
                    kind: ExpressionKind::Call {
                        function,
                        args: vec![self.lower_expression(left)?, self.lower_expression(right)?],
                        evaluation_order: vec![0, 1],
                    },
                    ty,
                    span,
                }));
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
                if self.method_values.contains_key(&span)
                    || self.resolved_expression_kind(expression) == Some(DefKind::Function)
                {
                    ExpressionKind::FunctionRef(self.resolve_function_value_target(expression)?)
                } else if self.enum_constructor_variant(expression, ty).is_some() {
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
            Expr::StringInterpolation(parts, _) => {
                let mut segments = Vec::with_capacity(parts.len());
                for part in parts {
                    segments.push(match part {
                        ast::StringPart::Literal(text) => StringSegment::Text(text.clone()),
                        ast::StringPart::Expr(value) => {
                            StringSegment::Value(self.lower_interpolation_value(value)?)
                        }
                    });
                }
                ExpressionKind::StringInterpolation(segments)
            }
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
            Expr::Comptime(value, source_span) => ExpressionKind::Comptime {
                value: Box::new(self.lower_expression(value)?),
                bindings: self.scoped_type_bindings.clone(),
                source_span: *source_span,
            },
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
                let Type::Function {
                    params: parameter_types,
                    view_params: parameter_views,
                    return_type,
                    ..
                } = self.parent.check.interner.resolve(ty).clone()
                else {
                    self.parent
                        .error(span, "closure has no checked function signature");
                    return None;
                };
                if params.len() != parameter_types.len()
                    || params.len() != parameter_views.len()
                    || params
                        .iter()
                        .zip(&parameter_views)
                        .any(|(param, view)| param.view != *view)
                {
                    self.parent.error(
                        span,
                        "closure parameters disagree with its checked signature",
                    );
                    return None;
                }
                let local_floor = self.locals.len() as u32;
                self.visible_bindings.push(HashMap::new());
                let mut lowered_params = Vec::with_capacity(params.len());
                let mut view_params = Vec::new();
                for (param, param_type) in params.iter().zip(parameter_types) {
                    let Some(definition) =
                        self.parent.definition_at(param.name.span, DefKind::Param)
                    else {
                        self.parent
                            .error(param.name.span, "closure parameter is unresolved");
                        return None;
                    };
                    // A generic instantiation's expression signature contains
                    // its concrete parameter types; global definition metadata
                    // shares source DefIds across specializations.
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
                let enclosing_return_type = self.return_type.replace(return_type);
                let body = self.lower_block(body);
                self.return_type = enclosing_return_type;
                self.visible_bindings.pop();
                ExpressionKind::InlineFunction {
                    scoped_type_bindings: self.scoped_type_bindings.clone(),
                    params: lowered_params,
                    view_params,
                    local_floor,
                    body,
                }
            }
            Expr::Spawn(inner, _) => self.lower_actor_spawn(inner, ty)?,
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
        let lowered = Expression { kind, ty, span };
        let checked_constructor_call = match expression {
            Expr::Call(callee, ..) | Expr::GenericCall(callee, ..) => {
                let callee = Self::unparenthesized(callee);
                self.struct_constructions.contains_key(&span)
                    || self.is_declaration_reference(callee, DefKind::Bitfield)
                    || self.is_declaration_reference(callee, DefKind::Machine)
            }
            _ => false,
        };
        Some(
            if matches!(
                expression,
                Expr::ListConstruct(..)
                    | Expr::MapConstruct(..)
                    | Expr::Some(..)
                    | Expr::None(..)
                    | Expr::Ok(..)
                    | Expr::Fail(..)
            ) || checked_constructor_call
            {
                normalize_secret_constructor(lowered, &self.parent.check.interner)
            } else {
                lowered
            },
        )
    }

    fn lower_actor_spawn(&mut self, inner: &Expr, ty: TypeId) -> Option<ExpressionKind> {
        let (callee, args, call_span) = match inner {
            Expr::Call(callee, args, span) => (callee.as_ref(), args.as_slice(), *span),
            _ => (inner, &[][..], inner.span()),
        };
        let actor_type = self.dotted_expression_name(callee)?;
        let (args, evaluation_order) = self.lower_arguments_in_parameter_order(args, call_span)?;
        let Some(&constructor) = self.parent.actor_constructor_ids.get(&ty) else {
            self.parent
                .error(call_span, "actor spawn has no checked constructor");
            return None;
        };
        Some(ExpressionKind::ActorSpawn {
            actor_type,
            args,
            evaluation_order,
            constructor: Some(constructor),
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
        let Some(&handler) = self
            .parent
            .actor_handler_ids
            .get(&(actor.ty, message.name.clone()))
        else {
            self.parent
                .error(message.span, "actor message has no checked handler");
            return None;
        };
        let (args, evaluation_order) = self.lower_arguments_in_parameter_order(args, call_span)?;
        Some(ExpressionKind::ActorMessage {
            actor,
            message: message.name.clone(),
            handler,
            args,
            evaluation_order,
            kind,
        })
    }

    fn refine_return_value(&mut self, value: Expression, span: Span) -> Option<Expression> {
        let Some(return_type) = self.return_type else {
            return Some(value);
        };
        if value.ty == return_type
            || value.ty == TypeInterner::NEVER
            || !matches!(
                self.parent.check.interner.resolve(return_type),
                Type::Refinement { .. }
            )
        {
            return Some(value);
        }
        let mut predicates = self.checked_refinement_predicates(return_type, span)?;
        if let Some(index) = predicates
            .iter()
            .position(|predicate| predicate.refined_type == value.ty)
        {
            predicates.drain(..=index);
        }
        let error_local = LocalId(self.locals.len() as u32);
        self.locals.push(Local {
            id: error_local,
            name: "$return.error".to_string(),
            ty: TypeInterner::STRING,
            debug_ty: TypeInterner::STRING,
            debug_type_name: None,
            mutable: false,
            view_source: None,
            span,
        });
        let failure = Block {
            statements: vec![Statement {
                kind: StatementKind::Expression(Expression {
                    kind: ExpressionKind::RuntimeFailureMessage(Box::new(Expression {
                        kind: ExpressionKind::Local(error_local),
                        ty: TypeInterner::STRING,
                        span,
                    })),
                    ty: TypeInterner::NOTHING,
                    span,
                }),
                span,
            }],
            span,
        };
        Some(Expression {
            kind: ExpressionKind::Handle {
                target: Box::new(value),
                kind: HandleKind::Refinement {
                    refined_type: return_type,
                    predicates,
                },
                error_local: Some(error_local),
                failure,
            },
            ty: return_type,
            span,
        })
    }

    fn checked_refinement_predicates(
        &mut self,
        refined_type: TypeId,
        span: Span,
    ) -> Option<Vec<RefinementPredicate>> {
        let mut predicates = Vec::new();
        let mut base_type = refined_type;
        while let Type::Refinement { base, .. } = self.parent.check.interner.resolve(base_type) {
            base_type = *base;
        }
        let input_type = if let Type::Secret(inner) = self.parent.check.interner.resolve(base_type)
        {
            *inner
        } else {
            base_type
        };
        let mut current = refined_type;
        while let Type::Refinement { name, base } = self.parent.check.interner.resolve(current) {
            let Some(function) = self.parent.refinement_function_ids.get(&current).copied() else {
                self.parent
                    .error(span, "refinement has no checked predicate function");
                return None;
            };
            predicates.push(RefinementPredicate {
                refined_type: current,
                type_name: name.clone(),
                function,
                base_type,
                input_type,
            });
            current = *base;
        }
        predicates.reverse();
        Some(predicates)
    }

    fn reflected_finish_predicates(
        &mut self,
        intrinsic: IntrinsicId,
        type_arguments: &[TypeId],
        span: Span,
    ) -> Option<Vec<Vec<RefinementPredicate>>> {
        if intrinsic != IntrinsicId::TypeConstructFinish || type_arguments.len() != 1 {
            return Some(Vec::new());
        }
        let field_types = match self.parent.check.interner.resolve(type_arguments[0]) {
            Type::Struct(id) => self
                .parent
                .check
                .interner
                .resolve_struct(*id)
                .fields
                .iter()
                .map(|(_, ty)| *ty)
                .collect::<Vec<_>>(),
            Type::Enum(id) => self
                .parent
                .check
                .interner
                .resolve_enum(*id)
                .variants
                .iter()
                .flat_map(|variant| variant.fields.iter().map(|(_, ty)| *ty))
                .collect::<Vec<_>>(),
            Type::Machine(id) => self
                .parent
                .check
                .interner
                .resolve_machine(*id)
                .states
                .iter()
                .flat_map(|state| state.fields.iter().map(|(_, ty)| *ty))
                .collect::<Vec<_>>(),
            Type::MachineState { machine, state } => self
                .parent
                .check
                .interner
                .resolve_machine(*machine)
                .state(*state)
                .expect("checked machine state")
                .fields
                .iter()
                .map(|(_, ty)| *ty)
                .collect::<Vec<_>>(),
            _ => return Some(Vec::new()),
        };
        if !field_types.iter().any(|ty| {
            matches!(
                self.parent.check.interner.resolve(*ty),
                Type::Refinement { .. }
            )
        }) {
            return Some(Vec::new());
        }
        field_types
            .into_iter()
            .map(|ty| {
                if matches!(
                    self.parent.check.interner.resolve(ty),
                    Type::Refinement { .. }
                ) {
                    self.checked_refinement_predicates(ty, span)
                } else {
                    Some(Vec::new())
                }
            })
            .collect()
    }

    fn lower_handle(
        &mut self,
        target: Expression,
        error_name: Option<&ast::Ident>,
        failure: &ast::Block,
        output_type: TypeId,
        span: Span,
    ) -> Option<Expression> {
        let refinement_predicates = if matches!(
            self.parent.check.interner.resolve(output_type),
            Type::Refinement { .. }
        ) {
            Some(self.checked_refinement_predicates(output_type, span)?)
        } else {
            None
        };
        let refines_whole_sum = refinement_predicates.as_ref().is_some_and(|predicates| {
            predicates.first().is_some_and(|predicate| {
                target.ty == predicate.base_type || target.ty == predicate.input_type
            })
        });
        let kind = match (
            self.parent.check.interner.resolve(target.ty),
            refinement_predicates,
        ) {
            (Type::Result(_, _), _) if !refines_whole_sum => HandleKind::Result,
            (Type::Optional(_), _) if !refines_whole_sum => HandleKind::Optional,
            (_, Some(mut predicates)) => {
                if let Some(index) = predicates
                    .iter()
                    .position(|predicate| predicate.refined_type == target.ty)
                {
                    predicates.drain(..=index);
                }
                HandleKind::Refinement {
                    refined_type: output_type,
                    predicates,
                }
            }
            (_, None) => {
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
                _ if matches!(kind, HandleKind::Refinement { .. }) => Some(TypeInterner::STRING),
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
        if let Some(call) = self.interface_calls.get(&call_span) {
            let key = FunctionKey::Interface {
                owner: call.interface_type,
                method: call.method_index,
            };
            let function = self.function_ids.get(&key).copied()?;
            let (args, evaluation_order) =
                self.lower_arguments_in_parameter_order(args, call_span)?;
            return Some(ExpressionKind::Call {
                function,
                args,
                evaluation_order,
            });
        }
        // Call checking treats parentheses as transparent when selecting a
        // declaration, intrinsic, or function-value signature. Its checked
        // callee type and definition therefore belong to the inner expression.
        let callee = Self::unparenthesized(callee);
        let enum_variant = self
            .expression_types
            .get(&call_span)
            .and_then(|ty| self.enum_constructor_variant(callee, *ty));
        if let Some(variant) = enum_variant {
            let ty = self.expression_types.get(&call_span).copied().or_else(|| {
                self.parent
                    .error(call_span, "enum construction has no checked type");
                None
            })?;
            return self.lower_enum_construct(ty, variant, args, call_span);
        }
        if self.is_declaration_reference(callee, DefKind::Bitfield) {
            return self.lower_bitfield_construct(callee, args, call_span);
        }
        if let Expr::FieldAccess(base, member, _) = callee
            && member.name == "transition"
            && (self.resolved_expression_kind(base) == Some(DefKind::Machine)
                || self.resolved_expression_kind(callee) == Some(DefKind::Machine))
        {
            return self.lower_machine_transition(args, call_span);
        }
        if self.is_declaration_reference(callee, DefKind::Machine) {
            return self.lower_machine_construct(callee, args, call_span);
        }
        if let Some(construction) = self.struct_constructions.get(&call_span).cloned() {
            let (fields, evaluation_order) =
                self.lower_arguments_in_parameter_order(args, call_span)?;
            let refinement_predicates = if construction.validates_refinements {
                let Type::Struct(id) = self.parent.check.interner.resolve(construction.struct_type)
                else {
                    self.parent.error(
                        call_span,
                        "checked struct constructor target is not a struct",
                    );
                    return None;
                };
                let field_types = self
                    .parent
                    .check
                    .interner
                    .resolve_struct(*id)
                    .fields
                    .clone();
                if field_types.len() != fields.len() {
                    self.parent
                        .error(call_span, "checked struct constructor field count changed");
                    return None;
                }
                let mut chains = Vec::with_capacity(fields.len());
                for (field, (_, expected)) in fields.iter().zip(field_types) {
                    if field.ty != expected
                        && matches!(
                            self.parent.check.interner.resolve(expected),
                            Type::Refinement { .. }
                        )
                    {
                        let mut predicates =
                            self.checked_refinement_predicates(expected, call_span)?;
                        if let Some(index) = predicates
                            .iter()
                            .position(|predicate| predicate.refined_type == field.ty)
                        {
                            predicates.drain(..=index);
                        }
                        chains.push(predicates);
                    } else {
                        chains.push(Vec::new());
                    }
                }
                chains
            } else {
                Vec::new()
            };
            return Some(ExpressionKind::StructConstruct {
                struct_type: construction.struct_type,
                fields,
                evaluation_order,
                validates_refinements: construction.validates_refinements,
                refinement_predicates,
            });
        }

        let (mut lowered_args, mut evaluation_order) =
            self.lower_arguments_in_parameter_order(args, call_span)?;
        let source_call = self.is_source_call(callee, call_span);
        // A callee can be any checked function value, including a projected
        // field or another call's result. Declaration and intrinsic identities
        // still select their existing direct paths.
        if !source_call
            && !self.intrinsic_ids.contains_key(&call_span)
            && self.is_checked_function_value(callee)
        {
            return Some(ExpressionKind::IndirectCall {
                callee: Box::new(self.lower_expression(callee)?),
                args: lowered_args,
                evaluation_order,
            });
        }
        if source_call {
            Some(ExpressionKind::Call {
                function: self.resolve_user_call_target(callee, call_span)?,
                args: lowered_args,
                evaluation_order,
            })
        } else {
            let intrinsic = self.checked_intrinsic_id(call_span)?;
            let type_arguments =
                self.checked_intrinsic_type_arguments(call_span, has_explicit_type_arguments)?;
            let mut reflection_arguments = self
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
            let enum_json_source = match intrinsic {
                IntrinsicId::JsonSerialize | IntrinsicId::JsonSerializePublic => {
                    Some("json_serialize_native_enum")
                }
                IntrinsicId::JsonParse => Some("json_parse_native_enum"),
                IntrinsicId::JsonParseExact => Some("json_parse_exact_native_enum"),
                _ => None,
            };
            if let Some(name) = enum_json_source
                && type_arguments.len() == 1
                && lowered_args.len() == 1
                && self.is_non_raw_json_enum(type_arguments[0])
                && let Some(function) =
                    self.trusted_stdlib_generic_function("json", name, call_span)
            {
                return Some(ExpressionKind::Call {
                    function,
                    args: lowered_args,
                    evaluation_order,
                });
            }
            let bitfield_json_source = match intrinsic {
                IntrinsicId::JsonSerialize | IntrinsicId::JsonSerializePublic => {
                    Some("json_serialize_native_bitfield")
                }
                IntrinsicId::JsonParse => Some("json_parse_native_bitfield"),
                IntrinsicId::JsonParseExact => Some("json_parse_exact_native_bitfield"),
                _ => None,
            };
            if let Some(name) = bitfield_json_source
                && type_arguments.len() == 1
                && lowered_args.len() == 1
                && matches!(
                    self.parent.check.interner.resolve(type_arguments[0]),
                    Type::Bitfield(_)
                )
                && let Some(function) =
                    self.trusted_stdlib_generic_function("json", name, call_span)
            {
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
                && self.is_secret_raw_json_tree(type_arguments[0])
            {
                let Some(function) =
                    self.trusted_stdlib_function("json", "json_parse_native_secret_tree")
                else {
                    self.parent
                        .error(call_span, "trusted secret raw JSON source is missing");
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
                && matches!(
                    self.parent.check.interner.resolve(type_arguments[0]),
                    Type::Bytes
                )
            {
                let Some(function) =
                    self.trusted_stdlib_function("json", "json_serialize_native_bytes")
                else {
                    self.parent
                        .error(call_span, "trusted bytes JSON serializer is missing");
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
            if intrinsic == IntrinsicId::TypeConstructStart
                && type_arguments.len() == 1
                && reflection_arguments
                    .first()
                    .is_some_and(|info| matches!(info.kind.as_str(), "struct" | "bitfield"))
            {
                let Some(fields) = self
                    .parent
                    .check
                    .reflection_metadata
                    .get_type_fields_for_id(type_arguments[0])
                else {
                    self.parent.error(
                        call_span,
                        "type.construct_start has no checked field metadata",
                    );
                    return None;
                };
                reflection_arguments.extend(fields.iter().map(|field| field.type_info.clone()));
            }
            if intrinsic == IntrinsicId::TypeConstructVariantStart
                && type_arguments.len() == 1
                && reflection_arguments
                    .first()
                    .is_some_and(|info| info.kind == "enum")
            {
                let Some(variants) = self
                    .parent
                    .check
                    .reflection_metadata
                    .get_type_variants_for_id(type_arguments[0])
                else {
                    self.parent.error(
                        call_span,
                        "type.construct_variant_start has no checked variant metadata",
                    );
                    return None;
                };
                reflection_arguments.extend(variants.iter().flat_map(|variant| {
                    variant.fields.iter().map(|field| field.type_info.clone())
                }));
            }
            if intrinsic == IntrinsicId::TypeConstructMachineStart
                && type_arguments.len() == 1
                && reflection_arguments
                    .first()
                    .is_some_and(|info| matches!(info.kind.as_str(), "machine" | "machine_state"))
            {
                let machine = self.checked_reflection_machine(type_arguments[0], call_span)?;
                reflection_arguments.extend(
                    machine
                        .states
                        .iter()
                        .flat_map(|state| state.fields.iter().map(|field| field.type_info.clone())),
                );
            }
            self.append_reflected_value_arguments(
                intrinsic,
                &type_arguments,
                &reflection_arguments,
                &mut lowered_args,
                &mut evaluation_order,
                self.expression_types.get(&call_span).copied(),
                call_span,
            )?;
            self.append_reflected_read_arguments(
                intrinsic,
                &type_arguments,
                &reflection_arguments,
                &mut lowered_args,
                &mut evaluation_order,
                call_span,
            )?;
            let refinement_predicates =
                self.reflected_finish_predicates(intrinsic, &type_arguments, call_span)?;
            let field_validation =
                self.reflected_read_validation(intrinsic, &type_arguments, call_span)?;
            Some(ExpressionKind::Intrinsic {
                intrinsic,
                type_arguments,
                reflection_arguments,
                refinement_predicates,
                field_validation,
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
                evaluation_order: Vec::new(),
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
            refinement_predicates: Vec::new(),
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
            refinement_predicates: Vec::new(),
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
            refinement_predicates: Vec::new(),
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
            refinement_predicates: Vec::new(),
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
            refinement_predicates: Vec::new(),
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
            refinement_predicates: Vec::new(),
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
            refinement_predicates: Vec::new(),
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
            refinement_predicates: Vec::new(),
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
        let enum_type =
            interface_values::representation_type(&self.parent.check.interner, enum_type);
        let Some(index) = self.enum_variant_index(enum_type, variant) else {
            self.parent
                .error(span, "checked enum construction has no matching variant");
            return None;
        };
        let (payloads, evaluation_order) = self.lower_arguments_in_parameter_order(args, span)?;
        Some(ExpressionKind::EnumConstruct {
            enum_type,
            variant: VariantId(index as u32),
            payloads,
            evaluation_order,
        })
    }

    fn enum_variant_index(&self, enum_type: TypeId, variant: &ast::Ident) -> Option<usize> {
        let enum_type =
            interface_values::representation_type(&self.parent.check.interner, enum_type);
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

    fn enum_constructor_variant<'b>(
        &self,
        callee: &'b Expr,
        output_type: TypeId,
    ) -> Option<&'b ast::Ident> {
        // Compiler-predefined enum names resolve as constants and have no
        // checked value expression. Actual fields keep a checked base.
        match callee {
            Expr::EnumVariant(_, variant, _) => Some(variant),
            Expr::FieldAccess(base, variant, _)
                if (matches!(
                    self.resolved_expression_kind(base),
                    Some(DefKind::Enum | DefKind::Type)
                ) || matches!(
                    self.resolved_expression_kind(callee),
                    Some(DefKind::Enum | DefKind::Type)
                ) || (matches!(
                    self.resolved_expression_kind(base),
                    None | Some(DefKind::Constant)
                ) && !self.expression_types.contains_key(&base.span())))
                    && self.enum_variant_index(output_type, variant).is_some() =>
            {
                Some(variant)
            }
            _ => None,
        }
    }

    fn is_checked_function_value(&self, expression: &Expr) -> bool {
        self.expression_types
            .get(&expression.span())
            .is_some_and(|ty| {
                matches!(
                    self.parent.check.interner.resolve(*ty),
                    Type::Function { .. }
                )
            })
    }

    fn unparenthesized(mut expression: &Expr) -> &Expr {
        while let Expr::Paren(inner, _) = expression {
            expression = inner;
        }
        expression
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
            let redirected_json = self.is_trusted_stdlib_intrinsic(definition, span)
                && matches!(
                    self.intrinsic_ids.get(&span).copied(),
                    Some(
                        IntrinsicId::JsonParse
                            | IntrinsicId::JsonParseExact
                            | IntrinsicId::JsonSerialize
                            | IntrinsicId::JsonSerializePublic
                    )
                );
            return !redirected_json;
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

    fn is_secret_raw_json_tree(&self, ty: TypeId) -> bool {
        matches!(self.parent.check.interner.resolve(ty), Type::Secret(inner) if self.is_raw_json_tree(*inner))
    }

    fn is_non_raw_json_enum(&self, ty: TypeId) -> bool {
        matches!(self.parent.check.interner.resolve(ty), Type::Enum(_))
            && !self.is_raw_json_tree(ty)
    }

    fn native_json_primitive_parser(&self, ty: TypeId) -> Option<&'static str> {
        match self.parent.check.interner.resolve(ty) {
            Type::String => Some("json_parse_native_string"),
            Type::Bool => Some("json_parse_native_bool"),
            Type::Int64 => Some("json_parse_native_int64"),
            Type::Uint8 => Some("json_parse_native_uint8"),
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
        if matches!(self.parent.check.interner.resolve(ty), Type::Nothing) {
            return Some(ExpressionKind::Call {
                function: self.trusted_stdlib_function("json", "json_serialize_native_nothing")?,
                args: vec![value.clone()],
                evaluation_order: vec![0],
            });
        }
        if matches!(self.parent.check.interner.resolve(ty), Type::Bool) {
            return Some(ExpressionKind::StringInterpolation(vec![
                StringSegment::Value(value.clone()),
            ]));
        }
        let (variant_name, payload) = match self.parent.check.interner.resolve(ty) {
            Type::String => (
                "string_value",
                Some(Expression {
                    kind: ExpressionKind::Clone(Box::new(value.clone())),
                    ty,
                    span,
                }),
            ),
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
        let payloads = payload.into_iter().collect::<Vec<_>>();
        let tree = Expression {
            kind: ExpressionKind::EnumConstruct {
                enum_type: tree_type,
                variant: VariantId(variant as u32),
                evaluation_order: (0..payloads.len()).collect(),
                payloads,
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

    fn trusted_stdlib_generic_function(
        &self,
        namespace: &str,
        name: &str,
        span: Span,
    ) -> Option<FunctionId> {
        let checked = self.generic_calls.get(&span)?;
        self.parent.module.items.iter().find_map(|item| {
            let Item::Function(function) = item else {
                return None;
            };
            if function.name.name != name || function.type_params.is_empty() {
                return None;
            }
            let definition = self
                .parent
                .definition_at(function.name.span, DefKind::Function)?;
            if definition != checked.definition {
                return None;
            }
            let declared = self.parent.resolve.scope_table.def(definition);
            if declared.namespace.as_deref() != Some(namespace)
                || self.parent.origins.get(&function.name.span.file) != Some(&SourceOrigin::Stdlib)
            {
                return None;
            }
            self.function_ids
                .get(&FunctionKey::Definition {
                    definition,
                    concrete_args: checked.concrete_args.clone(),
                    specialization: checked.specialization.clone(),
                })
                .copied()
        })
    }

    fn lower_bitfield_construct(
        &mut self,
        callee: &Expr,
        args: &[ast::CallArg],
        span: Span,
    ) -> Option<ExpressionKind> {
        let checked_type = self.expression_types.get(&span).copied()?;
        let output_type =
            constructor_type_without_secret(&self.parent.check.interner, checked_type);
        let (bitfield_type, validates_widths) =
            match self.parent.check.interner.resolve(output_type) {
                Type::Bitfield(_) => (output_type, false),
                Type::Result(ok, error)
                    if *error == TypeInterner::STRING
                        && matches!(self.parent.check.interner.resolve(*ok), Type::Bitfield(_)) =>
                {
                    (*ok, true)
                }
                _ => {
                    self.parent
                        .error(span, "checked bitfield construction has an invalid type");
                    return None;
                }
            };
        let declared_type = self
            .resolved_definition(callee)
            .and_then(|definition| self.parent.check.definition_types.get(&definition).copied());
        if declared_type != Some(bitfield_type) {
            self.parent.error(
                span,
                "checked bitfield construction target differs from its declaration",
            );
            return None;
        }
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
        callee: &Expr,
        args: &[ast::CallArg],
        span: Span,
    ) -> Option<ExpressionKind> {
        let checked_type = self.expression_types.get(&span).copied()?;
        let state_type = constructor_type_without_secret(&self.parent.check.interner, checked_type);
        let Type::MachineState { machine, state } = *self.parent.check.interner.resolve(state_type)
        else {
            self.parent
                .error(span, "machine construction lacks a checked state type");
            return None;
        };
        let declared_type = self
            .resolved_definition(callee)
            .and_then(|definition| self.parent.check.definition_types.get(&definition).copied());
        if !matches!(declared_type.map(|ty| self.parent.check.interner.resolve(ty)), Some(Type::Machine(selected)) if *selected == machine)
        {
            self.parent.error(
                span,
                "checked machine construction target differs from its declaration",
            );
            return None;
        }
        let source_state = args.first().and_then(|arg| match &arg.value {
            Expr::Ident(name) if arg.name.is_none() => self
                .parent
                .check
                .interner
                .resolve_machine(machine)
                .state_id(&name.name),
            _ => None,
        });
        if source_state != Some(state) {
            self.parent.error(
                span,
                "checked machine construction state differs from its source target",
            );
            return None;
        }
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
        if let Some(method) = self.method_values.get(&expression.span()) {
            let key = match method {
                CheckedMethodValue::Source { source_span } => FunctionKey::Method {
                    source_span: *source_span,
                },
                CheckedMethodValue::Interface {
                    interface_type,
                    method_index,
                } => FunctionKey::Interface {
                    owner: *interface_type,
                    method: *method_index,
                },
            };
            return self.function_ids.get(&key).copied().or_else(|| {
                self.parent.error(
                    expression.span(),
                    "method value target has no checked HIR function",
                );
                None
            });
        }
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
        let (mut args, mut evaluation_order) =
            self.lower_pipeline_arguments(piped, extra_args, step.span)?;
        let source_call = self.is_source_call(callee, step.span);
        let kind = if let Some(variant) = self.enum_constructor_variant(callee, output_type) {
            ExpressionKind::EnumConstruct {
                enum_type: output_type,
                variant: VariantId(self.enum_variant_index(output_type, variant)? as u32),
                payloads: args,
                evaluation_order,
            }
        } else if !source_call
            && !self.intrinsic_ids.contains_key(&step.span)
            && self.is_checked_function_value(callee)
        {
            ExpressionKind::IndirectCall {
                callee: Box::new(self.lower_expression(callee)?),
                args,
                evaluation_order,
            }
        } else if source_call {
            ExpressionKind::Call {
                function: self.resolve_user_call_target(callee, step.span)?,
                args,
                evaluation_order,
            }
        } else {
            let has_explicit_type_arguments = Self::has_explicit_type_arguments(&step.function);
            let intrinsic = self.checked_intrinsic_id(step.span)?;
            let type_arguments =
                self.checked_intrinsic_type_arguments(step.span, has_explicit_type_arguments)?;
            let reflection_arguments = self
                .intrinsic_reflection_arguments
                .get(&step.span)
                .cloned()
                .unwrap_or_default();
            self.append_reflected_value_arguments(
                intrinsic,
                &type_arguments,
                &reflection_arguments,
                &mut args,
                &mut evaluation_order,
                Some(output_type),
                step.span,
            )?;
            self.append_reflected_read_arguments(
                intrinsic,
                &type_arguments,
                &reflection_arguments,
                &mut args,
                &mut evaluation_order,
                step.span,
            )?;
            let refinement_predicates =
                self.reflected_finish_predicates(intrinsic, &type_arguments, step.span)?;
            let field_validation =
                self.reflected_read_validation(intrinsic, &type_arguments, step.span)?;
            ExpressionKind::Intrinsic {
                intrinsic,
                type_arguments,
                reflection_arguments,
                refinement_predicates,
                field_validation,
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
            Expr::Call(callee, args, _) | Expr::GenericCall(callee, _, args, _) => {
                (Self::unparenthesized(callee), args, piped_as_view)
            }
            _ => (
                Self::unparenthesized(function),
                &step.extra_args,
                piped_as_view,
            ),
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

fn constructor_type_without_secret(types: &TypeInterner, mut ty: TypeId) -> TypeId {
    while let Type::Secret(inner) = types.resolve(ty) {
        ty = *inner;
    }
    ty
}

fn normalize_secret_constructor(mut expression: Expression, types: &TypeInterner) -> Expression {
    let checked_type = expression.ty;
    let constructor_type = constructor_type_without_secret(types, checked_type);
    let exact_constructor = match (&expression.kind, types.resolve(constructor_type)) {
        (ExpressionKind::ListConstruct { .. }, Type::List(_))
        | (ExpressionKind::MapConstruct { .. }, Type::Map(..))
        | (ExpressionKind::OptionalSome(_) | ExpressionKind::OptionalNone, Type::Optional(_))
        | (ExpressionKind::ResultOk(_) | ExpressionKind::ResultFail(_), Type::Result(..)) => true,
        (
            ExpressionKind::StructConstruct {
                struct_type,
                validates_refinements: false,
                ..
            },
            Type::Struct(_),
        ) => *struct_type == constructor_type,
        (
            ExpressionKind::StructConstruct {
                struct_type,
                validates_refinements: true,
                ..
            },
            Type::Result(ok, error),
        ) => {
            *ok == *struct_type
                && *error == TypeInterner::STRING
                && matches!(types.resolve(*struct_type), Type::Struct(_))
        }
        (
            ExpressionKind::BitfieldConstruct {
                bitfield_type,
                validates_widths: false,
                ..
            },
            Type::Bitfield(_),
        ) => *bitfield_type == constructor_type,
        (
            ExpressionKind::BitfieldConstruct {
                bitfield_type,
                validates_widths: true,
                ..
            },
            Type::Result(ok, error),
        ) => {
            *ok == *bitfield_type
                && *error == TypeInterner::STRING
                && matches!(types.resolve(*bitfield_type), Type::Bitfield(_))
        }
        (
            ExpressionKind::MachineConstruct {
                state_type, state, ..
            },
            Type::MachineState {
                state: expected, ..
            },
        ) => *state_type == constructor_type && state.index() == expected.index(),
        _ => false,
    };
    if constructor_type == checked_type || !exact_constructor {
        return expression;
    }

    // Contextual checking keeps the secret qualification on the constructor.
    // Preserve its exact nominal or container type so payload conversions run
    // before qualification. Never peel a nominal refinement or change a
    // producer's checked type to make its representation fit a constructor.
    let span = expression.span;
    expression.ty = constructor_type;
    Expression {
        kind: ExpressionKind::interface_coerce(Box::new(expression)),
        ty: checked_type,
        span,
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
        lower_source_mode(source, false)
    }

    fn lower_source_mode(source: &str, include_test_bodies: bool) -> Program {
        lower_source_with_check(source, include_test_bodies).0
    }

    fn lower_source_with_check(
        source: &str,
        include_test_bodies: bool,
    ) -> (Program, jett_typecheck::CheckResult) {
        lower_source_with_prelude(source, include_test_bodies, false)
    }

    fn lower_source_with_prelude(
        source: &str,
        include_test_bodies: bool,
        include_equatable: bool,
    ) -> (Program, jett_typecheck::CheckResult) {
        let file = FileId::new(0);
        let mut parsed = jett_parser::parse(source, file);
        assert!(
            parsed.errors.is_empty(),
            "parse errors: {:?}",
            parsed.errors
        );
        let mut origins = HashMap::from([(file, SourceOrigin::Project)]);
        if include_equatable {
            let prelude_file = FileId::new(jett_common::STDLIB_FILE_ID_START);
            let mut prelude = jett_parser::parse(
                "namespace stdlib\nexport interface Equatable:\n    function equals(view self: Equatable, view other: Equatable) returns bool\n",
                prelude_file,
            );
            assert!(prelude.errors.is_empty());
            prelude.module.items.append(&mut parsed.module.items);
            parsed.module.items = prelude.module.items;
            origins.insert(prelude_file, SourceOrigin::Stdlib);
        }
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
        let program = if include_test_bodies {
            lower_with_test_bodies(&parsed.module, &resolved, &checked, &origins)
                .expect("test HIR lowering failed")
        } else {
            lower(&parsed.module, &resolved, &checked, &origins).expect("HIR lowering failed")
        };
        (program, checked)
    }

    #[test]
    fn completing_generated_value_conversions_reuses_existing_adapters() {
        let file = FileId::new(0);
        let source = "type Source = function(secret[int64]) returns secret[int64]\ntype Target = function(int64) returns secret[int64]\nfunction secret_identity(value: secret[int64]) returns secret[int64]:\n    return value\nfunction callback() returns Target:\n    return secret_identity\nfunction convert(items: list[Source]) returns list[Target]:\n    return items\n";
        let parsed = jett_parser::parse(source, file);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let resolved = jett_resolve::resolve(&parsed.module);
        let checked = jett_typecheck::check(&parsed.module, &resolved);
        assert!(
            !checked
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Error),
            "{:?}",
            checked.diagnostics
        );
        let mut program = lower(
            &parsed.module,
            &resolved,
            &checked,
            &HashMap::from([(file, SourceOrigin::Project)]),
        )
        .unwrap();
        let initial = program.clone();
        assert_eq!(
            program
                .functions
                .iter()
                .filter(|function| function
                    .identity
                    .declaration
                    .name
                    .starts_with("$interface.adapter."))
                .count(),
            1
        );
        complete_value_conversions(&mut program, &checked.interner).unwrap();
        complete_value_conversions(&mut program, &checked.interner).unwrap();
        assert_eq!(program, initial);
    }

    #[test]
    fn lowers_checked_verify_and_property_bodies_as_native_test_functions() {
        let program = lower_source_mode(
            r#"namespace app
verify equal_literals:
    assert 1 == 1
property identity:
    given value: int64
    assert value == value
"#,
            true,
        );
        assert_eq!(program.functions.len(), 2);
        assert_eq!(program.functions[0].identity.declaration.namespace, "app");
        assert_eq!(program.functions[1].identity.declaration.namespace, "app");
        assert_eq!(program.functions[0].params.len(), 0);
        assert_eq!(program.functions[1].params.len(), 1);
        assert_eq!(program.functions[1].params[0].ty, TypeInterner::INT64);
        assert!(matches!(
            program.functions[0].body.statements[0].kind,
            StatementKind::Assert { .. }
        ));
        assert!(matches!(
            program.functions[1].body.statements[0].kind,
            StatementKind::Assert { .. }
        ));
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
    fn validator_rejects_invalid_enum_payload_evaluation_orders() {
        let original = lower_source(
            r#"namespace app
enum Pair:
    values(left: int64, right: int64)
function make() returns Pair:
    return Pair.values(right: 2, left: 7)
"#,
        );
        for invalid_order in [vec![0, 0], vec![1], vec![0, 2]] {
            let mut program = original.clone();
            let StatementKind::Return(Some(Expression {
                kind:
                    ExpressionKind::EnumConstruct {
                        evaluation_order, ..
                    },
                ..
            })) = &mut program.functions[0].body.statements[0].kind
            else {
                panic!("expected enum constructor");
            };
            *evaluation_order = invalid_order;
            let errors = validate(&program).expect_err("invalid enum payload order");
            assert!(errors.iter().any(|error| {
                error
                    .message
                    .contains("argument evaluation order must be a permutation")
            }));
        }
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
                    kind: ExpressionKind::Comptime { .. },
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
    fn implicit_struct_equality_checks_results_before_negation_and_preserves_secret_types() {
        fn initializer<'a>(function: &'a Function, name: &str) -> &'a Expression {
            let local = function
                .locals
                .iter()
                .find(|local| local.name == name)
                .unwrap()
                .id;
            function
                .body
                .statements
                .iter()
                .find_map(|statement| match &statement.kind {
                    StatementKind::Let {
                        local: target,
                        value,
                    } if *target == local => Some(value),
                    _ => None,
                })
                .unwrap()
        }
        let (program, checked) = lower_source_with_prelude(
            r#"namespace app
struct Item:
    id: int64
implement Equatable for Item:
    function equals(view self: Item, view other: Item) returns bool:
        return run true
enum Choice:
    item(value: Item)
function inspect(view left: Item, view right: Item) returns bool:
    bool direct = Item.equals(view left, view right)
    bool equal = left == right
    bool different = left != right
    bool primitive = true == false
    trace direct
    trace equal
    trace primitive
    return different
function hidden(view left: secret[Item], view right: Item) returns secret[bool]:
    secret[bool] equal = left == right
    secret[bool] different = left != right
    trace equal
    return different
function nested(view left: Choice, view right: Choice) returns bool:
    return left == right
"#,
            false,
            true,
        );
        let inspect = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "inspect")
            .unwrap();
        let ExpressionKind::Call {
            function: target, ..
        } = initializer(inspect, "direct").kind
        else {
            panic!("explicit equality method call must remain unwrapped");
        };
        assert!(matches!(
            initializer(inspect, "primitive").kind,
            ExpressionKind::Binary { .. }
        ));
        for name in ["inspect", "hidden"] {
            let function = program
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == name)
                .unwrap();
            for binding in ["equal", "different"] {
                let value = initializer(function, binding);
                let boundary = if binding == "different" {
                    let ExpressionKind::Unary {
                        op: UnaryOp::Not,
                        value: inner,
                    } = &value.kind
                    else {
                        panic!("inequality must negate the checked result");
                    };
                    assert_eq!(inner.ty, value.ty);
                    inner.as_ref()
                } else {
                    value
                };
                let ExpressionKind::EquatableResult(call) = &boundary.kind else {
                    panic!("implicit equality must check its method result");
                };
                assert_eq!(call.ty, boundary.ty);
                assert_eq!(call.span, boundary.span);
                assert!(
                    matches!(call.kind, ExpressionKind::Call { function, .. } if function == target)
                );
                if name == "hidden" {
                    assert!(
                        matches!(checked.interner.resolve(boundary.ty), Type::Secret(inner)
                        if *inner == TypeInterner::BOOL)
                    );
                    let ExpressionKind::Call { args, .. } = &call.kind else {
                        unreachable!()
                    };
                    assert!(matches!(
                        checked.interner.resolve(args[0].ty),
                        Type::Secret(_)
                    ));
                } else {
                    assert_eq!(boundary.ty, TypeInterner::BOOL);
                }
            }
        }
        let nested = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "nested")
            .unwrap();
        assert!(matches!(
            &nested.body.statements[0].kind,
            StatementKind::Return(Some(Expression {
                kind: ExpressionKind::Binary { .. },
                ..
            }))
        ));
    }

    #[test]
    fn interpolation_checks_only_implicitly_selected_display_results() {
        let program = lower_source(
            r#"interface Displayable:
    function display(view self: Displayable) returns string
namespace app
struct Item:
    value: int64
implement Displayable for Item:
    function display(view self: Item) returns string:
        return run "shown"
function inspect(view item: Item) returns string:
    string direct = Item.display(view item)
    string pending = run "plain"
    return "{item}:{direct}:{pending}:{7}"
"#,
        );
        let function = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "inspect")
            .unwrap();
        let StatementKind::Let { value: direct, .. } = &function.body.statements[0].kind else {
            panic!("expected direct method call binding");
        };
        let ExpressionKind::Call {
            function: target, ..
        } = &direct.kind
        else {
            panic!("direct method call must remain unwrapped");
        };
        let StatementKind::Return(Some(interpolation)) = &function.body.statements[2].kind else {
            panic!("expected interpolation return");
        };
        let ExpressionKind::StringInterpolation(segments) = &interpolation.kind else {
            panic!("expected interpolation");
        };
        let values = segments
            .iter()
            .filter_map(|segment| match segment {
                StringSegment::Value(value) => Some(value),
                StringSegment::Text(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(values.len(), 4);
        let ExpressionKind::DisplayResult(selected) = &values[0].kind else {
            panic!("implicit display call must retain its result check");
        };
        assert_eq!(values[0].ty, TypeInterner::STRING);
        assert_eq!(selected.ty, TypeInterner::STRING);
        assert_eq!(selected.span, values[0].span);
        assert!(
            matches!(selected.kind, ExpressionKind::Call { function, .. }
            if function == *target)
        );
        assert!(matches!(values[1].kind, ExpressionKind::Local(_)));
        assert!(matches!(values[2].kind, ExpressionKind::Local(_)));
        assert!(matches!(values[3].kind, ExpressionKind::Int(7)));
    }

    #[test]
    fn omits_uninhabited_loop_bodies_without_extracting_inline_functions() {
        let program = lower_source(
            r#"namespace app
function main() returns nothing:
    for item in run list():
        function(int64) returns int64 callback = function(value: int64) returns int64:
            return value
        int64 copied = callback(item)
        trace copied
    for key, value in run map():
        trace key
        trace value
    return nothing
"#,
        );
        assert_eq!(program.functions.len(), 1);
        let function = &program.functions[0];
        assert_eq!(function.locals.len(), 3);
        for statement in &function.body.statements[..2] {
            let StatementKind::For {
                key,
                value,
                iterable,
                body,
                ..
            } = &statement.kind
            else {
                panic!("expected a retained typed loop");
            };
            assert_eq!(
                function.locals[key.index() as usize].ty,
                TypeInterner::NEVER
            );
            if let Some(value) = value {
                assert_eq!(
                    function.locals[value.index() as usize].ty,
                    TypeInterner::NEVER
                );
            }
            assert!(matches!(iterable.kind, ExpressionKind::Run(_)));
            assert!(body.statements.is_empty());
        }
    }

    #[test]
    fn uninhabited_generic_loops_consume_dead_body_facts_and_keep_reserved_identities() {
        let program = lower_source(
            r#"namespace app
function identity[T](value: T) returns T:
    return value
function count[T](view values: list[T]) returns int64:
    mutable int64 total = 0
    for value in view values:
        T copied = identity(value)
        if type.kind_tag[T]() == TypeKind.primitive_type:
            trace copied
        else:
            trace value
        total = total + 1
    return total
function main() returns int64:
    return count(list())
"#,
        );
        let count = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "count")
            .unwrap();
        assert_eq!(count.identity.type_arguments, [TypeInterner::NEVER]);
        assert!(!count.locals.iter().any(|local| local.name == "copied"));
        let StatementKind::For { by_view, body, .. } = &count.body.statements[1].kind else {
            panic!("expected an empty generic iteration body");
        };
        assert!(*by_view);
        assert!(body.statements.is_empty());
        // Checked generic identities remain eagerly reserved. Native
        // reachability, rather than HIR body lowering, excludes dead targets.
        let identity = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "identity")
            .unwrap();
        assert_eq!(identity.identity.type_arguments, [TypeInterner::NEVER]);
        assert_eq!(identity.params[0].ty, TypeInterner::NEVER);
        assert_eq!(identity.return_type, TypeInterner::NEVER);
    }

    #[test]
    fn retains_bodies_for_empty_collections_with_inhabited_element_types() {
        let program = lower_source(
            r#"namespace app
function main() returns nothing:
    list[int64] numbers = list()
    for number in view numbers:
        trace number
    map[int64, string] names = map()
    for key, value in view names:
        trace key
        trace value
    return nothing
"#,
        );
        let function = &program.functions[0];
        for (statement_index, expected_statements) in [(1, 1), (3, 2)] {
            let StatementKind::For { body, .. } = &function.body.statements[statement_index].kind
            else {
                panic!("expected an inhabited collection loop");
            };
            assert_eq!(body.statements.len(), expected_statements);
            assert!(
                body.statements
                    .iter()
                    .all(|statement| matches!(statement.kind, StatementKind::Trace(_)))
            );
        }
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

        assert_eq!(program.functions.len(), 2);
        let constructor = &program.functions[0];
        assert_eq!(
            constructor.identity.declaration.kind,
            DeclarationKind::ActorConstructor
        );
        assert!(matches!(
            constructor.body.statements.as_slice(),
            [
                Statement {
                    kind: StatementKind::Let { .. },
                    ..
                },
                Statement {
                    kind: StatementKind::Return(Some(Expression {
                        kind: ExpressionKind::ActorSpawn {
                            constructor: None,
                            ..
                        },
                        ..
                    })),
                    ..
                }
            ]
        ));
        let handler = &program.functions[1];
        assert_eq!(handler.identity.declaration.namespace, "app");
        assert_eq!(handler.identity.declaration.name, "Counter.add");
        assert_eq!(handler.capture_count, 1);
        assert_eq!(handler.params.len(), 2);
        assert_eq!(handler.params[0].name, "count");
        assert_eq!(handler.params[1].name, "amount");
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
        assert_eq!(
            method.debug_kind,
            FunctionDebugKind::Named("app.User.score".into())
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
    fn lowers_aliased_concrete_method_values_to_source_targets() {
        let program = lower_source(
            r#"namespace models
export interface Reader:
    function read(view self: Reader) returns int64
export struct Point:
    value: int64
    function amount(view self: Point) returns int64:
        return self.value
implement Reader for Point:
    function read(view self: Point) returns int64:
        return self.value
namespace app
struct Callbacks:
    amount: function(view models.Point) returns int64
    read: function(view models.Point) returns int64
function reader() returns function(view models.Point) returns int64:
    use models as m
    return (m.Point.read)
function main() returns int64:
    use models as m
    function(view m.Point) returns int64 callback = (m.Point.amount)
    Callbacks callbacks = Callbacks(amount: m.Point.amount, read: m.Point.read)
    m.Point point = m.Point(value: 7)
    return (callback)(view point)
"#,
        );
        let method = |name: &str| {
            program
                .functions
                .iter()
                .find(|function| function.debug_kind == FunctionDebugKind::Named(name.into()))
                .expect("concrete source method")
        };
        let amount = method("models.Point.amount");
        let read = method("models.Point.read");
        assert_eq!(amount.identity.declaration.kind, DeclarationKind::Method);
        assert_eq!(read.identity.declaration.kind, DeclarationKind::Method);
        assert_eq!(amount.params[0].mode, ParamMode::View);
        assert_eq!(read.params[0].mode, ParamMode::View);
        assert_ne!(amount.id, read.id);
        assert_eq!(
            read.identity.declaration.name,
            "models.Point as models.Reader.read"
        );

        let reader = method("app.reader");
        let StatementKind::Return(Some(value)) = &reader.body.statements[0].kind else {
            panic!("expected returned method value");
        };
        assert_eq!(value.kind, ExpressionKind::FunctionRef(read.id));

        let main = method("app.main");
        let StatementKind::Let { value, .. } = &main.body.statements[0].kind else {
            panic!("expected method local");
        };
        assert_eq!(value.kind, ExpressionKind::FunctionRef(amount.id));
        let StatementKind::Let {
            value:
                Expression {
                    kind: ExpressionKind::StructConstruct { fields, .. },
                    ..
                },
            ..
        } = &main.body.statements[1].kind
        else {
            panic!("expected aggregate containing method values");
        };
        assert_eq!(fields[0].kind, ExpressionKind::FunctionRef(amount.id));
        assert_eq!(fields[1].kind, ExpressionKind::FunctionRef(read.id));
        let StatementKind::Return(Some(Expression {
            kind: ExpressionKind::IndirectCall { args, .. },
            ..
        })) = &main.body.statements[3].kind
        else {
            panic!("expected indirect method call");
        };
        assert!(matches!(args[0].kind, ExpressionKind::View(_)));
    }

    #[test]
    fn lowers_callback_only_interface_slots_with_canonical_names_and_view_modes() {
        let program = lower_source(
            r#"namespace models
export interface Reader:
    function read(view self: Reader) returns int64
    function write(view self: Reader, view stdout: Stdout) returns nothing
implement Reader for int64:
    function read(view self: int64) returns int64:
        return self + 1
    function write(view self: int64, view stdout: Stdout) returns nothing:
        return nothing
namespace app
function reader() returns function(view models.Reader) returns int64:
    use models as m
    return (m.Reader.read)
function writer() returns function(view models.Reader, view Stdout) returns nothing:
    use models
    return models.Reader.write
function main() returns int64:
    use models as m
    function(view m.Reader) returns int64 callback = m.Reader.read
    m.Reader value = 7
    return callback(view value)
"#,
        );
        let named = |name: &str| {
            program
                .functions
                .iter()
                .find(|function| function.debug_kind == FunctionDebugKind::Named(name.into()))
                .expect("named HIR function")
        };
        let read = named("models.Reader.read");
        let write = named("models.Reader.write");
        assert_eq!(read.identity.declaration.namespace, "models");
        assert_eq!(read.identity.declaration.kind, DeclarationKind::Method);
        assert_eq!(read.capture_count, 0);
        assert_eq!(write.capture_count, 0);
        assert_eq!(read.params.len(), 1);
        assert_eq!(write.params.len(), 2);
        assert_eq!(read.params[0].mode, ParamMode::View);
        assert!(
            write
                .params
                .iter()
                .all(|param| param.mode == ParamMode::View)
        );
        assert_eq!(read.params[0].ty, write.params[0].ty);
        assert_eq!(read.return_type, TypeInterner::INT64);
        assert_eq!(write.return_type, TypeInterner::NOTHING);
        assert_eq!(
            read.body.statements.len(),
            2,
            "dispatch and failure fallback"
        );
        assert_eq!(write.body.statements.len(), 2);
        for (factory, target) in [("app.reader", read.id), ("app.writer", write.id)] {
            let StatementKind::Return(Some(value)) = &named(factory).body.statements[0].kind else {
                panic!("expected a returned interface method value");
            };
            assert_eq!(value.kind, ExpressionKind::FunctionRef(target));
        }
        let main = named("app.main");
        let StatementKind::Let { value, .. } = &main.body.statements[0].kind else {
            panic!("expected interface callback binding");
        };
        assert_eq!(value.kind, ExpressionKind::FunctionRef(read.id));
        let StatementKind::Return(Some(Expression {
            kind: ExpressionKind::IndirectCall { args, .. },
            ..
        })) = &main.body.statements[2].kind
        else {
            panic!("expected indirect interface callback invocation");
        };
        assert!(matches!(args[0].kind, ExpressionKind::View(_)));
        assert_eq!(
            program
                .functions
                .iter()
                .filter(|function| function.debug_kind == read.debug_kind)
                .count(),
            1,
            "repeated method values must share one dispatcher",
        );
    }

    #[test]
    fn collects_interface_method_values_found_only_in_generic_body_facts() {
        let program = lower_source(
            r#"namespace app
interface Reader:
    function read(view self: Reader) returns int64
implement Reader for int64:
    function read(view self: int64) returns int64:
        return self
function factory[T](seed: T) returns function(view Reader) returns int64:
    return Reader.read
function main() returns int64:
    function(view Reader) returns int64 first = factory[int64](1)
    function(view Reader) returns int64 second = factory[string]("two")
    Reader value = 7
    return first(view value) + second(view value)
"#,
        );
        let dispatcher = program
            .functions
            .iter()
            .find(|function| {
                function.debug_kind == FunctionDebugKind::Named("app.Reader.read".into())
            })
            .expect("callback-only generic interface dispatcher");
        let factories = program
            .functions
            .iter()
            .filter(|function| function.identity.declaration.name == "factory")
            .collect::<Vec<_>>();
        assert_eq!(factories.len(), 2);
        for factory in factories {
            let StatementKind::Return(Some(value)) = &factory.body.statements[0].kind else {
                panic!("expected specialized interface callback return");
            };
            assert_eq!(value.kind, ExpressionKind::FunctionRef(dispatcher.id));
        }
    }

    #[test]
    fn collects_interface_method_values_in_nested_reflected_body_facts() {
        let program = lower_source(
            r#"namespace app
interface Reader:
    function read(view self: Reader) returns int64
implement Reader for int64:
    function read(view self: int64) returns int64:
        return self
struct Inner:
    count: int64
struct Outer:
    item: Inner
function reflected[T](view value: T, view reader: Reader) returns int64:
    mutable int64 output = 0
    for outer in type.fields[T]():
        comptime type Field = outer.type_info:
            for inner in type.fields[Field]():
                comptime type Item = inner.type_info:
                    function(view Reader) returns int64 callback = Reader.read
                    output = callback(view reader)
    return output
function main() returns int64:
    Outer value = Outer(item: Inner(count: 1))
    Reader reader = 7
    return reflected[Outer](view value, view reader)
"#,
        );
        let dispatcher = program
            .functions
            .iter()
            .find(|function| {
                function.debug_kind == FunctionDebugKind::Named("app.Reader.read".into())
            })
            .expect("nested reflected interface dispatcher");
        let reflected = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "reflected")
            .expect("specialized reflected function");
        let StatementKind::For { body, .. } = &reflected.body.statements[1].kind else {
            panic!("expected outer reflected loop");
        };
        let StatementKind::ReflectedTypeDispatch { arms, .. } = &body.statements[0].kind else {
            panic!("expected outer type binding dispatch");
        };
        assert_eq!(arms.len(), 1);
        let StatementKind::For { body, .. } = &arms[0].body.statements[0].kind else {
            panic!("expected inner reflected loop");
        };
        let StatementKind::ReflectedTypeDispatch { arms, .. } = &body.statements[0].kind else {
            panic!("expected inner type binding dispatch");
        };
        assert_eq!(arms.len(), 1);
        let StatementKind::Let { value, .. } = &arms[0].body.statements[0].kind else {
            panic!("expected interface callback in nested binding body");
        };
        assert_eq!(value.kind, ExpressionKind::FunctionRef(dispatcher.id));
    }

    #[test]
    fn keeps_concrete_method_values_in_generic_and_inline_fact_contexts() {
        let program = lower_source(
            r#"namespace app
struct Point:
    value: int64
    function amount(view self: Point) returns int64:
        return self.value
function factory[T](seed: T) returns function(view Point) returns int64:
    return Point.amount
function inline_factory() returns function() returns function(view Point) returns int64:
    return function() returns function(view Point) returns int64: return Point.amount
function main() returns int64:
    function(view Point) returns int64 first = factory[int64](1)
    function(view Point) returns int64 second = factory[string]("two")
    Point point = Point(value: 7)
    return first(view point)
"#,
        );
        let method = program
            .functions
            .iter()
            .find(|function| {
                function.debug_kind == FunctionDebugKind::Named("app.Point.amount".into())
            })
            .expect("concrete method target");
        let mut factory_count = 0;
        let mut inline_count = 0;
        for function in &program.functions {
            if function.identity.declaration.name != "factory"
                && function.debug_kind != FunctionDebugKind::Inline
            {
                continue;
            }
            let StatementKind::Return(Some(value)) = &function.body.statements[0].kind else {
                panic!("expected returned concrete method value");
            };
            assert_eq!(value.kind, ExpressionKind::FunctionRef(method.id));
            if function.debug_kind == FunctionDebugKind::Inline {
                inline_count += 1;
            } else {
                factory_count += 1;
            }
        }
        assert_eq!(factory_count, 2);
        assert_eq!(inline_count, 1);
    }

    #[test]
    fn keeps_method_values_when_switching_reflected_body_facts() {
        let program = lower_source(
            r#"namespace app
struct Point:
    value: int64
    function amount(view self: Point) returns int64:
        return self.value
struct Mixed:
    label: string
    count: int64
function reflected[T](view value: T, view point: Point) returns int64:
    mutable int64 output = 0
    for field in type.fields[T]():
        comptime type Field = field.type_info:
            function(view Point) returns int64 callback = Point.amount
            output = callback(view point)
    function(view Point) returns int64 after = Point.amount
    return after(view point) + output
function main() returns int64:
    Mixed value = Mixed(label: "one", count: 1)
    Point point = Point(value: 7)
    return reflected[Mixed](view value, view point)
"#,
        );
        let method = program
            .functions
            .iter()
            .find(|function| {
                function.debug_kind == FunctionDebugKind::Named("app.Point.amount".into())
            })
            .expect("concrete method target");
        let reflected = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "reflected")
            .expect("concrete reflected function");
        let StatementKind::For { body, .. } = &reflected.body.statements[1].kind else {
            panic!("expected reflected loop");
        };
        let StatementKind::ReflectedTypeDispatch { arms, .. } = &body.statements[0].kind else {
            panic!("expected reflected type dispatch");
        };
        assert_eq!(arms.len(), 2);
        for arm in arms {
            let StatementKind::Let { value, .. } = &arm.body.statements[0].kind else {
                panic!("expected method local inside specialized body");
            };
            assert_eq!(value.kind, ExpressionKind::FunctionRef(method.id));
        }
        let StatementKind::Let { value, .. } = &reflected.body.statements[2].kind else {
            panic!("expected method local after specialized body");
        };
        assert_eq!(value.kind, ExpressionKind::FunctionRef(method.id));
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
    fn lowers_contextual_secret_collection_and_sum_constructors() {
        let (program, checked) = lower_source_with_check(
            r#"function main() returns nothing:
    secret[list[int64]] numbers = list(5, 7)
    secret[map[string, int64]] lookup = map("answer": 7)
    secret[optional[int64]] present = some(7)
    secret[optional[int64]] absent = none
    secret[result[int64, string]] success = ok(7)
    secret[result[int64, string]] failure = fail("reason")
    secret[list[string]] empty_list = list()
    secret[map[string, list[string]]] empty_map = map()
    return nothing
"#,
            false,
        );
        let main = &program.functions[0];
        for (index, statement) in main.body.statements[..8].iter().enumerate() {
            let StatementKind::Let { local, value } = &statement.kind else {
                panic!("expected constructor local");
            };
            assert_eq!(value.ty, main.locals[local.index() as usize].ty);
            assert_eq!(checked.type_map[&value.span], value.ty);
            let Type::Secret(inner_type) = checked.interner.resolve(value.ty) else {
                panic!("expected retained checked secret type");
            };
            let ExpressionKind::InterfaceCoerce {
                value: constructor,
                adapters,
            } = &value.kind
            else {
                panic!("expected explicit secret qualification");
            };
            assert!(adapters.is_empty());
            assert_eq!(constructor.ty, *inner_type);
            assert_eq!(constructor.span, value.span);
            match (&constructor.kind, checked.interner.resolve(*inner_type)) {
                (ExpressionKind::ListConstruct { elements }, Type::List(element_type)) => {
                    assert!(elements.iter().all(|element| element.ty == *element_type));
                    if index == 0 {
                        assert!(matches!(elements[0].kind, ExpressionKind::Int(5)));
                        assert!(matches!(elements[1].kind, ExpressionKind::Int(7)));
                    } else {
                        assert!(elements.is_empty());
                        assert_eq!(*element_type, TypeInterner::STRING);
                    }
                }
                (ExpressionKind::MapConstruct { entries }, Type::Map(key, value)) => {
                    assert!(entries.iter().all(|entry| entry.key.ty == *key));
                    assert!(entries.iter().all(|entry| entry.value.ty == *value));
                    if index == 7 {
                        assert!(entries.is_empty());
                        assert!(matches!(checked.interner.resolve(*value), Type::List(_)));
                    }
                }
                (ExpressionKind::OptionalSome(payload), Type::Optional(expected))
                | (ExpressionKind::ResultOk(payload), Type::Result(expected, _))
                | (ExpressionKind::ResultFail(payload), Type::Result(_, expected)) => {
                    assert_eq!(payload.ty, *expected);
                }
                (ExpressionKind::OptionalNone, Type::Optional(expected)) => {
                    assert_eq!(*expected, TypeInterner::INT64);
                }
                _ => panic!("constructor did not retain its exact container shape"),
            }
        }
    }

    #[test]
    fn secret_local_metadata_preserves_declared_qualification_and_producer_signatures() {
        let (program, checked) = lower_source_with_check(
            r#"function make(value: int8) returns list[int8]:
    return list(value)
function read(view values: list[int8]) returns int64:
    return 0
function main() returns int64:
    int8 seed = 7
    secret[secret[list[int8]]] twice = list(seed)
    secret[list[int8]] once = declassify clone twice
    list[int8] public = declassify clone once
    secret[list[int8]] from_call = make(seed)
    secret[secret[list[int8]]] twice_from_call = make(seed)
    return read(view public)
"#,
            false,
        );
        let make = &program.functions[0];
        assert!(matches!(
            checked.interner.resolve(make.return_type),
            Type::List(element) if *element == TypeInterner::INT8
        ));
        let main = &program.functions[2];
        let binding = |name: &str| {
            main.body
                .statements
                .iter()
                .find_map(|statement| match &statement.kind {
                    StatementKind::Let { local, value }
                        if main.locals[local.index() as usize].name == name =>
                    {
                        Some((&main.locals[local.index() as usize], value))
                    }
                    _ => None,
                })
                .expect("source binding")
        };
        let (twice, constructed) = binding("twice");
        let (once, declassified) = binding("once");
        let (public, _) = binding("public");
        assert_eq!(checked.interner.resolve(twice.ty), &Type::Secret(once.ty));
        assert_eq!(checked.interner.resolve(once.ty), &Type::Secret(public.ty));
        assert_eq!(public.ty, make.return_type);
        let ExpressionKind::InterfaceCoerce { value: literal, .. } = &constructed.kind else {
            panic!("expected qualification of the exact checked literal");
        };
        assert_eq!(constructed.ty, twice.ty);
        assert_eq!(literal.ty, make.return_type);
        assert_eq!(checked.type_map[&literal.span], literal.ty);
        assert!(matches!(literal.kind, ExpressionKind::ListConstruct { .. }));

        let ExpressionKind::Declassify(cloned) = &declassified.kind else {
            panic!("expected one-layer declassification");
        };
        let ExpressionKind::Clone(source) = &cloned.kind else {
            panic!("expected explicit clone of the qualified local");
        };
        assert_eq!(source.kind, ExpressionKind::Local(twice.id));
        assert_eq!(source.ty, twice.ty);
        assert_eq!(declassified.ty, once.ty);

        for name in ["from_call", "twice_from_call"] {
            let (local, initializer) = binding(name);
            assert_eq!(local.ty, initializer.ty);
            let producer = match &initializer.kind {
                ExpressionKind::InterfaceCoerce { value, .. } => value.as_ref(),
                _ => initializer,
            };
            let ExpressionKind::Call { function, .. } = producer.kind else {
                panic!("expected original checked public producer");
            };
            assert_eq!(function, make.id);
            assert_eq!(checked.type_map[&producer.span], producer.ty);
            if name == "twice_from_call" {
                assert_eq!(local.ty, twice.ty);
                assert_eq!(producer.ty, make.return_type);
            } else {
                assert_eq!(local.ty, once.ty);
            }
        }
        let StatementKind::Return(Some(Expression {
            kind: ExpressionKind::Call { function, .. },
            ty,
            ..
        })) = &main.body.statements.last().expect("return").kind
        else {
            panic!("expected ordinary public reader call");
        };
        assert_eq!(*function, program.functions[1].id);
        assert_eq!(*ty, TypeInterner::INT64);
    }

    #[test]
    fn secret_constructor_payloads_keep_nested_qualification_and_nominal_types() {
        let (program, checked) = lower_source_with_check(
            r#"type Count = int64 where true
function nested(value: Count) returns secret[list[secret[optional[Count]]]]:
    return list(some(value))
function pending() returns secret[list[int64]]:
    return run (list(7))
function fixed() returns secret[list[int64]]:
    return comptime (list(9))
"#,
            false,
        );
        let nested = &program.functions[0];
        let StatementKind::Return(Some(outer)) = &nested.body.statements[0].kind else {
            panic!("expected nested constructor return");
        };
        let ExpressionKind::InterfaceCoerce { value: list, .. } = &outer.kind else {
            panic!("expected secret list qualification");
        };
        let ExpressionKind::ListConstruct { elements } = &list.kind else {
            panic!("expected list constructor");
        };
        let ExpressionKind::InterfaceCoerce {
            value: optional, ..
        } = &elements[0].kind
        else {
            panic!("expected independently qualified optional payload");
        };
        let ExpressionKind::OptionalSome(payload) = &optional.kind else {
            panic!("expected optional constructor");
        };
        assert_eq!(payload.ty, nested.params[0].ty);
        assert!(matches!(
            checked.interner.resolve(payload.ty),
            Type::Refinement { .. }
        ));
        assert!(matches!(payload.kind, ExpressionKind::Local(_)));

        for function in &program.functions[1..3] {
            let StatementKind::Return(Some(value)) = &function.body.statements[0].kind else {
                panic!("expected wrapped constructor return");
            };
            let constructor = match &value.kind {
                ExpressionKind::Run(inner) | ExpressionKind::Comptime { value: inner, .. } => inner,
                _ => panic!("normalization must retain the source execution wrapper"),
            };
            assert_eq!(constructor.ty, value.ty);
            let ExpressionKind::InterfaceCoerce { value: inner, .. } = &constructor.kind else {
                panic!("expected normalized parenthesized literal");
            };
            assert!(matches!(inner.kind, ExpressionKind::ListConstruct { .. }));
            assert_eq!(value.ty, function.return_type);
        }
    }

    #[test]
    fn secret_constructor_normalization_enables_existing_payload_conversions() {
        let (program, checked) = lower_source_with_check(
            r#"interface Named:
    function name(view self: Named) returns string
struct Item:
    label: string
implement Named for Item:
    function name(view self: Item) returns string:
        return self.label
type Callback = function(int64) returns secret[int64]
function classified(value: secret[int64]) returns secret[int64]:
    return value
function names(item: Item) returns secret[list[Named]]:
    return list(item)
function callbacks() returns secret[list[Callback]]:
    return list(classified)
"#,
            false,
        );
        for name in ["names", "callbacks"] {
            let function = program
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == name)
                .expect("source function");
            let StatementKind::Return(Some(Expression {
                kind: ExpressionKind::InterfaceCoerce { value: list, .. },
                ..
            })) = &function.body.statements[0].kind
            else {
                panic!("expected secret qualification");
            };
            let ExpressionKind::ListConstruct { elements } = &list.kind else {
                panic!("expected list constructor");
            };
            let Type::List(expected_element) = checked.interner.resolve(list.ty) else {
                panic!("expected exact inner list type");
            };
            assert_eq!(elements[0].ty, *expected_element);
            if name == "names" {
                let ExpressionKind::InterfaceCoerce { value: source, .. } = &elements[0].kind
                else {
                    panic!("expected nominal-to-interface payload conversion");
                };
                assert_eq!(source.ty, function.params[0].ty);
                assert!(matches!(source.kind, ExpressionKind::Local(_)));
            } else {
                let ExpressionKind::FunctionAdapter {
                    value: source,
                    function: adapter,
                } = &elements[0].kind
                else {
                    panic!("expected existing checked function adapter");
                };
                assert!(matches!(source.kind, ExpressionKind::FunctionRef(_)));
                assert_ne!(source.ty, elements[0].ty);
                assert_eq!(program.functions[adapter.index() as usize].capture_count, 1);
            }
        }
    }

    #[test]
    fn secret_constructor_normalization_is_exact_and_does_not_peel_refinements() {
        let mut types = TypeInterner::new();
        let list = types.intern(Type::List(TypeInterner::INT64));
        let optional = types.intern(Type::Optional(TypeInterner::INT64));
        let refined = types.intern(Type::Refinement {
            name: "Numbers".into(),
            base: list,
        });
        let secret_list = types.intern(Type::Secret(list));
        let twice_secret = types.intern(Type::Secret(secret_list));
        let secret_optional = types.intern(Type::Secret(optional));
        let secret_refined = types.intern(Type::Secret(refined));
        let span = Span::new(FileId::new(0), 3, 9);
        for ty in [list, secret_optional, secret_refined] {
            let original = Expression {
                kind: ExpressionKind::ListConstruct { elements: vec![] },
                ty,
                span,
            };
            assert_eq!(
                normalize_secret_constructor(original.clone(), &types),
                original
            );
        }
        for kind in [
            ExpressionKind::Local(LocalId::new(0)),
            ExpressionKind::Call {
                function: FunctionId::new(0),
                args: vec![],
                evaluation_order: vec![],
            },
        ] {
            let original = Expression {
                kind,
                ty: secret_list,
                span,
            };
            assert_eq!(
                normalize_secret_constructor(original.clone(), &types),
                original
            );
        }
        let original = Expression {
            kind: ExpressionKind::ListConstruct { elements: vec![] },
            ty: twice_secret,
            span,
        };
        let normalized = normalize_secret_constructor(original, &types);
        assert_eq!(normalized.ty, twice_secret);
        let ExpressionKind::InterfaceCoerce { value, .. } = &normalized.kind else {
            panic!("expected outer qualification");
        };
        assert_eq!(value.ty, list);
        assert_eq!(value.span, span);
        assert_eq!(
            normalize_secret_constructor(normalized.clone(), &types),
            normalized
        );
    }

    #[test]
    fn contextual_secret_structs_preserve_checked_nominal_constructors() {
        let source = r#"struct Item:
    left: int64
    right: int64
struct Box[T]:
    value: T
function direct() returns secret[Item]:
    return Item(right: 2, left: 1)
function nested() returns secret[secret[Item]]:
    return (Item(left: 3, right: 4))
function pending() returns secret[Item]:
    return run (Item(left: 5, right: 6))
function generic[T](value: T) returns secret[Box[T]]:
    return Box[T](value: value)
function public_item() returns Item:
    return Item(left: 7, right: 8)
function producer() returns secret[Item]:
    return public_item()
function main() returns nothing:
    secret[Item] local = Item(left: 9, right: 10)
    secret[Box[int64]] explicit = Box[int64](value: 11)
    secret[Box[int64]] specialized = generic[int64](12)
    return nothing
"#;
        let (program, checked) = lower_source_with_check(source, false);
        let parsed = jett_parser::parse(source, FileId::new(0));
        assert!(parsed.errors.is_empty());
        let mut source_calls = HashMap::new();
        for item in &parsed.module.items {
            let Item::Function(function) = item else {
                continue;
            };
            for statement in &function.body.stmts {
                let (name, mut value) = match statement {
                    Stmt::Return(ast::ReturnStmt {
                        value: Some(value), ..
                    }) => (&function.name.name, value),
                    Stmt::VarDecl(declaration) => (&declaration.name.name, &declaration.value),
                    _ => continue,
                };
                while let Expr::Paren(inner, _) | Expr::Run(inner, _) = value {
                    value = inner;
                }
                if matches!(value, Expr::Call(..) | Expr::GenericCall(..)) {
                    source_calls.insert(name.as_str(), value.span());
                }
            }
        }
        let check_constructor = |name: &str, value: &Expression| {
            let mut nominal = value.ty;
            let mut depth = 0;
            while let Type::Secret(inner) = checked.interner.resolve(nominal) {
                nominal = *inner;
                depth += 1;
            }
            assert!(depth > 0);
            let ExpressionKind::InterfaceCoerce {
                value: constructor,
                adapters,
            } = &value.kind
            else {
                panic!("expected secret qualification around the constructor");
            };
            assert!(adapters.is_empty());
            let ExpressionKind::StructConstruct {
                struct_type,
                fields,
                evaluation_order,
                validates_refinements,
                refinement_predicates,
            } = &constructor.kind
            else {
                panic!("expected exact nominal struct constructor");
            };
            assert_eq!(constructor.ty, nominal);
            assert_eq!(*struct_type, nominal);
            assert!(matches!(checked.interner.resolve(nominal), Type::Struct(_)));
            assert!(!validates_refinements);
            assert!(refinement_predicates.is_empty());
            assert_eq!(fields.len(), evaluation_order.len());
            // Parentheses may widen the HIR constructor span before the return
            // coercion adds qualification. Facts retain the original call span.
            let source_span = source_calls[name];
            assert_eq!(constructor.span.file, source_span.file, "{name}");
            assert!(
                constructor.span.start <= source_span.start
                    && constructor.span.end >= source_span.end,
                "{name}: HIR span {:?} does not contain source call {source_span:?}",
                constructor.span,
            );
            let fact = checked
                .struct_constructions
                .get(&source_span)
                .or_else(|| {
                    checked
                        .generic_function_instantiations
                        .iter()
                        .find_map(|body| body.struct_constructions.get(&source_span))
                })
                .unwrap_or_else(|| panic!(
                    "{name}: source call {source_span:?} must have a checked construction fact (HIR span {:?})",
                    constructor.span,
                ));
            assert_eq!(fact.struct_type, nominal);
            depth
        };
        for name in ["direct", "nested", "pending", "generic"] {
            let function = program
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == name)
                .expect("source or specialized function");
            let StatementKind::Return(Some(value)) = &function.body.statements[0].kind else {
                panic!("expected constructor return");
            };
            assert_eq!(value.ty, function.return_type);
            let constructor = if name == "pending" {
                let ExpressionKind::Run(inner) = &value.kind else {
                    panic!("pending constructor must retain its source run wrapper");
                };
                assert_eq!(inner.ty, value.ty);
                inner.as_ref()
            } else {
                value
            };
            assert_eq!(
                check_constructor(name, constructor),
                if name == "nested" { 2 } else { 1 }
            );
            if name == "direct" {
                let ExpressionKind::InterfaceCoerce { value: inner, .. } = &value.kind else {
                    unreachable!();
                };
                let ExpressionKind::StructConstruct {
                    fields,
                    evaluation_order,
                    ..
                } = &inner.kind
                else {
                    unreachable!();
                };
                assert_eq!(evaluation_order, &[1, 0]);
                assert!(matches!(fields[0].kind, ExpressionKind::Int(1)));
                assert!(matches!(fields[1].kind, ExpressionKind::Int(2)));
                assert_eq!(inner.span, value.span);
            }
        }
        let main = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "main")
            .unwrap();
        for statement in &main.body.statements[..2] {
            let StatementKind::Let { local, value } = &statement.kind else {
                panic!("expected qualified constructor local");
            };
            assert_eq!(value.ty, main.locals[local.index() as usize].ty);
            assert_eq!(
                check_constructor(&main.locals[local.index() as usize].name, value),
                1
            );
        }
        let producer = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "producer")
            .unwrap();
        let StatementKind::Return(Some(value)) = &producer.body.statements[0].kind else {
            panic!("expected producer call");
        };
        let original = match &value.kind {
            ExpressionKind::InterfaceCoerce { value, .. } => value.as_ref(),
            _ => value,
        };
        assert!(matches!(original.kind, ExpressionKind::Call { .. }));
        assert_eq!(original.ty, checked.type_map[&original.span]);
        assert!(!checked.struct_constructions.contains_key(&original.span));
    }

    #[test]
    fn secret_struct_payload_conversions_preserve_source_evaluation_order() {
        let (program, checked) = lower_source_with_check(
            r#"interface Named:
    function name(view self: Named) returns string
struct Item:
    label: string
implement Named for Item:
    function name(view self: Item) returns string:
        return self.label
type Count = int64 where true
type Callback = function(int64) returns secret[int64]
struct Bundle:
    owner: Named
    callback: Callback
    count: list[Count]
    fallback: int64
struct Validated:
    count: Count
function classified(value: secret[int64]) returns secret[int64]:
    return value
function bundle(item: Item, count: Count, maybe: optional[int64]) returns secret[Bundle]:
    return Bundle(fallback: maybe handle:
        default 9
    , count: list(count), callback: classified, owner: item)
function validated(count: Count) returns result[Validated, string]:
    return Validated(count: count)
"#,
            false,
        );
        let function = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "bundle")
            .unwrap();
        let StatementKind::Return(Some(Expression {
            kind:
                ExpressionKind::InterfaceCoerce {
                    value: constructor, ..
                },
            ty,
            ..
        })) = &function.body.statements[0].kind
        else {
            panic!("expected qualified bundle construction");
        };
        assert_eq!(*ty, function.return_type);
        let ExpressionKind::StructConstruct {
            struct_type,
            fields,
            evaluation_order,
            validates_refinements,
            refinement_predicates,
        } = &constructor.kind
        else {
            panic!("expected exact bundle constructor");
        };
        assert_eq!(constructor.ty, *struct_type);
        assert_eq!(evaluation_order, &[3, 2, 1, 0]);
        assert!(!validates_refinements);
        assert!(refinement_predicates.is_empty());
        let Type::Struct(id) = checked.interner.resolve(*struct_type) else {
            unreachable!();
        };
        let declared = &checked.interner.resolve_struct(*id).fields;
        assert!(
            fields
                .iter()
                .zip(declared)
                .all(|(field, (_, ty))| field.ty == *ty)
        );
        let ExpressionKind::InterfaceCoerce { value: owner, .. } = &fields[0].kind else {
            panic!("expected the existing nominal-to-interface field conversion");
        };
        assert_eq!(owner.ty, function.params[0].ty);
        assert!(matches!(owner.kind, ExpressionKind::Local(_)));
        let ExpressionKind::FunctionAdapter {
            value: callback,
            function: adapter,
        } = &fields[1].kind
        else {
            panic!("expected the existing callback field adapter");
        };
        assert!(matches!(callback.kind, ExpressionKind::FunctionRef(_)));
        assert_eq!(program.functions[adapter.index() as usize].capture_count, 1);
        let ExpressionKind::ListConstruct { elements } = &fields[2].kind else {
            panic!("expected an independently checked nominal payload container");
        };
        assert_eq!(elements[0].ty, function.params[1].ty);
        assert!(matches!(
            checked.interner.resolve(elements[0].ty),
            Type::Refinement { .. }
        ));
        assert!(matches!(elements[0].kind, ExpressionKind::Local(_)));
        assert!(matches!(fields[3].kind, ExpressionKind::Handle { .. }));

        let validated = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "validated")
            .unwrap();
        let StatementKind::Return(Some(value)) = &validated.body.statements[0].kind else {
            panic!("expected validating constructor return");
        };
        let ExpressionKind::StructConstruct {
            struct_type,
            validates_refinements,
            refinement_predicates,
            ..
        } = &value.kind
        else {
            panic!("a refinement-field constructor must retain its result boundary");
        };
        assert!(*validates_refinements);
        assert_eq!(refinement_predicates, &[Vec::new()]);
        assert_eq!(
            checked.interner.resolve(value.ty),
            &Type::Result(*struct_type, TypeInterner::STRING)
        );
        assert_eq!(value.ty, validated.return_type);
    }

    #[test]
    fn secret_validating_structs_preserve_result_types_predicates_and_source_order() {
        let source = r#"type Positive = int8 where value > 0
struct Item:
    value: Positive
    label: string
struct Box[T]:
    value: Positive
    payload: T
function direct(input: int8) returns secret[result[Item, string]]:
    return Item(label: "direct", value: input)
function exact(input: Positive) returns secret[result[Item, string]]:
    return Item(label: "exact", value: input)
function nested(input: int8) returns secret[secret[result[Item, string]]]:
    return ((Item)(label: "nested", value: input))
function pending(input: int8) returns secret[result[Item, string]]:
    return run (Item(label: "pending", value: input))
function handled(input: optional[int8], fallback: int8) returns secret[result[Item, string]]:
    return Item(label: "handled", value: input handle:
        default fallback
    )
function generic[T](input: int8, payload: T) returns secret[result[Box[T], string]]:
    return Box[T](payload: payload, value: input)
function ordinary(input: int8) returns result[Item, string]:
    return Item(label: "ordinary", value: input)
function producer(input: int8) returns secret[result[Item, string]]:
    return ordinary(input)
function main() returns nothing:
    int8 seed = 7
    secret[result[Item, string]] local = Item(label: "local", value: seed)
    secret[result[Box[int64], string]] specialized = generic[int64](seed, 11)
    return nothing
"#;
        let (program, checked) = lower_source_with_check(source, false);
        let parsed = jett_parser::parse(source, FileId::new(0));
        let source_calls = parsed
            .module
            .items
            .iter()
            .filter_map(|item| {
                let Item::Function(function) = item else {
                    return None;
                };
                let Stmt::Return(ast::ReturnStmt {
                    value: Some(mut_value),
                    ..
                }) = function.body.stmts.first()?
                else {
                    return None;
                };
                let mut value = mut_value;
                while let Expr::Run(inner, _) | Expr::Paren(inner, _) = value {
                    value = inner;
                }
                Some((function.name.name.as_str(), value.span()))
            })
            .collect::<HashMap<_, _>>();
        for name in ["direct", "exact", "nested", "pending", "handled", "generic"] {
            let function = program
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == name)
                .unwrap();
            let StatementKind::Return(Some(value)) = &function.body.statements[0].kind else {
                panic!("{name}: constructor return");
            };
            assert_eq!(value.ty, function.return_type, "{name}");
            let value = if name == "pending" {
                let ExpressionKind::Run(inner) = &value.kind else {
                    panic!("pending constructor keeps its source run");
                };
                assert_eq!(inner.ty, value.ty);
                inner.as_ref()
            } else {
                value
            };
            let mut result_type = value.ty;
            let mut depth = 0;
            while let Type::Secret(inner) = checked.interner.resolve(result_type) {
                result_type = *inner;
                depth += 1;
            }
            assert_eq!(depth, if name == "nested" { 2 } else { 1 }, "{name}");
            let ExpressionKind::InterfaceCoerce {
                value: constructor,
                adapters,
            } = &value.kind
            else {
                panic!("{name}: exact result qualification");
            };
            assert!(adapters.is_empty());
            assert_eq!(constructor.ty, result_type);
            let ExpressionKind::StructConstruct {
                struct_type,
                fields,
                evaluation_order,
                validates_refinements,
                refinement_predicates,
            } = &constructor.kind
            else {
                panic!("{name}: validating constructor");
            };
            assert!(*validates_refinements);
            assert_eq!(
                checked.interner.resolve(result_type),
                &Type::Result(*struct_type, TypeInterner::STRING)
            );
            assert_eq!(evaluation_order, &[1, 0]);
            assert_eq!(fields.len(), 2);
            assert_eq!(refinement_predicates.len(), fields.len());
            assert!(refinement_predicates[1].is_empty());
            let Type::Struct(id) = checked.interner.resolve(*struct_type) else {
                panic!("exact nominal struct");
            };
            let definition = checked.interner.resolve_struct(*id);
            assert_eq!(fields[1].ty, definition.fields[1].1);
            if name == "exact" {
                assert!(refinement_predicates[0].is_empty());
                assert_eq!(fields[0].ty, definition.fields[0].1);
            } else {
                let [predicate] = refinement_predicates[0].as_slice() else {
                    panic!("{name}: preserved refinement predicate");
                };
                assert_eq!(predicate.refined_type, definition.fields[0].1);
                assert_eq!(fields[0].ty, TypeInterner::INT8);
                assert_eq!(
                    program.functions[predicate.function.index() as usize].return_type,
                    TypeInterner::BOOL
                );
            }
            if name == "handled" {
                assert!(matches!(fields[0].kind, ExpressionKind::Handle { .. }));
            }
            let source_span = source_calls[name];
            assert_eq!(constructor.span.file, source_span.file);
            assert!(
                constructor.span.start <= source_span.start
                    && constructor.span.end >= source_span.end
            );
            let fact = checked
                .struct_constructions
                .get(&source_span)
                .or_else(|| {
                    checked
                        .generic_function_instantiations
                        .iter()
                        .find_map(|body| body.struct_constructions.get(&source_span))
                })
                .expect("checked source construction fact");
            assert_eq!(fact.struct_type, *struct_type);
            assert!(fact.validates_refinements);
        }
        let ordinary = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "ordinary")
            .unwrap();
        let StatementKind::Return(Some(value)) = &ordinary.body.statements[0].kind else {
            panic!("ordinary constructor return");
        };
        assert!(matches!(
            value.kind,
            ExpressionKind::StructConstruct {
                validates_refinements: true,
                ..
            }
        ));
        assert_eq!(value.ty, checked.type_map[&value.span]);
        let producer = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "producer")
            .unwrap();
        let StatementKind::Return(Some(value)) = &producer.body.statements[0].kind else {
            panic!("producer return");
        };
        let call = match &value.kind {
            ExpressionKind::InterfaceCoerce { value, .. } => value.as_ref(),
            _ => value,
        };
        let ExpressionKind::Call { function, .. } = &call.kind else {
            panic!("ordinary producer must remain a call");
        };
        let target = &program.functions[function.index() as usize];
        assert_eq!(target.identity, ordinary.identity);
        assert_eq!(target.return_type, ordinary.return_type);
        assert!(matches!(
            checked.interner.resolve(target.return_type),
            Type::Result(_, TypeInterner::STRING)
        ));
        assert_eq!(call.span, source_calls["producer"]);
        assert_eq!(call.ty, checked.type_map[&call.span]);
        assert!(!checked.struct_constructions.contains_key(&call.span));
        let main = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "main")
            .unwrap();
        let StatementKind::Let { local, value } = &main.body.statements[1].kind else {
            panic!("qualified constructor local");
        };
        assert_eq!(value.ty, main.locals[local.index() as usize].ty);
        let ExpressionKind::InterfaceCoerce {
            value: constructor, ..
        } = &value.kind
        else {
            panic!("local result qualification");
        };
        assert_eq!(constructor.span, value.span);
        assert!(matches!(
            constructor.kind,
            ExpressionKind::StructConstruct {
                validates_refinements: true,
                ..
            }
        ));
    }

    #[test]
    fn secret_struct_normalization_keeps_nominal_and_validation_boundaries_exact() {
        let mut types = TypeInterner::new();
        let first_id = types.add_struct(jett_types::StructDef {
            name: "First".into(),
            fields: vec![("value".into(), TypeInterner::INT64)],
            methods: vec![],
        });
        let first = types.intern(Type::Struct(first_id));
        let second_id = types.add_struct(jett_types::StructDef {
            name: "Second".into(),
            fields: vec![("value".into(), TypeInterner::INT64)],
            methods: vec![],
        });
        let second = types.intern(Type::Struct(second_id));
        let refined = types.intern(Type::Refinement {
            name: "Selected".into(),
            base: first,
        });
        let result = types.intern(Type::Result(first, TypeInterner::STRING));
        let wrong_ok = types.intern(Type::Result(second, TypeInterner::STRING));
        let wrong_error = types.intern(Type::Result(first, TypeInterner::INT64));
        let refined_ok = types.intern(Type::Result(refined, TypeInterner::STRING));
        let refined_result = types.intern(Type::Refinement {
            name: "CheckedResult".into(),
            base: result,
        });
        let secret_first = types.intern(Type::Secret(first));
        let secret_second = types.intern(Type::Secret(second));
        let secret_refined = types.intern(Type::Secret(refined));
        let secret_result = types.intern(Type::Secret(result));
        let secret_wrong_ok = types.intern(Type::Secret(wrong_ok));
        let secret_wrong_error = types.intern(Type::Secret(wrong_error));
        let secret_refined_ok = types.intern(Type::Secret(refined_ok));
        let secret_refined_result = types.intern(Type::Secret(refined_result));
        let span = Span::new(FileId::new(0), 3, 9);
        let make = |ty, validates_refinements| Expression {
            kind: ExpressionKind::StructConstruct {
                struct_type: first,
                fields: vec![Expression {
                    kind: ExpressionKind::Int(7),
                    ty: TypeInterner::INT64,
                    span,
                }],
                evaluation_order: vec![0],
                validates_refinements,
                refinement_predicates: if validates_refinements {
                    vec![vec![]]
                } else {
                    vec![]
                },
            },
            ty,
            span,
        };
        for (ty, validating) in [
            (first, false),
            (secret_second, false),
            (secret_refined, false),
            (result, true),
            (secret_result, false),
            (secret_wrong_ok, true),
            (secret_wrong_error, true),
            (secret_refined_ok, true),
            (secret_refined_result, true),
            (secret_first, true),
        ] {
            let original = make(ty, validating);
            assert_eq!(
                normalize_secret_constructor(original.clone(), &types),
                original
            );
        }
        for (qualified, validating, output) in
            [(secret_first, false, first), (secret_result, true, result)]
        {
            let nested = types.intern(Type::Secret(qualified));
            for ty in [qualified, nested] {
                let original = make(ty, validating);
                let normalized = normalize_secret_constructor(original.clone(), &types);
                assert_eq!(normalized.ty, ty);
                assert_eq!(normalized.span, span);
                let ExpressionKind::InterfaceCoerce {
                    value: inner,
                    adapters,
                } = &normalized.kind
                else {
                    panic!("expected exact struct qualification");
                };
                assert_eq!(inner.ty, output);
                assert_eq!(inner.span, span);
                assert_eq!(inner.kind, original.kind);
                assert!(adapters.is_empty());
                assert_eq!(
                    normalize_secret_constructor(normalized.clone(), &types),
                    normalized
                );
            }
        }
        // Even a matching result success TypeId must denote the exact struct,
        // rather than a refinement whose representation happens to be one.
        let mut refined_constructor = make(secret_refined_ok, true);
        let ExpressionKind::StructConstruct { struct_type, .. } = &mut refined_constructor.kind
        else {
            unreachable!();
        };
        *struct_type = refined;
        assert_eq!(
            normalize_secret_constructor(refined_constructor.clone(), &types),
            refined_constructor
        );
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
    fn lowers_secret_enum_constructors_with_nominal_targets() {
        let program = lower_source(
            "namespace app\nenum Choice:\n    empty\n    number(value: int8)\nfunction main() returns nothing:\n    secret[Choice] empty = Choice.empty\n    secret[Choice] number = Choice.number(-128)\n    return nothing\n",
        );
        let main = &program.functions[0];
        let mut target = None;
        for (index, statement) in main.body.statements[..2].iter().enumerate() {
            let StatementKind::Let { value, .. } = &statement.kind else {
                panic!("expected secret enum local");
            };
            let ExpressionKind::EnumConstruct {
                enum_type,
                variant,
                payloads,
                ..
            } = &value.kind
            else {
                panic!("expected nominal enum construction");
            };
            assert_ne!(value.ty, *enum_type, "secrecy belongs to the expression");
            assert_eq!(*target.get_or_insert(*enum_type), *enum_type);
            assert_eq!(variant.index(), index as u32);
            assert_eq!(payloads.len(), index);
        }
    }

    fn return_refinement_guard(
        function: &Function,
    ) -> (&Expression, &[RefinementPredicate], LocalId, &Block) {
        let StatementKind::Return(Some(value)) = &function.body.statements.last().unwrap().kind
        else {
            panic!(
                "expected source return in {}",
                function.identity.declaration.name
            );
        };
        let ExpressionKind::Handle {
            target,
            kind:
                HandleKind::Refinement {
                    refined_type,
                    predicates,
                },
            error_local: Some(error),
            failure,
        } = &value.kind
        else {
            panic!("expected generated return refinement guard: {value:?}");
        };
        assert_eq!(*refined_type, function.return_type);
        assert_eq!(value.ty, function.return_type);
        let StatementKind::Expression(Expression {
            kind: ExpressionKind::RuntimeFailureMessage(message),
            ty,
            ..
        }) = &failure.statements[0].kind
        else {
            panic!("expected exact dynamic predicate rejection");
        };
        assert_eq!(*ty, TypeInterner::NOTHING);
        assert_eq!(message.ty, TypeInterner::STRING);
        assert_eq!(message.kind, ExpressionKind::Local(*error));
        let metadata = &function.locals[error.index() as usize];
        assert_eq!(metadata.ty, TypeInterner::STRING);
        assert!(metadata.view_source.is_none());
        (target, predicates, *error, failure)
    }

    #[test]
    fn native_return_refinement_checks_only_missing_predicate_suffixes() {
        let (program, checked) = lower_source_with_check(
            r#"namespace app
type Positive = int64 where value > 0
type Higher = Positive where value > 10
function from_base(raw: int64) returns Higher:
    return raw
function from_ancestor(ready: Positive) returns Higher:
    return ready
function exact(ready: Higher) returns Higher:
    return ready
function pending(raw: int64) returns Positive:
    return run raw
"#,
            false,
        );
        validate(&program).unwrap();
        validate_backend_types(&program, &checked.interner).unwrap();
        for (name, names) in [
            ("from_base", vec!["app.Positive", "app.Higher"]),
            ("from_ancestor", vec!["app.Higher"]),
            ("pending", vec!["app.Positive"]),
        ] {
            let function = program
                .functions
                .iter()
                .find(|f| f.identity.declaration.name == name)
                .unwrap();
            let (source, predicates, _, _) = return_refinement_guard(function);
            assert_eq!(
                predicates
                    .iter()
                    .map(|p| p.type_name.as_str())
                    .collect::<Vec<_>>(),
                names
            );
            let parameter = function.params[0].local;
            if name == "pending" {
                assert!(matches!(&source.kind, ExpressionKind::Run(value)
                    if matches!(value.kind, ExpressionKind::Local(id) if id == parameter)));
            } else {
                assert_eq!(source.kind, ExpressionKind::Local(parameter));
            }
            assert_eq!(
                source.ty, function.params[0].ty,
                "source signature must stay exact"
            );
        }
        let exact = program
            .functions
            .iter()
            .find(|f| f.identity.declaration.name == "exact")
            .unwrap();
        let StatementKind::Return(Some(value)) = &exact.body.statements[0].kind else {
            panic!("exact return");
        };
        assert_eq!(value.kind, ExpressionKind::Local(exact.params[0].local));
        assert!(
            !exact
                .locals
                .iter()
                .any(|local| local.name == "$return.error")
        );
    }

    #[test]
    fn native_return_refinement_uses_concrete_generic_and_method_targets() {
        let (program, checked) = lower_source_with_check(
            r#"namespace app
type Positive = int64 where value > 0
struct Holder:
    raw: int64
    function promoted(view self: Holder) returns Positive:
        return self.raw
function generic[T](raw: int64, witness: T) returns T:
    return raw
function main() returns nothing:
    Positive seed = 7 handle error: return nothing
    Positive found = generic[Positive](raw: 8, witness: seed)
    int64 plain = generic[int64](raw: 9, witness: 0)
    trace found
    trace plain
    return nothing
"#,
            false,
        );
        validate_backend_types(&program, &checked.interner).unwrap();
        let method = program
            .functions
            .iter()
            .find(|f| f.debug_kind == FunctionDebugKind::Named("app.Holder.promoted".into()))
            .unwrap();
        let (source, predicates, _, _) = return_refinement_guard(method);
        assert!(matches!(source.kind, ExpressionKind::Field { .. }));
        assert_eq!(predicates.len(), 1);
        let generic = program
            .functions
            .iter()
            .filter(|f| f.identity.declaration.name == "generic")
            .collect::<Vec<_>>();
        assert_eq!(generic.len(), 2);
        for function in generic {
            if function.return_type == TypeInterner::INT64 {
                let StatementKind::Return(Some(value)) = &function.body.statements[0].kind else {
                    panic!("plain generic");
                };
                assert_eq!(value.kind, ExpressionKind::Local(function.params[0].local));
                assert!(
                    !function
                        .locals
                        .iter()
                        .any(|local| local.name == "$return.error")
                );
            } else {
                let (source, predicates, _, _) = return_refinement_guard(function);
                assert_eq!(source.ty, TypeInterner::INT64);
                assert_eq!(predicates.len(), 1);
                assert_eq!(
                    predicates[0].refined_type,
                    function.identity.type_arguments[0]
                );
            }
        }
    }

    #[test]
    fn native_return_refinement_inline_targets_restore_enclosing_scope() {
        let (program, checked) = lower_source_with_check(
            r#"namespace app
type Positive = int64 where value > 0
type Higher = Positive where value > 10
function outer(raw: int64) returns Higher:
    function() returns int64 plain = function() returns int64:
        return raw
    function() returns Positive narrowed = function() returns Positive:
        function() returns int64 nested = function() returns int64:
            return raw
        return raw
    return raw
"#,
            false,
        );
        validate(&program).unwrap();
        validate_backend_types(&program, &checked.interner).unwrap();
        let outer = program
            .functions
            .iter()
            .find(|f| f.identity.declaration.name == "outer")
            .unwrap();
        let (_, predicates, _, _) = return_refinement_guard(outer);
        assert_eq!(
            predicates
                .iter()
                .map(|p| p.type_name.as_str())
                .collect::<Vec<_>>(),
            ["app.Positive", "app.Higher"]
        );
        let callbacks = program
            .functions
            .iter()
            .filter(|f| f.debug_kind == FunctionDebugKind::Inline)
            .collect::<Vec<_>>();
        assert_eq!(callbacks.len(), 3);
        for callback in callbacks {
            if callback.return_type == TypeInterner::INT64 {
                let StatementKind::Return(Some(value)) = &callback.body.statements[0].kind else {
                    panic!("plain inline return");
                };
                assert!(matches!(value.kind, ExpressionKind::Local(_)));
            } else {
                let (_, predicates, _, _) = return_refinement_guard(callback);
                assert_eq!(
                    predicates
                        .iter()
                        .map(|p| p.type_name.as_str())
                        .collect::<Vec<_>>(),
                    ["app.Positive"]
                );
            }
        }
    }

    #[test]
    fn native_return_refinement_survives_reflected_body_fact_switches() {
        let program = lower_source(
            r#"namespace app
type Positive = int64 where value > 0
struct Mixed:
    label: string
    count: int64
function reflected[T](view model: T, raw: int64) returns Positive:
    for field in type.fields[T]():
        comptime type Field = field.type_info:
            return raw
    return raw
function main() returns nothing:
    Mixed model = Mixed(label: "one", count: 1)
    Positive found = reflected[Mixed](view model, 7)
    trace found
    return nothing
"#,
        );
        let function = program
            .functions
            .iter()
            .find(|f| f.identity.declaration.name == "reflected")
            .unwrap();
        let StatementKind::For { body, .. } = &function.body.statements[0].kind else {
            panic!("reflected loop");
        };
        let StatementKind::ReflectedTypeDispatch { arms, .. } = &body.statements[0].kind else {
            panic!("bound types");
        };
        assert_eq!(arms.len(), 2);
        for arm in arms {
            let StatementKind::Return(Some(value)) = &arm.body.statements[0].kind else {
                panic!("bound return");
            };
            let ExpressionKind::Handle {
                target,
                kind:
                    HandleKind::Refinement {
                        refined_type,
                        predicates,
                    },
                ..
            } = &value.kind
            else {
                panic!("bound return guard");
            };
            assert_eq!(*refined_type, function.return_type);
            assert_eq!(target.ty, TypeInterner::INT64);
            assert_eq!(target.kind, ExpressionKind::Local(function.params[1].local));
            assert_eq!(predicates[0].type_name, "app.Positive");
        }
        let (_, predicates, _, _) = return_refinement_guard(function);
        assert_eq!(predicates[0].type_name, "app.Positive");
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
    fn lowers_whole_sum_refinements_before_sum_extraction() {
        let program = lower_source(
            r#"namespace app
type Choice = optional[int64] where true
type Outcome = result[int64, int64] where true
function check(maybe: optional[int64], outcome: result[int64, int64], wrapped: result[Outcome, int64], optional_wrapped: optional[Choice]) returns nothing:
    Choice first = maybe handle error: return nothing
    Outcome second = outcome handle error: return nothing
    Outcome third = wrapped handle error: return nothing
    Choice fourth = optional_wrapped handle: return nothing
    return nothing
"#,
        );
        let function = &program.functions[0];
        for (index, expected) in [
            TypeInterner::STRING,
            TypeInterner::STRING,
            TypeInterner::INT64,
        ]
        .into_iter()
        .enumerate()
        {
            let StatementKind::Let { value, .. } = &function.body.statements[index].kind else {
                panic!("expected a checked binding");
            };
            let ExpressionKind::Handle {
                kind,
                error_local: Some(error),
                ..
            } = &value.kind
            else {
                panic!("expected a checked handle");
            };
            assert_eq!(function.locals[error.index() as usize].ty, expected);
            if index < 2 {
                assert!(
                    matches!(kind, HandleKind::Refinement { predicates, .. } if predicates.len() == 1)
                );
            } else {
                assert!(matches!(kind, HandleKind::Result));
            }
        }
        let StatementKind::Let { value, .. } = &function.body.statements[3].kind else {
            panic!("expected optional extraction");
        };
        assert!(matches!(
            value.kind,
            ExpressionKind::Handle {
                kind: HandleKind::Optional,
                ..
            }
        ));
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
    fn secret_bitfield_constructors_preserve_plain_and_validating_outputs() {
        let (program, checked) = lower_source_with_check(
            r#"bitfield network Header:
    first: 4 bits
    last: 4 bits
    payload: list[uint8]
type HeaderAlias = Header
function fixed(payload: list[uint8]) returns secret[HeaderAlias]:
    return ((Header))(payload: payload, last: 15, first: 0)
function dynamic(width: int64, payload: list[uint8]) returns secret[result[Header, string]]:
    return Header(payload: payload, last: width, first: 0)
function nested(width: int64, payload: list[uint8]) returns secret[secret[result[Header, string]]]:
    return (Header(first: 0, last: width, payload: payload))
function nested_plain(payload: list[uint8]) returns secret[secret[Header]]:
    return (Header(first: 0, last: 1, payload: payload))
function pending(payload: list[uint8]) returns secret[Header]:
    return run (Header(first: 1, last: 2, payload: payload))
function parenthesized_number(payload: list[uint8]) returns secret[result[Header, string]]:
    return Header(first: 0, last: (7), payload: payload)
function handled(maybe: optional[list[uint8]]) returns secret[Header]:
    return Header(first: 1, last: 2, payload: maybe handle:
        default list()
    )
function generic[T](unused: T, payload: list[uint8]) returns secret[Header]:
    return Header(first: 1, last: 2, payload: payload)
function ordinary(width: int64, payload: list[uint8]) returns result[Header, string]:
    return Header(first: 0, last: width, payload: payload)
function producer(payload: list[uint8]) returns secret[Header]:
    return fixed(payload)
function main() returns nothing:
    secret[Header] local = Header(first: 1, last: 2, payload: list())
    secret[result[Header, string]] validation = Header(first: 1, last: (2), payload: list())
    secret[Header] specialized = generic[int64](3, list())
    return nothing
"#,
            false,
        );
        for (name, validating, depth, order) in [
            ("fixed", false, 1, vec![2, 1, 0]),
            ("dynamic", true, 1, vec![2, 1, 0]),
            ("nested", true, 2, vec![0, 1, 2]),
            ("nested_plain", false, 2, vec![0, 1, 2]),
            ("pending", false, 1, vec![0, 1, 2]),
            ("parenthesized_number", true, 1, vec![0, 1, 2]),
            ("handled", false, 1, vec![0, 1, 2]),
            ("generic", false, 1, vec![0, 1, 2]),
        ] {
            let function = program
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == name)
                .unwrap();
            let StatementKind::Return(Some(value)) = &function.body.statements[0].kind else {
                panic!("{name}: expected returned constructor");
            };
            assert_eq!(value.ty, function.return_type, "{name}");
            let value = if name == "pending" {
                let ExpressionKind::Run(inner) = &value.kind else {
                    panic!("pending construction must retain run");
                };
                assert_eq!(inner.ty, value.ty);
                inner.as_ref()
            } else {
                value
            };
            let mut expected = value.ty;
            for _ in 0..depth {
                let Type::Secret(inner) = checked.interner.resolve(expected) else {
                    panic!("{name}: missing checked secret layer");
                };
                expected = *inner;
            }
            let ExpressionKind::InterfaceCoerce {
                value: constructor,
                adapters,
            } = &value.kind
            else {
                panic!("{name}: expected qualification boundary");
            };
            assert!(adapters.is_empty());
            assert_eq!(constructor.ty, expected, "{name}");
            let ExpressionKind::BitfieldConstruct {
                bitfield_type,
                fields,
                evaluation_order,
                validates_widths,
            } = &constructor.kind
            else {
                panic!("{name}: expected bitfield constructor");
            };
            assert_eq!(*validates_widths, validating, "{name}");
            assert_eq!(evaluation_order, &order, "{name}");
            let Type::Bitfield(id) = checked.interner.resolve(*bitfield_type) else {
                panic!("exact nominal bitfield");
            };
            let definition = checked.interner.resolve_bitfield(*id);
            assert!(definition.network_order);
            assert!(
                fields
                    .iter()
                    .zip(&definition.fields)
                    .all(|(value, field)| value.ty == field.ty)
            );
            if validating {
                assert_eq!(
                    checked.interner.resolve(constructor.ty),
                    &Type::Result(*bitfield_type, TypeInterner::STRING),
                    "{name}"
                );
            } else {
                assert_eq!(constructor.ty, *bitfield_type, "{name}");
            }
            if name == "handled" {
                assert!(matches!(fields[2].kind, ExpressionKind::Handle { .. }));
            }
        }
        for name in ["ordinary", "producer"] {
            let function = program
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == name)
                .unwrap();
            let StatementKind::Return(Some(value)) = &function.body.statements[0].kind else {
                panic!("control return");
            };
            if name == "ordinary" {
                assert!(matches!(
                    value.kind,
                    ExpressionKind::BitfieldConstruct {
                        validates_widths: true,
                        ..
                    }
                ));
            } else {
                assert!(matches!(value.kind, ExpressionKind::Call { .. }));
            }
            assert_eq!(value.ty, checked.type_map[&value.span]);
        }
        let main = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "main")
            .unwrap();
        for (statement, validating) in main.body.statements[..2].iter().zip([false, true]) {
            let StatementKind::Let { local, value } = &statement.kind else {
                panic!("constructor local");
            };
            assert_eq!(value.ty, main.locals[local.index() as usize].ty);
            let ExpressionKind::InterfaceCoerce { value: inner, .. } = &value.kind else {
                panic!("local qualification");
            };
            assert_eq!(inner.span, value.span);
            assert!(
                matches!(inner.kind, ExpressionKind::BitfieldConstruct { validates_widths, .. } if validates_widths == validating)
            );
        }
    }

    #[test]
    fn secret_machine_constructors_preserve_state_payload_conversions_and_wrappers() {
        let (program, checked) = lower_source_with_check(
            r#"namespace models
export interface Named:
    function name(view self: Named) returns string
export struct Item:
    label: string
implement Named for Item:
    function name(view self: Item) returns string:
        return self.label
export type Callback = function(int64) returns secret[int64]
export machine Session:
    states:
        active(label: string, owner: Named, callback: Callback, count: int64)
        idle
    transitions:
        active to idle
export bitfield Header:
    value: 4 bits
namespace app
function classified(value: secret[int64]) returns secret[int64]:
    return value
function active(item: models.Item, maybe: optional[int64]) returns secret[models.Session at active]:
    use models
    use models as m
    return (m.Session)(active, "Ada", item, classified, maybe handle:
        default 7
    )
function nested() returns secret[secret[models.Session at idle]]:
    use models
    use models as m
    return (m.Session(idle))
function pending() returns secret[models.Session at idle]:
    use models
    use models as m
    return run (m.Session(idle))
function generic[T](unused: T) returns secret[models.Session at idle]:
    use models
    use models as m
    return m.Session(idle)
function header() returns secret[models.Header]:
    use models
    use models as m
    return (m.Header)(value: 7)
function main() returns nothing:
    use models as m
    use models
    secret[models.Session at idle] local = m.Session(idle)
    secret[models.Session at idle] specialized = generic[int64](3)
    return nothing
"#,
            false,
        );
        for (name, depth) in [("active", 1), ("nested", 2), ("pending", 1), ("generic", 1)] {
            let function = program
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == name)
                .unwrap();
            let StatementKind::Return(Some(value)) = &function.body.statements[0].kind else {
                panic!("{name}: state constructor return");
            };
            assert_eq!(value.ty, function.return_type);
            let value = if name == "pending" {
                let ExpressionKind::Run(inner) = &value.kind else {
                    panic!("preserved run");
                };
                assert_eq!(inner.ty, value.ty);
                inner.as_ref()
            } else {
                value
            };
            let mut state_type = value.ty;
            for _ in 0..depth {
                let Type::Secret(inner) = checked.interner.resolve(state_type) else {
                    panic!("retained secret layer");
                };
                state_type = *inner;
            }
            let ExpressionKind::InterfaceCoerce {
                value: constructor,
                adapters,
            } = &value.kind
            else {
                panic!("{name}: state qualification");
            };
            assert!(adapters.is_empty());
            assert_eq!(constructor.ty, state_type);
            let ExpressionKind::MachineConstruct {
                state_type: actual,
                state,
                payloads,
            } = &constructor.kind
            else {
                panic!("{name}: exact machine constructor");
            };
            assert_eq!(*actual, state_type);
            let Type::MachineState {
                machine,
                state: expected,
            } = checked.interner.resolve(state_type)
            else {
                panic!("exact nominal state");
            };
            assert_eq!(state.index(), expected.index());
            let definition = checked
                .interner
                .resolve_machine(*machine)
                .state(*expected)
                .unwrap();
            assert_eq!(payloads.len(), definition.fields.len());
            assert!(
                payloads
                    .iter()
                    .zip(&definition.fields)
                    .all(|(value, (_, ty))| value.ty == *ty)
            );
            if name == "active" {
                assert_eq!(definition.name, "active");
                assert!(
                    matches!(payloads[0].kind, ExpressionKind::String(ref label) if label == "Ada")
                );
                assert!(matches!(
                    payloads[1].kind,
                    ExpressionKind::InterfaceCoerce { .. }
                ));
                assert!(matches!(
                    payloads[2].kind,
                    ExpressionKind::FunctionAdapter { .. }
                ));
                assert!(matches!(payloads[3].kind, ExpressionKind::Handle { .. }));
            } else {
                assert_eq!(definition.name, "idle");
                assert!(payloads.is_empty());
            }
        }
        let main = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "main")
            .unwrap();
        let StatementKind::Let { local, value } = &main.body.statements[0].kind else {
            panic!("qualified local state");
        };
        assert_eq!(value.ty, main.locals[local.index() as usize].ty);
        let ExpressionKind::InterfaceCoerce {
            value: constructor, ..
        } = &value.kind
        else {
            panic!("local state qualification");
        };
        assert!(matches!(
            constructor.kind,
            ExpressionKind::MachineConstruct { .. }
        ));
        let header = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "header")
            .unwrap();
        assert!(
            matches!(&header.body.statements[0].kind, StatementKind::Return(Some(Expression { kind: ExpressionKind::InterfaceCoerce { value, .. }, .. })) if matches!(value.kind, ExpressionKind::BitfieldConstruct { validates_widths: false, .. }))
        );
    }

    #[test]
    fn secret_bitfield_and_machine_normalization_requires_exact_targets() {
        let mut types = TypeInterner::new();
        let first_id = types.add_bitfield(jett_types::BitfieldDef {
            name: "First".into(),
            network_order: false,
            fields: vec![],
        });
        let first = types.intern(Type::Bitfield(first_id));
        let other_id = types.add_bitfield(jett_types::BitfieldDef {
            name: "Other".into(),
            network_order: false,
            fields: vec![],
        });
        let other = types.intern(Type::Bitfield(other_id));
        let result = types.intern(Type::Result(first, TypeInterner::STRING));
        let wrong_ok = types.intern(Type::Result(other, TypeInterner::STRING));
        let wrong_error = types.intern(Type::Result(first, TypeInterner::INT64));
        let refined = types.intern(Type::Refinement {
            name: "Selected".into(),
            base: first,
        });
        let machine_id = types.add_machine(jett_types::MachineDef {
            name: "Session".into(),
            states: vec![
                jett_types::MachineStateDef {
                    name: "active".into(),
                    fields: vec![],
                },
                jett_types::MachineStateDef {
                    name: "idle".into(),
                    fields: vec![],
                },
            ],
            transitions: vec![],
        });
        let machine = types.intern(Type::Machine(machine_id));
        let active = types.intern(Type::MachineState {
            machine: machine_id,
            state: jett_types::MachineStateId::new(0),
        });
        let idle = types.intern(Type::MachineState {
            machine: machine_id,
            state: jett_types::MachineStateId::new(1),
        });
        let span = Span::new(FileId::new(0), 3, 9);
        let bitfield = |validates_widths| ExpressionKind::BitfieldConstruct {
            bitfield_type: first,
            fields: vec![],
            evaluation_order: vec![],
            validates_widths,
        };
        let state = |state_type, index| ExpressionKind::MachineConstruct {
            state_type,
            state: StateId(index),
            payloads: vec![],
        };
        for (output, kind, accepted) in [
            (first, bitfield(false), true),
            (result, bitfield(true), true),
            (first, bitfield(true), false),
            (result, bitfield(false), false),
            (other, bitfield(false), false),
            (wrong_ok, bitfield(true), false),
            (wrong_error, bitfield(true), false),
            (refined, bitfield(false), false),
            (active, state(active, 0), true),
            (active, state(active, 1), false),
            (active, state(idle, 0), false),
            (machine, state(active, 0), false),
            (
                active,
                ExpressionKind::Call {
                    function: FunctionId::new(0),
                    args: vec![],
                    evaluation_order: vec![],
                },
                false,
            ),
        ] {
            let secret = types.intern(Type::Secret(output));
            let twice = types.intern(Type::Secret(secret));
            for qualified in [secret, twice] {
                let original = Expression {
                    kind: kind.clone(),
                    ty: qualified,
                    span,
                };
                let normalized = normalize_secret_constructor(original.clone(), &types);
                if accepted {
                    assert_eq!(normalized.ty, qualified);
                    assert_eq!(normalized.span, span);
                    let ExpressionKind::InterfaceCoerce { value, adapters } = &normalized.kind
                    else {
                        panic!("exact qualified constructor");
                    };
                    assert_eq!(value.ty, output);
                    assert_eq!(value.span, span);
                    assert_eq!(value.kind, original.kind);
                    assert!(adapters.is_empty());
                    assert_eq!(
                        normalize_secret_constructor(normalized.clone(), &types),
                        normalized
                    );
                } else {
                    assert_eq!(normalized, original);
                }
            }
        }
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
        let constructor = &program.functions[1];
        assert_eq!(
            constructor.identity.declaration.kind,
            DeclarationKind::ActorConstructor
        );
        assert_eq!(constructor.params[0].name, "seed");
        assert_eq!(constructor.params[1].name, "step");
        assert!(matches!(
            constructor.body.statements[constructor.params.len()].kind,
            StatementKind::Let {
                value: Expression {
                    kind: ExpressionKind::Local(_),
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            value.kind,
            ExpressionKind::ActorSpawn {
                constructor: Some(id),
                ..
            } if id == constructor.id
        ));

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
        assert_eq!(main.debug_kind, FunctionDebugKind::Named("app.main".into()));
        assert_eq!(program.functions[1].debug_kind, FunctionDebugKind::Inline);
        assert_eq!(program.functions[1].params[0].name, "value");
        assert!(matches!(
            main.body.statements[1].kind,
            StatementKind::Return(Some(Expression {
                kind: ExpressionKind::IndirectCall { .. },
                ..
            }))
        ));
    }

    #[test]
    fn lowers_checked_function_expression_callees_without_losing_direct_calls() {
        let source = r#"namespace app
function increment(value: int64) returns int64:
    return value + 1
function factory(delta: int64) returns function(int64) returns int64:
    return function(value: int64) returns int64: return value + delta
struct Wrapper:
    invoke: function(int64) returns int64
function parenthesized() returns int64:
    function(int64) returns int64 callback = increment
    return (callback)(4)
function returned() returns int64:
    return factory(3)(4)
function projected() returns int64:
    Wrapper wrapper = Wrapper(invoke: increment)
    return wrapper.invoke(4)
function inline_call() returns int64:
    return (function(value: int64) returns int64: return value * 2)(4)
function captured_call() returns int64:
    int64 delta = 3
    return (function(value: int64) returns int64: return value + delta)(4)
function direct() returns int64:
    return increment(4)
"#;
        let program = lower_source(source);
        let returned_value = |name: &str| {
            let function = program
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == name)
                .expect("checked caller");
            let StatementKind::Return(Some(value)) = &function
                .body
                .statements
                .last()
                .expect("return statement")
                .kind
            else {
                panic!("expected returned value in {name}");
            };
            value
        };
        for name in [
            "parenthesized",
            "returned",
            "projected",
            "inline_call",
            "captured_call",
        ] {
            let ExpressionKind::IndirectCall {
                callee,
                args,
                evaluation_order,
            } = &returned_value(name).kind
            else {
                panic!("expected indirect call in {name}");
            };
            assert_eq!(evaluation_order, &[0]);
            assert!(matches!(args[0].kind, ExpressionKind::Int(4)));
            assert!(match name {
                "parenthesized" => matches!(callee.kind, ExpressionKind::Local(_)),
                "returned" => matches!(callee.kind, ExpressionKind::Call { .. }),
                "projected" => matches!(callee.kind, ExpressionKind::Field { .. }),
                "inline_call" => matches!(callee.kind, ExpressionKind::FunctionRef(_)),
                "captured_call" => matches!(callee.kind, ExpressionKind::ClosureRef { .. }),
                _ => unreachable!(),
            });
        }
        assert!(matches!(
            returned_value("direct").kind,
            ExpressionKind::Call { .. }
        ));
        let projected = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "projected")
            .expect("projected caller");
        assert!(matches!(
            projected.body.statements[0].kind,
            StatementKind::Let {
                value: Expression {
                    kind: ExpressionKind::StructConstruct { .. },
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn expression_callees_retain_view_arguments_and_argument_handlers() {
        let source = r#"namespace app
function choose(view first: int64, second: int64) returns int64:
    return first + second
function main() returns int64:
    function(view int64, int64) returns int64 callback = choose
    optional[int64] fallback = none
    return (callback)(view 3, fallback handle:
        default 4
    )
"#;
        let program = lower_source(source);
        let StatementKind::Return(Some(Expression {
            kind:
                ExpressionKind::IndirectCall {
                    callee,
                    args,
                    evaluation_order,
                },
            ..
        })) = &program.functions[1].body.statements[2].kind
        else {
            panic!("expected handled expression-callee call");
        };
        assert!(matches!(callee.kind, ExpressionKind::Local(_)));
        assert_eq!(evaluation_order, &[0, 1]);
        assert!(matches!(args[0].kind, ExpressionKind::View(_)));
        assert!(matches!(args[1].kind, ExpressionKind::Handle { .. }));
        assert_eq!(program.functions[0].params[0].mode, ParamMode::View);
    }

    #[test]
    fn parenthesized_callees_preserve_checked_declaration_and_intrinsic_identity() {
        let source = r#"namespace app
function subtract(left: int64, right: int64) returns int64:
    return left - right
function identity[T](value: T) returns T:
    return value
struct Pair:
    left: int64
    right: int64
enum Choice:
    item(left: int64, right: int64)
function main() returns int64:
    int64 named = ((subtract))(right: 3, left: 10)
    int64 qualified = (app.subtract)(right: 2, left: 9)
    int64 explicit = (identity)[int64](value: 8)
    int64 inferred = (identity)(value: 7)
    Pair pair = (Pair)(right: 5, left: 6)
    Choice choice = (Choice.item)(right: 4, left: 9)
    (println)(named)
    return qualified
"#;
        let program = lower_source(source);
        let main = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "main")
            .expect("main function");
        for (index, first, second) in [(0, 10, 3), (1, 9, 2)] {
            let StatementKind::Let {
                value:
                    Expression {
                        kind:
                            ExpressionKind::Call {
                                args,
                                evaluation_order,
                                ..
                            },
                        ..
                    },
                ..
            } = &main.body.statements[index].kind
            else {
                panic!("expected direct named call");
            };
            assert_eq!(evaluation_order, &[1, 0]);
            assert!(matches!(args[0].kind, ExpressionKind::Int(value) if value == first));
            assert!(matches!(args[1].kind, ExpressionKind::Int(value) if value == second));
        }
        for index in [2, 3] {
            assert!(matches!(
                main.body.statements[index].kind,
                StatementKind::Let {
                    value: Expression {
                        kind: ExpressionKind::Call { .. },
                        ..
                    },
                    ..
                }
            ));
        }
        let StatementKind::Let {
            value:
                Expression {
                    kind:
                        ExpressionKind::StructConstruct {
                            fields,
                            evaluation_order,
                            ..
                        },
                    ..
                },
            ..
        } = &main.body.statements[4].kind
        else {
            panic!("expected parenthesized struct constructor");
        };
        assert_eq!(evaluation_order, &[1, 0]);
        assert!(matches!(fields[0].kind, ExpressionKind::Int(6)));
        assert!(matches!(fields[1].kind, ExpressionKind::Int(5)));
        let StatementKind::Let {
            value:
                Expression {
                    kind:
                        ExpressionKind::EnumConstruct {
                            payloads,
                            evaluation_order,
                            ..
                        },
                    ..
                },
            ..
        } = &main.body.statements[5].kind
        else {
            panic!("expected parenthesized named enum constructor");
        };
        assert_eq!(evaluation_order, &[1, 0]);
        assert!(matches!(payloads[0].kind, ExpressionKind::Int(9)));
        assert!(matches!(payloads[1].kind, ExpressionKind::Int(4)));
        assert!(matches!(
            main.body.statements[6].kind,
            StatementKind::Expression(Expression {
                kind: ExpressionKind::Intrinsic {
                    intrinsic: IntrinsicId::Println,
                    ..
                },
                ..
            })
        ));
    }

    #[test]
    fn extracts_captured_inline_functions_with_environment_parameters() {
        let source = r#"namespace app
function make(seed: int64) returns function(int64) returns int64:
    return function(value: int64) returns int64: return value + seed
"#;
        let program = lower_source(source);
        assert_eq!(program.functions.len(), 2);
        assert!(matches!(
            program.functions[0].body.statements[0].kind,
            StatementKind::Return(Some(Expression {
                kind: ExpressionKind::ClosureRef {
                    function: FunctionId(1),
                    ref captures,
                },
                ..
            })) if captures == &[LocalId::new(0)]
        ));
        assert_eq!(program.functions[1].capture_count, 1);
        assert_eq!(program.functions[1].params.len(), 2);
        assert_eq!(program.functions[1].debug_kind, FunctionDebugKind::Inline);
        assert_eq!(program.functions[1].params[0].name, "seed");
        assert_eq!(program.functions[1].params[1].name, "value");
    }

    #[test]
    fn generic_inline_parameters_retain_each_concrete_signature_and_source_identity() {
        let program = lower_source(
            r#"namespace app
function factory[T](seed: T) returns function(view T) returns T:
    return function(view input: T) returns T: return seed
function main() returns int64:
    function(view int64) returns int64 integer = factory[int64](1)
    function(view string) returns string text = factory[string]("value")
    return integer(view 2)
"#,
        );
        let mut types = Vec::new();
        for function in &program.functions {
            if function.debug_kind != FunctionDebugKind::Inline {
                continue;
            }
            let concrete = function.identity.type_arguments[0];
            types.push(concrete);
            assert_eq!(function.capture_count, 1);
            assert_eq!(function.params[0].name, "seed");
            assert_eq!(function.params[0].ty, concrete);
            assert_eq!(function.params[1].name, "input");
            assert_eq!(function.params[1].ty, concrete);
            assert_eq!(function.params[1].mode, ParamMode::View);
            assert_eq!(function.return_type, concrete);
        }
        types.sort_by_key(|ty| ty.index());
        assert_eq!(types, vec![TypeInterner::INT64, TypeInterner::STRING]);
    }

    #[test]
    fn validates_closure_captures_by_type_across_local_id_spaces() {
        let source = r#"namespace app
function make(seed: int64) returns function(int64) returns int64:
    int64 alternate = 2
    return function(value: int64) returns int64: return value + seed
"#;
        let mut program = lower_source(source);
        let StatementKind::Return(Some(Expression {
            kind: ExpressionKind::ClosureRef { captures, .. },
            ..
        })) = &mut program.functions[0].body.statements[1].kind
        else {
            panic!("expected captured closure return");
        };
        captures[0] = LocalId::new(1);
        validate(&program).expect("caller capture IDs need only match target parameter types");

        program.functions[0].locals[1].ty = TypeInterner::STRING;
        assert!(
            validate(&program).is_err(),
            "capture type mismatch must fail"
        );
    }

    #[test]
    fn extracts_inline_function_view_parameter() {
        let program = lower_source(
            r#"namespace app
function make() returns function(view int64) returns int64:
    return function(view value: int64) returns int64: return value
"#,
        );
        assert_eq!(program.functions.len(), 2);
        assert!(matches!(
            program.functions[0].body.statements[0].kind,
            StatementKind::Return(Some(Expression {
                kind: ExpressionKind::FunctionRef(FunctionId(1)),
                ..
            }))
        ));
        assert_eq!(program.functions[1].params[0].mode, ParamMode::View);
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
    fn lowers_function_value_pipeline_steps_with_checked_views_and_arguments() {
        let source = r#"namespace app
function choose(view first: int64, second: int64) returns int64:
    return first + second
function apply(callback: function(view int64, int64) returns int64) returns int64:
    return 1 into view callback(2)
function main() returns int64:
    function(view int64, int64) returns int64 callback = choose
    return 3 into view callback(4)
"#;
        let program = lower_source(source);
        let choose = &program.functions[0];
        assert_eq!(choose.params[0].mode, ParamMode::View);
        for (function_index, statement_index, first, second) in [(1, 0, 1, 2), (2, 1, 3, 4)] {
            let StatementKind::Return(Some(Expression {
                kind:
                    ExpressionKind::IndirectCall {
                        callee,
                        args,
                        evaluation_order,
                    },
                ..
            })) = &program.functions[function_index].body.statements[statement_index].kind
            else {
                panic!("expected function-value pipeline call");
            };
            assert!(matches!(callee.kind, ExpressionKind::Local(LocalId(0))));
            assert_eq!(evaluation_order, &[0, 1]);
            let ExpressionKind::View(input) = &args[0].kind else {
                panic!("expected viewed pipeline input");
            };
            assert!(matches!(input.kind, ExpressionKind::Int(value) if value == first));
            assert!(matches!(args[1].kind, ExpressionKind::Int(value) if value == second));
        }
    }

    #[test]
    fn lowers_expression_pipeline_targets_without_unwrapping_grouped_calls() {
        let source = r#"namespace app
function choose(view first: int64, second: int64) returns int64:
    return first + second
function factory() returns function(view int64, int64) returns int64:
    return choose
struct Wrapper:
    invoke: function(view int64, int64) returns int64
function projected() returns int64:
    Wrapper wrapper = Wrapper(invoke: choose)
    return 3 into view wrapper.invoke(4)
function grouped() returns int64:
    return 3 into view (factory())(4)
function parenthesized() returns int64:
    function(view int64, int64) returns int64 callback = choose
    return 3 into view (callback)(4)
function unary_factory() returns function(int64) returns int64:
    return function(value: int64) returns int64: return value + 1
function grouped_without_arguments() returns int64:
    return 3 into (unary_factory())
"#;
        let program = lower_source(source);
        for name in [
            "projected",
            "grouped",
            "parenthesized",
            "grouped_without_arguments",
        ] {
            let function = program
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == name)
                .expect("pipeline caller");
            let StatementKind::Return(Some(Expression {
                kind:
                    ExpressionKind::IndirectCall {
                        callee,
                        args,
                        evaluation_order,
                    },
                ..
            })) = &function
                .body
                .statements
                .last()
                .expect("return statement")
                .kind
            else {
                panic!("expected expression pipeline call in {name}");
            };
            assert!(match name {
                "projected" => matches!(callee.kind, ExpressionKind::Field { .. }),
                "grouped" | "grouped_without_arguments" => {
                    matches!(&callee.kind, ExpressionKind::Call { args, .. } if args.is_empty())
                }
                "parenthesized" => matches!(callee.kind, ExpressionKind::Local(_)),
                _ => unreachable!(),
            });
            if name == "grouped_without_arguments" {
                assert_eq!(evaluation_order, &[0]);
                assert!(matches!(args[0].kind, ExpressionKind::Int(3)));
            } else {
                assert_eq!(evaluation_order, &[0, 1]);
                assert!(matches!(args[0].kind, ExpressionKind::View(_)));
                assert!(matches!(args[1].kind, ExpressionKind::Int(4)));
            }
        }
    }

    #[test]
    fn named_enum_calls_and_pipeline_payloads_use_checked_order_through_aliases() {
        let source = r#"namespace models
export enum Choice:
    item(first: int64, second: int64, third: int64)
namespace app
function direct() returns models.Choice:
    use models as m
    return (m.Choice.item)(third: 3, first: 1, second: 2)
function piped() returns models.Choice:
    use models as m
    return 1 into (m.Choice.item)(third: 3, second: 2)
"#;
        let program = lower_source(source);
        for (index, expected_order) in [(0, vec![2, 0, 1]), (1, vec![0, 2, 1])] {
            let StatementKind::Return(Some(Expression {
                kind:
                    ExpressionKind::EnumConstruct {
                        payloads,
                        evaluation_order,
                        ..
                    },
                ..
            })) = &program.functions[index]
                .body
                .statements
                .last()
                .expect("return statement")
                .kind
            else {
                panic!("expected named enum construction");
            };
            assert_eq!(evaluation_order, &expected_order);
            assert!(matches!(payloads[0].kind, ExpressionKind::Int(1)));
            assert!(matches!(payloads[1].kind, ExpressionKind::Int(2)));
            assert!(matches!(payloads[2].kind, ExpressionKind::Int(3)));
        }
    }

    #[test]
    fn function_fields_returning_enums_are_not_variant_constructors() {
        let source = r#"namespace app
enum Choice:
    item(value: int64)
    other(value: int64)
function other(value: int64) returns Choice:
    return Choice.other(value)
struct Holder:
    item: function(int64) returns Choice
function direct() returns Choice:
    Holder holder = Holder(item: other)
    return holder.item(4)
function piped() returns Choice:
    Holder holder = Holder(item: other)
    return 4 into holder.item
"#;
        let program = lower_source(source);
        for function in &program.functions[1..] {
            let StatementKind::Return(Some(Expression {
                kind: ExpressionKind::IndirectCall { callee, .. },
                ..
            })) = &function.body.statements[1].kind
            else {
                panic!("expected function field call");
            };
            assert!(matches!(callee.kind, ExpressionKind::Field { .. }));
        }
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

    #[test]
    fn contextual_absent_handle_preserves_the_local_producer_signature() {
        let program = lower_source(
            r#"function defaulted[T](value: optional[T]) returns int64:
    return value handle: default 7
function main() returns int64:
    return defaulted(none)
"#,
        );
        let defaulted = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "defaulted")
            .unwrap();
        let StatementKind::Return(Some(Expression {
            kind: ExpressionKind::Handle { target, .. },
            ty,
            ..
        })) = &defaulted.body.statements[0].kind
        else {
            panic!("expected contextual handle");
        };
        assert_eq!(*ty, TypeInterner::INT64);
        assert_eq!(target.ty, defaulted.params[0].ty);
        assert!(matches!(target.kind, ExpressionKind::Local(_)));
        assert_eq!(defaulted.identity.type_arguments, [TypeInterner::NEVER]);
    }

    #[test]
    fn contextual_inhabited_handle_converts_the_sum_before_payload_extraction() {
        let program = lower_source(
            r#"function defaulted[T](value: optional[list[T]]) returns list[string]:
    return value handle: default list("fallback")
function main() returns nothing:
    list[string] values = defaulted(some(list()))
    trace values
    return nothing
"#,
        );
        let defaulted = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "defaulted")
            .unwrap();
        let StatementKind::Return(Some(Expression {
            kind: ExpressionKind::Handle { target, .. },
            ..
        })) = &defaulted.body.statements[0].kind
        else {
            panic!("expected contextual handle");
        };
        let ExpressionKind::InterfaceCoerce { value, adapters } = &target.kind else {
            panic!("expected explicit contextual sum conversion");
        };
        assert_eq!(value.ty, defaulted.params[0].ty);
        assert!(matches!(value.kind, ExpressionKind::Local(_)));
        assert_ne!(target.ty, value.ty);
        assert!(adapters.is_empty());
        assert_eq!(defaulted.identity.type_arguments, [TypeInterner::NEVER]);
    }

    #[test]
    fn mixed_empty_literals_rebuild_the_first_nested_list_ownership_shape() {
        let program = lower_source(
            r#"function main() returns nothing:
    for values in list(list(), list("ready")):
        trace values
    return nothing
"#,
        );
        let StatementKind::For { iterable, .. } = &program.functions[0].body.statements[0].kind
        else {
            panic!("expected inferred iteration");
        };
        let ExpressionKind::ListConstruct { elements } = &iterable.kind else {
            panic!("expected mixed literal");
        };
        let ExpressionKind::InterfaceCoerce { value, .. } = &elements[0].kind else {
            panic!("expected empty list conversion");
        };
        assert_ne!(value.ty, elements[0].ty);
        assert_eq!(elements[0].ty, elements[1].ty);
        assert!(
            matches!(&value.kind, ExpressionKind::ListConstruct { elements } if elements.is_empty())
        );
    }

    #[test]
    fn mixed_callable_empty_returns_keep_source_signature_and_convert_adapter_return() {
        let program = lower_source(
            r#"function make_empty[T](view items: list[T]) returns function() returns list[T]:
    return function() returns list[T]: return list()
function ready() returns list[string]:
    return list("ready")
function main() returns nothing:
    for callback in list(make_empty(list()), ready):
        list[string] values = callback()
        trace values
    return nothing
"#,
        );
        let adapter = program
            .functions
            .iter()
            .find(|function| {
                function
                    .identity
                    .declaration
                    .name
                    .starts_with("$interface.adapter.")
            })
            .expect("source signature adapter");
        let StatementKind::Return(Some(Expression {
            kind: ExpressionKind::InterfaceCoerce { value, adapters },
            ty,
            ..
        })) = &adapter.body.statements[0].kind
        else {
            panic!("expected converted adapter return");
        };
        assert!(matches!(value.kind, ExpressionKind::IndirectCall { .. }));
        assert_eq!(*ty, adapter.return_type);
        assert_ne!(*ty, value.ty);
        assert!(adapters.is_empty());
    }

    #[test]
    fn machine_bare_local_annotations_keep_storage_type_and_exact_producer_provenance() {
        let (program, checked) = lower_source_with_check(
            r#"namespace app
machine Session:
    states:
        empty
        content(values: list[int64])
    transitions:
        empty to content
type SessionAlias = Session
function replace() returns Session:
    mutable Session source = Session(empty)
    source = Session(content, list(7))
    return source
function immutable() returns Session:
    Session source = Session(content, list(7))
    return source
function alias() returns Session:
    SessionAlias source = Session(empty)
    return source
function copy(view source: Session at content) returns Session:
    Session erased = clone source
    return erased
function pending() returns Session:
    mutable Session source = run Session(empty)
    source = run run Session(content, list(7))
    return source
function precise() returns Session at empty:
    Session at empty source = Session(empty)
    return source
"#,
            false,
        );
        validate(&program).unwrap();
        validate_backend_types(&program, &checked.interner).unwrap();
        for name in ["replace", "immutable", "alias", "copy", "pending"] {
            let function = program
                .functions
                .iter()
                .find(|f| f.identity.declaration.name == name)
                .unwrap();
            let StatementKind::Let { local, value } = &function.body.statements[0].kind else {
                panic!("{name}: machine binding");
            };
            let local = &function.locals[local.index() as usize];
            let Type::Machine(owner) = checked.interner.resolve(local.ty) else {
                panic!("{name}: declaration must erase initializer state in storage");
            };
            assert_eq!(local.ty, function.return_type);
            assert_eq!(local.debug_ty, local.ty);
            assert_eq!(checked.type_map.get(&value.span), Some(&value.ty));
            let Type::MachineState { machine, .. } = checked.interner.resolve(value.ty) else {
                panic!("{name}: original producer state must remain precise");
            };
            assert_eq!(machine, owner);
            if name == "copy" {
                assert!(matches!(value.kind, ExpressionKind::Clone(_)));
            } else {
                let mut producer = value;
                while let ExpressionKind::Run(inner) = &producer.kind {
                    producer = inner;
                }
                let ExpressionKind::MachineConstruct {
                    state_type, state, ..
                } = &producer.kind
                else {
                    panic!("{name}: exact constructor must remain intact");
                };
                assert_eq!(*state_type, producer.ty);
                let Type::MachineState {
                    state: expected, ..
                } = checked.interner.resolve(*state_type)
                else {
                    panic!("checked constructor state");
                };
                assert_eq!(state.index(), expected.index());
            }
            if matches!(name, "replace" | "pending") {
                let StatementKind::Assign { target, value } = &function.body.statements[1].kind
                else {
                    panic!("{name}: ordinary rebinding");
                };
                assert_eq!(target.ty, local.ty);
                assert_eq!(checked.type_map.get(&value.span), Some(&value.ty));
                assert!(
                    matches!(checked.interner.resolve(value.ty), Type::MachineState { machine, .. } if machine == owner)
                );
            }
        }
        let precise = program
            .functions
            .iter()
            .find(|f| f.identity.declaration.name == "precise")
            .unwrap();
        let StatementKind::Let { local, value } = &precise.body.statements[0].kind else {
            panic!("precise binding");
        };
        assert_eq!(precise.locals[local.index() as usize].ty, value.ty);
        assert!(matches!(
            checked.interner.resolve(value.ty),
            Type::MachineState { .. }
        ));
    }

    #[test]
    fn machine_rebinding_does_not_widen_explicit_or_guarded_state_targets() {
        for body in [
            "    mutable Session at empty source = Session(empty)\n    source = Session(content, list(7))\n    return source\n",
            "    mutable Session source = Session(empty)\n    if source at empty:\n        source = Session(content, list(7))\n    return source\n",
        ] {
            let source = format!(
                "namespace app\nmachine Session:\n    states:\n        empty\n        content(values: list[int64])\n    transitions:\n        empty to content\nfunction invalid() returns Session:\n{body}"
            );
            let parsed = jett_parser::parse(&source, FileId::new(0));
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            let resolved = jett_resolve::resolve(&parsed.module);
            let checked = jett_typecheck::check(&parsed.module, &resolved);
            assert!(
                checked
                    .diagnostics
                    .iter()
                    .any(|d| d.severity == jett_diagnostics::Severity::Error),
                "precise assignment must remain rejected: {:?}",
                checked.diagnostics
            );
        }
    }
}
