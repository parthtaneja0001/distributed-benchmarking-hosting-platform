use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::Message;

mod fleet {
    tonic::include_proto!("fleet");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create gRPC client – map error into Box<dyn Error>
    let worker_addr = "http://[::1]:50051";
    let mut client = fleet::bot_worker_client::BotWorkerClient::connect(worker_addr)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    // 2. Kafka consumer for sandbox.ready
    let consumer: StreamConsumer = rdkafka::ClientConfig::new()
        .set("bootstrap.servers", "localhost:9092")
        .set("group.id", "fleet-manager")
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", "earliest")
        .set("max.poll.interval.ms", "3600000")   // 1 hour
        .create()
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    consumer
        .subscribe(&["sandbox.ready"])
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    println!("Fleet Manager listening for sandbox.ready events...");

    // 3. Process events
    loop {
        let msg = match consumer.recv().await {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Kafka error: {}", e);
                return Err(Box::new(e) as Box<dyn std::error::Error>);
            }
        };

        // payload_view returns Option<Result<&str, Utf8Error>>
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

                let request = tonic::Request::new(fleet::TestRequest {
                    endpoint: event.endpoint.clone(),
                    num_traders: 10,
                    duration_secs: 30,
                    orders_per_second: 100.0,
                });

                match client.start_test(request).await {
                    Ok(response) => println!("Test started: {:?}", response.into_inner()),
                    Err(e) => eprintln!("gRPC call failed: {}", e),
                }
            }
        }
    }
}