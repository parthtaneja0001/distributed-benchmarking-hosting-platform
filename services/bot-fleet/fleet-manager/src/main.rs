use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::Message;
use std::env;

mod fleet {
    tonic::include_proto!("fleet");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ---- read worker addresses ----
    let worker_addresses = env::var("BOT_WORKER_ADDRESSES")
        // Workers bind 0.0.0.0 (IPv4), so default to 127.0.0.1 rather than [::1].
        .unwrap_or_else(|_| "http://127.0.0.1:50051".to_string());

    let addresses: Vec<String> = worker_addresses
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // Total load for one run, split evenly across workers.
    let total_traders: u32 = env_or("FLEET_TOTAL_TRADERS", 20);
    let total_ops: f64 = env_or("FLEET_TOTAL_OPS", 1000.0);
    let duration_secs: i32 = env_or("FLEET_DURATION_SECS", 30);

    let num_workers = addresses.len() as u32;
    if num_workers == 0 {
        return Err("BOT_WORKER_ADDRESSES is empty".into());
    }

    // ---- create one gRPC client per worker ----
    let mut clients = Vec::new();
    for addr in &addresses {
        // Workers may still be starting; retry for ~30s before giving up.
        let mut attempt = 0;
        let client = loop {
            match fleet::bot_worker_client::BotWorkerClient::connect(addr.clone()).await {
                Ok(c) => break c,
                Err(e) if attempt < 30 => {
                    attempt += 1;
                    eprintln!("worker {} not ready ({}), retrying...", addr, e);
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
                Err(e) => return Err(format!("failed to connect to {}: {}", addr, e).into()),
            }
        };
        clients.push((addr.clone(), client));
        println!("Connected to worker at {}", addr);
    }

    // ---- Kafka consumer for sandbox.ready ----
    let kafka_broker = env::var("KAFKA_BROKER").unwrap_or_else(|_| "localhost:9092".to_string());
    let consumer: StreamConsumer = rdkafka::ClientConfig::new()
        .set("bootstrap.servers", &kafka_broker)
        .set("group.id", "fleet-manager")
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", "earliest")
        .set("max.poll.interval.ms", "3600000") // 1 hour
        .create()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    consumer
        .subscribe(&["sandbox.ready"])
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    println!(
        "Fleet Manager listening for sandbox.ready events ({} workers)",
        num_workers
    );

    // ---- process events ----
    loop {
        let msg = match consumer.recv().await {
            Ok(m) => m,
            Err(e) => {
                // Transient broker errors (e.g. Kafka still starting) must not
                // kill the fleet manager; librdkafka reconnects on its own.
                eprintln!("Kafka error (retrying): {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                continue;
            }
        };

        if let Some(Ok(payload)) = msg.payload_view::<str>() {
            #[derive(serde::Deserialize)]
            struct SandboxReady {
                submission_id: String,
                endpoint: String,
            }
            if let Ok(event) = serde_json::from_str::<SandboxReady>(payload) {
                println!(
                    "Received sandbox.ready: {} -> {}",
                    event.submission_id, event.endpoint
                );

                // Distribute load evenly across workers
                let traders_per_worker = (total_traders / num_workers).max(1);
                let ops_per_worker = total_ops / num_workers as f64;

                for (addr, client) in &clients {
                    let mut client = client.clone();   // clone for the async task
                    let request = tonic::Request::new(fleet::TestRequest {
                        endpoint: event.endpoint.clone(),
                        num_traders: traders_per_worker as i32,
                        duration_secs,
                        orders_per_second: ops_per_worker,
                        test_id: event.submission_id.clone(),
                    });

                    let addr = addr.clone();
                    tokio::spawn(async move {
                        match client.start_test(request).await {
                            Ok(response) => println!(
                                "Worker {} started: {:?}",
                                addr,
                                response.into_inner()
                            ),
                            Err(e) => eprintln!("Worker {} failed: {}", addr, e),
                        }
                    });
                }
            }
        }
    }
}
fn env_or<T: std::str::FromStr>(key: &str, fallback: T) -> T {
    env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(fallback)
}
