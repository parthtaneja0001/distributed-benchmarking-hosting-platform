use hdrhistogram::Histogram;

const LOWEST_LATENCY_US: u64 = 1;
const HIGHEST_LATENCY_US: u64 = 60_000_000;
const SIGNIFICANT_DIGITS: u8 = 3;

pub struct LatencyHistogram {
    histogram: Histogram<u64>,
}

#[derive(Debug, Clone)]
pub struct LatencySnapshot {
    pub p50_us: u64,
    pub p90_us: u64,
    pub p99_us: u64,
}

impl LatencyHistogram {
    pub fn new() -> Self {
        Self {
            histogram: Histogram::new_with_bounds(
                LOWEST_LATENCY_US,
                HIGHEST_LATENCY_US,
                SIGNIFICANT_DIGITS,
            )
            .expect("latency histogram bounds must be valid"),
        }
    }

    pub fn record(&mut self, latency_us: u64) {
        // Clamp zero to one microsecond because HDR Histogram lower bound is 1.
        let value = latency_us.max(LOWEST_LATENCY_US);
        if let Err(err) = self.histogram.record(value) {
            eprintln!("failed to record latency {value}us: {err}");
        }
    }

    pub fn snapshot(&self) -> Option<LatencySnapshot> {
        if self.histogram.len() == 0 {
            return None;
        }

        Some(LatencySnapshot {
            p50_us: self.histogram.value_at_quantile(0.50),
            p90_us: self.histogram.value_at_quantile(0.90),
            p99_us: self.histogram.value_at_quantile(0.99),
        })
    }
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        Self::new()
    }
}
