use tokio::time;

use crate::{
    config::Config, kafka_consumer::TelemetryConsumer, metrics::MetricsSnapshot,
    metrics::MetricsWindow, redis_sink::RedisSink,
};

pub async fn run(config: Config, consumer: TelemetryConsumer, redis_sink: RedisSink) {
    let mut window = MetricsWindow::default();
    let mut flush = time::interval(config.flush_interval);
    let window_ms = config.flush_interval.as_millis();

    println!(
        "Telemetry ingester consuming topic={} broker={} group={} redis={} interval_ms={}",
        config.telemetry_topic, config.kafka_broker, config.group_id, config.redis_url, window_ms
    );

    loop {
        tokio::select! {
            event = consumer.recv() => {
                match event {
                    Ok(event) => window.record(event),
                    Err(err) => eprintln!("{err}"),
                }
            }
            _ = flush.tick() => {
                for snapshot in window.drain_snapshots() {
                    print_snapshot(&snapshot, window_ms);
                    if let Err(err) = redis_sink.write_latest(&snapshot, window_ms).await {
                        eprintln!("failed to write latest metrics to Redis: {err}");
                    }
                }
            }
        }
    }
}

fn print_snapshot(snapshot: &MetricsSnapshot, window_ms: u128) {
    println!(
        "test={} tps={:.2} success={} failure={} p50_us={} p90_us={} p99_us={}",
        snapshot.test_id,
        snapshot.tps(window_ms),
        snapshot.success,
        snapshot.failure,
        display_latency(snapshot.p50_us),
        display_latency(snapshot.p90_us),
        display_latency(snapshot.p99_us),
    );
}

fn display_latency(value: Option<u64>) -> String {
    value
        .map(|latency| latency.to_string())
        .unwrap_or_else(|| "-".to_string())
}
