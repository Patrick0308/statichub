use super::trait_::{FileInfo, Storage, StorageError};
use async_trait::async_trait;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tokio::fs;

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

    fn validate_deploy_id(&self, deploy_id: &str) -> Result<(), StorageError> {
        if deploy_id.is_empty() || deploy_id.contains("..") || deploy_id.starts_with('/') {
            return Err(StorageError::InvalidPath(format!(
                "Invalid deploy_id: {}",
                deploy_id
            )));
        }
        Ok(())
    }

    fn deploy_path(&self, deploy_id: &str) -> PathBuf {
        self.base_path.join(deploy_id)
    }

    /// Validate that the final resolved path is within base_path
    fn validate_resolved_path(&self, path: &Path) -> Result<(), StorageError> {
        // Canonicalize both paths to resolve any symlinks or relative components
        let base_canonical = self
            .base_path
            .canonicalize()
            .map_err(|e| StorageError::Io(e))?;

        // For paths that don't exist yet, we need to canonicalize the parent
        let path_canonical = if path.exists() {
            path.canonicalize().map_err(|e| StorageError::Io(e))?
        } else {
            // Find the first existing parent
            let mut check_path = path;
            while !check_path.exists() {
                if let Some(parent) = check_path.parent() {
                    check_path = parent;
                } else {
                    // No parent exists, can't validate
                    return Err(StorageError::InvalidPath(format!(
                        "Cannot validate path: {}",
                        path.display()
                    )));
                }
            }

            // Canonicalize the existing parent and append the non-existing parts
            let canonical_parent = check_path.canonicalize().map_err(|e| StorageError::Io(e))?;

            let remaining = path.strip_prefix(check_path).unwrap();
            canonical_parent.join(remaining)
        };

        // Verify the canonical path starts with the canonical base
        if !path_canonical.starts_with(&base_canonical) {
            return Err(StorageError::InvalidPath(format!(
                "Path escapes base directory: {}",
                path.display()
            )));
        }

        Ok(())
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
        self.validate_deploy_id(deploy_id)?;
        self.validate_path(path)?;

        let file_path = self.deploy_path(deploy_id).join(path);
        self.validate_resolved_path(&file_path)?;

        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&file_path, content).await?;
        Ok(())
    }

    async fn get_file(&self, deploy_id: &str, path: &str) -> Result<Vec<u8>, StorageError> {
        self.validate_deploy_id(deploy_id)?;
        self.validate_path(path)?;

        let file_path = self.deploy_path(deploy_id).join(path);
        self.validate_resolved_path(&file_path)?;

        if !file_path.exists() {
            return Err(StorageError::NotFound(path.to_string()));
        }

        let content = fs::read(&file_path).await?;
        Ok(content)
    }

    async fn list_files(&self, deploy_id: &str) -> Result<Vec<FileInfo>, StorageError> {
        self.validate_deploy_id(deploy_id)?;

        let deploy_path = self.deploy_path(deploy_id);
        self.validate_resolved_path(&deploy_path)?;

        if !deploy_path.exists() {
            return Ok(Vec::new());
        }

        let mut files = Vec::new();
        collect_files(&deploy_path, &deploy_path, &mut files).await?;
        Ok(files)
    }

    async fn delete_deploy(&self, deploy_id: &str) -> Result<(), StorageError> {
        self.validate_deploy_id(deploy_id)?;

        let deploy_path = self.deploy_path(deploy_id);
        self.validate_resolved_path(&deploy_path)?;

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
