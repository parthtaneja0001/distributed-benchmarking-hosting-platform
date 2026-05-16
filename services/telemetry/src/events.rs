use serde::Deserialize;

// This mirrors bot-worker's telemetry payload on the telemetry.raw topic.
#[derive(Debug, Deserialize)]
pub struct TelemetryEvent {
    pub test_id: String,
    pub order_id: String,
    pub latency_us: u64,
    pub success: bool,
}
