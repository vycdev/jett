//! Locate an authenticated ordinary Graph operand at its exact current Source call.
//! This module is installed below resource_layout::graph_shapes.
use super::*;
use jett_hir as hir;
use jett_mir::{GraphSourceEdgeProof, ResourceFunctionPlan, ResourceOperation};

pub(super) fn source_edge(
    function: &Function,
    types: &TypeInterner,
    caller: &ResourceFunctionPlan,
    operation: &ResourceOperation,
    target: FunctionId,
    order: &[usize],
    parameter: usize,
) -> Result<Option<GraphSourceEdgeProof>, CodegenError> {
    let (expected_target, source, formals, expected_order) = match operation.role() {
        custody::ResourceOperationRole::InvokeSourceFunction {
            function,
            source,
            formals,
            evaluation_order,
            ..
        }
        | custody::ResourceOperationRole::BeginSourceFunction {
            function,
            source,
            formals,
            evaluation_order,
            ..
        } => (*function, source, formals, evaluation_order),
        _ => {
            return Err(pending(
                "Graph Source edge requires a Source function operation",
            ));
        }
    };
    let site = operation.site();
    if caller.function() != function.id
        || site.function() != function.id
        || !caller
            .operations()
            .iter()
            .any(|row| std::ptr::eq(row, operation))
        || expected_target != target
        || expected_order != order
        || parameter >= formals.len()
    {
        return Err(pending(
            "Graph Source edge operation has no exact caller/target tuple",
        ));
    }
    // Staged ResourceCall regions retain an archive, not a physical current Call.
    // That archive cannot establish an ordinary Graph edge occurrence.
    if !operation.is_expression_operation() {
        return Ok(None);
    }
    let mut blocks = function
        .blocks
        .iter()
        .filter(|block| block.id == site.block());
    let block = blocks
        .next()
        .ok_or_else(|| pending("Graph Source edge site block is absent"))?;
    if blocks.next().is_some() {
        return Err(pending("Graph Source edge site block is ambiguous"));
    }
    let mut walk = Walk {
        pending: Vec::new(),
        queued: 0,
    };
    match site.position() {
        custody::ResourcePosition::Statement(index) => {
            let statement = block
                .statements
                .get(index)
                .ok_or_else(|| pending("Graph Source edge statement is absent"))?;
            walk.mir_statement(statement)?;
        }
        custody::ResourcePosition::Terminator => walk.terminator(&block.terminator)?,
    }
    let mut selected = None;
    while let Some(work) = walk.pending.pop() {
        match work {
            Work::Expression(expression) => {
                if caller
                    .operations_for_expression(expression)
                    .any(|row| std::ptr::eq(row, operation))
                {
                    if selected.is_some() {
                        return Err(pending("Graph Source edge has ambiguous current calls"));
                    }
                    let ExpressionKind::Call {
                        function: actual_target,
                        args,
                        evaluation_order,
                        ownership: hir::CallOwnership::Source(actual_source),
                        ..
                    } = &expression.kind
                    else {
                        return Err(pending(
                            "Graph Source edge operation lost its physical Source Call",
                        ));
                    };
                    if *actual_target != target
                        || actual_source != source
                        || evaluation_order != order
                        || args.len() != formals.len()
                        || source.arguments.len() != formals.len()
                        || args.len() > LIMIT
                        || order.len() != args.len()
                    {
                        return Err(pending(
                            "Graph Source edge current Call changed its sealed tuple",
                        ));
                    }
                    let mut seen = vec![false; args.len()];
                    for index in order {
                        let entry = seen.get_mut(*index).ok_or_else(|| {
                            pending("Graph Source edge argument order is out of range")
                        })?;
                        if *entry {
                            return Err(pending("Graph Source edge argument order is repeated"));
                        }
                        *entry = true;
                    }
                    for (index, ((argument, formal), actual)) in
                        source.arguments.iter().zip(formals).zip(args).enumerate()
                    {
                        if argument.parameter_index != index
                            || formal.parameter() != index
                            || argument.source_index != formal.source_index()
                            || argument.actual_type != formal.actual_type()
                            || argument.parameter_type != formal.parameter_type()
                            || actual.ty != formal.actual_type()
                        {
                            return Err(pending(
                                "Graph Source edge current Call changed a formal/actual tuple",
                            ));
                        }
                    }
                    selected = Some(expression);
                }
                walk.expression(expression)?;
            }
            Work::Block(block) => {
                for statement in &block.statements {
                    walk.push(Work::HirStatement(statement))?;
                }
            }
            Work::HirStatement(statement) => walk.hir_statement(statement)?,
        }
    }
    let expression =
        selected.ok_or_else(|| pending("Graph Source edge has no exact current Call"))?;
    Ok(custody::native_graph_source_edge(
        function, types, expression, parameter,
    ))
}

enum Work<'a> {
    Expression(&'a Expression),
    Block(&'a hir::Block),
    HirStatement(&'a hir::Statement),
}
struct Walk<'a> {
    pending: Vec<Work<'a>>,
    queued: usize,
}
impl<'a> Walk<'a> {
    fn push(&mut self, work: Work<'a>) -> Result<(), CodegenError> {
        if self.queued >= LIMIT {
            return Err(pending(
                "Graph Source edge current subtree exceeds the bounded row domain",
            ));
        }
        self.queued += 1;
        self.pending.push(work);
        Ok(())
    }
    fn value(&mut self, value: &'a Expression) -> Result<(), CodegenError> {
        self.push(Work::Expression(value))
    }
    fn optional(&mut self, value: &'a Option<Expression>) -> Result<(), CodegenError> {
        if let Some(value) = value {
            self.value(value)?;
        }
        Ok(())
    }
    fn values(&mut self, values: &'a [Expression]) -> Result<(), CodegenError> {
        for value in values {
            self.value(value)?;
        }
        Ok(())
    }
    fn mir_statement(&mut self, statement: &'a jett_mir::Statement) -> Result<(), CodegenError> {
        match &statement.kind {
            StatementKind::ResourceCall(custody::ResourceCallNode::Stage { value, .. })
            | StatementKind::ResourceConstructor(custody::ResourceConstructorNode::Stage {
                value,
                ..
            })
            | StatementKind::Let { value, .. }
            | StatementKind::BeginCallView { value, .. }
            | StatementKind::CheckRefinement { call: value, .. }
            | StatementKind::Evaluate(value)
            | StatementKind::HandleDefault(value) => self.value(value)?,
            StatementKind::Assign { target, value }
            | StatementKind::UpdateField { target, value, .. } => {
                self.value(target)?;
                self.value(value)?;
            }
            StatementKind::Assert { condition, message } => {
                self.value(condition)?;
                self.optional(message)?;
            }
            StatementKind::Breakpoint { condition, .. } => self.optional(condition)?,
            StatementKind::ResourceCall(
                custody::ResourceCallNode::Begin { .. }
                | custody::ResourceCallNode::Invoke { .. }
                | custody::ResourceCallNode::End { .. },
            )
            | StatementKind::ResourceConstructor(
                custody::ResourceConstructorNode::Begin { .. }
                | custody::ResourceConstructorNode::Commit { .. }
                | custody::ResourceConstructorNode::End { .. },
            )
            | StatementKind::ResourceLexicalExit(_)
            | StatementKind::OpenCallOwnerGeneration { .. }
            | StatementKind::ReplaceCallOwnerGeneration { .. }
            | StatementKind::PrepareFieldUpdate { .. }
            | StatementKind::CloseCallOwnerGeneration { .. }
            | StatementKind::EndCallView { .. }
            | StatementKind::ReflectedContainerReady { .. }
            | StatementKind::ReflectedValueReady { .. }
            | StatementKind::IterationBorrow { .. }
            | StatementKind::SequenceLength { .. }
            | StatementKind::SequenceGet { .. }
            | StatementKind::SumTag { .. }
            | StatementKind::SumTake { .. }
            | StatementKind::Trace(_) => {}
        }
        Ok(())
    }
    fn terminator(&mut self, terminator: &'a jett_mir::Terminator) -> Result<(), CodegenError> {
        match &terminator.kind {
            TerminatorKind::Return(value) => self.optional(value)?,
            TerminatorKind::Respond(value)
            | TerminatorKind::Branch {
                condition: value, ..
            }
            | TerminatorKind::Switch {
                scrutinee: value, ..
            }
            | TerminatorKind::ForEach {
                iterable: value, ..
            }
            | TerminatorKind::ReflectedTypeDispatch {
                type_info: value, ..
            } => self.value(value)?,
            TerminatorKind::Goto(_) | TerminatorKind::Unreachable => {}
        }
        Ok(())
    }
    fn hir_statement(&mut self, statement: &'a hir::Statement) -> Result<(), CodegenError> {
        match &statement.kind {
            hir::StatementKind::Let { value, .. }
            | hir::StatementKind::HandleDefault(value)
            | hir::StatementKind::Expression(value)
            | hir::StatementKind::Respond(value) => self.value(value)?,
            hir::StatementKind::Assign { target, value }
            | hir::StatementKind::UpdateField { target, value, .. } => {
                self.value(target)?;
                self.value(value)?;
            }
            hir::StatementKind::Return(value) => self.optional(value)?,
            hir::StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                self.value(condition)?;
                self.push(Work::Block(then_block))?;
                if let Some(block) = else_block {
                    self.push(Work::Block(block))?;
                }
            }
            hir::StatementKind::While { condition, body } => {
                self.value(condition)?;
                self.push(Work::Block(body))?;
            }
            hir::StatementKind::For { iterable, body, .. } => {
                self.value(iterable)?;
                self.push(Work::Block(body))?;
            }
            hir::StatementKind::Match { scrutinee, arms } => {
                self.value(scrutinee)?;
                for arm in arms {
                    self.push(Work::Block(&arm.body))?;
                }
            }
            hir::StatementKind::Assert { condition, message } => {
                self.value(condition)?;
                self.optional(message)?;
            }
            hir::StatementKind::Breakpoint { condition, .. } => self.optional(condition)?,
            hir::StatementKind::Scope(block) => self.push(Work::Block(block))?,
            hir::StatementKind::ReflectedTypeDispatch { type_info, arms } => {
                self.value(type_info)?;
                for arm in arms {
                    self.push(Work::Block(&arm.body))?;
                }
            }
            hir::StatementKind::Break
            | hir::StatementKind::Continue
            | hir::StatementKind::Trace(_) => {}
        }
        Ok(())
    }
    fn expression(&mut self, expression: &'a Expression) -> Result<(), CodegenError> {
        // Exhaustive over current MIR-carried expressions. ActorGraphImport's
        // factory archive is intentionally not a current physical subtree.
        match &expression.kind {
            ExpressionKind::Binary { left, right, .. } => {
                self.value(left)?;
                self.value(right)?;
            }
            ExpressionKind::Unary { value, .. }
            | ExpressionKind::ResultOk(value)
            | ExpressionKind::ResultFail(value)
            | ExpressionKind::OptionalSome(value)
            | ExpressionKind::Comptime { value, .. }
            | ExpressionKind::DisplayResult(value)
            | ExpressionKind::EquatableResult(value)
            | ExpressionKind::RuntimeFailureMessage(value)
            | ExpressionKind::Declassify(value)
            | ExpressionKind::Coarsen(value)
            | ExpressionKind::RefinementValidated(value)
            | ExpressionKind::InterfaceType(value)
            | ExpressionKind::Run(value)
            | ExpressionKind::Join(value)
            | ExpressionKind::Cancel(value)
            | ExpressionKind::View(value)
            | ExpressionKind::Clone(value)
            | ExpressionKind::FunctionAdapter { value, .. }
            | ExpressionKind::InterfaceCoerce { value, .. }
            | ExpressionKind::StateIs { value, .. }
            | ExpressionKind::Field { base: value, .. } => self.value(value)?,
            ExpressionKind::ActorGraphStep { args, .. }
            | ExpressionKind::ResourceInvoke { args, .. }
            | ExpressionKind::Call { args, .. }
            | ExpressionKind::Intrinsic { args, .. }
            | ExpressionKind::ActorSpawn { args, .. } => self.values(args)?,
            ExpressionKind::IndirectCall { callee, args, .. } => {
                self.value(callee)?;
                self.values(args)?;
            }
            ExpressionKind::StructConstruct { fields, .. }
            | ExpressionKind::BitfieldConstruct { fields, .. } => self.values(fields)?,
            ExpressionKind::MachineConstruct { payloads, .. }
            | ExpressionKind::EnumConstruct { payloads, .. } => self.values(payloads)?,
            ExpressionKind::MachineTransition {
                source, payloads, ..
            } => {
                self.value(source)?;
                self.values(payloads)?;
            }
            ExpressionKind::ListConstruct { elements } => self.values(elements)?,
            ExpressionKind::MapConstruct { entries } => {
                for entry in entries {
                    self.value(&entry.key)?;
                    self.value(&entry.value)?;
                }
            }
            ExpressionKind::Handle {
                target, failure, ..
            } => {
                self.value(target)?;
                self.push(Work::Block(failure))?;
            }
            ExpressionKind::StringInterpolation(parts) => {
                for part in parts {
                    if let hir::StringSegment::Value(value) = part {
                        self.value(value)?;
                    }
                }
            }
            ExpressionKind::InlineFunction { body, .. } => self.push(Work::Block(body))?,
            ExpressionKind::ActorMessage { actor, args, .. } => {
                self.value(actor)?;
                self.values(args)?;
            }
            ExpressionKind::ActorGraphImport(_)
            | ExpressionKind::ResourceHookValue { .. }
            | ExpressionKind::FunctionRef(_)
            | ExpressionKind::ClosureRef { .. }
            | ExpressionKind::Int(_)
            | ExpressionKind::Constant { .. }
            | ExpressionKind::Float(_)
            | ExpressionKind::String(_)
            | ExpressionKind::Bool(_)
            | ExpressionKind::Nothing
            | ExpressionKind::PropertyCaseContext(_)
            | ExpressionKind::RuntimeFailure(_)
            | ExpressionKind::Local(_)
            | ExpressionKind::OptionalNone => {}
        }
        Ok(())
    }
}
