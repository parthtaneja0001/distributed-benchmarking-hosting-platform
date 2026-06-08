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
        .unwrap_or_else(|_| "http://[::1]:50051".to_string());

    let addresses: Vec<String> = worker_addresses
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    let num_workers = addresses.len() as u32;
    if num_workers == 0 {
        return Err("BOT_WORKER_ADDRESSES is empty".into());
    }

    // ---- create one gRPC client per worker ----
    let mut clients = Vec::new();
    for addr in &addresses {
        let client = fleet::bot_worker_client::BotWorkerClient::connect(addr.clone())
            .await
            .map_err(|e| format!("failed to connect to {}: {}", addr, e))?;
        clients.push((addr.clone(), client));
        println!("Connected to worker at {}", addr);
    }

    // ---- Kafka consumer for sandbox.ready ----
    let consumer: StreamConsumer = rdkafka::ClientConfig::new()
        .set("bootstrap.servers", "localhost:9092")
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
                eprintln!("Kafka error: {}", e);
                return Err(Box::new(e) as Box<dyn std::error::Error>);
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
                let traders_per_worker = 10 / num_workers;
                let ops_per_worker = 100.0 / num_workers as f64;

                for (addr, client) in &clients {
                    let mut client = client.clone();   // clone for the async task
                    let request = tonic::Request::new(fleet::TestRequest {
                        endpoint: event.endpoint.clone(),
                        num_traders: traders_per_worker as i32,
                        duration_secs: 30,
                        orders_per_second: ops_per_worker,
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