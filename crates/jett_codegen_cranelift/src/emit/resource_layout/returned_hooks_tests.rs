use super::*;
const FACTORY: &str = include_str!(
    "../../../../jett_driver/tests/native_conformance/resource/27_returned_hook_factory.jett"
);
const BORROW: &str = include_str!(
    "../../../../jett_driver/tests/native_conformance/resource/28_returned_hook_borrow.jett"
);
const IMMEDIATE: &str = include_str!(
    "../../../../jett_driver/tests/native_conformance/resource/29_immediate_returned_hook_close.jett"
);
const ORDER: &str = include_str!(
    "../../../../jett_driver/tests/native_conformance/resource/30_immediate_returned_hook_factory.jett"
);
const CALLEE_FAILURE: &str = include_str!(
    "../../../../jett_driver/tests/native_conformance/resource/31_immediate_returned_hook_callee_failure.jett"
);
const ACTUAL_FAILURE: &str = include_str!(
    "../../../../jett_driver/tests/native_conformance/resource/32_immediate_returned_hook_actual_failure.jett"
);

const CLOSE: &str = include_str!(
    "../../../../jett_driver/tests/native_conformance/resource/23_returned_hook_close.jett"
);
const ALIAS: &str = include_str!(
    "../../../../jett_driver/tests/native_conformance/resource/24_returned_hook_alias.jett"
);
const RELAY: &str = include_str!(
    "../../../../jett_driver/tests/native_conformance/resource/25_returned_hook_relay.jett"
);
const UNUSED: &str = include_str!(
    "../../../../jett_driver/tests/native_conformance/resource/26_returned_hook_unused.jett"
);

#[test]
fn resource_returned_hook_layout_joins_return_shape_and_distinct_indirect_close_target() {
    for release in [false, true] {
        for source in [
            CLOSE,
            ALIAS,
            RELAY,
            UNUSED,
            FACTORY,
            BORROW,
            IMMEDIATE,
            ORDER,
            CALLEE_FAILURE,
            ACTUAL_FAILURE,
        ] {
            let checked = checked(source, release);
            let types = &checked.checked().interner;
            let program = lower_source(&checked);
            let layout =
                EmittedResourceLayout::from_program(&program, types, entry(&program)).unwrap();
            let mut rows = Rows::new(layout.plan()).unwrap();
            rows.populate().unwrap();
            let mut indirect = 0;
            for function in layout.plan().functions() {
                if let Some(hook) = function.descriptor_return() {
                    let signature = rows.function_signature(function.function()).unwrap();
                    let result = rows.signatures[signature as usize].result;
                    let hook_id = rows.hook(hook).unwrap();
                    assert_eq!(rows.shapes[result as usize], [10, hook_id]);
                    assert_eq!(function.return_type(), hook.function_type());
                    assert!(function.provisional_return().is_none());
                }
                for operation in function.operations() {
                    let Some(hook) = operation.indirect_hook_target() else {
                        continue;
                    };
                    indirect += 1;
                    let ordinal = layout
                        .operation(function.function(), operation.id())
                        .unwrap();
                    let row = &rows.operations[ordinal as usize].1;
                    assert_eq!(row.len(), 4);
                    assert_eq!(row[0], 15);
                    assert_eq!(&rows.hook_ids[row[1] as usize], hook);
                    assert_eq!(row[2], rows.hooks[row[1] as usize].2);
                    assert_ne!(
                        row[3], ordinal,
                        "descriptor row cannot replace its physical target"
                    );
                    let target = &rows.operations[row[3] as usize];
                    assert_eq!(target.0, operation.site());
                    assert_eq!(
                        target.1[0],
                        match hook.recipe() {
                            ResourceKernelRecipe::NetworkFactory => 1,
                            ResourceKernelRecipe::NetworkBorrow => 6,
                            ResourceKernelRecipe::Finalize => 7,
                        }
                    );
                    assert_eq!(
                        target.1[1],
                        layout
                            .frame(function.function(), operation.frame())
                            .unwrap()
                    );
                    assert_eq!(target.1[2], row[1]);
                    let Role::InvokeHook { source, .. } = operation.role() else {
                        panic!("hook role");
                    };
                    assert_eq!(
                        source.target,
                        jett_hir::CallTarget::Indirect {
                            signature_type: hook.function_type()
                        }
                    );
                    assert_eq!(source.bridge, jett_hir::CallBridge::Direct);
                }
            }
            assert_eq!(indirect, usize::from(source != UNUSED));
        }
    }
}

#[test]
fn resource_returned_hook_layout_refuses_current_producer_signature_target_and_source_frame_substitution()
 {
    for release in [false, true] {
        let checked = checked(CLOSE, release);
        let types = &checked.checked().interner;
        let program = lower_source(&checked);
        let selected = entry(&program);
        let descriptor = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "descriptor")
            .unwrap()
            .id;
        for mutation in 0..6 {
            let mut changed = program.clone();
            match mutation {
                0 => {
                    let function = &mut changed.functions[descriptor.index() as usize];
                    let value = function
                        .blocks
                        .iter_mut()
                        .find_map(|block| {
                            if let TerminatorKind::Return(Some(value)) = &mut block.terminator.kind
                            {
                                Some(value)
                            } else {
                                None
                            }
                        })
                        .unwrap();
                    value.kind = ExpressionKind::ResourceHookValue {
                        hook: program
                            .resource_manifest
                            .hooks()
                            .find(|hook| hook.recipe() == ResourceKernelRecipe::NetworkBorrow)
                            .unwrap()
                            .clone(),
                    };
                }
                1 => {
                    changed.functions[descriptor.index() as usize].return_type = TypeInterner::INT64
                }
                2 => {
                    let function = &mut changed.functions[descriptor.index() as usize];
                    function.identity.declaration.name.push_str("Foreign");
                }
                3 => {
                    let function = &mut changed.functions[selected.index() as usize];
                    let mut found = false;
                    for block in &mut function.blocks {
                        for statement in &mut block.statements {
                            if let StatementKind::Evaluate(Expression {
                                kind:
                                    ExpressionKind::IndirectCall {
                                        ownership: jett_hir::CallOwnership::Source(source),
                                        ..
                                    },
                                ..
                            }) = &mut statement.kind
                            {
                                source.target = jett_hir::CallTarget::Indirect {
                                    signature_type: TypeInterner::INT64,
                                };
                                found = true;
                            }
                        }
                    }
                    assert!(found, "authentic original Indirect site must be mutated");
                }
                4 => {
                    changed.functions[selected.index() as usize].params[0].ty = TypeInterner::INT64
                }
                5 => {
                    let function = &mut changed.functions[descriptor.index() as usize];
                    let value = function
                        .blocks
                        .iter_mut()
                        .find_map(|block| {
                            if let TerminatorKind::Return(Some(value)) = &mut block.terminator.kind
                            {
                                Some(value)
                            } else {
                                None
                            }
                        })
                        .unwrap();
                    value.kind = ExpressionKind::Nothing;
                }
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    EmittedResourceLayout::from_program(&changed, types, selected),
                    Err(CodegenError::InvalidMir(_))
                ),
                "mutation {mutation}"
            );
        }
    }
}
