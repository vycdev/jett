//! Source-derived borrowed match payload transport and corruption controls.
use super::*;
use jett_diagnostics::Severity;

const SOURCE: &str = r#"namespace app
struct Payload:
    values: list[int64]
enum Choice:
    first(data: Payload)
    second(data: Payload)
    empty
enum OtherChoice:
    foreign(data: Payload)
function observe(view values: list[int64]) returns int64:
    return list.length[int64](view values)
function observe_payload(view data: Payload) returns int64:
    return observe(view data.values)
function take(data: Payload) returns int64:
    return observe(view data.values)
function inspect(view choice: Choice) returns int64:
    match choice:
        first(data):
            Payload alias = view data
            return observe_payload(view data) + observe_payload(view alias) + observe(view data.values)
        second(data):
            return observe_payload(view data)
        empty:
            return 0
function owned(choice: Choice) returns int64:
    match choice:
        first(data):
            return take(data)
        second(data):
            return take(data)
        empty:
            return 0
"#;

fn checked_source(source: &str) -> (Program, CheckResult) {
    let file = FileId::new(0);
    let parsed = jett_parser::parse(source, file);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let stdlib_file = FileId::new(jett_common::STDLIB_FILE_ID_START);
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let stdlib_source =
        std::fs::read_to_string(root.join("stdlib/list.jett")).expect("actual shipped list facade");
    let mut stdlib = jett_parser::parse(&stdlib_source, stdlib_file);
    assert!(stdlib.errors.is_empty(), "{:?}", stdlib.errors);
    stdlib.module.items.extend(parsed.module.items);
    let module = Module {
        items: stdlib.module.items,
        span: parsed.module.span,
    };
    let resolved = jett_resolve::resolve(&module);
    assert!(
        resolved
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Error),
        "{:?}",
        resolved.diagnostics
    );
    let checked = jett_typecheck::check(&module, &resolved);
    assert!(
        checked
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Error),
        "{:?}",
        checked.diagnostics
    );
    let origins = HashMap::from([
        (file, SourceOrigin::Project),
        (stdlib_file, SourceOrigin::Stdlib),
    ]);
    let program = lower(&module, &resolved, &checked, &origins).expect("checked match lowers");
    validate_backend_types(&program, &checked.interner).expect("genuine match transport");
    (program, checked)
}

fn inspect_index(program: &Program) -> usize {
    program
        .functions
        .iter()
        .position(|function| function.identity.declaration.name == "inspect")
        .expect("source inspect")
}

fn matched(function: &mut Function) -> (&mut Expression, &mut Vec<MatchArm>) {
    match &mut function.body.statements[0].kind {
        StatementKind::Match { scrutinee, arms } => (scrutinee, arms),
        _ => panic!("source match"),
    }
}

#[test]
fn borrowed_match_payload_calls_keep_direct_projected_and_alias_origins() {
    checked_source(SOURCE);
    checked_source(
        "namespace app\nenum Number:\n    number(value: int64)\nfunction identity(value: int64) returns int64:\n    return value\nfunction inspect(view number: Number) returns int64:\n    match number:\n        number(value):\n            return identity(value)\n",
    );
}

#[test]
fn borrowed_match_transport_preserves_transparent_scrutinee_views() {
    for (declaration, input, scrutinee) in [
        ("", "Choice", "(view choice)"),
        ("", "secret[Choice]", "declassify(view choice)"),
        (
            "type RefinedChoice = Choice where true\n",
            "RefinedChoice",
            "coarsen(view choice)",
        ),
    ] {
        let source = format!(
            "namespace app\nstruct Payload:\n    values: list[int64]\nenum Choice:\n    item(data: Payload)\n{declaration}function observe(view data: Payload) returns int64:\n    return list.length[int64](view data.values)\nfunction inspect(view choice: {input}) returns int64:\n    match {scrutinee}:\n        item(data):\n            return observe(view data)\n"
        );
        checked_source(&source);
    }
}

#[test]
fn changed_match_tag_slot_owner_or_scope_cannot_reuse_source_call_authority() {
    let (program, checked) = checked_source(SOURCE);
    let index = inspect_index(&program);
    let first = match &program.functions[index].body.statements[0].kind {
        StatementKind::Match { arms, .. } => arms[0].bindings[0],
        _ => panic!("source match"),
    };
    let foreign_enum = checked
        .interner
        .type_ids()
        .find(|ty| {
            matches!(checked.interner.resolve(*ty), Type::Enum(id)
            if checked.interner.resolve_enum(*id).name.ends_with("OtherChoice"))
        })
        .expect("different nominal enum");
    for case in 0..8 {
        let mut changed = program.clone();
        let function = &mut changed.functions[index];
        match case {
            0 => {
                let (_, arms) = matched(function);
                arms[0].variant = arms[1].variant;
            }
            1 => {
                let (_, arms) = matched(function);
                arms[0].bindings.clear();
            }
            2 => {
                let (_, arms) = matched(function);
                arms[0].bindings[0] = arms[1].bindings[0];
            }
            3 => {
                let (_, arms) = matched(function);
                arms[0].bindings.push(first);
            }
            4 => {
                let (scrutinee, _) = matched(function);
                scrutinee.ty = foreign_enum;
            }
            5 => {
                function.locals[first.index() as usize].ty = TypeInterner::INT64;
            }
            6 => {
                let (_, arms) = matched(function);
                let returned = arms[0].body.statements.pop().expect("source call");
                function.body.statements.push(returned);
            }
            7 => {
                let (_, arms) = matched(function);
                let returned = arms[0].body.statements.pop().expect("source call");
                arms[1].body.statements.push(returned);
            }
            _ => unreachable!(),
        }
        let errors = validate_backend_types(&changed, &checked.interner)
            .expect_err("changed match binding provenance");
        assert!(
            errors.iter().any(|error| error.message.contains("match")
                || error.message.contains("borrowed binding")
                || error.message.contains("binding metadata")),
            "{case}: {errors:?}"
        );
    }
}
