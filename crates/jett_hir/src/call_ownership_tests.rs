//! Source-derived caller-packet corruption controls. Not executed in this draft.
use super::*;
use jett_diagnostics::Severity;

const NAMED: &str = r#"function observe(view first: list[int64], view second: list[int64]) returns int64:
    return 7
function alternate(view first: list[int64], view second: list[int64]) returns int64:
    return 9
function exercise(left: list[int64], right: list[int64], unrelated: list[string]) returns int64:
    return observe(second: right, first: left)
"#;

fn checked_source(source: &str) -> (Program, CheckResult) {
    checked_source_with_suites(source, false)
}

fn checked_source_with_suites(source: &str, include_suites: bool) -> (Program, CheckResult) {
    let file = FileId::new(0);
    let parsed = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "parse: {:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    assert!(
        resolved
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Error),
        "resolve: {:?}",
        resolved.diagnostics,
    );
    let checked = jett_typecheck::check(&parsed.module, &resolved);
    assert!(
        checked
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Error),
        "check: {:?}",
        checked.diagnostics,
    );
    let origins = HashMap::from([(file, SourceOrigin::Project)]);
    let lowered = if include_suites {
        lower_with_test_bodies(&parsed.module, &resolved, &checked, &origins)
    } else {
        lower(&parsed.module, &resolved, &checked, &origins)
    };
    let program = lowered.expect("source lowers");
    validate_program_call_ownership(&program, &checked.interner)
        .expect("original packet validates");
    (program, checked)
}

fn exercise_index(program: &Program) -> usize {
    program
        .functions
        .iter()
        .position(|function| function.identity.declaration.name == "exercise")
        .expect("source exercise")
}

fn returned_call(function: &Function) -> &Expression {
    function
        .body
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            StatementKind::Return(Some(value)) => Some(value),
            _ => None,
        })
        .expect("returned source call")
}

fn returned_call_mut(function: &mut Function) -> &mut Expression {
    function
        .body
        .statements
        .iter_mut()
        .find_map(|statement| match &mut statement.kind {
            StatementKind::Return(Some(value)) => Some(value),
            _ => None,
        })
        .expect("returned source call")
}

fn source_packet_mut(expression: &mut Expression) -> &mut SourceCallOwnership {
    let packet = match &mut expression.kind {
        ExpressionKind::Call { ownership, .. }
        | ExpressionKind::IndirectCall { ownership, .. }
        | ExpressionKind::Intrinsic { ownership, .. } => ownership,
        _ => panic!("source invocation"),
    };
    let CallOwnership::Source(source) = packet else {
        panic!("checked source packet");
    };
    source
}

fn local_id(function: &Function, name: &str) -> LocalId {
    function
        .locals
        .iter()
        .find(|local| local.name == name)
        .expect("source local")
        .id
}

fn rejects(program: &Program, checked: &CheckResult, case: &str) {
    let errors = validate_program_call_ownership(program, &checked.interner)
        .expect_err("corrupted checked invocation must be refused");
    assert!(
        errors.iter().any(|error| error.message.contains("call ownership")
            || error.message.contains("caller") || error.message.contains("invocation")
            || (case == "source-backed tail" && matches!(error.message.as_str(),
                "generated owner cannot acquire a borrowed binding"
                    | "generated operand differs from its original typed occurrence or backing"))),
        "{case}: {errors:?}",
    );
}

#[test]
fn caller_packets_preserve_named_order_and_borrowed_source_authority() {
    let (program, checked) = checked_source(NAMED);
    let function = &program.functions[exercise_index(&program)];
    let ExpressionKind::Call {
        args,
        evaluation_order,
        ownership,
        ..
    } = &returned_call(function).kind
    else {
        panic!("direct named invocation");
    };
    let CallOwnership::Source(source) = ownership else {
        panic!("source packet");
    };
    assert_eq!(evaluation_order, &[1, 0]);
    assert_eq!(
        source
            .arguments
            .iter()
            .map(|argument| argument.source_index)
            .collect::<Vec<_>>(),
        [1, 0]
    );
    assert!(source.arguments.iter().all(|argument| {
        argument.effect == jett_typecheck::CheckedCallerEffect::RelinquishOwned
            && argument.syntax == jett_typecheck::CheckedCallerSyntax::Bare
            && argument.staging == ArgumentStaging::Original
    }));
    assert!(
        matches!(args[0].kind, ExpressionKind::Local(local) if local == local_id(function, "left"))
    );
    assert!(
        matches!(args[1].kind, ExpressionKind::Local(local) if local == local_id(function, "right"))
    );
    assert!(matches!(
        checked.interner.resolve(source.arguments[0].actual_type),
        Type::List(_)
    ));

    let source = "function observe(view value: list[int64]) returns int64:\n    return 7\nfunction exercise(view value: list[int64]) returns int64:\n    return observe(view value)\n";
    let (program, _) = checked_source(source);
    let expression = returned_call(&program.functions[exercise_index(&program)]);
    let ExpressionKind::Call {
        ownership: CallOwnership::Source(source),
        ..
    } = &expression.kind
    else {
        panic!("retained source invocation");
    };
    let argument = &source.arguments[0];
    assert_eq!(
        argument.effect,
        jett_typecheck::CheckedCallerEffect::RetainBorrow
    );
    assert!(matches!(
        argument.origin,
        CallerOrigin::Binding(CallerBindingFact {
            mode: CallerBindingMode::View {
                source: CallerViewSource::Other
            },
            ..
        })
    ));
}

#[test]
fn caller_packets_reject_count_order_and_source_fact_corruption() {
    for case in [
        "count",
        "order",
        "source index",
        "effect",
        "syntax",
        "context",
        "origin",
    ] {
        let (mut program, checked) = checked_source(NAMED);
        let index = exercise_index(&program);
        let expression = returned_call_mut(&mut program.functions[index]);
        if case == "order" {
            let ExpressionKind::Call {
                evaluation_order, ..
            } = &mut expression.kind
            else {
                unreachable!();
            };
            *evaluation_order = vec![0, 1];
        } else {
            let packet = source_packet_mut(expression);
            match case {
                "count" => {
                    packet.arguments.pop();
                }
                "source index" => packet.arguments[0].source_index = 0,
                "effect" => {
                    packet.arguments[0].effect = jett_typecheck::CheckedCallerEffect::RetainBorrow
                }
                "syntax" => {
                    packet.arguments[0].syntax = jett_typecheck::CheckedCallerSyntax::WrittenView
                }
                "context" => packet.context = jett_typecheck::CheckedOwnershipContext::Verify,
                "origin" => packet.arguments[0].origin = CallerOrigin::OwnedExpression,
                _ => unreachable!(),
            }
        }
        rejects(&program, &checked, case);
    }
}

#[test]
fn caller_packets_bind_the_executed_target_and_complete_signature() {
    for case in ["physical target", "packet target", "signature"] {
        let (mut program, mut checked) = checked_source(NAMED);
        let alternate = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "alternate")
            .unwrap()
            .id;
        let index = exercise_index(&program);
        let expression = returned_call_mut(&mut program.functions[index]);
        if case == "physical target" {
            let ExpressionKind::Call { function, .. } = &mut expression.kind else {
                unreachable!();
            };
            *function = alternate;
        } else {
            let packet = source_packet_mut(expression);
            if case == "packet target" {
                packet.target = CallTarget::Function(alternate);
            } else {
                let ty = packet.arguments[0].parameter_type;
                packet.shape = jett_typecheck::CheckedInvocationShape::Function {
                    signature_type: checked.interner.intern(Type::Function {
                        params: vec![ty, ty],
                        view_params: vec![true, true],
                        return_type: TypeInterner::BOOL,
                    }),
                };
            }
        }
        rejects(&program, &checked, case);
    }
}

#[test]
fn caller_packets_require_the_original_operand_and_exact_local_metadata() {
    for case in [
        "different same-type owner",
        "different type",
        "local metadata",
        "foreign local",
    ] {
        let (mut program, checked) = checked_source(NAMED);
        let index = exercise_index(&program);
        let function = &mut program.functions[index];
        let left = local_id(function, "left");
        let right = local_id(function, "right");
        let unrelated = local_id(function, "unrelated");
        if case == "local metadata" {
            let replacement = function.locals[unrelated.index() as usize].ty;
            function.locals[left.index() as usize].ty = replacement;
        } else {
            let expression = returned_call_mut(function);
            let ExpressionKind::Call { args, .. } = &mut expression.kind else {
                unreachable!();
            };
            args[0].kind = ExpressionKind::Local(match case {
                "different same-type owner" => right,
                "different type" => unrelated,
                "foreign local" => LocalId::new(u32::MAX),
                _ => unreachable!(),
            });
        }
        rejects(&program, &checked, case);
    }
}

#[test]
fn caller_packets_cannot_import_mir_loan_staging_into_original_hir() {
    let (mut program, checked) = checked_source(NAMED);
    let index = exercise_index(&program);
    let owner = local_id(&program.functions[index], "left");
    let loan = local_id(&program.functions[index], "right");
    let expression = returned_call_mut(&mut program.functions[index]);
    let ExpressionKind::Call { ownership, .. } = &mut expression.kind else {
        unreachable!();
    };
    ownership
        .stage_parameter(0, Some(owner), loan)
        .expect("typed MIR stage mutation");
    rejects(&program, &checked, "MIR stage in original HIR");
}

#[test]
fn caller_packets_preserve_safe_secret_promotion_but_reject_forged_operands() {
    let source = "function observe(view value: secret[list[int64]]) returns int64:\n    return 7\nfunction exercise(value: list[int64]) returns int64:\n    return observe(value)\n";
    let (mut program, checked) = checked_source(source);
    let index = exercise_index(&program);
    let expression = returned_call_mut(&mut program.functions[index]);
    let ExpressionKind::Call {
        args,
        ownership: CallOwnership::Source(packet),
        ..
    } = &mut expression.kind
    else {
        panic!("source promoted operand");
    };
    assert_ne!(
        packet.arguments[0].actual_type,
        packet.arguments[0].parameter_type
    );
    assert!(matches!(
        checked.interner.resolve(packet.arguments[0].parameter_type),
        Type::Secret(_)
    ));
    args[0].ty = TypeInterner::INT64;
    rejects(&program, &checked, "invented promoted operand type");
}

const REFLECTED_METADATA_SOURCE: &str = r#"namespace app
struct Record:
    value: int64
function exercise(view value: Record, view field: TypeField) returns int64:
    return type.field_value[Record, int64](view value, view field)
"#;

#[test]
fn caller_packets_preserve_canonical_owned_metadata_and_reject_tail_corruption() {
    let (program, checked) = checked_source(REFLECTED_METADATA_SOURCE);
    let expression = returned_call(&program.functions[exercise_index(&program)]);
    let ExpressionKind::Intrinsic {
        args,
        ownership: CallOwnership::Source(source),
        ..
    } = &expression.kind
    else {
        panic!("reflected source intrinsic");
    };
    assert_eq!(source.arguments.len(), 2);
    assert_eq!(source.generated_operands.len(), 1);
    assert_eq!(args.len(), 3);
    assert_eq!(
        source.generated_operands[0].acquisition,
        GeneratedAcquisition::OwnedExpression
    );
    assert!(!jett_typecheck::ownership::is_implicitly_copyable(
        &checked.interner,
        args[2].ty
    ));
    validate_compiler_metadata_operand(&args[2], source, &checked.interner)
        .expect("canonical compiler metadata");

    for case in [
        "tail acquisition",
        "source-backed tail",
        "metadata schema",
        "tail evaluation order",
    ] {
        let (mut program, mut checked) = checked_source(REFLECTED_METADATA_SOURCE);
        let index = exercise_index(&program);
        let field = local_id(&program.functions[index], "field");
        let expression = returned_call_mut(&mut program.functions[index]);
        let ExpressionKind::Intrinsic {
            args,
            ownership: CallOwnership::Source(source),
            evaluation_order,
            ..
        } = &mut expression.kind
        else {
            panic!("reflected source intrinsic");
        };
        match case {
            "tail acquisition" => {
                source.generated_operands[0].acquisition = GeneratedAcquisition::Copy
            }
            "source-backed tail" => args[2].kind = ExpressionKind::Local(field),
            "metadata schema" => {
                let Type::Struct(id) = *checked.interner.resolve(args[2].ty) else {
                    panic!("metadata struct");
                };
                let mut definition = checked.interner.resolve_struct(id).clone();
                definition.fields[0].1 = TypeInterner::STRING;
                checked.interner.update_struct(id, definition);
            }
            "tail evaluation order" => *evaluation_order = vec![2, 0, 1],
            _ => unreachable!(),
        }
        rejects(&program, &checked, case);
    }
}

#[test]
fn caller_packets_seal_checked_results_and_exact_physical_operands() {
    for case in ["source result", "converted operand"] {
        let (mut program, checked) = checked_source(NAMED);
        let index = exercise_index(&program);
        let expression = returned_call_mut(&mut program.functions[index]);
        if case == "source result" {
            expression.ty = TypeInterner::BOOL;
        } else {
            let ExpressionKind::Call { args, .. } = &mut expression.kind else {
                panic!("source call");
            };
            args[0].ty = TypeInterner::STRING;
        }
        rejects(&program, &checked, case);
    }

    let source = "function exercise(callback: function(view list[int64]) returns int64, value: list[int64]) returns int64:\n    return callback(value)\n";
    let (mut program, checked) = checked_source(source);
    let index = exercise_index(&program);
    let expression = returned_call_mut(&mut program.functions[index]);
    let ExpressionKind::IndirectCall { args, .. } = &mut expression.kind else {
        panic!("source indirect call");
    };
    args[0].ty = TypeInterner::STRING;
    rejects(&program, &checked, "indirect converted operand");
}

#[test]
fn caller_packets_preserve_checked_contextual_secret_producer_results() {
    let source = "function produce() returns list[int64]:\n    return list(1)\nfunction exercise() returns secret[list[int64]]:\n    return produce()\n";
    let (mut program, checked) = checked_source(source);
    let index = exercise_index(&program);
    let expression = returned_call_mut(&mut program.functions[index]);
    assert!(
        matches!(checked.interner.resolve(expression.ty), Type::Secret(inner)
        if matches!(checked.interner.resolve(*inner), Type::List(_)))
    );
    expression.ty = TypeInterner::STRING;
    rejects(&program, &checked, "forged contextual producer result");
}

#[test]
fn caller_packets_separate_raw_and_qualified_source_occurrences() {
    let sources = [
        "function observe(view value: secret[list[int64]]) returns int64:\n    return 7\nfunction exercise(value: list[int64]) returns int64:\n    return observe(value)\n",
        "struct Packet:\n    items: list[int64]\nfunction observe(view value: secret[list[int64]]) returns int64:\n    return 7\nfunction exercise(value: Packet) returns int64:\n    return observe(value.items)\n",
        "struct Packet:\n    items: list[int64]\nfunction observe(view value: secret[list[int64]]) returns int64:\n    return 7\nfunction exercise(value: Packet) returns int64:\n    return observe(view value.items)\n",
        "function produce() returns list[int64]:\n    return list(1)\nfunction observe(view value: secret[list[int64]]) returns int64:\n    return 7\nfunction exercise() returns int64:\n    return observe(produce())\n",
        "function produce() returns list[int64]:\n    return list(1)\nfunction observe(view value: secret[list[int64]]) returns int64:\n    return 7\nfunction exercise() returns int64:\n    return observe(view produce())\n",
        "function observe(view value: secret[list[int64]]) returns int64:\n    return 7\nfunction exercise(value: list[int64]) returns int64:\n    return value into observe()\n",
        "function observe(view value: secret[list[int64]]) returns int64:\n    return 7\nfunction exercise(value: list[int64]) returns int64:\n    return observe(view value)\n",
    ];
    for source in sources {
        let (mut program, checked) = checked_source(source);
        let index = exercise_index(&program);
        let expression = returned_call_mut(&mut program.functions[index]);
        let ExpressionKind::Call {
            args,
            ownership: CallOwnership::Source(packet),
            ..
        } = &mut expression.kind
        else {
            panic!("qualified source call");
        };
        let argument = &packet.arguments[0];
        assert!(matches!(
            checked.interner.resolve(argument.actual_type),
            Type::List(_)
        ));
        assert!(
            matches!(checked.interner.resolve(argument.parameter_type), Type::Secret(inner)
            if *inner == argument.actual_type)
        );
        assert_eq!(
            checked.source_type_map.get(&argument.source_span),
            Some(&argument.actual_type)
        );
        assert!(
            argument.source_witness().occurrence_type() == argument.actual_type
                || matches!(checked.interner.resolve(argument.source_witness().occurrence_type()), Type::Secret(inner)
                if *inner == argument.actual_type)
        );
        args[0].ty = TypeInterner::STRING;
        rejects(&program, &checked, "forged qualified occurrence");
    }
}

#[test]
fn caller_packets_keep_pipeline_input_separate_from_field_callee_occurrences() {
    let sources = [
        "function increment(value: int64) returns int64:\n    return value + 1\nstruct Holder:\n    callback: function(int64) returns int64\nfunction exercise() returns int64:\n    Holder holder = Holder(callback: increment)\n    return 4 into holder.callback\n",
        "function increment(value: int64) returns int64:\n    return value + 1\nstruct Holder:\n    callback: function(int64) returns int64\nfunction exercise() returns int64:\n    Holder holder = Holder(callback: increment)\n    return 4 into holder.callback into holder.callback\n",
        "function recover(value: int64) returns result[int64, string]:\n    return ok(value)\nfunction increment(value: int64) returns int64:\n    return value + 1\nstruct Holder:\n    callback: function(int64) returns int64\nfunction exercise() returns int64:\n    Holder holder = Holder(callback: increment)\n    return 4\n        into recover() handle error:\n            default 0\n        into holder.callback\n",
    ];
    for source in sources {
        let (mut program, checked) = checked_source(source);
        assert!(!checked.pipeline_step_input_types.is_empty());
        assert!(
            checked
                .pipeline_step_input_types
                .values()
                .all(|ty| *ty == TypeInterner::INT64)
        );
        assert!(
            checked.pipeline_step_input_types.keys().any(|span| {
                checked.type_map.get(span).is_some_and(|ty| {
                    matches!(checked.interner.resolve(*ty), Type::Function { .. })
                })
            }),
            "field target shares a step span with its virtual input"
        );
        let index = exercise_index(&program);
        let expression = returned_call_mut(&mut program.functions[index]);
        let ExpressionKind::IndirectCall {
            args,
            ownership: CallOwnership::Source(packet),
            ..
        } = &mut expression.kind
        else {
            panic!("field pipeline remains an indirect invocation");
        };
        assert_eq!(packet.arguments[0].actual_type, TypeInterner::INT64);
        assert_eq!(
            packet.arguments[0].source_witness().occurrence_type(),
            TypeInterner::INT64
        );
        args[0].ty = TypeInterner::STRING;
        rejects(&program, &checked, "forged pipeline input operand");
    }
}

#[test]
fn generated_native_suites_seal_target_result_and_zero_argument_order() {
    let source = "function ordinary() returns nothing:\n    return nothing\nverify accepted:\n    assert true\n";
    let (program, checked) = checked_source_with_suites(source, true);
    let suite = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.kind == DeclarationKind::Verify)
        .expect("checked verify");
    let ordinary = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.kind == DeclarationKind::Function)
        .expect("ordinary function");
    let ownership = CallOwnership::generated(
        GeneratedOperation::NativeSuite { function: suite.id },
        &[],
        &[],
        &[],
        TypeInterner::NOTHING,
        &[],
        &checked.interner,
    )
    .expect("compiler suite packet");
    let original = Expression {
        kind: ExpressionKind::Call {
            ownership,
            function: suite.id,
            args: vec![],
            evaluation_order: vec![],
        },
        ty: TypeInterner::NOTHING,
        span: suite.span,
    };
    validate_hir_invocation(
        &program.functions,
        &[],
        &original,
        &checked.interner,
        jett_typecheck::CheckedOwnershipContext::Ordinary,
    )
    .expect("exact checked suite");
    for case in ["target", "operation", "result"] {
        let mut changed = original.clone();
        let ExpressionKind::Call {
            ownership: CallOwnership::Generated(packet),
            function,
            ..
        } = &mut changed.kind
        else {
            unreachable!()
        };
        match case {
            "target" => *function = ordinary.id,
            "operation" => {
                packet.operation = GeneratedOperation::NativeSuite {
                    function: ordinary.id,
                }
            }
            "result" => changed.ty = TypeInterner::BOOL,
            _ => unreachable!(),
        }
        assert!(
            validate_hir_invocation(
                &program.functions,
                &[],
                &changed,
                &checked.interner,
                jett_typecheck::CheckedOwnershipContext::Ordinary
            )
            .is_err(),
            "{case}"
        );
    }
    let mut ordinary_call = original.clone();
    let ExpressionKind::Call {
        ownership,
        function,
        ..
    } = &mut ordinary_call.kind
    else {
        unreachable!()
    };
    *function = ordinary.id;
    *ownership = CallOwnership::generated(
        GeneratedOperation::NativeSuite {
            function: ordinary.id,
        },
        &[],
        &[],
        &[],
        TypeInterner::NOTHING,
        &[],
        &checked.interner,
    )
    .unwrap();
    assert!(
        validate_hir_invocation(
            &program.functions,
            &[],
            &ordinary_call,
            &checked.interner,
            jett_typecheck::CheckedOwnershipContext::Ordinary
        )
        .is_err()
    );
}

#[test]
fn generated_evaluated_values_validate_closed_intrinsic_shapes() {
    let mut types = TypeInterner::new();
    let span = Span::new(FileId::new(0), 0, 1);
    let set = types.intern(Type::Set(TypeInterner::INT64));
    let bytes_result = types.intern(Type::Result(TypeInterner::BYTES, TypeInterner::STRING));
    let make = |intrinsic,
                type_arguments,
                args: Vec<Expression>,
                result,
                access: Vec<jett_typecheck::CheckedCalleeAccess>| {
        let order = (0..args.len()).collect::<Vec<_>>();
        let parameters = args.iter().map(|value| value.ty).collect::<Vec<_>>();
        let ownership = CallOwnership::generated(
            GeneratedOperation::EvaluatedValue { intrinsic },
            &args,
            &parameters,
            &access,
            result,
            &order,
            &types,
        )
        .unwrap();
        Expression {
            kind: ExpressionKind::Intrinsic {
                ownership,
                intrinsic,
                type_arguments,
                reflection_arguments: vec![],
                refinement_predicates: vec![],
                field_validation: None,
                args,
                evaluation_order: order,
            },
            ty: result,
            span,
        }
    };
    let empty_set = make(
        IntrinsicId::SetNew,
        vec![TypeInterner::INT64],
        vec![],
        set,
        vec![],
    );
    let inserted = make(
        IntrinsicId::SetAdd,
        vec![TypeInterner::INT64],
        vec![
            empty_set.clone(),
            Expression {
                kind: ExpressionKind::Int(7),
                ty: TypeInterner::INT64,
                span,
            },
        ],
        set,
        vec![jett_typecheck::CheckedCalleeAccess::Owned; 2],
    );
    let empty_bytes = make(
        IntrinsicId::BytesNew,
        vec![],
        vec![],
        TypeInterner::BYTES,
        vec![],
    );
    let parsed_bytes = make(
        IntrinsicId::BytesFromHex,
        vec![],
        vec![Expression {
            kind: ExpressionKind::String("00".into()),
            ty: TypeInterner::STRING,
            span,
        }],
        bytes_result,
        vec![jett_typecheck::CheckedCalleeAccess::Owned],
    );
    for original in [empty_set, inserted, empty_bytes, parsed_bytes] {
        validate_hir_invocation(
            &[],
            &[],
            &original,
            &types,
            jett_typecheck::CheckedOwnershipContext::Ordinary,
        )
        .expect("closed evaluated value");
        let mut changed = original.clone();
        let ExpressionKind::Intrinsic { type_arguments, .. } = &mut changed.kind else {
            unreachable!()
        };
        type_arguments.push(TypeInterner::STRING);
        assert!(
            validate_hir_invocation(
                &[],
                &[],
                &changed,
                &types,
                jett_typecheck::CheckedOwnershipContext::Ordinary
            )
            .is_err()
        );
    }
}

#[test]
fn generated_stages_keep_private_acquisition_and_snapshot_authority() {
    let mut types = TypeInterner::new();
    let list = types.intern(Type::List(TypeInterner::INT64));
    let span = Span::new(FileId::new(0), 0, 1);
    let borrowed = Expression {
        kind: ExpressionKind::View(Box::new(Expression {
            kind: ExpressionKind::Local(LocalId::new(0)),
            ty: list,
            span,
        })),
        ty: list,
        span,
    };
    let mut packet = CallOwnership::generated(
        GeneratedOperation::Display {
            method: FunctionId::new(0),
        },
        &[borrowed],
        &[list],
        &[jett_typecheck::CheckedCalleeAccess::View],
        TypeInterner::STRING,
        &[0],
        &types,
    )
    .unwrap();
    assert!(
        packet
            .stage_generated_parameter(
                0,
                GeneratedArgumentStaging::OwnedProducer {
                    owner: LocalId::new(1),
                    loan: LocalId::new(2)
                }
            )
            .is_err()
    );
    packet
        .stage_generated_parameter(
            0,
            GeneratedArgumentStaging::OrdinarySnapshot {
                owner: LocalId::new(1),
                loan: LocalId::new(2),
            },
        )
        .expect("ordinary data snapshot");
    let local = |id: LocalId| {
        Some(OwnershipLocalInfo {
            ty: list,
            mutable: false,
            span,
            view_source: (id == LocalId::new(2)).then_some(LocalId::new(1)),
            is_view_parameter: false,
            view_iteration: None,
        })
    };
    validate_operand_ownership(&packet, &[list], &[0], &types, local, false)
        .expect("typed staged shape");
    assert!(validate_operand_ownership(&packet, &[list], &[0], &types, local, true).is_err());
    assert!(
        packet
            .stage_generated_parameter(
                0,
                GeneratedArgumentStaging::Existing {
                    loan: Some(LocalId::new(2))
                }
            )
            .is_err(),
        "a second transition cannot replace a sealed storage association"
    );
    let CallOwnership::Generated(generated) = &mut packet else {
        unreachable!()
    };
    generated.arguments[0].staging = GeneratedArgumentStaging::OwnedProducer {
        owner: LocalId::new(1),
        loan: LocalId::new(2),
    };
    assert!(
        validate_operand_ownership(&packet, &[list], &[0], &types, local, false).is_err(),
        "public stage cannot relabel a snapshot as a producer"
    );
    let mut appended = packet.clone();
    let CallOwnership::Generated(generated) = &mut appended else {
        unreachable!()
    };
    generated.arguments.push(generated.arguments[0].clone());
    assert!(
        appended
            .stage_generated_parameter(
                1,
                GeneratedArgumentStaging::Existing {
                    loan: Some(LocalId::new(2))
                }
            )
            .is_err(),
        "public extra argument is bounded before certificate lookup"
    );
    let signature = types.intern(Type::Function {
        params: vec![],
        view_params: vec![],
        return_type: TypeInterner::NOTHING,
    });
    let descriptor = Expression {
        kind: ExpressionKind::Local(LocalId::new(0)),
        ty: signature,
        span,
    };
    let mut descriptor_packet = CallOwnership::generated(
        GeneratedOperation::FunctionAdapter {
            callee: LocalId::new(0),
            source_type: signature,
            target_type: signature,
        },
        &[descriptor],
        &[signature],
        &[jett_typecheck::CheckedCalleeAccess::View],
        TypeInterner::NOTHING,
        &[0],
        &types,
    )
    .unwrap();
    assert!(
        descriptor_packet
            .stage_generated_parameter(
                0,
                GeneratedArgumentStaging::OrdinarySnapshot {
                    owner: LocalId::new(1),
                    loan: LocalId::new(2)
                }
            )
            .is_err(),
        "signature cannot prove captures"
    );
}

fn generated_metadata_constant(types: &TypeInterner, ty: TypeId, span: Span) -> Expression {
    let kind = match types.resolve(ty) {
        Type::Int64 => ExpressionKind::Int(0),
        Type::String => ExpressionKind::String(String::new()),
        Type::Bool => ExpressionKind::Bool(false),
        Type::Optional(_) => ExpressionKind::OptionalNone,
        Type::List(_) => ExpressionKind::ListConstruct { elements: vec![] },
        Type::Enum(_) => ExpressionKind::EnumConstruct {
            enum_type: ty,
            variant: VariantId::new(0),
            payloads: vec![],
            evaluation_order: vec![],
        },
        Type::Struct(id) => {
            let fields = types
                .resolve_struct(*id)
                .fields
                .iter()
                .map(|(_, ty)| generated_metadata_constant(types, *ty, span))
                .collect::<Vec<_>>();
            ExpressionKind::StructConstruct {
                struct_type: ty,
                evaluation_order: (0..fields.len()).collect(),
                fields,
                validates_refinements: false,
                refinement_predicates: vec![],
            }
        }
        other => panic!("unexpected compiler metadata field: {other:?}"),
    };
    Expression { kind, ty, span }
}

#[test]
fn generated_evaluated_builders_keep_canonical_metadata_and_payload_shapes() {
    let source = "struct Record:\n    value: int64\nenum Choice:\n    chosen(value: int64)\nmachine Session:\n    states:\n        ready(value: int64)\n        empty\n    transitions:\n        ready to empty\nfunction exercise() returns nothing:\n    return nothing\n";
    let (_, checked) = checked_source(source);
    let mut types = checked.interner;
    let named = |types: &TypeInterner, name: &str| {
        types
            .type_ids()
            .find(|&ty| types.type_name(ty) == name)
            .expect("checked named type")
    };
    let record = named(&types, "Record");
    let choice = named(&types, "Choice");
    let session = named(&types, "Session");
    let field = named(&types, "TypeField");
    let variant = named(&types, "TypeVariant");
    let state = named(&types, "TypeMachineState");
    let result = types.intern(Type::Result(
        TypeInterner::TYPE_CONSTRUCTION,
        TypeInterner::STRING,
    ));
    let span = Span::new(FileId::new(0), 0, 1);
    let make = |intrinsic, type_arguments, args: Vec<Expression>, result| {
        let parameters = args.iter().map(|value| value.ty).collect::<Vec<_>>();
        let order = (0..args.len()).collect::<Vec<_>>();
        let access = (0..args.len())
            .map(|index| {
                jett_typecheck::intrinsic_operand_access(intrinsic, index, args.len()).unwrap()
            })
            .collect::<Vec<_>>();
        let ownership = CallOwnership::generated(
            GeneratedOperation::EvaluatedValue { intrinsic },
            &args,
            &parameters,
            &access,
            result,
            &order,
            &types,
        )
        .unwrap();
        Expression {
            kind: ExpressionKind::Intrinsic {
                ownership,
                intrinsic,
                type_arguments,
                reflection_arguments: vec![],
                refinement_predicates: vec![],
                field_validation: None,
                args,
                evaluation_order: order,
            },
            ty: result,
            span,
        }
    };
    let started = make(
        IntrinsicId::TypeConstructStart,
        vec![record],
        vec![],
        TypeInterner::TYPE_CONSTRUCTION,
    );
    let variant_started = make(
        IntrinsicId::TypeConstructVariantStart,
        vec![choice],
        vec![generated_metadata_constant(&types, variant, span)],
        result,
    );
    let machine_started = make(
        IntrinsicId::TypeConstructMachineStart,
        vec![session],
        vec![generated_metadata_constant(&types, state, span)],
        result,
    );
    let put = make(
        IntrinsicId::TypeConstructPut,
        vec![record, TypeInterner::INT64],
        vec![
            started.clone(),
            generated_metadata_constant(&types, field, span),
            Expression {
                kind: ExpressionKind::Int(7),
                ty: TypeInterner::INT64,
                span,
            },
        ],
        result,
    );
    for original in [started, variant_started, machine_started, put] {
        validate_hir_invocation(
            &[],
            &[],
            &original,
            &types,
            jett_typecheck::CheckedOwnershipContext::Ordinary,
        )
        .expect("closed evaluated builder");
        let mut changed = original.clone();
        let ExpressionKind::Intrinsic { type_arguments, .. } = &mut changed.kind else {
            unreachable!()
        };
        type_arguments.clear();
        assert!(
            validate_hir_invocation(
                &[],
                &[],
                &changed,
                &types,
                jett_typecheck::CheckedOwnershipContext::Ordinary
            )
            .is_err()
        );
    }
    let malformed = make(
        IntrinsicId::TypeConstructVariantStart,
        vec![choice],
        vec![generated_metadata_constant(&types, field, span)],
        result,
    );
    assert!(
        validate_hir_invocation(
            &[],
            &[],
            &malformed,
            &types,
            jett_typecheck::CheckedOwnershipContext::Ordinary
        )
        .is_err(),
        "field metadata cannot stand in for selected variant metadata"
    );
    let malformed_owner = make(
        IntrinsicId::TypeConstructStart,
        vec![TypeInterner::INT64],
        vec![],
        TypeInterner::TYPE_CONSTRUCTION,
    );
    assert!(
        validate_hir_invocation(
            &[],
            &[],
            &malformed_owner,
            &types,
            jett_typecheck::CheckedOwnershipContext::Ordinary
        )
        .is_err(),
        "a primitive cannot be substituted for the checked construction owner"
    );
}

#[test]
fn generated_zero_argument_adapters_bind_and_remap_the_actual_capture() {
    let mut types = TypeInterner::new();
    let signature = types.intern(Type::Function {
        params: vec![],
        view_params: vec![],
        return_type: TypeInterner::INT64,
    });
    let span = Span::new(FileId::new(0), 0, 1);
    let mut ownership = CallOwnership::generated(
        GeneratedOperation::FunctionAdapter {
            callee: LocalId::new(1),
            source_type: signature,
            target_type: signature,
        },
        &[],
        &[],
        &[],
        TypeInterner::INT64,
        &[],
        &types,
    )
    .unwrap();
    let local = |id: LocalId| {
        Some(OwnershipLocalInfo {
            ty: if id == LocalId::new(1) {
                signature
            } else {
                TypeInterner::INT64
            },
            mutable: false,
            span,
            view_source: None,
            is_view_parameter: false,
            view_iteration: None,
        })
    };
    validate_operand_ownership(&ownership, &[], &[], &types, local, true)
        .expect("capture type joined even without arguments");
    assert!(validate_operand_ownership(&ownership, &[], &[], &types, |_| None, true).is_err());
    assert!(
        validate_operand_ownership(
            &ownership,
            &[],
            &[],
            &types,
            |id| {
                let mut fact = local(id).unwrap();
                fact.ty = TypeInterner::INT64;
                Some(fact)
            },
            true
        )
        .is_err()
    );
    let mut original = Expression {
        kind: ExpressionKind::IndirectCall {
            ownership: ownership.clone(),
            callee: Box::new(Expression {
                kind: ExpressionKind::Local(LocalId::new(1)),
                ty: signature,
                span,
            }),
            args: vec![],
            evaluation_order: vec![],
        },
        ty: TypeInterner::INT64,
        span,
    };
    validate_invocation_target(
        &[],
        &original,
        &types,
        jett_typecheck::CheckedOwnershipContext::Ordinary,
    )
    .expect("exact captured target");
    let ExpressionKind::IndirectCall { callee, .. } = &mut original.kind else {
        unreachable!()
    };
    callee.kind = ExpressionKind::Local(LocalId::new(0));
    assert!(
        validate_invocation_target(
            &[],
            &original,
            &types,
            jett_typecheck::CheckedOwnershipContext::Ordinary
        )
        .is_err()
    );
    let mut ids = vec![];
    ownership.metadata_local_ids(|id| ids.push(id));
    assert!(ids.contains(&LocalId::new(1)));
    ownership
        .remap_metadata_locals(|id| Ok::<_, String>(LocalId::new(id.index() + 2)))
        .unwrap();
    validate_operand_ownership(
        &ownership,
        &[],
        &[],
        &types,
        |id| {
            (id == LocalId::new(3)).then_some(OwnershipLocalInfo {
                ty: signature,
                mutable: false,
                span,
                view_source: None,
                is_view_parameter: false,
                view_iteration: None,
            })
        },
        true,
    )
    .expect("public and private target identities remap together");
    let remapped = Expression {
        kind: ExpressionKind::IndirectCall {
            ownership,
            callee: Box::new(Expression {
                kind: ExpressionKind::Local(LocalId::new(3)),
                ty: signature,
                span,
            }),
            args: vec![],
            evaluation_order: vec![],
        },
        ty: TypeInterner::INT64,
        span,
    };
    validate_invocation_target(
        &[],
        &remapped,
        &types,
        jett_typecheck::CheckedOwnershipContext::Ordinary,
    )
    .unwrap();
}

#[test]
fn generated_metadata_return_comes_from_the_exact_intrinsic_schema() {
    let types = TypeInterner::new();
    let span = Span::new(FileId::new(0), 0, 1);
    let make = |intrinsic, type_arguments, result| {
        let ownership = CallOwnership::generated(
            GeneratedOperation::ReflectionMetadata { intrinsic },
            &[],
            &[],
            &[],
            result,
            &[],
            &types,
        )
        .unwrap();
        Expression {
            kind: ExpressionKind::Intrinsic {
                ownership,
                intrinsic,
                type_arguments,
                reflection_arguments: vec![],
                refinement_predicates: vec![],
                field_validation: None,
                args: vec![],
                evaluation_order: vec![],
            },
            ty: result,
            span,
        }
    };
    let original = make(
        IntrinsicId::TypeName,
        vec![TypeInterner::INT64],
        TypeInterner::STRING,
    );
    validate_hir_invocation(
        &[],
        &[],
        &original,
        &types,
        jett_typecheck::CheckedOwnershipContext::Ordinary,
    )
    .expect("exact generated TypeName");
    for changed in [
        make(
            IntrinsicId::TypeName,
            vec![TypeInterner::INT64],
            TypeInterner::BOOL,
        ),
        make(IntrinsicId::TypeName, vec![], TypeInterner::STRING),
        make(
            IntrinsicId::TypeName,
            vec![TypeInterner::INT64, TypeInterner::STRING],
            TypeInterner::STRING,
        ),
        make(
            IntrinsicId::TypeHasSecret,
            vec![TypeInterner::INT64],
            TypeInterner::BOOL,
        ),
    ] {
        assert!(
            validate_hir_invocation(
                &[],
                &[],
                &changed,
                &types,
                jett_typecheck::CheckedOwnershipContext::Ordinary
            )
            .is_err(),
            "a freshly minted certificate cannot manufacture a metadata return or operation schema"
        );
    }
}

#[test]
fn generated_owned_operand_cannot_acquire_a_logical_viewed_iteration_binding() {
    let mut types = TypeInterner::new();
    let item = types.intern(Type::List(TypeInterner::INT64));
    let iterable = types.intern(Type::List(item));
    let span = Span::new(FileId::new(0), 0, 10);
    let value = Expression {
        kind: ExpressionKind::Local(LocalId::new(0)),
        ty: item,
        span,
    };
    let packet = CallOwnership::generated(
        GeneratedOperation::NativeSuite {
            function: FunctionId::new(0),
        },
        &[value.clone()],
        &[item],
        &[jett_typecheck::CheckedCalleeAccess::Owned],
        TypeInterner::NOTHING,
        &[0],
        &types,
    )
    .unwrap();
    let CallOwnership::Generated(generated) = &packet else {
        unreachable!()
    };
    assert_eq!(
        generated.arguments[0].acquisition,
        GeneratedAcquisition::OwnedExpression,
        "the canonical owned formal must exercise the generated-owner gate"
    );
    let proof = crate::checked_view_iteration_binding(
        &types,
        span,
        iterable,
        item,
        crate::IterationPart::Element,
    )
    .unwrap();
    let local = OwnershipLocalInfo {
        ty: item,
        mutable: false,
        span,
        view_source: None,
        is_view_parameter: false,
        view_iteration: Some(proof),
    };
    let error = validate_generated_operand_tree(&value, &generated.arguments[0], &[local])
        .expect_err("generated acquisition cannot relabel logical source view");
    assert_eq!(error, "generated owner cannot acquire a borrowed binding");
}

const GENERATED_PRODUCER_SOURCE: &str = r#"interface Displayable:
    function display(view self: Displayable) returns string
namespace app
struct Item:
    value: int64
implement Displayable for Item:
    function display(view self: Item) returns string:
        return "shown"
function make() returns Item:
    return Item(value: 7)
function alternate() returns Item:
    return Item(value: 9)
"#;

fn displayed_call_mut(function: &mut Function) -> &mut Expression {
    let ExpressionKind::StringInterpolation(parts) = &mut returned_call_mut(function).kind else {
        panic!("source interpolation");
    };
    let call = parts
        .iter_mut()
        .find_map(|part| match part {
            StringSegment::Value(Expression {
                kind: ExpressionKind::DisplayResult(call),
                ..
            }) => Some(call.as_mut()),
            _ => None,
        })
        .expect("exact generated display invocation");
    assert!(matches!(
        call.kind,
        ExpressionKind::Call {
            ownership: CallOwnership::Generated(_),
            ..
        }
    ));
    call
}

fn generated_original_argument(
    call: &mut Expression,
) -> (&mut Expression, &GeneratedArgumentOwnership) {
    let ExpressionKind::Call {
        args,
        ownership: CallOwnership::Generated(packet),
        ..
    } = &mut call.kind
    else {
        panic!("generated invocation");
    };
    (&mut args[0], &packet.arguments[0])
}

fn generated_endpoint_mut(value: &mut Expression) -> &mut Expression {
    match value {
        Expression {
            kind: ExpressionKind::View(inner),
            ..
        } => inner,
        other => other,
    }
}

#[test]
fn generated_producer_shapes_keep_original_clone_call_and_constructor_distinct() {
    for endpoint in ["make()", "Item(value: 7)", "clone make()"] {
        let source = format!(
            "{GENERATED_PRODUCER_SOURCE}function exercise() returns string:\n    return \"{{{endpoint}}}\"\n"
        );
        let (program, checked) = checked_source(&source);
        let index = exercise_index(&program);
        let mut changed = program.clone();
        let (value, argument) =
            generated_original_argument(displayed_call_mut(&mut changed.functions[index]));
        assert!(argument.original_witness().owned_producer());
        let endpoint = generated_endpoint_mut(value);
        let original = endpoint.clone();
        endpoint.kind = ExpressionKind::Clone(Box::new(original));
        rejects(&changed, &checked, "added generated endpoint clone");
    }
    let source = format!(
        "{GENERATED_PRODUCER_SOURCE}function exercise() returns string:\n    return \"{{make()}}\"\n"
    );
    let (mut program, checked) = checked_source(&source);
    let alternate = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "alternate")
        .unwrap()
        .id;
    let index = exercise_index(&program);
    let (value, _) = generated_original_argument(displayed_call_mut(&mut program.functions[index]));
    let ExpressionKind::Call { function, .. } = &mut generated_endpoint_mut(value).kind else {
        panic!("original producer call");
    };
    *function = alternate;
    rejects(
        &program,
        &checked,
        "same-result generated target substitution",
    );
}

#[test]
fn generated_producer_backing_follows_local_and_function_metadata_remaps() {
    let mut types = TypeInterner::new();
    let list = types.intern(Type::List(TypeInterner::INT64));
    let signature = types.intern(Type::Function {
        params: vec![],
        view_params: vec![],
        return_type: list,
    });
    let span = Span::new(FileId::new(0), 0, 1);
    let source = Expression {
        kind: ExpressionKind::IndirectCall {
            callee: Box::new(Expression {
                kind: ExpressionKind::Local(LocalId::new(1)),
                ty: signature,
                span,
            }),
            args: vec![],
            evaluation_order: vec![],
            ownership: CallOwnership::generated(
                GeneratedOperation::FunctionAdapter {
                    callee: LocalId::new(1),
                    source_type: signature,
                    target_type: signature,
                },
                &[],
                &[],
                &[],
                list,
                &[],
                &types,
            )
            .unwrap(),
        },
        ty: list,
        span,
    };
    let mut packet = CallOwnership::generated(
        GeneratedOperation::Display {
            method: FunctionId::new(2),
        },
        &[source.clone()],
        &[list],
        &[jett_typecheck::CheckedCalleeAccess::View],
        TypeInterner::STRING,
        &[0],
        &types,
    )
    .unwrap();
    let CallOwnership::Generated(generated) = &packet else {
        unreachable!()
    };
    generated.arguments[0]
        .original_witness()
        .validate_producer_shape(&source)
        .unwrap();
    let mut substituted = source.clone();
    let ExpressionKind::IndirectCall { callee, .. } = &mut substituted.kind else {
        unreachable!()
    };
    callee.kind = ExpressionKind::Local(LocalId::new(0));
    assert!(
        generated.arguments[0]
            .original_witness()
            .validate_producer_shape(&substituted)
            .is_err()
    );
    let mut locals = vec![];
    packet.metadata_local_ids(|id| locals.push(id));
    assert!(locals.contains(&LocalId::new(1)));
    packet
        .remap_metadata_locals(|id| Ok::<_, String>(LocalId::new(id.index() + 3)))
        .unwrap();
    packet
        .remap_metadata_functions(|id| Ok::<_, String>(FunctionId::new(id.index() + 5)))
        .unwrap();
    let mut functions = vec![];
    packet.metadata_function_ids(|id| functions.push(id));
    assert!(functions.contains(&FunctionId::new(7)));
    let mut remapped = source;
    let ExpressionKind::IndirectCall { callee, .. } = &mut remapped.kind else {
        unreachable!()
    };
    callee.kind = ExpressionKind::Local(LocalId::new(4));
    let CallOwnership::Generated(generated) = &packet else {
        unreachable!()
    };
    generated.arguments[0]
        .original_witness()
        .validate_producer_shape(&remapped)
        .unwrap();
    validate_operand_ownership(&packet, &[list], &[0], &types, |_| None, true).unwrap();
}

#[test]
fn generated_constructor_shapes_keep_authenticated_inline_descriptors() {
    let source = r#"interface Displayable:
    function display(view self: Displayable) returns string
namespace app
struct Item:
    callback: function(int64) returns int64
implement Displayable for Item:
    function display(view self: Item) returns string:
        return "shown"
function exercise(offset: int64) returns string:
    return "{Item(callback: function(value: int64) returns int64: return value + offset)}"
"#;
    let (mut program, checked) = checked_source(source);
    let index = exercise_index(&program);
    let (value, _) = generated_original_argument(displayed_call_mut(&mut program.functions[index]));
    let ExpressionKind::StructConstruct { fields, .. } = &mut generated_endpoint_mut(value).kind
    else {
        panic!("owned constructor");
    };
    let ExpressionKind::ClosureRef { captures, .. } = &mut fields[0].kind else {
        panic!("canonical captured inline descriptor");
    };
    assert_eq!(captures.len(), 1);
    captures.clear();
    rejects(&program, &checked, "descriptor backing substitution");
}

#[test]
fn generated_handle_shape_restoration_keeps_source_snapshot_and_cfg_authority_separate() {
    let source = format!(
        "{GENERATED_PRODUCER_SOURCE}function exercise(incoming: optional[int64]) returns string:\n    return \"{{Item(value: incoming handle: default 7)}}\"\n"
    );
    let (mut program, _checked) = checked_source(&source);
    let index = exercise_index(&program);
    let (value, argument) =
        generated_original_argument(displayed_call_mut(&mut program.functions[index]));
    let original = value.clone();
    let ExpressionKind::StructConstruct { fields, .. } = &mut generated_endpoint_mut(value).kind
    else {
        panic!("owned constructor");
    };
    let ExpressionKind::Handle { target, .. } = &fields[0].kind else {
        panic!("original handled field");
    };
    let original_target = target.as_ref().clone();
    let handle_span = fields[0].span;
    fields[0].kind = ExpressionKind::Local(LocalId::new(100));
    assert!(
        argument
            .original_witness()
            .validate_producer_shape(value)
            .is_err(),
        "original HIR cannot carry lowered result authority"
    );
    let mut reached = 0;
    argument
        .original_witness()
        .validate_lowered_producer_shape(value, |produced, original| {
            let handle = original.handle().expect("exact original Handle node");
            assert_eq!(handle.kind(), &HandleKind::Optional);
            assert_eq!(produced.ty, handle.result_type());
            assert_eq!(produced.span, handle_span);
            handle.validate_source_shape(&original_target)?;
            let snapshot = Expression {
                kind: ExpressionKind::Clone(Box::new(original_target.clone())),
                ty: original_target.ty,
                span: original_target.span,
            };
            handle.validate_source_shape(&snapshot)?;
            let double = Expression {
                kind: ExpressionKind::Clone(Box::new(snapshot)),
                ty: original_target.ty,
                span: original_target.span,
            };
            assert!(handle.validate_source_shape(&double).is_err());
            reached += 1;
            Ok(())
        })
        .expect("callback only authenticates this original Handle node");
    assert_eq!(reached, 1);
    let mut foreign = original.clone();
    let ExpressionKind::StructConstruct { fields, .. } =
        &mut generated_endpoint_mut(&mut foreign).kind
    else {
        unreachable!()
    };
    fields[0].kind = ExpressionKind::Int(7);
    assert!(
        argument
            .original_witness()
            .validate_lowered_producer_shape(&foreign, |_, _| {
                panic!("a non-Local replacement never receives Handle authority")
            })
            .is_err()
    );
}

#[test]
fn generated_storage_callbacks_keep_exact_non_handle_nodes_and_copy_permissions() {
    let mut types = TypeInterner::new();
    let span = Span::new(FileId::new(0), 0, 1);
    let literal = Expression {
        kind: ExpressionKind::Int(7),
        ty: TypeInterner::INT64,
        span,
    };
    let packet = CallOwnership::generated(
        GeneratedOperation::Display {
            method: FunctionId::new(0),
        },
        &[literal.clone()],
        &[literal.ty],
        &[jett_typecheck::CheckedCalleeAccess::View],
        TypeInterner::STRING,
        &[0],
        &types,
    )
    .unwrap();
    let CallOwnership::Generated(packet) = packet else {
        unreachable!()
    };
    let witness = packet.arguments[0].original_witness();
    let storage = Expression {
        kind: ExpressionKind::Local(LocalId::new(100)),
        ..literal.clone()
    };
    assert!(witness.validate_producer_shape(&storage).is_err());
    witness
        .validate_lowered_producer_shape(&storage, |value, original| {
            assert_eq!((value.ty, value.span), (original.ty(), original.span()));
            assert!(original.handle().is_none());
            original
                .validate_reconstructed(&literal, |_, _| Err("unexpected nested storage".into()))?;
            let inserted_clone = Expression {
                kind: ExpressionKind::Clone(Box::new(literal.clone())),
                ..literal.clone()
            };
            assert!(
                original
                    .validate_reconstructed(&inserted_clone, |_, _| Err(
                        "unexpected nested storage".into()
                    ))
                    .is_err()
            );
            assert!(
                original
                    .validate_existing_local_snapshot(&inserted_clone, |_, _| Err(
                        "unexpected nested storage".into()
                    ))
                    .is_err()
            );
            Ok(())
        })
        .expect("read-only callback recurses against the same original primitive node");

    let list = types.intern(Type::List(TypeInterner::INT64));
    let descriptor = types.intern(Type::Function {
        params: vec![],
        view_params: vec![],
        return_type: TypeInterner::INT64,
    });
    for (ty, ordinary_copy) in [(list, true), (descriptor, false)] {
        let original = Expression {
            kind: ExpressionKind::Local(LocalId::new(1)),
            ty,
            span,
        };
        let packet = CallOwnership::generated(
            GeneratedOperation::Display {
                method: FunctionId::new(0),
            },
            &[original.clone()],
            &[ty],
            &[jett_typecheck::CheckedCalleeAccess::View],
            TypeInterner::STRING,
            &[0],
            &types,
        )
        .unwrap();
        let CallOwnership::Generated(packet) = packet else {
            unreachable!()
        };
        let witness = packet.arguments[0].original_witness();
        let storage = Expression {
            kind: ExpressionKind::Local(LocalId::new(100)),
            ty,
            span,
        };
        let snapshot = Expression {
            kind: ExpressionKind::Clone(Box::new(original.clone())),
            ty,
            span,
        };
        witness
            .validate_lowered_producer_shape(&storage, |_, node| {
                assert!(
                    node.validate_reconstructed(&snapshot, |_, _| Err(
                        "unexpected nested storage".into()
                    ))
                    .is_err()
                );
                assert_eq!(
                    node.validate_existing_local_snapshot(&snapshot, |_, _| Err(
                        "unexpected nested storage".into()
                    ))
                    .is_ok(),
                    ordinary_copy
                );
                let double = Expression {
                    kind: ExpressionKind::Clone(Box::new(snapshot.clone())),
                    ty,
                    span,
                };
                assert!(
                    node.validate_existing_local_snapshot(&double, |_, _| Err(
                        "unexpected nested storage".into()
                    ))
                    .is_err()
                );
                let wrong_backing = Expression {
                    kind: ExpressionKind::Clone(Box::new(storage.clone())),
                    ty,
                    span,
                };
                assert!(
                    node.validate_existing_local_snapshot(&wrong_backing, |_, _| Err(
                        "foreign storage cannot replace original backing".into()
                    ))
                    .is_err()
                );
                Ok(())
            })
            .expect("storage restoration does not infer descriptor copy authority");
    }
}

#[test]
fn caller_packets_preserve_exact_nominal_secret_argument_joins() {
    for callee in ["callback", "factory()"] {
        for (parameter, argument, result) in [
            ("int64", "Classified", "secret[int64]"),
            ("int64", "DeepClassified", "secret[int64]"),
            ("Classified", "secret[Classified]", "Classified"),
            ("DeepClassified", "secret[DeepClassified]", "DeepClassified"),
        ] {
            let source = format!(
                "type Classified = secret[int64] where true\n\
                 type DeepClassified = Classified where true\n\
                 type Alternative = secret[int64] where true\n\
                 type PublicValue = int64 where true\n\
                 function callback(value: {parameter}) returns {parameter}:\n    return value\n\
                 function factory() returns function({parameter}) returns {parameter}:\n    return callback\n\
                 function exercise(value: {argument}, unrelated: Alternative, public_value: PublicValue) returns {result}:\n    return {callee}(value)\n"
            );
            let (program, checked) = checked_source(&source);
            let exercise = &program.functions[exercise_index(&program)];
            let expression = returned_call(exercise);
            let (args, source) = match &expression.kind {
                ExpressionKind::Call {
                    args,
                    ownership: CallOwnership::Source(source),
                    ..
                }
                | ExpressionKind::IndirectCall {
                    args,
                    ownership: CallOwnership::Source(source),
                    ..
                } => (args, source),
                _ => panic!("checked source call"),
            };
            let argument = &source.arguments[0];
            assert_eq!(
                argument.actual_type,
                argument.source_witness().occurrence_type()
            );
            let CallerOrigin::Binding(binding) = &argument.origin else {
                panic!("exact nominal source binding");
            };
            assert_eq!(binding.ty, argument.actual_type);
            match checked.interner.resolve(argument.actual_type) {
                Type::Refinement { .. } => assert_eq!(args[0].ty, argument.actual_type),
                Type::Secret(inner) => assert_eq!(*inner, argument.parameter_type),
                _ => panic!("nominal-secret or original explicit-secret control"),
            }
            assert_eq!(source.arguments.len(), 1);
        }
    }
}

#[test]
fn caller_packets_reject_replaced_nominal_secret_operands_and_untainted_results() {
    for callee in ["callback", "factory()"] {
        let source = format!(
            "type Classified = secret[int64] where true\n\
             type Alternative = secret[int64] where true\n\
             type PublicValue = int64 where true\n\
             function callback(value: int64) returns int64:\n    return value\n\
             function factory() returns function(int64) returns int64:\n    return callback\n\
             function exercise(value: Classified, unrelated: Alternative, public_value: PublicValue) returns secret[int64]:\n    return {callee}(value)\n"
        );
        let (program, checked) = checked_source(&source);
        let index = exercise_index(&program);
        for name in ["unrelated", "public_value"] {
            let mut corrupted = program.clone();
            let replacement = corrupted.functions[index]
                .locals
                .iter()
                .find(|local| local.name == name)
                .expect("independent nominal control")
                .ty;
            let expression = returned_call_mut(&mut corrupted.functions[index]);
            let args = match &mut expression.kind {
                ExpressionKind::Call { args, .. } | ExpressionKind::IndirectCall { args, .. } => {
                    args
                }
                _ => panic!("checked source call"),
            };
            assert_ne!(replacement, args[0].ty);
            args[0].ty = replacement;
            let expression = returned_call(&corrupted.functions[index]);
            let error = validate_invocation_target(
                &corrupted.functions,
                expression,
                &checked.interner,
                jett_typecheck::CheckedOwnershipContext::Ordinary,
            )
            .expect_err("another nominal type cannot replace the immutable actual");
            assert!(
                error.contains("converted operand differs from its physical parameter"),
                "{callee}/{name}: {error}"
            );
            rejects(&corrupted, &checked, "replaced nominal Secret operand");
        }
        let mut corrupted = program.clone();
        returned_call_mut(&mut corrupted.functions[index]).ty = TypeInterner::INT64;
        let error = validate_invocation_target(
            &corrupted.functions,
            returned_call(&corrupted.functions[index]),
            &checked.interner,
            jett_typecheck::CheckedOwnershipContext::Ordinary,
        )
        .expect_err("Secret lifting retains its checked result");
        assert_eq!(
            error,
            "call ownership result differs from its checked source certificate"
        );
        rejects(&corrupted, &checked, "untainted nominal Secret result");
    }
}

#[test]
fn caller_packets_accept_exact_qualified_descriptors_but_not_value_substitutions() {
    let source = r#"namespace app
struct Holder:
    callback: function(int64) returns int64
function identity(value: int64) returns int64:
    return value
function inspect(view callback: function(int64) returns int64, marker: int64) returns int64:
    return marker
function exercise(holder: Holder, stored: function(int64) returns int64) returns int64:
    return inspect(marker: 1, callback: app.identity)
"#;
    for replacement in ["local", "field"] {
        let (mut program, checked) = checked_source(source);
        let index = exercise_index(&program);
        let holder = local_id(&program.functions[index], "holder");
        let stored = local_id(&program.functions[index], "stored");
        let holder_type = program.functions[index].locals[holder.index() as usize].ty;
        let expression = returned_call_mut(&mut program.functions[index]);
        let ExpressionKind::Call {
            args,
            ownership: CallOwnership::Source(packet),
            evaluation_order,
            ..
        } = &mut expression.kind
        else {
            panic!("qualified descriptor argument");
        };
        assert_eq!(evaluation_order, &[1, 0]);
        assert_eq!(packet.arguments[0].origin, CallerOrigin::OwnedExpression);
        assert!(matches!(args[0].kind, ExpressionKind::FunctionRef(_)));
        args[0].kind = if replacement == "local" {
            ExpressionKind::Local(stored)
        } else {
            ExpressionKind::Field {
                base: Box::new(Expression {
                    kind: ExpressionKind::Local(holder),
                    ty: holder_type,
                    span: args[0].span,
                }),
                owner_type: holder_type,
                field: FieldId::new(0),
            }
        };
        let errors = validate_program_call_ownership(&program, &checked.interner)
            .expect_err("a descriptor producer cannot be replaced by a value origin");
        assert!(errors.iter().any(|error| error.message ==
            "call ownership cannot manufacture a producer acquisition from a binding or field"),
            "{replacement}: {errors:?}");
    }
}

#[test]
fn caller_packets_keep_method_producers_and_real_callback_field_projections_distinct() {
    let source = r#"namespace models
export struct Point:
    value: int64
    function amount(view self: Point) returns int64:
        return self.value
namespace app
struct Holder:
    callback: function(view models.Point) returns int64
function inspect(view callback: function(view models.Point) returns int64) returns int64:
    return 7
function method_call() returns int64:
    use models as m
    return inspect(view m.Point.amount)
function exercise(view holder: Holder) returns int64:
    return inspect(view holder.callback)
"#;
    let (program, checked) = checked_source(source);
    let method = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "method_call")
        .expect("method source caller");
    let ExpressionKind::Call {
        args,
        ownership: CallOwnership::Source(packet),
        ..
    } = &returned_call(method).kind
    else {
        panic!("method descriptor call");
    };
    assert_eq!(packet.arguments[0].origin, CallerOrigin::OwnedExpression);
    let ExpressionKind::View(inner) = &args[0].kind else {
        panic!("written descriptor view");
    };
    assert!(matches!(inner.kind, ExpressionKind::FunctionRef(_)));
    let field = &program.functions[exercise_index(&program)];
    let ExpressionKind::Call {
        args,
        ownership: CallOwnership::Source(packet),
        ..
    } = &returned_call(field).kind
    else {
        panic!("borrowed value field call");
    };
    assert!(
        matches!(packet.arguments[0].origin, CallerOrigin::BorrowedProjection {
        source: CallerViewSource::Local(local)
    } if local == local_id(field, "holder"))
    );
    let ExpressionKind::View(inner) = &args[0].kind else {
        panic!("written field view");
    };
    assert!(matches!(inner.kind, ExpressionKind::Field { .. }));
    validate_program_call_ownership(&program, &checked.interner)
        .expect("both exact origins validate");
}

#[test]
fn caller_packets_generic_mutual_source_calls_have_exact_syntax_type_and_target_proofs() {
    let source = r#"namespace sample
mutual:
    function leaf[T](view value: T) returns int64
    function alternate[T](value: T) returns int64
    function forward[T](view value: T) returns int64
function leaf[T](view value: T) returns int64:
    return 7
function alternate[T](value: T) returns int64:
    return 9
function forward[T](view value: T) returns int64:
    return leaf[T](view value)
function exercise(values: list[int64]) returns int64:
    return forward[list[int64]](view values)
function instantiate_alternate(values: list[int64]) returns int64:
    return alternate[list[int64]](values)
"#;
    let (program, checked) = checked_source(source);
    let forward = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "forward")
        .expect("concrete mutual forwarding");
    let ExpressionKind::Call {
        ownership: CallOwnership::Source(packet),
        ..
    } = &returned_call(forward).kind
    else {
        panic!("source forward call");
    };
    assert_eq!(
        packet.arguments[0].syntax,
        jett_typecheck::CheckedCallerSyntax::WrittenView
    );
    assert_eq!(
        packet.arguments[0].effect,
        jett_typecheck::CheckedCallerEffect::RetainBorrow
    );
    assert_eq!(packet.arguments[0].callee_access, CheckedCalleeAccess::View);
    validate_program_call_ownership(&program, &checked.interner).expect("every exact packet");
    for mutation in ["syntax", "type", "target"] {
        let mut corrupted = program.clone();
        let alternate = corrupted
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "alternate")
            .expect("owned alternative")
            .id;
        let index = corrupted
            .functions
            .iter()
            .position(|function| function.identity.declaration.name == "forward")
            .unwrap();
        let expression = returned_call_mut(&mut corrupted.functions[index]);
        let ExpressionKind::Call {
            function,
            ownership: CallOwnership::Source(packet),
            ..
        } = &mut expression.kind
        else {
            panic!("real mutual packet");
        };
        match mutation {
            "syntax" => packet.arguments[0].syntax = jett_typecheck::CheckedCallerSyntax::Bare,
            "type" => packet.arguments[0].actual_type = TypeInterner::INT64,
            "target" => *function = alternate,
            _ => unreachable!(),
        }
        let errors = validate_program_call_ownership(&corrupted, &checked.interner)
            .expect_err("exact original mutual proof cannot be changed");
        assert!(!errors.is_empty(), "{mutation}");
    }
}

const MACHINE_STATE_SOURCE_JOINS: &str = r#"machine Session:
    states:
        guest
        logged_in(user_id: string)
    transitions:
        guest to logged_in
machine OtherSession:
    states:
        guest
        logged_in(user_id: string)
    transitions:
        guest to logged_in
function precise(session: Session at logged_in) returns string:
    return session.user_id
function guest_only(session: Session at guest) returns string:
    return "guest"
function other_only(view session: OtherSession at logged_in) returns int64:
    return 2
function wide(view session: Session) returns int64:
    return 7
function owned_wide(session: Session) returns int64:
    return 9
function guarded(session: Session) returns string:
    if session at logged_in:
        return precise(session)
    return "guest"
function exercise_direct(view session: Session at logged_in) returns int64:
    return wide(view session)
function exercise_indirect(view session: Session at logged_in, callback: function(view Session) returns int64) returns int64:
    return callback(view session)
"#;

fn machine_state_caller_index(program: &Program, name: &str) -> usize {
    program
        .functions
        .iter()
        .position(|function| function.identity.declaration.name == name)
        .expect("exact source function")
}

fn guarded_machine_call(function: &Function) -> &Expression {
    let StatementKind::If { then_block, .. } = &function.body.statements[0].kind else {
        panic!("checked source state guard");
    };
    let StatementKind::Return(Some(expression)) = &then_block.statements[0].kind else {
        panic!("call inside exact state guard");
    };
    expression
}

#[test]
fn caller_packets_machine_guard_keeps_exact_occurrence_and_declared_binding() {
    let (program, checked) = checked_source(MACHINE_STATE_SOURCE_JOINS);
    let caller = &program.functions[machine_state_caller_index(&program, "guarded")];
    let ExpressionKind::Call {
        args,
        ownership: CallOwnership::Source(packet),
        ..
    } = &guarded_machine_call(caller).kind
    else {
        panic!("checked narrowed call");
    };
    let argument = &packet.arguments[0];
    let CallerOrigin::Binding(fact) = &argument.origin else {
        panic!("original binding");
    };
    let Type::Machine(parent) = checked.interner.resolve(fact.ty) else {
        panic!("declaration remains bare machine");
    };
    let Type::MachineState { machine, .. } = checked.interner.resolve(argument.actual_type) else {
        panic!("checked occurrence retains exact state");
    };
    assert_eq!(parent, machine);
    assert_ne!(fact.ty, argument.actual_type);
    assert_eq!(caller.locals[fact.local.index() as usize].ty, fact.ty);
    assert_eq!(args[0].ty, argument.actual_type);
    assert_eq!(
        argument.source_witness().occurrence_type(),
        argument.actual_type
    );
    assert!(matches!(args[0].kind, ExpressionKind::Local(local) if local == fact.local));
    assert_eq!(argument.syntax, jett_typecheck::CheckedCallerSyntax::Bare);
    assert_eq!(
        argument.effect,
        jett_typecheck::CheckedCallerEffect::TransferOwned
    );
    assert_eq!(argument.callee_access, CheckedCalleeAccess::Owned);
    validate_program_call_ownership(&program, &checked.interner).expect("exact state proof");
}

#[test]
fn caller_packets_machine_state_views_erase_only_to_exact_parent_parameters() {
    let (program, checked) = checked_source(MACHINE_STATE_SOURCE_JOINS);
    for name in ["exercise_direct", "exercise_indirect"] {
        let caller = &program.functions[machine_state_caller_index(&program, name)];
        let (args, packet) = match &returned_call(caller).kind {
            ExpressionKind::Call {
                args,
                ownership: CallOwnership::Source(packet),
                ..
            }
            | ExpressionKind::IndirectCall {
                args,
                ownership: CallOwnership::Source(packet),
                ..
            } => (args, packet),
            _ => panic!("checked direct or indirect source call"),
        };
        let argument = &packet.arguments[0];
        let Type::MachineState { machine, .. } = checked.interner.resolve(argument.actual_type)
        else {
            panic!("physical argument remains exact state");
        };
        let Type::Machine(parent) = checked.interner.resolve(argument.parameter_type) else {
            panic!("parameter is bare parent machine");
        };
        assert_eq!(machine, parent);
        assert_ne!(argument.actual_type, argument.parameter_type);
        assert_eq!(args[0].ty, argument.actual_type);
        assert_eq!(
            argument.source_witness().occurrence_type(),
            argument.actual_type
        );
        assert!(matches!(args[0].kind, ExpressionKind::View(_)));
        assert_eq!(
            argument.syntax,
            jett_typecheck::CheckedCallerSyntax::WrittenView
        );
        assert_eq!(
            argument.effect,
            jett_typecheck::CheckedCallerEffect::RetainBorrow
        );
        assert_eq!(argument.callee_access, CheckedCalleeAccess::View);
    }
    validate_program_call_ownership(&program, &checked.interner)
        .expect("one-way exact parent joins");
}

#[test]
fn caller_packets_machine_state_joins_preserve_private_type_syntax_and_mode_proofs() {
    let (program, checked) = checked_source(MACHINE_STATE_SOURCE_JOINS);
    let direct = machine_state_caller_index(&program, "exercise_direct");
    let guest = program.functions[machine_state_caller_index(&program, "guest_only")].params[0].ty;
    let other = program.functions[machine_state_caller_index(&program, "other_only")].params[0].ty;
    let owned = program.functions[machine_state_caller_index(&program, "owned_wide")].id;
    for mutation in ["wrong state", "wrong machine", "syntax", "owned target"] {
        let mut corrupted = program.clone();
        let ExpressionKind::Call {
            function,
            args,
            ownership: CallOwnership::Source(packet),
            ..
        } = &mut returned_call_mut(&mut corrupted.functions[direct]).kind
        else {
            panic!("real source View packet");
        };
        match mutation {
            "wrong state" | "wrong machine" => {
                let replacement = if mutation == "wrong state" {
                    guest
                } else {
                    other
                };
                assert_ne!(args[0].ty, replacement);
                args[0].ty = replacement;
                let ExpressionKind::View(inner) = &mut args[0].kind else {
                    panic!("written View");
                };
                inner.ty = replacement;
            }
            "syntax" => packet.arguments[0].syntax = jett_typecheck::CheckedCallerSyntax::Bare,
            "owned target" => *function = owned,
            _ => unreachable!(),
        }
        let errors = validate_program_call_ownership(&corrupted, &checked.interner)
            .expect_err("private exact state/source/mode witness cannot be changed");
        assert!(!errors.is_empty(), "{mutation}");
    }
    let mut corrupted = program.clone();
    let guarded = machine_state_caller_index(&corrupted, "guarded");
    let StatementKind::If { then_block, .. } =
        &mut corrupted.functions[guarded].body.statements[0].kind
    else {
        panic!("original guard");
    };
    let StatementKind::Return(Some(expression)) = &mut then_block.statements[0].kind else {
        panic!("guarded call");
    };
    let packet = source_packet_mut(expression);
    let CallerOrigin::Binding(fact) = &mut packet.arguments[0].origin else {
        panic!("binding fact");
    };
    fact.ty = other;
    assert!(
        validate_program_call_ownership(&corrupted, &checked.interner).is_err(),
        "the declared binding fact stays exact"
    );
}

#[test]
fn caller_packets_machine_state_join_does_not_admit_wrong_nominal_or_state_sources() {
    for source in [
        MACHINE_STATE_SOURCE_JOINS.replace(
            "return wide(view session)",
            "return other_only(view session)",
        ),
        MACHINE_STATE_SOURCE_JOINS.replace("return precise(session)", "return guest_only(session)"),
    ] {
        let parsed = jett_parser::parse(&source, FileId::new(0));
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let resolved = jett_resolve::resolve(&parsed.module);
        assert!(
            resolved
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != Severity::Error),
            "{:?}",
            resolved.diagnostics
        );
        let checked = jett_typecheck::check(&parsed.module, &resolved);
        assert!(
            checked
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Error
                    && diagnostic.code.code() == 304),
            "wrong machine/state remains a checked argument error: {:?}",
            checked.diagnostics
        );
    }
}

const CONVERTED_SOURCE_HANDLE: &str = r#"namespace app
interface Show:
    function show(view self: Show) returns string
implement Show for uint64:
    function show(view self: uint64) returns string:
        return "uint64"
function read(view value: Show) returns int64:
    return 7
function exercise(incoming: optional[uint64], spare: uint64) returns int64:
    uint64 fallback = 0
    return read(view(incoming handle:
        default fallback
    ))
"#;

#[test]
fn caller_packets_converted_handle_keeps_inner_occurrence_separate_from_written_view() {
    let (program, checked) = checked_source(CONVERTED_SOURCE_HANDLE);
    let function = &program.functions[exercise_index(&program)];
    let ExpressionKind::Call {
        args,
        ownership: CallOwnership::Source(packet),
        ..
    } = &returned_call(function).kind
    else {
        panic!("source read");
    };
    let argument = &packet.arguments[0];
    let backing = call_ownership::validate_source_handled_operand(
        &args[0],
        argument,
        &packet.bridge,
        &checked.interner,
    )
    .expect("exact finite conversion")
    .expect("original Handle");
    assert!(matches!(backing.kind, ExpressionKind::Handle { .. }));
    assert_eq!(backing.ty, TypeInterner::UINT64);
    assert_eq!(argument.actual_type, TypeInterner::UINT64);
    assert!(matches!(
        checked.interner.resolve(argument.parameter_type),
        Type::Interface(_)
    ));
    assert_ne!(backing.span, argument.source_span);
    assert_eq!(
        argument.syntax,
        jett_typecheck::CheckedCallerSyntax::WrittenView
    );
    assert_eq!(argument.origin, CallerOrigin::OwnedExpression);
}

#[test]
fn caller_packets_converted_handle_rejects_lost_inner_occurrence_and_original_node() {
    let (program, checked) = checked_source(CONVERTED_SOURCE_HANDLE);
    for mutation in ["span", "type", "literal replacement"] {
        let mut corrupted = program.clone();
        let function = &mut corrupted.functions[exercise_index(&program)];
        let ExpressionKind::Call { args, .. } = &mut returned_call_mut(function).kind else {
            panic!("source read");
        };
        let mut backing = &mut args[0];
        while matches!(
            backing.kind,
            ExpressionKind::View(_)
                | ExpressionKind::InterfaceCoerce { .. }
                | ExpressionKind::FunctionAdapter { .. }
        ) {
            backing = match &mut backing.kind {
                ExpressionKind::View(inner)
                | ExpressionKind::InterfaceCoerce { value: inner, .. }
                | ExpressionKind::FunctionAdapter { value: inner, .. } => inner,
                _ => unreachable!("transparent conversion checked before borrowing"),
            };
        }
        assert!(matches!(backing.kind, ExpressionKind::Handle { .. }));
        match mutation {
            "span" => backing.span.start += 1,
            "type" => backing.ty = TypeInterner::INT64,
            "literal replacement" => backing.kind = ExpressionKind::Int(0),
            _ => unreachable!(),
        }
        rejects(&corrupted, &checked, mutation);
    }
}

const QUALIFIED_UNIT_ENUM_SOURCE: &str = r#"namespace models
export enum Choice:
    empty
    full(value: int64)
namespace app
struct Holder:
    choice: models.Choice
function inspect(view choice: models.Choice, marker: int64) returns int64:
    return marker
function exercise(holder: Holder, stored: models.Choice) returns int64:
    use models as m
    return inspect(marker: 1, choice: m.Choice.empty)
"#;

#[test]
fn caller_packets_qualified_unit_enums_are_producers_not_same_typed_value_origins() {
    let (program, checked) = checked_source(QUALIFIED_UNIT_ENUM_SOURCE);
    let index = exercise_index(&program);
    let ExpressionKind::Call {
        args,
        ownership: CallOwnership::Source(packet),
        evaluation_order,
        ..
    } = &returned_call(&program.functions[index]).kind
    else {
        panic!("qualified enum Source invocation");
    };
    assert_eq!(evaluation_order, &[1, 0]);
    assert_eq!(packet.arguments[0].origin, CallerOrigin::OwnedExpression);
    assert_eq!(
        packet.arguments[0].effect,
        jett_typecheck::CheckedCallerEffect::RelinquishOwned
    );
    let ExpressionKind::EnumConstruct {
        enum_type,
        variant,
        payloads,
        evaluation_order,
    } = &args[0].kind
    else {
        panic!("exact static unit construction");
    };
    assert_eq!(*enum_type, packet.arguments[0].actual_type);
    assert_eq!(args[0].ty, *enum_type);
    assert!(payloads.is_empty() && evaluation_order.is_empty());
    let Type::Enum(id) = checked.interner.resolve(*enum_type) else {
        panic!("nominal Enum identity");
    };
    let declaration = checked.interner.resolve_enum(*id);
    assert_eq!(declaration.name, "models.Choice");
    assert_eq!(declaration.variants[variant.index() as usize].name, "empty");
    assert!(
        declaration.variants[variant.index() as usize]
            .fields
            .is_empty()
    );
    for replacement in ["local", "field", "public projection"] {
        let mut corrupted = program.clone();
        let holder = local_id(&corrupted.functions[index], "holder");
        let stored = local_id(&corrupted.functions[index], "stored");
        let holder_type = corrupted.functions[index].locals[holder.index() as usize].ty;
        let expression = returned_call_mut(&mut corrupted.functions[index]);
        let ExpressionKind::Call {
            args,
            ownership: CallOwnership::Source(packet),
            ..
        } = &mut expression.kind
        else {
            panic!("checked enum invocation");
        };
        match replacement {
            "local" => args[0].kind = ExpressionKind::Local(stored),
            "field" => {
                args[0].kind = ExpressionKind::Field {
                    base: Box::new(Expression {
                        kind: ExpressionKind::Local(holder),
                        ty: holder_type,
                        span: args[0].span,
                    }),
                    owner_type: holder_type,
                    field: FieldId::new(0),
                }
            }
            "public projection" => {
                packet.arguments[0].origin = CallerOrigin::OwnedFieldCopy {
                    parent: CallerViewSource::Other,
                }
            }
            _ => unreachable!(),
        }
        let expected = if replacement == "public projection" {
            "call ownership disagrees with its original source witness"
        } else {
            "call ownership cannot manufacture a producer acquisition from a binding or field"
        };
        let errors = validate_program_call_ownership(&corrupted, &checked.interner)
            .expect_err("static producer authority cannot become a real value origin");
        assert!(
            errors.iter().any(|error| error.message == expected),
            "{replacement}: {errors:?}"
        );
    }
}

#[test]
fn caller_packets_enum_valued_bindings_fields_and_temporary_fields_keep_exact_roots() {
    let source = r#"namespace models
export enum Choice:
    empty
    full(value: int64)
namespace app
struct Holder:
    choice: models.Choice
function inspect(view choice: models.Choice, marker: int64) returns int64:
    return marker
function from_field(view holder: Holder) returns int64:
    return inspect(holder.choice, 1)
function from_view(view holder: Holder) returns int64:
    return inspect(view holder.choice, 2)
function exercise(view stored: models.Choice) returns int64:
    return inspect(view stored, 3)
function from_temporary() returns int64:
    use models
    return inspect(Holder(choice: models.Choice.empty).choice, 4)
"#;
    let (program, checked) = checked_source(source);
    for name in ["from_field", "from_view", "exercise", "from_temporary"] {
        let function = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == name)
            .expect("checked runtime origin control");
        let ExpressionKind::Call {
            args,
            ownership: CallOwnership::Source(packet),
            ..
        } = &returned_call(function).kind
        else {
            panic!("ordinary enum call");
        };
        let argument = &packet.arguments[0];
        assert!(matches!(
            checked.interner.resolve(argument.actual_type),
            Type::Enum(_)
        ));
        let mut value = &args[0];
        if let ExpressionKind::View(inner) = &value.kind {
            value = inner;
        }
        match name {
            "from_field" => {
                assert_eq!(
                    argument.origin,
                    CallerOrigin::OwnedFieldCopy {
                        parent: CallerViewSource::Local(local_id(function, "holder")),
                    }
                );
                assert!(matches!(value.kind, ExpressionKind::Field { .. }));
            }
            "from_view" => {
                assert_eq!(
                    argument.origin,
                    CallerOrigin::BorrowedProjection {
                        source: CallerViewSource::Local(local_id(function, "holder")),
                    }
                );
                assert!(matches!(value.kind, ExpressionKind::Field { .. }));
            }
            "exercise" => {
                assert!(matches!(argument.origin, CallerOrigin::Binding(fact)
                    if fact.local == local_id(function, "stored")));
                assert!(matches!(value.kind, ExpressionKind::Local(_)));
            }
            "from_temporary" => {
                assert_eq!(
                    argument.origin,
                    CallerOrigin::OwnedFieldCopy {
                        parent: CallerViewSource::Other,
                    }
                );
                let ExpressionKind::Field { base, .. } = &value.kind else {
                    panic!("a real temporary field is not a static enum producer");
                };
                assert!(matches!(base.kind, ExpressionKind::StructConstruct { .. }));
            }
            _ => unreachable!(),
        }
    }
    validate_program_call_ownership(&program, &checked.interner)
        .expect("enum TypeId alone does not change source roots or copying rules");
}

#[test]
fn caller_packets_retained_pipeline_seals_physical_header_separately_from_raw_source() {
    let source = r#"namespace app
function read(view values: list[int64], extra: int64) returns int64:
    return extra
function exercise(view values: list[int64]) returns int64:
    return values into view read(2)
"#;
    let (program, checked) = checked_source(source);
    let index = exercise_index(&program);
    let ExpressionKind::Call {
        ownership: CallOwnership::Source(packet),
        args,
        ..
    } = &returned_call(&program.functions[index]).kind
    else {
        panic!("source pipeline");
    };
    let argument = &packet.arguments[0];
    assert_eq!(argument.retained_snapshot_span(), Some(args[0].span));
    assert_ne!(args[0].span, argument.source_span);
    assert_eq!(argument.source_span, {
        let ExpressionKind::View(raw) = &args[0].kind else {
            panic!("written pipeline View");
        };
        raw.span
    });
    assert_eq!(
        argument.effect,
        jett_typecheck::CheckedCallerEffect::RetainBorrow
    );
    assert_eq!(
        argument.syntax,
        jett_typecheck::CheckedCallerSyntax::WrittenView
    );
    assert_eq!(argument.staging, ArgumentStaging::Original);

    let mut changed = program.clone();
    let ExpressionKind::Call { args, .. } =
        &mut returned_call_mut(&mut changed.functions[index]).kind
    else {
        panic!("original pipeline");
    };
    args[0].span.start += 1;
    let errors = validate_program_call_ownership(&changed, &checked.interner)
        .expect_err("a new physical header cannot replace the private initial occurrence");
    assert!(
        errors
            .iter()
            .any(|error| error.message
                == "call ownership retained snapshot physical occurrence changed"),
        "{errors:?}"
    );
}

const GENERATED_CONVERTED_CALLABLE_SOURCE: &str = r#"namespace app
interface Named:
    function name(view self: Named) returns string
struct User:
    label: string
implement Named for User:
    function name(view self: User) returns string:
        return "user:{self.label}"
function show(view item: Named) returns string:
    return Named.name(view item)
function invoke_reader(reader: function(view User) returns string) returns string:
    User input = User(label: "higher-order")
    return reader(view input)
function exercise() returns string:
    function(function(view Named) returns string) returns string higher = invoke_reader
    return higher(show)
"#;
const GENERATED_CONVERTED_CONTAINER_SOURCE: &str = r#"namespace app
interface Named:
    function name(view self: Named) returns string
struct User:
    label: string
implement Named for User:
    function name(view self: User) returns string:
        return "user:{self.label}"
function make_reader(prefix: string) returns function(view Named) returns string:
    return function(view item: Named) returns string: return "{prefix}:{Named.name(view item)}"
function first_reader(items: list[function(view User) returns string]) returns string:
    User input = User(label: "higher-container")
    for callback in items:
        return callback(view input)
    return "empty"
function exercise() returns string:
    function(list[function(view Named) returns string]) returns string adapted = first_reader
    return adapted(list(make_reader("higher")))
"#;
const GENERATED_CONVERTED_TWO_READERS: &str = r#"namespace app
interface Named:
    function name(view self: Named) returns string
struct User:
    label: string
implement Named for User:
    function name(view self: User) returns string:
        return "user:{self.label}"
function show(view item: Named) returns string:
    return Named.name(view item)
function invoke_reader(first: function(view User) returns string, second: function(view User) returns string) returns string:
    User input = User(label: "higher-order")
    return "{first(view input)}:{second(view input)}"
function exercise() returns string:
    function(function(view Named) returns string, function(view Named) returns string) returns string higher = invoke_reader
    return higher(show, show)
"#;

fn checked_generated_conversion_source(source: &str, release: bool) -> (Program, CheckResult) {
    let file = FileId::new(0);
    let parsed = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "parse: {:?}", parsed.errors);
    let resolved = jett_resolve::resolve(&parsed.module);
    assert!(
        resolved
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Error),
        "resolve: {:?}",
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
            .all(|diagnostic| diagnostic.severity != Severity::Error),
        "check: {:?}",
        checked.diagnostics
    );
    let program = lower(
        &parsed.module,
        &resolved,
        &checked,
        &HashMap::from([(file, SourceOrigin::Project)]),
    )
    .expect("canonical generated adapter keeps its sealed raw actual");
    validate_program_call_ownership(&program, &checked.interner)
        .expect("complete generated ownership");
    (program, checked)
}

fn generated_converted_adapter_index(program: &Program, container: bool) -> usize {
    program
        .functions
        .iter()
        .position(|function| {
            function.body.statements.iter().any(|statement| {
                let StatementKind::Return(Some(Expression {
                    kind:
                        ExpressionKind::IndirectCall {
                            ownership: CallOwnership::Generated(packet),
                            args,
                            ..
                        },
                    ..
                })) = &statement.kind
                else {
                    return false;
                };
                matches!(packet.operation, GeneratedOperation::FunctionAdapter { .. })
                    && args.iter().zip(&packet.arguments).any(|(value, argument)| {
                        value.ty != argument.actual_type
                            && match &value.kind {
                                ExpressionKind::InterfaceCoerce { adapters, .. } => {
                                    container && !adapters.is_empty()
                                }
                                ExpressionKind::FunctionAdapter { .. } => !container,
                                _ => false,
                            }
                    })
            })
        })
        .expect("source-derived generated conversion body")
}

fn generated_converted_argument(function: &Function) -> (&Expression, &GeneratedArgumentOwnership) {
    let ExpressionKind::IndirectCall {
        ownership: CallOwnership::Generated(packet),
        args,
        ..
    } = &returned_call(function).kind
    else {
        panic!("generated indirect adapter call");
    };
    args.iter()
        .zip(&packet.arguments)
        .find(|(value, argument)| value.ty != argument.actual_type)
        .expect("original actual differs from converted physical formal")
}

fn generated_converted_endpoint(mut value: &Expression, actual: TypeId) -> &Expression {
    while value.ty != actual {
        value = match &value.kind {
            ExpressionKind::InterfaceCoerce { value, .. }
            | ExpressionKind::FunctionAdapter { value, .. } => value,
            _ => panic!("canonical finite conversion"),
        };
    }
    value
}

fn generated_converted_endpoint_mut(mut value: &mut Expression, actual: TypeId) -> &mut Expression {
    while value.ty != actual {
        value = match &mut value.kind {
            ExpressionKind::InterfaceCoerce { value, .. }
            | ExpressionKind::FunctionAdapter { value, .. } => value,
            _ => panic!("canonical finite conversion"),
        };
    }
    value
}

fn generated_conversion_locals(function: &Function) -> Vec<OwnershipLocalInfo> {
    function
        .locals
        .iter()
        .map(|local| OwnershipLocalInfo {
            ty: local.ty,
            mutable: local.mutable,
            span: local.span,
            view_source: local.view_source,
            is_view_parameter: function
                .params
                .iter()
                .any(|param| param.local == local.id && param.mode == ParamMode::View),
            view_iteration: None,
        })
        .collect()
}

#[test]
fn generated_converted_callable_rejoins_original_target_parameter_without_place_peeling() {
    for release in [false, true] {
        let (mut program, checked) =
            checked_generated_conversion_source(GENERATED_CONVERTED_CALLABLE_SOURCE, release);
        let index = generated_converted_adapter_index(&program, false);
        let function = &program.functions[index];
        let (value, argument) = generated_converted_argument(function);
        assert!(matches!(value.kind, ExpressionKind::FunctionAdapter { .. }));
        assert_eq!(
            call_ownership::immediate_source(value),
            CallerViewSource::Other,
            "a callable adapter remains outside general place provenance"
        );
        let raw = generated_converted_endpoint(value, argument.actual_type);
        let ExpressionKind::Local(local) = raw.kind else {
            panic!("sealed raw target parameter");
        };
        assert_eq!(
            argument.original_witness().origin(),
            CallerViewSource::Local(local)
        );
        assert_eq!(raw.span, argument.original_witness().source_span());
        assert_eq!(value.span, raw.span);
        assert_eq!(raw.ty, function.locals[local.index() as usize].ty);
        assert_eq!(argument.acquisition, GeneratedAcquisition::OwnedExpression);
        validate_generated_operand_tree(value, argument, &generated_conversion_locals(function))
            .expect("strict raw backing after canonical callable conversion");
        let count = program.functions.len();
        complete_value_conversions(&mut program, &checked.interner)
            .expect("existing adapter completion is repeatable");
        assert_eq!(program.functions.len(), count, "no new duplicate adapters");
    }
}

#[test]
fn generated_converted_container_rejoins_raw_local_without_erasing_callback_table() {
    for release in [false, true] {
        let (program, checked) =
            checked_generated_conversion_source(GENERATED_CONVERTED_CONTAINER_SOURCE, release);
        let function = &program.functions[generated_converted_adapter_index(&program, true)];
        let (value, argument) = generated_converted_argument(function);
        let ExpressionKind::InterfaceCoerce { adapters, .. } = &value.kind else {
            panic!("container conversion");
        };
        assert!(!adapters.is_empty());
        assert_eq!(
            call_ownership::immediate_source(value),
            CallerViewSource::Other
        );
        let raw = generated_converted_endpoint(value, argument.actual_type);
        let ExpressionKind::Local(local) = raw.kind else {
            panic!("raw list-of-callables target parameter");
        };
        assert_eq!(
            argument.original_witness().origin(),
            CallerViewSource::Local(local)
        );
        assert_eq!(argument.acquisition, GeneratedAcquisition::OwnedExpression);
        for adapter in adapters {
            let target = &program.functions[adapter.function.index() as usize];
            assert_eq!(target.id, adapter.function);
            assert_eq!(target.capture_count, 1);
            assert_eq!(target.params[0].ty, adapter.source);
            let Type::Function {
                params,
                view_params,
                return_type,
            } = checked.interner.resolve(adapter.target)
            else {
                panic!("checked callback target signature");
            };
            assert_eq!(
                target.params[1..]
                    .iter()
                    .map(|param| param.ty)
                    .collect::<Vec<_>>(),
                *params
            );
            assert_eq!(
                target.params[1..]
                    .iter()
                    .map(|param| param.mode == ParamMode::View)
                    .collect::<Vec<_>>(),
                *view_params
            );
            assert_eq!(target.return_type, *return_type);
        }
        validate_generated_operand_tree(value, argument, &generated_conversion_locals(function))
            .expect("raw origin does not replace independent callback table validation");
    }
}

#[test]
fn generated_converted_raw_occurrence_refuses_same_typed_backing_and_arbitrary_wrappers() {
    let (program, checked) =
        checked_generated_conversion_source(GENERATED_CONVERTED_TWO_READERS, false);
    let index = generated_converted_adapter_index(&program, false);
    let function = &program.functions[index];
    let (value, argument) = generated_converted_argument(function);
    let raw = generated_converted_endpoint(value, argument.actual_type);
    let ExpressionKind::Local(local) = raw.kind else {
        panic!("raw target parameter");
    };
    let spare = function
        .params
        .iter()
        .find(|param| param.local != local && param.ty == raw.ty)
        .expect("independent same-typed source parameter")
        .local;
    for mutation in [
        "physical span",
        "raw span",
        "foreign local",
        "same type clone",
        "same type adapter",
        "nonconversion",
    ] {
        let mut changed = program.clone();
        let function = &mut changed.functions[index];
        let ExpressionKind::IndirectCall {
            args,
            ownership: CallOwnership::Generated(packet),
            ..
        } = &mut returned_call_mut(function).kind
        else {
            panic!("generated converted call");
        };
        let actual = packet.arguments[0].actual_type;
        if mutation == "physical span" {
            args[0].span.start += 1;
        } else if mutation == "nonconversion" {
            let original = args[0].clone();
            args[0].kind = ExpressionKind::Clone(Box::new(original));
        } else {
            let raw = generated_converted_endpoint_mut(&mut args[0], actual);
            match mutation {
                "raw span" => raw.span.start += 1,
                "foreign local" => raw.kind = ExpressionKind::Local(spare),
                "same type clone" => raw.kind = ExpressionKind::Clone(Box::new(raw.clone())),
                "same type adapter" => {
                    raw.kind = ExpressionKind::FunctionAdapter {
                        value: Box::new(raw.clone()),
                        function: program.functions[index].id,
                    }
                }
                _ => unreachable!(),
            }
        }
        let errors = validate_program_call_ownership(&changed, &checked.interner).expect_err(
            "conversion cannot replace original raw occurrence/backing or mint same-type peel",
        );
        assert!(
            errors.iter().any(|error| matches!(
                error.message.as_str(),
                "generated operand differs from its original typed occurrence or backing"
                    | "generated operand has no checked actual/conversion type"
            )),
            "{mutation}: {errors:?}"
        );
    }
}

#[test]
fn generated_converted_raw_owner_keeps_borrowed_slot_gate_and_metadata_remap_identity() {
    let (program, _) =
        checked_generated_conversion_source(GENERATED_CONVERTED_CALLABLE_SOURCE, false);
    let function = &program.functions[generated_converted_adapter_index(&program, false)];
    let (value, argument) = generated_converted_argument(function);
    let raw = generated_converted_endpoint(value, argument.actual_type);
    let ExpressionKind::Local(local) = raw.kind else {
        panic!("raw original Local");
    };
    let mut locals = generated_conversion_locals(function);
    locals[local.index() as usize].is_view_parameter = true;
    assert_eq!(
        validate_generated_operand_tree(value, argument, &locals).unwrap_err(),
        "generated owner cannot acquire a borrowed binding"
    );

    let ExpressionKind::IndirectCall { ownership, .. } = &returned_call(function).kind else {
        panic!("adapter call");
    };
    let mut remapped = ownership.clone();
    remapped
        .remap_metadata_locals(|id| Ok::<_, ()>(LocalId::new(id.index() + 3)))
        .expect("structural remap");
    let CallOwnership::Generated(packet) = remapped else {
        panic!("same generated packet");
    };
    let argument = &packet.arguments[0];
    let mapped = LocalId::new(local.index() + 3);
    assert_eq!(
        argument.original_witness().origin(),
        CallerViewSource::Local(mapped)
    );
    let mut raw = raw.clone();
    raw.kind = ExpressionKind::Local(mapped);
    argument
        .original_witness()
        .validate_producer_shape(&raw)
        .expect("raw shape remaps with the origin");
    let mut stale = raw;
    stale.kind = ExpressionKind::Local(local);
    assert!(
        argument
            .original_witness()
            .validate_producer_shape(&stale)
            .is_err(),
        "old numeric Local cannot replace remapped original backing"
    );
}

const FLOW_PROJECTED_MACHINE: &str = r#"namespace app
interface Named:
    function name(view self: Named) returns string
struct Item:
    label: string
implement Named for Item:
    function name(view self: Item) returns string:
        return self.label
machine Session:
    states:
        active(item: Named)
        cached(item: Named)
        empty
machine Other:
    states:
        active(item: Named)
function exercise(source: Session, other: Session, foreign: Other) returns string:
    if source at active:
        return Named.name(view source.item)
    return "empty"
machine TaskState:
    states:
        ready(value: nothing)
        cached(value: nothing)
        empty
function pending_depth(value: nothing) returns int64:
    return 7
function copied(state: TaskState) returns int64:
    if state at ready:
        return pending_depth(state.value)
    return -1
"#;

fn flow_projected_call(function: &Function) -> &Expression {
    let StatementKind::If { then_block, .. } = &function.body.statements[0].kind else {
        panic!("source state guard");
    };
    let StatementKind::Return(Some(value)) = &then_block.statements[0].kind else {
        panic!("guarded source invocation");
    };
    value
}

fn flow_projected_call_mut(function: &mut Function) -> &mut Expression {
    let StatementKind::If { then_block, .. } = &mut function.body.statements[0].kind else {
        panic!("source state guard");
    };
    let StatementKind::Return(Some(value)) = &mut then_block.statements[0].kind else {
        panic!("guarded source invocation");
    };
    value
}

fn flow_projected_field_mut(expression: &mut Expression) -> &mut Expression {
    let ExpressionKind::Call { args, .. } = &mut expression.kind else {
        panic!("source invocation");
    };
    let value = &mut args[0];
    fn field(value: &mut Expression) -> &mut Expression {
        if matches!(value.kind, ExpressionKind::View(_)) {
            let ExpressionKind::View(inner) = &mut value.kind else {
                unreachable!()
            };
            return field(inner);
        }
        assert!(matches!(value.kind, ExpressionKind::Field { .. }));
        value
    }
    field(value)
}

#[test]
fn caller_packets_flow_projected_machine_calls_preserve_view_and_nothing_copy() {
    use jett_typecheck::CheckedCallerEffect;
    let (program, checked) = checked_source(FLOW_PROJECTED_MACHINE);
    for (name, effect) in [
        ("exercise", CheckedCallerEffect::RetainBorrow),
        ("copied", CheckedCallerEffect::Copy),
    ] {
        let function = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == name)
            .unwrap();
        let ExpressionKind::Call {
            ownership: CallOwnership::Source(packet),
            args,
            ..
        } = &flow_projected_call(function).kind
        else {
            panic!("original source packet");
        };
        let argument = &packet.arguments[0];
        let (local, span, occurrence, storage) = argument
            .source_witness()
            .projection_root()
            .expect("exact checked flow root");
        assert_eq!(argument.effect, effect);
        assert!(matches!(
            checked.interner.resolve(storage),
            Type::Machine(_)
        ));
        assert!(matches!(
            checked.interner.resolve(occurrence),
            Type::MachineState { .. }
        ));
        assert_eq!(function.locals[local.index() as usize].ty, storage);
        assert!(
            function
                .locals
                .iter()
                .all(|local| local.view_source.is_none())
        );
        let value = if let ExpressionKind::View(inner) = &args[0].kind {
            inner
        } else {
            &args[0]
        };
        let ExpressionKind::Field {
            base, owner_type, ..
        } = &value.kind
        else {
            panic!("source field");
        };
        assert_eq!(*owner_type, occurrence);
        assert_eq!(base.span, span);
        assert!(matches!(base.kind, ExpressionKind::Local(id) if id == local));
        assert_eq!(base.ty, occurrence);
        assert!(
            validate_local_view_initializer(
                &args[0],
                local,
                storage,
                args[0].ty,
                &checked.interner
            )
            .is_err(),
            "Source certificate cannot weaken the persistent alias helper"
        );
        if name == "copied" {
            assert_eq!(argument.actual_type, TypeInterner::NOTHING);
        }
    }
}

#[test]
fn caller_packets_flow_projected_machine_rejects_sibling_state_and_root_corruption() {
    let (program, mut checked) = checked_source(FLOW_PROJECTED_MACHINE);
    let index = exercise_index(&program);
    let function = &program.functions[index];
    let source = local_id(function, "source");
    let other = local_id(function, "other");
    let foreign = local_id(function, "foreign");
    let Type::Machine(machine) = *checked
        .interner
        .resolve(function.locals[source.index() as usize].ty)
    else {
        panic!("bare stored owner");
    };
    let Type::Machine(foreign_machine) = *checked
        .interner
        .resolve(function.locals[foreign.index() as usize].ty)
    else {
        panic!("foreign nominal owner");
    };
    let cached = checked
        .interner
        .resolve_machine(machine)
        .state_id("cached")
        .unwrap();
    let foreign_active = checked
        .interner
        .resolve_machine(foreign_machine)
        .state_id("active")
        .unwrap();
    let cached_type = checked.interner.intern(Type::MachineState {
        machine,
        state: cached,
    });
    let foreign_type = checked.interner.intern(Type::MachineState {
        machine: foreign_machine,
        state: foreign_active,
    });
    let stale = checked.interner.intern(Type::MachineState {
        machine,
        state: jett_types::MachineStateId::new(u32::MAX),
    });
    for mutation in [
        "sibling",
        "foreign",
        "stale",
        "root local",
        "root span",
        "field",
        "endpoint",
        "public origin",
    ] {
        let mut changed = program.clone();
        let invocation = flow_projected_call_mut(&mut changed.functions[index]);
        if mutation == "public origin" {
            source_packet_mut(invocation).arguments[0].origin = CallerOrigin::BorrowedProjection {
                source: CallerViewSource::Local(other),
            };
        } else {
            let field = flow_projected_field_mut(invocation);
            if mutation == "endpoint" {
                field.ty = TypeInterner::BOOL;
            } else {
                let ExpressionKind::Field {
                    base,
                    owner_type,
                    field,
                    ..
                } = &mut field.kind
                else {
                    panic!("exact field");
                };
                match mutation {
                    "sibling" | "foreign" | "stale" => {
                        let ty = match mutation {
                            "sibling" => cached_type,
                            "foreign" => foreign_type,
                            _ => stale,
                        };
                        *owner_type = ty;
                        base.ty = ty;
                    }
                    "root local" => base.kind = ExpressionKind::Local(other),
                    "root span" => base.span.start += 1,
                    "field" => *field = FieldId::new(1),
                    _ => unreachable!(),
                }
            }
        }
        assert!(
            validate_program_call_ownership(&changed, &checked.interner).is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn caller_packets_flow_projection_metadata_visits_and_remaps_private_root() {
    let (program, checked) = checked_source(FLOW_PROJECTED_MACHINE);
    let function = &program.functions[exercise_index(&program)];
    let ExpressionKind::Call { ownership, .. } = &flow_projected_call(function).kind else {
        panic!("source call");
    };
    let CallOwnership::Source(packet) = ownership else {
        panic!("source packet");
    };
    let original = packet.arguments[0]
        .source_witness()
        .projection_root()
        .unwrap();
    let mut visited = Vec::new();
    ownership.metadata_types(|ty| visited.push(ty));
    assert!(visited.contains(&original.2) && visited.contains(&original.3));
    let mut locals = Vec::new();
    ownership.metadata_local_ids(|local| locals.push(local));
    assert!(locals.contains(&original.0));
    let mut remapped = ownership.clone();
    remapped
        .remap_metadata_locals(|local| Ok::<_, ()>(LocalId::new(local.index() + 1)))
        .unwrap();
    let CallOwnership::Source(packet) = remapped else {
        panic!("remapped packet");
    };
    let rewritten = packet.arguments[0]
        .source_witness()
        .projection_root()
        .unwrap();
    assert_eq!(
        rewritten,
        (
            LocalId::new(original.0.index() + 1),
            original.1,
            original.2,
            original.3
        )
    );
    assert!(
        matches!(packet.arguments[0].origin, CallerOrigin::BorrowedProjection {
        source: CallerViewSource::Local(local) } if local == rewritten.0)
    );
    validate_program_call_ownership(&program, &checked.interner).unwrap();
}

const ORDINARY_PROJECTED_ROOTS: &str = r#"namespace app
interface Named:
    function name(view self: Named) returns string
struct Holder:
    item: Named
machine Session:
    states:
        active(item: Named)
        empty
function ordinary(view source: Holder) returns string:
    return Named.name(view source.item)
function qualified(view source: Session at active) returns string:
    return Named.name(view source.item)
"#;

#[test]
fn caller_packets_ordinary_and_declared_state_projection_roots_keep_existing_helper() {
    let (program, checked) = checked_source(ORDINARY_PROJECTED_ROOTS);
    for name in ["ordinary", "qualified"] {
        let function = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == name)
            .unwrap();
        let ExpressionKind::Call {
            ownership: CallOwnership::Source(packet),
            args,
            ..
        } = &returned_call(function).kind
        else {
            panic!("original source packet");
        };
        let argument = &packet.arguments[0];
        assert_eq!(argument.source_witness().projection_root(), None);
        assert_eq!(
            argument.effect,
            jett_typecheck::CheckedCallerEffect::RetainBorrow
        );
        let CallerOrigin::BorrowedProjection {
            source: CallerViewSource::Local(local),
        } = argument.origin
        else {
            panic!("exact original source root");
        };
        let stored = &function.locals[local.index() as usize];
        assert!(matches!(
            checked.interner.resolve(stored.ty),
            Type::Struct(_) | Type::MachineState { .. }
        ));
        validate_local_view_initializer(&args[0], local, stored.ty, args[0].ty, &checked.interner)
            .expect("ordinary stored root keeps the original stable-backing proof");
    }
}
