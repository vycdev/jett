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
            "Stack truncation: 0 samples\n",
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

#[test]
fn cpu_human_reports_truncated_stack_samples() {
    let profile = CpuProfile::aggregate(
        CpuConfig::default(),
        1,
        0,
        0,
        vec![CpuSample::jett(vec![frame("recursive", 3); 129])],
    );
    assert!(render_cpu_profile(&profile).contains("Stack truncation: 1 sample\n"));
}

#[test]
fn cpu_human_escapes_controls_in_every_metadata_field() {
    let frame = FrameIdentity::new("app\u{1b}[2J", "work\nforged", "src/\r\t\u{7f}é.jett", 1, 2);
    let profile = CpuProfile::aggregate(
        CpuConfig::default(),
        1,
        0,
        0,
        vec![CpuSample::jett(vec![frame])],
    );
    let rendered = render_cpu_profile(&profile);
    assert!(rendered.contains("app\\u{1b}[2J.work\\nforged at src/\\r\\t\\u{7f}é.jett:1:2"));
    assert!(rendered.contains("Inspect the hot lines for app\\u{1b}[2J.work\\nforged."));
    assert!(
        !rendered
            .chars()
            .any(|character| character.is_control() && character != '\n')
    );
    assert!(!rendered.contains("\nforged"));
}
