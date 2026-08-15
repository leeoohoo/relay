use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LatencyPercentiles {
    pub observed_count: u64,
    pub sample_count: usize,
    pub failure_count: u64,
    pub average_us: u64,
    pub p50_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
    pub max_us: u64,
}

#[derive(Debug)]
pub struct LatencyRecorder {
    report_interval: Duration,
    min_observations: u64,
    sample_capacity: usize,
    state: Mutex<LatencyState>,
}

#[derive(Debug)]
struct LatencyState {
    samples_us: Vec<u64>,
    next_sample: usize,
    observed_count: u64,
    failure_count: u64,
    last_reported_at: Instant,
}

impl LatencyRecorder {
    pub fn new(report_interval: Duration, min_observations: u64, sample_capacity: usize) -> Self {
        let sample_capacity = sample_capacity.max(1);
        Self {
            report_interval,
            min_observations: min_observations.max(1),
            sample_capacity,
            state: Mutex::new(LatencyState {
                samples_us: Vec::with_capacity(sample_capacity),
                next_sample: 0,
                observed_count: 0,
                failure_count: 0,
                last_reported_at: Instant::now(),
            }),
        }
    }

    pub fn operational_default() -> Self {
        Self::new(Duration::from_secs(60), 20, 256)
    }

    pub fn record(&self, elapsed: Duration, succeeded: bool) -> Option<LatencyPercentiles> {
        let mut state = self.state.lock().ok()?;
        let elapsed_us = elapsed.as_micros().min(u64::MAX as u128) as u64;
        if state.samples_us.len() < self.sample_capacity {
            state.samples_us.push(elapsed_us);
        } else {
            let index = state.next_sample;
            state.samples_us[index] = elapsed_us;
            state.next_sample = (index + 1) % self.sample_capacity;
        }
        state.observed_count = state.observed_count.saturating_add(1);
        if !succeeded {
            state.failure_count = state.failure_count.saturating_add(1);
        }
        if state.observed_count < self.min_observations
            || state.last_reported_at.elapsed() < self.report_interval
        {
            return None;
        }

        let mut samples = state.samples_us.clone();
        samples.sort_unstable();
        let sample_count = samples.len();
        let total_us = samples.iter().map(|value| u128::from(*value)).sum::<u128>();
        let summary = LatencyPercentiles {
            observed_count: state.observed_count,
            sample_count,
            failure_count: state.failure_count,
            average_us: (total_us / sample_count as u128).min(u64::MAX as u128) as u64,
            p50_us: percentile(&samples, 50),
            p95_us: percentile(&samples, 95),
            p99_us: percentile(&samples, 99),
            max_us: samples.last().copied().unwrap_or_default(),
        };
        state.samples_us.clear();
        state.next_sample = 0;
        state.observed_count = 0;
        state.failure_count = 0;
        state.last_reported_at = Instant::now();
        Some(summary)
    }
}

fn percentile(sorted_samples: &[u64], percentile: usize) -> u64 {
    let rank = sorted_samples
        .len()
        .saturating_mul(percentile)
        .div_ceil(100)
        .saturating_sub(1);
    sorted_samples.get(rank).copied().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_percentiles_only_after_the_minimum_window() {
        let recorder = LatencyRecorder::new(Duration::ZERO, 3, 8);
        assert!(recorder.record(Duration::from_micros(10), true).is_none());
        assert!(recorder.record(Duration::from_micros(20), false).is_none());
        let summary = recorder
            .record(Duration::from_micros(100), true)
            .expect("third observation should report");
        assert_eq!(summary.observed_count, 3);
        assert_eq!(summary.sample_count, 3);
        assert_eq!(summary.failure_count, 1);
        assert_eq!(summary.average_us, 43);
        assert_eq!(summary.p50_us, 20);
        assert_eq!(summary.p95_us, 100);
        assert_eq!(summary.p99_us, 100);
        assert_eq!(summary.max_us, 100);
    }

    #[test]
    fn bounded_window_keeps_the_most_recent_samples() {
        let recorder = LatencyRecorder::new(Duration::ZERO, 4, 3);
        for micros in [1, 2, 3] {
            assert!(recorder
                .record(Duration::from_micros(micros), true)
                .is_none());
        }
        let summary = recorder
            .record(Duration::from_micros(4), true)
            .expect("fourth observation should report");
        assert_eq!(summary.observed_count, 4);
        assert_eq!(summary.sample_count, 3);
        assert_eq!(summary.average_us, 3);
        assert_eq!(summary.p50_us, 3);
        assert_eq!(summary.max_us, 4);
    }
}
