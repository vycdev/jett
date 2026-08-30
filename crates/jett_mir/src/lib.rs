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
pub enum Statement {
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
pub enum Terminator {
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
    let mut builder = Builder::new();
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
    fn new() -> Self {
        Self {
            blocks: vec![BasicBlock {
                id: BlockId(0),
                statements: Vec::new(),
                terminator: Terminator::Unreachable,
            }],
            current: BlockId(0),
            loops: Vec::new(),
        }
    }

    fn new_block(&mut self) -> BlockId {
        let id = BlockId(self.blocks.len() as u32);
        self.blocks.push(BasicBlock {
            id,
            statements: Vec::new(),
            terminator: Terminator::Unreachable,
        });
        id
    }

    fn open(&self) -> bool {
        matches!(
            self.blocks[self.current.index() as usize].terminator,
            Terminator::Unreachable
        )
    }

    fn terminate(&mut self, terminator: Terminator) {
        self.blocks[self.current.index() as usize].terminator = terminator;
    }

    fn push(&mut self, statement: Statement) {
        self.blocks[self.current.index() as usize]
            .statements
            .push(statement);
    }

    fn close_to(&mut self, target: BlockId) {
        if self.open() {
            self.terminate(Terminator::Goto(target));
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
            hir::StatementKind::Let { local, value } => self.push(Statement::Let {
                local: *local,
                value: value.clone(),
            }),
            hir::StatementKind::Assign { target, value } => self.push(Statement::Assign {
                target: target.clone(),
                value: value.clone(),
            }),
            hir::StatementKind::Expression(value) => self.push(Statement::Evaluate(value.clone())),
            hir::StatementKind::HandleDefault(value) => {
                self.push(Statement::HandleDefault(value.clone()))
            }
            hir::StatementKind::Return(value) => self.terminate(Terminator::Return(value.clone())),
            hir::StatementKind::Break => {
                let target = self.loops.last().expect("validated break has a loop").1;
                self.terminate(Terminator::Goto(target));
            }
            hir::StatementKind::Continue => {
                let target = self.loops.last().expect("validated continue has a loop").0;
                self.terminate(Terminator::Goto(target));
            }
            hir::StatementKind::If {
                condition,
                then_block,
                else_block,
            } => self.lower_if(condition, then_block, else_block.as_ref()),
            hir::StatementKind::While { condition, body } => self.lower_while(condition, body),
            hir::StatementKind::For {
                key,
                value,
                by_view,
                iterable,
                body,
            } => self.lower_for(*key, *value, *by_view, iterable, body),
            hir::StatementKind::Match { scrutinee, arms } => self.lower_match(scrutinee, arms),
            hir::StatementKind::Assert { condition, message } => self.push(Statement::Assert {
                condition: condition.clone(),
                message: message.clone(),
            }),
            hir::StatementKind::Trace(local) => self.push(Statement::Trace(*local)),
            hir::StatementKind::Breakpoint(condition) => {
                self.push(Statement::Breakpoint(condition.clone()))
            }
            hir::StatementKind::Respond(value) => self.push(Statement::Respond(value.clone())),
            hir::StatementKind::Scope(block) => self.lower_block(block),
        }
    }

    fn lower_if(
        &mut self,
        condition: &Expression,
        then_body: &hir::Block,
        else_body: Option<&hir::Block>,
    ) {
        let then_block = self.new_block();
        let else_block = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::Branch {
            condition: condition.clone(),
            then_block,
            else_block,
        });
        self.current = then_block;
        self.lower_block(then_body);
        self.close_to(join);
        self.current = else_block;
        if let Some(body) = else_body {
            self.lower_block(body);
        }
        self.close_to(join);
        self.current = join;
    }

    fn lower_while(&mut self, condition: &Expression, body: &hir::Block) {
        let condition_block = self.new_block();
        let body_block = self.new_block();
        let exit = self.new_block();
        self.terminate(Terminator::Goto(condition_block));
        self.current = condition_block;
        self.terminate(Terminator::Branch {
            condition: condition.clone(),
            then_block: body_block,
            else_block: exit,
        });
        self.loops.push((condition_block, exit));
        self.current = body_block;
        self.lower_block(body);
        self.close_to(condition_block);
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
    ) {
        let header = self.current;
        let body_block = self.new_block();
        let exit = self.new_block();
        self.terminate(Terminator::ForEach {
            key,
            value,
            by_view,
            iterable: iterable.clone(),
            body: body_block,
            exit,
        });
        self.loops.push((header, exit));
        self.current = body_block;
        self.lower_block(body);
        self.close_to(header);
        self.loops.pop();
        self.current = exit;
    }

    fn lower_match(&mut self, scrutinee: &Expression, arms: &[hir::MatchArm]) {
        let join = self.new_block();
        let arm_blocks = arms.iter().map(|_| self.new_block()).collect::<Vec<_>>();
        let mut variants = Vec::new();
        let mut otherwise = None;
        for (arm, block) in arms.iter().zip(&arm_blocks) {
            if let Some(variant) = arm.variant {
                variants.push((variant, *block, arm.bindings.clone()));
            } else {
                otherwise = Some(*block);
            }
        }
        self.terminate(Terminator::Switch {
            scrutinee: scrutinee.clone(),
            variants,
            otherwise,
        });
        for (arm, block) in arms.iter().zip(arm_blocks) {
            self.current = block;
            self.lower_block(&arm.body);
            self.close_to(join);
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
                .any(|block| matches!(block.terminator, Terminator::Branch { .. }))
        );
        assert!(
            function
                .blocks
                .iter()
                .any(|block| matches!(block.terminator, Terminator::Return(_)))
        );
    }
}
