//! Jett's backend-neutral control-flow graph representation.

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
        let join = self.new_block(statement_span);
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
        self.close_to(join, then_body.span);
        self.current = else_block;
        if let Some(body) = else_body {
            self.lower_block(body);
        }
        self.close_to(join, else_span);
        self.current = join;
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
        let header = self.current;
        let body_block = self.new_block(body.span);
        let exit = self.new_block(statement_span);
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
        let join = self.new_block(statement_span);
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
        for (arm, block) in arms.iter().zip(arm_blocks) {
            self.current = block;
            self.lower_block(&arm.body);
            self.close_to(join, arm.span);
        }
        self.current = join;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

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
