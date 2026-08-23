use jett_common::FileId;
use jett_parser::{ast::Module, parse};

fn parse_fixture(source: &str) -> Module {
    let result = parse(source, FileId::new(0));
    assert!(
        result.errors.is_empty(),
        "snapshot fixture must parse without diagnostics: {:#?}",
        result.errors
    );
    result.module
}

#[test]
fn snapshots_representative_declarations() {
    let module = parse_fixture(include_str!(
        "../../../tests/snapshots/ast_declarations.jett"
    ));
    insta::assert_debug_snapshot!("ast_declarations", module);
}

#[test]
fn snapshots_nested_control_flow() {
    let module = parse_fixture(include_str!(
        "../../../tests/snapshots/ast_control_flow.jett"
    ));
    insta::assert_debug_snapshot!("ast_control_flow", module);
}
