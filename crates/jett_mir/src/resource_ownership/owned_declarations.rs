//! Exact nested owned declarations minted only during fresh checked MIR lowering.
use super::*;

#[derive(Debug, Clone)]
struct Initialization {
    site: ResourceSite,
    header: Local,
    endpoint: Expression,
}
impl PartialEq for Initialization {
    fn eq(&self, other: &Self) -> bool {
        self.site == other.site
            && self.header == other.header
            && crate::breakpoint_regions::expressions_equal(&self.endpoint, &other.endpoint)
    }
}

#[derive(Debug, Clone)]
pub(super) struct OwnedDeclaration {
    original_path: Vec<usize>,
    original_scope: Vec<usize>,
    original_header: Local,
    original_span: Span,
    original_value: Expression,
    current: Option<Initialization>,
}
impl PartialEq for OwnedDeclaration {
    fn eq(&self, other: &Self) -> bool {
        self.original_path == other.original_path
            && self.original_scope == other.original_scope
            && self.original_header == other.original_header
            && self.original_span == other.original_span
            && crate::breakpoint_regions::expressions_equal(
                &self.original_value,
                &other.original_value,
            )
            && self.current == other.current
    }
}
impl OwnedDeclaration {
    pub(super) fn original_local(&self) -> LocalId {
        self.original_header.id
    }
    pub(super) fn current_local(&self) -> Option<LocalId> {
        self.current.as_ref().map(|current| current.header.id)
    }
    pub(super) fn declaration(&self) -> lexical_borrows::Declaration {
        lexical_borrows::Declaration {
            path: self.original_path.clone(),
            scope: self.original_scope.clone(),
            local: self.original_header.id,
            value: self.original_value.clone(),
            span: self.original_span,
        }
    }
}

/// Root declarations retain their existing function-completion custody.
/// Wider carriers retain their dedicated conservative admission path.
pub(super) fn capture_owned(
    function: &hir::Function,
    manifest: &hir::ResourceManifest,
    types: &TypeInterner,
) -> Vec<OwnedDeclaration> {
    lexical_borrows::source_declarations(function, manifest)
        .into_iter()
        .filter_map(|declaration| {
            let header = function
                .locals
                .get(declaration.local.index() as usize)
                .filter(|header| header.id == declaration.local)?;
            if declaration.scope.is_empty()
                || header.view_source.is_some()
                || !shape(manifest, types, header.ty).is_ok_and(|shape| shape.is_some())
            {
                return None;
            }
            Some(OwnedDeclaration {
                original_path: declaration.path,
                original_scope: declaration.scope,
                original_header: header.clone(),
                original_span: declaration.span,
                original_value: declaration.value,
                current: None,
            })
        })
        .collect()
}

pub(super) fn same_owners(left: &[OwnedDeclaration], right: &[OwnedDeclaration]) -> bool {
    left == right
}

pub(super) fn owner_current_local(
    witness: &ResourceLoweringWitness,
    original: LocalId,
) -> Option<LocalId> {
    witness
        .lexical_owners
        .iter()
        .find(|row| row.original_local() == original)
        .and_then(OwnedDeclaration::current_local)
}

impl Capture {
    /// Called only at the freshly lowered Let, before its exact node is pushed.
    pub(crate) fn bind_owned_declaration(
        &mut self,
        original: &hir::Statement,
        endpoint: &Expression,
        header: &Local,
        block: BlockId,
        index: usize,
    ) -> Result<bool, String> {
        let Some(witness) = &mut self.witness else {
            return Ok(false);
        };
        let hir::StatementKind::Let { local, value } = &original.kind else {
            return Ok(false);
        };
        let mut rows = witness
            .lexical_owners
            .iter_mut()
            .filter(|row| row.original_local() == *local);
        let Some(row) = rows.next() else {
            return Ok(false);
        };
        if rows.next().is_some()
            || row.current.is_some()
            || row.original_span != original.span
            || !crate::breakpoint_regions::expressions_equal(&row.original_value, value)
            || row.original_header != *header
            || header.view_source.is_some()
            || endpoint.ty != header.ty
        {
            return Err("Resource owned declaration changed its unique original initializer or current header".into());
        }
        row.current = Some(Initialization {
            site: ResourceSite {
                function: witness.original.id,
                block,
                position: ResourcePosition::Statement(index),
            },
            header: header.clone(),
            endpoint: endpoint.clone(),
        });
        Ok(true)
    }
}

pub(super) fn current(
    witness: &ResourceLoweringWitness,
    function: &Function,
) -> Result<(), String> {
    if !same_owners(&witness.lexical_owners, &witness.lexical_owner_seal) {
        return Err(
            "Resource owned declarations changed their independent original/current seal".into(),
        );
    }
    let originals = lexical_borrows::source_declarations(&witness.original, &witness.manifest);
    for (index, row) in witness.lexical_owners.iter().enumerate() {
        let Some(current) = &row.current else {
            return Err("Resource owned declaration lost its fresh current initialization".into());
        };
        if row.original_scope.is_empty()
            || row.original_header.view_source.is_some()
            || witness
                .original
                .locals
                .get(row.original_local().index() as usize)
                != Some(&row.original_header)
            || originals
                .iter()
                .filter(|original| {
                    original.local == row.original_local()
                        && original.path == row.original_path
                        && original.scope == row.original_scope
                        && original.span == row.original_span
                        && crate::breakpoint_regions::expressions_equal(
                            &original.value,
                            &row.original_value,
                        )
                })
                .count()
                != 1
            || witness.lexical_owners[..index].iter().any(|other| {
                other.original_local() == row.original_local()
                    || other.original_path == row.original_path
                    || other.current_local() == row.current_local()
                    || other
                        .current
                        .as_ref()
                        .is_some_and(|other| other.site == current.site)
            })
        {
            return Err(
                "Resource owned declaration lost its exact original lexical membership".into(),
            );
        }
        let ResourcePosition::Statement(position) = current.site.position else {
            return Err("Resource owned initialization has no exact statement site".into());
        };
        let statement = function
            .blocks
            .get(current.site.block.index() as usize)
            .filter(|block| block.id == current.site.block)
            .and_then(|block| block.statements.get(position))
            .ok_or("Resource owned initialization lost its current statement")?;
        if current.site.function != function.id
            || function.local(current.header.id) != Some(&current.header)
            || current.header.view_source.is_some()
            || current.header.ty != row.original_header.ty
            || current.endpoint.ty != current.header.ty
            || statement.span != row.original_span
            || !matches!(&statement.kind, StatementKind::Let { local, value }
                if *local == current.header.id
                    && crate::breakpoint_regions::expressions_equal(value, &current.endpoint))
            || function
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .filter(|statement| {
                    matches!(&statement.kind, StatementKind::Let { local, .. }
                    if *local == current.header.id)
                })
                .count()
                != 1
        {
            return Err(
                "Resource owned initialization changed its unique current Let relation".into(),
            );
        }
    }
    Ok(())
}

pub(super) fn remap_blocks(
    witness: &mut ResourceLoweringWitness,
    map: &[Option<BlockId>],
) -> Result<(), String> {
    for row in &mut witness.lexical_owners {
        if let Some(current) = &mut row.current {
            current.site.block = map
                .get(current.site.block.index() as usize)
                .copied()
                .flatten()
                .ok_or("Resource canonical map removed an owned initialization")?;
        }
    }
    Ok(())
}

pub(super) fn remap_locals(
    witness: &mut ResourceLoweringWitness,
    remap: &mut impl FnMut(&mut LocalId),
) {
    for row in &mut witness.lexical_owners {
        let Some(current) = &mut row.current else {
            continue;
        };
        remap(&mut current.header.id);
        if let Some(source) = &mut current.header.view_source {
            remap(source);
        }
        let mut wrapper = BasicBlock {
            id: BlockId(0),
            statements: vec![Statement {
                kind: StatementKind::Evaluate(current.endpoint.clone()),
                span: current.endpoint.span,
            }],
            terminator: Terminator {
                kind: TerminatorKind::Unreachable,
                span: current.endpoint.span,
            },
        };
        crate::sequences::prune::block_locals(&mut wrapper, remap, &mut |_| {});
        if let StatementKind::Evaluate(endpoint) = wrapper.statements.remove(0).kind {
            current.endpoint = endpoint;
        }
    }
}

pub(super) fn shift_prefix(witness: &mut ResourceLoweringWitness, block: BlockId, count: usize) {
    for row in &mut witness.lexical_owners {
        if let Some(current) = &mut row.current
            && current.site.block == block
            && let ResourcePosition::Statement(index) = &mut current.site.position
        {
            *index += count;
        }
    }
}
