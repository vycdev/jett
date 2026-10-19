//! Derived only after the whole original checked HIR archive is authenticated.
//! A current body or signature alone can never add a function to this family.
use super::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct ResourceExecutionClosure {
    functions: BTreeSet<u32>,
    metadata_handlers: BTreeSet<u32>,
    graph_analysis: Option<std::sync::Arc<graph_metadata::OriginalGraphAnalysis>>,
}
impl PartialEq for ResourceExecutionClosure {
    fn eq(&self, other: &Self) -> bool {
        self.functions == other.functions
            && self.metadata_handlers == other.metadata_handlers
            && match (&self.graph_analysis, &other.graph_analysis) {
                (None, None) => true,
                (Some(left), Some(right)) => left.same_source(right),
                _ => false,
            }
    }
}
impl Eq for ResourceExecutionClosure {}
impl ResourceExecutionClosure {
    pub(super) fn from_archive(
        archive: &hir::ResourceSourceArchive,
        types: &TypeInterner,
    ) -> Result<Self, String> {
        let analysis = std::sync::Arc::new(graph_metadata::OriginalGraphAnalysis::prepare(
            archive, types,
        )?);
        Self::from_analysis(archive, types, analysis)
    }

    /// Recompute closure membership from original bodies while retaining only
    /// the immutable equations authenticated for this exact execution archive.
    pub(super) fn reconstructed(
        &self,
        archive: &hir::ResourceSourceArchive,
        types: &TypeInterner,
    ) -> Result<Self, String> {
        let analysis = self
            .graph_analysis
            .as_ref()
            .ok_or("graph storage lost its private original archive analysis")?
            .clone();
        Self::from_analysis(archive, types, analysis)
    }

    fn from_analysis(
        archive: &hir::ResourceSourceArchive,
        types: &TypeInterner,
        analysis: std::sync::Arc<graph_metadata::OriginalGraphAnalysis>,
    ) -> Result<Self, String> {
        analysis.validate_archive(archive, types)?;
        let mut family = Self::from_original(archive.execution_functions(), types, archive)?;
        let mut candidates = BTreeSet::new();
        // A completed empty Graph table cannot retire any custody function.
        // Keep independent closure reconstruction and archive/type checks above.
        let metadata_functions = if analysis.has_rows() {
            archive.execution_functions()
        } else {
            &[]
        };
        for function in metadata_functions {
            let metadata = analysis.capture(function, archive, types)?;
            if !metadata.needed() {
                continue;
            }
            let mut pure_metadata = (!resource_type_pending(types, function.return_type)
                || metadata.original_storage_for_return().is_some())
                && function.locals.iter().all(|local| {
                    !resource_type_pending(types, local.ty)
                        || metadata.original_storage_for_local(local).is_some()
                });
            walk::hir_block(&function.body, &mut |value| {
                pure_metadata &= (!resource_type_pending(types, value.ty)
                    || metadata.original_storage_for_expression(value).is_some())
                    && !matches!(
                        value.kind,
                        hir::ExpressionKind::ResourceInvoke { .. }
                            | hir::ExpressionKind::ResourceHookValue { .. }
                    )
                    && !matches!(&value.kind,hir::ExpressionKind::ActorGraphStep {instruction,..}
                        if matches!(instruction.operation(),hir::ActorGraphOperation::HookBox {..}));
            });
            if pure_metadata {
                candidates.insert(function.id.index());
            }
        }
        // Metadata helpers can call each other, but an edge to any remaining
        // custody function keeps the exact original caller in that family.
        loop {
            let mut retained = Vec::new();
            for function in archive
                .execution_functions()
                .iter()
                .filter(|function| candidates.contains(&function.id.index()))
            {
                let mut calls_custody = false;
                walk::hir_block(&function.body, &mut |value| {
                    if let hir::ExpressionKind::Call {
                        function: target, ..
                    } = &value.kind
                    {
                        calls_custody |=
                            family.contains(*target) && !candidates.contains(&target.index());
                    }
                });
                if calls_custody {
                    retained.push(function.id.index());
                }
            }
            if retained.is_empty() {
                break;
            }
            for index in retained {
                candidates.remove(&index);
            }
        }
        for index in candidates {
            family.metadata_handlers.insert(index);
            family.functions.remove(&index);
        }

        family.functions.retain(|index| {
            !archive
                .required_only_function_ids()
                .iter()
                .any(|function| function.index() == *index)
        });
        family.graph_analysis = Some(analysis);
        Ok(family)
    }

    pub(super) fn has_no_original_graph_rows(&self, archive: &hir::ResourceSourceArchive) -> bool {
        self.graph_analysis
            .as_ref()
            .is_some_and(|analysis| analysis.has_no_rows_for(archive))
    }

    pub(super) fn original_graph(
        &self,
        original: &hir::Function,
        archive: &hir::ResourceSourceArchive,
        types: &TypeInterner,
    ) -> Result<&graph_metadata::Witness, String> {
        self.graph_analysis
            .as_ref()
            .ok_or("graph storage lost its private original archive analysis")?
            .capture(original, archive, types)
    }

    pub(super) fn metadata_handler(&self, function: FunctionId) -> bool {
        self.metadata_handlers.contains(&function.index())
    }
    pub(super) fn contains(&self, function: FunctionId) -> bool {
        self.functions.contains(&function.index())
    }
    pub(super) fn direct_call(&self, value: &Expression) -> bool {
        matches!(&value.kind, hir::ExpressionKind::Call { function, .. } if self.contains(*function))
    }
    pub(super) fn from_original(
        functions: &[hir::Function],
        types: &TypeInterner,
        archive: &hir::ResourceSourceArchive,
    ) -> Result<Self, String> {
        let mut family = Self::default();
        for (index, function) in functions.iter().enumerate() {
            if function.id.index() as usize != index {
                return Err(
                    "Resource original call closure has no exact dense function identity".into(),
                );
            }
            let mut needed = custody_type(types, function.return_type)
                || function
                    .locals
                    .iter()
                    .any(|local| custody_type(types, local.ty));
            walk::hir_block(&function.body, &mut |value| {
                needed |= custody_type(types, value.ty)
                    || matches!(
                        value.kind,
                        hir::ExpressionKind::ResourceHookValue { .. }
                            | hir::ExpressionKind::ResourceInvoke { .. }
                    );
            });
            if needed {
                family.functions.insert(function.id.index());
            }
        }
        loop {
            let mut predecessors = Vec::new();
            for function in functions {
                if family.contains(function.id) {
                    continue;
                }
                let mut calls = false;
                let mut malformed = false;
                let mut ordinal = 0;
                walk::hir_block(&function.body, &mut |value| {
                    ordinal += 1;
                    if let hir::ExpressionKind::Call {
                        function: target,
                        ownership,
                        ..
                    } = &value.kind
                        && family.contains(*target)
                    {
                        calls = true;
                        let suite = archive.native_suite_call(function.id, value, ordinal);
                        malformed |= !matches!(ownership, hir::CallOwnership::Source(_))
                            && !suite.is_some_and(|proof| {
                                proof.target() == *target
                                    && proof.belongs_to_archive(archive)
                                    && proof.matches(function.id, value, ordinal)
                            });
                    }
                });
                if malformed {
                    return Err(
                        "Resource execution predecessor has neither an original Source call certificate nor its exact private suite receipt"
                            .into(),
                    );
                }
                if calls {
                    predecessors.push(function.id);
                }
            }
            if predecessors.is_empty() {
                break;
            }
            family
                .functions
                .extend(predecessors.into_iter().map(|function| function.index()));
        }
        Ok(family)
    }
}
