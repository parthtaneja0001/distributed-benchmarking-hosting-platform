use tokio::time;

use crate::{
    config::Config,
    kafka_consumer::TelemetryConsumer,   // existing consumer for telemetry.raw
    metrics::{MetricsSnapshot, MetricsWindow},
    redis_sink::RedisSink,
};

use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::Message;
use serde::Deserialize;

// ---------- new event types (match the bot worker's output) ----------

#[derive(Debug, Deserialize)]
struct OrderSent {
    test_id: String,
    order_id: String,
    #[serde(rename = "order_type")]
    order_type: String,
    side: String,
    price: f64,
    quantity: u32,
}

#[derive(Debug, Deserialize)]
struct FillActual {
    test_id: String,
    order_id: String,
    fill_price: f64,
    fill_quantity: u32,
}

// ---------- processor ----------

pub async fn run(config: Config, consumer: TelemetryConsumer, redis_sink: RedisSink) {
    let mut window = MetricsWindow::new();
    let mut flush = time::interval(config.flush_interval);
    let window_ms = config.flush_interval.as_millis();

    // Create consumers for the correctness topics
    let orders_consumer: StreamConsumer = rdkafka::ClientConfig::new()
        .set("bootstrap.servers", &config.kafka_broker)
        .set("group.id", format!("{}-orders", config.group_id))
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", "earliest")
        .create()
        .expect("failed to create orders consumer");
    orders_consumer.subscribe(&["orders.sent"]).expect("subscribe orders.sent");

    let fills_consumer: StreamConsumer = rdkafka::ClientConfig::new()
        .set("bootstrap.servers", &config.kafka_broker)
        .set("group.id", format!("{}-fills", config.group_id))
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", "earliest")
        .create()
        .expect("failed to create fills consumer");
    fills_consumer.subscribe(&["fills.actual"]).expect("subscribe fills.actual");

    println!(
        "Telemetry ingester consuming topic={} broker={} group={} redis={} interval_ms={}",
        config.telemetry_topic, config.kafka_broker, config.group_id, config.redis_url, window_ms
    );

    loop {
        tokio::select! {
            // 1. standard telemetry (latency) events
            event = consumer.recv() => {
                match event {
                    Ok(event) => window.record_telemetry(event),
                    Err(err) => eprintln!("{err}"),
                }
            }

            // 2. orders sent by bots
            order_msg = orders_consumer.recv() => {
                match order_msg {
                    Ok(msg) => {
                        if let Some(Ok(payload)) = msg.payload_view::<str>() {
                            if let Ok(order) = serde_json::from_str::<OrderSent>(payload) {
                                window.process_order_event(
                                    &order.test_id,
                                    order.order_id,
                                    order.side,
                                    order.order_type,
                                    order.price,
                                    order.quantity,
                                );
                            }
                        }
                    }
                    Err(err) => eprintln!("orders consumer error: {err}"),
                }
            }

            // 3. fills reported by the contestant's engine
            fill_msg = fills_consumer.recv() => {
                match fill_msg {
                    Ok(msg) => {
                        if let Some(Ok(payload)) = msg.payload_view::<str>() {
                            if let Ok(fill) = serde_json::from_str::<FillActual>(payload) {
                                window.process_fill_event(
                                    &fill.test_id,
                                    fill.order_id,
                                    fill.fill_price,
                                    fill.fill_quantity,
                                );
                            }
                        }
                    }
                    Err(err) => eprintln!("fills consumer error: {err}"),
                }
            }

            // 4. periodic flush – write latest metrics to Redis
            _ = flush.tick() => {
                for snapshot in window.drain_snapshots(window_ms) {
                    print_snapshot(&snapshot);
                    if let Err(err) = redis_sink.write_latest(&snapshot, window_ms).await {
                        eprintln!("failed to write latest metrics to Redis: {err}");
                    }
                }
            }
        }
    }
}

// ---------- helper to print snapshot ----------

fn print_snapshot(snapshot: &MetricsSnapshot) {
    println!(
        "test={} tps={:.2} success={} failure={} p50_us={} p90_us={} p99_us={} correctness={:.4}",
        snapshot.test_id,
        snapshot.tps,
        snapshot.success,
        snapshot.failure,
        display_latency(snapshot.p50_us),
        display_latency(snapshot.p90_us),
        display_latency(snapshot.p99_us),
        snapshot.correctness,
    );
}

fn display_latency(value: Option<u64>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "-".to_string())
}