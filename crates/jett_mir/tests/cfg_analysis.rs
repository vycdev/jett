use std::collections::{HashMap, HashSet};

use jett_common::{FileId, SourceOrigin};
use jett_mir::ControlFlowGraph;

fn lower_source(source: &str) -> jett_mir::Program {
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
    jett_mir::lower(&hir).expect("MIR lowering")
}

#[test]
fn cfg_analysis_reports_deterministic_edges_predecessors_and_reachable_order() {
    let program = lower_source(
        r#"namespace app
function countdown(value: int64) returns int64:
    mutable int64 current = value
    while current > 0:
        if current == 1:
            break
        current = current - 1
    return current
"#,
    );
    let function = &program.functions[0];

    let cfg = ControlFlowGraph::analyze(function).expect("valid lowered CFG");
    let order = cfg.reverse_postorder();
    assert_eq!(order.first(), Some(&function.entry));
    assert_eq!(
        order.iter().copied().collect::<HashSet<_>>().len(),
        order.len(),
        "reachable blocks should appear once"
    );

    for block in order {
        for successor in cfg.successors(*block) {
            assert!(cfg.predecessors(*successor).contains(block));
        }
    }

    let loop_header = cfg.successors(function.entry)[0];
    assert!(
        cfg.predecessors(loop_header).len() >= 2,
        "loop header should have entry and back-edge predecessors"
    );
}
