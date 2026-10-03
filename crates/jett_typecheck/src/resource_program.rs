//! Retain one internally derived compiler session before live Resource integration.

use std::collections::HashMap;
use std::fmt;

use jett_common::{FileId, SourceOrigin};
use jett_diagnostics::{Diagnostic, Severity};
use jett_parser::{ParseResult, ast::Module};
use jett_resolve::{
    ResolveResult, ResourceKernelError, ResourceKernelSpec, resolve_with_resource_kernels,
};

use crate::{
    CheckOptions, CheckResult, ResourceHookError, check_with_resource_kernels,
    validate_resource_hooks,
};

/// One owned parsed program and the exact resolution/checking session derived from it.
///
/// Immutable observations do not grant runtime provider authority. There is no
/// constructor from independent checked parts, mutation or ownership extraction.
#[derive(Debug)]
pub struct CheckedResourceProgram {
    module: Module,
    resolved: ResolveResult,
    checked: CheckResult,
    source_origins: HashMap<FileId, SourceOrigin>,
    parse_diagnostics: Vec<Diagnostic>,
}

/// A rejected source phase or invalid compiler-owned Resource association.
#[derive(Debug)]
pub enum ResourceProgramError {
    ParseDiagnostics(Vec<Diagnostic>),
    ResolveDiagnostics(Vec<Diagnostic>),
    CheckDiagnostics(Vec<Diagnostic>),
    ResolveMetadata {
        source: ResourceKernelError,
        diagnostics: Vec<Diagnostic>,
    },
    CheckMetadata {
        source: ResourceHookError,
        diagnostics: Vec<Diagnostic>,
    },
}

impl ResourceProgramError {
    /// Observations through the failed phase, in original source-phase order.
    pub fn diagnostics(&self) -> Option<&[Diagnostic]> {
        match self {
            Self::ParseDiagnostics(diagnostics)
            | Self::ResolveDiagnostics(diagnostics)
            | Self::CheckDiagnostics(diagnostics)
            | Self::ResolveMetadata { diagnostics, .. }
            | Self::CheckMetadata { diagnostics, .. } => Some(diagnostics),
        }
    }
}

impl fmt::Display for ResourceProgramError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseDiagnostics(_) => formatter.write_str("resource program has parser errors"),
            Self::ResolveDiagnostics(_) => {
                formatter.write_str("resource program has resolver errors")
            }
            Self::CheckDiagnostics(_) => formatter.write_str("resource program has checker errors"),
            Self::ResolveMetadata { source, .. } => write!(formatter, "{source}"),
            Self::CheckMetadata { source, .. } => write!(formatter, "{source}"),
        }
    }
}

impl std::error::Error for ResourceProgramError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ResolveMetadata { source, .. } => Some(source),
            Self::CheckMetadata { source, .. } => Some(source),
            Self::ParseDiagnostics(_) | Self::ResolveDiagnostics(_) | Self::CheckDiagnostics(_) => {
                None
            }
        }
    }
}

impl CheckedResourceProgram {
    /// Resolve and check this owned parse with explicit trusted compiler inputs.
    /// Ordinary production callers supply an empty kernel catalog.
    pub fn prepare(
        parsed: ParseResult,
        source_origins: HashMap<FileId, SourceOrigin>,
        kernels: &[ResourceKernelSpec],
        options: CheckOptions,
    ) -> Result<Self, ResourceProgramError> {
        let ParseResult {
            module,
            errors: parse_diagnostics,
        } = parsed;
        if has_errors(&parse_diagnostics) {
            return Err(ResourceProgramError::ParseDiagnostics(parse_diagnostics));
        }
        let resolved =
            resolve_with_resource_kernels(&module, &source_origins, kernels).map_err(|source| {
                ResourceProgramError::ResolveMetadata {
                    source,
                    diagnostics: observed_diagnostics(&parse_diagnostics, &[], &[]),
                }
            })?;
        if has_errors(&resolved.diagnostics) {
            return Err(ResourceProgramError::ResolveDiagnostics(
                observed_diagnostics(&parse_diagnostics, &resolved.diagnostics, &[]),
            ));
        }
        let checked =
            check_with_resource_kernels(&module, &resolved, options).map_err(|source| {
                ResourceProgramError::CheckMetadata {
                    source,
                    diagnostics: observed_diagnostics(
                        &parse_diagnostics,
                        &resolved.diagnostics,
                        &[],
                    ),
                }
            })?;
        if has_errors(&checked.diagnostics) {
            return Err(ResourceProgramError::CheckDiagnostics(
                observed_diagnostics(
                    &parse_diagnostics,
                    &resolved.diagnostics,
                    &checked.diagnostics,
                ),
            ));
        }
        validate_resource_hooks(&module, &resolved, &checked).map_err(|source| {
            ResourceProgramError::CheckMetadata {
                source,
                diagnostics: observed_diagnostics(
                    &parse_diagnostics,
                    &resolved.diagnostics,
                    &checked.diagnostics,
                ),
            }
        })?;
        Ok(Self {
            module,
            resolved,
            checked,
            source_origins,
            parse_diagnostics,
        })
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn resolved(&self) -> &ResolveResult {
        &self.resolved
    }

    pub fn checked(&self) -> &CheckResult {
        &self.checked
    }

    pub fn source_origins(&self) -> &HashMap<FileId, SourceOrigin> {
        &self.source_origins
    }

    pub fn parse_diagnostics(&self) -> &[Diagnostic] {
        &self.parse_diagnostics
    }
}

fn observed_diagnostics(
    parsed: &[Diagnostic],
    resolved: &[Diagnostic],
    checked: &[Diagnostic],
) -> Vec<Diagnostic> {
    parsed
        .iter()
        .chain(resolved)
        .chain(checked)
        .cloned()
        .collect()
}

fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jett_common::{STDLIB_FILE_ID_START, Span};
    use jett_parser::{ast::Item, parse};
    use jett_types::{ResourceKernelRecipe, Type, TypeInterner};

    const SOURCE: &str = "namespace resource_probe\nresource TestHandle\ntype HandleAlias = TestHandle\nfunction checked_calls(view net: Network) returns nothing:\n    TestHandle first = kernel_create(view net, 1) handle error:\n        return nothing\n    int64 observed = kernel_borrow(view net, view first) handle error:\n        return nothing\n    kernel_close(first)\n    return nothing\n";

    fn inputs(
        source: &str,
    ) -> (
        ParseResult,
        HashMap<FileId, SourceOrigin>,
        Vec<ResourceKernelSpec>,
    ) {
        let file = FileId::new(STDLIB_FILE_ID_START);
        let parsed = parse(source, file);
        assert!(!has_errors(&parsed.errors), "{:?}", parsed.errors);
        let span = parsed
            .module
            .items
            .iter()
            .find_map(|item| {
                if let Item::Resource(resource) = item {
                    Some(resource.name.span)
                } else {
                    None
                }
            })
            .expect("source Resource declaration");
        let kernels = [
            ("kernel_create", ResourceKernelRecipe::NetworkFactory),
            ("kernel_borrow", ResourceKernelRecipe::NetworkBorrow),
            ("kernel_close", ResourceKernelRecipe::Finalize),
        ]
        .into_iter()
        .map(|(member, recipe)| ResourceKernelSpec {
            resource_declaration: span,
            member: member.into(),
            recipe,
        })
        .collect();
        (
            parsed,
            HashMap::from([(file, SourceOrigin::Stdlib)]),
            kernels,
        )
    }

    #[test]
    fn retained_resource_program_preserves_exact_private_calls_and_modes_in_both_profiles() {
        for release in [false, true] {
            let (parsed, origins, kernels) = inputs(SOURCE);
            let program = CheckedResourceProgram::prepare(
                parsed,
                origins,
                &kernels,
                CheckOptions { release },
            )
            .unwrap();
            assert_eq!(program.checked().release, release);
            assert_eq!(program.checked().resource_hooks.len(), 3);
            validate_resource_hooks(program.module(), program.resolved(), program.checked())
                .unwrap();
            let declaration = program
                .module()
                .items
                .iter()
                .find_map(|item| {
                    if let Item::Resource(resource) = item {
                        Some(resource)
                    } else {
                        None
                    }
                })
                .unwrap();
            let definition = program.resolved().resolutions[&declaration.name.span];
            let resource_type = program.checked().definition_types[&definition];
            assert!(
                matches!(program.checked().interner.resolve(resource_type), Type::Resource(name) if name == "resource_probe.TestHandle")
            );
            for hook in program.checked().resource_hooks.values() {
                assert_eq!(hook.resource_definition, definition);
                assert_eq!(hook.resource_type, resource_type);
                assert_eq!(
                    program.checked().definition_types[&hook.definition],
                    hook.function_type
                );
            }
        }
    }

    #[test]
    fn retained_resource_program_rejects_each_source_error_phase_with_original_diagnostics() {
        let file = FileId::new(0);
        let malformed = parse("function broken(\n", file);
        let original = malformed
            .errors
            .iter()
            .find(|diagnostic| diagnostic.severity == Severity::Error)
            .unwrap()
            .clone();
        let error = CheckedResourceProgram::prepare(
            malformed,
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            CheckOptions::default(),
        )
        .unwrap_err();
        assert!(matches!(&error, ResourceProgramError::ParseDiagnostics(_)));
        assert!(
            error
                .diagnostics()
                .unwrap()
                .iter()
                .any(|diagnostic| diagnostic.code == original.code
                    && diagnostic.span == original.span
                    && diagnostic.message == original.message)
        );
        for (source, phase) in [
            (
                "function inspect() returns nothing:\n    missing()\n    return nothing\n",
                "resolve",
            ),
            (
                "function inspect() returns nothing:\n    int64 broken = true\n    return nothing\n",
                "check",
            ),
        ] {
            let parsed = parse(source, file);
            assert!(!has_errors(&parsed.errors));
            let error = CheckedResourceProgram::prepare(
                parsed,
                HashMap::from([(file, SourceOrigin::Project)]),
                &[],
                CheckOptions::default(),
            )
            .unwrap_err();
            assert!(
                match (&error, phase) {
                    (ResourceProgramError::ResolveDiagnostics(_), "resolve") => true,
                    (ResourceProgramError::CheckDiagnostics(_), "check") => true,
                    _ => false,
                },
                "{error:?}"
            );
            assert!(has_errors(error.diagnostics().unwrap()));
            if phase == "check" {
                assert!(
                    error
                        .diagnostics()
                        .unwrap()
                        .iter()
                        .any(|diagnostic| diagnostic.code.code() == 311)
                );
            }
        }
    }

    #[test]
    fn retained_resource_program_keeps_warnings_and_empty_catalog_without_hook_authority() {
        let file = FileId::new(0);
        let mut parsed = parse(
            "function kernel_close(value: int64) returns int64:\n    return value\n",
            file,
        );
        let warning = Diagnostic::warning(1, "retained parser observation", Span::new(file, 0, 8));
        parsed.errors.push(warning.clone());
        let program = CheckedResourceProgram::prepare(
            parsed,
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            CheckOptions::default(),
        )
        .unwrap();
        assert_eq!(program.parse_diagnostics().len(), 1);
        assert_eq!(program.parse_diagnostics()[0].span, warning.span);
        assert_eq!(program.parse_diagnostics()[0].message, warning.message);
        assert_eq!(program.parse_diagnostics()[0].severity, Severity::Warning);
        assert!(program.resolved().resource_kernels.is_empty());
        assert!(program.checked().resource_hooks.is_empty());
        assert_eq!(
            program.source_origins().get(&file),
            Some(&SourceOrigin::Project)
        );
    }

    #[test]
    fn retained_resource_program_keeps_release_policy_at_preparation() {
        let file = FileId::new(0);
        let source =
            "function main() returns nothing:\n    println(\"visible\")\n    return nothing\n";
        let debug = CheckedResourceProgram::prepare(
            parse(source, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            CheckOptions::default(),
        )
        .unwrap();
        assert!(!debug.checked().release);
        let error = CheckedResourceProgram::prepare(
            parse(source, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            CheckOptions { release: true },
        )
        .unwrap_err();
        assert!(matches!(&error, ResourceProgramError::CheckDiagnostics(_)));
        assert!(
            error
                .diagnostics()
                .unwrap()
                .iter()
                .any(|diagnostic| diagnostic.code.code() == 362)
        );
    }

    #[test]
    fn retained_resource_program_validates_unused_catalog_and_rejects_untrusted_origin() {
        let source = "namespace resource_probe\nresource TestHandle\n";
        let (parsed, origins, kernels) = inputs(source);
        let program =
            CheckedResourceProgram::prepare(parsed, origins, &kernels, CheckOptions::default())
                .unwrap();
        assert_eq!(
            program.checked().resource_hooks.len(),
            3,
            "unused hooks remain fully checked"
        );
        let (parsed, mut origins, kernels) = inputs(source);
        origins.insert(FileId::new(STDLIB_FILE_ID_START), SourceOrigin::Project);
        let error =
            CheckedResourceProgram::prepare(parsed, origins, &kernels, CheckOptions::default())
                .unwrap_err();
        assert!(matches!(
            error,
            ResourceProgramError::ResolveMetadata {
                source: ResourceKernelError::UntrustedOrigin(_),
                ..
            }
        ));
        let (parsed, origins, mut kernels) = inputs(source);
        kernels.push(kernels[0].clone());
        let error =
            CheckedResourceProgram::prepare(parsed, origins, &kernels, CheckOptions::default())
                .unwrap_err();
        assert!(matches!(
            error,
            ResourceProgramError::ResolveMetadata {
                source: ResourceKernelError::DuplicateCatalogEntry(_),
                ..
            }
        ));
    }

    #[test]
    fn retained_resource_program_preserves_generic_scoped_and_named_argument_facts() {
        let file = FileId::new(0);
        let source = r#"struct User:
    name: string
    age: int64
function inspect[T](view value: T) returns nothing:
    for field in type.fields[T]():
        comptime type Field = field.type_info:
            string reflected_name = type.name[Field]()
    return nothing
function order(first: int64, second: int64) returns int64:
    return first - second
function main() returns int64:
    User user = User(name: "Ada", age: 42)
    inspect[User](view user)
    return order(second: 2, first: 1)
"#;
        let program = CheckedResourceProgram::prepare(
            parse(source, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            CheckOptions::default(),
        )
        .unwrap();
        let body = program
            .checked()
            .generic_function_instantiations
            .iter()
            .find(|body| !body.comptime_type_bindings.is_empty())
            .expect("retained reflected generic body");
        let bindings = body.comptime_type_bindings.values().next().unwrap();
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].bound_type, TypeInterner::STRING);
        assert_eq!(bindings[1].bound_type, TypeInterner::INT64);
        assert!(
            program
                .checked()
                .call_argument_orders
                .values()
                .any(|order| order.source_indices == vec![1, 0])
        );
        assert!(!program.checked().type_map.is_empty());
        assert!(!body.type_map.is_empty());
    }

    #[test]
    fn retained_resource_program_is_independent_of_mutated_caller_input_copies() {
        let (mut parsed, mut origins, mut kernels) = inputs(SOURCE);
        let owned_copy = ParseResult {
            module: parsed.module.clone(),
            errors: parsed.errors.clone(),
        };
        let program = CheckedResourceProgram::prepare(
            owned_copy,
            origins.clone(),
            &kernels,
            CheckOptions::default(),
        )
        .unwrap();
        parsed.module.items.clear();
        parsed.errors.push(Diagnostic::error(
            1,
            "later caller mutation",
            Span::new(FileId::new(0), 0, 0),
        ));
        origins.clear();
        kernels[0].member = "different_member".into();
        kernels.clear();
        assert!(!program.module().items.is_empty());
        assert!(program.parse_diagnostics().is_empty());
        assert_eq!(
            program
                .source_origins()
                .get(&FileId::new(STDLIB_FILE_ID_START)),
            Some(&SourceOrigin::Stdlib)
        );
        assert_eq!(program.checked().resource_hooks.len(), 3);
        validate_resource_hooks(program.module(), program.resolved(), program.checked()).unwrap();
    }

    #[test]
    fn retained_resource_program_keeps_prior_warnings_before_source_failures() {
        let file = FileId::new(0);
        let warning = Diagnostic::warning(1, "injected parser observation", Span::new(file, 0, 8));
        for (source, check_failure) in [
            (
                "function inspect() returns nothing:\n    missing()\n    return nothing\n",
                false,
            ),
            (
                "function inspect() returns nothing:\n    int64 retained_warning = 1\n    int64 _broken = true\n    return nothing\n",
                true,
            ),
        ] {
            let mut parsed = parse(source, file);
            assert!(!has_errors(&parsed.errors), "{:?}", parsed.errors);
            parsed.errors.push(warning.clone());
            // These observations come from a real source resolution, not a fabricated warning.
            let expected_resolved = resolve_with_resource_kernels(
                &parsed.module,
                &HashMap::from([(file, SourceOrigin::Project)]),
                &[],
            )
            .unwrap();
            let error = CheckedResourceProgram::prepare(
                parsed,
                HashMap::from([(file, SourceOrigin::Project)]),
                &[],
                CheckOptions::default(),
            )
            .unwrap_err();
            let diagnostics = error.diagnostics().unwrap();
            assert_eq!(diagnostics[0].code, warning.code);
            assert_eq!(diagnostics[0].span, warning.span);
            assert_eq!(diagnostics[0].message, warning.message);
            for (actual, expected) in diagnostics[1..].iter().zip(&expected_resolved.diagnostics) {
                assert_eq!(actual.code, expected.code);
                assert_eq!(actual.span, expected.span);
                assert_eq!(actual.message, expected.message);
                assert_eq!(actual.severity, expected.severity);
            }
            if check_failure {
                assert!(matches!(&error, ResourceProgramError::CheckDiagnostics(_)));
                assert_eq!(expected_resolved.diagnostics.len(), 1);
                assert_eq!(diagnostics[1].code.code(), 202);
                assert_eq!(
                    diagnostics[1].message,
                    "unused variable: `retained_warning`"
                );
                assert_eq!(diagnostics.len(), 3);
                assert_eq!(diagnostics[2].code.code(), 311);
            } else {
                assert!(matches!(
                    &error,
                    ResourceProgramError::ResolveDiagnostics(_)
                ));
                assert_eq!(diagnostics.len(), 1 + expected_resolved.diagnostics.len());
            }
        }
    }

    #[test]
    fn retained_resource_program_preserves_observations_and_typed_metadata_causes() {
        use std::error::Error;
        let source = "namespace resource_probe\nresource TestHandle\n";
        for with_warning in [false, true] {
            let (mut parsed, mut origins, kernels) = inputs(source);
            let file = FileId::new(STDLIB_FILE_ID_START);
            let warning =
                Diagnostic::warning(1, "injected parser observation", Span::new(file, 0, 8));
            if with_warning {
                parsed.errors.push(warning.clone());
            }
            origins.insert(file, SourceOrigin::Project);
            let error =
                CheckedResourceProgram::prepare(parsed, origins, &kernels, CheckOptions::default())
                    .unwrap_err();
            let diagnostics = error
                .diagnostics()
                .expect("metadata failures also retain observations");
            assert_eq!(diagnostics.len(), usize::from(with_warning));
            if with_warning {
                assert_eq!(diagnostics[0].code, warning.code);
                assert_eq!(diagnostics[0].span, warning.span);
                assert_eq!(diagnostics[0].message, warning.message);
            }
            let ResourceProgramError::ResolveMetadata { source, .. } = &error else {
                panic!("wrong metadata phase: {error:?}");
            };
            assert_eq!(error.to_string(), source.to_string());
            assert_eq!(error.source().unwrap().to_string(), source.to_string());
        }
    }

    #[test]
    fn retained_resource_program_keeps_real_resolver_warnings_on_success() {
        let file = FileId::new(0);
        let mut parsed = parse(
            "function inspect() returns nothing:\n    int64 retained_warning = 1\n    return nothing\n",
            file,
        );
        let warning = Diagnostic::warning(1, "injected parser observation", Span::new(file, 0, 8));
        parsed.errors.push(warning.clone());
        let program = CheckedResourceProgram::prepare(
            parsed,
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            CheckOptions::default(),
        )
        .unwrap();
        assert_eq!(program.parse_diagnostics().len(), 1);
        assert_eq!(program.parse_diagnostics()[0].message, warning.message);
        assert_eq!(program.resolved().diagnostics.len(), 1);
        assert_eq!(program.resolved().diagnostics[0].code.code(), 202);
        assert_eq!(
            program.resolved().diagnostics[0].message,
            "unused variable: `retained_warning`"
        );
        assert!(program.checked().diagnostics.is_empty());
    }
}
