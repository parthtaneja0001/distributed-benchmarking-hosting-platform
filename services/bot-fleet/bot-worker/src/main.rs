mod connection;
mod metrics;
mod order_generator;

use tonic::{transport::Server, Request, Response, Status};
use tokio::sync::mpsc;
use std::time::Duration;

mod fleet {
    tonic::include_proto!("fleet");
}

pub struct BotWorkerService;

#[tonic::async_trait]
impl fleet::bot_worker_server::BotWorker for BotWorkerService {
    async fn start_test(
        &self,
        request: Request<fleet::TestRequest>,
    ) -> Result<Response<fleet::TestResponse>, Status> {
        let req = request.into_inner();
        println!("Received test request: {:?}", req);

        let test_id = uuid::Uuid::new_v4().to_string();
        let endpoint = req.endpoint.clone();
        let num_traders = req.num_traders.max(1) as u32;
        let duration_secs = req.duration_secs.max(1);
        let orders_per_second = req.orders_per_second;

        // Channel for telemetry events from bots -> Kafka publisher
        let (tx, rx) = mpsc::unbounded_channel();

        // Spawn the Kafka publisher task
        tokio::spawn(metrics::kafka_publisher(rx));

        // Spawn bot tasks
        let test_id_clone = test_id.clone();
        tokio::spawn(async move {
            let duration = Duration::from_secs(duration_secs as u64);
            let mut handles = Vec::new();

            // Per-trader rate
            let rate_per_trader = orders_per_second / num_traders as f64;

            for i in 0..num_traders {
                let endpoint = endpoint.clone();
                let tx = tx.clone();
                let test_id = test_id_clone.clone();
                let handle = tokio::spawn(connection::run_bot(
                    endpoint,
                    i,
                    rate_per_trader,
                    duration,
                    test_id,
                    tx,
                ));
                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.await;
            }
            println!("Test {} completed", test_id_clone);
        });

        Ok(Response::new(fleet::TestResponse {
            accepted: true,
            test_id,
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse().unwrap();
    println!("Bot Worker listening on {}", addr);

    Server::builder()
        .add_service(fleet::bot_worker_server::BotWorkerServer::new(BotWorkerService))
        .serve(addr)
        .await?;

    Ok(())
}