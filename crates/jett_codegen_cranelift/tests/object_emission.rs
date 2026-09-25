use std::collections::HashMap;

use jett_codegen_cranelift::{
    CodegenError, JETT_AOT_ENTRY_SUCCESS_V1, JETT_AOT_ENTRY_SYMBOL_V1, emit_host_object,
    emit_host_program_object, emit_object_for_target, emit_program_object_for_target, host_target,
    symbol_name,
};
use jett_common::{FileId, SourceOrigin};
use jett_hir::{DeclarationId, DeclarationKind, FunctionIdentity};
use jett_mir::{Program, ReflectedTypeDispatchArm, TerminatorKind};
use jett_typecheck::CheckedGenericSpecialization;
use jett_types::{Type, TypeInterner};
use object::{Object, ObjectSection, ObjectSymbol, RelocationTarget, SymbolKind, SymbolScope};

fn lower_source(source: &str) -> (Program, TypeInterner) {
    let file = FileId::new(0);
    let parsed = jett_parser::parse(source, file);
    assert!(
        parsed.errors.is_empty(),
        "parse errors: {:?}",
        parsed.errors
    );
    let resolved = jett_resolve::resolve(&parsed.module);
    let checked = jett_typecheck::check(&parsed.module, &resolved);
    assert!(
        checked
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
        "check errors: {:?}",
        checked.diagnostics
    );
    let hir = jett_hir::lower(
        &parsed.module,
        &resolved,
        &checked,
        &HashMap::from([(file, SourceOrigin::Project)]),
    )
    .expect("HIR lowering");
    let mir = jett_mir::lower(&hir, &checked.interner).expect("MIR lowering");
    (mir, checked.interner)
}

#[test]
fn emits_arithmetic_with_a_refined_integer_operand() {
    let (program, types) = lower_source(
        r#"namespace app
type NonZeroInt = int64 where value != 0
function quotient(value: int64, divisor: NonZeroInt) returns int64:
    return value / divisor
function remainder(value: int64, divisor: NonZeroInt) returns int64:
    return value modulo divisor
"#,
    );

    let object = emit_host_object(&program, &types).expect("refined integer object emission");
    assert_eq!(object.symbols.len(), 2);
}

#[test]
fn emits_coarsening_for_scalar_and_owned_refinements() {
    let (program, types) = lower_source(
        r#"namespace app
type Positive = int64 where value > 0
type AboveTen = Positive where value > 10
type NonEmpty = string where value != ""
function coarsen_positive(value: Positive) returns int64:
    return coarsen value
function coarsen_to_ancestor(value: AboveTen) returns int64:
    return coarsen value
function coarsen_non_empty(value: NonEmpty) returns string:
    return coarsen value
"#,
    );

    let object = emit_host_object(&program, &types).expect("refinement coarsen object emission");
    assert_eq!(object.symbols.len(), 3);
}

#[test]
fn emits_a_deterministic_host_object_for_scalar_control_flow() {
    let (program, types) = lower_source(
        r#"namespace app
function adjust(value: int64, delta: int64) returns int64:
    mutable int64 current = value + delta
    if current > 10:
        current = current - 1
    else:
        current = current + 1
    return current
function main() returns int64:
    return adjust(delta: 2, value: 10)
"#,
    );

    let caller = &program.functions[1];
    let jett_mir::TerminatorKind::Return(Some(call)) =
        &caller.blocks[caller.entry.index() as usize].terminator.kind
    else {
        panic!("expected direct call return");
    };
    let jett_hir::ExpressionKind::Call {
        evaluation_order, ..
    } = &call.kind
    else {
        panic!("expected direct call");
    };
    assert_eq!(
        evaluation_order,
        &[1, 0],
        "MIR arguments stay in parameter order while evaluation order stays lexical"
    );

    let first = emit_host_object(&program, &types).expect("first object emission");
    let second = emit_host_object(&program, &types).expect("second object emission");
    let explicit = emit_object_for_target(&program, &types, &host_target().to_string())
        .expect("explicit host object emission");

    assert_eq!(first.target, host_target().to_string());
    assert_eq!(first.symbols, second.symbols);
    assert_eq!(first.bytes, second.bytes);
    assert_eq!(first, explicit);
    assert_eq!(first.symbols.len(), 2);
    let object = object::File::parse(first.bytes.as_slice()).expect("parse emitted object");
    let object_symbols = object
        .symbols()
        .filter_map(|symbol| symbol.name().ok())
        .collect::<Vec<_>>();
    for symbol in &first.symbols {
        assert!(
            object_symbols.contains(&symbol.as_str()),
            "object does not define {symbol}"
        );
    }
    let mut relocated_symbols = Vec::new();
    for section in object.sections() {
        for (_, relocation) in section.relocations() {
            if let RelocationTarget::Symbol(symbol_index) = relocation.target()
                && let Ok(symbol) = object.symbol_by_index(symbol_index)
                && let Ok(name) = symbol.name()
            {
                relocated_symbols.push(name.to_string());
            }
        }
    }
    assert!(
        relocated_symbols.contains(&first.symbols[0]),
        "the caller must relocate against callee symbol {}; relocation targets: {relocated_symbols:?}",
        first.symbols[0]
    );
}

#[test]
fn omits_an_unreached_unsupported_stdlib_function() {
    let (mut program, types) = lower_source(
        r#"namespace app
function unused_text() returns string:
    return "unused"
function main() returns int64:
    return 7
"#,
    );
    let unused = program
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "unused_text")
        .expect("unused function");
    unused.identity.declaration.origin = SourceOrigin::Stdlib;
    let main = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "main")
        .expect("project root");
    let main_symbol = symbol_name(&main.identity, &types).expect("main symbol");

    let artifact = emit_host_object(&program, &types)
        .expect("unreached unsupported stdlib MIR must not block emission");

    assert_eq!(artifact.symbols, [main_symbol]);
    object::File::parse(artifact.bytes.as_slice()).expect("parse reachable-only object");
}

#[test]
fn emits_dependency_and_stdlib_functions_reached_from_a_project_root() {
    let (mut program, types) = lower_source(
        r#"namespace app
function dependency_leaf(value: int64) returns int64:
    return value + 1
function unrelated() returns int64:
    return 99
function stdlib_helper(value: int64) returns int64:
    return dependency_leaf(value)
function main() returns int64:
    return stdlib_helper(41)
"#,
    );
    let dependency_leaf = program
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "dependency_leaf")
        .expect("dependency function");
    dependency_leaf.identity.declaration.origin = SourceOrigin::Dependency("dep.math".to_string());
    let dependency_symbol =
        symbol_name(&dependency_leaf.identity, &types).expect("dependency symbol");
    let stdlib_helper = program
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "stdlib_helper")
        .expect("stdlib function");
    stdlib_helper.identity.declaration.origin = SourceOrigin::Stdlib;
    let stdlib_symbol = symbol_name(&stdlib_helper.identity, &types).expect("stdlib symbol");
    let unrelated = program
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "unrelated")
        .expect("unrelated function");
    unrelated.identity.declaration.origin = SourceOrigin::Stdlib;
    let main = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "main")
        .expect("project root");
    let main_symbol = symbol_name(&main.identity, &types).expect("main symbol");

    let first = emit_host_object(&program, &types).expect("reachable stdlib object emission");
    let second = emit_host_object(&program, &types).expect("deterministic repeated emission");

    assert_eq!(first, second);
    assert_eq!(
        first.symbols,
        [
            dependency_symbol.clone(),
            stdlib_symbol.clone(),
            main_symbol
        ]
    );
    let object = object::File::parse(first.bytes.as_slice()).expect("parse reachable object");
    let relocated_symbols = object
        .sections()
        .flat_map(|section| section.relocations().map(|(_, relocation)| relocation))
        .filter_map(|relocation| match relocation.target() {
            RelocationTarget::Symbol(index) => object.symbol_by_index(index).ok(),
            _ => None,
        })
        .filter_map(|symbol| symbol.name().ok())
        .collect::<Vec<_>>();
    assert!(
        relocated_symbols.contains(&dependency_symbol.as_str())
            && relocated_symbols.contains(&stdlib_symbol.as_str()),
        "reached dependency and stdlib functions must retain their original relocation symbols; relocation targets: {relocated_symbols:?}"
    );
}

#[test]
fn emits_a_deterministic_exported_program_entry_wrapper() {
    let (program, types) = lower_source(
        r#"namespace app
function helper() returns nothing:
    return nothing
function selected_entry() returns nothing:
    return helper()
"#,
    );
    let entry = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "selected_entry")
        .expect("selected entry");
    let entry_id = entry.id;
    let entry_symbol = symbol_name(&entry.identity, &types).expect("entry symbol");

    let first =
        emit_host_program_object(&program, &types, entry_id).expect("program object emission");
    let second =
        emit_host_program_object(&program, &types, entry_id).expect("repeated program emission");
    let explicit =
        emit_program_object_for_target(&program, &types, &host_target().to_string(), entry_id)
            .expect("explicit-target program emission");
    assert_eq!(first, second);
    assert_eq!(first, explicit);
    assert_eq!(JETT_AOT_ENTRY_SUCCESS_V1, 0);
    assert_eq!(
        first.symbols.last().map(String::as_str),
        Some(JETT_AOT_ENTRY_SYMBOL_V1)
    );

    let ordinary = emit_host_object(&program, &types).expect("ordinary object emission");
    assert!(
        !ordinary
            .symbols
            .iter()
            .any(|symbol| symbol == JETT_AOT_ENTRY_SYMBOL_V1),
        "ordinary object emission must remain wrapper-free"
    );

    let object = object::File::parse(first.bytes.as_slice()).expect("parse program object");
    let wrapper = object
        .symbols()
        .find(|symbol| symbol.name() == Ok(JETT_AOT_ENTRY_SYMBOL_V1))
        .expect("exported entry wrapper symbol");
    assert!(!wrapper.is_undefined());
    assert!(wrapper.is_global());
    assert_eq!(wrapper.kind(), SymbolKind::Text);
    assert!(matches!(
        wrapper.scope(),
        SymbolScope::Linkage | SymbolScope::Dynamic
    ));

    let relocated_symbols = object
        .sections()
        .flat_map(|section| section.relocations().map(|(_, relocation)| relocation))
        .filter_map(|relocation| match relocation.target() {
            RelocationTarget::Symbol(index) => object.symbol_by_index(index).ok(),
            _ => None,
        })
        .filter_map(|symbol| symbol.name().ok())
        .collect::<Vec<_>>();
    assert!(
        relocated_symbols.contains(&entry_symbol.as_str()),
        "entry wrapper must relocate against the exact selected MIR function symbol {entry_symbol}; relocation targets: {relocated_symbols:?}"
    );
}

#[test]
fn rejects_an_absent_program_entry_id() {
    let (mut program, types) = lower_source(
        r#"namespace app
function entry() returns nothing:
    return nothing
"#,
    );
    let entry = program.functions[0].id;
    program.functions.clear();

    let error = emit_host_program_object(&program, &types, entry)
        .expect_err("an absent MIR entry must be rejected explicitly");

    assert_eq!(
        error,
        CodegenError::MissingProgramEntry {
            function_id: entry.index()
        }
    );
}

#[test]
fn rejects_a_duplicate_program_entry_id() {
    let (mut program, types) = lower_source(
        r#"namespace app
function entry() returns nothing:
    return nothing
"#,
    );
    let entry = program.functions[0].id;
    program.functions.push(program.functions[0].clone());

    let error = emit_host_program_object(&program, &types, entry)
        .expect_err("a duplicate MIR entry ID must be rejected explicitly");

    assert_eq!(
        error,
        CodegenError::DuplicateProgramEntry {
            function_id: entry.index()
        }
    );
}

#[test]
fn rejects_incompatible_program_entry_signatures() {
    let cases = [
        (
            r#"namespace app
function entry(value: int64) returns nothing:
    return nothing
"#,
            "unsupported native entry capability parameter(s): int64",
        ),
        (
            r#"namespace app
function entry() returns int64:
    return 1
"#,
            "expected return type `nothing`",
        ),
    ];
    for (source, expected_message) in cases {
        let (program, types) = lower_source(source);
        let entry = program.functions[0].id;

        let error = emit_host_program_object(&program, &types, entry)
            .expect_err("an incompatible entry signature must be rejected explicitly");

        assert!(matches!(
            error,
            CodegenError::IncompatibleProgramEntry {
                function_id,
                message,
            } if function_id == entry.index() && message.contains(expected_message)
        ));
    }
}

#[test]
fn rejects_an_unreachable_program_entry() {
    let (mut program, types) = lower_source(
        r#"namespace app
function library_entry() returns nothing:
    return nothing
function project_root() returns nothing:
    return nothing
"#,
    );
    let library_entry = program
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "library_entry")
        .expect("library entry");
    library_entry.identity.declaration.origin = SourceOrigin::Stdlib;
    let entry = library_entry.id;

    let error = emit_host_program_object(&program, &types, entry)
        .expect_err("an unreachable library function cannot be selected as the program entry");

    assert_eq!(
        error,
        CodegenError::UnreachableProgramEntry {
            function_id: entry.index()
        }
    );
}

#[test]
fn honors_a_valid_nonzero_mir_entry_block() {
    let (mut program, types) = lower_source(
        r#"namespace app
function choose() returns int64:
    if true:
        return 1
    else:
        return 2
"#,
    );
    program.functions[0].entry = program.functions[0].blocks[1].id;
    program.functions[0].blocks[0].terminator.kind =
        program.functions[0].blocks[2].terminator.kind.clone();

    emit_host_object(&program, &types).expect("nonzero entry block object emission");
}

#[test]
fn rejects_a_control_flow_predecessor_to_the_abi_entry_block() {
    let (mut program, types) = lower_source(
        r#"namespace app
function identity(value: int64) returns int64:
    return value
"#,
    );
    let function = &mut program.functions[0];
    let entry = function.entry;
    function.blocks[entry.index() as usize].terminator.kind = TerminatorKind::Goto(entry);

    let error = emit_host_object(&program, &types)
        .expect_err("an ABI entry block cannot have a MIR predecessor");

    assert!(matches!(
        error,
        CodegenError::InvalidMirContract { message, .. }
            if message.contains("targets ABI entry block")
    ));
}

#[test]
fn emits_all_scalar_primitive_widths_and_operators() {
    let (program, types) = lower_source(
        r#"namespace app
function signed8(value: int8) returns bool:
    int8 added = value + 1
    int8 subtracted = added - 1
    int8 multiplied = subtracted * 2
    int8 divided = multiplied / 2
    int8 remainder = multiplied modulo 2
    return divided >= remainder
function signed16(value: int16) returns int16:
    return -value
function signed32(value: int32) returns int32:
    return value
function signed64_min_division() returns int64:
    return ((-9223372036854775807) - 1) / (-1)
function unsigned8(value: uint8) returns bool:
    uint8 added = value + 255
    uint8 divided = added / 2
    uint8 remainder = added modulo 2
    return divided < remainder
function unsigned16(value: uint16) returns uint16:
    return value + 65535
function unsigned32(value: uint32) returns uint32:
    return value + 4294967295
function unsigned64_max() returns uint64:
    return 18446744073709551615
function float32_ops(value: float32) returns bool:
    float32 calculated = ((value + 1.0) - 1.0) * 2.0 / 2.0
    return calculated != value
function float64_ops(value: float64) returns bool:
    return -value <= value
function bool_ops(left: bool, right: bool) returns bool:
    return !left || (left && right)
function count_down(value: int64) returns int64:
    mutable int64 current = value
    while current > 0:
        current = current - 1
    return current
function consume_nothing(value: nothing) returns nothing:
    return value
function call_nothing() returns nothing:
    return consume_nothing(nothing)
"#,
    );

    let artifact = emit_host_object(&program, &types).expect("scalar object emission");

    assert_eq!(artifact.symbols.len(), 14);
    object::File::parse(artifact.bytes.as_slice()).expect("parse scalar object");
}

#[test]
fn emits_direct_signed_minimum_literals_at_every_width() {
    let (program, types) = lower_source(
        r#"namespace app
function minimum8() returns int8:
    return -128
function minimum16() returns int16:
    return -32768
function minimum32() returns int32:
    return -2147483648
function minimum64() returns int64:
    return -9223372036854775808
"#,
    );

    let artifact = emit_host_object(&program, &types).expect("signed minimum object emission");

    assert_eq!(artifact.symbols.len(), 4);
    object::File::parse(artifact.bytes.as_slice()).expect("parse signed minimum object");
}

#[test]
fn rejects_statically_zero_integer_divisors() {
    for operator in ["/", "modulo"] {
        let source = format!(
            "namespace app\nfunction invalid(value: int64) returns int64:\n    return value {operator} 1\n"
        );
        let (mut program, types) = lower_source(&source);
        let function = &mut program.functions[0];
        let entry = function.entry.index() as usize;
        let TerminatorKind::Return(Some(expression)) = &mut function.blocks[entry].terminator.kind
        else {
            panic!("expected returned expression");
        };
        let jett_hir::ExpressionKind::Binary { right, .. } = &mut expression.kind else {
            panic!("expected binary expression");
        };
        right.kind = jett_hir::ExpressionKind::Int(0);

        let error = emit_host_object(&program, &types)
            .expect_err("a statically zero integer divisor must be rejected");

        assert!(matches!(
            error,
            CodegenError::InvalidMirContract { message, .. }
                if message.contains("statically zero divisor")
        ));
    }
}

#[test]
fn rejects_an_explicit_non_host_target_before_object_generation() {
    let program = Program {
        functions: Vec::new(),
    };
    let types = TypeInterner::new();

    let error = emit_object_for_target(&program, &types, "wasm32-unknown-unknown")
        .expect_err("cross-target emission must be rejected");

    assert!(matches!(
        error,
        CodegenError::UnsupportedTarget { requested, supported }
            if requested == "wasm32-unknown-unknown" && supported == host_target().to_string()
    ));
}

#[test]
fn rejects_malformed_target_text() {
    let program = Program {
        functions: Vec::new(),
    };
    let types = TypeInterner::new();

    let error = emit_object_for_target(&program, &types, "not a target")
        .expect_err("malformed target must be rejected");

    assert!(
        matches!(error, CodegenError::InvalidTarget { target, .. } if target == "not a target")
    );
}

#[test]
fn symbols_use_structural_types_instead_of_session_local_type_ids() {
    let mut first_types = TypeInterner::new();
    let first_argument = first_types.intern(Type::List(TypeInterner::INT64));
    let mut second_types = TypeInterner::new();
    second_types.intern(Type::Optional(TypeInterner::BOOL));
    let second_argument = second_types.intern(Type::List(TypeInterner::INT64));
    assert_ne!(first_argument, second_argument);

    let declaration = DeclarationId {
        origin: SourceOrigin::Dependency("example.math".to_string()),
        namespace: "numbers".to_string(),
        name: "identity".to_string(),
        kind: DeclarationKind::Function,
    };
    let first = FunctionIdentity {
        declaration: declaration.clone(),
        type_arguments: vec![first_argument],
        specialization: CheckedGenericSpecialization::default(),
    };
    let second = FunctionIdentity {
        declaration,
        type_arguments: vec![second_argument],
        specialization: CheckedGenericSpecialization::default(),
    };

    let first_symbol = symbol_name(&first, &first_types).expect("first symbol");
    let second_symbol = symbol_name(&second, &second_types).expect("second symbol");
    assert_eq!(first_symbol, second_symbol);
    assert_eq!(
        first_symbol,
        "jett_v0_e2a99a2b07c19f4355c2c76e50f4ff613045299da68e13af7a842a035e453591"
    );

    let specialized = FunctionIdentity {
        declaration: second.declaration.clone(),
        type_arguments: second.type_arguments.clone(),
        specialization: CheckedGenericSpecialization {
            type_argument_kinds: vec!["alias".to_string()],
            ..CheckedGenericSpecialization::default()
        },
    };
    assert_ne!(
        symbol_name(&second, &second_types).expect("unspecialized symbol"),
        symbol_name(&specialized, &second_types).expect("specialized symbol")
    );
}

#[test]
fn emits_custom_assert_message() {
    let (mut program, types) = lower_source(
        r#"namespace app
function checked(value: bool) returns bool:
    bool copy = value
    return value
"#,
    );
    let statement = &mut program.functions[0].blocks[0].statements[0];
    let condition = match &statement.kind {
        jett_mir::StatementKind::Let { value, .. } => value.clone(),
        other => panic!("expected lowered let, got {other:?}"),
    };
    statement.kind = jett_mir::StatementKind::Assert {
        message: Some(jett_hir::Expression {
            kind: jett_hir::ExpressionKind::String("detail".to_owned()),
            ty: TypeInterner::STRING,
            span: condition.span,
        }),
        condition,
    };

    let object = emit_host_object(&program, &types).expect("custom assert message emits");
    assert!(!object.bytes.is_empty());
}

#[test]
fn rejects_reflected_type_dispatch_without_type_info() {
    let (mut program, types) = lower_source(
        r#"namespace app
function choose(value: bool) returns int64:
    if value:
        return 1
    else:
        return 2
"#,
    );
    let function = &mut program.functions[0];
    let entry = function.entry.index() as usize;
    let (type_info, target, otherwise) = match function.blocks[entry].terminator.kind.clone() {
        TerminatorKind::Branch {
            condition,
            then_block,
            else_block,
        } => (condition, then_block, else_block),
        other => panic!("expected lowered branch, got {other:?}"),
    };
    function.blocks[entry].terminator.kind = TerminatorKind::ReflectedTypeDispatch {
        type_info,
        arms: vec![ReflectedTypeDispatchArm {
            iteration_index: 0,
            bound_type: TypeInterner::BOOL,
            canonical_identity: "4:bool0:0:0".to_string(),
            target,
        }],
        otherwise,
    };

    let error = emit_host_object(&program, &types)
        .expect_err("reflected dispatch must select a checked TypeInfo");

    assert!(matches!(
        error,
        CodegenError::InvalidMirContract { message, .. }
            if message == "invalid checked reflected type dispatch"
    ));
}

#[test]
fn rejects_invalid_mir_before_object_generation() {
    let (mut program, types) = lower_source(
        r#"namespace app
function callee() returns int64:
    return 1
function caller() returns int64:
    return callee()
"#,
    );
    program.functions.remove(0);

    let error = emit_host_object(&program, &types).expect_err("invalid MIR must be rejected");

    assert!(matches!(error, CodegenError::InvalidMir(_)));
}

#[test]
fn rejects_unbaked_comptime_instead_of_executing_it_at_runtime() {
    let (program, types) = lower_source("function main() returns int64:\n    return comptime 42\n");
    let error = emit_host_object(&program, &types)
        .expect_err("unbaked comptime must not become runtime code");
    assert!(error.to_string().contains("unbaked comptime"), "{error}");
}

#[test]
fn rejects_implicit_struct_equality_inside_enum_payloads() {
    let (program, types) = lower_source(
        r#"namespace app
struct Item:
    id: int64
enum Value:
    empty
    item(value: Item)
function main() returns bool:
    Value left = Value.item(Item(id: 1))
    Value right = Value.item(Item(id: 1))
    return left == right
"#,
    );
    let error = emit_host_object(&program, &types)
        .expect_err("nested user structs must not gain structural equality");
    assert!(
        error.to_string().contains("enum equality payload type"),
        "{error}"
    );
}

#[test]
fn malformed_sequence_element_type_returns_error_without_panicking() {
    for by_view in [true, false] {
        let source = format!(
            "function length(items: list[int64]) returns int64:\n    for item in {}items:\n        return item\n    return 0\n",
            if by_view { "view " } else { "" }
        );
        let (mut program, mut types) = lower_source(&source);
        emit_host_object(&program, &types).expect("valid baseline");
        let mut foreign = TypeInterner::new();
        let mut inner = TypeInterner::INT64;
        for _ in 0..1000 {
            inner = foreign.intern(jett_types::Type::List(inner));
        }
        assert!(inner.index() as usize > types.len());
        let invalid = types.intern(jett_types::Type::List(inner));
        for function in &mut program.functions {
            for block in &mut function.blocks {
                if let jett_mir::TerminatorKind::ForEach { iterable, .. } =
                    &mut block.terminator.kind
                {
                    iterable.ty = invalid;
                }
            }
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            emit_host_object(&program, &types)
        }));
        assert!(
            matches!(result, Ok(Err(_))),
            "must reject malformed element without panic: {result:?}"
        );
    }
}

#[test]
fn struct_layout_and_projection_contracts_are_validated_before_emission() {
    use jett_hir::ExpressionKind;
    let (baseline, types) = lower_source(
        r#"
struct Pair:
    number: int64
    text: string
function make() returns Pair:
    return Pair(number: 7, text: "value")
function read(view pair: Pair) returns string:
    return pair.text
"#,
    );
    emit_host_object(&baseline, &types).expect("valid struct layout");
    for corruption in 0..6 {
        let mut program = baseline.clone();
        if corruption < 4 {
            let TerminatorKind::Return(Some(value)) =
                &mut program.functions[0].blocks[0].terminator.kind
            else {
                panic!("constructor return");
            };
            let ExpressionKind::StructConstruct {
                fields,
                struct_type,
                evaluation_order,
                validates_refinements,
                ..
            } = &mut value.kind
            else {
                panic!("constructor");
            };
            match corruption {
                0 => fields[0].ty = TypeInterner::BOOL,
                1 => *struct_type = TypeInterner::INT64,
                2 => evaluation_order.swap(0, 1), // valid reordering, but duplicate below
                3 => *validates_refinements = true,
                _ => unreachable!(),
            }
            if corruption == 2 {
                evaluation_order[1] = evaluation_order[0];
            }
        } else {
            let TerminatorKind::Return(Some(value)) =
                &mut program.functions[1].blocks[0].terminator.kind
            else {
                panic!("projection return");
            };
            let ExpressionKind::Field { owner_type, .. } = &mut value.kind else {
                panic!("field");
            };
            if corruption == 4 {
                *owner_type = TypeInterner::INT64;
            } else {
                value.ty = TypeInterner::BOOL;
            }
        }
        let rejection = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            emit_host_object(&program, &types)
        }));
        assert!(
            matches!(rejection, Ok(Err(_))),
            "corruption {corruption}: {rejection:?}"
        );
    }
    // Copy-only public plan must not become a back door for struct ownership.
    assert!(jett_mir::copy_values::CopyValuePlan::analyze(&baseline.functions[0], &types).is_err());
}
