use super::{CheckOptions, CheckResult, check_with_options};
use jett_common::{FileId, STDLIB_FILE_ID_START};
use jett_diagnostics::Severity;
use jett_parser::{
    ast::{Item, Module},
    parse,
};
use jett_resolve::resolve;
use jett_types::TypeInterner;

const PREFIX: &str = "namespace resource_copy\nexport resource Handle\ntype HandleAlias = Handle\nstruct Holder:\n    token: Handle\n    count: int64\nstruct Envelope:\n    child: Holder\n    label: string\nstruct Slot[T]:\n    item: T\n    count: int64\nstruct Phantom[T]:\n    count: int64\n";

fn parsed(source: &str, file: FileId) -> Module {
    let parsed = parse(source, file);
    assert!(
        !parsed
            .errors
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error),
        "parse errors: {:?}",
        parsed.errors,
    );
    parsed.module
}

fn checked_module(module: &Module, release: bool) -> CheckResult {
    let resolved = resolve(module);
    assert!(
        !resolved
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error),
        "resolution errors: {:?}",
        resolved.diagnostics,
    );
    assert!(resolved.resource_kernels.is_empty());
    let checked = check_with_options(module, &resolved, CheckOptions { release });
    assert!(checked.resource_hooks.is_empty());
    checked
}

fn checked_source(body: &str, release: bool) -> CheckResult {
    let source = format!("{PREFIX}{body}");
    let module = parsed(&source, FileId::new(STDLIB_FILE_ID_START));
    checked_module(&module, release)
}

fn assert_codes(checked: &CheckResult, expected: &[u16]) {
    let mut actual = checked
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| diagnostic.code.code())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected, "diagnostics: {:?}", checked.diagnostics);
}

#[test]
fn resource_copy_owned_field_acquisition_rejects_borrowed_and_retained_owned_parents() {
    let bodies = [
        "function copy_from_view(view holder: Holder) returns Handle:\n    return holder.token\n",
        "function receive_both(token: Handle, holder: Holder) returns nothing:\n    return nothing\nfunction duplicate_parent(holder: Holder) returns nothing:\n    Handle copied = holder.token\n    receive_both(copied, holder)\n    return nothing\n",
        "function alias_copy(view holder: Holder) returns HandleAlias:\n    return holder.token\n",
    ];
    for release in [false, true] {
        for body in bodies {
            assert_codes(&checked_source(body, release), &[364]);
        }
    }
}

#[test]
fn resource_copy_nested_projection_checks_only_the_terminal_endpoint() {
    let denied = [
        "function copy_nested(view envelope: Envelope) returns Handle:\n    return envelope.child.token\n",
        "function copy_aggregate(view envelope: Envelope) returns Holder:\n    return envelope.child\n",
        "function copy_optional(view slot: Slot[optional[Handle]]) returns optional[Handle]:\n    return slot.item\n",
    ];
    let allowed = "function inspect(view token: Handle) returns nothing:\n    return nothing\nfunction count(view envelope: Envelope) returns int64:\n    return envelope.child.count\nfunction label(view envelope: Envelope) returns string:\n    return envelope.label\nfunction observe(view envelope: Envelope) returns nothing:\n    Holder child = view envelope.child\n    Handle token = view child.token\n    Handle forwarded = token\n    inspect(view forwarded)\n    return nothing\n";
    for release in [false, true] {
        for body in denied {
            assert_codes(&checked_source(body, release), &[364]);
        }
        let checked = checked_source(allowed, release);
        assert_codes(&checked, &[]);
        assert!(
            checked
                .binding_modes
                .values()
                .any(|mode| matches!(mode, super::CheckedBindingMode::View { .. }))
        );
    }
}

#[test]
fn resource_copy_explicit_views_whole_moves_and_assignment_addresses_stay_valid() {
    let body = "function inspect(view token: Handle) returns nothing:\n    return nothing\nfunction move_handle(token: Handle) returns Handle:\n    return token\nfunction move_holder(holder: Holder) returns Holder:\n    return holder\nfunction owned_payload(token: Handle) returns Holder:\n    return Holder(token: token, count: 1)\nfunction borrow(view holder: Holder) returns nothing:\n    Handle alias = view (holder.token)\n    Handle forwarded = alias\n    inspect(view forwarded)\n    return nothing\nfunction replace(holder: Holder, token: Handle) returns Holder:\n    mutable Holder changed = holder\n    changed.token = token\n    return changed\nfunction update_count(envelope: Envelope) returns Envelope:\n    mutable Envelope changed = envelope\n    changed.child.count = 2\n    return changed\n";
    for release in [false, true] {
        assert_codes(&checked_source(body, release), &[]);
    }
}

#[test]
fn resource_copy_projection_modes_do_not_escape_into_owning_calls_or_wrappers() {
    let bodies = [
        "function wrapped(view holder: Holder) returns nothing:\n    optional[Handle] borrowed = view some(holder.token)\n    return nothing\n",
        "function wrap(token: Handle) returns Holder:\n    return Holder(token: token, count: 1)\nfunction wrapped(view holder: Holder) returns nothing:\n    Holder borrowed = view wrap(holder.token)\n    return nothing\n",
        "function wrapped(view holder: Holder) returns nothing:\n    list[Handle] borrowed = view list(holder.token)\n    return nothing\n",
    ];
    for release in [false, true] {
        for body in bodies {
            assert_codes(&checked_source(body, release), &[364]);
        }
    }
}

#[test]
fn resource_copy_explicit_clone_has_one_refusal_and_keeps_safe_nested_fields() {
    let denied = [
        "function cloned(view envelope: Envelope) returns Holder:\n    return clone envelope.child\n",
        "function cloned(view envelope: Envelope) returns Handle:\n    return clone view envelope.child.token\n",
    ];
    let allowed = "function cloned_count(view envelope: Envelope) returns int64:\n    return clone envelope.child.count\n";
    for release in [false, true] {
        for body in denied {
            assert_codes(&checked_source(body, release), &[364]);
        }
        assert_codes(&checked_source(allowed, release), &[]);
    }
}

#[test]
fn resource_copy_generic_field_specializations_use_concrete_data_not_phantom_arguments() {
    let denied = "function extract[T](view slot: Slot[T]) returns T:\n    return slot.item\nfunction copied(view slot: Slot[Handle]) returns Handle:\n    return extract[Handle](view slot)\n";
    let allowed = "function extract[T](view slot: Slot[T]) returns T:\n    return slot.item\nfunction copied(view slot: Slot[int64]) returns int64:\n    return extract[int64](view slot)\nfunction cloned_phantom(view slot: Slot[Phantom[Handle]]) returns Phantom[Handle]:\n    return clone slot.item\n";
    for release in [false, true] {
        assert_codes(&checked_source(denied, release), &[364]);
        let checked = checked_source(allowed, release);
        assert_codes(&checked, &[]);
        assert!(
            checked
                .generic_function_instantiations
                .iter()
                .any(|body| body.concrete_args == [TypeInterner::INT64])
        );
    }
}

#[test]
fn resource_copy_list_clone_getter_rejects_recursive_elements_and_preserves_exact_states() {
    let denied = [
        "function copied(view items: list[Handle]) returns optional[Handle]:\n    return list.__get_clone[Handle](view items, 0)\n",
        "function copied(view items: list[Holder]) returns optional[Holder]:\n    return list.__get_clone[Holder](view items, 0)\n",
        "function copied(view items: list[list[optional[Handle]]]) returns optional[list[optional[Handle]]]:\n    return list.__get_clone[list[optional[Handle]]](view items, 0)\n",
        "function copied(view items: list[Handle]) returns optional[Handle]:\n    return items into view list.__get_clone[Handle](0)\n",
    ];
    let allowed = "machine Session:\n    states:\n        ready(count: int64)\n        holding(token: Handle)\n    transitions:\n        ready to holding\nfunction copied(view items: list[Phantom[Handle]]) returns optional[Phantom[Handle]]:\n    return list.__get_clone[Phantom[Handle]](view items, 0)\nfunction selected(view items: list[Session at ready]) returns optional[Session at ready]:\n    return list.__get_clone[Session at ready](view items, 0)\n";
    for release in [false, true] {
        for body in denied {
            assert_codes(&checked_source(body, release), &[364]);
        }
        assert_codes(&checked_source(allowed, release), &[]);
    }
}

#[test]
fn resource_copy_reflected_getters_check_returned_data_and_allow_unrelated_owner_resources() {
    let types = "struct NestedHolder:\n    values: list[optional[Handle]]\nenum Event:\n    held(token: Handle, count: int64)\n    nested(values: list[optional[Handle]])\nmachine Session:\n    states:\n        held(token: Handle, count: int64)\n        nested(values: list[optional[Handle]])\n        empty\n    transitions:\n        held to empty\n        nested to empty\n";
    let cases = [
        ("type.field_value", "Holder", "NestedHolder"),
        ("type.variant_field_value", "Event", "Event"),
        (
            "type.machine_field_value",
            "Session at held",
            "Session at nested",
        ),
    ];
    for release in [false, true] {
        for (getter, owner, nested_owner) in cases {
            for (owner, returned) in [(owner, "Handle"), (nested_owner, "list[optional[Handle]]")] {
                let body = format!(
                    "{types}function copied(view owner: {owner}, view field: TypeField) returns {returned}:\n    return {getter}[{owner}, {returned}](view owner, view field)\n"
                );
                assert_codes(&checked_source(&body, release), &[364]);
            }
            let direct = format!(
                "{types}function copied(view owner: {owner}, view field: TypeField) returns int64:\n    return {getter}[{owner}, int64](view owner, view field)\n"
            );
            assert_codes(&checked_source(&direct, release), &[]);
            let piped = format!(
                "{types}function copied(view owner: {owner}, view field: TypeField) returns Handle:\n    return owner into view {getter}[{owner}, Handle](view field)\n"
            );
            assert_codes(&checked_source(&piped, release), &[364]);
        }
    }
}

#[test]
fn resource_copy_reflected_field_getters_refuse_exact_resource_root_and_alias() {
    for release in [false, true] {
        for getter in [
            "type.field_value",
            "type.variant_field_value",
            "type.machine_field_value",
        ] {
            for resource in ["Handle", "HandleAlias"] {
                let body = format!(
                    "function invalid_root(view owner: {resource}, view field: TypeField) returns int64:\n    return {getter}[{resource}, int64](view owner, view field)\n"
                );
                let checked = checked_source(&body, release);
                assert_codes(&checked, &[300]);
                assert!(checked.diagnostics.iter().any(|diagnostic| {
                    diagnostic.severity == Severity::Error
                        && diagnostic
                            .message
                            .contains(&format!("non-resource value for {getter}"))
                }));
            }
        }
    }
}

fn public_get_module(project: &str) -> Module {
    let resource = "namespace copy_provider\nexport resource Handle\n";
    let mut module = parsed(resource, FileId::new(STDLIB_FILE_ID_START));
    // Parse the real shipped facade, retaining its original body and source spans.
    let mut list = parsed(
        include_str!("../../../../stdlib/list.jett"),
        FileId::new(STDLIB_FILE_ID_START + 1),
    );
    list.items.retain(|item| {
        matches!(item, Item::Namespace(_))
            || matches!(item, Item::Function(function) if function.name.name == "get")
    });
    assert_eq!(
        list.items.len(),
        2,
        "expected namespace and actual public get"
    );
    module.items.extend(list.items);
    module.items.extend(parsed(project, FileId::new(0)).items);
    module
}

#[test]
fn resource_copy_public_list_get_checks_the_real_facade_concrete_resource_specialization() {
    let project = "namespace caller\nfunction copied(view items: list[copy_provider.Handle]) returns optional[copy_provider.Handle]:\n    return list.get[copy_provider.Handle](view items, 0)\n";
    let module = public_get_module(project);
    for release in [false, true] {
        let checked = checked_module(&module, release);
        assert_codes(&checked, &[364]);
        let diagnostic = checked
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.severity == Severity::Error)
            .unwrap();
        assert_eq!(diagnostic.span.file, FileId::new(STDLIB_FILE_ID_START + 1));
        assert!(diagnostic.message.contains("copy_provider.Handle"));
    }
}

#[test]
fn resource_copy_public_list_get_preserves_non_resource_and_phantom_specializations() {
    let project = "namespace caller\nstruct Holder:\n    token: copy_provider.Handle\n    count: int64\nstruct Phantom[T]:\n    count: int64\nfunction copied(view items: list[int64]) returns optional[int64]:\n    return list.get[int64](view items, 0)\nfunction phantom(view items: list[Phantom[copy_provider.Handle]]) returns optional[Phantom[copy_provider.Handle]]:\n    return list.get[Phantom[copy_provider.Handle]](view items, 0)\nfunction count(view holder: Holder) returns int64:\n    return holder.count\n";
    let module = public_get_module(project);
    for release in [false, true] {
        let checked = checked_module(&module, release);
        assert_codes(&checked, &[]);
        assert!(checked.generic_function_instantiations.iter().any(|body| {
            body.concrete_args == [TypeInterner::INT64]
                && body
                    .intrinsic_type_arguments
                    .values()
                    .any(|arguments| arguments == &[TypeInterner::INT64])
        }));
    }
}
