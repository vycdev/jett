//! Finite Source-only baseline data shared by reference and object probes.
//! No native status, channel, message or exit assertion is admitted here.
#![allow(dead_code)] // Each independent harness consumes its own phases.

use jett_common::{FileId, SourceOrigin, Span};
use jett_parser::ast::Item;
use jett_resolve::ResourceKernelSpec;
use jett_typecheck::{CheckOptions, CheckedResourceProgram, ResourceProgramError};
use jett_types::ResourceKernelRecipe;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

pub const SUPPORT: &str = include_str!("resource/resource_probe.jett");
pub const ORIGINAL: &str = include_str!("resource_absent_aggregate/original.jett");
pub const LIST_LENGTH: &str = include_str!("resource_absent_aggregate/list_length.jett");
pub const MAP_LENGTH: &str = include_str!("resource_absent_aggregate/map_length.jett");

#[derive(Clone, Copy, Debug)]
pub enum ReferenceExpectation {
    Nothing,
    Error(&'static str),
}

pub struct Input {
    pub name: &'static str,
    pub source: &'static str,
    pub reference: ReferenceExpectation,
}

macro_rules! clean {
    ($name:literal, $path:literal) => {
        Input {
            name: $name,
            source: include_str!($path),
            reference: ReferenceExpectation::Nothing,
        }
    };
}

pub const INPUTS: &[Input] = &[
    clean!(
        "01_empty_tokens",
        "resource_absent_aggregate/01_empty_tokens.jett"
    ),
    clean!(
        "02_absent_tokens",
        "resource_absent_aggregate/02_absent_tokens.jett"
    ),
    clean!(
        "03_empty_map",
        "resource_absent_aggregate/03_empty_map.jett"
    ),
    clean!(
        "04_absent_map",
        "resource_absent_aggregate/04_absent_map.jett"
    ),
    clean!(
        "05_empty_result_tokens",
        "resource_absent_aggregate/05_empty_result_tokens.jett"
    ),
    clean!(
        "06_failed_result_tokens",
        "resource_absent_aggregate/06_failed_result_tokens.jett"
    ),
    clean!(
        "07_absent_envelope",
        "resource_absent_aggregate/07_absent_envelope.jett"
    ),
    clean!(
        "08_absent_box",
        "resource_absent_aggregate/08_absent_box.jett"
    ),
    clean!(
        "09_empty_choice",
        "resource_absent_aggregate/09_empty_choice.jett"
    ),
    clean!(
        "10_absent_choice",
        "resource_absent_aggregate/10_absent_choice.jett"
    ),
    clean!(
        "11_absent_state",
        "resource_absent_aggregate/11_absent_state.jett"
    ),
    clean!(
        "12_absent_exact_state",
        "resource_absent_aggregate/12_absent_exact_state.jett"
    ),
    Input {
        name: "13_wrong_empty_length_control",
        source: include_str!("resource_absent_aggregate/13_wrong_empty_length_control.jett"),
        reference: ReferenceExpectation::Error("list.__remove_at: index -1 out of bounds"),
    },
    clean!(
        "14_required_primitive_controls",
        "resource_absent_aggregate/14_required_primitive_controls.jett"
    ),
];

#[derive(Clone, Copy, Debug)]
pub enum Phase {
    Unstarted,
    SourceIdentity,
    Parsing,
    Catalog,
    CheckedPrepare,
    Resolution,
    Checking,
    ResolveMetadata,
    CheckMetadata,
    RequiredCapture,
    RequiredCache,
    ReferenceSetup,
    ReferenceEntryIdentity,
    ReferenceCall,
    ReferenceObservations,
    WorkerEligibility,
    OriginalConstructorCall,
    VerifyPropertyWorkers,
    HirLowering,
    ValueBaking,
    HirEntryIdentity,
    ValueConversions,
    MirLowering,
    MirValidation,
    ObjectEmission,
    ObjectObservations,
}

#[derive(Clone, Copy, Debug)]
pub enum FailureKind {
    Returned,
    UnexpectedPanic,
}

#[derive(Debug)]
pub struct Failure {
    pub phase: Phase,
    pub kind: FailureKind,
    pub detail: String,
}

pub struct Attempt<T> {
    pub entered: Vec<Phase>,
    pub outcome: Result<T, Failure>,
}

pub fn enter(entered: &mut Vec<Phase>, phase: Phase) {
    entered.push(phase);
}

pub fn failure(phase: Phase, detail: impl std::fmt::Debug) -> Failure {
    Failure {
        phase,
        kind: FailureKind::Returned,
        detail: format!("{detail:?}"),
    }
}

pub fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_string())
        })
        .unwrap_or_else(|| "non-string panic payload".to_string())
}

pub fn attempt<T>(run: impl FnOnce(&mut Vec<Phase>) -> Result<T, Failure>) -> Attempt<T> {
    let mut entered = vec![Phase::Unstarted];
    let outcome = match catch_unwind(AssertUnwindSafe(|| run(&mut entered))) {
        Ok(outcome) => outcome,
        Err(payload) => Err(Failure {
            phase: *entered.last().unwrap(),
            kind: FailureKind::UnexpectedPanic,
            detail: panic_message(payload.as_ref()),
        }),
    };
    Attempt { entered, outcome }
}

fn checked_failure(error: ResourceProgramError) -> Failure {
    let phase = match &error {
        ResourceProgramError::ParseDiagnostics(_) => Phase::Parsing,
        ResourceProgramError::ResolveDiagnostics(_) => Phase::Resolution,
        ResourceProgramError::CheckDiagnostics(_) => Phase::Checking,
        ResourceProgramError::ResolveMetadata { .. } => Phase::ResolveMetadata,
        ResourceProgramError::CheckMetadata { .. } => Phase::CheckMetadata,
    };
    // Display alone omits ordinary diagnostic messages; retain full spans/codes.
    failure(phase, (&error, error.diagnostics()))
}

pub fn prepare(
    input: &Input,
    release: bool,
    entered: &mut Vec<Phase>,
) -> Result<(Arc<CheckedResourceProgram>, Span), Failure> {
    enter(entered, Phase::SourceIdentity);
    if !input.source.starts_with(ORIGINAL) {
        return Err(failure(
            Phase::SourceIdentity,
            "whole original Source17 prefix changed",
        ));
    }
    let primary = FileId::new(0);
    let support = FileId::new(10_000);
    let list = FileId::new(10_001);
    let map = FileId::new(10_002);
    enter(entered, Phase::Parsing);
    let mut parsed = jett_parser::parse(SUPPORT, support);
    let resource_spans = parsed
        .module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Resource(resource) => Some(resource.name.span),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut entries = Vec::new();
    for (source, file) in [
        (LIST_LENGTH, list),
        (MAP_LENGTH, map),
        (input.source, primary),
    ] {
        let fragment = jett_parser::parse(source, file);
        if file == primary {
            entries.extend(fragment.module.items.iter().filter_map(|item| match item {
                Item::Function(function)
                    if function.name.name == "main" && function.name.span.file == primary =>
                {
                    Some(function.span)
                }
                _ => None,
            }));
        }
        parsed.module.items.extend(fragment.module.items);
        parsed.errors.extend(fragment.errors);
    }
    if !parsed.errors.is_empty() {
        return Err(failure(Phase::Parsing, &parsed.errors));
    }
    enter(entered, Phase::Catalog);
    let [resource] = resource_spans.as_slice() else {
        return Err(failure(
            Phase::Catalog,
            "sole original Support Resource declaration missing",
        ));
    };
    let catalog = [
        ("kernel_create", ResourceKernelRecipe::NetworkFactory),
        ("kernel_borrow", ResourceKernelRecipe::NetworkBorrow),
        ("kernel_close", ResourceKernelRecipe::Finalize),
    ]
    .into_iter()
    .map(|(member, recipe)| ResourceKernelSpec {
        resource_declaration: *resource,
        member: member.into(),
        recipe,
    })
    .collect::<Vec<_>>();
    let [entry] = entries.as_slice() else {
        return Err(failure(
            Phase::Catalog,
            "primary original main declaration is not unique",
        ));
    };
    enter(entered, Phase::CheckedPrepare);
    let checked = CheckedResourceProgram::prepare(
        parsed,
        HashMap::from([
            (primary, SourceOrigin::Project),
            (support, SourceOrigin::Stdlib),
            (list, SourceOrigin::Stdlib),
            (map, SourceOrigin::Stdlib),
        ]),
        &catalog,
        CheckOptions { release },
    )
    .map_err(checked_failure)?;
    Ok((Arc::new(checked), *entry))
}
