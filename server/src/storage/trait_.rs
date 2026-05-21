use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: String,
    pub size: u64,
}

#[async_trait]
pub trait Storage: Send + Sync {
    /// Store a single file in a deploy
    async fn store_file(
        &self,
        deploy_id: &str,
        path: &str,
        content: &[u8],
    ) -> Result<(), StorageError>;

    /// Get a file from a deploy
    async fn get_file(
        &self,
        deploy_id: &str,
        path: &str,
    ) -> Result<Vec<u8>, StorageError>;

    /// List all files in a deploy
    async fn list_files(
        &self,
        deploy_id: &str,
    ) -> Result<Vec<FileInfo>, StorageError>;

    /// Delete an entire deploy
    async fn delete_deploy(
        &self,
        deploy_id: &str,
    ) -> Result<(), StorageError>;
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("File not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid path: {0}")]
    InvalidPath(String),
}
