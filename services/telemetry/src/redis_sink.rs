use redis::AsyncCommands;
use serde::Serialize;

use crate::metrics::MetricsSnapshot;

const LATEST_KEY_SUFFIX: &str = "latest";

pub struct RedisSink {
    client: redis::Client,
    key_prefix: String,
}

#[derive(Serialize)]
struct RedisMetricsDocument<'a> {
    test_id: &'a str,
    window_ms: u128,
    tps: f64,
    total: u64,
    success: u64,
    failure: u64,
    p50_us: Option<u64>,
    p90_us: Option<u64>,
    p99_us: Option<u64>,
}

impl RedisSink {
    pub fn new(redis_url: &str, key_prefix: String) -> redis::RedisResult<Self> {
        let client = redis::Client::open(redis_url)?;
        Ok(Self { client, key_prefix })
    }

    pub async fn write_latest(
        &self,
        snapshot: &MetricsSnapshot,
        window_ms: u128,
    ) -> redis::RedisResult<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.latest_key(&snapshot.test_id);
        let payload = serde_json::to_string(&RedisMetricsDocument {
            test_id: &snapshot.test_id,
            window_ms,
            tps: snapshot.tps(window_ms),
            total: snapshot.total,
            success: snapshot.success,
            failure: snapshot.failure,
            p50_us: snapshot.p50_us,
            p90_us: snapshot.p90_us,
            p99_us: snapshot.p99_us,
        })
        .expect("metrics snapshot serialization should not fail");

        // Store a compact latest-state JSON document. Leaderboard service will
        // read this first; historical writes belong in the later Timescale path.
        conn.set::<_, _, ()>(key, payload).await
    }

    fn latest_key(&self, test_id: &str) -> String {
        format!("{}:{}:{}", self.key_prefix, test_id, LATEST_KEY_SUFFIX)
    }
}
