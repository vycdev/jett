use super::*;

fn reflected_source(actual: &str, requested: &str) -> String {
    format!(
        r#"namespace app
type Positive = int64 where value > 0
type Higher = Positive where value > 5
type Values = list[Positive] where true
struct Record:
    values: {actual}
enum Event:
    content(values: {actual})
machine Session:
    states:
        empty
        content(values: {actual})
    transitions:
        empty to content
function record_read(view source: Record, view field: TypeField) returns {requested}:
    return type.field_value[Record, {requested}](view source, view field)
function variant_read(view source: Event, view field: TypeField) returns {requested}:
    return type.variant_field_value[Event, {requested}](view source, view field)
function machine_read(view source: Session, view field: TypeField) returns {requested}:
    return type.machine_field_value[Session, {requested}](view source, view field)
function record_pipe(view source: Record, view field: TypeField) returns {requested}:
    return source into view type.field_value[Record, {requested}](view field)
function variant_pipe(view source: Event, view field: TypeField) returns {requested}:
    return source into view type.variant_field_value[Event, {requested}](view field)
function machine_pipe(view source: Session, view field: TypeField) returns {requested}:
    return source into view type.machine_field_value[Session, {requested}](view field)
"#
    )
}

fn reads(program: &Program) -> impl Iterator<Item = &Function> {
    program.functions.iter().filter(|function| {
        function.identity.declaration.name.ends_with("_read")
            || function.identity.declaration.name.ends_with("_pipe")
    })
}

#[test]
fn recursive_reflected_producers_finish_structural_pass_before_predicates() {
    let source = reflected_source(
        "list[optional[result[map[int64, list[int64]], set[int64]]]]",
        "list[optional[result[map[Positive, list[Positive]], set[Positive]]]]",
    );
    let (program, types) = lower_handler_source(&source);
    validate(&program).expect("recursive producer CFG");
    assert_eq!(reads(&program).count(), 6);
    for function in reads(&program) {
        let mut ready = Vec::new();
        let mut checks = Vec::new();
        let mut getters = 0;
        for block in &function.blocks {
            for statement in &block.statements {
                match &statement.kind {
                    StatementKind::ReflectedContainerReady { source, kind } => {
                        assert!(kind.matches_type(&types, function.local(*source).unwrap().ty));
                        ready.push((block.id.index(), *kind));
                    }
                    StatementKind::CheckRefinement { .. } => checks.push(block.id.index()),
                    StatementKind::SequenceGet { consume, .. } => assert!(!consume),
                    StatementKind::Let { value, .. }
                        if matches!(
                            value.kind,
                            ExpressionKind::Intrinsic {
                                field_validation: Some(hir::ReflectedFieldValidation::Read),
                                ..
                            }
                        ) =>
                    {
                        getters += 1;
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(getters, 1, "one raw getter after staged source operands");
        for kind in [
            ReflectedContainerKind::List,
            ReflectedContainerKind::Set,
            ReflectedContainerKind::Map,
            ReflectedContainerKind::Optional,
            ReflectedContainerKind::Result,
        ] {
            assert!(ready.iter().any(|(_, observed)| *observed == kind));
        }
        assert!(
            ready.iter().map(|(block, _)| *block).max().unwrap() < *checks.iter().min().unwrap(),
            "all structural blocks precede predicate traversal"
        );
        let plan = crate::move_values::MoveValuePlan::analyze(&program, function, &types)
            .expect("candidate snapshots have independent owners");
        assert!(!plan.owned_locals.is_empty());
    }
}

#[test]
fn recursive_reflected_reuse_distinguishes_whole_exact_from_child_ancestor() {
    for (actual, requested, readiness) in [
        ("list[Positive]", "list[Positive]", false),
        ("Values", "Values", false),
        ("Values", "list[Positive]", true),
        ("list[Higher]", "list[Positive]", true),
    ] {
        let (program, types) = lower_handler_source(&reflected_source(actual, requested));
        for function in reads(&program) {
            let statements: Vec<_> = function
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .collect();
            assert_eq!(
                statements.iter().any(|statement| matches!(
                    statement.kind,
                    StatementKind::ReflectedContainerReady { .. }
                )),
                readiness,
                "whole exact schema skips; generic base/changed wrapper keeps readiness: {actual}/{requested}"
            );
            assert!(
                !statements.iter().any(|statement| matches!(
                    statement.kind,
                    StatementKind::CheckRefinement { .. }
                )),
                "already proven child predicate must not rerun"
            );
            crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
        }
    }
}

#[test]
fn recursive_reflected_refine_validates_base_children_before_outer_predicate() {
    let (program, types) = lower_handler_source(&reflected_source("list[int64]", "Values"));
    for function in reads(&program) {
        let checks: Vec<_> = function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .filter_map(|statement| match &statement.kind {
                StatementKind::CheckRefinement { type_name, .. } => Some(type_name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(checks, ["app.Positive", "app.Values"]);
        assert!(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .any(|statement| matches!(
                    statement.kind,
                    StatementKind::ReflectedContainerReady {
                        kind: ReflectedContainerKind::List,
                        ..
                    }
                ))
        );
        crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
    }
}

#[test]
fn recursive_reflected_map_projection_keeps_key_then_value_without_consuming_candidate() {
    let (program, types) = lower_handler_source(&reflected_source(
        "map[int64, int64]",
        "map[Positive, Positive]",
    ));
    for function in reads(&program) {
        let projections: Vec<_> = function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .filter_map(|statement| match statement.kind {
                StatementKind::SequenceGet { part, consume, .. } => Some((part, consume)),
                _ => None,
            })
            .collect();
        assert_eq!(
            projections,
            [(SequencePart::Key, false), (SequencePart::Value, false)]
        );
        crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
    }
}

#[test]
fn recursive_reflected_root_requests_keep_base_readiness_or_shared_declared_prefix() {
    for (actual, requested, readiness, suffix) in [
        ("Values", "Independent", true, vec!["app.Independent"]),
        ("Values", "Strong", false, vec!["app.Strong"]),
        ("Strong", "Sibling", false, vec!["app.Sibling"]),
        ("Strong", "Values", false, vec![]),
    ] {
        let source = reflected_source(actual, requested).replace(
            "type Values = list[Positive] where true",
            "type Values = list[Positive] where true\ntype Independent = list[Positive] where true\ntype Strong = Values where true\ntype Sibling = Values where true",
        );
        let (program, types) = lower_handler_source(&source);
        validate(&program).unwrap();
        for function in reads(&program) {
            let statements: Vec<_> = function
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .collect();
            assert_eq!(
                statements.iter().any(|statement| matches!(
                    statement.kind,
                    StatementKind::ReflectedContainerReady { .. }
                )),
                readiness,
                "independent root base preflights; shared declared root preserves proof"
            );
            let checks: Vec<_> = statements
                .iter()
                .filter_map(|statement| match &statement.kind {
                    StatementKind::CheckRefinement { type_name, .. } => Some(type_name.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(checks, suffix, "{actual}/{requested}");
            crate::move_values::MoveValuePlan::analyze(&program, function, &types).unwrap();
        }
    }
}
