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
    lower_source_with_equatable(source, false)
}

fn lower_source_with_equatable(source: &str, include_equatable: bool) -> (Program, TypeInterner) {
    let file = FileId::new(0);
    let mut parsed = jett_parser::parse(source, file);
    assert!(
        parsed.errors.is_empty(),
        "parse errors: {:?}",
        parsed.errors
    );
    let mut origins = HashMap::from([(file, SourceOrigin::Project)]);
    if include_equatable {
        let prelude_file = FileId::new(jett_common::STDLIB_FILE_ID_START);
        let mut prelude = jett_parser::parse(
            "namespace stdlib\nexport interface Equatable:\n    function equals(view self: Equatable, view other: Equatable) returns bool\n",
            prelude_file,
        );
        assert!(prelude.errors.is_empty());
        prelude.module.items.append(&mut parsed.module.items);
        parsed.module.items = prelude.module.items;
        origins.insert(prelude_file, SourceOrigin::Stdlib);
    }
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
    let hir = jett_hir::lower(&parsed.module, &resolved, &checked, &origins).expect("HIR lowering");
    let mir = jett_mir::lower(&hir, &checked.interner).expect("MIR lowering");
    (mir, checked.interner)
}

#[test]
fn native_resource_kind_tags_emit_without_resource_values() {
    let file = FileId::new(jett_common::STDLIB_FILE_ID_START);
    let mut parsed = jett_parser::parse(
        r#"namespace opaque_metadata
export resource FileHandle
type HandleAlias = FileHandle
export function resource_kind() returns bool:
    return type.kind_tag[FileHandle]() == TypeKind.resource_type
export function resource_info() returns bool:
    return type.info[FileHandle]().kind_tag == TypeKind.resource_type
export function alias_kind() returns bool:
    return type.kind_tag[HandleAlias]() == TypeKind.alias_type
"#,
        file,
    );
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let project_file = FileId::new(0);
    let mut caller = jett_parser::parse(
        r#"namespace app
function main() returns bool:
    use opaque_metadata
    bool kind = opaque_metadata.resource_kind()
    bool info = opaque_metadata.resource_info()
    bool alias = opaque_metadata.alias_kind()
    return kind and info and alias
"#,
        project_file,
    );
    assert!(caller.errors.is_empty(), "{:?}", caller.errors);
    parsed.module.items.append(&mut caller.module.items);
    let resolved = jett_resolve::resolve(&parsed.module);
    let checked = jett_typecheck::check(&parsed.module, &resolved);
    assert!(
        checked
            .diagnostics
            .iter()
            .all(|diagnostic| { diagnostic.severity != jett_diagnostics::Severity::Error }),
        "{:?}",
        checked.diagnostics
    );
    let origins = HashMap::from([
        (file, SourceOrigin::Stdlib),
        (project_file, SourceOrigin::Project),
    ]);
    let hir = jett_hir::lower(&parsed.module, &resolved, &checked, &origins)
        .expect("resource metadata HIR");
    let mir = jett_mir::lower(&hir, &checked.interner).expect("resource metadata MIR");
    let artifact = emit_host_object(&mir, &checked.interner).expect("type-only resource object");
    assert!(!artifact.bytes.is_empty());
    for name in ["resource_kind", "resource_info", "alias_kind"] {
        let function = mir
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == name)
            .unwrap();
        assert!(
            artifact
                .symbols
                .contains(&symbol_name(&function.identity, &checked.interner).unwrap())
        );
    }
}

#[test]
fn native_reflected_root_reads_retain_only_selected_predicates_and_failure_leaf() {
    let (mut program, types) = lower_source(
        r#"namespace app
type Positive = int64 where value > 0
type Higher = Positive where value > 10
type Sibling = int64 where value >= 0
struct Record:
    raw: int64
    positive: Positive
    higher: Higher
    sibling: Sibling
enum Event:
    first(raw: int64, positive: Positive, higher: Higher, sibling: Sibling)
    second(higher: Higher, raw: int64)
machine Session:
    states:
        first(raw: int64, positive: Positive, higher: Higher, sibling: Sibling)
        second(higher: Higher, raw: int64)
    transitions:
        first to second
function record_read(view source: Record, view field: TypeField) returns Higher:
    return type.field_value[Record, Higher](view source, view field)
function event_read(view source: Event, view field: TypeField) returns Higher:
    return type.variant_field_value[Event, Higher](view source, view field)
function session_read(view source: Session, view field: TypeField) returns Higher:
    return type.machine_field_value[Session, Higher](view source, view field)
"#,
    );
    let mut predicate_symbols = HashMap::new();
    for function in &mut program.functions {
        if function.identity.declaration.kind == DeclarationKind::RefinementPredicate {
            function.identity.declaration.origin = SourceOrigin::Stdlib;
            predicate_symbols.insert(
                function.identity.declaration.name.clone(),
                symbol_name(&function.identity, &types).unwrap(),
            );
        }
    }
    let artifact = emit_host_object(&program, &types).expect("root reflected producer object");
    for name in ["Positive", "Higher"] {
        assert!(
            artifact.symbols.contains(&predicate_symbols[name]),
            "selected predicate {name} remains reachable"
        );
    }
    assert!(
        !artifact.symbols.contains(&predicate_symbols["Sibling"]),
        "declared sibling proof is not an extra requested predicate"
    );
    let object = object::File::parse(artifact.bytes.as_slice()).unwrap();
    let failure = jett_runtime::native_abi::values::NativeLeaf::RuntimeFailMessage.symbol();
    assert!(
        object.symbols().any(|s| s.name().ok() == Some(failure)),
        "dynamic first predicate error uses existing borrowed failure ABI"
    );
}

#[test]
fn native_reflected_root_emission_rejects_missing_plans_and_invalid_type_operands() {
    let source = r#"namespace app
type Positive = int64 where value > 0
struct Record:
    raw: int64
function main(view source: Record, view field: TypeField) returns Positive:
    return type.field_value[Record, Positive](view source, view field)
"#;
    for mutation in 0..4 {
        let (mut program, types) = lower_source(source);
        let mut foreign = TypeInterner::new();
        let mut invalid_type = TypeInterner::INT64;
        for index in 0..types.len() + 1 {
            invalid_type = foreign.intern(Type::Refinement {
                name: format!("Foreign{index}"),
                base: TypeInterner::INT64,
            });
        }
        assert!(invalid_type.index() as usize >= types.len());
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.identity.declaration.name == "main")
            .unwrap();
        let kind = function
            .blocks
            .iter_mut()
            .flat_map(|b| &mut b.statements)
            .find_map(|s| match &mut s.kind {
                jett_mir::StatementKind::Let {
                    value:
                        jett_hir::Expression {
                            kind: kind @ jett_hir::ExpressionKind::Intrinsic { .. },
                            ..
                        },
                    ..
                } => Some(kind),
                _ => None,
            })
            .unwrap();
        let jett_hir::ExpressionKind::Intrinsic {
            field_validation,
            type_arguments,
            ..
        } = kind
        else {
            unreachable!();
        };
        match mutation {
            0 => *field_validation = None,
            1 => *field_validation = Some(jett_hir::ReflectedFieldValidation::Validate(Vec::new())),
            2 => type_arguments[0] = invalid_type,
            3 => {
                type_arguments.pop();
            }
            _ => unreachable!(),
        }
        let error = emit_host_object(&program, &types).expect_err("malformed reflected selector");
        if mutation < 2 {
            assert!(
                matches!(error, CodegenError::InvalidMir(ref errors) if errors.iter().any(|e| e.message.contains("proof plans were not lowered"))),
                "{error:?}"
            );
        } else {
            assert!(
                matches!(error, CodegenError::InvalidMirContract { .. }),
                "{error:?}"
            );
        }
    }
}

#[test]
fn native_reflected_root_emission_rejects_wrong_same_signature_predicate() {
    let (mut program, types) = lower_source(
        r#"namespace app
type Positive = int64 where value > 0
type Sibling = int64 where value >= 0
struct Record:
    raw: int64
function main(view source: Record, view field: TypeField) returns Positive:
    return type.field_value[Record, Positive](view source, view field)
"#,
    );
    let sibling = program
        .functions
        .iter()
        .find(|f| {
            f.identity.declaration.kind == DeclarationKind::RefinementPredicate
                && f.identity.declaration.name == "Sibling"
        })
        .unwrap()
        .id;
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.identity.declaration.name == "main")
        .unwrap();
    let call = function
        .blocks
        .iter_mut()
        .flat_map(|b| &mut b.statements)
        .find_map(|s| match &mut s.kind {
            jett_mir::StatementKind::CheckRefinement { call, .. } => Some(call),
            _ => None,
        })
        .unwrap();
    let jett_hir::ExpressionKind::Call { function, .. } = &mut call.kind else {
        panic!("canonical predicate call");
    };
    *function = sibling;
    let error =
        emit_host_object(&program, &types).expect_err("predicate identity is part of proof");
    assert!(
        matches!(error, CodegenError::InvalidMir(ref errors) if errors.iter().any(|e| e.message.contains("exact predicate declaration"))),
        "{error:?}"
    );
}

#[test]
fn native_return_refinement_rejections_use_the_existing_borrowed_failure_leaf() {
    let (program, types) = lower_source(
        r#"namespace app
type Positive = int64 where value > 0
type Higher = Positive where value > 10
function source(raw: int64) returns int64:
    return run raw
function main(raw: int64) returns Higher:
    return source(raw)
"#,
    );
    let main = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "main")
        .unwrap();
    let checks = main
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .filter_map(|statement| {
            if let jett_mir::StatementKind::CheckRefinement { type_name, .. } = &statement.kind {
                Some(type_name.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(checks, ["app.Positive", "app.Higher"]);
    let artifact = emit_host_object(&program, &types).expect("return predicate object");
    let object = object::File::parse(artifact.bytes.as_slice()).unwrap();
    let leaf = jett_runtime::native_abi::values::NativeLeaf::RuntimeFailMessage;
    assert!(
        object
            .symbols()
            .any(|symbol| symbol.name().ok() == Some(leaf.symbol()))
    );
    assert_eq!(
        leaf.parameters(),
        &[
            jett_runtime::native_abi::values::AbiScalar::Pointer,
            jett_runtime::native_abi::values::AbiScalar::I64
        ]
    );
    assert_eq!(
        leaf.result(),
        jett_runtime::native_abi::values::AbiScalar::I32
    );
}

#[test]
fn native_return_dynamic_failure_keeps_its_message_producer_reachable() {
    let (mut program, types) = lower_source(
        r#"namespace app
function message() returns string:
    return "dynamic error"
function unreferenced_message() returns string:
    return "unused error"
function main() returns nothing:
    message()
    return nothing
"#,
    );
    let message = program
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "message")
        .unwrap();
    message.identity.declaration.origin = SourceOrigin::Stdlib;
    let message_id = message.id;
    let message_symbol = symbol_name(&message.identity, &types).unwrap();
    let unreferenced = program
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "unreferenced_message")
        .unwrap();
    unreferenced.identity.declaration.origin = SourceOrigin::Stdlib;
    let unreferenced_symbol = symbol_name(&unreferenced.identity, &types).unwrap();
    let main = program
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "main")
        .unwrap();
    let statement = main.blocks[main.entry.index() as usize]
        .statements
        .iter_mut()
        .find(|statement| {
            matches!(&statement.kind,
                jett_mir::StatementKind::Evaluate(jett_hir::Expression {
                    kind: jett_hir::ExpressionKind::Call { function, .. }, ..
                }) if *function == message_id)
        })
        .expect("checked message call statement");
    let jett_mir::StatementKind::Evaluate(value) = &mut statement.kind else {
        unreachable!("selected Evaluate statement");
    };
    assert!(matches!(&value.kind,
        jett_hir::ExpressionKind::Call {
            function,
            ownership: jett_hir::CallOwnership::Source(_),
            ..
        } if *function == message_id));
    let span = value.span;
    // Reparent the checked call without changing its source certificate.
    let message_call = std::mem::replace(
        value,
        jett_hir::Expression {
            kind: jett_hir::ExpressionKind::Nothing,
            ty: TypeInterner::NOTHING,
            span,
        },
    );
    *value = jett_hir::Expression {
        kind: jett_hir::ExpressionKind::RuntimeFailureMessage(Box::new(message_call)),
        ty: TypeInterner::NOTHING,
        span,
    };
    let artifact = emit_host_object(&program, &types).expect("dynamic message producer");
    assert!(artifact.symbols.contains(&message_symbol));
    assert!(!artifact.symbols.contains(&unreferenced_symbol));
}

#[test]
fn native_return_dynamic_failure_rejects_malformed_types_before_emission() {
    for (input, output) in [
        (TypeInterner::INT64, TypeInterner::NOTHING),
        (TypeInterner::STRING, TypeInterner::STRING),
    ] {
        let (mut program, types) =
            lower_source("function main() returns nothing:\n    return nothing\n");
        let function = &mut program.functions[0];
        let span = function.span;
        function.blocks[function.entry.index() as usize]
            .statements
            .insert(
                0,
                jett_mir::Statement {
                    kind: jett_mir::StatementKind::Evaluate(jett_hir::Expression {
                        kind: jett_hir::ExpressionKind::RuntimeFailureMessage(Box::new(
                            jett_hir::Expression {
                                kind: if input == TypeInterner::STRING {
                                    jett_hir::ExpressionKind::String("error".into())
                                } else {
                                    jett_hir::ExpressionKind::Int(7)
                                },
                                ty: input,
                                span,
                            },
                        )),
                        ty: output,
                        span,
                    }),
                    span,
                },
            );
        let error = emit_host_object(&program, &types).expect_err("invalid dynamic failure");
        assert!(
            matches!(error, CodegenError::InvalidMirContract { ref message, .. }
            if message.contains("dynamic runtime failure")),
            "{error:?}"
        );
    }
}

#[test]
fn display_result_checks_retain_the_selected_method_and_borrowed_runtime_leaf() {
    let (mut program, types) = lower_source(
        r#"interface Displayable:
    function display(view self: Displayable) returns string
namespace app
struct Item:
    value: int64
implement Displayable for Item:
    function display(view self: Item) returns string:
        return run "shown:{self.value}"
function main() returns string:
    Item item = Item(value: 7)
    return "value:{item}"
"#,
    );
    let main = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "main")
        .unwrap();
    let TerminatorKind::Return(Some(value)) =
        &main.blocks[main.entry.index() as usize].terminator.kind
    else {
        panic!("expected interpolation return");
    };
    let jett_hir::ExpressionKind::StringInterpolation(segments) = &value.kind else {
        panic!("expected interpolation");
    };
    let selected = segments
        .iter()
        .find_map(|segment| match segment {
            jett_hir::StringSegment::Value(jett_hir::Expression {
                kind: jett_hir::ExpressionKind::DisplayResult(value),
                ..
            }) => match value.kind {
                jett_hir::ExpressionKind::Call { function, .. } => Some(function),
                _ => None,
            },
            _ => None,
        })
        .expect("selected display call has a result check");
    for function in &mut program.functions {
        if function.identity.declaration.name != "main" {
            function.identity.declaration.origin = SourceOrigin::Stdlib;
        }
    }
    let selected_symbol = symbol_name(
        &program.functions[selected.index() as usize].identity,
        &types,
    )
    .unwrap();
    let artifact = emit_host_object(&program, &types).expect("checked display object");
    assert!(artifact.symbols.contains(&selected_symbol));
    let object = object::File::parse(artifact.bytes.as_slice()).unwrap();
    let check = jett_runtime::native_abi::values::NativeLeaf::DisplayResultCheck;
    assert!(
        object
            .symbols()
            .any(|symbol| symbol.is_undefined() && symbol.name().ok() == Some(check.symbol()))
    );
    assert_eq!(
        check.parameters(),
        &[
            jett_runtime::native_abi::values::AbiScalar::Pointer,
            jett_runtime::native_abi::values::AbiScalar::I64,
        ]
    );
    assert_eq!(
        check.result(),
        jett_runtime::native_abi::values::AbiScalar::I32
    );
}

#[test]
fn rejects_malformed_display_result_types_before_emission() {
    for (child_type, result_type) in [
        (TypeInterner::INT64, TypeInterner::STRING),
        (TypeInterner::STRING, TypeInterner::INT64),
    ] {
        let (mut program, types) =
            lower_source("function main() returns nothing:\n    return nothing\n");
        let function = &mut program.functions[0];
        let child = jett_hir::Expression {
            kind: if child_type == TypeInterner::STRING {
                jett_hir::ExpressionKind::String("ready".into())
            } else {
                jett_hir::ExpressionKind::Int(7)
            },
            ty: child_type,
            span: function.span,
        };
        function.blocks[function.entry.index() as usize]
            .statements
            .insert(
                0,
                jett_mir::Statement {
                    kind: jett_mir::StatementKind::Evaluate(jett_hir::Expression {
                        kind: jett_hir::ExpressionKind::DisplayResult(Box::new(child)),
                        ty: result_type,
                        span: function.span,
                    }),
                    span: function.span,
                },
            );
        let error = emit_host_object(&program, &types).expect_err("invalid display result");
        assert!(
            matches!(error, CodegenError::InvalidMirContract { ref message, .. }
                if message.contains("display result")),
            "{error:?}"
        );
    }
}

#[test]
fn equality_result_checks_preserve_qualified_calls_and_method_reachability() {
    for operator in ["==", "!="] {
        for qualified in [false, true] {
            let input = if qualified { "secret[Item]" } else { "Item" };
            let result = if qualified { "secret[bool]" } else { "bool" };
            let (mut program, types) = lower_source_with_equatable(
                &format!(
                    "namespace app\nstruct Item:\n    value: int64\nimplement Equatable for Item:\n    function equals(view self: Item, view other: Item) returns bool:\n        return run (self.value == other.value)\nfunction compare(view left: {input}, view right: Item) returns {result}:\n    return left {operator} right\n"
                ),
                true,
            );
            let compare = program
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == "compare")
                .unwrap();
            let TerminatorKind::Return(Some(value)) = &compare.blocks
                [compare.entry.index() as usize]
                .terminator
                .kind
            else {
                panic!("expected comparison return");
            };
            let checked = if operator == "!=" {
                let jett_hir::ExpressionKind::Unary {
                    op: jett_hir::UnaryOp::Not,
                    value,
                } = &value.kind
                else {
                    panic!("inequality must negate the checked method result");
                };
                value.as_ref()
            } else {
                value
            };
            let jett_hir::ExpressionKind::EquatableResult(call) = &checked.kind else {
                panic!("implicit equality call must check its result");
            };
            assert_eq!(call.ty, checked.ty);
            assert_eq!(checked.ty, compare.return_type);
            let jett_hir::ExpressionKind::Call { function, .. } = call.kind else {
                panic!("expected selected equality method");
            };
            for candidate in &mut program.functions {
                if candidate.identity.declaration.name != "compare" {
                    candidate.identity.declaration.origin = SourceOrigin::Stdlib;
                }
            }
            let method_symbol = symbol_name(
                &program.functions[function.index() as usize].identity,
                &types,
            )
            .unwrap();
            let artifact = emit_host_object(&program, &types).expect("checked equality object");
            assert!(artifact.symbols.contains(&method_symbol));
            let object = object::File::parse(artifact.bytes.as_slice()).unwrap();
            let check = jett_runtime::native_abi::values::NativeLeaf::RejectPendingScalars;
            assert!(
                object
                    .symbols()
                    .any(|symbol| symbol.is_undefined()
                        && symbol.name().ok() == Some(check.symbol()))
            );
        }
    }
}

#[test]
fn rejects_malformed_equality_result_types_before_emission() {
    for mutation in ["integer", "secret mismatch", "refinement"] {
        let (mut program, mut types) = lower_source(
            "namespace app\ntype Truth = bool where true\nfunction main() returns nothing:\n    return nothing\n",
        );
        let refined = types
            .type_ids()
            .find(|id| matches!(types.resolve(*id), Type::Refinement { .. }))
            .unwrap();
        let secret = types.intern(Type::Secret(TypeInterner::BOOL));
        let (input, output) = match mutation {
            "integer" => (TypeInterner::INT64, TypeInterner::BOOL),
            "secret mismatch" => (TypeInterner::BOOL, secret),
            "refinement" => (refined, refined),
            _ => unreachable!(),
        };
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "main")
            .unwrap();
        let child = jett_hir::Expression {
            kind: if input == TypeInterner::INT64 {
                jett_hir::ExpressionKind::Int(7)
            } else {
                jett_hir::ExpressionKind::Bool(true)
            },
            ty: input,
            span: function.span,
        };
        function.blocks[function.entry.index() as usize]
            .statements
            .insert(
                0,
                jett_mir::Statement {
                    kind: jett_mir::StatementKind::Evaluate(jett_hir::Expression {
                        kind: jett_hir::ExpressionKind::EquatableResult(Box::new(child)),
                        ty: output,
                        span: function.span,
                    }),
                    span: function.span,
                },
            );
        let error = emit_host_object(&program, &types).expect_err("invalid equality result");
        assert!(
            matches!(error, CodegenError::InvalidMirContract { ref message, .. }
                if message.contains("equality result")),
            "{mutation}: {error:?}"
        );
    }
}

#[test]
fn rejects_malformed_property_case_metadata_before_emission() {
    for (name, trial, ty) in [
        ("", 1, TypeInterner::NOTHING),
        ("property", 0, TypeInterner::NOTHING),
        ("property", 1, TypeInterner::INT64),
    ] {
        let (mut program, types) =
            lower_source("function main() returns nothing:\n    return nothing\n");
        let function = &mut program.functions[0];
        function.blocks[0].statements.insert(
            0,
            jett_mir::Statement {
                kind: jett_mir::StatementKind::Evaluate(jett_hir::Expression {
                    kind: jett_hir::ExpressionKind::PropertyCaseContext(Some(
                        jett_hir::NativePropertyCase {
                            name: name.to_owned(),
                            trial,
                        },
                    )),
                    ty,
                    span: function.span,
                }),
                span: function.span,
            },
        );
        let error = emit_host_object(&program, &types).expect_err("invalid case metadata");
        assert!(
            error.to_string().contains("property case context"),
            "{error}"
        );
    }
}

#[test]
fn emits_function_debug_descriptors_with_source_labels() {
    let (program, types) = lower_source(
        r#"namespace display
function named(first: int64, second: int64) returns int64:
    return first + second
function factory[T](value: T) returns function(T) returns T:
    return function(input: T) returns T: return value
struct Holder:
    callback: function(int64, int64) returns int64
function main() returns int64:
    function(int64, int64) returns int64 declared = named
    function(int64, int64) returns int64 plain = function(left: int64, right: int64) returns int64: return left + right
    int64 delta = 3
    function(int64) returns int64 captured = function(argument: int64) returns int64: return argument + delta
    function(int64) returns int64 generic = factory[int64](9)
    Holder holder = Holder(callback: clone declared)
    list[optional[function(int64, int64) returns int64]] nested = list(some(clone plain))
    trace declared
    trace plain
    trace captured
    trace generic
    trace holder
    trace nested
    breakpoint
    return declared(1, 2) + plain(1, 2) + captured(4) + generic(8)
"#,
    );
    let artifact = emit_host_object(&program, &types).expect("function debug object emission");
    for label in [
        "function(display.named)",
        "function(left, right)",
        "function(argument)",
        "function(input)",
    ] {
        assert!(
            artifact
                .bytes
                .windows(label.len())
                .any(|bytes| bytes == label.as_bytes()),
            "missing source label {label}"
        );
    }
    for function in &program.functions {
        let label_is_inline = matches!(function.debug_kind, jett_hir::FunctionDebugKind::Inline);
        if function.capture_count > 0 {
            assert!(
                label_is_inline,
                "captured source closure must retain inline identity"
            );
        }
    }
}

#[test]
fn rejects_malformed_function_debug_metadata_before_emission() {
    let (program, types) = lower_source(
        "namespace app\nfunction identity(value: int64) returns int64:\n    return value\n",
    );
    for name in ["", "app.identity\n"] {
        let mut malformed = program.clone();
        malformed.functions[0].debug_kind = jett_hir::FunctionDebugKind::Named(name.into());
        let error = emit_host_object(&malformed, &types).expect_err("invalid named metadata");
        assert!(
            error
                .to_string()
                .contains("debug identity has invalid source name metadata"),
            "{error}"
        );
    }
    let mut malformed = program.clone();
    malformed.functions[0].debug_kind = jett_hir::FunctionDebugKind::Inline;
    malformed.functions[0].params[0].name.clear();
    malformed.functions[0].locals[0].name.clear();
    let error = emit_host_object(&malformed, &types).expect_err("invalid inline metadata");
    assert!(
        error
            .to_string()
            .contains("debug parameter has invalid source name metadata"),
        "{error}"
    );

    let mut malformed = program;
    malformed.functions[0].capture_count = 2;
    assert!(matches!(
        emit_host_object(&malformed, &types),
        Err(CodegenError::InvalidMir(_))
    ));
}

#[test]
fn rejects_function_references_that_omit_a_capture_environment() {
    let (mut program, types) = lower_source(
        "namespace app\nfunction identity(value: int64) returns int64:\n    return value\nfunction factory() returns function(int64) returns int64:\n    return identity\n",
    );
    program.functions[0].capture_count = 1;
    let error =
        emit_host_object(&program, &types).expect_err("function reference needs environment");
    assert!(
        error
            .to_string()
            .contains("function value signature does not match target"),
        "{error}"
    );
}

#[test]
fn emits_checked_secret_arguments_for_direct_and_indirect_calls() {
    for callee in ["callback", "factory()"] {
        for (parameter, returned, body, argument, result) in [
            ("int64", "int64", "value", "secret[int64]", "secret[int64]"),
            ("secret[int64]", "int64", "7", "int64", "int64"),
            ("secret[int64]", "int64", "7", "secret[int64]", "int64"),
            (
                "int64",
                "secret[int64]",
                "value",
                "secret[int64]",
                "secret[int64]",
            ),
            ("int64", "nothing", "nothing", "secret[int64]", "nothing"),
            (
                "Positive",
                "Positive",
                "value",
                "secret[Positive]",
                "secret[Positive]",
            ),
            ("Count", "Count", "value", "secret[Count]", "secret[Count]"),
            ("int64", "int64", "value", "Classified", "secret[int64]"),
            (
                "Classified",
                "Classified",
                "value",
                "secret[Classified]",
                "Classified",
            ),
        ] {
            let source = format!(
                "namespace app\n\
                 type Positive = int64 where value > 0\n\
                 type Count = int64\n\
                 type Classified = secret[int64] where true\n\
                 function callback(value: {parameter}) returns {returned}:\n    return {body}\n\
                 function factory() returns function({parameter}) returns {returned}:\n    return callback\n\
                 function caller(value: {argument}) returns {result}:\n    return {callee}(value)\n"
            );
            let (program, types) = lower_source(&source);
            emit_host_object(&program, &types)
                .unwrap_or_else(|error| panic!("{source}\nobject emission failed: {error:?}"));
        }
    }
}

fn returned_call_mut(function: &mut jett_mir::Function) -> &mut jett_hir::Expression {
    let entry = function.entry.index() as usize;
    let TerminatorKind::Return(Some(expression)) = &mut function.blocks[entry].terminator.kind
    else {
        panic!("expected returned call");
    };
    assert!(matches!(
        expression.kind,
        jett_hir::ExpressionKind::Call { .. } | jett_hir::ExpressionKind::IndirectCall { .. }
    ));
    expression
}

#[test]
fn rejects_secret_lifted_calls_with_untainted_mir_results() {
    for callee in ["callback", "factory()"] {
        let source = format!(
            "namespace app\n\
             function callback(value: int64) returns int64:\n    return value\n\
             function factory() returns function(int64) returns int64:\n    return callback\n\
             function caller(value: secret[int64]) returns secret[int64]:\n    return {callee}(value)\n"
        );
        let (mut program, types) = lower_source(&source);
        let caller = program
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "caller")
            .expect("caller");
        caller.return_type = TypeInterner::INT64;
        returned_call_mut(caller).ty = TypeInterner::INT64;

        let error = emit_host_object(&program, &types)
            .expect_err("secret lifting must retain the checked result taint");
        assert!(
            matches!(error, CodegenError::InvalidMir(ref errors)
                if errors.iter().any(|error| error.message == "call ownership result differs from its checked source certificate")),
            "{callee}: {error:?}"
        );
    }
}

#[test]
fn container_callback_adapters_have_native_data_relocations() {
    let (mut program, types) = lower_source(CONTAINER_CALLBACK_SOURCE);
    let adapter = program
        .functions
        .iter_mut()
        .find(|function| {
            function
                .identity
                .declaration
                .name
                .starts_with("$interface.adapter.")
        })
        .expect("generated container callback adapter");
    // It must remain reachable through the descriptor, independently of project roots.
    adapter.identity.declaration.origin = SourceOrigin::Stdlib;
    let symbol = symbol_name(&adapter.identity, &types).unwrap();
    let target = host_target().to_string();
    {
        let emitted = emit_object_for_target(&program, &types, &target).unwrap();
        let object = object::File::parse(emitted.bytes.as_slice()).unwrap();
        let mut found = false;
        for section in object
            .sections()
            .filter(|section| section.kind() != object::SectionKind::Text)
        {
            for (_, relocation) in section.relocations() {
                if let RelocationTarget::Symbol(index) = relocation.target()
                    && object.symbol_by_index(index).unwrap().name().unwrap() == symbol
                {
                    assert_eq!(relocation.size(), 64);
                    found = true;
                }
            }
        }
        assert!(
            found,
            "{target} omitted the callback descriptor relocation for {symbol}"
        );
    }
}

const CONTAINER_CALLBACK_SOURCE: &str = "type Source = function(secret[int64]) returns secret[int64]\ntype Target = function(int64) returns secret[int64]\nfunction convert(items: list[Source]) returns list[Target]:\n    return items\n";

#[test]
fn container_callback_adapters_reject_missing_or_invalid_checked_metadata() {
    for mutation in 0..4 {
        let (mut program, types) = lower_source(CONTAINER_CALLBACK_SOURCE);
        let convert = program
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "convert")
            .unwrap();
        let TerminatorKind::Return(Some(expression)) = &mut convert.blocks
            [convert.entry.index() as usize]
            .terminator
            .kind
        else {
            panic!("expected a returned container conversion");
        };
        let jett_hir::ExpressionKind::InterfaceCoerce { adapters, .. } = &mut expression.kind
        else {
            panic!("expected container callback metadata");
        };
        assert_eq!(adapters.len(), 1);
        match mutation {
            0 => adapters.clear(),
            1 => adapters[0].function = convert.id,
            2 => adapters[0].target = adapters[0].source,
            _ => adapters.push(adapters[0]),
        }
        let error = emit_host_object(&program, &types).expect_err("malformed callback conversion");
        assert!(
            matches!(error, CodegenError::InvalidMirContract { .. }),
            "{mutation}: {error:?}"
        );
    }
}

#[test]
fn interface_coercion_cannot_remove_secret_call_result_taint() {
    let source = "function identity(value: int64) returns int64:\n    return value\nfunction caller(value: secret[int64]) returns secret[int64]:\n    return identity(value)\n";
    let (mut program, types) = lower_source(source);
    let caller = program
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "caller")
        .unwrap();
    caller.return_type = TypeInterner::INT64;
    let value = returned_call_mut(caller);
    value.kind = jett_hir::ExpressionKind::interface_coerce(Box::new(value.clone()));
    value.ty = TypeInterner::INT64;
    let error = emit_host_object(&program, &types)
        .expect_err("interface coercion cannot declassify a secret");
    assert!(
        matches!(error, CodegenError::InvalidMirContract { ref message, .. } if message.contains("interface conversion")),
        "{error:?}"
    );
}

#[test]
fn rejects_secret_lifting_with_mismatched_argument_types() {
    for callee in ["callback", "factory()"] {
        for parameter in ["int64", "Positive"] {
            let source = format!(
                "namespace app\n\
                 type Positive = int64 where value > 0\n\
                 function callback(value: {parameter}) returns {parameter}:\n    return value\n\
                 function factory() returns function({parameter}) returns {parameter}:\n    return callback\n\
                 function caller(value: secret[{parameter}]) returns secret[{parameter}]:\n    return {callee}(value)\n"
            );
            let (mut program, mut types) = lower_source(&source);
            let expression = returned_call_mut(
                program
                    .functions
                    .iter_mut()
                    .find(|function| function.identity.declaration.name == "caller")
                    .expect("caller"),
            );
            let (jett_hir::ExpressionKind::Call { args, .. }
            | jett_hir::ExpressionKind::IndirectCall { args, .. }) = &mut expression.kind
            else {
                unreachable!("returned_call_mut validates the expression kind");
            };
            // Neither an unrelated primitive nor the refinement's bare base is
            // a valid replacement for its checked secret payload type.
            let payload_type = if parameter == "Positive" {
                TypeInterner::INT64
            } else {
                TypeInterner::INT32
            };
            args[0].ty = types.intern(Type::Secret(payload_type));
            args[0].kind = jett_hir::ExpressionKind::Int(7);

            let error = emit_host_object(&program, &types)
                .expect_err("secret lifting must preserve the expected payload type");
            assert!(
                matches!(error, CodegenError::InvalidMir(ref errors)
                    if errors.iter().any(|error| error.message == if callee == "callback" {
                        "call ownership converted operand differs from its physical parameter"
                    } else { "call ownership indirect converted operand differs from its physical parameter" })),
                "{callee}({parameter}): {error:?}"
            );
        }
    }
}

#[test]
fn rejects_secret_lifting_into_impure_call_parameters() {
    for callee in ["callback", "factory()"] {
        let source = format!(
            "namespace app\n\
             function callback(view stdout: Stdout, value: int64) returns int64:\n    return value\n\
             function factory() returns function(view Stdout, int64) returns int64:\n    return callback\n\
             function caller(view stdout: Stdout, value: int64) returns int64:\n    return {callee}(view stdout, value)\n"
        );
        let (mut program, mut types) = lower_source(&source);
        let expression = returned_call_mut(
            program
                .functions
                .iter_mut()
                .find(|function| function.identity.declaration.name == "caller")
                .expect("caller"),
        );
        let (jett_hir::ExpressionKind::Call { args, .. }
        | jett_hir::ExpressionKind::IndirectCall { args, .. }) = &mut expression.kind
        else {
            unreachable!("returned_call_mut validates the expression kind");
        };
        args[1].ty = types.intern(Type::Secret(TypeInterner::INT64));
        args[1].kind = jett_hir::ExpressionKind::Int(7);

        let error = emit_host_object(&program, &types)
            .expect_err("impure calls cannot accept secret-lifted arguments");
        assert!(
            matches!(error, CodegenError::InvalidMir(ref errors)
                if errors.iter().any(|error| error.message == if callee == "callback" {
                    "call ownership converted operand differs from its physical parameter"
                } else { "call ownership indirect converted operand differs from its physical parameter" })),
            "{callee}: {error:?}"
        );
    }
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
fn emits_pending_nothing_through_parameters_returns_and_the_entry_wrapper() {
    let (program, types) = lower_source(
        r#"namespace app
function empty() returns nothing:
    return nothing
function forward(value: nothing) returns nothing:
    return value
function main() returns nothing:
    nothing pending = run forward(empty())
    nothing forwarded = forward(pending)
    nothing joined = join forwarded handle error:
        return nothing
    bool equal = joined == nothing
    bool different = joined != nothing
    trace pending
    println(joined)
"#,
    );
    let entry = program.functions.last().expect("main function").id;
    let artifact = emit_host_program_object(&program, &types, entry)
        .expect("pending nothing must retain its runtime depth through calls");
    let object = object::File::parse(artifact.bytes.as_slice()).expect("parse task object");
    let undefined = object
        .symbols()
        .filter(|symbol| symbol.is_undefined())
        .filter_map(|symbol| symbol.name().ok())
        .collect::<Vec<_>>();
    for leaf in [
        jett_runtime::native_abi::values::NativeLeaf::NothingRun,
        jett_runtime::native_abi::values::NativeLeaf::NothingJoin,
        jett_runtime::native_abi::values::NativeLeaf::NothingFormat,
        jett_runtime::native_abi::values::NativeLeaf::NothingEqual,
    ] {
        assert!(
            undefined.contains(&leaf.symbol()),
            "missing typed task leaf {}",
            leaf.symbol()
        );
    }
    assert_eq!(
        artifact.symbols.last().map(String::as_str),
        Some(JETT_AOT_ENTRY_SYMBOL_V1)
    );
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
        equality_methods: HashMap::new(),
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
        equality_methods: HashMap::new(),
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
        scoped_type_bindings: Vec::new(),
        type_arguments: vec![first_argument],
        specialization: CheckedGenericSpecialization::default(),
    };
    let second = FunctionIdentity {
        declaration,
        scoped_type_bindings: Vec::new(),
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

    let mut first_scoped = first.clone();
    first_scoped.scoped_type_bindings = vec![jett_hir::ScopedTypeBinding {
        name: "Field".into(),
        ty: first_argument,
        reflection: jett_types::ReflectionTypeInfo::new(
            "list[int64]",
            "list",
            None,
            false,
            Vec::new(),
        ),
    }];
    let mut second_scoped = second.clone();
    second_scoped.scoped_type_bindings = vec![jett_hir::ScopedTypeBinding {
        ty: second_argument,
        ..first_scoped.scoped_type_bindings[0].clone()
    }];
    assert_eq!(
        symbol_name(&first_scoped, &first_types).unwrap(),
        symbol_name(&second_scoped, &second_types).unwrap(),
    );
    assert_ne!(
        symbol_name(&first_scoped, &first_types).unwrap(),
        first_symbol
    );
    let mut aliased = first_scoped.clone();
    aliased.scoped_type_bindings[0].reflection = jett_types::ReflectionTypeInfo::new(
        "Items",
        "alias",
        None,
        false,
        vec![first_scoped.scoped_type_bindings[0].reflection.clone()],
    );
    assert_ne!(
        symbol_name(&first_scoped, &first_types).unwrap(),
        symbol_name(&aliased, &first_types).unwrap()
    );
    second_scoped.scoped_type_bindings[0].ty = TypeInterner::STRING;
    assert_ne!(
        symbol_name(&first_scoped, &first_types).unwrap(),
        symbol_name(&second_scoped, &second_types).unwrap(),
    );

    let specialized = FunctionIdentity {
        declaration: second.declaration.clone(),
        scoped_type_bindings: Vec::new(),
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
            reflection_identity: "4:bool0:0:0".to_string(),
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
fn rejects_unbaked_namespace_constants_before_object_generation() {
    let (program, types) = lower_source(
        "namespace app\nint64 answer = 42\nfunction main() returns int64:\n    return answer\n",
    );
    let error = emit_host_object(&program, &types)
        .expect_err("constant reads require the compiler-produced value table");
    assert!(
        error.to_string().contains("unbaked namespace constant"),
        "{error}"
    );
}

#[test]
fn rejects_secret_binary_results_and_wrappers_that_remove_taint() {
    for expression in ["hidden == plain", "clone hidden"] {
        let source = format!(
            "namespace app\nfunction compare(hidden: secret[bool], plain: bool) returns secret[bool]:\n    return {expression}\n"
        );
        let (mut program, types) = lower_source(&source);
        emit_host_object(&program, &types).expect("checked secret binary or wrapper");
        let function = &mut program.functions[0];
        function.return_type = TypeInterner::BOOL;
        let entry = function.entry.index() as usize;
        let TerminatorKind::Return(Some(value)) = &mut function.blocks[entry].terminator.kind
        else {
            panic!("expected returned expression");
        };
        value.ty = TypeInterner::BOOL;
        let error = emit_host_object(&program, &types)
            .expect_err("a compiler handoff cannot silently declassify");
        assert!(
            matches!(error, CodegenError::InvalidMirContract { ref message, .. }
                if message.contains("secret result") || message.contains("wrapper")),
            "{expression}: {error:?}"
        );
    }
}

#[test]
fn wrapped_enum_equality_rejects_missing_exact_payload_methods() {
    let (mut program, types) = lower_source_with_equatable(
        "namespace app\nstruct Item:\n    id: int64\nimplement Equatable for Item:\n    function equals(view self: Item, view other: Item) returns bool:\n        return self.id == other.id\nenum Choice:\n    item(value: Item)\nfunction compare(view hidden: secret[Choice], view plain: Choice) returns secret[bool]:\n    return hidden == plain\n",
        true,
    );
    let &method = program.equality_methods.values().next().unwrap();
    program.functions[method.index() as usize]
        .identity
        .declaration
        .origin = SourceOrigin::Stdlib;
    let symbol = symbol_name(&program.functions[method.index() as usize].identity, &types).unwrap();
    assert!(
        emit_host_object(&program, &types)
            .expect("checked wrapped equality")
            .symbols
            .contains(&symbol),
        "wrapped enum payload methods remain reachable"
    );
    program.equality_methods.clear();
    let error = emit_host_object(&program, &types)
        .expect_err("secrecy must not bypass explicit payload equality");
    assert!(
        error.to_string().contains("enum equality payload type"),
        "{error}"
    );
}

#[test]
fn emits_exact_enum_payload_method_and_rejects_broken_handoff() {
    let (mut program, types) = lower_source_with_equatable(
        r#"namespace app
struct Item:
    id: int64
implement Equatable for Item:
    function equals(view self: Item, view other: Item) returns bool:
        return self.id == other.id
enum Value:
    empty
    item(value: Item)
function main() returns bool:
    Value left = Value.item(Item(id: 1))
    Value right = Value.item(Item(id: 1))
    return left == right
"#,
        true,
    );
    let (&owner, &method) = program
        .equality_methods
        .iter()
        .next()
        .expect("checked exact target");
    assert_eq!(program.equality_methods.len(), 1);
    program.functions[method.index() as usize]
        .identity
        .declaration
        .origin = SourceOrigin::Stdlib;
    let expected_symbol =
        symbol_name(&program.functions[method.index() as usize].identity, &types).unwrap();
    let first = emit_host_object(&program, &types).expect("checked enum method emission");
    assert!(
        first.symbols.contains(&expected_symbol),
        "implicitly called stdlib methods remain reachable"
    );
    assert_eq!(first, emit_host_object(&program, &types).unwrap());
    for broken in 0..3 {
        let mut invalid = program.clone();
        match broken {
            0 => {
                invalid.functions[method.index() as usize].params[0].mode =
                    jett_mir::ParamMode::Owned
            }
            1 => invalid.functions[method.index() as usize].return_type = TypeInterner::INT64,
            _ => {
                invalid.equality_methods.insert(TypeInterner::BOOL, method);
            }
        }
        assert!(
            emit_host_object(&invalid, &types).is_err(),
            "invalid handoff {broken} must fail"
        );
    }
    program.equality_methods.remove(&owner);
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
fn secret_field_reads_preserve_exact_qualification_and_nominal_contracts() {
    use jett_hir::{ExpressionKind, FieldId};
    let (baseline, mut types) = lower_source(
        r#"namespace app
type Hidden = secret[int64] where true
struct Item:
    number: int64
    text: string
    hidden: secret[int64]
    nominal: Hidden
    unit: nothing
struct Other:
    number: int64
function read(view item: secret[Item]) returns secret[int64]:
    return item.number
function text(view item: secret[Item]) returns secret[string]:
    return item.text
function already(view item: secret[Item]) returns secret[int64]:
    return item.hidden
function nominal(view item: secret[Item]) returns Hidden:
    return item.nominal
function unit(view item: secret[Item]) returns nothing:
    return item.unit
function public_read(view item: Item) returns int64:
    return item.number
"#,
    );
    emit_host_object(&baseline, &types).expect("exact source-qualified field reads");
    let secret_int = types.intern(Type::Secret(TypeInterner::INT64));
    let twice_secret_int = types.intern(Type::Secret(secret_int));
    let secret_nothing = types.intern(Type::Secret(TypeInterner::NOTHING));
    let other = types
        .type_ids()
        .find(|ty| {
            matches!(types.resolve(*ty), Type::Struct(id)
            if types.resolve_struct(*id).name == "app.Other")
        })
        .unwrap();
    for (name, corruption) in [
        ("read", "declassify"),
        ("public_read", "classify"),
        ("already", "double"),
        ("nominal", "erase nominal"),
        ("unit", "qualify nothing"),
        ("read", "different owner"),
        ("read", "invalid index"),
        ("read", "nested owner"),
    ] {
        let mut program = baseline.clone();
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == name)
            .unwrap();
        if corruption == "nested owner" {
            let nested = types.intern(Type::Secret(function.params[0].ty));
            function.params[0].ty = nested;
            function.locals[function.params[0].local.index() as usize].ty = nested;
        }
        let TerminatorKind::Return(Some(value)) = &mut function.blocks
            [function.entry.index() as usize]
            .terminator
            .kind
        else {
            panic!("field return");
        };
        let ExpressionKind::Field {
            base,
            owner_type,
            field,
        } = &mut value.kind
        else {
            panic!("nominal field read");
        };
        match corruption {
            "declassify" => value.ty = TypeInterner::INT64,
            "classify" | "erase nominal" => value.ty = secret_int,
            "double" => value.ty = twice_secret_int,
            "qualify nothing" => value.ty = secret_nothing,
            "different owner" => *owner_type = other,
            "invalid index" => *field = FieldId::new(99),
            "nested owner" => base.ty = function.params[0].ty,
            _ => unreachable!(),
        }
        function.return_type = value.ty;
        let error = emit_host_object(&program, &types).expect_err("invalid field metadata");
        assert!(
            matches!(error, CodegenError::InvalidMirContract { ref message, .. }
                if message.contains("field")),
            "{corruption}: {error:?}"
        );
    }
}

#[test]
fn qualified_struct_constructors_keep_strict_nominal_contracts() {
    use jett_hir::ExpressionKind;
    let (baseline, types) = lower_source(
        "namespace app\nstruct Item:\n    number: int64\nstruct Other:\n    number: int64\nfunction make() returns secret[Item]:\n    return Item(number: 7)\n",
    );
    emit_host_object(&baseline, &types).expect("qualified nominal struct construction");
    let other = types
        .type_ids()
        .find(|ty| {
            matches!(types.resolve(*ty), Type::Struct(id)
            if types.resolve_struct(*id).name == "app.Other")
        })
        .unwrap();
    for corruption in 0..4 {
        let mut program = baseline.clone();
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "make")
            .unwrap();
        let TerminatorKind::Return(Some(value)) = &mut function.blocks
            [function.entry.index() as usize]
            .terminator
            .kind
        else {
            panic!("qualified constructor return");
        };
        let ExpressionKind::InterfaceCoerce { value: inner, .. } = &mut value.kind else {
            panic!("separate checked secret qualification");
        };
        if corruption == 0 {
            value.kind = inner.kind.clone();
        } else {
            let ExpressionKind::StructConstruct {
                struct_type,
                validates_refinements,
                ..
            } = &mut inner.kind
            else {
                panic!("exact nominal constructor");
            };
            match corruption {
                1 => inner.ty = value.ty,
                2 => *struct_type = other,
                3 => *validates_refinements = true,
                _ => unreachable!(),
            }
        }
        let error = emit_host_object(&program, &types).expect_err("invalid nominal constructor");
        assert!(
            matches!(error, CodegenError::InvalidMirContract { ref message, .. }
                if message.contains("struct")),
            "corruption {corruption}: {error:?}"
        );
    }
}

#[test]
fn qualified_validating_struct_constructors_keep_exact_result_contracts() {
    use jett_hir::ExpressionKind;
    let (baseline, mut types) = lower_source(
        r#"namespace app
type Positive = int64 where value > 0
struct Item:
    number: Positive
struct Other:
    number: Positive
function make(value: Positive) returns secret[result[Item, string]]:
    return Item(number: value)
"#,
    );
    emit_host_object(&baseline, &types)
        .expect("qualified validating struct without new predicates");
    let item = types
        .type_ids()
        .find(|ty| {
            matches!(types.resolve(*ty), Type::Struct(id)
            if types.resolve_struct(*id).name == "app.Item")
        })
        .unwrap();
    let other = types
        .type_ids()
        .find(|ty| {
            matches!(types.resolve(*ty), Type::Struct(id)
            if types.resolve_struct(*id).name == "app.Other")
        })
        .unwrap();
    let bad_error = types.intern(Type::Result(item, TypeInterner::BOOL));
    let bad_success = types.intern(Type::Result(other, TypeInterner::STRING));
    for corruption in 0..7 {
        let mut program = baseline.clone();
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "make")
            .unwrap();
        let TerminatorKind::Return(Some(value)) = &mut function.blocks
            [function.entry.index() as usize]
            .terminator
            .kind
        else {
            panic!("validating constructor return");
        };
        let ExpressionKind::InterfaceCoerce { value: inner, .. } = &mut value.kind else {
            panic!("result qualification boundary");
        };
        if corruption == 0 {
            value.kind = inner.kind.clone();
        } else {
            let ExpressionKind::StructConstruct {
                struct_type,
                validates_refinements,
                ..
            } = &mut inner.kind
            else {
                panic!("empty predicate chain remains a validating constructor");
            };
            match corruption {
                1 => inner.ty = value.ty,
                2 => *struct_type = other,
                3 => *validates_refinements = false,
                4 => inner.ty = bad_error,
                5 => inner.ty = bad_success,
                6 => inner.ty = item,
                _ => unreachable!(),
            }
        }
        let error =
            emit_host_object(&program, &types).expect_err("invalid validating struct result");
        assert!(
            matches!(error, CodegenError::InvalidMirContract { ref message, .. }
                if message.contains("struct")),
            "corruption {corruption}: {error:?}"
        );
    }
}

#[test]
fn qualified_bitfield_and_machine_constructors_keep_exact_contracts() {
    use jett_hir::{Expression, ExpressionKind, StateId};
    let (baseline, mut types) = lower_source(
        r#"namespace app
bitfield Header:
    value: 4 bits
bitfield Other:
    value: 4 bits
machine Session:
    states:
        active(label: string, count: int8)
        closed
    transitions:
        active to closed
machine Another:
    states:
        active(label: string, count: int8)
        closed
    transitions:
        active to closed
function plain() returns secret[Header]:
    return Header(value: 7)
function checked(value: int64) returns secret[result[Header, string]]:
    return Header(value: value)
function state() returns secret[Session at active]:
    return Session(active, "value", 7)
function another() returns Another at active:
    return Another(active, "value", 7)
"#,
    );
    emit_host_object(&baseline, &types).expect("exact qualified bitfield and machine constructors");
    let header = types
        .type_ids()
        .find(|ty| {
            matches!(types.resolve(*ty), Type::Bitfield(id)
            if types.resolve_bitfield(*id).name == "app.Header")
        })
        .unwrap();
    let other = types
        .type_ids()
        .find(|ty| {
            matches!(types.resolve(*ty), Type::Bitfield(id)
            if types.resolve_bitfield(*id).name == "app.Other")
        })
        .unwrap();
    let another_state = types
        .type_ids()
        .find(|ty| {
            matches!(types.resolve(*ty), Type::MachineState { machine, .. }
            if types.resolve_machine(*machine).name == "app.Another")
        })
        .unwrap();
    let bad_error = types.intern(Type::Result(header, TypeInterner::BOOL));
    let bad_success = types.intern(Type::Result(other, TypeInterner::STRING));
    let qualified_header = types.intern(Type::Secret(header));
    for (name, corruption) in [
        ("plain", "raw qualified constructor"),
        ("plain", "qualified inner"),
        ("plain", "different bitfield"),
        ("plain", "flipped validation"),
        ("plain", "wrong field type"),
        ("checked", "raw qualified constructor"),
        ("checked", "qualified inner"),
        ("checked", "different bitfield"),
        ("checked", "flipped validation"),
        ("checked", "wrong result error"),
        ("checked", "wrong result success"),
        ("checked", "erased result"),
        ("checked", "qualified bitfield target"),
        ("state", "raw qualified constructor"),
        ("state", "qualified inner"),
        ("state", "different state owner"),
        ("state", "wrong state index"),
        ("state", "missing payload"),
        ("state", "swapped payloads"),
    ] {
        let mut program = baseline.clone();
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == name)
            .unwrap();
        let TerminatorKind::Return(Some(value)) = &mut function.blocks
            [function.entry.index() as usize]
            .terminator
            .kind
        else {
            panic!("constructor return");
        };
        let ExpressionKind::InterfaceCoerce { value: inner, .. } = &mut value.kind else {
            panic!("separate qualification coercion");
        };
        if corruption == "raw qualified constructor" {
            value.kind = inner.kind.clone();
        } else if corruption == "qualified inner" {
            inner.ty = value.ty;
        } else {
            match &mut inner.kind {
                ExpressionKind::BitfieldConstruct {
                    bitfield_type,
                    fields,
                    validates_widths,
                    ..
                } => match corruption {
                    "different bitfield" => *bitfield_type = other,
                    "flipped validation" => *validates_widths = !*validates_widths,
                    "wrong field type" => {
                        fields[0] = Expression {
                            kind: ExpressionKind::Bool(true),
                            ty: TypeInterner::BOOL,
                            span: fields[0].span,
                        };
                    }
                    "wrong result error" => inner.ty = bad_error,
                    "wrong result success" => inner.ty = bad_success,
                    "erased result" => inner.ty = header,
                    "qualified bitfield target" => *bitfield_type = qualified_header,
                    _ => unreachable!(),
                },
                ExpressionKind::MachineConstruct {
                    state_type,
                    state,
                    payloads,
                } => match corruption {
                    "different state owner" => *state_type = another_state,
                    "wrong state index" => *state = StateId::new(state.index() + 1),
                    "missing payload" => {
                        payloads.pop();
                    }
                    "swapped payloads" => payloads.swap(0, 1),
                    _ => unreachable!(),
                },
                _ => panic!("exact inner constructor"),
            }
        }
        let error = emit_host_object(&program, &types).expect_err("invalid constructor metadata");
        assert!(
            matches!(error, CodegenError::InvalidMirContract { ref message, .. }
                if message.contains("bitfield") || message.contains("machine")),
            "{name}/{corruption}: {error:?}"
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

#[test]
fn native_reflected_getter_pipelines_emit_checked_root_and_owned_reads() {
    let (program, types) = lower_source(
        r#"namespace app
type Positive = int64 where value > 0
type Higher = Positive where value > 10
struct Record:
    raw: int64
    positive: Positive
    higher: Higher
enum Event:
    first(raw: int64, higher: Higher)
    second(higher: Higher, raw: int64)
machine Session:
    states:
        first(raw: int64, higher: Higher)
        second(higher: Higher, raw: int64)
    transitions:
        first to second
bitfield Header:
    first: 4 bits
    second: 8 bits
struct Parcel:
    text: string
    values: list[int64]
enum Packet:
    first(text: string)
    second(values: list[int64])
machine Store:
    states:
        first(text: string)
        second(values: list[int64])
    transitions:
        first to second
function record_pipe(view source: Record, view field: TypeField) returns Higher:
    return source into view type.field_value[Record, Higher](view field)
function event_pipe(view source: Event, view field: TypeField) returns Higher:
    return source into view type.variant_field_value[Event, Higher](view field)
function session_pipe(view source: Session, view field: TypeField) returns Higher:
    return source into view type.machine_field_value[Session, Higher](view field)
function narrowed_pipe(view source: Session at first, view field: TypeField) returns Higher:
    return source into view type.machine_field_value[Session at first, Higher](view field)
function header_pipe(view source: Header, view field: TypeField) returns int64:
    return source into view type.field_value[Header, int64](view field)
function parcel_text(view source: Parcel, view field: TypeField) returns string:
    return source into view type.field_value[Parcel, string](view field)
function parcel_values(view source: Parcel, view field: TypeField) returns list[int64]:
    return source into view type.field_value[Parcel, list[int64]](view field)
function packet_text(view source: Packet, view field: TypeField) returns string:
    return source into view type.variant_field_value[Packet, string](view field)
function packet_values(view source: Packet, view field: TypeField) returns list[int64]:
    return source into view type.variant_field_value[Packet, list[int64]](view field)
function store_text(view source: Store, view field: TypeField) returns string:
    return source into view type.machine_field_value[Store, string](view field)
function store_values(view source: Store, view field: TypeField) returns list[int64]:
    return source into view type.machine_field_value[Store, list[int64]](view field)
"#,
    );
    let artifact = emit_host_object(&program, &types).expect("checked reflected getter pipelines");
    let object = object::File::parse(artifact.bytes.as_slice()).unwrap();
    let failure = jett_runtime::native_abi::values::NativeLeaf::RuntimeFailMessage.symbol();
    assert!(object.symbols().any(|s| s.name().ok() == Some(failure)));
}

const REFLECTED_VALUE_PIPELINES: &str = r#"namespace app
struct Holder[T]:
    value: T
type Positive = int64 where value > 0
enum Event:
    empty = 17
    content(text: string, values: list[optional[int64]])
machine Session:
    states:
        empty
        content(text: string, values: list[optional[int64]])
    transitions:
        empty to content
function variant_pipe(view source: Event) returns TypeVariant:
    TypeVariant selected = source into view type.variant_value[Event]()
    return selected
function state_pipe(view source: Session) returns TypeMachineState:
    TypeMachineState selected = source into view type.machine_state_value[Session]()
    return selected
function narrowed_pipe(view source: Session at content) returns TypeMachineState:
    TypeMachineState selected = source into view type.machine_state_value[Session at content]()
    return selected
function map_pipe(index: int64) returns TypeInfo:
    TypeInfo selected = index into type.arg[map[string, list[optional[int64]]]]()
    return selected
function holder_pipe(index: int64) returns TypeInfo:
    return index into type.arg[Holder[list[optional[int64]]]]()
function refinement_pipe(index: int64) returns TypeInfo:
    return index into type.arg[Positive]()
function primitive_pipe(index: int64) returns TypeInfo:
    return index into type.arg[int64]()
"#;

#[test]
fn native_reflected_value_pipelines_emit_owned_metadata_and_existing_observation_guards() {
    let (program, types) = lower_source(REFLECTED_VALUE_PIPELINES);
    let artifact = emit_host_object(&program, &types).expect("checked reflected value pipelines");
    let object = object::File::parse(artifact.bytes.as_slice()).unwrap();
    for leaf in [
        jett_runtime::native_abi::values::NativeLeaf::TypeArgCheckedIndex,
        jett_runtime::native_abi::values::NativeLeaf::ReflectedOwnerPendingCheck,
        jett_runtime::native_abi::values::NativeLeaf::StructClone,
    ] {
        assert!(
            object
                .symbols()
                .any(|symbol| symbol.name().ok() == Some(leaf.symbol())),
            "{} must retain its existing checked ownership/observation ABI",
            leaf.symbol()
        );
    }
}

#[test]
fn native_reflected_value_pipelines_reject_missing_and_wrong_typed_metadata() {
    for name in ["variant_pipe", "state_pipe", "narrowed_pipe", "map_pipe"] {
        for remove in [true, false] {
            let (mut program, types) = lower_source(REFLECTED_VALUE_PIPELINES);
            let function = program
                .functions
                .iter_mut()
                .find(|f| f.identity.declaration.name == name)
                .unwrap();
            let value = function
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.statements)
                .find_map(|statement| match &mut statement.kind {
                    jett_mir::StatementKind::Let { value, .. }
                        if matches!(value.kind, jett_hir::ExpressionKind::Intrinsic { .. }) =>
                    {
                        Some(value)
                    }
                    _ => None,
                })
                .expect("lowered observer initializer");
            let jett_hir::ExpressionKind::Intrinsic {
                args,
                evaluation_order,
                ..
            } = &mut value.kind
            else {
                panic!("observer intrinsic");
            };
            assert!(args.len() > 1, "{name}: checked hidden metadata");
            if remove {
                args.pop();
                evaluation_order.pop();
            } else {
                let metadata = args.last_mut().unwrap();
                metadata.kind = jett_hir::ExpressionKind::Bool(false);
                metadata.ty = TypeInterner::BOOL;
            }
            let rejection = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                emit_host_object(&program, &types)
            }));
            assert!(
                matches!(rejection, Ok(Err(CodegenError::InvalidMir(ref errors)))
                    if errors.iter().any(|error| error.message == if remove {
                        "call ownership operand count is invalid"
                    } else { "generated operand has no checked actual/conversion type" })),
                "{name}, remove={remove}: malformed metadata must be rejected without panicking: {rejection:?}"
            );
        }
    }
}

const MACHINE_REBINDING: &str = r#"namespace app
machine Session:
    states:
        empty
        content(values: list[int64])
    transitions:
        empty to content
machine Other:
    states:
        empty
        content(values: list[int64])
    transitions:
        empty to content
function other() returns Other at empty:
    return Other(empty)
function replace(values: list[int64]) returns Session:
    mutable Session source = Session(empty)
    source = Session(content, values)
    source = Session(empty)
    return source
function copied(view source: Session at content) returns Session:
    mutable Session destination = Session(empty)
    destination = clone source
    return destination
function pending() returns Session:
    mutable Session source = run Session(empty)
    source = run run Session(content, list(7))
    return source
function same_guard() returns Session:
    mutable Session source = Session(content, list(1))
    if source at content:
        source = Session(content, list(2))
    return source
"#;

#[test]
fn native_machine_rebinding_emits_whole_owner_storage_and_owned_cleanup() {
    let (program, types) = lower_source(MACHINE_REBINDING);
    for name in ["replace", "copied", "pending", "same_guard"] {
        let function = program
            .functions
            .iter()
            .find(|f| f.identity.declaration.name == name)
            .unwrap();
        let local = function.locals.iter().find(|local| local.mutable).unwrap();
        assert!(
            matches!(types.resolve(local.ty), Type::Machine(_)),
            "{name}: bare annotation owns the whole machine"
        );
    }
    let artifact = emit_host_object(&program, &types).expect("checked whole-machine rebinding");
    let object = object::File::parse(artifact.bytes.as_slice()).unwrap();
    let drop = jett_runtime::native_abi::values::NativeLeaf::DropValue.symbol();
    assert!(
        object
            .symbols()
            .any(|symbol| symbol.name().ok() == Some(drop)),
        "replaced owned payloads retain cleanup"
    );
}

#[test]
fn native_machine_rebinding_rejects_foreign_owner_and_forged_exact_targets() {
    for mutation in ["foreign_owner", "explicit_target", "guarded_target"] {
        let (mut program, types) = lower_source(MACHINE_REBINDING);
        let foreign = program
            .functions
            .iter()
            .find(|f| f.identity.declaration.name == "other")
            .unwrap()
            .return_type;
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.identity.declaration.name == "replace")
            .unwrap();
        let initial = function
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .find_map(|statement| match &statement.kind {
                jett_mir::StatementKind::Let { local, value }
                    if matches!(
                        value.kind,
                        jett_hir::ExpressionKind::MachineConstruct { .. }
                    ) =>
                {
                    Some((*local, value.ty))
                }
                _ => None,
            })
            .unwrap();
        if mutation == "explicit_target" {
            function.locals[initial.0.index() as usize].ty = initial.1;
        }
        let (target, value) = function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.statements)
            .find_map(|statement| match &mut statement.kind {
                jett_mir::StatementKind::Assign { target, value } => Some((target, value)),
                _ => None,
            })
            .unwrap();
        if mutation == "foreign_owner" {
            value.kind = jett_hir::ExpressionKind::MachineConstruct {
                state_type: foreign,
                state: jett_hir::StateId::new(0),
                payloads: vec![],
            };
            value.ty = foreign;
        } else if mutation == "guarded_target" {
            // The storage remains the full machine; only this checked target
            // is exact, as it is inside a visible source-level state guard.
            target.ty = initial.1;
        }
        let error = emit_host_object(&program, &types).expect_err("forged machine assignment");
        let expected = if mutation == "guarded_target" {
            "exact machine target"
        } else {
            "match its local"
        };
        assert!(
            matches!(error, CodegenError::InvalidMirContract { ref message, .. } if message.contains(expected)),
            "{mutation}: {error:?}"
        );
    }
}

const RECURSIVE_REFLECTED_SOURCE: &str = r#"namespace app
type Positive = int64 where value > 0
struct Record:
    values: list[optional[result[map[int64, list[int64]], set[int64]]]]
function inspect(view source: Record, view field: TypeField) returns list[optional[result[map[Positive, list[Positive]], set[Positive]]]]:
    return source into view type.field_value[Record, list[optional[result[map[Positive, list[Positive]], set[Positive]]]]](view field)
"#;

#[test]
fn native_recursive_reflected_producers_emit_existing_projection_and_failure_leaves() {
    let (program, types) = lower_source(RECURSIVE_REFLECTED_SOURCE);
    let artifact = emit_host_object(&program, &types).expect("recursive reflected checked CFG");
    let object = object::File::parse(artifact.bytes.as_slice()).unwrap();
    for leaf in [
        jett_runtime::native_abi::values::NativeLeaf::RejectPendingHandle,
        jett_runtime::native_abi::values::NativeLeaf::ListElementClone,
        jett_runtime::native_abi::values::NativeLeaf::SetElementClone,
        jett_runtime::native_abi::values::NativeLeaf::MapKeyClone,
        jett_runtime::native_abi::values::NativeLeaf::MapValueClone,
        jett_runtime::native_abi::values::NativeLeaf::SumClone,
        jett_runtime::native_abi::values::NativeLeaf::SumTake,
        jett_runtime::native_abi::values::NativeLeaf::RuntimeFailMessage,
    ] {
        assert!(
            object
                .symbols()
                .any(|symbol| symbol.name().ok() == Some(leaf.symbol())),
            "{} must use the existing native ABI",
            leaf.symbol()
        );
    }
}

#[test]
fn native_recursive_reflected_readiness_rejects_wrong_kind_and_missing_source() {
    for corruption in [0, 1, 2] {
        let (mut program, types) = lower_source(RECURSIVE_REFLECTED_SOURCE);
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.identity.declaration.name == "inspect")
            .unwrap();
        let scalar = function
            .locals
            .iter()
            .find(|local| local.ty == TypeInterner::BOOL)
            .unwrap()
            .id;
        let statement = function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.statements)
            .find(|statement| {
                matches!(
                    statement.kind,
                    jett_mir::StatementKind::ReflectedContainerReady { .. }
                )
            })
            .expect("typed reflected readiness");
        let jett_mir::StatementKind::ReflectedContainerReady { source, kind } = &mut statement.kind
        else {
            unreachable!()
        };
        match corruption {
            0 => *kind = jett_mir::ReflectedContainerKind::Result,
            1 => *source = jett_hir::LocalId::new(u32::MAX),
            2 => *source = scalar,
            _ => unreachable!(),
        }
        let rejection = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            emit_host_object(&program, &types)
        }));
        let expected = if corruption == 1 {
            matches!(rejection, Ok(Err(CodegenError::InvalidMir(_))))
        } else {
            matches!(rejection, Ok(Err(CodegenError::InvalidMirContract { .. })))
        };
        assert!(
            expected,
            "readiness corruption {corruption} must be rejected without panicking: {rejection:?}"
        );
    }
}

#[test]
fn native_recursive_reflected_new_root_keeps_original_declared_base_preflight() {
    let (program, types) = lower_source(
        r#"namespace app
type Positive = int64 where value > 0
type Values = list[Positive] where true
type Independent = list[Positive] where true
struct Record:
    values: Values
function inspect(view source: Record, view field: TypeField) returns Independent:
    return source into view type.field_value[Record, Independent](view field)
"#,
    );
    let artifact = emit_host_object(&program, &types).expect("original declared root base CFG");
    let object = object::File::parse(artifact.bytes.as_slice()).unwrap();
    assert!(object.symbols().any(|symbol| {
        symbol.name().ok()
            == Some(jett_runtime::native_abi::values::NativeLeaf::RejectPendingHandle.symbol())
    }));
    let function = program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "inspect")
        .unwrap();
    let checks: Vec<_> = function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .filter_map(|statement| match &statement.kind {
            jett_mir::StatementKind::CheckRefinement { type_name, .. } => Some(type_name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(checks, ["app.Independent"]);
}
