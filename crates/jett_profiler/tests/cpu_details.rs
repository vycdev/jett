use jett_profiler::{CpuConfig, CpuProfile, CpuSample, FrameIdentity, SourceLocation};

fn frame(function: &str) -> FrameIdentity {
    FrameIdentity::new("app", function, "src/main.jett", 1, 1)
}

#[test]
fn cpu_profile_aggregates_ranked_hot_lines_and_call_chains() {
    let root = frame("main");
    let target = frame("target");
    let cases = [
        ("alpha", 10, 4),
        ("beta", 20, 3),
        ("gamma", 30, 2),
        ("delta", 40, 1),
    ];
    let mut samples = Vec::new();

    for (caller, line, count) in cases {
        for _ in 0..count {
            samples.push(CpuSample::jett_at(
                vec![root.clone(), frame(caller), target.clone()],
                SourceLocation::new("src/main.jett", line, 5),
            ));
        }
    }

    let profile = CpuProfile::aggregate(CpuConfig::new(0, 10).unwrap(), 10, 0, 0, samples);
    let target = profile
        .bottlenecks
        .iter()
        .find(|entry| entry.frame.function == "target")
        .expect("target bottleneck");

    assert_eq!(target.hot_lines.len(), 3);
    assert_eq!(target.hot_lines[0].location.line, 10);
    assert_eq!(target.hot_lines[0].self_samples, 4);
    assert_eq!(target.hot_lines[1].location.line, 20);
    assert_eq!(target.hot_lines[1].self_samples, 3);
    assert_eq!(target.hot_lines[2].location.line, 30);
    assert_eq!(target.hot_lines[2].self_samples, 2);

    assert_eq!(target.call_chains.len(), 3);
    assert_eq!(target.call_chains[0].samples, 4);
    assert_eq!(target.call_chains[0].frames[1].function, "alpha");
    assert_eq!(target.call_chains[1].samples, 3);
    assert_eq!(target.call_chains[1].frames[1].function, "beta");
    assert_eq!(target.call_chains[2].samples, 2);
    assert_eq!(target.call_chains[2].frames[1].function, "gamma");
}
