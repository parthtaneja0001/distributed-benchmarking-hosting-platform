use aws_sdk_s3::Client;
use aws_sdk_s3::primitives::ByteStream;
use std::sync::Arc;

#[derive(Clone)]
pub struct Storage {
    client: Arc<Client>,
    bucket: String,
}

impl Storage {
    pub async fn new(bucket: &str) -> Self {
        // Load the base AWS config with endpoint and credentials
        let base_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .endpoint_url("http://localhost:9000")
            .load()
            .await;

        // Build the S3-specific config with path-style addressing
        let s3_config = aws_sdk_s3::config::Builder::from(&base_config)
            .force_path_style(true)
            .build();

        let client = Client::from_conf(s3_config);
        Self {
            client: Arc::new(client),
            bucket: bucket.to_string(),
        }
    }

    pub async fn store(&self, id: &str, data: &[u8]) -> anyhow::Result<String> {
        let key = format!("{}/submission.tar.gz", id);
        let body = ByteStream::from(data.to_vec());

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(body)
            .send()
            .await?;

        Ok(key)
    }
}