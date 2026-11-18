use std::collections::BTreeMap;
use std::time::{Duration, Instant};

// Forget a test's in-memory state after this long without events.
const IDLE_EVICT_AFTER: Duration = Duration::from_secs(600);

use crate::events::TelemetryEvent;
use crate::hdr_wrapper::LatencyHistogram;
use crate::shadow_book::ShadowBook;
use crate::validation::FillValidator;

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
        metrics
            .validator
            .record_order(order_id.clone(), side.clone(), order_type.clone(), price, quantity);
        metrics.shadow_book.process_order(order_id, side, order_type, price, quantity);
        // Only the reference book's total volume is used, so drain its fill
        // list; letting it grow made every later fill an O(n) scan.
        metrics.expected_volume += metrics
            .shadow_book
            .expected_fills
            .drain(..)
            .map(|f| f.quantity as u64)
            .sum::<u64>();
        metrics.touch();
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
        metrics.validator.record_fill(order_id, fill_price, fill_quantity);
        metrics.touch();
    }

    /// Snapshots for tests that saw activity since the last flush.
    ///
    /// Metrics are cumulative over the whole run, so a finished run keeps its
    /// final numbers: idle tests are skipped instead of being overwritten with
    /// an empty window (which previously zeroed TPS/latency seconds after a
    /// run ended).
    pub fn drain_snapshots(&mut self, window_ms: u128) -> Vec<MetricsSnapshot> {
        let now = Instant::now();
        self.tests
            .retain(|_, m| now.duration_since(m.last_event) < IDLE_EVICT_AFTER);
        self.tests
            .iter_mut()
            .filter(|(_, m)| m.dirty)
            .map(|(test_id, metrics)| {
                metrics.dirty = false;
                metrics.into_snapshot(test_id.clone(), window_ms)
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
    pub shadow_book: ShadowBook,   // reference book (expected volume only)
    validator: FillValidator,
    expected_volume: u64,
    /// Ack times reported by the bots (epoch µs); preferred over local
    /// processing time, which lags when Kafka consumption falls behind.
    first_ts_us: Option<u64>,
    last_ts_us: Option<u64>,
    first_ack: Option<Instant>,
    last_ack: Option<Instant>,
    last_event: Instant,
    dirty: bool,
}

impl TestMetrics {
    fn new() -> Self {
        Self {
            total: 0,
            success: 0,
            failure: 0,
            histogram: LatencyHistogram::new(),
            shadow_book: ShadowBook::new(),
            validator: FillValidator::new(),
            expected_volume: 0,
            first_ts_us: None,
            last_ts_us: None,
            first_ack: None,
            last_ack: None,
            last_event: Instant::now(),
            dirty: false,
        }
    }

    fn touch(&mut self) {
        self.last_event = Instant::now();
        self.dirty = true;
    }

    fn record(&mut self, event: TelemetryEvent) {
        let now = Instant::now();
        self.first_ack.get_or_insert(now);
        if let Some(ts) = event.ts_us {
            self.first_ts_us = Some(self.first_ts_us.map_or(ts, |f| f.min(ts)));
            self.last_ts_us = Some(self.last_ts_us.map_or(ts, |l| l.max(ts)));
        }
        self.last_ack = Some(now);
        self.touch();
        self.total += 1;
        if event.success {
            self.success += 1;
            self.histogram.record(event.latency_us);
        } else {
            self.failure += 1;
        }
    }

    fn into_snapshot(&self, test_id: String, _window_ms: u128) -> MetricsSnapshot {
        let latency = self.histogram.snapshot();
        let correctness = self.validator.correctness(self.expected_volume);
        if std::env::var("TELEMETRY_DEBUG").is_ok() {
            eprintln!("[{}] {}", test_id, self.validator.debug_counts(self.expected_volume));
        }

        MetricsSnapshot {
            test_id,
            total: self.total,
            success: self.success,
            failure: self.failure,
            p50_us: latency.as_ref().map(|s| s.p50_us),
            p90_us: latency.as_ref().map(|s| s.p90_us),
            p99_us: latency.as_ref().map(|s| s.p99_us),
            // Acknowledged orders per second over the run's active span.
            tps: match (self.first_ts_us, self.last_ts_us, self.first_ack, self.last_ack) {
                (Some(first), Some(last), _, _) => {
                    let secs = ((last - first) as f64 / 1e6).max(1.0);
                    self.success as f64 / secs
                }
                (None, None, Some(first), Some(last)) => {
                    let secs = last.duration_since(first).as_secs_f64().max(1.0);
                    self.success as f64 / secs
                }
                _ => 0.0,
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