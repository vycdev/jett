use jett_profiler::{CpuConfig, CpuProfile, CpuSample, FrameIdentity, human::render_cpu_profile};

fn frame(function: &str, line: u32) -> FrameIdentity {
    FrameIdentity::new(
        "pipeline.transform",
        function,
        "src/transform.jett",
        line,
        5,
    )
}

#[test]
fn cpu_human_summary_preserves_totals_ranking_and_fixed_precision() {
    let root = frame("main", 3);
    let work = frame("process_image", 142);
    let profile = CpuProfile::aggregate(
        CpuConfig::default(),
        8,
        2,
        1,
        vec![
            CpuSample::jett(vec![root.clone(), work.clone()]),
            CpuSample::jett(vec![root.clone(), work]),
            CpuSample::jett(vec![root]),
            CpuSample::runtime(),
            CpuSample::waiting(),
        ],
    );

    assert_eq!(
        render_cpu_profile(&profile),
        concat!(
            "CPU profile\n",
            "Coverage: Jett runtime samples\n",
            "Samples: 3 attributed / 5 recorded / 8 requested\n",
            "Other samples: 1 runtime, 1 waiting, 0 unavailable\n",
            "Sampling loss: 2 coalesced, 1 collector-dropped\n",
            "Bottlenecks: 2 emitted / 2 eligible / 0 truncated\n",
            "1. pipeline.transform.main at src/transform.jett:3:5\n",
            "   100.00% inclusive (3 samples), 1 self sample\n",
            "   CPU_CALLEE_DOMINATED: Inspect the dominant call chain for ",
            "pipeline.transform.main.\n",
            "2. pipeline.transform.process_image at src/transform.jett:142:5\n",
            "   66.67% inclusive (2 samples), 2 self samples\n",
            "   CPU_HIGH_SELF: Inspect the hot lines for ",
            "pipeline.transform.process_image.\n",
        )
    );
}
