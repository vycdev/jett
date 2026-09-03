use std::fmt::Write;

use crate::{CpuProfile, CpuSuggestionRule};

/// Render a deterministic human-readable CPU profile summary.
///
/// The caller owns channel selection. `jett run` writes this summary to stderr
/// so program stdout remains unchanged.
pub fn render_cpu_profile(profile: &CpuProfile) -> String {
    let totals = &profile.totals;
    let mut output = String::from("CPU profile\nCoverage: Jett runtime samples\n");
    writeln!(
        output,
        "Samples: {} attributed / {} recorded / {} requested",
        totals.attributed_samples, totals.recorded_samples, totals.requested_ticks
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "Other samples: {} runtime, {} waiting, {} unavailable",
        totals.runtime_samples, totals.waiting_samples, totals.unavailable_samples
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "Sampling loss: {} coalesced, {} collector-dropped",
        totals.coalesced_ticks, totals.collector_dropped_ticks
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "Bottlenecks: {} emitted / {} eligible / {} truncated",
        profile.bottlenecks.len(),
        profile.eligible_bottlenecks,
        profile.truncated_bottlenecks
    )
    .expect("writing to a String cannot fail");

    for (index, bottleneck) in profile.bottlenecks.iter().enumerate() {
        let frame = &bottleneck.frame;
        let qualified_name = format!("{}.{}", frame.namespace, frame.function);
        writeln!(
            output,
            "{}. {} at {}:{}:{}",
            index + 1,
            qualified_name,
            frame.path,
            frame.line,
            frame.column
        )
        .expect("writing to a String cannot fail");
        writeln!(
            output,
            "   {}.{:02}% inclusive ({} {}), {} {}",
            bottleneck.cpu_percent_hundredths / 100,
            bottleneck.cpu_percent_hundredths % 100,
            bottleneck.inclusive_samples,
            sample_label(bottleneck.inclusive_samples),
            bottleneck.self_samples,
            self_sample_label(bottleneck.self_samples)
        )
        .expect("writing to a String cannot fail");

        let (rule, action) = match bottleneck.suggestion {
            CpuSuggestionRule::HighSelf => ("CPU_HIGH_SELF", "hot lines"),
            CpuSuggestionRule::CalleeDominated => ("CPU_CALLEE_DOMINATED", "dominant call chain"),
        };
        writeln!(
            output,
            "   {rule}: Inspect the {action} for {qualified_name}."
        )
        .expect("writing to a String cannot fail");
    }

    output
}

fn sample_label(count: u64) -> &'static str {
    if count == 1 { "sample" } else { "samples" }
}

fn self_sample_label(count: u64) -> &'static str {
    if count == 1 {
        "self sample"
    } else {
        "self samples"
    }
}
