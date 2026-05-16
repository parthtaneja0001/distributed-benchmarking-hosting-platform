use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use crate::order_generator::generate_order;

#[derive(Deserialize)]
struct Ack {
    event: String,               
    order_id: String,
    timestamp: u128,
}

#[derive(Serialize, Clone)]
pub struct TelemetryEvent {
    pub test_id: String,
    pub order_id: String,
    pub latency_us: u64,
    pub success: bool,
}

/// One simulated trader: connects, sends orders, reads acks, produces telemetry.
pub async fn run_bot(
    endpoint: String,
    trader_id: u32,
    orders_per_sec: f64,
    duration: Duration,
    test_id: String,
    tx: mpsc::UnboundedSender<TelemetryEvent>,
) {
    // Connect to the WebSocket endpoint
    let ws_stream = match connect_async(&endpoint).await {
        Ok((stream, _)) => stream,
        Err(e) => {
            eprintln!("Bot {}: connection failed: {}", trader_id, e);
            return;
        }
    };

    let (mut write, mut read) = ws_stream.split();

    // Spawn a reader task that listens for acks
    let tx_clone = tx.clone();
    let test_id_clone = test_id.clone();
    let read_handle = tokio::spawn(async move {
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(ack) = serde_json::from_str::<Ack>(&text) {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_micros();
                        let latency = if now >= ack.timestamp {
                            (now - ack.timestamp) as u64
                        } else {
                            0
                        };
                        let event = TelemetryEvent {
                            test_id: test_id_clone.clone(),
                            order_id: ack.order_id,
                            latency_us: latency,
                            success: true,
                        };
                        let _ = tx_clone.send(event);
                    }
                }
                Ok(Message::Close(_)) => break,
                Err(e) => {
                    eprintln!("Bot {} read error: {}", trader_id, e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Sender task: generate orders at the given rate
    let interval = tokio::time::interval(Duration::from_secs_f64(1.0 / orders_per_sec));
    tokio::pin!(interval);
    let end_time = tokio::time::Instant::now() + duration;

    let mut order_counter: u64 = 0;
    while tokio::time::Instant::now() < end_time {
        interval.as_mut().tick().await;

        let order = generate_order(trader_id, order_counter);

        let payload = match serde_json::to_string(&order) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Bot {}: json error: {}", trader_id, e);
                continue;
            }
        };

        if write.send(Message::Text(payload)).await.is_err() {
            break;
        }
        order_counter += 1;
    }

    // Closing connection 
    let _ = write.send(Message::Close(None)).await;
    let _ = read_handle.await;
}