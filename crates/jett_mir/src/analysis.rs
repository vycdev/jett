use crate::{BlockId, Function, TerminatorKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisError {
    pub block: Option<BlockId>,
    pub message: String,
}

/// Deterministic edge and traversal facts for one MIR function.
///
/// The graph contains one entry for every block, including blocks that are not
/// reachable from the function entry. Reverse postorder contains only reachable
/// blocks and is suitable for forward dataflow passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlFlowGraph {
    successors: Vec<Vec<BlockId>>,
    predecessors: Vec<Vec<BlockId>>,
    reverse_postorder: Vec<BlockId>,
}

impl ControlFlowGraph {
    pub fn analyze(function: &Function) -> Result<Self, Vec<AnalysisError>> {
        let block_count = function.blocks.len();
        let mut errors = Vec::new();
        if function.entry.index() as usize >= block_count {
            errors.push(AnalysisError {
                block: None,
                message: format!(
                    "function entry block {} is out of range",
                    function.entry.index()
                ),
            });
        }

        let mut successors = vec![Vec::new(); block_count];
        for (index, block) in function.blocks.iter().enumerate() {
            if block.id.index() as usize != index {
                errors.push(AnalysisError {
                    block: Some(block.id),
                    message: format!(
                        "block at index {index} has noncanonical ID {}",
                        block.id.index()
                    ),
                });
                continue;
            }
            let targets = terminator_successors(&block.terminator.kind);
            for target in targets {
                if target.index() as usize >= block_count {
                    errors.push(AnalysisError {
                        block: Some(block.id),
                        message: format!("target block {} is out of range", target.index()),
                    });
                } else if !successors[index].contains(&target) {
                    successors[index].push(target);
                }
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        let mut predecessors = vec![Vec::new(); block_count];
        for (source_index, targets) in successors.iter().enumerate() {
            let source = BlockId(source_index as u32);
            for target in targets {
                predecessors[target.index() as usize].push(source);
            }
        }
        let reverse_postorder = reverse_postorder(function.entry, &successors);

        Ok(Self {
            successors,
            predecessors,
            reverse_postorder,
        })
    }

    pub fn successors(&self, block: BlockId) -> &[BlockId] {
        self.successors
            .get(block.index() as usize)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn predecessors(&self, block: BlockId) -> &[BlockId] {
        self.predecessors
            .get(block.index() as usize)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn reverse_postorder(&self) -> &[BlockId] {
        &self.reverse_postorder
    }
}

fn terminator_successors(terminator: &TerminatorKind) -> Vec<BlockId> {
    match terminator {
        TerminatorKind::Return(_) | TerminatorKind::Respond(_) | TerminatorKind::Unreachable => {
            Vec::new()
        }
        TerminatorKind::Goto(target) => vec![*target],
        TerminatorKind::Branch {
            then_block,
            else_block,
            ..
        } => vec![*then_block, *else_block],
        TerminatorKind::Switch {
            variants,
            otherwise,
            ..
        } => variants
            .iter()
            .map(|(_, block, _)| *block)
            .chain(*otherwise)
            .collect(),
        TerminatorKind::ForEach { body, exit, .. } => vec![*body, *exit],
    }
}

fn reverse_postorder(entry: BlockId, successors: &[Vec<BlockId>]) -> Vec<BlockId> {
    let mut visited = vec![false; successors.len()];
    let mut postorder = Vec::new();
    let mut stack = vec![(entry, false)];

    while let Some((block, expanded)) = stack.pop() {
        let index = block.index() as usize;
        if expanded {
            postorder.push(block);
            continue;
        }
        if visited[index] {
            continue;
        }
        visited[index] = true;
        stack.push((block, true));
        for successor in successors[index].iter().rev() {
            if !visited[successor.index() as usize] {
                stack.push((*successor, false));
            }
        }
    }

    postorder.reverse();
    postorder
}
