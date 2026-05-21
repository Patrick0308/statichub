use super::trait_::{FileInfo, Storage, StorageError};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;
use std::pin::Pin;
use std::future::Future;

pub struct FilesystemStorage {
    base_path: PathBuf,
}

impl FilesystemStorage {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    fn validate_path(&self, path: &str) -> Result<(), StorageError> {
        if path.contains("..") || path.starts_with('/') {
            return Err(StorageError::InvalidPath(path.to_string()));
        }
        Ok(())
    }

    fn deploy_path(&self, deploy_id: &str) -> PathBuf {
        self.base_path.join(deploy_id)
    }
}

#[async_trait]
impl Storage for FilesystemStorage {
    async fn store_file(
        &self,
        deploy_id: &str,
        path: &str,
        content: &[u8],
    ) -> Result<(), StorageError> {
        self.validate_path(path)?;

        let file_path = self.deploy_path(deploy_id).join(path);

        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&file_path, content).await?;
        Ok(())
    }

    async fn get_file(
        &self,
        deploy_id: &str,
        path: &str,
    ) -> Result<Vec<u8>, StorageError> {
        self.validate_path(path)?;

        let file_path = self.deploy_path(deploy_id).join(path);

        if !file_path.exists() {
            return Err(StorageError::NotFound(path.to_string()));
        }

        let content = fs::read(&file_path).await?;
        Ok(content)
    }

    async fn list_files(
        &self,
        deploy_id: &str,
    ) -> Result<Vec<FileInfo>, StorageError> {
        let deploy_path = self.deploy_path(deploy_id);

        if !deploy_path.exists() {
            return Ok(Vec::new());
        }

        let mut files = Vec::new();
        collect_files(&deploy_path, &deploy_path, &mut files).await?;
        Ok(files)
    }

    async fn delete_deploy(
        &self,
        deploy_id: &str,
    ) -> Result<(), StorageError> {
        let deploy_path = self.deploy_path(deploy_id);

        if deploy_path.exists() {
            fs::remove_dir_all(&deploy_path).await?;
        }

        Ok(())
    }
}

fn collect_files<'a>(
    base: &'a Path,
    current: &'a Path,
    files: &'a mut Vec<FileInfo>,
) -> Pin<Box<dyn Future<Output = Result<(), std::io::Error>> + Send + 'a>> {
    Box::pin(async move {
        let mut entries = fs::read_dir(current).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let metadata = entry.metadata().await?;

            if metadata.is_file() {
                let relative = path.strip_prefix(base).unwrap();
                files.push(FileInfo {
                    path: relative.to_string_lossy().to_string(),
                    size: metadata.len(),
                });
            } else if metadata.is_dir() {
                collect_files(base, &path, files).await?;
            }
        }

        Ok(())
    })
}
