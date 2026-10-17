use super::{CheckOptions, CheckResult, check_with_options};
use jett_common::{FileId, STDLIB_FILE_ID_START};
use jett_diagnostics::Severity;

fn checked(source: &str, release: bool) -> CheckResult {
    let parsed = jett_parser::parse(source, FileId::new(STDLIB_FILE_ID_START));
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    assert!(
        resolved
            .diagnostics
            .iter()
            .all(|error| error.severity != Severity::Error),
        "{:?}",
        resolved.diagnostics,
    );
    check_with_options(&parsed.module, &resolved, CheckOptions { release })
}

#[test]
fn list_sum_kernel_accepts_all_numeric_primitives_and_transparent_alias_chains() {
    for primitive in [
        "int8", "int16", "int32", "int64", "uint8", "uint16", "uint32", "uint64", "float32",
        "float64",
    ] {
        for element in [primitive, "Number", "Alias"] {
            let source = format!(
                "namespace app\ntype Number = {primitive}\ntype Alias = Number\nfunction total(view values: list[{element}]) returns {element}:\n    return list.__sum[{element}](view values)\n"
            );
            for release in [false, true] {
                let checked = checked(&source, release);
                assert!(
                    checked
                        .diagnostics
                        .iter()
                        .all(|error| error.severity != Severity::Error),
                    "{primitive}/{element}/{release}: {:?}",
                    checked.diagnostics,
                );
            }
        }
    }
}

#[test]
fn list_sum_kernel_rejects_nonnumeric_refinement_and_secret_elements() {
    let declarations = "namespace app\ntype Positive = int64 where value > 0\ntype PositiveAlias = Positive\ntype Hidden = secret[int64]\nstruct Record:\n    value: int64\nenum Choice:\n    empty\ninterface Named:\n    function name(view self: Named) returns string\n";
    for element in [
        "bool",
        "string",
        "nothing",
        "bytes",
        "list[int64]",
        "map[int64, int64]",
        "set[int64]",
        "optional[int64]",
        "result[int64, string]",
        "Positive",
        "PositiveAlias",
        "secret[int64]",
        "secret[secret[int64]]",
        "Hidden",
        "secret[Positive]",
        "Record",
        "Choice",
        "Named",
        "function() returns int64",
    ] {
        let source = format!(
            "{declarations}function total(view values: list[{element}]) returns {element}:\n    return list.__sum[{element}](view values)\n"
        );
        for release in [false, true] {
            let checked = checked(&source, release);
            let errors = checked
                .diagnostics
                .iter()
                .filter(|error| error.severity == Severity::Error)
                .collect::<Vec<_>>();
            assert_eq!(errors.len(), 1, "{element}/{release}: {errors:?}");
            assert_eq!(errors[0].code.code(), 300, "{element}: {errors:?}");
            assert!(errors[0].message.contains("numeric primitive for list.sum"));
        }
    }
}

#[test]
fn list_sum_kernel_requires_its_selected_type_instead_of_a_wildcard() {
    for invocation in [
        "list.__sum(view values)",
        "list.__sum[int64, bool](view values)",
    ] {
        let source = format!(
            "namespace app\nfunction total(view values: list[bool]) returns bool:\n    return {invocation}\n"
        );
        for release in [false, true] {
            let checked = checked(&source, release);
            let errors = checked
                .diagnostics
                .iter()
                .filter(|error| error.severity == Severity::Error)
                .collect::<Vec<_>>();
            assert_eq!(errors.len(), 1, "{invocation}/{release}: {errors:?}");
            assert_eq!(errors[0].code.code(), 309, "{invocation}: {errors:?}");
            assert!(errors[0].message.contains("expected 1 type argument"));
        }
    }
}
