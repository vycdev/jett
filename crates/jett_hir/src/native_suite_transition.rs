//! Constructor-only native suite occurrences over a complete checked table.
use crate::{Expression, ExpressionKind, Function, FunctionId, Program, ResourceSourceArchive};
use jett_common::FileId;
use jett_comptime::ExplicitComptimeValues;
use jett_typecheck::CheckedResourceProgram;
use std::sync::Arc;

#[derive(Debug)]
enum Purpose {
    Verify { targets: Vec<FunctionId> },
    Property(crate::CheckedNativePropertyPlan),
}

#[derive(Debug)]
struct Origin {
    checked: Arc<CheckedResourceProgram>,
    before: Program,
    entry_file: FileId,
    canonical: Function,
    purpose: Purpose,
}

#[derive(Debug)]
struct Call {
    origin: Arc<Origin>,
    ordinal: usize,
    expression: Expression,
}

/// An exact constructor-owned wrapper occurrence, never Source call authority.
#[derive(Clone)]
pub struct CheckedNativeSuiteCall(Arc<Call>);

impl std::fmt::Debug for CheckedNativeSuiteCall {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CheckedNativeSuiteCall")
            .field("owner", &self.owner())
            .field("target", &self.target())
            .field("ordinal", &self.expression_ordinal())
            .finish()
    }
}

impl PartialEq for CheckedNativeSuiteCall {
    fn eq(&self, other: &Self) -> bool {
        self.same_receipt(other)
    }
}
impl Eq for CheckedNativeSuiteCall {}

impl CheckedNativeSuiteCall {
    pub fn owner(&self) -> FunctionId {
        self.0.origin.canonical.id
    }
    pub fn target(&self) -> FunctionId {
        let ExpressionKind::Call { function, .. } = &self.0.expression.kind else {
            unreachable!("private suite call constructor")
        };
        *function
    }
    pub fn expression_ordinal(&self) -> usize {
        self.0.ordinal
    }
    pub fn arguments(&self) -> &[Expression] {
        let ExpressionKind::Call { args, .. } = &self.0.expression.kind else {
            unreachable!("private suite call constructor")
        };
        args
    }
    pub fn evaluation_order(&self) -> &[usize] {
        let ExpressionKind::Call {
            evaluation_order, ..
        } = &self.0.expression.kind
        else {
            unreachable!("private suite call constructor")
        };
        evaluation_order
    }
    pub fn same_receipt(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn matches(&self, owner: FunctionId, expression: &Expression, ordinal: usize) -> bool {
        self.owner() == owner
            && self.0.ordinal == ordinal
            && crate::resource_materialization::expression_equal(&self.0.expression, expression)
    }
    pub fn belongs_to_archive(&self, archive: &ResourceSourceArchive) -> bool {
        archive.belongs_to(&self.0.origin.checked)
            && archive.native_suites.as_ref().is_some_and(|transition| {
                transition.calls.iter().any(|call| self.same_receipt(call))
            })
    }
}

#[derive(Debug)]
pub(crate) struct Transition {
    origins: Vec<Arc<Origin>>,
    current: Vec<Function>,
    calls: Vec<CheckedNativeSuiteCall>,
}

impl Transition {
    pub(crate) fn functions(&self) -> &[Function] {
        &self.current
    }
    pub(crate) fn function(&self, id: FunctionId) -> Option<&Function> {
        self.origins
            .iter()
            .find(|origin| origin.canonical.id == id)?;
        self.current
            .get(id.index() as usize)
            .filter(|function| function.id == id)
    }
    pub(crate) fn call(
        &self,
        owner: FunctionId,
        expression: &Expression,
        ordinal: usize,
    ) -> Option<&CheckedNativeSuiteCall> {
        self.calls
            .iter()
            .find(|call| call.matches(owner, expression, ordinal))
    }
}

pub(crate) fn same_required_cache(program: &Program, values: &ExplicitComptimeValues) -> bool {
    program
        .resource_source
        .required_cache
        .as_ref()
        .is_some_and(|retained| retained.same_checked_cache(values))
        || crate::reflected_callable::baked::same_required_cache(program, values)
}

pub(crate) fn has_property_suite(program: &Program) -> bool {
    program
        .resource_source
        .native_suites
        .as_ref()
        .is_some_and(|transition| {
            transition
                .origins
                .iter()
                .any(|origin| matches!(origin.purpose, Purpose::Property(_)))
        })
        || crate::reflected_callable::baked::has_property_suite(program)
}

pub(crate) fn owns_property_suite(
    program: &Program,
    plan: &crate::CheckedNativePropertyPlan,
) -> bool {
    program
        .resource_source
        .native_suites
        .as_ref()
        .is_some_and(|transition| {
            transition.origins.iter().any(|origin| {
            matches!(&origin.purpose, Purpose::Property(current) if current.same_plan(plan))
        })
        })
}

pub(crate) fn append_verify(
    candidate: &mut Program,
    before: &Program,
    checked: &Arc<CheckedResourceProgram>,
    entry_file: FileId,
    canonical: &Function,
    targets: Vec<FunctionId>,
) -> Result<(), String> {
    append(
        candidate,
        before,
        checked,
        entry_file,
        canonical,
        Purpose::Verify { targets },
    )
}

pub(crate) fn append_property(
    candidate: &mut Program,
    before: &Program,
    checked: &Arc<CheckedResourceProgram>,
    entry_file: FileId,
    canonical: &Function,
    plan: &crate::CheckedNativePropertyPlan,
) -> Result<(), String> {
    append(
        candidate,
        before,
        checked,
        entry_file,
        canonical,
        Purpose::Property(plan.clone()),
    )
}

fn append(
    candidate: &mut Program,
    before: &Program,
    checked: &Arc<CheckedResourceProgram>,
    entry_file: FileId,
    canonical: &Function,
    purpose: Purpose,
) -> Result<(), String> {
    before
        .resource_source
        .validate_current(before, &checked.checked().interner)?;
    if !before.resource_source.belongs_to(checked)
        || candidate.resource_manifest != before.resource_manifest
        || candidate.equality_methods != before.equality_methods
        || candidate.functions.len() != before.functions.len() + 1
        || canonical.id.index() as usize != before.functions.len()
        || !crate::resource_materialization::functions_equal(
            &candidate.functions[..before.functions.len()],
            &before.functions,
        )
        || !crate::resource_materialization::functions_equal(
            &candidate.functions[before.functions.len()..],
            std::slice::from_ref(canonical),
        )
    {
        return Err("native suite append changed its complete checked before table".into());
    }
    let mut origins = before
        .resource_source
        .native_suites
        .as_ref()
        .map_or_else(Vec::new, |transition| transition.origins.clone());
    origins.push(Arc::new(Origin {
        checked: checked.clone(),
        before: before.clone(),
        entry_file,
        canonical: canonical.clone(),
        purpose,
    }));
    let transition = transition(origins, &candidate.functions)?;
    candidate.resource_source.native_suites = Some(Arc::new(transition));
    candidate
        .resource_source
        .validate_current(candidate, &checked.checked().interner)
}

fn transition(origins: Vec<Arc<Origin>>, current: &[Function]) -> Result<Transition, String> {
    let mut calls = Vec::new();
    for origin in &origins {
        if !origin.before.resource_source.belongs_to(&origin.checked)
            || origin.canonical.span.file != origin.entry_file
        {
            return Err("native suite origin lost its checked entry-file purpose".into());
        }
        let function = current
            .get(origin.canonical.id.index() as usize)
            .filter(|function| function.id == origin.canonical.id)
            .ok_or("native suite conversion lost its canonical owner")?;
        let mut header = function.clone();
        header.body = origin.canonical.body.clone();
        if !crate::resource_materialization::functions_equal(
            std::slice::from_ref(&header),
            std::slice::from_ref(&origin.canonical),
        ) {
            return Err("native suite conversion changed its canonical header or locals".into());
        }
        let mut body = function.body.clone();
        let mut ordinal = 0;
        let mut targets = Vec::new();
        crate::resource_materialization::walk_block(&mut body, &mut |expression| {
            ordinal += 1;
            let ExpressionKind::Call {
                function,
                ownership: crate::CallOwnership::Generated(packet),
                ..
            } = &expression.kind
            else {
                return Ok(());
            };
            if !matches!(&packet.operation, crate::GeneratedOperation::NativeSuite { function: target } if target == function)
            {
                return Ok(());
            }
            targets.push(*function);
            calls.push(CheckedNativeSuiteCall(Arc::new(Call {
                origin: origin.clone(),
                ordinal,
                expression: expression.clone(),
            })));
            Ok(())
        })?;
        match &origin.purpose {
            Purpose::Verify { targets: expected } if &targets != expected => {
                return Err("native verify suite changed its complete ordered targets".into());
            }
            Purpose::Property(plan) => {
                let mut canonical_targets = Vec::new();
                let mut canonical = origin.canonical.body.clone();
                crate::resource_materialization::walk_block(&mut canonical, &mut |expression| {
                    if let ExpressionKind::Call {
                        function,
                        ownership: crate::CallOwnership::Generated(packet),
                        ..
                    } = &expression.kind
                        && matches!(&packet.operation, crate::GeneratedOperation::NativeSuite { function: target } if target == function)
                    {
                        canonical_targets.push(*function);
                    }
                    Ok(())
                })?;
                if plan.entry() != origin.canonical.id || targets != canonical_targets {
                    return Err(
                        "native Property suite changed its complete active case order".into(),
                    );
                }
            }
            _ => {}
        }
    }
    Ok(Transition {
        origins,
        current: current.to_vec(),
        calls,
    })
}

pub(crate) fn validate_conversion_input(program: &Program) -> Result<(), String> {
    let Some(transition) = &program.resource_source.native_suites else {
        return Ok(());
    };
    for origin in &transition.origins {
        if !program.resource_source.belongs_to(&origin.checked) {
            return Err("native suite conversion belongs to another checked program".into());
        }
        program
            .resource_source
            .validate_current(program, &origin.checked.checked().interner)?;
    }
    Ok(())
}

/// Only the canonical conversion pass calls this after authenticating its input.
/// Peer transitions still receive the exact unchanged-before archive.
pub(crate) fn converted(
    candidate: &Program,
    before: &Program,
) -> Result<Option<Arc<Transition>>, String> {
    validate_conversion_input(before)?;
    let Some(previous) = &before.resource_source.native_suites else {
        return Ok(None);
    };
    if candidate.resource_source != before.resource_source {
        return Err("native suite conversion replaced its unchanged-before archive".into());
    }
    Ok(Some(Arc::new(transition(
        previous.origins.clone(),
        &candidate.functions,
    )?)))
}

pub(crate) fn callable_sealing(
    program: &Program,
    before: &Program,
    edits: &[(crate::ReflectedCallablePlan, crate::ReflectedCallablePlan)],
) -> Result<Option<Arc<Transition>>, Vec<crate::LowerError>> {
    let result = (|| -> Result<Option<Arc<Transition>>, String> {
        validate_conversion_input(before)?;
        if before.resource_source.native_suites.is_none() {
            return Ok(None);
        }
        for checked in [before, program] {
            crate::mutable_sum_backings::validate_program(checked)
                .and_then(|()| crate::actor_handlers::validate_program(checked))
                .map_err(|errors| format!("native suite independent receipts: {errors:?}"))?;
        }
        let mut replay = before.clone();
        for owner in &mut replay.functions {
            crate::resource_materialization::walk_block(&mut owner.body, &mut |expression| {
                let replace = |plan: &mut crate::ReflectedCallablePlan| {
                    if let Some((_, current)) = edits.iter().find(|(previous, _)| previous == plan)
                    {
                        *plan = current.clone();
                    }
                };
                match &mut expression.kind {
                    ExpressionKind::Intrinsic {
                        field_validation: Some(crate::ReflectedFieldValidation::Validate(fields)),
                        ..
                    } => {
                        for field in fields {
                            if let crate::ReflectedFieldAction::CallableReturn(plan) =
                                &mut field.action
                            {
                                replace(plan);
                            }
                        }
                    }
                    ExpressionKind::Handle {
                        kind:
                            crate::HandleKind::ReflectedReturn {
                                callable: Some(plan),
                                ..
                            },
                        ..
                    } => replace(plan),
                    _ => {}
                }
                Ok(())
            })?;
        }
        let semantic = |program: &Program| {
            let mut functions = program.functions.clone();
            for function in &mut functions {
                // These three peer inventories passed their own complete check
                // above. Every actual operation and temporary marker stays exact.
                function.mutable_sum_backings.clear();
                function.actor_handler = None;
                function.actor_state_views.clear();
            }
            functions
        };
        if !crate::resource_materialization::functions_equal(&semantic(program), &semantic(&replay))
        {
            return Err("native suite callable sealing changed outside exact token replay".into());
        }
        converted(program, before)
    })();
    result.map_err(|message| {
        vec![crate::LowerError {
            span: before
                .functions
                .first()
                .map_or(jett_common::Span::new(FileId::new(0), 0, 0), |function| {
                    function.span
                }),
            message,
        }]
    })
}

#[cfg(test)]
#[path = "native_suite_transition_tests.rs"]
mod tests;
