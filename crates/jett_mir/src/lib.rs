//! Jett's backend-neutral control-flow graph representation.

mod analysis;
pub mod copy_values;
mod handlers;
mod sequences;
pub use sequences::prepare_native_sequences;
pub mod move_values;

pub use analysis::{AnalysisError, ControlFlowGraph};
pub use jett_hir::{FunctionId, Local, LocalId, Param, ParamMode};

use jett_common::Span;
use jett_hir::{self as hir, Expression, FunctionIdentity, VariantId};
use jett_types::TypeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(u32);

impl BlockId {
    pub fn index(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub id: FunctionId,
    pub identity: FunctionIdentity,
    pub params: Vec<Param>,
    pub return_type: TypeId,
    pub locals: Vec<Local>,
    pub entry: BlockId,
    pub blocks: Vec<BasicBlock>,
    pub span: Span,
}

impl Function {
    /// Look up a local through the canonical dense local table.
    pub fn local(&self, id: LocalId) -> Option<&Local> {
        self.locals
            .get(id.index() as usize)
            .filter(|local| local.id == id)
    }

    /// Look up the parameter metadata associated with a local, if any.
    pub fn parameter_for_local(&self, id: LocalId) -> Option<&Param> {
        self.params.iter().find(|param| param.local == id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BasicBlock {
    pub id: BlockId,
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatementKind {
    SequenceLength {
        source: LocalId,
        target: LocalId,
    },
    SequenceGet {
        source: LocalId,
        index: LocalId,
        target: LocalId,
    },
    IterationBorrow {
        source: LocalId,
        token: LocalId,
        start: bool,
    },
    /// Read a sum discriminant without consuming its owner.
    SumTag {
        source: LocalId,
        target: LocalId,
    },
    /// Consume the sum and transfer only its selected initialized payload.
    SumTake {
        source: LocalId,
        target: LocalId,
        success: bool,
    },
    Let {
        local: LocalId,
        value: Expression,
    },
    Assign {
        target: Expression,
        value: Expression,
    },
    Evaluate(Expression),
    HandleDefault(Expression),
    Assert {
        condition: Expression,
        message: Option<Expression>,
    },
    Trace(LocalId),
    Breakpoint(Option<Expression>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Terminator {
    pub kind: TerminatorKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectedTypeDispatchArm {
    /// Canonical position exported by the checker for stable diagnostics.
    pub iteration_index: usize,
    /// Canonical concrete type identity matched against the runtime `TypeInfo`.
    pub bound_type: TypeId,
    pub target: BlockId,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TerminatorKind {
    Return(Option<Expression>),
    Respond(Expression),
    Goto(BlockId),
    Branch {
        condition: Expression,
        then_block: BlockId,
        else_block: BlockId,
    },
    Switch {
        scrutinee: Expression,
        variants: Vec<(VariantId, BlockId, Vec<LocalId>)>,
        otherwise: Option<BlockId>,
    },
    ForEach {
        key: LocalId,
        value: Option<LocalId>,
        by_view: bool,
        iterable: Expression,
        body: BlockId,
        exit: BlockId,
    },
    /// Select exactly one checker-specialized arm by canonical reflected type
    /// identity. `otherwise` is a defensive edge for malformed runtime
    /// `TypeInfo` values and normally leads to `Unreachable`.
    ReflectedTypeDispatch {
        type_info: Expression,
        arms: Vec<ReflectedTypeDispatchArm>,
        otherwise: BlockId,
    },
    Unreachable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LowerError {
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub span: Span,
    pub message: String,
}

/// Validate structural MIR invariants before a backend consumes the program.
pub fn validate(program: &Program) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    let mut function_ids = std::collections::HashSet::new();
    for (index, function) in program.functions.iter().enumerate() {
        if function.id.index() as usize != index {
            errors.push(ValidationError {
                span: function.span,
                message: format!(
                    "function at index {index} has noncanonical ID {}",
                    function.id.index()
                ),
            });
        }
        if !function_ids.insert(function.id) {
            errors.push(ValidationError {
                span: function.span,
                message: format!("multiple functions use ID {}", function.id.index()),
            });
        }
        FunctionValidator {
            function,
            function_count: program.functions.len(),
            errors: &mut errors,
        }
        .validate();
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

struct FunctionValidator<'function, 'errors> {
    function: &'function Function,
    function_count: usize,
    errors: &'errors mut Vec<ValidationError>,
}

impl FunctionValidator<'_, '_> {
    fn validate(&mut self) {
        let function = self.function;
        if function.entry.index() as usize >= function.blocks.len() {
            self.error(
                function.span,
                format!(
                    "function entry block {} is out of range",
                    function.entry.index()
                ),
            );
        }

        for (index, local) in function.locals.iter().enumerate() {
            if local.id.index() as usize != index {
                self.error(
                    local.span,
                    format!(
                        "local at index {index} has noncanonical ID {}",
                        local.id.index()
                    ),
                );
            }
        }

        let mut parameter_locals = std::collections::HashSet::new();
        for param in &function.params {
            self.check_local(param.local, param.span, "parameter");
            if !parameter_locals.insert(param.local) {
                self.error(
                    param.span,
                    format!(
                        "multiple parameters reference local {}",
                        param.local.index()
                    ),
                );
            }
            if let Some(local) = function.local(param.local) {
                let metadata_matches = local.name == param.name
                    && local.ty == param.ty
                    && local.mutable == param.mutable
                    && local.span == param.span;
                if !metadata_matches {
                    self.error(
                        param.span,
                        format!(
                            "parameter metadata does not match local {}",
                            param.local.index()
                        ),
                    );
                }
            }
        }

        for (index, block) in function.blocks.iter().enumerate() {
            if block.id.index() as usize != index {
                self.error(
                    function.span,
                    format!(
                        "block at index {index} has noncanonical ID {}",
                        block.id.index()
                    ),
                );
            }
            for statement in &block.statements {
                self.statement(statement);
            }
            self.terminator(&block.terminator);
        }
    }

    fn error(&mut self, span: Span, message: impl Into<String>) {
        self.errors.push(ValidationError {
            span,
            message: message.into(),
        });
    }

    fn check_local(&mut self, local: LocalId, span: Span, context: &str) {
        let local_count = self.function.locals.len();
        if local.index() as usize >= local_count {
            self.error(
                span,
                format!(
                    "{context} references local {} outside function local table of length {local_count}",
                    local.index()
                ),
            );
        }
    }

    fn check_target(&mut self, target: BlockId, edge: &str) {
        if target.index() as usize >= self.function.blocks.len() {
            self.error(
                self.function.span,
                format!("{edge} target {} is out of range", target.index()),
            );
        }
    }

    fn check_function(&mut self, function: FunctionId, span: Span) {
        if function.index() as usize >= self.function_count {
            self.error(
                span,
                format!(
                    "direct call references function {} outside function table of length {}",
                    function.index(),
                    self.function_count
                ),
            );
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
                "evaluation order must be a permutation of the operand indexes",
            );
        }
    }

    fn statement(&mut self, statement: &Statement) {
        match &statement.kind {
            StatementKind::SequenceLength { source, target } => {
                self.check_local(*source, statement.span, "sequence source");
                self.check_local(*target, statement.span, "sequence length");
            }
            StatementKind::SequenceGet {
                source,
                index,
                target,
            } => {
                self.check_local(*source, statement.span, "sequence source");
                self.check_local(*index, statement.span, "sequence index");
                self.check_local(*target, statement.span, "sequence element");
            }
            StatementKind::IterationBorrow { source, token, .. } => {
                self.check_local(*source, statement.span, "iteration borrow");
                self.check_local(*token, statement.span, "iteration loan token");
            }

            StatementKind::SumTag { source, target }
            | StatementKind::SumTake { source, target, .. } => {
                self.check_local(*source, statement.span, "sum source");
                self.check_local(*target, statement.span, "sum target");
            }

            StatementKind::Let { local, value } => {
                self.check_local(*local, statement.span, "let statement");
                self.expression(value);
            }
            StatementKind::Assign { target, value } => {
                self.expression(target);
                self.expression(value);
            }
            StatementKind::Evaluate(value) | StatementKind::HandleDefault(value) => {
                self.expression(value);
            }
            StatementKind::Assert { condition, message } => {
                self.expression(condition);
                if let Some(message) = message {
                    self.expression(message);
                }
            }
            StatementKind::Trace(local) => {
                self.check_local(*local, statement.span, "trace statement");
            }
            StatementKind::Breakpoint(condition) => {
                if let Some(condition) = condition {
                    self.expression(condition);
                }
            }
        }
    }

    fn terminator(&mut self, terminator: &Terminator) {
        match &terminator.kind {
            TerminatorKind::Return(value) => {
                if let Some(value) = value {
                    self.expression(value);
                }
            }
            TerminatorKind::Respond(value) => self.expression(value),
            TerminatorKind::Goto(target) => self.check_target(*target, "goto"),
            TerminatorKind::Branch {
                condition,
                then_block,
                else_block,
            } => {
                self.expression(condition);
                self.check_target(*then_block, "branch then");
                self.check_target(*else_block, "branch else");
            }
            TerminatorKind::Switch {
                scrutinee,
                variants,
                otherwise,
            } => {
                self.expression(scrutinee);
                for (variant, target, bindings) in variants {
                    self.check_target(*target, &format!("switch variant {}", variant.index()));
                    for binding in bindings {
                        self.check_local(*binding, terminator.span, "switch binding");
                    }
                }
                if let Some(target) = otherwise {
                    self.check_target(*target, "switch otherwise");
                }
            }
            TerminatorKind::ForEach {
                key,
                value,
                iterable,
                body,
                exit,
                ..
            } => {
                self.check_local(*key, terminator.span, "for key");
                if let Some(value) = value {
                    self.check_local(*value, terminator.span, "for value");
                }
                self.expression(iterable);
                self.check_target(*body, "for body");
                self.check_target(*exit, "for exit");
            }
            TerminatorKind::ReflectedTypeDispatch {
                type_info,
                arms,
                otherwise,
            } => {
                self.expression(type_info);
                if arms.is_empty() {
                    self.error(terminator.span, "reflected type dispatch has no arms");
                }
                let mut iteration_indexes = std::collections::HashSet::new();
                let mut bound_types = std::collections::HashSet::new();
                let mut targets = std::collections::HashSet::new();
                for arm in arms {
                    if !iteration_indexes.insert(arm.iteration_index) {
                        self.error(
                            terminator.span,
                            "reflected type dispatch contains a duplicate iteration index",
                        );
                    }
                    if !bound_types.insert(arm.bound_type) {
                        self.error(
                            terminator.span,
                            "reflected type dispatch contains a duplicate bound type",
                        );
                    }
                    targets.insert(arm.target);
                    self.check_target(arm.target, "reflected type dispatch arm");
                }
                if targets.contains(otherwise) {
                    self.error(
                        terminator.span,
                        "reflected type dispatch otherwise target overlaps an arm target",
                    );
                }
                self.check_target(*otherwise, "reflected type dispatch otherwise");
            }
            TerminatorKind::Unreachable => {}
        }
    }

    fn hir_block(&mut self, block: &hir::Block) {
        for statement in &block.statements {
            self.hir_statement(statement);
        }
    }

    fn hir_statement(&mut self, statement: &hir::Statement) {
        match &statement.kind {
            hir::StatementKind::Let { local, value } => {
                self.check_local(*local, statement.span, "nested let statement");
                self.expression(value);
            }
            hir::StatementKind::Assign { target, value } => {
                self.expression(target);
                self.expression(value);
            }
            hir::StatementKind::Return(value) => {
                if let Some(value) = value {
                    self.expression(value);
                }
            }
            hir::StatementKind::HandleDefault(value)
            | hir::StatementKind::Expression(value)
            | hir::StatementKind::Respond(value) => self.expression(value),
            hir::StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                self.expression(condition);
                self.hir_block(then_block);
                if let Some(else_block) = else_block {
                    self.hir_block(else_block);
                }
            }
            hir::StatementKind::While { condition, body } => {
                self.expression(condition);
                self.hir_block(body);
            }
            hir::StatementKind::For {
                key,
                value,
                iterable,
                body,
                ..
            } => {
                self.check_local(*key, statement.span, "nested for key");
                if let Some(value) = value {
                    self.check_local(*value, statement.span, "nested for value");
                }
                self.expression(iterable);
                self.hir_block(body);
            }
            hir::StatementKind::Match { scrutinee, arms } => {
                self.expression(scrutinee);
                for arm in arms {
                    for binding in &arm.bindings {
                        self.check_local(*binding, arm.span, "nested match binding");
                    }
                    self.hir_block(&arm.body);
                }
            }
            hir::StatementKind::Break | hir::StatementKind::Continue => {}
            hir::StatementKind::Assert { condition, message } => {
                self.expression(condition);
                if let Some(message) = message {
                    self.expression(message);
                }
            }
            hir::StatementKind::Trace(local) => {
                self.check_local(*local, statement.span, "nested trace statement");
            }
            hir::StatementKind::Breakpoint(condition) => {
                if let Some(condition) = condition {
                    self.expression(condition);
                }
            }
            hir::StatementKind::Scope(block) => self.hir_block(block),
            hir::StatementKind::ReflectedTypeDispatch { type_info, arms } => {
                self.expression(type_info);
                let mut iteration_indexes = std::collections::HashSet::new();
                let mut bound_types = std::collections::HashSet::new();
                for arm in arms {
                    if !iteration_indexes.insert(arm.iteration_index) {
                        self.error(
                            statement.span,
                            "nested reflected type dispatch contains a duplicate iteration index",
                        );
                    }
                    if !bound_types.insert(arm.bound_type) {
                        self.error(
                            statement.span,
                            "nested reflected type dispatch contains a duplicate bound type",
                        );
                    }
                    self.hir_block(&arm.body);
                }
                if arms.is_empty() {
                    self.error(statement.span, "nested reflected type dispatch has no arms");
                }
            }
        }
    }

    fn expression(&mut self, expression: &Expression) {
        match &expression.kind {
            hir::ExpressionKind::Local(local) => {
                self.check_local(*local, expression.span, "expression");
            }
            hir::ExpressionKind::FunctionRef(function) => {
                self.check_function(*function, expression.span);
            }
            hir::ExpressionKind::Binary { left, right, .. } => {
                self.expression(left);
                self.expression(right);
            }
            hir::ExpressionKind::Unary { value, .. }
            | hir::ExpressionKind::ResultOk(value)
            | hir::ExpressionKind::ResultFail(value)
            | hir::ExpressionKind::OptionalSome(value)
            | hir::ExpressionKind::Comptime(value)
            | hir::ExpressionKind::Declassify(value)
            | hir::ExpressionKind::Coarsen(value)
            | hir::ExpressionKind::Run(value)
            | hir::ExpressionKind::Join(value)
            | hir::ExpressionKind::Cancel(value)
            | hir::ExpressionKind::View(value)
            | hir::ExpressionKind::Clone(value) => self.expression(value),
            hir::ExpressionKind::Call {
                function,
                args,
                evaluation_order,
            } => {
                self.check_function(*function, expression.span);
                self.check_evaluation_order(evaluation_order, args.len(), expression.span);
                for argument in args {
                    self.expression(argument);
                }
            }
            hir::ExpressionKind::Intrinsic {
                args,
                evaluation_order,
                ..
            }
            | hir::ExpressionKind::ActorSpawn {
                args,
                evaluation_order,
                ..
            } => {
                self.check_evaluation_order(evaluation_order, args.len(), expression.span);
                for argument in args {
                    self.expression(argument);
                }
            }
            hir::ExpressionKind::IndirectCall {
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
            hir::ExpressionKind::StructConstruct {
                fields,
                evaluation_order,
                ..
            }
            | hir::ExpressionKind::BitfieldConstruct {
                fields,
                evaluation_order,
                ..
            } => {
                self.check_evaluation_order(evaluation_order, fields.len(), expression.span);
                for field in fields {
                    self.expression(field);
                }
            }
            hir::ExpressionKind::MachineConstruct { payloads, .. }
            | hir::ExpressionKind::EnumConstruct { payloads, .. } => {
                for payload in payloads {
                    self.expression(payload);
                }
            }
            hir::ExpressionKind::MachineTransition {
                source, payloads, ..
            } => {
                self.expression(source);
                for payload in payloads {
                    self.expression(payload);
                }
            }
            hir::ExpressionKind::ListConstruct { elements } => {
                for element in elements {
                    self.expression(element);
                }
            }
            hir::ExpressionKind::MapConstruct { entries } => {
                for entry in entries {
                    self.expression(&entry.key);
                    self.expression(&entry.value);
                }
            }
            hir::ExpressionKind::Handle {
                target,
                error_local,
                failure,
                ..
            } => {
                self.expression(target);
                if let Some(error_local) = error_local {
                    self.check_local(*error_local, expression.span, "handle error binding");
                }
                self.hir_block(failure);
            }
            hir::ExpressionKind::StringInterpolation(parts) => {
                for part in parts {
                    if let hir::StringSegment::Value(value) = part {
                        self.expression(value);
                    }
                }
            }
            hir::ExpressionKind::StateIs { value, .. } => self.expression(value),
            hir::ExpressionKind::InlineFunction { params, body } => {
                for param in params {
                    self.check_local(*param, expression.span, "inline function parameter");
                }
                self.hir_block(body);
            }
            hir::ExpressionKind::ActorMessage {
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
            hir::ExpressionKind::Field { base, .. } => self.expression(base),
            hir::ExpressionKind::Int(_)
            | hir::ExpressionKind::Float(_)
            | hir::ExpressionKind::String(_)
            | hir::ExpressionKind::Bool(_)
            | hir::ExpressionKind::Nothing
            | hir::ExpressionKind::OptionalNone => {}
        }
    }
}

pub fn lower(program: &hir::Program) -> Result<Program, Vec<LowerError>> {
    if let Err(errors) = hir::validate(program) {
        return Err(errors
            .into_iter()
            .map(|error| LowerError {
                span: error.span,
                message: error.message,
            })
            .collect());
    }
    Ok(Program {
        functions: program.functions.iter().map(lower_function).collect(),
    })
}

fn lower_function(function: &hir::Function) -> Function {
    let mut builder = Builder::new(function.body.span);
    builder.locals = function.locals.clone();
    builder.lower_block(&function.body);
    if builder.open() && function.return_type == jett_types::TypeInterner::NOTHING {
        builder.terminate(TerminatorKind::Return(None), function.body.span);
    }
    Function {
        id: function.id,
        identity: function.identity.clone(),
        params: function.params.clone(),
        return_type: function.return_type,
        locals: builder.locals,
        entry: BlockId(0),
        blocks: builder.blocks,
        span: function.span,
    }
}

struct Builder {
    blocks: Vec<BasicBlock>,
    current: BlockId,
    loops: Vec<(BlockId, BlockId)>,
    locals: Vec<Local>,
    handlers: Vec<(LocalId, BlockId)>,
}

impl Builder {
    fn new(span: Span) -> Self {
        Self {
            blocks: vec![BasicBlock {
                id: BlockId(0),
                statements: Vec::new(),
                terminator: Terminator {
                    kind: TerminatorKind::Unreachable,
                    span,
                },
            }],
            current: BlockId(0),
            loops: Vec::new(),
            locals: Vec::new(),
            handlers: Vec::new(),
        }
    }

    fn new_block(&mut self, span: Span) -> BlockId {
        let id = BlockId(self.blocks.len() as u32);
        self.blocks.push(BasicBlock {
            id,
            statements: Vec::new(),
            terminator: Terminator {
                kind: TerminatorKind::Unreachable,
                span,
            },
        });
        id
    }

    fn open(&self) -> bool {
        matches!(
            self.blocks[self.current.index() as usize].terminator.kind,
            TerminatorKind::Unreachable
        )
    }

    fn terminate(&mut self, kind: TerminatorKind, span: Span) {
        self.blocks[self.current.index() as usize].terminator = Terminator { kind, span };
    }

    fn push(&mut self, kind: StatementKind, span: Span) {
        self.blocks[self.current.index() as usize]
            .statements
            .push(Statement { kind, span });
    }

    fn close_to(&mut self, target: BlockId, span: Span) {
        if self.open() {
            self.terminate(TerminatorKind::Goto(target), span);
        }
    }

    fn lower_block(&mut self, block: &hir::Block) {
        for statement in &block.statements {
            if !self.open() {
                break;
            }
            self.lower_statement(statement);
        }
    }

    fn lower_statement(&mut self, statement: &hir::Statement) {
        match &statement.kind {
            hir::StatementKind::Let { local, value } => {
                let value = self.lower_value(value);
                self.push(
                    StatementKind::Let {
                        local: *local,
                        value,
                    },
                    statement.span,
                );
            }
            hir::StatementKind::Assign { target, value } => {
                let value = self.lower_value(value);
                self.push(
                    StatementKind::Assign {
                        target: target.clone(),
                        value,
                    },
                    statement.span,
                );
            }
            hir::StatementKind::Expression(value) => {
                let value = self.lower_value(value);
                self.push(StatementKind::Evaluate(value), statement.span);
            }
            hir::StatementKind::HandleDefault(value) => {
                let value = self.lower_value(value);
                if let Some(&(local, continuation)) = self.handlers.last() {
                    self.push(StatementKind::Let { local, value }, statement.span);
                    self.terminate(TerminatorKind::Goto(continuation), statement.span);
                } else {
                    self.push(StatementKind::HandleDefault(value), statement.span);
                }
            }
            hir::StatementKind::Return(value) => {
                let value = value.as_ref().map(|v| self.lower_value(v));
                self.terminate(TerminatorKind::Return(value), statement.span);
            }
            hir::StatementKind::Break => {
                let target = self.loops.last().expect("validated break has a loop").1;
                self.terminate(TerminatorKind::Goto(target), statement.span);
            }
            hir::StatementKind::Continue => {
                let target = self.loops.last().expect("validated continue has a loop").0;
                self.terminate(TerminatorKind::Goto(target), statement.span);
            }
            hir::StatementKind::If {
                condition,
                then_block,
                else_block,
            } => self.lower_if(condition, then_block, else_block.as_ref(), statement.span),
            hir::StatementKind::While { condition, body } => {
                self.lower_while(condition, body, statement.span)
            }
            hir::StatementKind::For {
                key,
                value,
                by_view,
                iterable,
                body,
            } => self.lower_for(*key, *value, *by_view, iterable, body, statement.span),
            hir::StatementKind::Match { scrutinee, arms } => {
                self.lower_match(scrutinee, arms, statement.span)
            }
            hir::StatementKind::Assert { condition, message } => self.push(
                StatementKind::Assert {
                    condition: condition.clone(),
                    message: message.clone(),
                },
                statement.span,
            ),
            hir::StatementKind::Trace(local) => {
                self.push(StatementKind::Trace(*local), statement.span)
            }
            hir::StatementKind::Breakpoint(condition) => {
                self.push(StatementKind::Breakpoint(condition.clone()), statement.span)
            }
            hir::StatementKind::Respond(value) => {
                self.terminate(TerminatorKind::Respond(value.clone()), statement.span)
            }
            hir::StatementKind::Scope(block) => self.lower_block(block),
            hir::StatementKind::ReflectedTypeDispatch { type_info, arms } => {
                self.lower_reflected_type_dispatch(type_info, arms, statement.span)
            }
        }
    }

    fn lower_if(
        &mut self,
        condition: &Expression,
        then_body: &hir::Block,
        else_body: Option<&hir::Block>,
        statement_span: Span,
    ) {
        let then_block = self.new_block(then_body.span);
        let else_span = else_body.map_or(statement_span, |body| body.span);
        let else_block = self.new_block(else_span);
        self.terminate(
            TerminatorKind::Branch {
                condition: condition.clone(),
                then_block,
                else_block,
            },
            condition.span,
        );
        self.current = then_block;
        self.lower_block(then_body);
        let then_exit = self.current;
        let then_falls_through = self.open();
        self.current = else_block;
        if let Some(body) = else_body {
            self.lower_block(body);
        }
        let else_exit = self.current;
        let else_falls_through = self.open();

        if then_falls_through || else_falls_through {
            let join = self.new_block(statement_span);
            self.current = then_exit;
            self.close_to(join, then_body.span);
            self.current = else_exit;
            self.close_to(join, else_span);
            self.current = join;
        }
    }

    fn lower_while(&mut self, condition: &Expression, body: &hir::Block, statement_span: Span) {
        let condition_block = self.new_block(condition.span);
        let body_block = self.new_block(body.span);
        let exit = self.new_block(statement_span);
        self.terminate(TerminatorKind::Goto(condition_block), statement_span);
        self.current = condition_block;
        self.terminate(
            TerminatorKind::Branch {
                condition: condition.clone(),
                then_block: body_block,
                else_block: exit,
            },
            condition.span,
        );
        self.loops.push((condition_block, exit));
        self.current = body_block;
        self.lower_block(body);
        self.close_to(condition_block, body.span);
        self.loops.pop();
        self.current = exit;
    }

    fn lower_for(
        &mut self,
        key: LocalId,
        value: Option<LocalId>,
        by_view: bool,
        iterable: &Expression,
        body: &hir::Block,
        statement_span: Span,
    ) {
        let header = self.new_block(iterable.span);
        let body_block = self.new_block(body.span);
        let exit = self.new_block(statement_span);
        self.terminate(TerminatorKind::Goto(header), statement_span);
        self.current = header;
        self.terminate(
            TerminatorKind::ForEach {
                key,
                value,
                by_view,
                iterable: iterable.clone(),
                body: body_block,
                exit,
            },
            iterable.span,
        );
        self.loops.push((header, exit));
        self.current = body_block;
        self.lower_block(body);
        self.close_to(header, body.span);
        self.loops.pop();
        self.current = exit;
    }

    fn lower_match(
        &mut self,
        scrutinee: &Expression,
        arms: &[hir::MatchArm],
        statement_span: Span,
    ) {
        let arm_blocks = arms
            .iter()
            .map(|arm| self.new_block(arm.span))
            .collect::<Vec<_>>();
        let mut variants = Vec::new();
        let mut otherwise = None;
        for (arm, block) in arms.iter().zip(&arm_blocks) {
            if let Some(variant) = arm.variant {
                variants.push((variant, *block, arm.bindings.clone()));
            } else {
                otherwise = Some(*block);
            }
        }
        self.terminate(
            TerminatorKind::Switch {
                scrutinee: scrutinee.clone(),
                variants,
                otherwise,
            },
            scrutinee.span,
        );

        let mut arm_exits = Vec::with_capacity(arms.len());
        for (arm, block) in arms.iter().zip(arm_blocks) {
            self.current = block;
            self.lower_block(&arm.body);
            arm_exits.push((self.current, self.open(), arm.span));
        }

        if arm_exits.iter().any(|(_, falls_through, _)| *falls_through) {
            let join = self.new_block(statement_span);
            for (exit, _, span) in arm_exits {
                self.current = exit;
                self.close_to(join, span);
            }
            self.current = join;
        }
    }

    fn lower_reflected_type_dispatch(
        &mut self,
        type_info: &Expression,
        arms: &[hir::ReflectedTypeArm],
        statement_span: Span,
    ) {
        let arm_blocks = arms
            .iter()
            .map(|arm| self.new_block(arm.body.span))
            .collect::<Vec<_>>();
        let otherwise = self.new_block(statement_span);
        let dispatch_arms = arms
            .iter()
            .zip(&arm_blocks)
            .map(|(arm, target)| ReflectedTypeDispatchArm {
                iteration_index: arm.iteration_index,
                bound_type: arm.bound_type,
                target: *target,
            })
            .collect();
        self.terminate(
            TerminatorKind::ReflectedTypeDispatch {
                type_info: type_info.clone(),
                arms: dispatch_arms,
                otherwise,
            },
            type_info.span,
        );

        let mut arm_exits = Vec::with_capacity(arms.len());
        for (arm, block) in arms.iter().zip(arm_blocks) {
            self.current = block;
            self.lower_block(&arm.body);
            arm_exits.push((self.current, self.open(), arm.body.span));
        }

        if arm_exits.iter().any(|(_, falls_through, _)| *falls_through) {
            let join = self.new_block(statement_span);
            for (exit, _, span) in arm_exits {
                self.current = exit;
                self.close_to(join, span);
            }
            self.current = join;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use jett_common::{FileId, SourceOrigin};

    use super::*;

    const REFLECTED_DISPATCH_SOURCE: &str = r#"namespace app
struct User:
    name: string
    age: int64
function reflected_names[T](view value: T) returns string:
    mutable string output = ""
    for field in type.fields[T]():
        comptime type Field = field.type_info:
            output = type.name[Field]()
    return output
function main() returns string:
    User user = User(name: "Ada", age: 37)
    return reflected_names[User](view user)
"#;

    const REFLECTED_CONTROL_FLOW_SOURCE: &str = r#"namespace app
struct User:
    name: string
    age: int64
function break_after_first[T]() returns string:
    for field in type.fields[T]():
        comptime type Field = field.type_info:
            break
    return "break"
function continue_all[T]() returns string:
    for field in type.fields[T]():
        comptime type Field = field.type_info:
            continue
    return "continue"
function return_first[T]() returns string:
    for field in type.fields[T]():
        comptime type Field = field.type_info:
            return type.name[Field]()
    return "empty"
function main() returns string:
    string broken = break_after_first[User]()
    string continued = continue_all[User]()
    return return_first[User]()
"#;

    fn lower_source(source: &str) -> Program {
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
                .all(|diagnostic| { diagnostic.severity != jett_diagnostics::Severity::Error }),
            "check errors: {:?}",
            checked.diagnostics
        );
        let hir = jett_hir::lower(
            &parsed.module,
            &resolved,
            &checked,
            &HashMap::from([(file, SourceOrigin::Project)]),
        )
        .expect("HIR lowering");
        lower(&hir).expect("MIR lowering")
    }

    fn reachable_blocks(function: &Function) -> HashSet<BlockId> {
        let mut reachable = HashSet::new();
        let mut pending = vec![function.entry];
        while let Some(block_id) = pending.pop() {
            if !reachable.insert(block_id) {
                continue;
            }
            let block = &function.blocks[block_id.index() as usize];
            match &block.terminator.kind {
                TerminatorKind::Goto(target) => pending.push(*target),
                TerminatorKind::Branch {
                    then_block,
                    else_block,
                    ..
                } => {
                    pending.push(*then_block);
                    pending.push(*else_block);
                }
                TerminatorKind::Switch {
                    variants,
                    otherwise,
                    ..
                } => {
                    pending.extend(variants.iter().map(|(_, target, _)| *target));
                    pending.extend(otherwise.iter().copied());
                }
                TerminatorKind::ForEach { body, exit, .. } => {
                    pending.push(*body);
                    pending.push(*exit);
                }
                TerminatorKind::ReflectedTypeDispatch {
                    arms, otherwise, ..
                } => {
                    pending.extend(arms.iter().map(|arm| arm.target));
                    pending.push(*otherwise);
                }
                TerminatorKind::Return(_)
                | TerminatorKind::Respond(_)
                | TerminatorKind::Unreachable => {}
            }
        }
        reachable
    }

    fn returned_expression(function: &mut Function) -> &mut Expression {
        function
            .blocks
            .iter_mut()
            .find_map(|block| match &mut block.terminator.kind {
                TerminatorKind::Return(Some(value)) => Some(value),
                _ => None,
            })
            .expect("expected a returned expression")
    }

    #[test]
    fn preserves_function_ids_parameters_locals_and_call_order() {
        let signature = lower_source(
            r#"namespace app
function combine(view left: int64, right: int64) returns int64:
    int64 total = left + right
    return total
"#,
        );
        let function = &signature.functions[0];

        assert_eq!(function.id.index(), 0);
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.locals.len(), 3);
        assert_eq!(function.params[0].mode, ParamMode::View);
        assert_eq!(function.params[1].mode, ParamMode::Owned);
        for parameter in &function.params {
            assert_eq!(
                function.parameter_for_local(parameter.local),
                Some(parameter)
            );
            let local = function.local(parameter.local).expect("parameter local");
            assert_eq!(local.name, parameter.name);
            assert_eq!(local.ty, parameter.ty);
            assert_eq!(local.mutable, parameter.mutable);
            assert_eq!(local.span, parameter.span);
        }
        validate(&signature).expect("preserved signature must validate");

        let mut calls = lower_source(
            r#"namespace app
function add(first: int64, second: int64) returns int64:
    return first + second
function call_add() returns int64:
    return add(second: 2, first: 1)
"#,
        );
        let callee_id = calls.functions[0].id;
        let value = returned_expression(&mut calls.functions[1]);
        let hir::ExpressionKind::Call {
            function,
            evaluation_order,
            ..
        } = &value.kind
        else {
            panic!("expected direct call");
        };
        assert_eq!(*function, callee_id);
        assert_eq!(evaluation_order, &[1, 0]);
        validate(&calls).expect("valid call order must validate");
    }

    #[test]
    fn validation_rejects_noncanonical_and_duplicate_function_ids() {
        let mut program = lower_source(
            r#"namespace app
function first() returns int64:
    return 1
function second() returns int64:
    return 2
"#,
        );
        program.functions[1].id = program.functions[0].id;

        let errors = validate(&program).expect_err("duplicate function ID must be rejected");
        let messages = errors
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>();
        assert!(messages.contains(&"function at index 1 has noncanonical ID 0"));
        assert!(messages.contains(&"multiple functions use ID 0"));
    }

    #[test]
    fn validation_rejects_a_direct_call_outside_the_function_table() {
        let mut program = lower_source(
            r#"namespace app
function callee() returns int64:
    return 1
function caller() returns int64:
    return callee()
function spare() returns int64:
    return 2
"#,
        );
        let removed_id = program.functions[2].id;
        let value = returned_expression(&mut program.functions[1]);
        let hir::ExpressionKind::Call { function, .. } = &mut value.kind else {
            panic!("expected direct call");
        };
        *function = removed_id;
        program.functions.pop();

        let errors = validate(&program).expect_err("unknown direct call must be rejected");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "direct call references function 2 outside function table of length 2"
        );
    }

    #[test]
    fn validation_rejects_a_non_permutation_evaluation_order() {
        let mut program = lower_source(
            r#"namespace app
function add(first: int64, second: int64) returns int64:
    return first + second
function caller() returns int64:
    return add(1, 2)
"#,
        );
        let value = returned_expression(&mut program.functions[1]);
        let hir::ExpressionKind::Call {
            evaluation_order, ..
        } = &mut value.kind
        else {
            panic!("expected direct call");
        };
        *evaluation_order = vec![0, 0];

        let errors = validate(&program).expect_err("invalid order must be rejected");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "evaluation order must be a permutation of the operand indexes"
        );
    }

    #[test]
    fn validation_rejects_duplicate_and_mismatched_parameter_metadata() {
        let program = lower_source(
            r#"namespace app
function first(left: int64, right: int64) returns int64:
    return left
"#,
        );

        let mut duplicate = program.clone();
        duplicate.functions[0].params[1].local = duplicate.functions[0].params[0].local;
        let errors = validate(&duplicate).expect_err("duplicate parameter local must be rejected");
        let messages = errors
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>();
        assert!(messages.contains(&"multiple parameters reference local 0"));
        assert!(messages.contains(&"parameter metadata does not match local 0"));

        let mut mismatch = program;
        mismatch.functions[0].params[0].name = "renamed".to_string();
        let errors = validate(&mismatch).expect_err("mismatched metadata must be rejected");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "parameter metadata does not match local 0"
        );
    }

    #[test]
    fn validation_rejects_noncanonical_and_missing_parameter_locals() {
        let program = lower_source(
            r#"namespace app
function first(left: int64, right: int64) returns int64:
    return left
"#,
        );

        let mut noncanonical = program.clone();
        noncanonical.functions[0].locals[1].id = noncanonical.functions[0].locals[0].id;
        let errors = validate(&noncanonical).expect_err("noncanonical local ID must be rejected");
        assert!(
            errors
                .iter()
                .any(|error| error.message == "local at index 1 has noncanonical ID 0")
        );

        let mut missing = program;
        missing.functions[0].locals.pop();
        let errors = validate(&missing).expect_err("missing parameter local must be rejected");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "parameter references local 1 outside function local table of length 1"
        );
    }

    #[test]
    fn lowers_reflected_type_dispatch_to_distinct_arm_blocks() {
        let program = lower_source(REFLECTED_DISPATCH_SOURCE);
        let function = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "reflected_names")
            .expect("concrete reflected function should lower");
        let (dispatch_block, type_info, arms, otherwise) = function
            .blocks
            .iter()
            .find_map(|block| match &block.terminator.kind {
                TerminatorKind::ReflectedTypeDispatch {
                    type_info,
                    arms,
                    otherwise,
                } => Some((block.id, type_info, arms, *otherwise)),
                _ => None,
            })
            .expect("expected reflected type dispatch terminator");

        assert!(matches!(type_info.kind, hir::ExpressionKind::Field { .. }));
        assert_eq!(arms.len(), 2);
        assert_eq!(
            arms.iter()
                .map(|arm| arm.iteration_index)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        assert_ne!(arms[0].bound_type, arms[1].bound_type);
        assert_ne!(arms[0].target, arms[1].target);
        assert!(arms.iter().all(|arm| arm.target != otherwise));
        assert!(matches!(
            function.blocks[otherwise.index() as usize].terminator.kind,
            TerminatorKind::Unreachable
        ));

        let mut join = None;
        for arm in arms {
            let block = &function.blocks[arm.target.index() as usize];
            let [
                Statement {
                    kind: StatementKind::Assign { value, .. },
                    ..
                },
            ] = block.statements.as_slice()
            else {
                panic!("each reflected arm should contain its specialized assignment");
            };
            let hir::ExpressionKind::Intrinsic { type_arguments, .. } = &value.kind else {
                panic!("expected specialized reflection intrinsic");
            };
            assert_eq!(type_arguments, &[arm.bound_type]);
            let TerminatorKind::Goto(target) = block.terminator.kind else {
                panic!("fallthrough arm should jump to the shared join");
            };
            assert_eq!(*join.get_or_insert(target), target);
        }

        validate(&program).expect("lowered reflected dispatch must validate");
        let cfg = ControlFlowGraph::analyze(function).expect("reflected dispatch CFG");
        assert_eq!(
            cfg.successors(dispatch_block),
            &[arms[0].target, arms[1].target, otherwise]
        );
    }

    #[test]
    fn validation_rejects_invalid_reflected_dispatch_selector_and_targets() {
        let mut program = lower_source(REFLECTED_DISPATCH_SOURCE);
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "reflected_names")
            .expect("concrete reflected function should lower");
        let TerminatorKind::ReflectedTypeDispatch {
            type_info,
            arms,
            otherwise,
        } = &mut function
            .blocks
            .iter_mut()
            .find(|block| {
                matches!(
                    block.terminator.kind,
                    TerminatorKind::ReflectedTypeDispatch { .. }
                )
            })
            .expect("expected reflected dispatch")
            .terminator
            .kind
        else {
            unreachable!();
        };
        let bound_type = arms[0].bound_type;
        type_info.kind = hir::ExpressionKind::Intrinsic {
            intrinsic: hir::IntrinsicId::TypeName,
            type_arguments: vec![bound_type],
            args: Vec::new(),
            evaluation_order: vec![0],
        };
        arms[0].target = BlockId(u32::MAX);
        *otherwise = BlockId(u32::MAX - 1);

        let errors = validate(&program).expect_err("invalid reflected dispatch must be rejected");
        let messages = errors
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>();
        assert!(
            messages.contains(&"evaluation order must be a permutation of the operand indexes")
        );
        assert!(
            messages.contains(&"reflected type dispatch arm target 4294967295 is out of range")
        );
        assert!(
            messages
                .contains(&"reflected type dispatch otherwise target 4294967294 is out of range")
        );
    }

    #[test]
    fn validation_rejects_duplicate_reflected_dispatch_arm_metadata() {
        let mut program = lower_source(REFLECTED_DISPATCH_SOURCE);
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "reflected_names")
            .expect("concrete reflected function should lower");
        let TerminatorKind::ReflectedTypeDispatch { arms, .. } = &mut function
            .blocks
            .iter_mut()
            .find(|block| {
                matches!(
                    block.terminator.kind,
                    TerminatorKind::ReflectedTypeDispatch { .. }
                )
            })
            .expect("expected reflected dispatch")
            .terminator
            .kind
        else {
            unreachable!();
        };
        arms[1].iteration_index = arms[0].iteration_index;
        arms[1].bound_type = arms[0].bound_type;

        let errors = validate(&program).expect_err("duplicate arm metadata must be rejected");
        let messages = errors
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>();
        assert!(messages.contains(&"reflected type dispatch contains a duplicate iteration index"));
        assert!(messages.contains(&"reflected type dispatch contains a duplicate bound type"));
    }

    #[test]
    fn reflected_dispatch_preserves_return_break_and_continue_edges() {
        let program = lower_source(REFLECTED_CONTROL_FLOW_SOURCE);
        for (name, expected) in [
            ("break_after_first", "break"),
            ("continue_all", "continue"),
            ("return_first", "return"),
        ] {
            let function = program
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == name)
                .unwrap_or_else(|| panic!("concrete {name} function should lower"));
            let (header, exit) = function
                .blocks
                .iter()
                .find_map(|block| match block.terminator.kind {
                    TerminatorKind::ForEach { exit, .. } => Some((block.id, exit)),
                    _ => None,
                })
                .expect("expected reflected field loop");
            let arms = function
                .blocks
                .iter()
                .find_map(|block| match &block.terminator.kind {
                    TerminatorKind::ReflectedTypeDispatch { arms, .. } => Some(arms),
                    _ => None,
                })
                .expect("expected reflected dispatch");

            assert_eq!(arms.len(), 2);
            for arm in arms {
                let terminator = &function.blocks[arm.target.index() as usize].terminator.kind;
                match expected {
                    "break" => assert_eq!(terminator, &TerminatorKind::Goto(exit)),
                    "continue" => assert_eq!(terminator, &TerminatorKind::Goto(header)),
                    "return" => assert!(matches!(terminator, TerminatorKind::Return(Some(_)))),
                    _ => unreachable!(),
                }
            }
        }
        validate(&program).expect("reflected control-flow edges must validate");
    }

    #[test]
    fn lowers_structured_control_flow_to_basic_blocks() {
        let program = lower_source(
            r#"namespace app
function choose(value: int64) returns int64:
    mutable int64 current = value
    while current > 0:
        if current == 1:
            break
        current = current - 1
    return current
"#,
        );
        let function = &program.functions[0];
        assert_eq!(function.entry.index(), 0);
        assert!(function.blocks.len() >= 7);
        assert!(
            function
                .blocks
                .iter()
                .any(|block| matches!(block.terminator.kind, TerminatorKind::Branch { .. }))
        );
        assert!(
            function
                .blocks
                .iter()
                .any(|block| matches!(block.terminator.kind, TerminatorKind::Return(_)))
        );
    }

    #[test]
    fn validation_rejects_an_out_of_range_goto_target() {
        let mut program = lower_source(
            r#"namespace app
function answer() returns int64:
    return 42
"#,
        );
        let span = program.functions[0].span;
        program.functions[0].blocks[0].terminator.kind = TerminatorKind::Goto(BlockId(u32::MAX));

        let errors = validate(&program).expect_err("invalid target must be rejected");

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].span, span);
        assert_eq!(errors[0].message, "goto target 4294967295 is out of range");
    }

    #[test]
    fn validation_rejects_an_out_of_range_branch_target() {
        let mut program = lower_source(
            r#"namespace app
function choose(flag: bool) returns int64:
    if flag:
        return 1
    return 2
"#,
        );
        let span = program.functions[0].span;
        let TerminatorKind::Branch { else_block, .. } =
            &mut program.functions[0].blocks[0].terminator.kind
        else {
            panic!("expected branch terminator");
        };
        *else_block = BlockId(u32::MAX);

        let errors = validate(&program).expect_err("invalid target must be rejected");

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].span, span);
        assert_eq!(
            errors[0].message,
            "branch else target 4294967295 is out of range"
        );
    }

    #[test]
    fn validation_rejects_an_out_of_range_function_entry() {
        let mut program = lower_source(
            r#"namespace app
function answer() returns int64:
    return 42
"#,
        );
        let span = program.functions[0].span;
        program.functions[0].entry = BlockId(u32::MAX);

        let errors = validate(&program).expect_err("invalid entry must be rejected");

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].span, span);
        assert_eq!(
            errors[0].message,
            "function entry block 4294967295 is out of range"
        );
    }

    #[test]
    fn validation_rejects_a_noncanonical_block_id() {
        let mut program = lower_source(
            r#"namespace app
function answer() returns int64:
    return 42
"#,
        );
        let span = program.functions[0].span;
        program.functions[0].blocks[0].id = BlockId(7);

        let errors = validate(&program).expect_err("noncanonical ID must be rejected");

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].span, span);
        assert_eq!(errors[0].message, "block at index 0 has noncanonical ID 7");
    }

    #[test]
    fn validation_rejects_an_out_of_range_switch_target() {
        let mut program = lower_source(
            r#"namespace app
function answer() returns int64:
    return 42
"#,
        );
        let span = program.functions[0].span;
        let TerminatorKind::Return(Some(scrutinee)) =
            program.functions[0].blocks[0].terminator.kind.clone()
        else {
            panic!("expected return value");
        };
        program.functions[0].blocks[0].terminator.kind = TerminatorKind::Switch {
            scrutinee,
            variants: Vec::new(),
            otherwise: Some(BlockId(u32::MAX)),
        };

        let errors = validate(&program).expect_err("invalid target must be rejected");

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].span, span);
        assert_eq!(
            errors[0].message,
            "switch otherwise target 4294967295 is out of range"
        );
    }

    #[test]
    fn validation_rejects_an_out_of_range_for_target() {
        let mut program = lower_source(
            r#"namespace app
function first(items: list[int64]) returns int64:
    for item in view items:
        return item
    return 0
"#,
        );
        let span = program.functions[0].span;
        let block = program.functions[0]
            .blocks
            .iter_mut()
            .find(|block| matches!(block.terminator.kind, TerminatorKind::ForEach { .. }))
            .expect("expected for terminator");
        let TerminatorKind::ForEach { exit, .. } = &mut block.terminator.kind else {
            unreachable!();
        };
        *exit = BlockId(u32::MAX);

        let errors = validate(&program).expect_err("invalid target must be rejected");

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].span, span);
        assert_eq!(
            errors[0].message,
            "for exit target 4294967295 is out of range"
        );
    }

    #[test]
    fn for_loop_back_edge_skips_preheader_statements() {
        let program = lower_source(
            r#"namespace app
function total(items: list[int64]) returns int64:
    mutable int64 sum = 0
    for item in view items:
        sum = sum + item
    return sum
"#,
        );
        let function = &program.functions[0];
        let entry = &function.blocks[function.entry.index() as usize];
        assert_eq!(
            entry.statements.len(),
            1,
            "preheader should initialize sum once"
        );
        let TerminatorKind::Goto(loop_header) = entry.terminator.kind else {
            panic!("preheader should jump to a dedicated for-loop header");
        };

        let header = &function.blocks[loop_header.index() as usize];
        assert!(header.statements.is_empty());
        let TerminatorKind::ForEach { body, .. } = header.terminator.kind else {
            panic!("dedicated loop header should own the for terminator");
        };
        assert_eq!(
            function.blocks[body.index() as usize].terminator.kind,
            TerminatorKind::Goto(loop_header),
            "for-loop back edge should not repeat preheader statements"
        );
    }

    #[test]
    fn terminating_if_branches_do_not_leave_an_unreachable_join() {
        let program = lower_source(
            r#"namespace app
function choose(flag: bool) returns int64:
    if flag:
        return 1
    else:
        return 2
"#,
        );
        let function = &program.functions[0];

        assert_eq!(reachable_blocks(function).len(), function.blocks.len());
    }

    #[test]
    fn terminating_match_arms_do_not_leave_an_unreachable_join() {
        let program = lower_source(
            r#"namespace app
enum Choice:
    first
    second
function choose(value: Choice) returns int64:
    match value:
        first:
            return 1
        second:
            return 2
"#,
        );
        let function = &program.functions[0];

        assert_eq!(reachable_blocks(function).len(), function.blocks.len());
    }

    #[test]
    fn preserves_source_spans_for_mir_statements_and_terminators() {
        let source = r#"namespace app
function choose(value: int64) returns int64:
    int64 next = value + 1
    if next > 1:
        return next
    return value
"#;
        let program = lower_source(source);
        let function = &program.functions[0];
        let entry = &function.blocks[function.entry.index() as usize];

        assert_eq!(
            &source[entry.statements[0].span.start as usize..entry.statements[0].span.end as usize],
            "int64 next = value + 1"
        );
        assert_eq!(
            &source[entry.terminator.span.start as usize..entry.terminator.span.end as usize],
            "next > 1"
        );

        let return_spans = function
            .blocks
            .iter()
            .filter_map(|block| match block.terminator.kind {
                TerminatorKind::Return(_) => Some(block.terminator.span),
                _ => None,
            })
            .map(|span| &source[span.start as usize..span.end as usize])
            .collect::<Vec<_>>();
        assert_eq!(return_spans, ["return next", "return value"]);
    }

    #[test]
    fn responding_handler_branches_do_not_fall_through() {
        let program = lower_source(
            r#"namespace app
actor Counter:
    receive choose(flag: bool) responds int64:
        if flag:
            respond 1
        else:
            respond 2
"#,
        );
        let handler = &program.functions[0];
        assert_eq!(handler.blocks.len(), 3, "responding branches need no join");
        let cfg = ControlFlowGraph::analyze(handler).expect("valid handler CFG");
        for target in cfg.successors(handler.entry) {
            assert!(
                cfg.successors(*target).is_empty(),
                "respond ends the handler"
            );
        }
    }
}
