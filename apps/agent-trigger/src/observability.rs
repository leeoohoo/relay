use std::{sync::OnceLock, time::Duration};

use ai_chat_shared::latency::{LatencyPercentiles, LatencyRecorder};

pub(super) fn record_trigger_claim(elapsed: Duration, succeeded: bool) {
    static RECORDER: OnceLock<LatencyRecorder> = OnceLock::new();
    record(
        "agent_trigger_claim",
        RECORDER.get_or_init(LatencyRecorder::operational_default),
        elapsed,
        succeeded,
    );
}

pub(super) fn record_control_request(elapsed: Duration, succeeded: bool) {
    static RECORDER: OnceLock<LatencyRecorder> = OnceLock::new();
    record(
        "codex_control_request",
        RECORDER.get_or_init(LatencyRecorder::operational_default),
        elapsed,
        succeeded,
    );
}

fn record(operation: &str, recorder: &LatencyRecorder, elapsed: Duration, succeeded: bool) {
    if let Some(summary) = recorder.record(elapsed, succeeded) {
        log_summary(operation, summary);
    }
}

fn log_summary(operation: &str, summary: LatencyPercentiles) {
    tracing::info!(
        operation,
        observations = summary.observed_count,
        sampled = summary.sample_count,
        failures = summary.failure_count,
        average_us = summary.average_us,
        p50_us = summary.p50_us,
        p95_us = summary.p95_us,
        p99_us = summary.p99_us,
        max_us = summary.max_us,
        "Trigger latency window"
    );
}
