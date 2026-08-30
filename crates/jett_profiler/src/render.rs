use std::fmt::Write;

use crate::{CpuProfile, CpuSuggestionRule};

/// Render the deterministic CPU aggregation as the stable TOON profile object.
///
/// Runtime-owned status, termination, backend, and coverage fields are added by
/// the eventual run-envelope integration; this renderer owns only fields that
/// are already known to the backend-neutral aggregation model.
pub fn render_cpu_profile_toon(profile: &CpuProfile) -> String {
    let mut output = String::new();
    writeln!(output, "profile:").expect("writing to a String cannot fail");
    writeln!(output, "  schema: jett.profile.v1").expect("writing to a String cannot fail");
    writeln!(output, "  mode: cpu").expect("writing to a String cannot fail");
    writeln!(output, "  config:").expect("writing to a String cannot fail");
    writeln!(
        output,
        "    threshold_basis_points: {}",
        profile.config.threshold_basis_points
    )
    .expect("writing to a String cannot fail");
    writeln!(output, "    limit: {}", profile.config.limit)
        .expect("writing to a String cannot fail");
    writeln!(output, "  totals:").expect("writing to a String cannot fail");
    writeln!(
        output,
        "    requested_ticks: {}",
        profile.totals.requested_ticks
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "    recorded_samples: {}",
        profile.totals.recorded_samples
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "    attributed_samples: {}",
        profile.totals.attributed_samples
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "    runtime_samples: {}",
        profile.totals.runtime_samples
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "    waiting_samples: {}",
        profile.totals.waiting_samples
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "    unavailable_samples: {}",
        profile.totals.unavailable_samples
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "    coalesced_ticks: {}",
        profile.totals.coalesced_ticks
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "    collector_dropped_ticks: {}",
        profile.totals.collector_dropped_ticks
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "  eligible_bottlenecks: {}",
        profile.eligible_bottlenecks
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "  emitted_bottlenecks: {}",
        profile.bottlenecks.len()
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "  truncated_bottlenecks: {}",
        profile.truncated_bottlenecks
    )
    .expect("writing to a String cannot fail");
    writeln!(
        output,
        "  bottlenecks[{}]{{rank,namespace,function,path,line,column,inclusive_samples,self_samples,cpu_percent,suggestion_rule}}:",
        profile.bottlenecks.len()
    )
    .expect("writing to a String cannot fail");

    for (index, bottleneck) in profile.bottlenecks.iter().enumerate() {
        let percent = bottleneck.cpu_percent_hundredths;
        writeln!(
            output,
            "    {},{},{},{},{},{},{},{},{}.{:02},{}",
            index + 1,
            escape_toon_scalar(&bottleneck.frame.namespace),
            escape_toon_scalar(&bottleneck.frame.function),
            escape_toon_scalar(&bottleneck.frame.path),
            bottleneck.frame.line,
            bottleneck.frame.column,
            bottleneck.inclusive_samples,
            bottleneck.self_samples,
            percent / 100,
            percent % 100,
            suggestion_rule_name(&bottleneck.suggestion),
        )
        .expect("writing to a String cannot fail");
    }

    output
}

fn suggestion_rule_name(rule: &CpuSuggestionRule) -> &'static str {
    match rule {
        CpuSuggestionRule::HighSelf => "CPU_HIGH_SELF",
        CpuSuggestionRule::CalleeDominated => "CPU_CALLEE_DOMINATED",
    }
}

fn escape_toon_scalar(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
        .replace(',', "\\,")
}
