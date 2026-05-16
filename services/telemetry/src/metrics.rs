use std::collections::BTreeMap;

use crate::{events::TelemetryEvent, hdr_wrapper::LatencyHistogram};

#[derive(Default)]
pub struct MetricsWindow {
    tests: BTreeMap<String, TestMetrics>,
}

#[derive(Debug)]
pub struct MetricsSnapshot {
    pub test_id: String,
    pub total: u64,
    pub success: u64,
    pub failure: u64,
    pub p50_us: Option<u64>,
    pub p90_us: Option<u64>,
    pub p99_us: Option<u64>,
}

struct TestMetrics {
    total: u64,
    success: u64,
    failure: u64,
    latencies: LatencyHistogram,
}

impl MetricsWindow {
    pub fn record(&mut self, event: TelemetryEvent) {
        let metrics = self
            .tests
            .entry(event.test_id.clone())
            .or_insert_with(TestMetrics::new);

        metrics.record(event);
    }

    pub fn drain_snapshots(&mut self) -> Vec<MetricsSnapshot> {
        let tests = std::mem::take(&mut self.tests);
        tests
            .into_iter()
            .map(|(test_id, metrics)| metrics.into_snapshot(test_id))
            .collect()
    }
}

impl MetricsSnapshot {
    pub fn tps(&self, window_ms: u128) -> f64 {
        if window_ms == 0 {
            return 0.0;
        }

        (self.total as f64 * 1_000.0) / window_ms as f64
    }
}

impl TestMetrics {
    fn new() -> Self {
        Self {
            total: 0,
            success: 0,
            failure: 0,
            latencies: LatencyHistogram::new(),
        }
    }

    fn record(&mut self, event: TelemetryEvent) {
        self.total += 1;

        if event.success {
            self.success += 1;
            self.latencies.record(event.latency_us);
        } else {
            self.failure += 1;
        }

        // Keep order_id parsed and owned by the event contract, even though v1
        // aggregates by test_id only. Correctness checks will use it later.
        let _ = event.order_id;
    }

    fn into_snapshot(self, test_id: String) -> MetricsSnapshot {
        let latency = self.latencies.snapshot();

        MetricsSnapshot {
            test_id,
            total: self.total,
            success: self.success,
            failure: self.failure,
            p50_us: latency.as_ref().map(|snapshot| snapshot.p50_us),
            p90_us: latency.as_ref().map(|snapshot| snapshot.p90_us),
            p99_us: latency.as_ref().map(|snapshot| snapshot.p99_us),
        }
    }
}
