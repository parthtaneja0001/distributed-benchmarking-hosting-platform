use std::{env, time::Duration};

const DEFAULT_KAFKA_BROKER: &str = "localhost:9092";
const DEFAULT_TELEMETRY_TOPIC: &str = "telemetry.raw";
const DEFAULT_GROUP_ID: &str = "telemetry-ingester";
const DEFAULT_AUTO_OFFSET_RESET: &str = "latest";
const DEFAULT_FLUSH_INTERVAL_MS: u64 = 1_000;
const DEFAULT_REDIS_URL: &str = "redis://127.0.0.1:6379";
const DEFAULT_REDIS_KEY_PREFIX: &str = "test";

#[derive(Debug, Clone)]
pub struct Config {
    pub kafka_broker: String,
    pub telemetry_topic: String,
    pub group_id: String,
    pub auto_offset_reset: String,
    pub flush_interval: Duration,
    pub redis_url: String,
    pub redis_key_prefix: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            kafka_broker: env_or_default("KAFKA_BROKER", DEFAULT_KAFKA_BROKER),
            telemetry_topic: env_or_default("TELEMETRY_TOPIC", DEFAULT_TELEMETRY_TOPIC),
            group_id: env_or_default("TELEMETRY_GROUP_ID", DEFAULT_GROUP_ID),
            auto_offset_reset: env_or_default(
                "TELEMETRY_AUTO_OFFSET_RESET",
                DEFAULT_AUTO_OFFSET_RESET,
            ),
            flush_interval: Duration::from_millis(env_u64_or_default(
                "METRICS_FLUSH_INTERVAL_MS",
                DEFAULT_FLUSH_INTERVAL_MS,
            )),
            redis_url: env_or_default("REDIS_URL", DEFAULT_REDIS_URL),
            redis_key_prefix: env_or_default("REDIS_KEY_PREFIX", DEFAULT_REDIS_KEY_PREFIX),
        }
    }
}

fn env_or_default(key: &str, fallback: &str) -> String {
    env::var(key).unwrap_or_else(|_| fallback.to_string())
}

fn env_u64_or_default(key: &str, fallback: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(fallback)
}
