use jett_profiler::{CpuConfig, CpuProfile, CpuSample, FrameIdentity, render_cpu_profile_toon};

fn frame(namespace: &str, function: &str) -> FrameIdentity {
    FrameIdentity::new(namespace, function, "src/main.jett", 1, 1)
}

#[test]
fn cpu_toon_rendering_is_deterministic_and_escaped() {
    let root = frame("app,core", "main");
    let leaf = frame("app,core", "work\nitem");
    let profile = CpuProfile::aggregate(
        CpuConfig::default(),
        6,
        2,
        1,
        vec![
            CpuSample::jett(vec![root.clone(), leaf.clone()]),
            CpuSample::jett(vec![root.clone(), leaf]),
            CpuSample::jett(vec![root]),
            CpuSample::runtime(),
            CpuSample::waiting(),
            CpuSample::unavailable(),
        ],
    );

    assert_eq!(
        render_cpu_profile_toon(&profile),
        "profile:\n  schema: jett.profile.v1\n  mode: cpu\n  config:\n    threshold_basis_points: 500\n    limit: 10\n  totals:\n    requested_ticks: 6\n    recorded_samples: 6\n    attributed_samples: 3\n    runtime_samples: 1\n    waiting_samples: 1\n    unavailable_samples: 1\n    coalesced_ticks: 2\n    collector_dropped_ticks: 1\n  eligible_bottlenecks: 2\n  emitted_bottlenecks: 2\n  truncated_bottlenecks: 0\n  bottlenecks[2]{rank,namespace,function,path,line,column,inclusive_samples,self_samples,cpu_percent,suggestion_rule}:\n    1,app\\,core,main,src/main.jett,1,1,3,1,100.00,CPU_CALLEE_DOMINATED\n    2,app\\,core,work\\nitem,src/main.jett,1,1,2,2,66.67,CPU_HIGH_SELF\n"
    );
}
