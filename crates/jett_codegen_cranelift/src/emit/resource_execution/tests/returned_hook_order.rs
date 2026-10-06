use super::*;

#[test]
fn resource_returned_hook_all_recipes_and_immediate_callees_prepare_exact_descriptor_before_commit()
{
    for release in [false, true] {
        for (source, effect) in [
            (CLOSE, "jett_rt_v1_resource_descriptor_close"),
            (IMMEDIATE, "jett_rt_v1_resource_descriptor_close"),
            (FACTORY, "jett_rt_v1_resource_factory_prepare"),
            (BORROW, "jett_rt_v1_resource_borrow_commit"),
            (ORDER, "jett_rt_v1_resource_factory_prepare"),
            (CALLEE_FAILURE, "jett_rt_v1_resource_factory_prepare"),
            (ACTUAL_FAILURE, "jett_rt_v1_resource_factory_prepare"),
        ] {
            let (function, names, indirect) = hook_clif(source, release, "main");
            assert_eq!(indirect.len(), 1);
            let all = instructions(&function);
            let prepare = all
                .iter()
                .copied()
                .find(|instruction| {
                    call_name(&function, &names, *instruction)
                        == Some("jett_rt_v1_resource_hook_prepare")
                        && constant(&function, function.dfg.inst_args(*instruction)[2])
                            == Some(i64::from(indirect[0]))
                })
                .unwrap();
            assert_ne!(
                constant(&function, function.dfg.inst_args(prepare)[3]),
                Some(0)
            );
            let effect = all
                .iter()
                .copied()
                .find(|instruction| call_name(&function, &names, *instruction) == Some(effect))
                .unwrap();
            let (accepted, refused) =
                checked_branch(&function, function.dfg.inst_results(prepare)[0]);
            let mut cfg = ControlFlowGraph::new();
            cfg.compute(&function);
            let mut dom = DominatorTree::new();
            dom.compute(&function, &cfg);
            assert!(dom.dominates(accepted, effect, &function.layout));
            let mut pending = vec![refused];
            let mut seen = std::collections::HashSet::new();
            while let Some(block) = pending.pop() {
                if seen.insert(block) {
                    assert_ne!(Some(block), function.layout.inst_block(effect));
                    pending.extend(cfg.succ_iter(block));
                }
            }
            assert!(seen.iter().any(|block| {
                function.layout.block_insts(*block).any(|instruction| {
                    call_name(&function, &names, instruction)
                        == Some("jett_rt_v1_resource_scope_complete")
                })
            }));
            if [ORDER, CALLEE_FAILURE, ACTUAL_FAILURE].contains(&source) {
                let argument = all
                    .iter()
                    .copied()
                    .find(|instruction| {
                        call_name(&function, &names, *instruction) == Some("argument_label")
                    })
                    .unwrap();
                let callee = all
                    .iter()
                    .copied()
                    .find(|instruction| {
                        call_name(&function, &names, *instruction) == Some("select_create")
                    })
                    .unwrap();
                assert!(dom.dominates(argument, callee, &function.layout));
                assert!(dom.dominates(callee, prepare, &function.layout));
                let statuses = all
                    .iter()
                    .copied()
                    .filter(|instruction| {
                        call_name(&function, &names, *instruction)
                            == Some("jett_rt_v1_resource_source_status")
                    })
                    .collect::<Vec<_>>();
                for (called, next) in [(argument, callee), (callee, prepare)] {
                    let status = statuses
                        .iter()
                        .copied()
                        .find(|status| {
                            dom.dominates(called, *status, &function.layout)
                                && dom.dominates(*status, next, &function.layout)
                        })
                        .unwrap();
                    let (accepted, refused) =
                        checked_branch(&function, function.dfg.inst_results(status)[0]);
                    assert!(dom.dominates(accepted, next, &function.layout));
                    let mut pending = vec![refused];
                    let mut seen = std::collections::HashSet::new();
                    while let Some(block) = pending.pop() {
                        if seen.insert(block) {
                            assert_ne!(Some(block), function.layout.inst_block(next));
                            pending.extend(cfg.succ_iter(block));
                        }
                    }
                }
            }
        }
    }
}
