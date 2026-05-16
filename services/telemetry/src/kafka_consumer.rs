use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    error::KafkaError,
    ClientConfig, Message,
};

use crate::{config::Config, events::TelemetryEvent};

pub struct TelemetryConsumer {
    inner: StreamConsumer,
}

impl TelemetryConsumer {
    pub fn new(config: &Config) -> Result<Self, KafkaError> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &config.kafka_broker)
            .set("group.id", &config.group_id)
            .set("enable.auto.commit", "true")
            .set("auto.offset.reset", &config.auto_offset_reset)
            .create()?;

        consumer.subscribe(&[&config.telemetry_topic])?;
        Ok(Self { inner: consumer })
    }

    pub async fn recv(&self) -> Result<TelemetryEvent, String> {
        let message = self
            .inner
            .recv()
            .await
            .map_err(|err| format!("kafka receive failed: {err}"))?;

        let payload = message
            .payload_view::<str>()
            .ok_or_else(|| "telemetry message has no payload".to_string())?
            .map_err(|err| format!("telemetry payload is not UTF-8: {err}"))?;

        serde_json::from_str::<TelemetryEvent>(payload)
            .map_err(|err| format!("telemetry payload is not valid JSON: {err}; payload={payload}"))
    }
}
