//! Canonical suite construction over an exact retained checked ordinary table.
use crate::{
    Block, CallOwnership, DeclarationId, DeclarationKind, Expression, ExpressionKind, Function,
    FunctionDebugKind, FunctionId, FunctionIdentity, GeneratedOperation, LowerError, Program,
    Statement, StatementKind,
};
use jett_common::{FileId, Span};
use jett_parser::ast::Item;
use jett_types::TypeInterner;
use std::sync::Arc;

/// Append only the original checked entry file's zero-argument verify bodies.
/// The caller supplies neither a body nor a list of target IDs. Empty archives
/// cannot renew protected required-value or Graph custody through this boundary.
pub fn append_checked_native_verify_suite(
    program: &mut Program,
    checked: &Arc<jett_typecheck::CheckedResourceProgram>,
    entry_file: FileId,
) -> Result<Option<FunctionId>, Vec<LowerError>> {
    let span = program
        .functions
        .first()
        .map_or(Span::new(entry_file, 0, 0), |function| function.span);
    let result = append(program, checked, entry_file);
    result.map_err(|message| vec![LowerError { span, message }])
}

fn append(
    program: &mut Program,
    checked: &Arc<jett_typecheck::CheckedResourceProgram>,
    entry_file: FileId,
) -> Result<Option<FunctionId>, String> {
    let retained = program
        .resource_source
        .checked_program()
        .ok_or("native verify transition has no retained checked ordinary Source")?
        .clone();
    if !Arc::ptr_eq(&retained, checked) || !program.resource_source.belongs_to(checked) {
        return Err("native verify transition belongs to another checked Source".into());
    }
    if !program.resource_manifest.kinds().is_empty() {
        return Err("ordinary verify transition cannot authorize Resource execution".into());
    }
    let types = &checked.checked().interner;
    program.resource_source.validate_current(program, types)?;
    crate::reflected_callable::baked::validate_conversion_input(program).map_err(messages)?;
    crate::actor_graph::validate_conversion_input(program).map_err(messages)?;
    let targets = original_targets(program, entry_file)?;
    let Some(first) = targets.first() else {
        return Ok(None);
    };
    let namespace = first.identity.declaration.namespace.clone();
    let span = first.span;
    let name = format!("__native_verify_suite:{}", span.start);
    if program.functions.iter().any(|function| {
        function.span == span
            && function.identity.declaration.name == name
            && function.identity.declaration.kind == DeclarationKind::Verify
    }) {
        return Err("native verify transition already has its canonical suite".into());
    }
    let id = FunctionId::new(
        u32::try_from(program.functions.len())
            .map_err(|_| "native verify suite function table exceeds its ID range")?,
    );
    let statements = targets
        .iter()
        .map(|target| suite_call(target, &program.functions, types))
        .collect::<Result<Vec<_>, _>>()?;
    let suite = Function {
        id,
        debug_kind: FunctionDebugKind::named(&namespace, &name),
        identity: FunctionIdentity {
            declaration: DeclarationId {
                origin: first.identity.declaration.origin.clone(),
                namespace,
                name,
                kind: DeclarationKind::Verify,
            },
            scoped_type_bindings: Vec::new(),
            type_arguments: Vec::new(),
            specialization: jett_typecheck::CheckedGenericSpecialization::default(),
        },
        source_definition: None,
        params: Vec::new(),
        capture_count: 0,
        return_type: TypeInterner::NOTHING,
        locals: Vec::new(),
        body: Block { statements, span },
        span,
    };
    let mut candidate = program.clone();
    candidate.functions.push(suite);
    // The candidate differs by exactly the private factory's canonical suite.
    // Each existing transition checks the unchanged archive and exact old table
    // before retaining the new complete table, including this appended body.
    let mut graph_candidate = candidate.clone();
    crate::actor_graph::complete_conversion(&mut graph_candidate, program).map_err(messages)?;
    crate::reflected_callable::baked::complete_conversion(&mut candidate, program)
        .map_err(messages)?;
    candidate.resource_source.graph_values = graph_candidate.resource_source.graph_values;
    if program.resource_source.graph_values.is_some() {
        candidate
            .resource_source
            .install_materialized(crate::resource_source::Materialized {
                functions: candidate.functions.clone(),
                values: program.resource_source.required_materializations().to_vec(),
                required_only: program
                    .resource_source
                    .required_only_function_ids()
                    .to_vec(),
            });
    }
    crate::reflected_callable::baked::validate_conversion_input(&candidate).map_err(messages)?;
    crate::actor_graph::validate_conversion_input(&candidate).map_err(messages)?;
    candidate
        .resource_source
        .validate_current(&candidate, types)?;
    *program = candidate;
    Ok(Some(id))
}

fn original_targets(program: &Program, entry_file: FileId) -> Result<Vec<Function>, String> {
    let checked = program
        .resource_source
        .checked_program()
        .ok_or("native verify targets lost their checked Source")?;
    let mut namespaces = std::collections::HashMap::new();
    let mut targets = Vec::new();
    for item in &checked.module().items {
        if let Item::Namespace(namespace) = item {
            namespaces.insert(namespace.span.file, namespace.name.name.as_str());
            continue;
        }
        let Item::Verify(verify) = item else {
            continue;
        };
        if verify.span.file != entry_file {
            continue;
        }
        let namespace = namespaces
            .get(&verify.span.file)
            .copied()
            .unwrap_or_default();
        let origin = checked
            .source_origins()
            .get(&verify.span.file)
            .ok_or("native verify target has no checked Source origin")?;
        let name = format!("{}:{}", verify.name.name, verify.span.start);
        let mut originals = program
            .resource_source
            .functions()
            .iter()
            .filter(|function| {
                function.span == verify.span
                    && function.identity.declaration.kind == DeclarationKind::Verify
                    && function.identity.declaration.namespace == namespace
                    && function.identity.declaration.name == name
                    && &function.identity.declaration.origin == origin
            });
        let original = originals
            .next()
            .ok_or("native verify target has no exact original checked HIR body")?;
        if originals.next().is_some() {
            return Err("native verify target has multiple original checked HIR bodies".into());
        }
        let current = program
            .functions
            .get(original.id.index() as usize)
            .ok_or("native verify target disappeared from its protected current table")?;
        if current.id != original.id
            || current.identity != original.identity
            || current.debug_kind != original.debug_kind
            || current.source_definition != original.source_definition
            || current.span != original.span
            || current.body.span != original.body.span
            || !current.params.is_empty()
            || current.capture_count != 0
            || current.return_type != TypeInterner::NOTHING
            || !current.identity.scoped_type_bindings.is_empty()
            || !current.identity.type_arguments.is_empty()
            || current.identity.specialization
                != jett_typecheck::CheckedGenericSpecialization::default()
        {
            return Err(
                "native verify target changed its exact checked zero-argument header".into(),
            );
        }
        targets.push(current.clone());
    }
    Ok(targets)
}

fn suite_call(
    target: &Function,
    functions: &[Function],
    types: &TypeInterner,
) -> Result<Statement, String> {
    let ownership = CallOwnership::generated(
        GeneratedOperation::NativeSuite {
            function: target.id,
        },
        &[],
        &[],
        &[],
        TypeInterner::NOTHING,
        &[],
        types,
    )?;
    let expression = Expression {
        kind: ExpressionKind::Call {
            function: target.id,
            args: Vec::new(),
            evaluation_order: Vec::new(),
            ownership,
        },
        ty: TypeInterner::NOTHING,
        span: target.span,
    };
    crate::validate_hir_invocation(
        functions,
        &[],
        &expression,
        types,
        jett_typecheck::CheckedOwnershipContext::Verify,
    )?;
    Ok(Statement {
        kind: StatementKind::Expression(expression),
        span: target.span,
    })
}

fn messages(errors: Vec<LowerError>) -> String {
    errors
        .into_iter()
        .map(|error| error.message)
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
#[path = "native_verify_suite_tests.rs"]
mod tests;
