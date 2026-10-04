use super::*;
use jett_common::{FileId, SourceOrigin};
use std::collections::HashMap;

const HANDLED: &str = r#"namespace app
function maybe_flag(valid: bool) returns result[bool, string]:
    if valid:
        return ok(true)
    return fail("missing")
function exercise() returns bool:
    bool before = true
    breakpoint(maybe_flag(true) handle error:
        default false
    )
    breakpoint(maybe_flag(false) handle error:
        default false
    )
    return before
"#;

fn source_program(source: &str, release: bool) -> (Program, TypeInterner) {
    let file = FileId::new(0);
    let parsed = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    assert!(
        resolved
            .diagnostics
            .iter()
            .all(|d| d.severity != jett_diagnostics::Severity::Error),
        "{:?}",
        resolved.diagnostics
    );
    let checked = jett_typecheck::check_with_options(
        &parsed.module,
        &resolved,
        jett_typecheck::CheckOptions { release },
    );
    assert!(
        checked
            .diagnostics
            .iter()
            .all(|d| d.severity != jett_diagnostics::Severity::Error),
        "{:?}",
        checked.diagnostics
    );
    let high = hir::lower(
        &parsed.module,
        &resolved,
        &checked,
        &HashMap::from([(file, SourceOrigin::Project)]),
    )
    .expect("exact checked HIR");
    (
        crate::lower(&high, &checked.interner).expect("exact source MIR"),
        checked.interner,
    )
}

fn exercise(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|f| f.identity.declaration.name == "exercise")
        .expect("exercise")
}

fn exercise_mut(program: &mut Program) -> &mut Function {
    program
        .functions
        .iter_mut()
        .find(|f| f.identity.declaration.name == "exercise")
        .expect("exercise")
}

fn rejects(program: &Program, types: &TypeInterner) {
    let errors = crate::validate_call_ownership(program, types)
        .expect_err("corrupted extraction is refused");
    assert!(
        errors
            .iter()
            .any(|e| e.message.contains("breakpoint") || e.message.contains("source context")),
        "{errors:?}"
    );
}

#[test]
fn breakpoint_extraction_rejoins_real_nested_handler_conditions_and_release_elision() {
    let (mut program, types) = source_program(HANDLED, false);
    let function = exercise(&program);
    assert_eq!(function.breakpoint_regions.len(), 2);
    let sites = validate(function, &types).expect("original exact sites");
    let first = &function.blocks[function.entry.index() as usize].statements[0];
    assert!(matches!(first.kind, StatementKind::Let { .. }));
    assert!(
        !sites.statement(function.entry, 0),
        "ordinary shared prefix remains ordinary"
    );
    assert!(
        !sites.terminator(function.blocks.last().expect("return block").id),
        "ordinary return remains ordinary"
    );
    for pass in [
        crate::prepare_native_sequences,
        crate::prepare_native_uninhabited_sums,
        crate::prepare_native_generated_functions,
    ] {
        pass(&mut program, &types);
        crate::validate_call_ownership(&program, &types)
            .expect("every canonical boundary preserves exact sites");
    }
    let (released, types) = source_program(HANDLED, true);
    assert!(exercise(&released).breakpoint_regions.is_empty());
    assert!(exercise(&released).blocks.iter().all(|b| {
        b.statements
            .iter()
            .all(|s| !matches!(s.kind, StatementKind::Breakpoint { .. }))
    }));
    crate::validate_call_ownership(&released, &types)
        .expect("release never evaluates debug condition");
}

#[test]
fn breakpoint_extraction_does_not_authorize_copied_moved_or_new_marker_calls() {
    let (program, types) = source_program(HANDLED, false);
    let function = exercise(&program);
    let (site, original) = function.breakpoint_regions[0].statements.iter().find(|(_, statement)| matches!(&statement.kind, StatementKind::Let { value, .. } if matches!(value.kind, hir::ExpressionKind::Call { .. }))).map(|(site, value)| (*site, value.clone())).expect("extracted real call");
    for mutation in ["copy", "move", "marker", "endpoint", "condition"] {
        let mut changed = program.clone();
        let function = exercise_mut(&mut changed);
        match mutation {
            "copy" => {
                let id = BlockId(function.blocks.len() as u32);
                function.blocks.push(BasicBlock {
                    id,
                    statements: vec![original.clone()],
                    terminator: Terminator {
                        kind: TerminatorKind::Unreachable,
                        span: original.span,
                    },
                });
            }
            "move" => {
                function.blocks[site.block.index() as usize]
                    .statements
                    .remove(site.index);
            }
            "marker" => {
                let Endpoint::Live(endpoint) = &function.breakpoint_regions[0].endpoint else {
                    panic!("live endpoint");
                };
                let value = function.blocks[endpoint.block.index() as usize].statements
                    [endpoint.index]
                    .clone();
                let id = BlockId(function.blocks.len() as u32);
                function.blocks.push(BasicBlock {
                    id,
                    statements: vec![value],
                    terminator: Terminator {
                        kind: TerminatorKind::Unreachable,
                        span: original.span,
                    },
                });
            }
            "endpoint" | "condition" => {
                let Endpoint::Live(endpoint) = &function.breakpoint_regions[0].endpoint else {
                    panic!("live endpoint");
                };
                let endpoint = *endpoint;
                let target = &mut function.blocks[endpoint.block.index() as usize].statements
                    [endpoint.index];
                if mutation == "endpoint" {
                    target.kind = StatementKind::Evaluate(Expression {
                        kind: hir::ExpressionKind::Bool(true),
                        ty: TypeInterner::BOOL,
                        span: target.span,
                    });
                } else {
                    let StatementKind::Breakpoint { condition, .. } = &mut target.kind else {
                        panic!("endpoint");
                    };
                    condition.as_mut().expect("condition").kind = hir::ExpressionKind::Bool(true);
                }
            }
            _ => unreachable!(),
        }
        rejects(&changed, &types);
    }
}

#[test]
fn breakpoint_extraction_rejects_orphan_duplicate_foreign_and_alternate_entry_records() {
    let (program, types) = source_program(HANDLED, false);
    for mutation in ["duplicate", "orphan", "foreign", "index", "edge"] {
        let mut changed = program.clone();
        let function = exercise_mut(&mut changed);
        match mutation {
            "duplicate" => function
                .breakpoint_regions
                .push(function.breakpoint_regions[0].clone()),
            "orphan" => function.breakpoint_regions[0].parent = Some(usize::MAX),
            "foreign" => function.breakpoint_regions[0].owner = program.functions[0].id,
            "index" => {
                let record = &mut function.breakpoint_regions[0];
                let (site, value) = record
                    .statements
                    .first_key_value()
                    .map(|(site, value)| (*site, value.clone()))
                    .expect("site");
                record.statements.remove(&site);
                record.statements.insert(
                    Site {
                        index: site.index + 1,
                        ..site
                    },
                    value,
                );
            }
            "edge" => {
                let id = BlockId(function.blocks.len() as u32);
                function.blocks.push(BasicBlock {
                    id,
                    statements: Vec::new(),
                    terminator: Terminator {
                        kind: TerminatorKind::Goto(function.breakpoint_regions[0].start.block),
                        span: function.span,
                    },
                });
            }
            _ => unreachable!(),
        }
        rejects(&changed, &types);
    }
}

const NESTED: &str = r#"namespace app
function inspect(view values: list[int64]) returns bool:
    return true
function maybe_flag(valid: bool) returns result[bool, string]:
    if valid:
        return ok(true)
    return fail("missing")
function exercise(values: list[int64]) returns bool:
    breakpoint(false or (maybe_flag(false) handle error:
        for item in view values:
            breakpoint(inspect(view values))
        default true
    ))
    return inspect(view values)
"#;

#[test]
fn breakpoint_extraction_nested_lazy_loop_sites_survive_sequence_prefixes_edges_and_remaps() {
    let (mut program, types) = source_program(NESTED, false);
    let function = exercise(&program);
    assert_eq!(function.breakpoint_regions.len(), 2);
    assert_eq!(
        function.breakpoint_regions[1].parent,
        Some(function.breakpoint_regions[0].id)
    );
    let parent_sites = function.breakpoint_regions[0]
        .statements
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    assert!(
        function.breakpoint_regions[1]
            .statements
            .keys()
            .all(|site| !parent_sites.contains(site))
    );
    crate::prepare_native_sequences(&mut program, &types);
    crate::validate_call_ownership(&program, &types)
        .expect("exact sequence prefixes and split edges");
    assert!(
        exercise(&program)
            .blocks
            .iter()
            .all(|b| !matches!(b.terminator.kind, TerminatorKind::ForEach { .. }))
    );
    crate::prepare_native_uninhabited_sums(&mut program, &types);
    crate::prepare_native_generated_functions(&mut program, &types);
    crate::validate_call_ownership(&program, &types).expect("dense prepared nested sites");
    let mut changed = program.clone();
    let function = exercise_mut(&mut changed);
    let prefix = function
        .blocks
        .iter_mut()
        .flat_map(|b| &mut b.statements)
        .find(|s| matches!(s.kind, StatementKind::SequenceGet { .. }))
        .expect("canonical getter");
    prefix.span.start += 1;
    rejects(&changed, &types);
}

#[test]
fn breakpoint_extraction_exact_never_side_selection_retains_aborted_evaluation_context() {
    let source = r#"namespace app
function accept[T](value: T) returns bool:
    return true
function exercise[T](outcome: optional[T]) returns bool:
    breakpoint(accept(outcome handle:
        return false
    ))
    return true
function main() returns nothing:
    bool observed = exercise(none)
    trace observed
    return nothing
"#;
    let (mut program, types) = source_program(source, false);
    crate::prepare_native_uninhabited_sums(&mut program, &types);
    let function = exercise(&program);
    assert_eq!(function.breakpoint_regions.len(), 1);
    assert!(matches!(
        function.breakpoint_regions[0].endpoint,
        Endpoint::CanonicallyAborted { .. }
    ));
    assert!(!function.blocks.iter().any(|b| {
        b.statements
            .iter()
            .any(|s| matches!(s.kind, StatementKind::Breakpoint { .. }))
    }));
    crate::validate_call_ownership(&program, &types)
        .expect("absent call executes before the exact canonical abort");
    let mut changed = program.clone();
    let function = exercise_mut(&mut changed);
    function.breakpoint_regions[0].selections[0].selected = function.entry;
    rejects(&changed, &types);
    let mut changed = program.clone();
    exercise_mut(&mut changed).breakpoint_regions[0].abort_allowed = false;
    rejects(&changed, &types);
}

#[test]
fn breakpoint_extraction_observation_stays_lexical_across_generic_and_inline_bodies() {
    let source = r#"namespace app
function consume[T](value: T) returns bool:
    return true
function exercise(values: list[int64]) returns bool:
    function(list[int64]) returns bool callback = function(item: list[int64]) returns bool: return consume(item)
    breakpoint(consume(values) and callback(clone values))
    return consume(values)
"#;
    let (program, types) = source_program(source, false);
    crate::validate_call_ownership(&program, &types)
        .expect("ordinary callee bodies do not inherit observation");
    assert!(
        program
            .functions
            .iter()
            .filter(|f| f.debug_kind == hir::FunctionDebugKind::Inline)
            .all(|f| f.breakpoint_regions.is_empty())
    );
    let function = exercise(&program);
    let sites = validate(function, &types).expect("lexical observed call sites");
    assert!(!sites.terminator(function.blocks.last().expect("return").id));
    let ordinary = source.replace(
        "    breakpoint(consume(values) and callback(clone values))",
        "    bool used = consume(values)",
    );
    let parsed = jett_parser::parse(&ordinary, FileId::new(0));
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    let checked = jett_typecheck::check(&parsed.module, &resolved);
    assert!(
        checked.diagnostics.iter().any(|d| d.code.code() == 400),
        "{:?}",
        checked.diagnostics
    );
}

#[test]
fn breakpoint_extraction_float_snapshot_equality_is_bitwise_including_nan_and_signed_zero() {
    let span = Span::new(FileId::new(0), 1, 2);
    let statement = |bits| Statement {
        kind: StatementKind::Evaluate(Expression {
            kind: hir::ExpressionKind::Float(f64::from_bits(bits)),
            ty: TypeInterner::FLOAT64,
            span,
        }),
        span,
    };
    let nan = statement(0x7ff8_0000_0000_0001);
    assert!(statement_equal(&nan, &nan));
    assert!(!statement_equal(&nan, &statement(0x7ff8_0000_0000_0002)));
    assert!(!statement_equal(&statement(0), &statement(1_u64 << 63)));
}

#[test]
fn breakpoint_extraction_invalid_original_never_disappears_in_a_preparation() {
    let (program, types) = source_program(HANDLED, false);
    for pass in [
        crate::prepare_native_sequences,
        crate::prepare_native_uninhabited_sums,
        crate::prepare_native_generated_functions,
    ] {
        let mut changed = program.clone();
        let record = &mut exercise_mut(&mut changed).breakpoint_regions[0];
        record.edges.clear();
        let before = changed.clone();
        pass(&mut changed, &types);
        assert_eq!(
            changed, before,
            "failed original proof keeps all runtime and private records"
        );
        rejects(&changed, &types);
    }
}

#[test]
fn breakpoint_extraction_present_success_selection_keeps_the_live_endpoint_and_tag() {
    let source = r#"namespace app
function exercise[T, E](outcome: result[T, E]) returns bool:
    breakpoint(outcome handle error:
        return false
    )
    return true
function main() returns nothing:
    bool observed = exercise(ok(true))
    trace observed
    return nothing
"#;
    let (mut program, types) = source_program(source, false);
    crate::prepare_native_uninhabited_sums(&mut program, &types);
    let function = exercise(&program);
    assert!(matches!(
        function.breakpoint_regions[0].endpoint,
        Endpoint::Live(_)
    ));
    assert_eq!(function.breakpoint_regions[0].selections.len(), 1);
    assert!(function.blocks.iter().any(|block| {
        block
            .statements
            .iter()
            .any(|statement| matches!(statement.kind, StatementKind::SumTag { .. }))
    }));
    crate::validate_call_ownership(&program, &types)
        .expect("selected true edge retains runtime tag and exact live endpoint");
    let mut changed = program.clone();
    let function = exercise_mut(&mut changed);
    let selecting = function.breakpoint_regions[0].selections[0].block;
    function.blocks[selecting.index() as usize]
        .terminator
        .span
        .start += 1;
    rejects(&changed, &types);
}

#[test]
fn breakpoint_extraction_ordinary_loop_prefixes_shift_sites_without_gaining_observation() {
    let source = r#"namespace app
function maybe_flag(valid: bool) returns result[bool, string]:
    if valid:
        return ok(true)
    return fail("missing")
function exercise(view values: list[int64]) returns bool:
    for item in view values:
        breakpoint(maybe_flag(true) handle error:
            default false
        )
    return true
"#;
    let (mut program, types) = source_program(source, false);
    crate::prepare_native_sequences(&mut program, &types);
    let function = exercise(&program);
    let sites =
        validate(function, &types).expect("ordinary loop boundary and dense breakpoint sites");
    let mut getters = 0;
    for block in &function.blocks {
        for (index, statement) in block.statements.iter().enumerate() {
            if matches!(statement.kind, StatementKind::SequenceGet { .. }) {
                getters += 1;
                assert!(
                    !sites.statement(block.id, index),
                    "outside ForEach prefix stays ordinary"
                );
            }
        }
    }
    assert_eq!(getters, 1);
    crate::prepare_native_generated_functions(&mut program, &types);
    crate::validate_call_ownership(&program, &types)
        .expect("shared body prefix index remaps are exact");
}
