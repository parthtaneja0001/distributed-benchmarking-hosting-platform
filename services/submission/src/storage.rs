use std::path::PathBuf;
use tokio::fs;
use tokio::io;

pub struct Storage {
    base_path: PathBuf,
}

impl Storage {
    pub fn new(base: &str) -> Self {
        Self {
            base_path: PathBuf::from(base),
        }
    }

    /// Save submission data and return the relative path where it's stored.
    pub async fn store(&self, id: &str, data: &[u8]) -> io::Result<String> {
        let dir = self.base_path.join(id);
        fs::create_dir_all(&dir).await?;
        let file_path = dir.join("submission.tar.gz");
        fs::write(&file_path, data).await?;
        Ok(file_path.to_string_lossy().into_owned())
    }
}