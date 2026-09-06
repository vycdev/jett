//! Jett's backend-neutral control-flow graph representation.

mod analysis;

pub use analysis::{AnalysisError, ControlFlowGraph};

use jett_common::Span;
use jett_hir::{self as hir, Expression, FunctionIdentity, LocalId, VariantId};
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
    pub identity: FunctionIdentity,
    pub return_type: TypeId,
    pub entry: BlockId,
    pub blocks: Vec<BasicBlock>,
    pub span: Span,
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
    Respond(Expression),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Terminator {
    pub kind: TerminatorKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TerminatorKind {
    Return(Option<Expression>),
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
    for function in &program.functions {
        if function.entry.index() as usize >= function.blocks.len() {
            errors.push(ValidationError {
                span: function.span,
                message: format!(
                    "function entry block {} is out of range",
                    function.entry.index()
                ),
            });
        }
        for (index, block) in function.blocks.iter().enumerate() {
            if block.id.index() as usize != index {
                errors.push(ValidationError {
                    span: function.span,
                    message: format!(
                        "block at index {index} has noncanonical ID {}",
                        block.id.index()
                    ),
                });
            }
            let block_count = function.blocks.len();
            let mut check_target = |target: BlockId, edge: &str| {
                if target.index() as usize >= block_count {
                    errors.push(ValidationError {
                        span: function.span,
                        message: format!("{edge} target {} is out of range", target.index()),
                    });
                }
            };
            match &block.terminator.kind {
                TerminatorKind::Goto(target) => check_target(*target, "goto"),
                TerminatorKind::Branch {
                    then_block,
                    else_block,
                    ..
                } => {
                    check_target(*then_block, "branch then");
                    check_target(*else_block, "branch else");
                }
                TerminatorKind::Switch {
                    variants,
                    otherwise,
                    ..
                } => {
                    for (variant, target, _) in variants {
                        check_target(*target, &format!("switch variant {}", variant.index()));
                    }
                    if let Some(target) = otherwise {
                        check_target(*target, "switch otherwise");
                    }
                }
                TerminatorKind::ForEach { body, exit, .. } => {
                    check_target(*body, "for body");
                    check_target(*exit, "for exit");
                }
                TerminatorKind::Return(_) | TerminatorKind::Unreachable => {}
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
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
    builder.lower_block(&function.body);
    Function {
        identity: function.identity.clone(),
        return_type: function.return_type,
        entry: BlockId(0),
        blocks: builder.blocks,
        span: function.span,
    }
}

struct Builder {
    blocks: Vec<BasicBlock>,
    current: BlockId,
    loops: Vec<(BlockId, BlockId)>,
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
            hir::StatementKind::Let { local, value } => self.push(
                StatementKind::Let {
                    local: *local,
                    value: value.clone(),
                },
                statement.span,
            ),
            hir::StatementKind::Assign { target, value } => self.push(
                StatementKind::Assign {
                    target: target.clone(),
                    value: value.clone(),
                },
                statement.span,
            ),
            hir::StatementKind::Expression(value) => {
                self.push(StatementKind::Evaluate(value.clone()), statement.span)
            }
            hir::StatementKind::HandleDefault(value) => {
                self.push(StatementKind::HandleDefault(value.clone()), statement.span)
            }
            hir::StatementKind::Return(value) => {
                self.terminate(TerminatorKind::Return(value.clone()), statement.span)
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
                self.push(StatementKind::Respond(value.clone()), statement.span)
            }
            hir::StatementKind::Scope(block) => self.lower_block(block),
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
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use jett_common::{FileId, SourceOrigin};

    use super::*;

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
                TerminatorKind::Return(_) | TerminatorKind::Unreachable => {}
            }
        }
        reachable
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
}
