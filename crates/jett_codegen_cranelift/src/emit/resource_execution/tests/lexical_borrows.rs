//! Public object admission must not infer lexical custody from editable MIR.
use super::*;

const SOURCES: [(&str, &str); 3] = [
    (
        "Source58 if",
        include_str!(
            "../../../../../jett_driver/tests/native_conformance/resource/58_if_optional_written_some.jett"
        ),
    ),
    (
        "Source74 while",
        include_str!(
            "../../../../../jett_driver/tests/native_conformance/resource/74_while_optional_written_some.jett"
        ),
    ),
    (
        "Source89 sibling scopes",
        include_str!(
            "../../../../../jett_driver/tests/native_conformance/resource/89_scope_sibling_result_bindings.jett"
        ),
    ),
];

#[derive(Debug, Clone, Copy)]
enum Mutation {
    CopiedExit,
    RetargetedExit,
    MissingWitness,
}

fn public_object(
    program: &Program,
    checked: &Arc<CheckedResourceProgram>,
    release: bool,
) -> Result<ObjectArtifact, CodegenError> {
    crate::emit_host_program_object_with_options(
        program,
        &checked.checked().interner,
        entry(program),
        CodegenOptions { optimize: release },
    )
}

/// Construct an ordinary function through public lowering, then copy only the
/// editable MIR fields. The private Resource witness and other private records
/// stay absent; no test-only authority constructor or representation cast is used.
fn without_resource_witness(
    original: &jett_mir::Function,
    checked: &Arc<CheckedResourceProgram>,
) -> jett_mir::Function {
    let hir = jett_hir::lower_checked_resource_program(checked).unwrap();
    let mut seed = hir.functions[original.id.index() as usize].clone();
    seed.id = FunctionId::new(0);
    seed.params.clear();
    seed.capture_count = 0;
    seed.return_type = TypeInterner::NOTHING;
    seed.locals.clear();
    seed.body.statements.clear();
    let ordinary = jett_hir::Program {
        resource_manifest: jett_hir::ResourceManifest::empty(),
        resource_source: jett_hir::ResourceSourceArchive::empty(),
        functions: vec![seed],
        equality_methods: HashMap::new(),
    };
    let mut lowered = jett_mir::lower(&ordinary, &checked.checked().interner).unwrap();
    let mut unwitnessed = lowered.functions.remove(0);
    unwitnessed.id = original.id;
    unwitnessed.identity = original.identity.clone();
    unwitnessed.debug_kind = original.debug_kind.clone();
    unwitnessed.params = original.params.clone();
    unwitnessed.capture_count = original.capture_count;
    unwitnessed.return_type = original.return_type;
    unwitnessed.locals = original.locals.clone();
    unwitnessed.entry = original.entry;
    unwitnessed.blocks = original.blocks.clone();
    unwitnessed.span = original.span;
    unwitnessed
}

#[test]
fn resource_lexical_borrows_public_object_refuses_copied_retargeted_and_unwitnessed_exits() {
    for release in [false, true] {
        for (label, source) in SOURCES {
            let checked = checked(source, release);
            let baseline = lower_source(&checked);
            // Establish that this exact checked Source reaches public object
            // emission before attributing any refusal to the mutation.
            let artifact = public_object(&baseline, &checked, release)
                .unwrap_or_else(|error| panic!("{label} release={release} baseline: {error:?}"));
            let (definitions, imports) = symbols(&artifact);
            assert!(
                definitions
                    .iter()
                    .any(|name| name == JETT_AOT_ENTRY_SYMBOL_V1)
            );
            assert!(
                imports
                    .iter()
                    .any(|name| name == "jett_rt_v1_resource_borrow_end")
            );

            let (function_id, block_id, index) = baseline
                .functions
                .iter()
                .find_map(|function| {
                    if function.identity.declaration.namespace != "app" {
                        return None;
                    }
                    function.blocks.iter().find_map(|block| {
                        block
                            .statements
                            .iter()
                            .enumerate()
                            .find_map(|(index, statement)| {
                                matches!(statement.kind, StatementKind::ResourceLexicalExit(_))
                                    .then_some((function.id, block.id, index))
                            })
                    })
                })
                .expect("genuine fixture has an app lexical-exit node");
            let position = jett_mir::ResourcePosition::Statement(index);
            let helper = &baseline.functions[function_id.index() as usize];
            assert!(
                helper
                    .resource_lexical_exit(block_id, position)
                    .unwrap()
                    .is_some()
            );
            assert_ne!(
                (block_id, index),
                (helper.entry, 0),
                "retarget must change the site"
            );

            for mutation in [
                Mutation::CopiedExit,
                Mutation::RetargetedExit,
                Mutation::MissingWitness,
            ] {
                let mut forged = baseline.clone();
                let helper = &mut forged.functions[function_id.index() as usize];
                match mutation {
                    Mutation::CopiedExit => {
                        let copied =
                            helper.blocks[block_id.index() as usize].statements[index].clone();
                        helper.blocks[block_id.index() as usize]
                            .statements
                            .insert(index, copied);
                    }
                    Mutation::RetargetedExit => {
                        let moved = helper.blocks[block_id.index() as usize]
                            .statements
                            .remove(index);
                        helper.blocks[helper.entry.index() as usize]
                            .statements
                            .insert(0, moved);
                    }
                    Mutation::MissingWitness => {
                        *helper = without_resource_witness(helper, &checked);
                        assert!(
                            helper
                                .resource_lexical_exit(block_id, position)
                                .unwrap()
                                .is_none()
                        );
                    }
                }
                jett_mir::validate(&forged).unwrap_or_else(|errors|
                    panic!("{label} release={release} {mutation:?} must retain structural MIR: {errors:?}"));
                match public_object(&forged, &checked, release) {
                    Err(CodegenError::InvalidMir(errors)) => {
                        assert!(!errors.is_empty(), "{label} release={release} {mutation:?}");
                        if matches!(mutation, Mutation::MissingWitness) {
                            assert!(
                                errors.iter().any(|error| error.message.contains(
                                    "no initially authenticated Source constructor witness"
                                )),
                                "{label} release={release} {mutation:?}: {errors:?}"
                            );
                        }
                    }
                    Err(error) => panic!(
                        "{label} release={release} {mutation:?}: wrong public refusal {error:?}"
                    ),
                    Ok(_) => panic!(
                        "{label} release={release} {mutation:?}: forged MIR produced an object"
                    ),
                }
            }
        }
    }
}
