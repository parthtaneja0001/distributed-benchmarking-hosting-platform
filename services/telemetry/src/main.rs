mod config;
mod events;
mod hdr_wrapper;
mod kafka_consumer;
mod metrics;
mod processor;
mod redis_sink;
mod validation;

use config::Config;
use kafka_consumer::TelemetryConsumer;
use redis_sink::RedisSink;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env();
    let consumer = TelemetryConsumer::new(&config)?;
    let redis_sink = RedisSink::new(&config.redis_url, config.redis_key_prefix.clone())?;

    processor::run(config, consumer, redis_sink).await;
    Ok(())
}
