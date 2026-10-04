use super::*;
use std::collections::HashMap;

const SIGNATURE_REUSE_SOURCE: &str = r#"namespace app
function accept(value: int64) returns int64:
    return 7
function first() returns int64:
    return accept(1)
function second() returns int64:
    return accept(2)
"#;

#[test]
fn whole_ownership_pass_rejoins_fresh_signatures_and_preserves_error_order() {
    let (mut program, types) = source_program(SIGNATURE_REUSE_SOURCE);
    crate::validate_call_ownership(&program, &types).expect("original complete ownership pass");
    for function in &program.functions {
        validate_function(&program, function, &types).expect("public per-function ownership pass");
    }

    // Keep structural parameter/local agreement while invalidating the exact
    // callee signature retained by both already-checked source call packets.
    let callee = program
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "accept")
        .expect("declared accept");
    let parameter = &mut callee.params[0];
    parameter.ty = TypeInterner::INT32;
    let local = &mut callee.locals[parameter.local.index() as usize];
    local.ty = TypeInterner::INT32;
    local.debug_ty = TypeInterner::INT32;
    crate::validate(&program).expect("signature mutation remains structurally well formed");

    let expected = program
        .functions
        .iter()
        .filter_map(|function| {
            validate_function(&program, function, &types)
                .err()
                .map(|message| crate::ValidationError {
                    span: function.span,
                    message,
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        expected.len(),
        2,
        "both unused caller functions retain exact signature obligations"
    );
    assert!(
        expected.iter().all(|error| error.message
            == "source function ownership signature differs from the exact HIR declaration"),
        "{expected:?}"
    );
    let errors = crate::validate_call_ownership(&program, &types)
        .expect_err("a new whole pass cannot reuse the preceding callee signature");
    assert_eq!(
        errors, expected,
        "complete pass retains per-function diagnostics and declaration order"
    );
}

#[test]
fn whole_ownership_pass_checks_a_late_disconnected_invocation() {
    let (mut program, types) = source_program(SIGNATURE_REUSE_SOURCE);
    crate::validate_call_ownership(&program, &types).expect("original complete ownership pass");
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "second")
        .expect("last caller");
    let mut value = read_call(function).clone();
    value.ty = TypeInterner::BOOL;
    let id = BlockId(function.blocks.len() as u32);
    function.blocks.push(crate::BasicBlock {
        id,
        statements: vec![crate::Statement {
            span: value.span,
            kind: S::Evaluate(value),
        }],
        terminator: crate::Terminator {
            span: function.span,
            kind: T::Unreachable,
        },
    });
    let span = function.span;
    crate::validate(&program)
        .expect("disconnected expression metadata is structurally well formed");
    let errors = crate::validate_call_ownership(&program, &types)
        .expect_err("table reuse cannot skip the unused caller's disconnected block");
    assert_eq!(
        errors,
        vec![crate::ValidationError {
            span,
            message: "call ownership result differs from its checked source certificate".into(),
        }]
    );
}

const SOURCE: &str = r#"namespace app
function read(view values: list[int64], extra: int64) returns int64:
    return extra
function exercise(values: list[int64], other: list[int64], incoming: optional[int64]) returns int64:
    return read(values, incoming handle:
        default 3
    )
"#;

fn source_program(source: &str) -> (Program, TypeInterner) {
    let file = jett_common::FileId::new(0);
    let parsed = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    let checked = jett_typecheck::check(&parsed.module, &resolved);
    assert!(
        checked
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
        "{:?}",
        checked.diagnostics
    );
    let high = hir::lower(
        &parsed.module,
        &resolved,
        &checked,
        &HashMap::from([(file, jett_common::SourceOrigin::Project)]),
    )
    .expect("source HIR");
    (
        crate::lower(&high, &checked.interner).expect("source MIR"),
        checked.interner,
    )
}

fn exercise(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "exercise")
        .expect("exercise")
}

fn exercise_mut(program: &mut Program) -> &mut Function {
    program
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "exercise")
        .expect("exercise")
}

fn read_call(function: &Function) -> &Expression {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .find_map(|statement| match &statement.kind {
            S::Let { value, .. } | S::Evaluate(value)
                if matches!(value.kind, E::Call { .. } | E::IndirectCall { .. }) =>
            {
                Some(value)
            }
            _ => None,
        })
        .or_else(|| {
            function
                .blocks
                .iter()
                .find_map(|block| match &block.terminator.kind {
                    T::Return(Some(value))
                        if matches!(value.kind, E::Call { .. } | E::IndirectCall { .. }) =>
                    {
                        Some(value)
                    }
                    _ => None,
                })
        })
        .expect("read invocation")
}

fn read_call_mut(function: &mut Function) -> &mut Expression {
    // The handled source fixture materializes its actual call as an owning Let.
    function
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.statements)
        .find_map(|statement| match &mut statement.kind {
            S::Let { value, .. } | S::Evaluate(value)
                if matches!(value.kind, E::Call { .. } | E::IndirectCall { .. }) =>
            {
                Some(value)
            }
            _ => None,
        })
        .expect("materialized read invocation")
}

fn owning_stage(function: &Function) -> (LocalId, LocalId) {
    let E::Call {
        ownership: hir::CallOwnership::Source(source),
        ..
    } = &read_call(function).kind
    else {
        panic!("source read");
    };
    let hir::ArgumentStaging::Relinquished { owner, loan } = source.arguments[0].staging else {
        panic!("relinquished stage");
    };
    (owner, loan)
}

fn rejects(program: &Program, types: &TypeInterner) {
    let error = validate_function(program, exercise(program), types)
        .expect_err("malformed acquisition must fail");
    assert!(
        error.contains("call ownership") || error.contains("call view"),
        "{error}"
    );
}

#[test]
fn caller_acquisition_uses_original_owner_and_exact_handled_copy_result() {
    for retained in [false, true] {
        let source = if retained {
            SOURCE.replace("return read(values,", "return read(view values,")
        } else {
            SOURCE.to_owned()
        };
        let (program, types) = source_program(&source);
        let function = exercise(&program);
        let acquisitions =
            validate_function(&program, function, &types).expect("valid call acquisition");
        let call = read_call(function);
        let arguments = acquisitions
            .arguments(call)
            .expect("validated call occurrence");
        assert!(
            arguments.is_empty(),
            "staged owner must be consumed at its initializer"
        );
        if retained {
            assert_eq!(acquisitions.owner_initializers().count(), 0);
        } else {
            let (owner, loan) = owning_stage(function);
            let acquired = acquisitions
                .owner_initializer(owner)
                .expect("owning initializer");
            assert_eq!(acquired.binding, Some(function.params[0].local));
            assert_ne!(acquired.binding, Some(owner));
            assert_ne!(acquired.binding, Some(loan));
            assert_eq!(acquired.effect, CheckedCallerEffect::RelinquishOwned);
            assert_eq!(acquired.actual_type, function.params[0].ty);
        }
    }
}

#[test]
fn caller_acquisition_original_owned_input_keeps_formal_and_source_indices() {
    let source = r#"namespace app
function take(values: list[int64], extra: int64) returns int64:
    return extra
function exercise(values: list[int64]) returns int64:
    return take(extra: 3, values: values)
"#;
    let (program, types) = source_program(source);
    let function = exercise(&program);
    let call = read_call(function);
    let acquisitions = validate_function(&program, function, &types).expect("original owned call");
    let [argument] = acquisitions.arguments(call).expect("typed acquisition") else {
        panic!("one owner");
    };
    assert_eq!(argument.parameter_index, 0);
    assert_eq!(argument.source_index, 1);
    assert_eq!(argument.source.binding, Some(function.params[0].local));
    assert_eq!(argument.source.effect, CheckedCallerEffect::TransferOwned);
    let E::Call { args, .. } = &call.kind else {
        panic!("owned call");
    };
    assert_eq!(
        acquisitions.argument_binding(&args[0]),
        Some(function.params[0].local)
    );
    assert_eq!(acquisitions.argument_binding(&args[1]), None);
    assert_eq!(
        acquisitions.argument_binding(&args[0].clone()),
        None,
        "a clone is not a validated occurrence"
    );
    assert_eq!(acquisitions.owner_initializers().count(), 0);
}

#[test]
fn caller_acquisition_rejects_original_laundering_and_wrong_current_slot() {
    let (program, types) = source_program(SOURCE);
    let mut changed = program.clone();
    let E::Call {
        ownership: hir::CallOwnership::Source(source),
        ..
    } = &mut read_call_mut(exercise_mut(&mut changed)).kind
    else {
        panic!("call");
    };
    source.arguments[0].staging = hir::ArgumentStaging::Original;
    rejects(&changed, &types);

    let mut changed = program.clone();
    let other = exercise(&changed).params[1].local;
    let E::Call { args, .. } = &mut read_call_mut(exercise_mut(&mut changed)).kind else {
        panic!("call");
    };
    let E::View(value) = &mut args[0].kind else {
        panic!("physical view");
    };
    value.kind = E::Local(other);
    rejects(&changed, &types);
}

#[test]
fn caller_acquisition_rejects_same_type_owner_substitution_and_duplicate_initializer() {
    let (program, types) = source_program(SOURCE);
    let (owner, _) = owning_stage(exercise(&program));
    let other = exercise(&program).params[1].local;
    let mut changed = program.clone();
    let initializer = exercise_mut(&mut changed)
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.statements)
        .find_map(|statement| match &mut statement.kind {
            S::Let { local, value } if *local == owner => Some(value),
            _ => None,
        })
        .expect("owner initializer");
    initializer.kind = E::Local(other);
    rejects(&changed, &types);

    let mut changed = program.clone();
    let function = exercise_mut(&mut changed);
    let (block, statement) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, value)| {
            value
                .statements
                .iter()
                .position(
                    |statement| matches!(statement.kind, S::Let { local, .. } if local == owner),
                )
                .map(|statement| (block, statement))
        })
        .expect("owner statement");
    let duplicate = function.blocks[block].statements[statement].clone();
    function.blocks[block]
        .statements
        .insert(statement, duplicate);
    rejects(&changed, &types);
}

#[test]
fn caller_acquisition_rejects_wrong_sum_payload_selection_and_missing_end() {
    let (program, types) = source_program(SOURCE);
    let mut changed = program.clone();
    let function = exercise_mut(&mut changed);
    let output = function
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.statements)
        .find_map(|statement| match &mut statement.kind {
            S::SumTake { success, .. } if *success => Some(success),
            _ => None,
        })
        .expect("handled success");
    *output = false;
    rejects(&changed, &types);

    let mut changed = program.clone();
    for block in &mut exercise_mut(&mut changed).blocks {
        block
            .statements
            .retain(|statement| !matches!(statement.kind, S::EndCallView { .. }));
    }
    rejects(&changed, &types);
}

#[test]
fn caller_acquisition_does_not_skip_disconnected_corrupt_call_packets() {
    let (mut program, types) = source_program(SOURCE);
    let function = exercise_mut(&mut program);
    let mut value = read_call(function).clone();
    let E::Call {
        ownership: hir::CallOwnership::Source(source),
        ..
    } = &mut value.kind
    else {
        panic!("call");
    };
    source.arguments[0].source_index = source.arguments[1].source_index;
    let block = BlockId(function.blocks.len() as u32);
    function.blocks.push(crate::BasicBlock {
        id: block,
        statements: vec![crate::Statement {
            span: value.span,
            kind: S::Evaluate(value),
        }],
        terminator: crate::Terminator {
            span: function.span,
            kind: T::Unreachable,
        },
    });
    rejects(&program, &types);
}

#[test]
fn caller_acquisition_defaults_require_contained_spans_and_the_exact_failed_arm() {
    let (program, types) = source_program(SOURCE);
    validate_function(&program, exercise(&program), &types)
        .expect("valid contained handled default");
    for malformed in [
        "statement span",
        "value span",
        "success arm",
        "missing anchor",
        "disconnected default",
    ] {
        let mut changed = program.clone();
        let function = exercise_mut(&mut changed);
        let output = function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .find_map(|statement| match statement.kind {
                S::SumTake {
                    target,
                    success: true,
                    ..
                } => Some(target),
                _ => None,
            })
            .expect("handled success output");
        let (block, index) = function.blocks.iter().enumerate().find_map(|(block, value)|
            value.statements.iter().position(|statement| matches!(statement.kind, S::Let { local, .. } if local == output))
                .map(|index| (block, index))).expect("contained default definition");
        match malformed {
            "statement span" => {
                function.blocks[block].statements[index].span.file =
                    jett_common::FileId::new(u32::MAX);
            }
            "value span" => {
                let S::Let { value, .. } = &mut function.blocks[block].statements[index].kind
                else {
                    panic!("default Let");
                };
                value.span.file = jett_common::FileId::new(u32::MAX);
            }
            "success arm" => {
                let default = function.blocks[block].statements.remove(index);
                let success = function.blocks.iter_mut().find(|block| block.statements.iter().any(|statement|
                    matches!(statement.kind, S::SumTake { target, success: true, .. } if target == output))).expect("success arm");
                success.statements.push(default);
            }
            "missing anchor" => {
                for block in &mut function.blocks {
                    block.statements.retain(|statement| {
                        !matches!(statement.kind,
                        S::SumTake { target, success: true, .. } if target == output)
                    });
                }
            }
            "disconnected default" => {
                let default = function.blocks[block].statements[index].clone();
                function.blocks.push(crate::BasicBlock {
                    id: BlockId(function.blocks.len() as u32),
                    statements: vec![default],
                    terminator: crate::Terminator {
                        span: function.span,
                        kind: T::Unreachable,
                    },
                });
            }
            _ => unreachable!(),
        }
        rejects(&changed, &types);
    }
}

#[test]
fn caller_acquisition_written_direct_owned_producer_restores_exact_backing() {
    let source = r#"namespace app
interface Named:
    function name(view self: Named) returns string
implement Named for int64:
    function name(view self: int64) returns string:
        return "member"
function produce() returns Named:
    return 7
function read(view item: Named, extra: int64) returns int64:
    return extra
function exercise(spare: Named, incoming: optional[int64]) returns int64:
    return read(view produce(), incoming handle:
        default 3
    )
"#;
    let (program, types) = source_program(source);
    let function = exercise(&program);
    let acquisitions = validate_function(&program, function, &types)
        .expect("written View of original owned producer");
    assert_eq!(
        acquisitions.owner_initializers().count(),
        0,
        "backing is not a source Binding acquisition"
    );
    let loan = function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .find_map(|statement| match statement.kind {
            S::BeginCallView { local, .. } => Some(local),
            _ => None,
        })
        .expect("direct produced view stage");
    let owner = function
        .local(loan)
        .unwrap()
        .view_source
        .expect("exact backing owner");
    assert!(function.local(owner).unwrap().view_source.is_none());
    let produce = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "produce")
        .unwrap()
        .id;
    assert_eq!(function.blocks.iter().flat_map(|block| &block.statements).filter(|statement| matches!(&statement.kind,
        S::Let { local, value } if *local == owner && matches!(value.kind, E::Call { function, .. } if function == produce))).count(), 1);
    for malformed in [
        "renamed binding",
        "missing initializer",
        "duplicate initializer",
    ] {
        let mut changed = program.clone();
        let function = exercise_mut(&mut changed);
        let (block, index) = function.blocks.iter().enumerate().find_map(|(block, value)|
            value.statements.iter().position(|statement| matches!(statement.kind, S::Let { local, .. } if local == owner))
                .map(|index| (block, index))).expect("owning producer initializer");
        match malformed {
            "renamed binding" => {
                let spare = function.params[0].local;
                let S::Let { value, .. } = &mut function.blocks[block].statements[index].kind
                else {
                    panic!("owner Let");
                };
                value.kind = E::Local(spare);
            }
            "missing initializer" => {
                function.blocks[block].statements.remove(index);
            }
            "duplicate initializer" => {
                let duplicate = function.blocks[block].statements[index].clone();
                function.blocks[block].statements.insert(index, duplicate);
            }
            _ => unreachable!(),
        }
        rejects(&changed, &types);
    }
}

#[test]
fn caller_acquisition_handled_storage_keeps_sealed_occurrence_separate_from_actual() {
    let source = r#"namespace app
function read(view values: secret[list[int64]], extra: int64) returns int64:
    return extra
function exercise(incoming: optional[list[int64]]) returns int64:
    return read((incoming handle:
        default list(7)
    ), 3)
"#;
    let (program, types) = source_program(source);
    let function = exercise(&program);
    let acquisitions =
        validate_function(&program, function, &types).expect("handled Secret-qualified occurrence");
    let (owner, _) = owning_stage(function);
    let source = acquisitions
        .owner_initializer(owner)
        .expect("produced owning acquisition");
    assert!(matches!(types.resolve(source.actual_type), Type::List(_)));
    assert!(
        matches!(types.resolve(source.storage_type), Type::Secret(inner) if *inner == source.actual_type)
    );
    assert_eq!(source.binding, None);
    assert_eq!(source.effect, CheckedCallerEffect::RelinquishOwned);
    let mut changed = program.clone();
    let E::Call {
        ownership: hir::CallOwnership::Source(packet),
        ..
    } = &mut read_call_mut(exercise_mut(&mut changed)).kind
    else {
        panic!("qualified source read");
    };
    packet.arguments[0].actual_type = source.storage_type;
    rejects(&changed, &types);
}

const ABSENT_SUCCESS_SOURCE: &str = r#"namespace app
function read(view values: list[int64], extra: int64) returns int64:
    return extra
function exercise(values: list[int64]) returns int64:
    return read(values, (none handle:
        default 3
    ))
"#;

fn absent_original_plan(program: &Program, types: &TypeInterner) -> AbsentSuccessPlan {
    let proof = validate_function(program, exercise(program), types)
        .expect("typed original absent success");
    let [plan] = proof.absent_successes.as_slice() else {
        panic!("one exact absent-success plan");
    };
    plan.clone()
}

#[test]
fn caller_acquisition_absent_success_is_original_metadata_then_private_prepared_proof() {
    for source in [
        ABSENT_SUCCESS_SOURCE.to_owned(),
        ABSENT_SUCCESS_SOURCE.replace("none handle:", "fail(\"missing\") handle error:"),
    ] {
        let (mut program, types) = source_program(&source);
        let plan = absent_original_plan(&program, &types);
        assert!(
            matches!(types.resolve(plan.source_type), Type::Optional(inner) if *inner == TypeInterner::NEVER)
                || matches!(types.resolve(plan.source_type), Type::Result(ok, error) if *ok == TypeInterner::NEVER && *error == TypeInterner::STRING)
        );
        assert_eq!(plan.output_type, TypeInterner::INT64);
        assert!(exercise(&program).prepared_absent_successes.is_empty());
        crate::prepare_native_sequences(&mut program, &types);
        crate::prepare_native_uninhabited_sums(&mut program, &types);
        crate::prepare_native_generated_functions(&mut program, &types);
        crate::validate_call_ownership(&program, &types).expect("all prepared caller proofs");
        let function = exercise(&program);
        let [record] = function.prepared_absent_successes.as_slice() else {
            panic!("one private preparation certificate");
        };
        let prepared = record.plan();
        assert_eq!(prepared.function, function.id);
        assert_eq!(prepared.identity, function.identity);
        assert_eq!(prepared.source_type, plan.source_type);
        assert_eq!(prepared.output_type, plan.output_type);
        assert_eq!(prepared.span, plan.span);
        assert!(
            matches!(function.blocks[prepared.selecting.index() as usize].terminator.kind,
            T::Goto(target) if target == prepared.failure)
        );
        assert!(!function.blocks.iter().flat_map(|block| &block.statements).any(|statement|
            matches!(statement.kind, S::SumTake { target, success: true, .. } if target == prepared.output)));
        assert!(
            matches!(function.blocks[prepared.selecting.index() as usize].statements.last().unwrap().kind,
            S::SumTag { source, target } if source == prepared.source && target == prepared.tag)
        );
        let original = program.clone();
        crate::prepare_native_uninhabited_sums(&mut program, &types);
        assert_eq!(
            program, original,
            "a second preparation never duplicates authority"
        );
    }
}

#[test]
fn caller_acquisition_pending_sum_receiver_and_nested_absent_defaults_keep_order() {
    let pending = ABSENT_SUCCESS_SOURCE.replace(
        "    return read(values, (none handle:",
        "    optional[int64] incoming = run run none\n    return read(values, (incoming handle:",
    );
    let (mut program, types) = source_program(&pending);
    let proof = validate_function(&program, exercise(&program), &types)
        .expect("typed inhabited pending receiver");
    assert!(
        proof.absent_successes.is_empty(),
        "an inhabited Optional[int64] never receives absent-success authority"
    );
    crate::prepare_native_uninhabited_sums(&mut program, &types);
    crate::validate_call_ownership(&program, &types)
        .expect("pending receiver remains an ordinary typed sum");
    let function = exercise(&program);
    assert!(function.prepared_absent_successes.is_empty());
    let (pending_block, pending_position, incoming) = function.blocks.iter().find_map(|block| {
        block.statements.iter().enumerate().find_map(|(position, statement)| match &statement.kind {
            S::Let { local, value } if matches!(&value.kind, E::Run(first) if matches!(first.kind, E::Run(_))) => Some((block.id, position, *local)),
            _ => None,
        })
    }).expect("both original pending layers remain executable");
    assert!(
        matches!(types.resolve(function.local(incoming).unwrap().ty), Type::Optional(inner) if *inner == TypeInterner::INT64)
    );
    let (block, source, tag, tag_position) =
        function
            .blocks
            .iter()
            .find_map(|block| {
                block.statements.iter().enumerate().find_map(
                    |(position, statement)| match statement.kind {
                        S::SumTag { source, target } => Some((block, source, target, position)),
                        _ => None,
                    },
                )
            })
            .expect("ordinary runtime tag remains");
    let source_position = block
        .statements
        .iter()
        .position(|statement| matches!(statement.kind, S::Let { local, .. } if local == source))
        .expect("owned sum source remains");
    assert_eq!(block.id, pending_block);
    assert!(
        pending_position < source_position && source_position < tag_position,
        "original pending creation and ordinary snapshot precede the runtime tag"
    );
    let S::Let { value, .. } = &block.statements[source_position].kind else {
        unreachable!()
    };
    assert!(
        matches!(&value.kind, E::Clone(inner) if matches!(inner.kind, E::Local(local) if local == incoming)),
        "the existing handle snapshot preserves the pending receiver"
    );
    let T::Branch {
        condition,
        then_block,
        else_block,
    } = &block.terminator.kind
    else {
        panic!("inhabited sum retains its branch");
    };
    assert!(matches!(condition.kind, E::Local(local) if local == tag));
    assert_ne!(then_block, else_block);
    assert!(function.blocks[then_block.index() as usize].statements.iter().any(|statement|
        matches!(statement.kind, S::SumTake { source: owner, success: true, .. } if owner == source)),
        "inhabited success extraction remains");

    let nested = ABSENT_SUCCESS_SOURCE.replace("default 3", "optional[int64] fallback = none\n        default (fallback handle:\n            default 4\n        )");
    let (mut program, types) = source_program(&nested);
    absent_original_plan(&program, &types);
    crate::prepare_native_uninhabited_sums(&mut program, &types);
    crate::validate_call_ownership(&program, &types).expect("nested prepared original occurrence");
    let function = exercise(&program);
    let [record] = function.prepared_absent_successes.as_slice() else {
        panic!("one exact nested absent-success certificate");
    };
    let plan = record.plan();
    let block = &function.blocks[plan.selecting.index() as usize];
    let source_position = block
        .statements
        .iter()
        .position(
            |statement| matches!(statement.kind, S::Let { local, .. } if local == plan.source),
        )
        .expect("owning sum source remains executable");
    let tag_position = block.statements.iter().position(|statement|
        matches!(statement.kind, S::SumTag { source, target } if source == plan.source && target == plan.tag)).unwrap();
    assert!(
        source_position < tag_position,
        "source evaluation precedes the runtime sum guard"
    );
    assert_ne!(
        plan.selecting, plan.failure,
        "nested defaults remain after the exact false edge"
    );
    let inner = function.blocks.iter().find(|block|
        block.id != plan.selecting && matches!(&block.terminator.kind, T::Branch { .. })
            && block.statements.iter().any(|statement| matches!(statement.kind,
                S::SumTag { source, .. } if matches!(types.resolve(function.local(source).unwrap().ty),
                    Type::Optional(payload) if *payload == TypeInterner::INT64))))
        .expect("typed nested fallback retains its inhabited branch");
    let T::Branch { then_block, .. } = inner.terminator.kind else {
        unreachable!()
    };
    assert!(
        function.blocks[then_block.index() as usize]
            .statements
            .iter()
            .any(|statement| matches!(statement.kind, S::SumTake { success: true, .. })),
        "inner inhabited success extraction remains after outer absence preparation"
    );
}

#[test]
fn caller_acquisition_absent_success_rejects_original_forged_tag_default_and_manual_goto() {
    let (program, types) = source_program(ABSENT_SUCCESS_SOURCE);
    let plan = absent_original_plan(&program, &types);
    for malformed in [
        "tag type",
        "same branches",
        "false extraction",
        "foreign default",
        "manual goto",
    ] {
        let mut changed = program.clone();
        let function = exercise_mut(&mut changed);
        match malformed {
            "tag type" => function.locals[plan.tag.index() as usize].ty = TypeInterner::INT64,
            "same branches" => {
                let T::Branch {
                    then_block,
                    else_block,
                    ..
                } = &mut function.blocks[plan.selecting.index() as usize]
                    .terminator
                    .kind
                else {
                    panic!("original branch");
                };
                *then_block = *else_block;
            }
            "false extraction" => {
                let statement = function.blocks.iter_mut().flat_map(|block| &mut block.statements).find(|statement|
                    matches!(statement.kind, S::SumTake { target, success: true, .. } if target == plan.output)).unwrap();
                let S::SumTake { success, .. } = &mut statement.kind else {
                    panic!("anchor");
                };
                *success = false;
            }
            "foreign default" => {
                let statement = function.blocks.iter_mut().flat_map(|block| &mut block.statements).find(|statement|
                    matches!(statement.kind, S::Let { local, .. } if local == plan.output)).unwrap();
                statement.span.file = jett_common::FileId::new(u32::MAX);
            }
            "manual goto" => {
                function.blocks[plan.selecting.index() as usize]
                    .terminator
                    .kind = T::Goto(plan.failure);
                crate::sequences::prune::unreachable(function);
                assert!(function.prepared_absent_successes.is_empty());
            }
            _ => unreachable!(),
        }
        rejects(&changed, &types);
        let before = changed.clone();
        crate::prepare_native_uninhabited_sums(&mut changed, &types);
        assert_eq!(
            changed, before,
            "bad original graph is never rescued by pruning"
        );
    }
}

#[test]
fn caller_acquisition_absent_success_rejoins_every_private_prepared_record() {
    let (mut program, types) = source_program(ABSENT_SUCCESS_SOURCE);
    crate::prepare_native_uninhabited_sums(&mut program, &types);
    crate::validate_call_ownership(&program, &types).unwrap();
    let plan = exercise(&program).prepared_absent_successes[0]
        .plan()
        .clone();
    for malformed in [
        "missing record",
        "duplicate record",
        "changed goto",
        "missing tag",
        "wrong output type",
        "foreign default",
        "disconnected default",
    ] {
        let mut changed = program.clone();
        let function = exercise_mut(&mut changed);
        match malformed {
            "missing record" => function.prepared_absent_successes.clear(),
            "duplicate record" => function
                .prepared_absent_successes
                .push(function.prepared_absent_successes[0].clone()),
            "changed goto" => {
                function.blocks[plan.selecting.index() as usize]
                    .terminator
                    .kind = T::Goto(plan.selecting)
            }
            "missing tag" => {
                function.blocks[plan.selecting.index() as usize].statements.retain(|statement|
                    !matches!(statement.kind, S::SumTag { source, target } if source == plan.source && target == plan.tag));
            }
            "wrong output type" => {
                function.locals[plan.output.index() as usize].debug_ty = TypeInterner::BOOL
            }
            "foreign default" => {
                let statement = function.blocks.iter_mut().flat_map(|block| &mut block.statements).find(|statement|
                    matches!(statement.kind, S::Let { local, .. } if local == plan.output)).unwrap();
                statement.span.file = jett_common::FileId::new(u32::MAX);
            }
            "disconnected default" => {
                let statement = function.blocks.iter().flat_map(|block| &block.statements).find(|statement|
                    matches!(statement.kind, S::Let { local, .. } if local == plan.output)).unwrap().clone();
                function.blocks.push(crate::BasicBlock {
                    id: BlockId(function.blocks.len() as u32),
                    statements: vec![statement],
                    terminator: crate::Terminator {
                        span: function.span,
                        kind: T::Unreachable,
                    },
                });
            }
            _ => unreachable!(),
        }
        rejects(&changed, &types);
    }
}

#[test]
fn caller_acquisition_absent_success_certificates_follow_real_dense_remapping() {
    let source = ABSENT_SUCCESS_SOURCE.replace("    return read(values,", "    function(int64) returns int64 callback = function(ignored: int64) returns int64:\n        return ignored\n    int64 first = callback(0)\n    return read(values,");
    let (mut program, types) = source_program(&source);
    let original = absent_original_plan(&program, &types);
    crate::prepare_native_uninhabited_sums(&mut program, &types);
    let after_sum = exercise(&program).prepared_absent_successes[0]
        .plan()
        .clone();
    assert_ne!(
        after_sum.failure, original.failure,
        "removed success block changes the failure ID"
    );
    assert_ne!(
        after_sum.source, original.source,
        "detached inline formal changes the owner ID"
    );
    crate::prepare_native_generated_functions(&mut program, &types);
    crate::prepare_native_sequences(&mut program, &types);
    crate::validate_call_ownership(&program, &types)
        .expect("remapped certificate joins actual storage and graph");
    let function = exercise(&program);
    let plan = function.prepared_absent_successes[0].plan();
    assert_eq!(function.local(plan.source).unwrap().ty, plan.source_type);
    assert_eq!(function.local(plan.output).unwrap().ty, plan.output_type);
    assert_eq!(function.local(plan.tag).unwrap().ty, TypeInterner::BOOL);
    assert!(
        matches!(function.blocks[plan.selecting.index() as usize].terminator.kind, T::Goto(target) if target == plan.failure)
    );
}

const GENERATED_SNAPSHOT_SOURCE: &str = r#"interface Displayable:
    function display(view self: Displayable) returns string
namespace app
struct Item:
    value: int64
implement Displayable for Item:
    function display(view self: Item) returns string:
        return "shown"
function exercise(view item: optional[Item], view spare: Item) returns string:
    return "{item handle: default Item(value: 7)}"
"#;

const GENERATED_PRODUCER_SOURCE: &str = r#"interface Displayable:
    function display(view self: Displayable) returns string
namespace app
struct Item:
    value: int64
implement Displayable for Item:
    function display(view self: Item) returns string:
        return "shown"
function make(value: int64) returns Item:
    return Item(value: value)
function other(value: int64) returns Item:
    return Item(value: value)
function exercise(incoming: optional[int64]) returns string:
    return "{make(incoming handle: default 7)}"
"#;

fn generated_in_expression(value: &Expression) -> Option<&Expression> {
    if matches!(
        &value.kind,
        E::Call {
            ownership: hir::CallOwnership::Generated(_),
            ..
        } | E::IndirectCall {
            ownership: hir::CallOwnership::Generated(_),
            ..
        } | E::Intrinsic {
            ownership: hir::CallOwnership::Generated(_),
            ..
        }
    ) {
        return Some(value);
    }
    match &value.kind {
        E::View(inner)
        | E::Clone(inner)
        | E::Coarsen(inner)
        | E::Declassify(inner)
        | E::Run(inner)
        | E::Join(inner)
        | E::RefinementValidated(inner)
        | E::DisplayResult(inner)
        | E::EquatableResult(inner) => generated_in_expression(inner),
        E::StringInterpolation(parts) => parts.iter().find_map(|part| match part {
            hir::StringSegment::Value(value) => generated_in_expression(value),
            _ => None,
        }),
        E::Call { args, .. } | E::IndirectCall { args, .. } | E::Intrinsic { args, .. } => {
            args.iter().find_map(generated_in_expression)
        }
        _ => None,
    }
}

fn generated_in_expression_mut(value: &mut Expression) -> Option<&mut Expression> {
    if matches!(
        &value.kind,
        E::Call {
            ownership: hir::CallOwnership::Generated(_),
            ..
        } | E::IndirectCall {
            ownership: hir::CallOwnership::Generated(_),
            ..
        } | E::Intrinsic {
            ownership: hir::CallOwnership::Generated(_),
            ..
        }
    ) {
        return Some(value);
    }
    match &mut value.kind {
        E::View(inner)
        | E::Clone(inner)
        | E::Coarsen(inner)
        | E::Declassify(inner)
        | E::Run(inner)
        | E::Join(inner)
        | E::RefinementValidated(inner)
        | E::DisplayResult(inner)
        | E::EquatableResult(inner) => generated_in_expression_mut(inner),
        E::StringInterpolation(parts) => parts.iter_mut().find_map(|part| match part {
            hir::StringSegment::Value(value) => generated_in_expression_mut(value),
            _ => None,
        }),
        E::Call { args, .. } | E::IndirectCall { args, .. } | E::Intrinsic { args, .. } => {
            args.iter_mut().find_map(generated_in_expression_mut)
        }
        _ => None,
    }
}

fn generated_call(function: &Function) -> &Expression {
    function
        .blocks
        .iter()
        .find_map(|block| {
            block
                .statements
                .iter()
                .find_map(|statement| match &statement.kind {
                    S::Let { value, .. } | S::Evaluate(value) | S::HandleDefault(value) => {
                        generated_in_expression(value)
                    }
                    S::CheckRefinement { call, .. } => generated_in_expression(call),
                    _ => None,
                })
                .or_else(|| match &block.terminator.kind {
                    T::Return(Some(value)) => generated_in_expression(value),
                    _ => None,
                })
        })
        .expect("source-derived generated invocation")
}

fn generated_call_mut(function: &mut Function) -> &mut Expression {
    for block in &mut function.blocks {
        for statement in &mut block.statements {
            let found = match &mut statement.kind {
                S::Let { value, .. } | S::Evaluate(value) | S::HandleDefault(value) => {
                    generated_in_expression_mut(value)
                }
                S::CheckRefinement { call, .. } => generated_in_expression_mut(call),
                _ => None,
            };
            if let Some(found) = found {
                return found;
            }
        }
        if let T::Return(Some(value)) = &mut block.terminator.kind
            && let Some(found) = generated_in_expression_mut(value)
        {
            return found;
        }
    }
    panic!("source-derived generated invocation");
}

fn generated_argument(function: &Function) -> &hir::GeneratedArgumentOwnership {
    let E::Call {
        ownership: hir::CallOwnership::Generated(call),
        ..
    } = &generated_call(function).kind
    else {
        panic!("generated direct call");
    };
    &call.arguments[0]
}

fn generated_owner_stage(function: &Function) -> (LocalId, LocalId) {
    match generated_argument(function).staging {
        hir::GeneratedArgumentStaging::OwnedProducer { owner, loan }
        | hir::GeneratedArgumentStaging::OrdinarySnapshot { owner, loan } => (owner, loan),
        other => panic!("generated owning stage: {other:?}"),
    }
}

fn generated_initializer_mut(function: &mut Function, owner: LocalId) -> &mut Expression {
    function
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.statements)
        .find_map(|statement| match &mut statement.kind {
            S::Let { local, value } if *local == owner => Some(value),
            _ => None,
        })
        .expect("generated owning initializer")
}

fn generated_rejects(program: &Program, types: &TypeInterner) -> String {
    let error = validate_function(program, exercise(program), types)
        .expect_err("malformed generated storage must fail");
    assert!(
        error.contains("call ownership")
            || error.contains("call view")
            || error.contains("generated")
            || error.contains("sealed"),
        "{error}"
    );
    error
}

#[test]
fn caller_acquisition_generated_snapshot_keeps_handled_origin_and_empty_source_acquisitions() {
    let (mut program, types) = source_program(GENERATED_SNAPSHOT_SOURCE);
    let function = exercise(&program);
    let argument = generated_argument(function);
    assert!(
        argument
            .original_witness()
            .ordinary_snapshot_proof()
            .is_some()
    );
    assert!(!argument.original_witness().owned_producer());
    assert!(matches!(
        argument.staging,
        hir::GeneratedArgumentStaging::OrdinarySnapshot { .. }
    ));
    let (owner, loan) = generated_owner_stage(function);
    let initializer = function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .find_map(|statement| match &statement.kind {
            S::Let { local, value } if *local == owner => Some(value),
            _ => None,
        })
        .expect("snapshot initializer");
    assert!(matches!(initializer.kind, E::Clone(_)));
    assert_eq!(function.local(loan).expect("loan").view_source, Some(owner));
    let acquisitions =
        validate_function(&program, function, &types).expect("generated handled snapshot");
    assert_eq!(
        acquisitions.owner_initializers().count(),
        0,
        "generated copy must not consume a source binding"
    );
    crate::prepare_native_uninhabited_sums(&mut program, &types);
    crate::prepare_native_generated_functions(&mut program, &types);
    validate_function(&program, exercise(&program), &types).expect("prepared generated snapshot");
}

#[test]
fn caller_acquisition_generated_snapshot_rejects_lost_clone_and_same_type_borrow_substitution() {
    let (program, types) = source_program(GENERATED_SNAPSHOT_SOURCE);
    let (owner, _) = generated_owner_stage(exercise(&program));
    let mut changed = program.clone();
    let initializer = generated_initializer_mut(exercise_mut(&mut changed), owner);
    let E::Clone(inner) = &initializer.kind else {
        panic!("snapshot clone");
    };
    *initializer = inner.as_ref().clone();
    generated_rejects(&changed, &types);

    let mut changed = program.clone();
    let spare = exercise(&changed).params[1].local;
    let initializer = generated_initializer_mut(exercise_mut(&mut changed), owner);
    let E::Clone(observed) = &mut initializer.kind else {
        panic!("snapshot clone");
    };
    let E::View(value) = &mut observed.kind else {
        panic!("physical observed view");
    };
    value.kind = E::Local(spare);
    generated_rejects(&changed, &types);

    let mut changed = program.clone();
    let mut replacement = exercise(&changed)
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .find_map(|statement| match &statement.kind {
            S::Let { value, .. } if matches!(value.kind, E::StructConstruct { .. }) => {
                Some(value.clone())
            }
            _ => None,
        })
        .expect("typed default constructor");
    let initializer = generated_initializer_mut(exercise_mut(&mut changed), owner);
    let E::Clone(observed) = &mut initializer.kind else {
        panic!("snapshot clone");
    };
    let E::View(value) = &mut observed.kind else {
        panic!("physical observed view");
    };
    replacement.span = value.span;
    assert_eq!(replacement.ty, value.ty);
    **value = replacement;
    generated_rejects(&changed, &types);
}

#[test]
fn caller_acquisition_generated_storage_rejects_duplicate_owner_missing_end_and_public_relabel() {
    let (program, types) = source_program(GENERATED_SNAPSHOT_SOURCE);
    let (owner, loan) = generated_owner_stage(exercise(&program));
    let mut changed = program.clone();
    let function = exercise_mut(&mut changed);
    let (block, index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, value)| {
            value
                .statements
                .iter()
                .position(
                    |statement| matches!(statement.kind, S::Let { local, .. } if local == owner),
                )
                .map(|index| (block, index))
        })
        .expect("owner definition");
    let duplicate = function.blocks[block].statements[index].clone();
    function.blocks[block].statements.insert(index, duplicate);
    generated_rejects(&changed, &types);

    let mut changed = program.clone();
    for block in &mut exercise_mut(&mut changed).blocks {
        block.statements.retain(
            |statement| !matches!(statement.kind, S::EndCallView { local } if local == loan),
        );
    }
    generated_rejects(&changed, &types);

    let mut changed = program.clone();
    let E::Call {
        ownership: hir::CallOwnership::Generated(call),
        ..
    } = &mut generated_call_mut(exercise_mut(&mut changed)).kind
    else {
        panic!("generated call");
    };
    call.arguments[0].staging = hir::GeneratedArgumentStaging::OwnedProducer { owner, loan };
    generated_rejects(&changed, &types);
}

#[test]
fn caller_acquisition_generated_owned_producer_rejects_added_clone_and_wrong_exact_target() {
    let (program, types) = source_program(GENERATED_PRODUCER_SOURCE);
    let function = exercise(&program);
    let argument = generated_argument(function);
    assert!(argument.original_witness().owned_producer());
    assert!(matches!(
        argument.staging,
        hir::GeneratedArgumentStaging::OwnedProducer { .. }
    ));
    let (owner, _) = generated_owner_stage(function);
    validate_function(&program, function, &types).expect("complete generated producer");

    let mut changed = program.clone();
    let initializer = generated_initializer_mut(exercise_mut(&mut changed), owner);
    let original = initializer.clone();
    initializer.kind = E::Clone(Box::new(original));
    generated_rejects(&changed, &types);

    let mut changed = program.clone();
    let other = changed
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "other")
        .expect("same-signature control")
        .id;
    let initializer = generated_initializer_mut(exercise_mut(&mut changed), owner);
    let E::Call { function, .. } = &mut initializer.kind else {
        panic!("full endpoint producer");
    };
    *function = other;
    generated_rejects(&changed, &types);
}

#[test]
fn caller_acquisition_generated_explicit_clone_and_constructor_producers_remain_distinct() {
    for source in [
        GENERATED_PRODUCER_SOURCE.replace("{make(incoming", "{clone make(incoming"),
        GENERATED_PRODUCER_SOURCE.replace(
            "{make(incoming handle: default 7)}",
            "{Item(value: incoming handle: default 7)}",
        ),
    ] {
        let (program, types) = source_program(&source);
        let function = exercise(&program);
        let argument = generated_argument(function);
        assert!(argument.original_witness().owned_producer());
        assert!(matches!(
            argument.staging,
            hir::GeneratedArgumentStaging::OwnedProducer { .. }
        ));
        validate_function(&program, function, &types)
            .expect("original explicit Clone or exact constructor producer");
    }
}

#[test]
fn caller_acquisition_generated_constructor_restores_each_exact_materialized_field() {
    let source = GENERATED_PRODUCER_SOURCE
        .replace(
            "    value: int64\n",
            "    value: int64\n    marker: int64\n",
        )
        .replace("Item(value: value)", "Item(value: value, marker: 1)")
        .replace(
            "{make(incoming handle: default 7)}",
            "{Item(value: incoming handle: default 7, marker: 2)}",
        );
    let (program, types) = source_program(&source);
    let function = exercise(&program);
    let (owner, _) = generated_owner_stage(function);
    validate_function(&program, function, &types)
        .expect("handled and primitive fields preserve their original nodes");
    let field = function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .find_map(|statement| match &statement.kind {
            S::Let {
                local,
                value:
                    Expression {
                        kind: E::StructConstruct { fields, .. },
                        ..
                    },
            } if *local == owner => {
                assert_eq!(fields.len(), 2);
                let E::Local(field) = fields[1].kind else {
                    panic!("second field materialized at lexical position");
                };
                Some(field)
            }
            _ => None,
        })
        .expect("complete constructor initializer");

    let mut changed = program.clone();
    let initializer = generated_initializer_mut(exercise_mut(&mut changed), field);
    let original = initializer.clone();
    initializer.kind = E::Clone(Box::new(original));
    generated_rejects(&changed, &types);

    let mut changed = program.clone();
    let initializer = generated_initializer_mut(exercise_mut(&mut changed), field);
    initializer.kind = E::Local(field);
    generated_rejects(&changed, &types);
}

#[test]
fn caller_acquisition_generated_storage_and_private_association_follow_dense_local_remapping() {
    let source = GENERATED_SNAPSHOT_SOURCE.replace("    return \"{item handle:",
        "    function(int64) returns int64 callback = function(ignored: int64) returns int64:\n        return ignored\n    int64 first = callback(0)\n    return \"{item handle:");
    let (mut program, types) = source_program(&source);
    let before = generated_owner_stage(exercise(&program));
    crate::prepare_native_generated_functions(&mut program, &types);
    let function = exercise(&program);
    let after = generated_owner_stage(function);
    assert_ne!(
        after, before,
        "detached inline formal must force real stage ID remapping"
    );
    assert_eq!(
        function.local(after.1).expect("remapped loan").view_source,
        Some(after.0)
    );
    validate_function(&program, function, &types)
        .expect("all original and private Generated stage IDs remapped");
}

#[test]
fn caller_acquisition_generated_owned_copy_storage_requires_one_exact_original_initializer() {
    let source = r#"namespace app
type Positive = int64 where value > 0
function exercise(raw: int64) returns Positive:
    return raw
"#;
    let (mut program, types) = source_program(source);
    let function = exercise_mut(&mut program);
    let (block, index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, value)| {
            value
                .statements
                .iter()
                .position(|statement| matches!(statement.kind, S::CheckRefinement { .. }))
                .map(|index| (block, index))
        })
        .expect("refinement check");
    let (original, span) = {
        let S::CheckRefinement { call, .. } = &function.blocks[block].statements[index].kind else {
            panic!("predicate");
        };
        let E::Call {
            args,
            ownership: hir::CallOwnership::Generated(packet),
            ..
        } = &call.kind
        else {
            panic!("generated predicate");
        };
        assert_eq!(
            packet.arguments[0].acquisition,
            hir::GeneratedAcquisition::Copy
        );
        assert_eq!(
            packet.arguments[0].callee_access,
            jett_typecheck::CheckedCalleeAccess::Owned
        );
        assert_eq!(
            packet.arguments[0].staging,
            hir::GeneratedArgumentStaging::Existing { loan: None }
        );
        (args[0].clone(), args[0].span)
    };
    let owner = LocalId::new(function.locals.len() as u32);
    let mut metadata = function.locals[function.params[0].local.index() as usize].clone();
    metadata.id = owner;
    metadata.name = "isolated_materialized_operand".into();
    metadata.ty = original.ty;
    metadata.debug_ty = original.ty;
    metadata.mutable = true;
    metadata.view_source = None;
    metadata.span = span;
    function.locals.push(metadata);
    let S::CheckRefinement { call, .. } = &mut function.blocks[block].statements[index].kind else {
        panic!("predicate");
    };
    let E::Call { args, .. } = &mut call.kind else {
        panic!("call");
    };
    args[0] = Expression {
        kind: E::Local(owner),
        ty: original.ty,
        span,
    };
    function.blocks[block].statements.insert(
        index,
        crate::Statement {
            span,
            kind: S::Let {
                local: owner,
                value: original,
            },
        },
    );
    let acquisitions = validate_function(&program, exercise(&program), &types)
        .expect("finite owned Copy materialization");
    assert_eq!(acquisitions.owner_initializers().count(), 0);

    let mut changed = program.clone();
    let function = exercise_mut(&mut changed);
    let duplicate = function.blocks[block].statements[index].clone();
    function.blocks[block].statements.insert(index, duplicate);
    generated_rejects(&changed, &types);

    let mut changed = program.clone();
    exercise_mut(&mut changed).locals[owner.index() as usize].debug_ty = TypeInterner::BOOL;
    generated_rejects(&changed, &types);
}

const PRESENT_SUCCESS_SOURCE: &str = r#"namespace app
function read(view values: list[int64], extra: int64) returns int64:
    return extra
function exercise[E](incoming: result[list[int64], E]) returns int64:
    return read((incoming handle error:
        default list(7)
    ), 3)
function main() returns nothing:
    int64 answer = exercise(ok(list(1, 2)))
    return nothing
"#;

fn present_original_plan(program: &Program, types: &TypeInterner) -> PresentSuccessPlan {
    let proof = validate_function(program, exercise(program), types)
        .expect("typed original inhabited-success edge");
    let [plan] = proof.present_successes.as_slice() else {
        panic!("one exact present-success plan");
    };
    plan.clone()
}

#[test]
fn caller_acquisition_present_success_preserves_tag_and_owned_payload_without_default_authority() {
    for source in [
        PRESENT_SUCCESS_SOURCE.to_owned(),
        PRESENT_SUCCESS_SOURCE.replace(
            "read(view values: list[int64]",
            "read(view values: secret[list[int64]]",
        ),
    ] {
        let (mut program, types) = source_program(&source);
        let original = present_original_plan(&program, &types);
        assert!(
            matches!(types.resolve(original.source_type), Type::Result(ok, error)
            if *ok != TypeInterner::NEVER && *error == TypeInterner::NEVER)
        );
        assert!(exercise(&program).prepared_present_successes.is_empty());
        crate::prepare_native_uninhabited_sums(&mut program, &types);
        crate::validate_call_ownership(&program, &types)
            .expect("canonical prepared success retains acquisition authority");
        let function = exercise(&program);
        let [record] = function.prepared_present_successes.as_slice() else {
            panic!("one private present-success record");
        };
        let plan = record.plan();
        assert_eq!(
            (
                plan.function,
                &plan.identity,
                plan.source_type,
                plan.output_type,
                plan.span
            ),
            (
                function.id,
                &function.identity,
                original.source_type,
                original.output_type,
                original.span
            )
        );
        assert!(function.prepared_absent_successes.is_empty());
        assert!(
            matches!(function.blocks[plan.selecting.index() as usize].terminator.kind,
            T::Goto(target) if target == plan.success)
        );
        assert!(
            matches!(function.blocks[plan.selecting.index() as usize].statements.last().unwrap().kind,
            S::SumTag { source, target } if source == plan.source && target == plan.tag)
        );
        let extractions = function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .filter(|statement| {
                matches!(statement.kind, S::SumTake { source, target, success: true }
                if source == plan.source && target == plan.output)
            })
            .count();
        assert_eq!(
            extractions, 1,
            "inhabited successful payload extraction stays executable"
        );
        assert!(!function.blocks.iter().flat_map(|block| &block.statements).any(|statement|
            matches!(statement.kind, S::Let { local, .. } if local == plan.output)), "erased default gains no prepared authority");
        crate::move_values::MoveValuePlan::analyze(&program, function, &types)
            .expect("successful payload/source ownership and cleanup remain exact");
        let prepared = program.clone();
        crate::prepare_native_uninhabited_sums(&mut program, &types);
        assert_eq!(
            program, prepared,
            "repeated preparation cannot mint a second record from Goto"
        );
    }

    let control = PRESENT_SUCCESS_SOURCE.replace("int64 answer = exercise(ok(list(1, 2)))",
        "result[list[int64], string] incoming = ok(list(1, 2))\n    int64 answer = exercise(incoming)");
    let (mut program, types) = source_program(&control);
    assert!(
        validate_function(&program, exercise(&program), &types)
            .unwrap()
            .present_successes
            .is_empty()
    );
    crate::prepare_native_uninhabited_sums(&mut program, &types);
    crate::validate_call_ownership(&program, &types).unwrap();
    assert!(exercise(&program).prepared_present_successes.is_empty());
    assert!(
        exercise(&program)
            .blocks
            .iter()
            .any(|block| matches!(block.terminator.kind, T::Branch { .. })),
        "an inhabited error arm keeps its ordinary selecting Branch"
    );
}

#[test]
fn caller_acquisition_present_pending_receiver_keeps_original_snapshot_before_tag_and_take() {
    let source = PRESENT_SUCCESS_SOURCE.replace(
        "exercise(ok(list(1, 2)))",
        "exercise(run run ok(list(1, 2)))",
    );
    let (mut program, types) = source_program(&source);
    present_original_plan(&program, &types);
    let main = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "main")
        .unwrap();
    fn double_pending(value: &Expression) -> bool {
        match &value.kind {
            E::Run(first) if matches!(first.kind, E::Run(_)) => true,
            E::Call { args, .. } | E::Intrinsic { args, .. } => args.iter().any(double_pending),
            E::View(value) | E::Clone(value) => double_pending(value),
            _ => false,
        }
    }
    assert!(
        main.blocks
            .iter()
            .any(|block| block
                .statements
                .iter()
                .any(|statement| match &statement.kind {
                    S::Let { value, .. } | S::Evaluate(value) => double_pending(value),
                    _ => false,
                })),
        "both pending layers remain in the checked caller"
    );
    crate::prepare_native_uninhabited_sums(&mut program, &types);
    crate::validate_call_ownership(&program, &types)
        .expect("pending exact result keeps prepared source proof");
    let function = exercise(&program);
    let plan = function.prepared_present_successes[0].plan();
    let block = &function.blocks[plan.selecting.index() as usize];
    let owning = block
        .statements
        .iter()
        .position(|statement| {
            matches!(statement.kind,
        S::Let { local, .. } if local == plan.source)
        })
        .expect("original source owning snapshot");
    let tag = block
        .statements
        .iter()
        .position(|statement| {
            matches!(statement.kind,
        S::SumTag { source, target } if source == plan.source && target == plan.tag)
        })
        .unwrap();
    assert!(owning < tag);
    assert_eq!(tag + 1, block.statements.len());
    assert!(matches!(block.terminator.kind, T::Goto(target) if target == plan.success));
    assert!(
        function.blocks[plan.success.index() as usize]
            .statements
            .iter()
            .any(
                |statement| matches!(statement.kind, S::SumTake { source, target, success: true }
            if source == plan.source && target == plan.output)
            ),
        "pending guard remains before extraction"
    );
}

#[test]
fn caller_acquisition_present_success_refuses_original_manual_or_foreign_edges_before_pruning() {
    let (program, types) = source_program(PRESENT_SUCCESS_SOURCE);
    let plan = present_original_plan(&program, &types);
    for malformed in [
        "manual goto",
        "wrong tag type",
        "wrong tag owner",
        "same branches",
        "wrong extraction",
        "foreign incoming",
    ] {
        let mut changed = program.clone();
        let function = exercise_mut(&mut changed);
        match malformed {
            "manual goto" => {
                function.blocks[plan.selecting.index() as usize]
                    .terminator
                    .kind = T::Goto(plan.success);
                crate::sequences::prune::unreachable(function);
                assert!(function.prepared_present_successes.is_empty());
            }
            "wrong tag type" => function.locals[plan.tag.index() as usize].ty = TypeInterner::INT64,
            "wrong tag owner" => {
                let foreign = function.params[0].local;
                let S::SumTag { source, .. } = &mut function.blocks
                    [plan.selecting.index() as usize]
                    .statements
                    .last_mut()
                    .unwrap()
                    .kind
                else {
                    unreachable!()
                };
                *source = foreign;
            }
            "same branches" => {
                let T::Branch {
                    then_block,
                    else_block,
                    ..
                } = &mut function.blocks[plan.selecting.index() as usize]
                    .terminator
                    .kind
                else {
                    unreachable!()
                };
                *else_block = *then_block;
            }
            "wrong extraction" => {
                let statement = function.blocks[plan.success.index() as usize].statements.iter_mut().find(|statement|
                    matches!(statement.kind, S::SumTake { target, .. } if target == plan.output)).unwrap();
                let S::SumTake { success, .. } = &mut statement.kind else {
                    unreachable!()
                };
                *success = false;
            }
            "foreign incoming" => function.blocks.push(crate::BasicBlock {
                id: BlockId(function.blocks.len() as u32),
                statements: vec![],
                terminator: crate::Terminator {
                    span: plan.span,
                    kind: T::Goto(plan.success),
                },
            }),
            _ => unreachable!(),
        }
        rejects(&changed, &types);
        let before = changed.clone();
        crate::prepare_native_uninhabited_sums(&mut changed, &types);
        assert_eq!(
            changed, before,
            "canonical preparation never rescues an unproved original edge"
        );
    }
}

#[test]
fn caller_acquisition_present_success_rejoins_all_private_records_and_current_extraction() {
    let (mut program, types) = source_program(PRESENT_SUCCESS_SOURCE);
    crate::prepare_native_uninhabited_sums(&mut program, &types);
    crate::validate_call_ownership(&program, &types).unwrap();
    let plan = exercise(&program).prepared_present_successes[0]
        .plan()
        .clone();
    for malformed in [
        "missing record",
        "duplicate record",
        "wrong goto",
        "missing tag",
        "wrong output type",
        "foreign identity",
        "wrong extraction",
        "foreign incoming",
        "disconnected extraction",
    ] {
        let mut changed = program.clone();
        let function = exercise_mut(&mut changed);
        match malformed {
            "missing record" => function.prepared_present_successes.clear(),
            "duplicate record" => {
                let duplicate = function.prepared_present_successes[0].clone();
                function.prepared_present_successes.push(duplicate);
            }
            "wrong goto" => function.blocks[plan.selecting.index() as usize].terminator.kind = T::Goto(plan.selecting),
            "missing tag" => function.blocks[plan.selecting.index() as usize].statements.retain(|statement|
                !matches!(statement.kind, S::SumTag { source, target } if source == plan.source && target == plan.tag)),
            "wrong output type" => function.locals[plan.output.index() as usize].debug_ty = TypeInterner::BOOL,
            "foreign identity" => function.identity.declaration.namespace.push_str("_foreign"),
            "wrong extraction" => {
                let statement = function.blocks[plan.success.index() as usize].statements.iter_mut().find(|statement|
                    matches!(statement.kind, S::SumTake { target, .. } if target == plan.output)).unwrap();
                let S::SumTake { source, .. } = &mut statement.kind else { unreachable!() };
                *source = function.params[0].local;
            }
            "foreign incoming" => function.blocks.push(crate::BasicBlock {
                id: BlockId(function.blocks.len() as u32), statements: vec![],
                terminator: crate::Terminator { span: plan.span, kind: T::Goto(plan.success) },
            }),
            "disconnected extraction" => {
                let statement = function.blocks[plan.success.index() as usize].statements.iter().find(|statement|
                    matches!(statement.kind, S::SumTake { target, .. } if target == plan.output)).unwrap().clone();
                function.blocks.push(crate::BasicBlock { id: BlockId(function.blocks.len() as u32), statements: vec![statement],
                    terminator: crate::Terminator { span: plan.span, kind: T::Unreachable } });
            }
            _ => unreachable!(),
        }
        rejects(&changed, &types);
    }
}

#[test]
fn caller_acquisition_present_success_private_ids_follow_real_dense_remapping() {
    let source = PRESENT_SUCCESS_SOURCE.replace("    return read((incoming",
        "    function(int64) returns int64 callback = function(ignored: int64) returns int64:\n        return ignored\n    int64 first = callback(0)\n    return read((incoming");
    let (mut program, types) = source_program(&source);
    let original = present_original_plan(&program, &types);
    crate::prepare_native_uninhabited_sums(&mut program, &types);
    let after_sum = exercise(&program).prepared_present_successes[0]
        .plan()
        .clone();
    assert_ne!(
        after_sum.source, original.source,
        "unused extracted inline formal causes actual dense local remapping"
    );
    crate::prepare_native_generated_functions(&mut program, &types);
    crate::prepare_native_sequences(&mut program, &types);
    crate::validate_call_ownership(&program, &types)
        .expect("every private ID rejoins remapped actual graph and storage");
    let function = exercise(&program);
    let plan = function.prepared_present_successes[0].plan();
    assert_eq!(function.local(plan.source).unwrap().ty, plan.source_type);
    assert_eq!(function.local(plan.output).unwrap().ty, plan.output_type);
    assert_eq!(function.local(plan.tag).unwrap().ty, TypeInterner::BOOL);
    assert!(
        matches!(function.blocks[plan.selecting.index() as usize].terminator.kind, T::Goto(target) if target == plan.success)
    );
}

const CONVERTED_OBSERVATION_SOURCE: &str = r#"namespace app
interface Named:
    function name(view self: Named) returns string
struct Boxed[T]:
    value: T
implement Named for Boxed[string]:
    function name(view self: Boxed[string]) returns string:
        return "text"
function read(values: list[Named], expected: string) returns bool:
    return true
function count(view values: list[Boxed[string]]) returns int64:
    return 0
property observed:
    given values: list[Boxed[string]]
    given other: list[Boxed[string]]
    assert read(values, "text")
    assert read(clone values, "text")
    assert count(view values) == 0
"#;

fn observation_source_program(source: &str) -> (Program, TypeInterner) {
    let file = jett_common::FileId::new(0);
    let parsed = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    let checked = jett_typecheck::check(&parsed.module, &resolved);
    assert!(
        checked
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
        "{:?}",
        checked.diagnostics
    );
    let high = hir::lower_with_test_bodies(
        &parsed.module,
        &resolved,
        &checked,
        &HashMap::from([(file, jett_common::SourceOrigin::Project)]),
    )
    .expect("property HIR");
    (
        crate::lower(&high, &checked.interner).expect("property MIR"),
        checked.interner,
    )
}

fn observed_property(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.kind == hir::DeclarationKind::Property
                && function.identity.declaration.name.starts_with("observed:")
        })
        .expect("checked property")
}

fn observed_read_calls(function: &Function) -> Vec<&Expression> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .filter_map(|statement| {
            let S::Assert { condition, .. } = &statement.kind else {
                return None;
            };
            matches!(condition.kind, E::Call { .. }).then_some(condition)
        })
        .collect()
}

fn observation_owner(call: &Expression) -> (LocalId, &hir::ArgumentOwnership) {
    let E::Call {
        ownership: hir::CallOwnership::Source(source),
        args,
        ..
    } = &call.kind
    else {
        panic!("checked observed invocation");
    };
    let argument = &source.arguments[0];
    assert_eq!(argument.effect, CheckedCallerEffect::ObserveData);
    assert_eq!(
        argument.physical_access,
        jett_typecheck::CheckedCalleeAccess::Owned
    );
    let hir::ArgumentStaging::Observed { owner } = argument.staging else {
        panic!("observed owning slot");
    };
    assert!(matches!(args[0].kind, E::Local(local) if local == owner));
    (owner, argument)
}

fn observation_initializer(function: &Function, owner: LocalId) -> &Expression {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .find_map(|statement| match &statement.kind {
            S::Let { local, value } if *local == owner => Some(value),
            _ => None,
        })
        .expect("observed owning initializer")
}

#[test]
fn source_observation_snapshots_raw_generic_data_before_interface_conversion() {
    let (program, types) = observation_source_program(CONVERTED_OBSERVATION_SOURCE);
    let function = observed_property(&program);
    let calls = observed_read_calls(function);
    assert_eq!(calls.len(), 2);
    validate_function(&program, function, &types).expect("converted observed acquisition");
    crate::move_values::MoveValuePlan::analyze(&program, function, &types)
        .expect("original concrete owner stays available");
    for (index, call) in calls.iter().enumerate() {
        let (owner, argument) = observation_owner(call);
        let initializer = observation_initializer(function, owner);
        let E::InterfaceCoerce {
            value: snapshot, ..
        } = &initializer.kind
        else {
            panic!("converted snapshot retains its checked contextual conversion");
        };
        assert_ne!(initializer.ty, argument.actual_type);
        assert!(hir::observation_data_type(&types, argument.actual_type));
        assert!(!crate::handlers::can_snapshot_view(&types, initializer.ty));
        let E::Clone(original) = &snapshot.kind else {
            panic!("raw snapshot");
        };
        assert_eq!(snapshot.ty, argument.actual_type);
        assert_eq!(original.ty, argument.actual_type);
        assert_eq!(snapshot.span, argument.source_span);
        assert_eq!(original.span, argument.source_span);
        if index == 0 {
            assert!(matches!(original.kind, E::Local(local) if local == function.params[0].local));
        } else {
            assert!(
                matches!(&original.kind, E::Clone(inner)
                if matches!(inner.kind, E::Local(local) if local == function.params[0].local)),
                "the explicit source Clone remains inside the compiler snapshot"
            );
        }
    }
}

#[test]
fn source_observation_without_conversion_keeps_its_direct_snapshot_shape() {
    let source = CONVERTED_OBSERVATION_SOURCE.replace(
        "function read(values: list[Named]",
        "function read(values: list[Boxed[string]]",
    );
    let (program, types) = observation_source_program(&source);
    let function = observed_property(&program);
    let calls = observed_read_calls(function);
    assert_eq!(calls.len(), 2);
    for call in calls {
        let (owner, argument) = observation_owner(call);
        let initializer = observation_initializer(function, owner);
        let E::Clone(original) = &initializer.kind else {
            panic!("existing direct snapshot");
        };
        assert_eq!(initializer.ty, argument.actual_type);
        assert_eq!(original.ty, initializer.ty);
        assert!(crate::handlers::can_snapshot_view(&types, initializer.ty));
    }
    validate_function(&program, function, &types).expect("direct observed acquisition");
    crate::move_values::MoveValuePlan::analyze(&program, function, &types)
        .expect("direct observation retains its owner");
}

#[test]
fn source_observation_conversion_rejects_missing_erased_and_foreign_snapshots() {
    for mutation in 0..5 {
        let (mut program, types) = observation_source_program(CONVERTED_OBSERVATION_SOURCE);
        let function_id = observed_property(&program).id;
        let (owner, _) = observation_owner(observed_read_calls(observed_property(&program))[0]);
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.id == function_id)
            .unwrap();
        if mutation == 4 {
            let call = function
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.statements)
                .find_map(|statement| {
                    let S::Assert { condition, .. } = &mut statement.kind else {
                        return None;
                    };
                    matches!(condition.kind, E::Call { .. }).then_some(condition)
                })
                .unwrap();
            let E::Call {
                ownership: hir::CallOwnership::Source(source),
                ..
            } = &mut call.kind
            else {
                unreachable!();
            };
            source.arguments[0].staging = hir::ArgumentStaging::Original;
        } else {
            let other = function.params[1].local;
            let initializer = function
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.statements)
                .find_map(|statement| match &mut statement.kind {
                    S::Let { local, value } if *local == owner => Some(value),
                    _ => None,
                })
                .unwrap();
            let E::InterfaceCoerce {
                value: snapshot, ..
            } = &mut initializer.kind
            else {
                panic!("checked conversion");
            };
            let E::Clone(original) = &mut snapshot.kind else {
                panic!("raw snapshot");
            };
            match mutation {
                0 => *snapshot = Box::new(original.as_ref().clone()),
                1 => {
                    *snapshot = Box::new(original.as_ref().clone());
                    let original_conversion = initializer.clone();
                    initializer.kind = E::Clone(Box::new(original_conversion));
                }
                2 => snapshot.ty = TypeInterner::INT64,
                3 => original.kind = E::Local(other),
                _ => unreachable!(),
            }
        }
        let error = validate_function(&program, observed_property(&program), &types)
            .expect_err("malformed observation snapshot must fail before preparation");
        let expected = match mutation {
            0 | 2 => "observation lost its exact raw snapshot occurrence",
            1 => "observation cannot snapshot an erased or unsupported value",
            3 => "binding does not match its actual typed operand",
            4 => "requires explicit owning staging",
            _ => unreachable!(),
        };
        assert!(error.contains(expected), "{mutation}: {error}");
    }
}

// Checked predicate identities and nominal types are taken from real source.
// The extra function mirrors native_property_cases' compiler-owned replay call.
const GENERATED_REFINEMENT_REPLAY_SOURCE: &str = r#"namespace app
type Positive = int64 where value > 0
type Higher = Positive where value > 1
type Nonempty = list[int64] where true
property refined:
    given number: Higher
    given items: Nonempty
    assert true
function number() returns Higher:
    return 2
function items() returns Nonempty:
    return list(2)
"#;

fn generated_refinement_program(default: bool) -> (Program, TypeInterner) {
    let file = jett_common::FileId::new(0);
    let parsed = jett_parser::parse(GENERATED_REFINEMENT_REPLAY_SOURCE, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    let checked = jett_typecheck::check(&parsed.module, &resolved);
    assert!(
        checked
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
        "{:?}",
        checked.diagnostics
    );
    let mut high = hir::lower_with_test_bodies(
        &parsed.module,
        &resolved,
        &checked,
        &HashMap::from([(file, jett_common::SourceOrigin::Project)]),
    )
    .expect("checked refinement HIR");
    let property_declaration = parsed
        .module
        .items
        .iter()
        .find_map(|item| match item {
            jett_parser::ast::Item::Property(property) if property.name.name == "refined" => {
                Some(property)
            }
            _ => None,
        })
        .expect("exact parsed property");
    let targets = high
        .functions
        .iter()
        .filter(|function| {
            function.identity.declaration.kind == hir::DeclarationKind::Property
                && function.span == property_declaration.span
                && function.identity.declaration.name
                    == format!(
                        "{}:{}",
                        property_declaration.name.name, property_declaration.span.start
                    )
        })
        .collect::<Vec<_>>();
    let [property] = targets.as_slice() else {
        panic!("one exact checked property target");
    };
    let property = (*property).clone();
    let args = ["number", "items"]
        .into_iter()
        .map(|name| {
            let function = high
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == name)
                .expect("checked candidate");
            let hir::StatementKind::Return(Some(mut candidate)) =
                function.body.statements[0].kind.clone()
            else {
                panic!("checked return guard");
            };
            let E::Handle {
                error_local,
                failure,
                ..
            } = &mut candidate.kind
            else {
                panic!("checked nominal refinement handle");
            };
            *error_local = None;
            let failure_span = failure.span;
            let body = if default {
                let mut fallback = candidate.clone();
                fn occurrence(value: &mut Expression, span: Span) {
                    value.span = span;
                    match &mut value.kind {
                        E::ListConstruct { elements } => {
                            for value in elements {
                                occurrence(value, span);
                            }
                        }
                        E::Handle {
                            target,
                            error_local,
                            failure,
                            ..
                        } => {
                            occurrence(target, span);
                            *error_local = None;
                            *failure = hir::Block {
                                statements: vec![hir::Statement {
                                    kind: hir::StatementKind::Return(None),
                                    span,
                                }],
                                span,
                            };
                        }
                        _ => {}
                    }
                }
                occurrence(&mut fallback, failure_span);
                hir::StatementKind::HandleDefault(fallback)
            } else {
                hir::StatementKind::Return(None)
            };
            let E::Handle { failure, .. } = &mut candidate.kind else {
                unreachable!();
            };
            *failure = hir::Block {
                statements: vec![hir::Statement {
                    kind: body,
                    span: failure_span,
                }],
                span: failure_span,
            };
            candidate
        })
        .collect::<Vec<_>>();
    let parameters = property
        .params
        .iter()
        .map(|parameter| parameter.ty)
        .collect::<Vec<_>>();
    let order = vec![0, 1];
    let ownership = hir::CallOwnership::generated(
        hir::GeneratedOperation::NativeSuite {
            function: property.id,
        },
        &args,
        &parameters,
        &[jett_typecheck::CheckedCalleeAccess::Owned; 2],
        TypeInterner::NOTHING,
        &order,
        &checked.interner,
    )
    .expect("exact replay factory");
    let span = property.span;
    let call = Expression {
        kind: E::Call {
            function: property.id,
            args,
            evaluation_order: order,
            ownership,
        },
        ty: TypeInterner::NOTHING,
        span,
    };
    let mut identity = property.identity.clone();
    identity.declaration.name = "exercise".into();
    let id = hir::FunctionId::new(high.functions.len() as u32);
    high.functions.push(hir::Function {
        id,
        identity,
        debug_kind: hir::FunctionDebugKind::named("app", "exercise"),
        source_definition: None,
        params: Vec::new(),
        capture_count: 0,
        return_type: TypeInterner::NOTHING,
        locals: Vec::new(),
        body: hir::Block {
            statements: vec![
                hir::Statement {
                    kind: hir::StatementKind::Expression(call),
                    span,
                },
                hir::Statement {
                    kind: hir::StatementKind::Return(None),
                    span,
                },
            ],
            span,
        },
        span,
    });
    hir::validate_backend_types(&high, &checked.interner)
        .expect("original immutable replay candidates");
    let program = crate::lower(&high, &checked.interner).expect("canonical refinement replay MIR");
    (program, checked.interner)
}

#[test]
fn caller_acquisition_generated_refinement_replay_keeps_exact_chain_and_terminal_failure() {
    let (program, types) = generated_refinement_program(false);
    let function = exercise(&program);
    let acquisitions = validate_function(&program, function, &types)
        .expect("ordered checked refinement candidates");
    assert_eq!(
        acquisitions.owner_initializers().count(),
        0,
        "generated candidates do not consume source bindings"
    );
    let mut checks = function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .filter_map(|statement| match &statement.kind {
            S::CheckRefinement { type_name, .. } => Some(type_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    checks.sort_unstable();
    assert_eq!(checks, ["app.Higher", "app.Nonempty", "app.Positive"]);
    assert_eq!(
        function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .filter(|statement| matches!(
                statement.kind,
                S::Let {
                    value: Expression {
                        kind: E::RefinementValidated(_),
                        ..
                    },
                    ..
                }
            ))
            .count(),
        2
    );
    assert!(
        function
            .blocks
            .iter()
            .any(|block| matches!(block.terminator.kind, T::Return(None)))
    );
}

#[test]
fn caller_acquisition_generated_refinement_defaults_and_preparation_preserve_owned_candidates() {
    let (mut program, types) = generated_refinement_program(true);
    validate_function(&program, exercise(&program), &types)
        .expect("defaults remain inside the exact failure body");
    crate::prepare_native_uninhabited_sums(&mut program, &types);
    crate::prepare_native_generated_functions(&mut program, &types);
    crate::prepare_native_sequences(&mut program, &types);
    crate::validate_call_ownership(&program, &types)
        .expect("refinement source and predicate identities survive canonical preparation");
}

#[test]
fn caller_acquisition_generated_refinement_rejects_changed_predicates_candidate_and_edges() {
    let (original, types) = generated_refinement_program(false);
    for mutation in 0..8 {
        let mut program = original.clone();
        let other_predicate = program
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.name == "Higher"
                    && function.identity.declaration.kind
                        == hir::DeclarationKind::RefinementPredicate
            })
            .expect("distinct checked predicate")
            .id;
        let function = exercise_mut(&mut program);
        let (block_index, check_index) = function
            .blocks
            .iter()
            .enumerate()
            .find_map(|(block_index, block)| {
                block
                    .statements
                    .iter()
                    .position(|statement| matches!(statement.kind, S::CheckRefinement { .. }))
                    .map(|index| (block_index, index))
            })
            .expect("first exact check");
        match mutation {
            0 => {
                let S::CheckRefinement { type_name, .. } =
                    &mut function.blocks[block_index].statements[check_index].kind
                else {
                    unreachable!();
                };
                *type_name = "app.Higher".into();
            }
            1 => {
                let S::CheckRefinement {
                    call:
                        Expression {
                            kind: E::Call { args, .. },
                            ..
                        },
                    ..
                } = &mut function.blocks[block_index].statements[check_index].kind
                else {
                    unreachable!();
                };
                let E::Clone(inner) = &mut args[0].kind else {
                    panic!("canonical input clone");
                };
                inner.kind = E::Int(9);
            }
            2 => {
                let S::Let { value, .. } =
                    &mut function.blocks[block_index].statements[check_index + 1].kind
                else {
                    panic!("passed local");
                };
                value.kind = E::Bool(true);
            }
            3 => {
                let T::Branch {
                    then_block,
                    else_block,
                    ..
                } = &mut function.blocks[block_index].terminator.kind
                else {
                    panic!("exact branch");
                };
                std::mem::swap(then_block, else_block);
            }
            4 => {
                let T::Branch { else_block, .. } = function.blocks[block_index].terminator.kind
                else {
                    panic!("exact branch");
                };
                function.blocks[else_block.index() as usize].terminator.kind = T::Return(None);
            }
            5 => {
                let T::Branch { then_block, .. } = function.blocks[block_index].terminator.kind
                else {
                    panic!("exact branch");
                };
                function.blocks[block_index].terminator.kind = T::Goto(then_block);
            }
            6 => {
                let success = function
                    .blocks
                    .iter_mut()
                    .find(|block| {
                        block.statements.iter().any(|statement| {
                            matches!(
                                statement.kind,
                                S::Let {
                                    value: Expression {
                                        kind: E::RefinementValidated(_),
                                        ..
                                    },
                                    ..
                                }
                            )
                        })
                    })
                    .expect("validated output");
                success.statements.push(success.statements[0].clone());
            }
            _ => {
                let S::CheckRefinement {
                    call:
                        Expression {
                            kind: E::Call { function, .. },
                            ..
                        },
                    ..
                } = &mut function.blocks[block_index].statements[check_index].kind
                else {
                    unreachable!();
                };
                *function = other_predicate;
            }
        }
        generated_rejects(&program, &types);
    }
}

#[test]
fn caller_acquisition_generated_refinement_rejects_default_failure_bypass_and_borrowed_output() {
    let (original, types) = generated_refinement_program(true);
    let (output, success) = exercise(&original)
        .blocks
        .iter()
        .find_map(|block| {
            block
                .statements
                .iter()
                .find_map(|statement| match statement.kind {
                    S::Let {
                        local,
                        value:
                            Expression {
                                kind: E::RefinementValidated(_),
                                ..
                            },
                    } => Some((local, block.id)),
                    _ => None,
                })
        })
        .expect("first validated output");
    for borrowed in [false, true] {
        let mut program = original.clone();
        let function = exercise_mut(&mut program);
        let default = function
            .blocks
            .iter()
            .find(|block| {
                block.id != success && block.statements.iter().any(|statement|
            matches!(statement.kind, S::Let { local, .. } if local == output))
            })
            .expect("exact default block")
            .id;
        if borrowed {
            let S::Let { value, .. } = function.blocks[default.index() as usize]
                .statements
                .iter_mut()
                .find(|statement| matches!(statement.kind, S::Let { local, .. } if local == output))
                .map(|statement| &mut statement.kind)
                .expect("default owning Let")
            else {
                unreachable!();
            };
            *value = Expression {
                kind: E::View(Box::new(value.clone())),
                ty: value.ty,
                span: value.span,
            };
        } else {
            let block = function
                .blocks
                .iter_mut()
                .find(|block| {
                    block
                        .statements
                        .iter()
                        .any(|statement| matches!(statement.kind, S::CheckRefinement { .. }))
                })
                .expect("selecting predicate");
            let T::Branch { else_block, .. } = &mut block.terminator.kind else {
                panic!("canonical selecting Branch");
            };
            *else_block = default;
        }
        generated_rejects(&program, &types);
    }
}

const GENERATED_REFINEMENT_LOOP_DEFAULT_SOURCE: &str = r#"namespace app
type Nonempty = list[int64] where true
property refined:
    given items: Nonempty
    assert true
function candidate() returns Nonempty:
    return list(2)
function loop_default(view values: list[secret[Nonempty]]) returns Nonempty:
    for item in view values:
        return clone declassify item
    return list(2)
"#;

fn generated_refinement_loop_default_program() -> (Program, TypeInterner) {
    let file = jett_common::FileId::new(0);
    let parsed = jett_parser::parse(GENERATED_REFINEMENT_LOOP_DEFAULT_SOURCE, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    let checked = jett_typecheck::check(&parsed.module, &resolved);
    assert!(
        checked
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
        "{:?}",
        checked.diagnostics
    );
    let mut high = hir::lower_with_test_bodies(
        &parsed.module,
        &resolved,
        &checked,
        &HashMap::from([(file, jett_common::SourceOrigin::Project)]),
    )
    .expect("checked viewed-loop HIR");
    let property_span = parsed
        .module
        .items
        .iter()
        .find_map(|item| match item {
            jett_parser::ast::Item::Property(property) if property.name.name == "refined" => {
                Some(property.span)
            }
            _ => None,
        })
        .expect("exact parsed property");
    let targets = high
        .functions
        .iter()
        .filter(|function| {
            function.span == property_span
                && function.identity.declaration.kind == hir::DeclarationKind::Property
                && function.identity.declaration.name == format!("refined:{}", property_span.start)
        })
        .collect::<Vec<_>>();
    let [property] = targets.as_slice() else {
        panic!("one exact checked property");
    };
    let property = (*property).clone();
    let fallback = high
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "loop_default")
        .expect("source checked loop")
        .clone();
    let original = high
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "candidate")
        .expect("checked refinement candidate");
    let hir::StatementKind::Return(Some(mut candidate)) = original.body.statements[0].kind.clone()
    else {
        panic!("refinement guard");
    };
    let mut iteration = fallback.body.statements[0].clone();
    let hir::StatementKind::For { body, .. } = &mut iteration.kind else {
        panic!("source viewed For");
    };
    let hir::StatementKind::Return(Some(value)) = body.statements[0].kind.clone() else {
        panic!("source checked explicit Clone");
    };
    body.statements[0].kind = hir::StatementKind::HandleDefault(value);
    let failure_span = iteration.span;
    let E::Handle {
        error_local,
        failure,
        ..
    } = &mut candidate.kind
    else {
        panic!("exact candidate Handle");
    };
    *error_local = None;
    *failure = hir::Block {
        statements: vec![
            iteration,
            hir::Statement {
                kind: hir::StatementKind::Return(None),
                span: failure_span,
            },
        ],
        span: failure_span,
    };
    let args = vec![candidate];
    let order = vec![0];
    let ownership = hir::CallOwnership::generated(
        hir::GeneratedOperation::NativeSuite {
            function: property.id,
        },
        &args,
        &[property.params[0].ty],
        &[jett_typecheck::CheckedCalleeAccess::Owned],
        TypeInterner::NOTHING,
        &order,
        &checked.interner,
    )
    .expect("exact replay factory");
    let span = property.span;
    let call = Expression {
        kind: E::Call {
            function: property.id,
            args,
            evaluation_order: order,
            ownership,
        },
        ty: TypeInterner::NOTHING,
        span,
    };
    let mut identity = property.identity.clone();
    identity.declaration.name = "exercise".into();
    high.functions.push(hir::Function {
        id: hir::FunctionId::new(high.functions.len() as u32),
        identity,
        debug_kind: hir::FunctionDebugKind::named("app", "exercise"),
        source_definition: None,
        params: fallback.params,
        capture_count: 0,
        return_type: TypeInterner::NOTHING,
        locals: fallback.locals,
        body: hir::Block {
            statements: vec![
                hir::Statement {
                    kind: hir::StatementKind::Expression(call),
                    span,
                },
                hir::Statement {
                    kind: hir::StatementKind::Return(None),
                    span,
                },
            ],
            span,
        },
        span,
    });
    hir::validate_backend_types(&high, &checked.interner)
        .expect("checked explicit Clone default preserves loop view");
    (
        crate::lower(&high, &checked.interner).expect("canonical replay with viewed-loop default"),
        checked.interner,
    )
}

#[test]
fn caller_acquisition_generated_refinement_defaults_distinguish_logical_loop_borrows_from_clone() {
    let (original, types) = generated_refinement_loop_default_program();
    validate_function(&original, exercise(&original), &types)
        .expect("explicit ordinary-data Clone acquires the default");
    let function = exercise(&original);
    let (binding, body) = function
        .blocks
        .iter()
        .find_map(|block| match &block.terminator.kind {
            T::ForEach {
                key,
                value: None,
                body,
                ..
            } => Some((*key, *body)),
            _ => None,
        })
        .expect("original viewed loop binding");
    assert!(
        !function.is_view_local(binding),
        "loop backing is distinct from ABI/persistent local views"
    );
    let target_type = match types.resolve(function.local(binding).unwrap().ty) {
        Type::Secret(inner) => *inner,
        _ => panic!("exact Secret loop endpoint"),
    };
    for wrapped in [false, true] {
        let mut program = original.clone();
        let function = exercise_mut(&mut program);
        let block = &mut function.blocks[body.index() as usize];
        let value = block
            .statements
            .iter_mut()
            .find_map(|statement| match &mut statement.kind {
                S::Let { value, .. }
                    if value.ty == target_type && matches!(value.kind, E::Clone(_)) =>
                {
                    Some(value)
                }
                _ => None,
            })
            .expect("source explicit Clone default");
        let mut borrowed = Expression {
            kind: E::Local(binding),
            ty: function.locals[binding.index() as usize].ty,
            span: value.span,
        };
        if wrapped {
            borrowed = Expression {
                kind: E::View(Box::new(borrowed)),
                ty: function.locals[binding.index() as usize].ty,
                span: value.span,
            };
        }
        *value = Expression {
            kind: E::Declassify(Box::new(borrowed)),
            ty: target_type,
            span: value.span,
        };
        let error = generated_rejects(&program, &types);
        assert!(
            error.contains("default lost its exact owned failure occurrence"),
            "{error}"
        );
    }
}

fn source_sum_loop_default(result: bool, qualified: bool) -> String {
    let mut source = String::from("namespace app\n");
    if qualified {
        source.push_str("type Nonempty = list[int64] where true\n");
    }
    source.push_str("function count(view values: list[int64]) returns int64:\n    return 1\n");
    if result {
        source.push_str("function candidate() returns result[list[int64], string]:\n    return fail(\"missing\")\n");
    } else {
        source.push_str("function candidate() returns optional[list[int64]]:\n    return none\n");
    }
    let container = if qualified {
        "list[secret[Nonempty]]"
    } else {
        "list[list[int64]]"
    };
    let handler = if result { "handle error" } else { "handle" };
    let default = if qualified {
        "clone coarsen declassify item"
    } else {
        "clone item"
    };
    source.push_str(&format!("function exercise(view values: {container}) returns int64:\n    for item in view values:\n        return count(candidate() {handler}:\n            default {default}\n        )\n    return 0\n"));
    source
}

fn source_sum_loop_default_ids(function: &Function) -> (LocalId, LocalId) {
    let binding = function
        .blocks
        .iter()
        .find_map(|block| match block.terminator.kind {
            T::ForEach {
                key, value: None, ..
            } => Some(key),
            _ => None,
        })
        .expect("one original viewed loop binder");
    let outputs = function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .filter_map(|statement| match statement.kind {
            S::SumTake {
                target,
                success: true,
                ..
            } => Some(target),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [output] = outputs.as_slice() else {
        panic!("one exact inhabited sum success output");
    };
    (binding, *output)
}

fn source_sum_loop_default_value(function: &mut Function, output: LocalId) -> &mut Expression {
    function
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.statements)
        .find_map(|statement| match &mut statement.kind {
            S::Let { local, value } if *local == output => Some(value),
            _ => None,
        })
        .expect("exact handled default definition")
}

#[test]
fn caller_acquisition_source_sum_defaults_keep_explicit_clone_and_exact_sum_proofs() {
    for result in [false, true] {
        for qualified in [false, true] {
            let (original, types) = source_program(&source_sum_loop_default(result, qualified));
            let function = exercise(&original);
            let (binding, output) = source_sum_loop_default_ids(function);
            assert!(
                !function.is_view_local(binding),
                "logical loop backing is not a persistent/ABI view slot"
            );
            let target = original
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == "count")
                .expect("checked count target")
                .id;
            let call = function.blocks.iter().flat_map(|block| &block.statements).find_map(|statement| match &statement.kind {
                S::Let { value, .. } | S::Evaluate(value) if matches!(value.kind, E::Call { function, .. } if function == target) => Some(value), _ => None,
            }).expect("exact staged count invocation");
            let E::Call {
                ownership: hir::CallOwnership::Source(source),
                ..
            } = &call.kind
            else {
                panic!("real Source packet");
            };
            let hir::ArgumentStaging::Relinquished { owner, .. } = source.arguments[0].staging
            else {
                panic!("owned handled result to view formal");
            };
            let acquisitions = validate_function(&original, function, &types)
                .expect("explicit Clone owns the sum default");
            assert_eq!(
                acquisitions
                    .owner_initializer(owner)
                    .expect("validated handled owner")
                    .binding,
                None
            );
            assert!(function.blocks.iter().flat_map(|block| &block.statements).any(|statement|
                matches!(&statement.kind, S::Let { local, value } if *local == output && matches!(value.kind, E::Clone(_)))));
            let mut prepared = original.clone();
            crate::prepare_native_sequences(&mut prepared, &types);
            crate::prepare_native_uninhabited_sums(&mut prepared, &types);
            crate::prepare_native_generated_functions(&mut prepared, &types);
            validate_function(&prepared, exercise(&prepared), &types)
                .expect("canonical preparation preserves explicit owned defaults");
            assert!(
                exercise(&prepared).prepared_absent_successes.is_empty(),
                "inhabited sums gain no absence authority"
            );
        }
    }
}

#[test]
fn caller_acquisition_source_sum_defaults_reject_logical_borrows_under_transparent_wrappers() {
    for result in [false, true] {
        for qualified in [false, true] {
            let (original, types) = source_program(&source_sum_loop_default(result, qualified));
            let (binding, output) = source_sum_loop_default_ids(exercise(&original));
            let binding_ty = exercise(&original).local(binding).expect("loop local").ty;
            let nominal = if qualified {
                let Type::Secret(inner) = types.resolve(binding_ty) else {
                    panic!("exact Secret loop endpoint");
                };
                Some(*inner)
            } else {
                None
            };
            for hidden_view in [false, true] {
                let mut changed = original.clone();
                let value = source_sum_loop_default_value(exercise_mut(&mut changed), output);
                let mut borrowed = Expression {
                    kind: E::Local(binding),
                    ty: binding_ty,
                    span: value.span,
                };
                if hidden_view {
                    borrowed = Expression {
                        kind: E::View(Box::new(borrowed)),
                        ty: binding_ty,
                        span: value.span,
                    };
                }
                if let Some(nominal) = nominal {
                    borrowed = Expression {
                        kind: E::Declassify(Box::new(borrowed)),
                        ty: nominal,
                        span: value.span,
                    };
                    borrowed = Expression {
                        kind: E::Coarsen(Box::new(borrowed)),
                        ty: value.ty,
                        span: value.span,
                    };
                }
                *value = borrowed;
                let error = validate_function(&changed, exercise(&changed), &types)
                    .err()
                    .expect("borrowed default must fail");
                let expected = if hidden_view && !qualified {
                    "call ownership producer result contains an unsupported definition"
                } else {
                    "call ownership handled default cannot acquire a borrowed owner"
                };
                assert_eq!(error, expected);
                if !hidden_view {
                    let mut prepared = original.clone();
                    crate::prepare_native_sequences(&mut prepared, &types);
                    let function = exercise_mut(&mut prepared);
                    let prepared_binding = function
                        .blocks
                        .iter()
                        .flat_map(|block| &block.statements)
                        .find_map(|statement| match statement.kind {
                            S::SequenceGet { target, .. }
                                if function
                                    .local(target)
                                    .is_some_and(|local| local.ty == binding_ty) =>
                            {
                                Some(target)
                            }
                            _ => None,
                        })
                        .expect("canonical viewed element storage");
                    let prepared_output = function
                        .blocks
                        .iter()
                        .flat_map(|block| &block.statements)
                        .find_map(|statement| match statement.kind {
                            S::SumTake {
                                target,
                                success: true,
                                ..
                            } => Some(target),
                            _ => None,
                        })
                        .expect("retained inhabited success output");
                    let value = source_sum_loop_default_value(function, prepared_output);
                    let mut borrowed = Expression {
                        kind: E::Local(prepared_binding),
                        ty: binding_ty,
                        span: value.span,
                    };
                    if let Some(nominal) = nominal {
                        borrowed = Expression {
                            kind: E::Declassify(Box::new(borrowed)),
                            ty: nominal,
                            span: value.span,
                        };
                        borrowed = Expression {
                            kind: E::Coarsen(Box::new(borrowed)),
                            ty: value.ty,
                            span: value.span,
                        };
                    }
                    *value = borrowed;
                    let error = validate_function(&prepared, exercise(&prepared), &types)
                        .err()
                        .expect("prepared logical borrow must fail");
                    assert_eq!(
                        error,
                        "call ownership handled default cannot acquire a borrowed owner"
                    );
                }
            }
        }
    }
}

const CONVERTED_HANDLED_SOURCE: &str = r#"namespace app
interface Show:
    function show(view self: Show) returns string
implement Show for uint64:
    function show(view self: uint64) returns string:
        return "uint64"
function exercise(incoming: optional[uint64], spare: uint64) returns string:
    uint64 fallback = 0
    return Show.show(view(incoming handle:
        default fallback
    ))
"#;

const OWNED_CONVERTED_HANDLED_SOURCE: &str = r#"namespace app
interface Show:
    function show(view self: Show) returns string
implement Show for uint64:
    function show(view self: uint64) returns string:
        return "uint64"
function read(value: Show) returns string:
    return "received"
function exercise(incoming: optional[uint64], spare: uint64) returns string:
    uint64 fallback = 0
    return read((incoming handle:
        default fallback
    ))
"#;

fn converted_handle_stage(function: &Function) -> Option<LocalId> {
    let E::Call {
        ownership: hir::CallOwnership::Source(packet),
        ..
    } = &read_call(function).kind
    else {
        panic!("checked converted source call");
    };
    match packet.arguments[0].staging {
        hir::ArgumentStaging::Original => None,
        hir::ArgumentStaging::Borrowed { loan } => Some(loan),
        hir::ArgumentStaging::Transferred { owner }
        | hir::ArgumentStaging::Relinquished { owner, .. }
        | hir::ArgumentStaging::RetainedSnapshot { owner, .. }
        | hir::ArgumentStaging::Observed { owner } => Some(owner),
        hir::ArgumentStaging::Copied { value } => Some(value),
    }
}

fn converted_handle_operand(function: &Function) -> &Expression {
    if let Some(stage) = converted_handle_stage(function) {
        return function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .find_map(|statement| match &statement.kind {
                S::Let { local, value } | S::BeginCallView { local, value } if *local == stage => {
                    Some(value)
                }
                _ => None,
            })
            .expect("exact source staging initializer");
    }
    let E::Call { args, .. } = &read_call(function).kind else {
        panic!("source read");
    };
    &args[0]
}

fn converted_handle_operand_mut(function: &mut Function) -> &mut Expression {
    if let Some(stage) = converted_handle_stage(function) {
        return function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.statements)
            .find_map(|statement| match &mut statement.kind {
                S::Let { local, value } | S::BeginCallView { local, value } if *local == stage => {
                    Some(value)
                }
                _ => None,
            })
            .expect("exact source staging initializer");
    }
    for block in &mut function.blocks {
        for statement in &mut block.statements {
            if let S::Let { value, .. } | S::Evaluate(value) = &mut statement.kind
                && let E::Call { args, .. } = &mut value.kind
            {
                return &mut args[0];
            }
        }
        if let T::Return(Some(value)) = &mut block.terminator.kind
            && let E::Call { args, .. } = &mut value.kind
        {
            return &mut args[0];
        }
    }
    panic!("source read invocation");
}

fn converted_handle_leaf_mut(mut value: &mut Expression) -> &mut Expression {
    while matches!(
        value.kind,
        E::View(_) | E::InterfaceCoerce { .. } | E::FunctionAdapter { .. }
    ) {
        value = match &mut value.kind {
            E::View(inner)
            | E::InterfaceCoerce { value: inner, .. }
            | E::FunctionAdapter { value: inner, .. } => inner,
            _ => unreachable!("transparent conversion checked before borrowing"),
        };
    }
    value
}

#[test]
fn caller_acquisition_converted_handle_joins_exact_inner_result_and_full_cfg() {
    let (program, types) = source_program(CONVERTED_HANDLED_SOURCE);
    let function = exercise(&program);
    let E::Call {
        ownership: hir::CallOwnership::Source(packet),
        ..
    } = &read_call(function).kind
    else {
        panic!("source read");
    };
    let argument = &packet.arguments[0];
    let backing = hir::validate_source_handled_operand(
        converted_handle_operand(function),
        argument,
        &packet.bridge,
        &types,
    )
    .expect("exact conversion")
    .expect("private original Handle backing");
    let E::Local(local) = backing.kind else {
        panic!("lowered Handle result");
    };
    assert_eq!(backing.ty, TypeInterner::UINT64);
    assert_ne!(backing.span, argument.source_span);
    assert_ne!(local, function.params[1].local);
    assert!(function.blocks.iter().flat_map(|block| &block.statements).any(|statement|
        matches!(statement.kind, S::SumTake { target, success: true, .. } if target == local)));
    validate_function(&program, function, &types).expect("original result and complete CFG proofs");
    // A written View of a statically selected method retains the inner source
    // span. A separate owning interface formal exercises actual conversion.
    let (converted, converted_types) = source_program(OWNED_CONVERTED_HANDLED_SOURCE);
    let converted_function = exercise(&converted);
    let E::Call {
        ownership: hir::CallOwnership::Source(packet),
        ..
    } = &read_call(converted_function).kind
    else {
        panic!("owning interface source call");
    };
    let operand = converted_handle_operand(converted_function);
    assert!(matches!(operand.kind, E::InterfaceCoerce { .. }));
    let backing = hir::validate_source_handled_operand(
        operand,
        &packet.arguments[0],
        &packet.bridge,
        &converted_types,
    )
    .expect("actual finite conversion")
    .expect("private original Handle");
    assert!(matches!(backing.kind, E::Local(_)));
    assert_eq!(backing.ty, TypeInterner::UINT64);
    assert_ne!(operand.ty, backing.ty);
    validate_function(&converted, converted_function, &converted_types)
        .expect("converted result CFG");
}

#[test]
fn caller_acquisition_converted_handle_refuses_foreign_occurrences_and_unproved_slots() {
    let (program, types) = source_program(CONVERTED_HANDLED_SOURCE);
    for mutation in [
        "inner span",
        "inner type",
        "parameter substitution",
        "outer source span",
        "sum occurrence",
        "failed tag",
    ] {
        let mut corrupted = program.clone();
        let function = exercise_mut(&mut corrupted);
        let spare = function.params[1].local;
        let backing = converted_handle_leaf_mut(converted_handle_operand_mut(function));
        let E::Local(output) = backing.kind else {
            panic!("original CFG result");
        };
        match mutation {
            "inner span" => backing.span.start += 1,
            "inner type" => backing.ty = TypeInterner::INT64,
            "parameter substitution" => backing.kind = E::Local(spare),
            "outer source span" => converted_handle_operand_mut(function).span.start += 1,
            "sum occurrence" | "failed tag" => {
                let statement = function.blocks.iter_mut().flat_map(|block| &mut block.statements)
                    .find(|statement| matches!(statement.kind, S::SumTake { target, success: true, .. } if target == output))
                    .expect("exact handled success");
                if mutation == "sum occurrence" {
                    statement.span.start += 1;
                } else {
                    let S::SumTake { success, .. } = &mut statement.kind else {
                        unreachable!()
                    };
                    *success = false;
                }
            }
            _ => unreachable!(),
        }
        rejects(&corrupted, &types);
    }
    let (mut converted, converted_types) = source_program(OWNED_CONVERTED_HANDLED_SOURCE);
    let original = converted_handle_operand_mut(exercise_mut(&mut converted));
    let E::InterfaceCoerce { value, .. } = &original.kind else {
        panic!("actual conversion");
    };
    original.kind = value.kind.clone();
    rejects(&converted, &converted_types);
}

const CONVERTED_VIEW_OWNER_SOURCE: &str = r#"namespace app
interface Named:
    function name(view self: Named) returns string
struct Boxed[T]:
    value: T
implement Named for Boxed[int8]:
    function name(view self: Boxed[int8]) returns string:
        return "narrow"
function read(view value: Named) returns int64:
    return 7
function exercise() returns int64:
    return read(Boxed[int8](value: 5))
"#;

#[test]
fn caller_acquisition_converted_view_producer_owns_the_complete_endpoint_once() {
    let (program, types) = source_program(CONVERTED_VIEW_OWNER_SOURCE);
    let function = exercise(&program);
    let call = read_call(function);
    let E::Call {
        ownership: hir::CallOwnership::Source(packet),
        args,
        ..
    } = &call.kind
    else {
        panic!("checked Source call");
    };
    let argument = &packet.arguments[0];
    assert_eq!(argument.syntax, jett_typecheck::CheckedCallerSyntax::Bare);
    assert_eq!(argument.effect, CheckedCallerEffect::RelinquishOwned);
    assert_eq!(
        argument.callee_access,
        jett_typecheck::CheckedCalleeAccess::View
    );
    assert_eq!(
        argument.physical_access,
        jett_typecheck::CheckedCalleeAccess::View
    );
    assert!(matches!(
        argument.origin,
        hir::CallerOrigin::OwnedExpression
    ));
    assert!(matches!(
        types.resolve(argument.actual_type),
        Type::Struct(_)
    ));
    let (owner, loan) = owning_stage(function);
    assert_ne!(owner, loan);
    let initializer = observation_initializer(function, owner);
    let E::InterfaceCoerce { value: raw, .. } = &initializer.kind else {
        panic!("owning storage contains the full checked conversion");
    };
    assert!(matches!(raw.kind, E::StructConstruct { .. }));
    assert_eq!(raw.ty, argument.actual_type);
    assert_eq!(raw.span, argument.source_span);
    assert_ne!(initializer.ty, raw.ty);
    assert!(matches!(types.resolve(initializer.ty), Type::Interface(_)));
    assert!(!crate::handlers::can_snapshot_view(&types, initializer.ty));
    let original_view = Expression {
        kind: E::View(Box::new(initializer.clone())),
        ty: initializer.ty,
        span: initializer.span,
    };
    assert!(
        crate::call_views::temporary_root(&types, &function.locals, &original_view).is_none(),
        "the converted endpoint does not acquire raw-temporary borrowing permission"
    );
    assert!(
        matches!(&args[0].kind, E::View(value) if matches!(value.kind, E::Local(id) if id == loan))
    );
    let owner_metadata = function.local(owner).expect("independent owning slot");
    assert_eq!(owner_metadata.ty, initializer.ty);
    assert_eq!(owner_metadata.debug_ty, initializer.ty);
    assert_eq!(owner_metadata.view_source, None);
    assert_eq!(function.local(loan).unwrap().view_source, Some(owner));
    let block = function
        .blocks
        .iter()
        .find(|block| {
            block.statements.iter().any(|statement|
        matches!(&statement.kind, S::Let { value, .. } if std::ptr::eq(value, call)))
        })
        .expect("materialized consuming operation");
    let owner_at = block
        .statements
        .iter()
        .position(|statement| matches!(statement.kind, S::Let { local, .. } if local == owner))
        .expect("one full owner");
    let begin_at = block
        .statements
        .iter()
        .position(
            |statement| matches!(statement.kind, S::BeginCallView { local, .. } if local == loan),
        )
        .expect("loan begins");
    let call_at = block.statements.iter().position(|statement|
        matches!(&statement.kind, S::Let { value, .. } if std::ptr::eq(value, call))).unwrap();
    let end_at = block
        .statements
        .iter()
        .position(|statement| matches!(statement.kind, S::EndCallView { local } if local == loan))
        .expect("loan ends");
    assert!(owner_at < begin_at && begin_at < call_at && call_at < end_at);
    assert_eq!(
        function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .filter(|statement| matches!(statement.kind, S::Let { local, .. } if local == owner))
            .count(),
        1
    );
    let acquisitions =
        validate_function(&program, function, &types).expect("exact owning conversion and loan");
    assert_eq!(acquisitions.owner_initializers().count(), 1);
    let acquired = acquisitions.owner_initializer(owner).unwrap();
    assert_eq!(acquired.binding, None);
    assert_eq!(acquired.effect, CheckedCallerEffect::RelinquishOwned);
    assert_eq!(acquired.actual_type, raw.ty);
    assert_eq!(acquired.storage_type, initializer.ty);
    crate::move_values::MoveValuePlan::analyze(&program, function, &types)
        .expect("ordinary owning storage and bounded View scope");
}

#[test]
fn caller_acquisition_converted_owner_preflight_preserves_written_borrow_paths() {
    let written_producer = CONVERTED_VIEW_OWNER_SOURCE.replace(
        "return read(Boxed[int8](value: 5))",
        "return read(view Boxed[int8](value: 5))",
    );
    let (program, types) = source_program(&written_producer);
    let function = exercise(&program);
    let E::Call {
        ownership: hir::CallOwnership::Source(packet),
        ..
    } = &read_call(function).kind
    else {
        panic!("written producer Source call");
    };
    let argument = &packet.arguments[0];
    assert_eq!(
        argument.syntax,
        jett_typecheck::CheckedCallerSyntax::WrittenView
    );
    assert_eq!(argument.effect, CheckedCallerEffect::RetainBorrow);
    assert_eq!(argument.staging, hir::ArgumentStaging::Original);
    assert_eq!(
        validate_function(&program, function, &types)
            .expect("original written producer")
            .owner_initializers()
            .count(),
        0
    );
    assert!(
        !function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .any(|statement| matches!(statement.kind, S::BeginCallView { .. })),
        "no owning-stage permission from written View"
    );

    let source = r#"namespace app
interface Named:
    function name(view self: Named) returns string
function read(view value: Named, extra: int64) returns int64:
    return extra
function exercise(view value: Named, incoming: optional[int64]) returns int64:
    return read(view value, incoming handle:
        default 3
    )
"#;
    let (program, types) = source_program(source);
    let function = exercise(&program);
    let E::Call {
        ownership: hir::CallOwnership::Source(packet),
        ..
    } = &read_call(function).kind
    else {
        panic!("written parameter Source call");
    };
    let argument = &packet.arguments[0];
    assert_eq!(
        argument.syntax,
        jett_typecheck::CheckedCallerSyntax::WrittenView
    );
    assert_eq!(argument.effect, CheckedCallerEffect::RetainBorrow);
    let hir::ArgumentStaging::Borrowed { loan } = argument.staging else {
        panic!("retained ABI parameter stages as a borrow");
    };
    assert_eq!(
        function.local(loan).unwrap().view_source,
        Some(function.params[0].local)
    );
    assert_eq!(
        function
            .local(function.params[0].local)
            .unwrap()
            .view_source,
        None
    );
    assert_eq!(
        validate_function(&program, function, &types)
            .expect("existing written parameter borrow")
            .owner_initializers()
            .count(),
        0
    );
}

#[test]
fn caller_acquisition_converted_view_owner_keeps_original_and_owned_slot_guards() {
    let (program, types) = source_program(CONVERTED_VIEW_OWNER_SOURCE);
    let (owner, loan) = owning_stage(exercise(&program));
    for mutation in [
        "changed effect",
        "missing stage",
        "borrowed owner",
        "borrowed initializer",
        "missing initializer",
        "missing conversion",
        "foreign loan owner",
    ] {
        let mut changed = program.clone();
        let function = exercise_mut(&mut changed);
        match mutation {
            "changed effect" | "missing stage" | "borrowed owner" => {
                let E::Call {
                    ownership: hir::CallOwnership::Source(packet),
                    ..
                } = &mut read_call_mut(function).kind
                else {
                    panic!("Source converted call");
                };
                match mutation {
                    "changed effect" => {
                        packet.arguments[0].effect = CheckedCallerEffect::RetainBorrow
                    }
                    "missing stage" => packet.arguments[0].staging = hir::ArgumentStaging::Original,
                    "borrowed owner" => {
                        packet.arguments[0].staging =
                            hir::ArgumentStaging::Relinquished { owner: loan, loan }
                    }
                    _ => unreachable!(),
                }
            }
            "borrowed initializer" | "missing initializer" | "missing conversion" => {
                let (block, index) = function.blocks.iter().enumerate().find_map(|(block, body)|
                    body.statements.iter().position(|statement|
                        matches!(statement.kind, S::Let { local, .. } if local == owner)).map(|index| (block, index)))
                    .expect("original full owning initializer");
                if mutation == "missing initializer" {
                    function.blocks[block].statements.remove(index);
                } else {
                    let S::Let { value, .. } = &mut function.blocks[block].statements[index].kind
                    else {
                        unreachable!();
                    };
                    if mutation == "missing conversion" {
                        let E::InterfaceCoerce { value: raw, .. } = &value.kind else {
                            panic!("original full owning conversion");
                        };
                        value.kind = raw.kind.clone();
                    } else {
                        value.kind = E::View(Box::new(value.clone()));
                    }
                }
            }
            "foreign loan owner" => function.locals[loan.index() as usize].view_source = None,
            _ => unreachable!(),
        }
        let expected = match mutation {
            "changed effect" => "call ownership disagrees with its original source witness",
            "missing stage" => {
                "call ownership requires explicit owning staging at this physical boundary"
            }
            "borrowed owner" => "relinquished call owner cannot be a borrowed slot",
            "borrowed initializer" => {
                "call ownership owner initializer changes type, occurrence or ownership"
            }
            "missing initializer" => "call ownership acquired slot requires one unique initializer",
            "missing conversion" => {
                "call ownership current operand has no typed conversion from the checked actual"
            }
            "foreign loan owner" => "call view has no borrowed origin",
            _ => unreachable!(),
        };
        assert_eq!(
            validate_function(&changed, exercise(&changed), &types)
                .expect_err("original witness and owning storage remain mandatory"),
            expected,
            "{mutation}"
        );
    }
}

const RETAINED_SNAPSHOT_DIRECT: &str = r#"namespace app
function read(view values: list[int64], extra: int64) returns int64:
    mutable int64 total = extra
    for value in view values:
        total = total + value
    return total
function exercise(incoming: optional[int64]) returns int64:
    mutable list[int64] values = list(3)
    list[int64] spare = list(9)
    int64 answer = read(view values, incoming handle:
        values = list(8)
        default 2
    )
    int64 after = read(view values, 0)
    return answer + after
"#;

const RETAINED_SNAPSHOT_PROJECTED: &str = r#"namespace app
struct Store:
    items: list[int64]
function read(view values: list[int64], extra: int64) returns int64:
    mutable int64 total = extra
    for value in view values:
        total = total + value
    return total
function exercise(incoming: optional[int64]) returns int64:
    mutable Store store = Store(items: list(3))
    int64 answer = read(view store.items, incoming handle:
        store = Store(items: list(8))
        default 2
    )
    int64 after = read(view store.items, 0)
    return answer + after
"#;

fn retained_snapshot_stage(function: &Function) -> (LocalId, LocalId) {
    let ownership = match &read_call(function).kind {
        E::Call { ownership, .. } | E::IndirectCall { ownership, .. } => ownership,
        _ => panic!("checked source invocation"),
    };
    let hir::CallOwnership::Source(source) = ownership else {
        panic!("Source packet");
    };
    let argument = &source.arguments[0];
    assert_eq!(argument.effect, CheckedCallerEffect::RetainBorrow);
    assert_eq!(
        argument.syntax,
        jett_typecheck::CheckedCallerSyntax::WrittenView
    );
    let hir::ArgumentStaging::RetainedSnapshot { owner, loan } = argument.staging else {
        panic!("independent retained snapshot");
    };
    assert_ne!(owner, loan);
    (owner, loan)
}

fn retained_snapshot_initializer(function: &Function, owner: LocalId) -> &Expression {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .find_map(|statement| match &statement.kind {
            S::Let { local, value } if *local == owner => Some(value),
            _ => None,
        })
        .expect("unique retained owner initializer")
}

fn retained_snapshot_initializer_mut(function: &mut Function, owner: LocalId) -> &mut Expression {
    function
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.statements)
        .find_map(|statement| match &mut statement.kind {
            S::Let { local, value } if *local == owner => Some(value),
            _ => None,
        })
        .expect("unique retained owner initializer")
}

fn retained_snapshot_error(program: &Program, types: &TypeInterner) -> String {
    validate_function(program, exercise(program), types).expect_err("corrupted retained snapshot")
}

#[test]
fn caller_acquisition_retained_snapshots_capture_direct_and_projected_endpoint_before_rebinding() {
    for (source, projected) in [
        (RETAINED_SNAPSHOT_DIRECT, false),
        (RETAINED_SNAPSHOT_PROJECTED, true),
    ] {
        let (program, types) = source_program(source);
        let function = exercise(&program);
        let (owner, loan) = retained_snapshot_stage(function);
        let acquisitions =
            validate_function(&program, function, &types).expect("retained snapshot proof");
        assert_eq!(
            acquisitions.owner_initializers().count(),
            0,
            "caller ownership is retained"
        );
        assert!(
            acquisitions
                .arguments(read_call(function))
                .expect("exact call")
                .is_empty()
        );
        assert_eq!(function.local(loan).expect("loan").view_source, Some(owner));
        assert!(
            function
                .local(owner)
                .expect("snapshot")
                .view_source
                .is_none()
        );
        let initializer = retained_snapshot_initializer(function, owner);
        let E::Clone(endpoint) = &initializer.kind else {
            panic!("full endpoint snapshot");
        };
        let E::View(endpoint) = &endpoint.kind else {
            panic!("original written view");
        };
        assert_eq!(matches!(endpoint.kind, E::Field { .. }), projected);
        assert_eq!(matches!(endpoint.kind, E::Local(_)), !projected);
        let owner_block = function
            .blocks
            .iter()
            .find(|block| {
                block.statements.iter().any(
                    |statement| matches!(statement.kind, S::Let { local, .. } if local == owner),
                )
            })
            .expect("snapshot block");
        let tag_block = function
            .blocks
            .iter()
            .find(|block| {
                block
                    .statements
                    .iter()
                    .any(|statement| matches!(statement.kind, S::SumTag { .. }))
            })
            .expect("later handled argument");
        assert_eq!(owner_block.id, tag_block.id);
        let owner_index = owner_block
            .statements
            .iter()
            .position(|statement| matches!(statement.kind, S::Let { local, .. } if local == owner))
            .expect("owner position");
        let begin_index = owner_block.statements.iter().position(|statement|
            matches!(statement.kind, S::BeginCallView { local, .. } if local == loan)).expect("loan position");
        let tag_index = owner_block
            .statements
            .iter()
            .position(|statement| matches!(statement.kind, S::SumTag { .. }))
            .expect("tag position");
        assert!(owner_index < begin_index && begin_index < tag_index);
        assert!(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .any(
                    |statement| matches!(statement.kind, S::EndCallView { local } if local == loan)
                )
        );
        crate::move_values::MoveValuePlan::analyze(&program, function, &types)
            .expect("rebind does not free the independent snapshot loan");
    }
}

#[test]
fn caller_acquisition_retained_snapshot_indirect_and_abort_keep_exact_scope_cleanup() {
    let indirect = RETAINED_SNAPSHOT_DIRECT.replace(
        "    int64 answer = read(view values,",
        "    function(view list[int64], int64) returns int64 callback = read\n    int64 answer = callback(view values,",
    );
    let abort = RETAINED_SNAPSHOT_DIRECT.replace("        default 2", "        return 11");
    for (source, indirect) in [(&indirect, true), (&abort, false)] {
        let (program, types) = source_program(source);
        let function = exercise(&program);
        let (_, loan) = retained_snapshot_stage(function);
        assert_eq!(
            matches!(read_call(function).kind, E::IndirectCall { .. }),
            indirect
        );
        validate_function(&program, function, &types).expect("call and aborted-call scopes");
        crate::move_values::MoveValuePlan::analyze(&program, function, &types)
            .expect("retained owner and independent snapshot cleanup");
        if !indirect {
            let returned = function.blocks.iter().find(|block|
                matches!(&block.terminator.kind, T::Return(Some(value)) if matches!(value.kind, E::Int(11))))
                .expect("original early handler return");
            assert!(returned.statements.iter().any(
                |statement| matches!(statement.kind, S::EndCallView { local } if local == loan)
            ));
        }
    }
}

#[test]
fn caller_acquisition_retained_snapshot_rejects_removed_clone_and_changed_source_occurrence() {
    let (program, types) = source_program(RETAINED_SNAPSHOT_DIRECT);
    let (owner, _) = retained_snapshot_stage(exercise(&program));
    for mutation in [
        "remove clone",
        "span",
        "endpoint type",
        "different root",
        "effect",
    ] {
        let mut changed = program.clone();
        let function = exercise_mut(&mut changed);
        if mutation == "effect" {
            let E::Call {
                ownership: hir::CallOwnership::Source(source),
                ..
            } = &mut read_call_mut(function).kind
            else {
                panic!("source call");
            };
            source.arguments[0].effect = CheckedCallerEffect::RelinquishOwned;
        } else {
            let other = function
                .locals
                .iter()
                .find(|local| local.name == "spare")
                .expect("same-type unused owner")
                .id;
            let initializer = retained_snapshot_initializer_mut(function, owner);
            match mutation {
                "remove clone" => {
                    let E::Clone(endpoint) = &initializer.kind else {
                        panic!("snapshot");
                    };
                    *initializer = endpoint.as_ref().clone();
                }
                "span" => initializer.span.start += 1,
                "endpoint type" => {
                    let E::Clone(endpoint) = &mut initializer.kind else {
                        panic!("snapshot");
                    };
                    endpoint.ty = TypeInterner::BOOL;
                }
                "different root" => {
                    let E::Clone(endpoint) = &mut initializer.kind else {
                        panic!("snapshot");
                    };
                    let E::View(endpoint) = &mut endpoint.kind else {
                        panic!("written view");
                    };
                    endpoint.kind = E::Local(other);
                }
                _ => unreachable!(),
            }
        }
        let error = retained_snapshot_error(&changed, &types);
        assert!(
            error.contains("snapshot") || error.contains("call ownership"),
            "{mutation}: {error}"
        );
    }
}

#[test]
fn caller_acquisition_retained_snapshot_rejects_loan_substitution_missing_end_and_late_capture() {
    let (program, types) = source_program(RETAINED_SNAPSHOT_DIRECT);
    let (owner, loan) = retained_snapshot_stage(exercise(&program));
    for mutation in [
        "loan source",
        "missing end",
        "late capture",
        "aliased owner",
    ] {
        let mut changed = program.clone();
        let function = exercise_mut(&mut changed);
        match mutation {
            "loan source" => {
                function.locals[loan.index() as usize].view_source = Some(function.params[0].local)
            }
            "aliased owner" => {
                function.locals[owner.index() as usize].view_source = Some(function.params[0].local)
            }
            "missing end" => {
                for block in &mut function.blocks {
                    block.statements.retain(|statement| !matches!(statement.kind, S::EndCallView { local } if local == loan));
                }
            }
            "late capture" => {
                let block = function
                    .blocks
                    .iter_mut()
                    .find(|block| {
                        block.statements.iter().any(|statement|
                    matches!(statement.kind, S::Let { local, .. } if local == owner))
                    })
                    .expect("capture block");
                let at = block.statements.iter().position(|statement|
                    matches!(statement.kind, S::Let { local, .. } if local == owner)).expect("capture index");
                let capture = block.statements.remove(at);
                let begin = block.statements.iter().position(|statement|
                    matches!(statement.kind, S::BeginCallView { local, .. } if local == loan)).expect("loan");
                let begin = block.statements.remove(begin);
                // Keep the loan valid on both handler arms, but move capture
                // after the later source's SumTag evaluation. Lexical snapshot
                // order must be proved independently from call dominance.
                assert!(
                    block
                        .statements
                        .iter()
                        .any(|statement| matches!(statement.kind, S::SumTag { .. }))
                );
                block.statements.push(capture);
                block.statements.push(begin);
            }
            _ => unreachable!(),
        }
        let error = retained_snapshot_error(&changed, &types);
        assert!(
            error.contains("snapshot")
                || error.contains("call ownership")
                || error.contains("call view"),
            "{mutation}: {error}"
        );
    }
}

#[test]
fn caller_acquisition_retained_snapshot_metadata_remaps_both_slots_without_source_reads() {
    let (program, types) = source_program(RETAINED_SNAPSHOT_PROJECTED);
    let function = exercise(&program);
    let (owner, loan) = retained_snapshot_stage(function);
    let E::Call { ownership, .. } = &read_call(function).kind else {
        panic!("source call");
    };
    let mut changed = ownership.clone();
    let mut ids = Vec::new();
    changed.metadata_local_ids(|id| ids.push(id));
    assert!(ids.contains(&owner) && ids.contains(&loan));
    changed
        .remap_metadata_locals(|id| Ok::<_, ()>(LocalId::new(id.index() + 1)))
        .expect("structural remap");
    let hir::CallOwnership::Source(source) = changed else {
        panic!("Source");
    };
    assert_eq!(
        source.arguments[0].staging,
        hir::ArgumentStaging::RetainedSnapshot {
            owner: LocalId::new(owner.index() + 1),
            loan: LocalId::new(loan.index() + 1),
        }
    );
    assert_eq!(
        source.arguments[0].effect,
        CheckedCallerEffect::RetainBorrow
    );
    assert_eq!(
        source.arguments[0].retained_snapshot_type(),
        Some(source.arguments[0].source_witness().occurrence_type())
    );
    let mut prepared = program.clone();
    crate::prepare_native_sequences(&mut prepared, &types);
    crate::prepare_native_uninhabited_sums(&mut prepared, &types);
    crate::prepare_native_generated_functions(&mut prepared, &types);
    crate::validate_call_ownership(&prepared, &types)
        .expect("actual canonical remaps keep snapshot proof");
    crate::move_values::MoveValuePlan::analyze(&prepared, exercise(&prepared), &types)
        .expect("metadata does not reread the rebound source at the call");
}

#[test]
fn caller_acquisition_retained_snapshot_does_not_grant_descriptor_signature_clone_authority() {
    let source = r#"namespace app
function value() returns int64:
    return 7
function read(view callback: function() returns int64, extra: int64) returns int64:
    return callback() + extra
function exercise(incoming: optional[int64]) returns int64:
    function() returns int64 callback = value
    return read(view callback, incoming handle:
        default 2
    )
"#;
    let (program, types) = source_program(source);
    let function = exercise(&program);
    let E::Call {
        ownership: hir::CallOwnership::Source(source),
        ..
    } = &read_call(function).kind
    else {
        panic!("descriptor source call");
    };
    assert!(source.arguments[0].retained_snapshot_type().is_none());
    assert!(matches!(
        source.arguments[0].staging,
        hir::ArgumentStaging::Borrowed { .. }
    ));
    let mut ownership = hir::CallOwnership::Source(source.clone());
    assert!(
        ownership
            .stage_retained_snapshot(0, LocalId::new(100), LocalId::new(101))
            .is_err()
    );
    validate_function(&program, function, &types)
        .expect("existing descriptor raw loan, no clone authority");

    let produced = r#"namespace app
function read(view values: list[int64], extra: int64) returns int64:
    return extra
function exercise(incoming: optional[int64]) returns int64:
    return read(view list(3), incoming handle:
        default 2
    )
"#;
    let (program, types) = source_program(produced);
    let function = exercise(&program);
    let E::Call {
        ownership: hir::CallOwnership::Source(source),
        ..
    } = &read_call(function).kind
    else {
        panic!("written producer Source call");
    };
    assert_eq!(
        source.arguments[0].origin,
        hir::CallerOrigin::OwnedExpression
    );
    assert!(source.arguments[0].retained_snapshot_type().is_none());
    assert_eq!(source.arguments[0].staging, hir::ArgumentStaging::Original);
    let mut ownership = hir::CallOwnership::Source(source.clone());
    assert!(
        ownership
            .stage_retained_snapshot(0, LocalId::new(100), LocalId::new(101))
            .is_err()
    );
    validate_function(&program, function, &types)
        .expect("existing fresh producer backing, no snapshot permission from type/span");
}

fn transparent_retained_source(secret: bool, projected: bool) -> String {
    let declaration = if secret {
        ""
    } else {
        "type Numbers = list[int64] where true\n"
    };
    let ty = if secret {
        "secret[list[int64]]"
    } else {
        "Numbers"
    };
    let wrapper = if secret { "declassify" } else { "coarsen" };
    let (holder, input_ty, setup, endpoint, rebind) = if projected {
        (
            format!("struct Store:\n    items: {ty}\n"),
            "Store",
            "    mutable Store store = input\n",
            "store.items",
            "store = replacement",
        )
    } else {
        (
            String::new(),
            ty,
            "    mutable ",
            "values",
            "values = replacement",
        )
    };
    let setup = if projected {
        setup.to_string()
    } else {
        format!("{setup}{ty} values = input\n")
    };
    format!(
        "namespace app\n{declaration}{holder}function read(view values: list[int64], extra: int64) returns int64:\n    mutable int64 total = extra\n    for value in view values:\n        total = total + value\n    return total\nfunction exercise(input: {input_ty}, replacement: {input_ty}, spare: {input_ty}, incoming: optional[int64]) returns int64:\n{setup}    int64 answer = read(view({wrapper} {endpoint}), incoming handle:\n        {rebind}\n        default 2\n    )\n    int64 after = read(view({wrapper} {endpoint}), 0)\n    return answer + after\n"
    )
}

fn transparent_retained_program(source: &str, release: bool) -> (Program, TypeInterner) {
    let file = jett_common::FileId::new(0);
    let parsed = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    assert!(
        resolved
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
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
            .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
        "{:?}",
        checked.diagnostics
    );
    let high = hir::lower(
        &parsed.module,
        &resolved,
        &checked,
        &HashMap::from([(file, jett_common::SourceOrigin::Project)]),
    )
    .expect("exact checked transparent-view Source HIR");
    let program = crate::lower(&high, &checked.interner)
        .expect("retained transparent place keeps its original Source backing");
    (program, checked.interner)
}

fn transparent_retained_leaf(mut value: &Expression, secret: bool, projected: bool) -> LocalId {
    let E::Clone(endpoint) = &value.kind else {
        panic!("one full-endpoint snapshot Clone");
    };
    value = endpoint;
    let E::View(endpoint) = &value.kind else {
        panic!("original written View");
    };
    value = endpoint;
    value = match (&value.kind, secret) {
        (E::Declassify(inner), true) | (E::Coarsen(inner), false) => inner,
        _ => panic!("original checked qualifier conversion"),
    };
    if projected {
        let E::Field { base, .. } = &value.kind else {
            panic!("original projected endpoint");
        };
        value = base;
    }
    let E::Local(local) = value.kind else {
        panic!("exact original Local backing, no introduced inner Clone");
    };
    local
}

fn transparent_retained_leaf_mut(mut value: &mut Expression) -> &mut Expression {
    while !matches!(value.kind, E::Local(_)) {
        value = match &mut value.kind {
            E::Clone(inner) | E::View(inner) | E::Coarsen(inner) | E::Declassify(inner) => inner,
            E::Field { base, .. } => base,
            _ => panic!("checked finite place spine"),
        };
    }
    value
}

fn assert_transparent_retained_scope(
    program: &Program,
    types: &TypeInterner,
    secret: bool,
    projected: bool,
) {
    let function = exercise(program);
    let (owner, loan) = retained_snapshot_stage(function);
    let original = transparent_retained_leaf(
        retained_snapshot_initializer(function, owner),
        secret,
        projected,
    );
    let source_name = if projected { "store" } else { "values" };
    assert_eq!(
        function.local(original).expect("original binding").name,
        source_name
    );
    assert_eq!(function.local(loan).expect("loan").view_source, Some(owner));
    let acquisitions =
        validate_function(program, function, types).expect("unchanged Source place proof");
    assert_eq!(
        acquisitions.owner_initializers().count(),
        0,
        "caller retains its owner"
    );
    assert!(
        acquisitions
            .arguments(read_call(function))
            .expect("exact source call")
            .is_empty()
    );
    let block = function
        .blocks
        .iter()
        .find(|block| {
            block
                .statements
                .iter()
                .any(|statement| matches!(statement.kind, S::Let { local, .. } if local == owner))
        })
        .expect("capture block");
    let capture = block
        .statements
        .iter()
        .position(|statement| matches!(statement.kind, S::Let { local, .. } if local == owner))
        .expect("capture");
    let begin = block
        .statements
        .iter()
        .position(
            |statement| matches!(statement.kind, S::BeginCallView { local, .. } if local == loan),
        )
        .expect("loan begins");
    let tag = block
        .statements
        .iter()
        .position(|statement| matches!(statement.kind, S::SumTag { .. }))
        .expect("later handled selection");
    assert!(
        capture < begin && begin < tag,
        "snapshot before later handler work"
    );
    assert!(
        function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .any(|statement| matches!(statement.kind, S::EndCallView { local } if local == loan))
    );
    crate::move_values::MoveValuePlan::analyze(program, function, types)
        .expect("independent snapshot survives original owner rebind or abort");
}

#[test]
fn caller_acquisition_retained_transparent_direct_places_survive_later_rebind() {
    for secret in [false, true] {
        let source = transparent_retained_source(secret, false);
        for release in [false, true] {
            let (program, types) = transparent_retained_program(&source, release);
            assert_transparent_retained_scope(&program, &types, secret, false);
        }
    }
}

#[test]
fn caller_acquisition_retained_transparent_projected_places_snapshot_only_the_endpoint() {
    for secret in [false, true] {
        let source = transparent_retained_source(secret, true);
        for release in [false, true] {
            let (program, types) = transparent_retained_program(&source, release);
            assert_transparent_retained_scope(&program, &types, secret, true);
        }
    }
}

#[test]
fn caller_acquisition_retained_transparent_indirect_and_return_keep_exact_loan_scope() {
    for secret in [false, true] {
        let direct = transparent_retained_source(secret, false);
        let indirect = direct.replace("    int64 answer = read(view(",
            "    function(view list[int64], int64) returns int64 callback = read\n    int64 answer = callback(view(");
        let aborted = direct.replace("        default 2", "        return 11");
        for (source, is_indirect) in [(&indirect, true), (&aborted, false)] {
            for release in [false, true] {
                let (program, types) = transparent_retained_program(source, release);
                assert_transparent_retained_scope(&program, &types, secret, false);
                assert_eq!(
                    matches!(read_call(exercise(&program)).kind, E::IndirectCall { .. }),
                    is_indirect
                );
                if !is_indirect {
                    let (_, loan) = retained_snapshot_stage(exercise(&program));
                    let returned = exercise(&program).blocks.iter().find(|block|
                        matches!(&block.terminator.kind, T::Return(Some(value)) if matches!(value.kind, E::Int(11))))
                        .expect("original handler early return");
                    assert!(returned.statements.iter().any(|statement|
                        matches!(statement.kind, S::EndCallView { local } if local == loan)));
                }
            }
        }
    }
}

#[test]
fn caller_acquisition_retained_transparent_spines_reject_added_clone_and_foreign_backing() {
    for (secret, projected) in [(false, false), (true, false), (false, true), (true, true)] {
        let source = transparent_retained_source(secret, projected);
        let (program, types) = transparent_retained_program(&source, false);
        let (owner, _) = retained_snapshot_stage(exercise(&program));
        for mutation in [
            "inner clone",
            "foreign backing",
            "endpoint type",
            "endpoint span",
        ] {
            let mut changed = program.clone();
            let function = exercise_mut(&mut changed);
            let spare = function.params[2].local;
            let initializer = retained_snapshot_initializer_mut(function, owner);
            if matches!(mutation, "endpoint type" | "endpoint span") {
                let E::Clone(endpoint) = &mut initializer.kind else {
                    panic!("snapshot");
                };
                assert!(matches!(endpoint.kind, E::View(_)), "written View");
                if mutation == "endpoint type" {
                    endpoint.ty = TypeInterner::BOOL;
                } else {
                    endpoint.span.start += 1;
                }
            } else {
                let leaf = transparent_retained_leaf_mut(initializer);
                if mutation == "inner clone" {
                    leaf.kind = E::Clone(Box::new(leaf.clone()));
                } else {
                    leaf.kind = E::Local(spare);
                }
            }
            let error = retained_snapshot_error(&changed, &types);
            assert!(
                error.contains("call ownership") || error.contains("snapshot"),
                "{mutation}: {error}"
            );
        }
        let mut prepared = program.clone();
        crate::prepare_native_sequences(&mut prepared, &types);
        crate::prepare_native_uninhabited_sums(&mut prepared, &types);
        crate::prepare_native_generated_functions(&mut prepared, &types);
        crate::validate_call_ownership(&prepared, &types)
            .expect("exact transparent place survives canonical remaps");
    }
}

#[test]
fn caller_acquisition_retained_transparent_places_keep_known_borrow_and_owned_formal_policy() {
    for (ty, declaration, wrapper) in [
        (
            "Numbers",
            "type Numbers = list[int64] where true\n",
            "coarsen",
        ),
        ("secret[list[int64]]", "", "declassify"),
    ] {
        for (owned_formal, expected) in [(false, 401), (true, 375)] {
            let formal = if owned_formal { "" } else { "view " };
            let argument = if owned_formal {
                format!("view({wrapper} values)")
            } else {
                format!("{wrapper} values")
            };
            let source = format!(
                "namespace app\n{declaration}function read({formal}values: list[int64], extra: int64) returns int64:\n    return extra\nfunction rejected(view values: {ty}, incoming: optional[int64]) returns int64:\n    return read({argument}, incoming handle:\n        default 2\n    )\n"
            );
            for release in [false, true] {
                let parsed = jett_parser::parse(&source, jett_common::FileId::new(0));
                assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
                let resolved = jett_resolve::resolve(&parsed.module);
                assert!(
                    resolved
                        .diagnostics
                        .iter()
                        .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
                    "{:?}",
                    resolved.diagnostics
                );
                let checked = jett_typecheck::check_with_options(
                    &parsed.module,
                    &resolved,
                    jett_typecheck::CheckOptions { release },
                );
                let errors = checked
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
                    .collect::<Vec<_>>();
                assert_eq!(errors.len(), 1, "{source}: {errors:?}");
                assert_eq!(errors[0].code.code(), expected, "{source}: {errors:?}");
            }
        }
    }
}

const RETAINED_PHYSICAL_PIPELINE: &str = r#"namespace app
function read(view values: list[int64], extra: int64) returns int64:
    mutable int64 total = extra
    for value in view values:
        total = total + value
    return total
function exercise(incoming: optional[int64]) returns int64:
    mutable list[int64] values = list(3)
    list[int64] spare = list(9)
    int64 answer = values into view read(incoming handle:
        values = list(8)
        default 2
    )
    return answer + read(view values, 0)
"#;

#[test]
fn caller_acquisition_retained_physical_pipeline_span_preserves_raw_source_and_order() {
    for release in [false, true] {
        let (program, types) = transparent_retained_program(RETAINED_PHYSICAL_PIPELINE, release);
        let function = exercise(&program);
        let (owner, loan) = retained_snapshot_stage(function);
        let E::Call {
            ownership: hir::CallOwnership::Source(packet),
            args,
            evaluation_order,
            ..
        } = &read_call(function).kind
        else {
            panic!("real source pipeline");
        };
        let argument = &packet.arguments[0];
        let physical = argument
            .retained_snapshot_span()
            .expect("private initial physical occurrence");
        assert_ne!(physical, argument.source_span);
        assert_eq!(evaluation_order, &[0, 1]);
        assert_eq!(argument.source_index, 0);
        assert_eq!(argument.effect, CheckedCallerEffect::RetainBorrow);
        assert_eq!(args[0].span, physical);
        let initializer = retained_snapshot_initializer(function, owner);
        let E::Clone(endpoint) = &initializer.kind else {
            panic!("complete snapshot");
        };
        assert_eq!(initializer.span, physical);
        assert_eq!(endpoint.span, physical);
        let E::View(raw) = &endpoint.kind else {
            panic!("initial physical View");
        };
        assert_eq!(raw.span, argument.source_span);
        let E::Local(original) = raw.kind else {
            panic!("raw original binding");
        };
        assert_eq!(
            function.local(original).expect("source binding").name,
            "values"
        );
        assert_eq!(function.local(loan).expect("loan").view_source, Some(owner));
        let acquired =
            validate_function(&program, function, &types).expect("physical and raw joins");
        assert_eq!(acquired.owner_initializers().count(), 0);
        assert!(
            acquired
                .arguments(read_call(function))
                .expect("call")
                .is_empty()
        );
        crate::move_values::MoveValuePlan::analyze(&program, function, &types)
            .expect("snapshot before later handler rebind");
        let mut prepared = program.clone();
        crate::prepare_native_sequences(&mut prepared, &types);
        crate::prepare_native_uninhabited_sums(&mut prepared, &types);
        crate::prepare_native_generated_functions(&mut prepared, &types);
        crate::validate_call_ownership(&prepared, &types)
            .expect("no span rewrite in structural remaps");
    }
}

#[test]
fn caller_acquisition_retained_physical_reflected_getter_keeps_initial_view_and_raw_binding() {
    let original_source = r#"namespace app
struct Record:
    values: list[int64]
function exercise(view source: Record, view field: TypeField) returns list[int64]:
    return source into view type.field_value[Record, list[int64]](view field)
"#;
    // Match the existing recursive object regression: non-Exact field plans
    // enter ordered call staging before the raw read and refinement checks.
    let ordered_source = r#"namespace app
type Positive = int64 where value > 0
struct Record:
    values: list[optional[result[map[int64, list[int64]], set[int64]]]]
function exercise(view source: Record, view field: TypeField) returns list[optional[result[map[Positive, list[Positive]], set[Positive]]]]:
    return source into view type.field_value[Record, list[optional[result[map[Positive, list[Positive]], set[Positive]]]]](view field)
"#;
    for release in [false, true] {
        for (source, ordered) in [(original_source, false), (ordered_source, true)] {
            let (program, types) = transparent_retained_program(source, release);
            let function = exercise(&program);
            let call = function
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .find_map(|statement| match &statement.kind {
                    S::Let { value, .. } | S::Evaluate(value)
                        if matches!(
                            value.kind,
                            E::Intrinsic {
                                intrinsic: hir::IntrinsicId::TypeFieldValue,
                                ..
                            }
                        ) =>
                    {
                        Some(value)
                    }
                    _ => None,
                })
                .or_else(|| {
                    function
                        .blocks
                        .iter()
                        .find_map(|block| match &block.terminator.kind {
                            T::Return(Some(value))
                                if matches!(
                                    value.kind,
                                    E::Intrinsic {
                                        intrinsic: hir::IntrinsicId::TypeFieldValue,
                                        ..
                                    }
                                ) =>
                            {
                                Some(value)
                            }
                            _ => None,
                        })
                })
                .expect("real reflected getter invocation");
            let E::Intrinsic {
                ownership: hir::CallOwnership::Source(packet),
                args,
                ..
            } = &call.kind
            else {
                panic!("unchanged reflected Source packet");
            };
            let argument = &packet.arguments[0];
            assert_eq!(argument.effect, CheckedCallerEffect::RetainBorrow);
            assert_eq!(
                argument.syntax,
                jett_typecheck::CheckedCallerSyntax::WrittenView
            );
            if !ordered {
                // Exact field plans use the existing unstaged raw getter path.
                assert_eq!(argument.staging, hir::ArgumentStaging::Original);
                let E::View(raw) = &args[0].kind else {
                    panic!("initial written physical View");
                };
                assert!(matches!(raw.kind, E::Local(local) if local == function.params[0].local));
                validate_function(&program, function, &types)
                    .expect("exact field getter stays Original");
                crate::move_values::MoveValuePlan::analyze(&program, function, &types)
                    .expect("existing unstaged reflected read");
                continue;
            }
            let hir::ArgumentStaging::RetainedSnapshot { owner, loan } = argument.staging else {
                panic!("ordinary Record endpoint snapshot");
            };
            let physical = argument
                .retained_snapshot_span()
                .expect("initial pipeline occurrence");
            assert_ne!(physical, argument.source_span);
            assert_eq!(args[0].span, physical);
            let initializer = retained_snapshot_initializer(function, owner);
            let E::Clone(endpoint) = &initializer.kind else {
                panic!("full endpoint snapshot");
            };
            let E::View(raw) = &endpoint.kind else {
                panic!("original written physical View");
            };
            assert_eq!(initializer.span, physical);
            assert_eq!(endpoint.span, physical);
            assert_eq!(raw.span, argument.source_span);
            assert!(matches!(raw.kind, E::Local(local) if local == function.params[0].local));
            assert_eq!(function.local(loan).expect("loan").view_source, Some(owner));
            validate_function(&program, function, &types)
                .expect("reflected certificate and exact raw source stay active");
            crate::move_values::MoveValuePlan::analyze(&program, function, &types)
                .expect("existing reflected owning-result/read analysis");
        }
    }
}

#[test]
fn caller_acquisition_retained_physical_span_does_not_replace_raw_or_backing_authority() {
    let (program, types) = transparent_retained_program(RETAINED_PHYSICAL_PIPELINE, false);
    let (owner, loan) = retained_snapshot_stage(exercise(&program));
    for mutation in [
        "physical occurrence",
        "raw occurrence",
        "foreign binding",
        "physical loan",
        "public raw witness",
    ] {
        let mut changed = program.clone();
        let function = exercise_mut(&mut changed);
        if mutation == "public raw witness" {
            let E::Call {
                ownership: hir::CallOwnership::Source(packet),
                ..
            } = &mut read_call_mut(function).kind
            else {
                panic!("source pipeline");
            };
            packet.arguments[0].source_span.start += 1;
        } else if mutation == "physical loan" {
            let E::Call { args, .. } = &mut read_call_mut(function).kind else {
                panic!("call");
            };
            args[0].span.start += 1;
        } else if mutation == "physical occurrence" {
            let span = {
                let initializer = retained_snapshot_initializer_mut(function, owner);
                initializer.span.start += 1;
                let E::Clone(endpoint) = &mut initializer.kind else {
                    panic!("snapshot");
                };
                endpoint.span = initializer.span;
                initializer.span
            };
            // Preserve owner/initializer agreement so only the sealed original
            // physical occurrence can reject this coordinated replacement.
            function.locals[owner.index() as usize].span = span;
        } else {
            let spare = function
                .locals
                .iter()
                .find(|local| local.name == "spare")
                .expect("same-type source")
                .id;
            let initializer = retained_snapshot_initializer_mut(function, owner);
            let E::Clone(endpoint) = &mut initializer.kind else {
                panic!("snapshot");
            };
            let E::View(raw) = &mut endpoint.kind else {
                panic!("physical pipeline View");
            };
            if mutation == "raw occurrence" {
                raw.span.start += 1;
            } else {
                raw.kind = E::Local(spare);
            }
        }
        let error = retained_snapshot_error(&changed, &types);
        assert!(
            error.contains("retained snapshot") || error.contains("call ownership"),
            "{mutation}: {error}"
        );
    }
    assert_eq!(
        exercise(&program)
            .local(loan)
            .expect("original loan")
            .view_source,
        Some(owner)
    );
}
const FLOW_PROJECTED_SOURCE: &str = r#"namespace app
machine Session:
    states:
        active(items: list[int64])
        cached(items: list[int64])
        empty
function read(view values: list[int64]) returns int64:
    return 7
function exercise(source: Session, other: Session) returns int64:
    if source at active:
        return read(view source.items)
    return 0
machine TaskState:
    states:
        ready(value: nothing)
        cached(value: nothing)
        empty
function depth(value: nothing) returns int64:
    return 3
function copied(source: TaskState) returns int64:
    if source at ready:
        return depth(source.value)
    return 0
"#;

fn flow_projected_mir_field(value: &mut Expression) -> &mut Expression {
    if matches!(value.kind, E::View(_)) {
        let E::View(inner) = &mut value.kind else {
            unreachable!()
        };
        return flow_projected_mir_field(inner);
    }
    assert!(matches!(value.kind, E::Field { .. }));
    value
}

fn flow_projected_mir_call_mut(function: &mut Function) -> &mut Expression {
    for block in &mut function.blocks {
        if let T::Return(Some(value)) = &mut block.terminator.kind {
            if matches!(value.kind, E::Call { .. }) {
                return value;
            }
        }
    }
    panic!("guarded original source call")
}

#[test]
fn caller_acquisition_flow_projection_keeps_bare_storage_and_exact_original_state() {
    let (mut program, types) = source_program(FLOW_PROJECTED_SOURCE);
    for name in ["exercise", "copied"] {
        let function = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == name)
            .unwrap();
        let E::Call {
            ownership: hir::CallOwnership::Source(packet),
            ..
        } = &read_call(function).kind
        else {
            panic!("guarded source packet");
        };
        let root = packet.arguments[0]
            .source_witness()
            .projection_root()
            .unwrap();
        assert!(matches!(
            types.resolve(root.2),
            jett_types::Type::MachineState { .. }
        ));
        assert!(matches!(
            types.resolve(root.3),
            jett_types::Type::Machine(_)
        ));
        assert_eq!(function.local(root.0).unwrap().ty, root.3);
        assert_eq!(
            packet.arguments[0].effect,
            if name == "exercise" {
                CheckedCallerEffect::RetainBorrow
            } else {
                CheckedCallerEffect::Copy
            }
        );
        assert!(matches!(
            packet.arguments[0].staging,
            hir::ArgumentStaging::Original
        ));
        validate_function(&program, function, &types)
            .expect("original flow projection Source proof");
    }
    crate::prepare_native_sequences(&mut program, &types);
    crate::prepare_native_uninhabited_sums(&mut program, &types);
    crate::prepare_native_generated_functions(&mut program, &types);
    crate::validate_call_ownership(&program, &types)
        .expect("dense remaps preserve private root without a new loan");
}

#[test]
fn caller_acquisition_flow_projection_rejects_changed_state_root_span_and_endpoint() {
    let (program, mut types) = source_program(FLOW_PROJECTED_SOURCE);
    let function = exercise(&program);
    let other = function
        .locals
        .iter()
        .find(|local| local.name == "other")
        .unwrap()
        .id;
    let E::Call {
        ownership: hir::CallOwnership::Source(packet),
        ..
    } = &read_call(function).kind
    else {
        panic!("source packet");
    };
    let root = packet.arguments[0]
        .source_witness()
        .projection_root()
        .unwrap();
    let jett_types::Type::Machine(machine) = *types.resolve(root.3) else {
        panic!("bare owner");
    };
    let cached = types.resolve_machine(machine).state_id("cached").unwrap();
    let sibling = types.intern(jett_types::Type::MachineState {
        machine,
        state: cached,
    });
    for mutation in ["state", "span", "local", "field", "endpoint"] {
        let mut changed = program.clone();
        let E::Call { args, .. } =
            &mut flow_projected_mir_call_mut(exercise_mut(&mut changed)).kind
        else {
            panic!("source call");
        };
        let value = flow_projected_mir_field(&mut args[0]);
        if mutation == "endpoint" {
            value.ty = TypeInterner::BOOL;
        } else {
            let E::Field {
                base,
                owner_type,
                field,
            } = &mut value.kind
            else {
                panic!("field");
            };
            match mutation {
                "state" => {
                    *owner_type = sibling;
                    base.ty = sibling;
                }
                "span" => base.span.start += 1,
                "local" => base.kind = E::Local(other),
                "field" => *field = hir::FieldId::new(1),
                _ => unreachable!(),
            }
        }
        assert!(
            validate_function(&changed, exercise(&changed), &types).is_err(),
            "{mutation}"
        );
    }
}

const MAP_GET_PUBLIC_DECLARATION: &str = "export function get[K, V](items: map[K, V], view key: K) returns optional[V]:\n    return map.__get[K, V](items, view key)\n";

fn intrinsic_owning_map_program(release: bool) -> (Program, TypeInterner) {
    assert!(include_str!("../../../../stdlib/map.jett").contains(MAP_GET_PUBLIC_DECLARATION));
    let stdlib_file = jett_common::FileId::new(jett_common::STDLIB_FILE_ID_START);
    let project_file = jett_common::FileId::new(0);
    let stdlib_source = format!(
        "namespace map\nexport interface Named:\n    function name(view self: Named) returns string\n{MAP_GET_PUBLIC_DECLARATION}function retained(view items: map[string, Named], incoming: optional[string]) returns optional[Named]:\n    return map.__get[string, Named](view items, incoming handle:\n        default \"key\"\n    )\n"
    );
    let project_source = "namespace app\nfunction exercise(items: map[string, map.Named], view key: string) returns optional[map.Named]:\n    use map\n    return map.get[string, map.Named](items, view key)\n";
    let mut stdlib = jett_parser::parse(&stdlib_source, stdlib_file);
    let mut project = jett_parser::parse(project_source, project_file);
    assert!(stdlib.errors.is_empty(), "{:?}", stdlib.errors);
    assert!(project.errors.is_empty(), "{:?}", project.errors);
    stdlib.module.items.append(&mut project.module.items);
    let resolved = jett_resolve::resolve(&stdlib.module);
    assert!(
        resolved
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
        "{:?}",
        resolved.diagnostics
    );
    let checked = jett_typecheck::check_with_options(
        &stdlib.module,
        &resolved,
        jett_typecheck::CheckOptions { release },
    );
    assert!(
        checked
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
        "{:?}",
        checked.diagnostics
    );
    let origins = HashMap::from([
        (stdlib_file, jett_common::SourceOrigin::Stdlib),
        (project_file, jett_common::SourceOrigin::Project),
    ]);
    let high = hir::lower(&stdlib.module, &resolved, &checked, &origins)
        .expect("original compiler-origin facade and ordinary source caller");
    (
        crate::lower(&high, &checked.interner).expect("closed intrinsic owning staging"),
        checked.interner,
    )
}

fn map_get_function(program: &Program) -> &Function {
    let mut matches = program
        .functions
        .iter()
        .filter(|function| function.identity.declaration.name == "get");
    let function = matches.next().expect("concrete public map.get body");
    assert!(
        matches.next().is_none(),
        "one selected public instantiation"
    );
    function
}

fn map_get_function_mut(program: &mut Program) -> &mut Function {
    program
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "get")
        .expect("concrete public map.get body")
}

fn map_get_intrinsic(function: &Function) -> &Expression {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .find_map(|statement| {
            let (S::Let { value, .. } | S::Evaluate(value)) = &statement.kind else {
                return None;
            };
            matches!(
                value.kind,
                E::Intrinsic {
                    intrinsic: hir::IntrinsicId::MapGet,
                    ..
                }
            )
            .then_some(value)
        })
        .expect("materialized exact MapGet operation")
}

fn map_get_intrinsic_mut(function: &mut Function) -> &mut Expression {
    function
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.statements)
        .find_map(|statement| {
            let (S::Let { value, .. } | S::Evaluate(value)) = &mut statement.kind else {
                return None;
            };
            matches!(
                value.kind,
                E::Intrinsic {
                    intrinsic: hir::IntrinsicId::MapGet,
                    ..
                }
            )
            .then_some(value)
        })
        .expect("materialized exact MapGet operation")
}

#[test]
fn caller_acquisition_intrinsic_map_get_transfers_opaque_containing_owner_without_clone() {
    for release in [false, true] {
        let (program, types) = intrinsic_owning_map_program(release);
        let function = map_get_function(&program);
        let call = map_get_intrinsic(function);
        let E::Intrinsic {
            ownership: hir::CallOwnership::Source(packet),
            args,
            evaluation_order,
            ..
        } = &call.kind
        else {
            panic!("exact checked intrinsic packet");
        };
        let argument = &packet.arguments[0];
        assert_eq!(argument.syntax, jett_typecheck::CheckedCallerSyntax::Bare);
        assert_eq!(argument.effect, CheckedCallerEffect::RelinquishOwned);
        assert_eq!(
            argument.callee_access,
            jett_typecheck::CheckedCalleeAccess::View
        );
        assert_eq!(
            argument.physical_access,
            jett_typecheck::CheckedCalleeAccess::View
        );
        assert_eq!(evaluation_order.as_slice(), &[0, 1]);
        let hir::ArgumentStaging::Relinquished { owner, loan } = argument.staging else {
            panic!("source owning transfer is staged");
        };
        assert_ne!(owner, loan);
        let initializer = observation_initializer(function, owner);
        assert!(matches!(initializer.kind, E::Local(local) if local == function.params[0].local));
        assert_eq!(initializer.ty, argument.actual_type);
        assert_eq!(initializer.span, argument.source_span);
        let Type::Map(key, value) = types.resolve(initializer.ty) else {
            panic!("exact map owner");
        };
        assert_eq!(*key, TypeInterner::STRING);
        assert!(matches!(types.resolve(*value), Type::Interface(_)));
        assert!(!crate::handlers::can_snapshot_view(&types, initializer.ty));
        assert_eq!(function.local(owner).unwrap().view_source, None);
        assert_eq!(function.local(loan).unwrap().view_source, Some(owner));
        assert!(
            matches!(&args[0].kind, E::View(value) if matches!(value.kind, E::Local(id) if id == loan))
        );
        assert_eq!(
            packet.arguments[1].syntax,
            jett_typecheck::CheckedCallerSyntax::WrittenView
        );
        assert_eq!(packet.arguments[1].effect, CheckedCallerEffect::Copy);
        let hir::ArgumentStaging::Copied { value: key_owner } = packet.arguments[1].staging else {
            panic!("the implicitly copyable string key keeps its separate copied slot");
        };
        let key_initializer = observation_initializer(function, key_owner);
        assert_eq!(key_initializer.ty, TypeInterner::STRING);
        assert!(matches!(key_initializer.kind, E::Clone(_)));
        let block = function
            .blocks
            .iter()
            .find(|block| {
                block.statements.iter().any(|statement|
            matches!(&statement.kind, S::Let { value, .. } if std::ptr::eq(value, call)))
            })
            .expect("one consuming operation owns both loans");
        let owner_at = block
            .statements
            .iter()
            .position(|statement| matches!(statement.kind, S::Let { local, .. } if local == owner))
            .unwrap();
        let begin_at = block.statements.iter().position(|statement|
            matches!(statement.kind, S::BeginCallView { local, .. } if local == loan)).unwrap();
        let call_at = block.statements.iter().position(|statement|
            matches!(&statement.kind, S::Let { value, .. } if std::ptr::eq(value, call))).unwrap();
        let end_at = block
            .statements
            .iter()
            .position(
                |statement| matches!(statement.kind, S::EndCallView { local } if local == loan),
            )
            .unwrap();
        assert!(owner_at < begin_at && begin_at < call_at && call_at < end_at);
        assert_eq!(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .filter(
                    |statement| matches!(statement.kind, S::Let { local, .. } if local == owner)
                )
                .count(),
            1
        );
        let acquisitions = validate_function(&program, function, &types)
            .expect("original map move and exact loan");
        assert_eq!(acquisitions.owner_initializers().count(), 1);
        let acquisition = acquisitions.owner_initializer(owner).unwrap();
        assert_eq!(acquisition.binding, Some(function.params[0].local));
        assert_eq!(acquisition.effect, CheckedCallerEffect::RelinquishOwned);
        let E::Call {
            ownership: hir::CallOwnership::Source(public),
            ..
        } = &read_call(exercise(&program)).kind
        else {
            panic!("ordinary caller preserves public source ownership");
        };
        assert_eq!(
            public.arguments[0].effect,
            CheckedCallerEffect::TransferOwned
        );
        assert_eq!(
            public.arguments[0].callee_access,
            jett_typecheck::CheckedCalleeAccess::Owned
        );
        crate::validate_call_ownership(&program, &types)
            .expect("all source and intrinsic packets remain exact");
        crate::move_values::MoveValuePlan::analyze(&program, function, &types)
            .expect("map owner consumed once and released after its scoped borrow");
    }
}

#[test]
fn caller_acquisition_intrinsic_map_get_keeps_retained_maps_borrowed_across_later_handler() {
    for release in [false, true] {
        let (program, types) = intrinsic_owning_map_program(release);
        let function = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "retained")
            .expect("written view control");
        let E::Intrinsic {
            ownership: hir::CallOwnership::Source(packet),
            ..
        } = &map_get_intrinsic(function).kind
        else {
            panic!("exact intrinsic packet");
        };
        let argument = &packet.arguments[0];
        assert_eq!(
            argument.syntax,
            jett_typecheck::CheckedCallerSyntax::WrittenView
        );
        assert_eq!(argument.effect, CheckedCallerEffect::RetainBorrow);
        let hir::ArgumentStaging::Borrowed { loan } = argument.staging else {
            panic!("retained map gains only a scoped loan");
        };
        assert_eq!(
            function.local(loan).unwrap().view_source,
            Some(function.params[0].local)
        );
        assert!(!crate::handlers::can_snapshot_view(
            &types,
            argument.actual_type
        ));
        let acquisitions =
            validate_function(&program, function, &types).expect("retained map and handled key");
        assert_eq!(
            acquisitions.owner_initializers().count(),
            0,
            "neither the retained map nor the copied key transfers a source owner"
        );
        assert_eq!(packet.arguments[1].effect, CheckedCallerEffect::Copy);
        let hir::ArgumentStaging::Copied { value: copied } = packet.arguments[1].staging else {
            panic!("the handled string key uses its own copied slot");
        };
        assert_eq!(
            observation_initializer(function, copied).ty,
            TypeInterner::STRING
        );
        crate::move_values::MoveValuePlan::analyze(&program, function, &types)
            .expect("later handler does not consume the retained map");
    }
}

#[test]
fn caller_acquisition_intrinsic_map_get_refuses_missing_or_forged_owner_staging() {
    for release in [false, true] {
        let (program, types) = intrinsic_owning_map_program(release);
        let E::Intrinsic {
            ownership: hir::CallOwnership::Source(packet),
            ..
        } = &map_get_intrinsic(map_get_function(&program)).kind
        else {
            panic!("source intrinsic packet");
        };
        let hir::ArgumentStaging::Relinquished { owner, loan } = packet.arguments[0].staging else {
            panic!("original owning stage");
        };
        for mutation in [
            "stage",
            "effect",
            "aliased owner",
            "added clone",
            "missing owner",
            "missing end",
            "loan backing",
            "order",
        ] {
            let mut changed = program.clone();
            let function = map_get_function_mut(&mut changed);
            match mutation {
                "stage" | "effect" | "aliased owner" | "order" => {
                    let E::Intrinsic {
                        ownership: hir::CallOwnership::Source(packet),
                        evaluation_order,
                        ..
                    } = &mut map_get_intrinsic_mut(function).kind
                    else {
                        panic!("source intrinsic packet");
                    };
                    match mutation {
                        "stage" => packet.arguments[0].staging = hir::ArgumentStaging::Original,
                        "effect" => packet.arguments[0].effect = CheckedCallerEffect::RetainBorrow,
                        "aliased owner" => {
                            packet.arguments[0].staging =
                                hir::ArgumentStaging::Relinquished { owner: loan, loan }
                        }
                        "order" => *evaluation_order = vec![0, 0],
                        _ => unreachable!(),
                    }
                }
                "added clone" => {
                    let initializer = function
                        .blocks
                        .iter_mut()
                        .flat_map(|block| &mut block.statements)
                        .find_map(|statement| match &mut statement.kind {
                            S::Let { local, value } if *local == owner => Some(value),
                            _ => None,
                        })
                        .expect("unique original owner initializer");
                    initializer.kind = E::Clone(Box::new(initializer.clone()));
                }
                "missing owner" | "missing end" => {
                    let (block, index) = function
                        .blocks
                        .iter()
                        .enumerate()
                        .find_map(|(block, body)| {
                            body.statements
                                .iter()
                                .position(|statement| match statement.kind {
                                    S::Let { local, .. } if mutation == "missing owner" => {
                                        local == owner
                                    }
                                    S::EndCallView { local } if mutation == "missing end" => {
                                        local == loan
                                    }
                                    _ => false,
                                })
                                .map(|index| (block, index))
                        })
                        .expect("original owner or scope endpoint");
                    function.blocks[block].statements.remove(index);
                }
                "loan backing" => {
                    function.locals[loan.index() as usize].view_source =
                        Some(function.params[0].local)
                }
                _ => unreachable!(),
            }
            assert!(
                validate_function(&changed, map_get_function(&changed), &types).is_err(),
                "{mutation}"
            );
        }
    }
}
