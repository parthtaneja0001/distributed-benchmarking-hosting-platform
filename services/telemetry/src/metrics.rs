use std::collections::BTreeMap;

use crate::events::TelemetryEvent;
use crate::hdr_wrapper::LatencyHistogram;
use crate::shadow_book::ShadowBook;

// ---------- metrics window (persistent per test) ----------

pub struct MetricsWindow {
    tests: BTreeMap<String, TestMetrics>,
}

impl MetricsWindow {
    pub fn new() -> Self {
        Self {
            tests: BTreeMap::new(),
        }
    }
    /// Record a standard telemetry event (latency).
    pub fn record_telemetry(&mut self, event: TelemetryEvent) {
        let metrics = self
            .tests
            .entry(event.test_id.clone())
            .or_insert_with(TestMetrics::new);
        metrics.record(event);
    }

    /// Feed an incoming order to the shadow book for correctness.
    pub fn process_order_event(
        &mut self,
        test_id: &str,
        order_id: String,
        side: String,
        order_type: String,
        price: f64,
        quantity: u32,
    ) {
        let metrics = self
            .tests
            .entry(test_id.to_string())
            .or_insert_with(TestMetrics::new);
        metrics.shadow_book.process_order(order_id, side, order_type, price, quantity);
    }

    /// Feed an incoming actual fill to the shadow book.
    pub fn process_fill_event(
        &mut self,
        test_id: &str,
        order_id: String,
        fill_price: f64,
        fill_quantity: u32,
    ) {
        let metrics = self
            .tests
            .entry(test_id.to_string())
            .or_insert_with(TestMetrics::new);
        metrics.shadow_book.match_fill(&order_id, fill_price, fill_quantity);
    }

    /// Compute per‑window snapshots **without removing the test state**.
    /// Only the latency/throughput counters are reset; the shadow book
    /// (order book + cumulative correctness) remains intact.
    pub fn drain_snapshots(&mut self, window_ms: u128) -> Vec<MetricsSnapshot> {
        self.tests
            .iter_mut()
            .map(|(test_id, metrics)| {
                let snapshot = metrics.into_snapshot(test_id.clone(), window_ms);
                metrics.reset_window_counters(); // only resets totals & histogram
                snapshot
            })
            .collect()
    }

    /// Remove a finished test entirely (optional, prevents memory leaks).
    pub fn remove_test(&mut self, test_id: &str) {
        self.tests.remove(test_id);
    }
}

// ---------- per‑test metrics ----------

pub struct TestMetrics {
    pub total: u64,
    pub success: u64,
    pub failure: u64,
    pub histogram: LatencyHistogram,
    pub shadow_book: ShadowBook,   // order book + fill counters
}

impl TestMetrics {
    fn new() -> Self {
        Self {
            total: 0,
            success: 0,
            failure: 0,
            histogram: LatencyHistogram::new(),
            shadow_book: ShadowBook::new(),
        }
    }

    fn record(&mut self, event: TelemetryEvent) {
        self.total += 1;
        if event.success {
            self.success += 1;
            self.histogram.record(event.latency_us);
        } else {
            self.failure += 1;
        }
    }

    /// Reset only the window counters (latency/throughput) – the order book
    /// state and cumulative fill counters are kept across windows.
    fn reset_window_counters(&mut self) {
        self.total = 0;
        self.success = 0;
        self.failure = 0;
        self.histogram.reset();
        // shadow_book fill counters are NOT reset here – they accumulate
    }

    fn into_snapshot(&self, test_id: String, window_ms: u128) -> MetricsSnapshot {
        let latency = self.histogram.snapshot();
        let correctness = self.shadow_book.correctness();

        MetricsSnapshot {
            test_id,
            total: self.total,
            success: self.success,
            failure: self.failure,
            p50_us: latency.as_ref().map(|s| s.p50_us),
            p90_us: latency.as_ref().map(|s| s.p90_us),
            p99_us: latency.as_ref().map(|s| s.p99_us),
            tps: if window_ms > 0 {
                (self.total as f64 * 1_000.0) / window_ms as f64
            } else {
                0.0
            },
            correctness,
        }
    }
}

// ---------- snapshot returned to the processor ----------

#[derive(Debug)]
pub struct MetricsSnapshot {
    pub test_id: String,
    pub total: u64,
    pub success: u64,
    pub failure: u64,
    pub p50_us: Option<u64>,
    pub p90_us: Option<u64>,
    pub p99_us: Option<u64>,
    pub tps: f64,
    pub correctness: f64,   // cumulative correctness at this point
}