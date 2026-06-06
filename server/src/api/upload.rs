use crate::{
    error::{AppError, Result},
    markdown,
    storage::Storage,
};
use axum::extract::Multipart;
use std::sync::Arc;

const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;
const MAX_TOTAL_SIZE: u64 = 500 * 1024 * 1024;
const MAX_FILE_COUNT: i64 = 1000;

pub struct ProcessedUpload {
    pub file_count: i64,
    pub total_size: u64,
}

struct UploadItem {
    filename: String,
    data: Vec<u8>,
}

pub async fn process_multipart_files(
    multipart: &mut Multipart,
    storage: &Arc<dyn Storage>,
    storage_path: &str,
) -> Result<ProcessedUpload> {
    let mut items = collect_upload_items(multipart).await?;

    if is_single_markdown_deploy(&items) {
        let html =
            markdown::render_markdown_document(&items[0].data).map_err(AppError::BadRequest)?;
        items.push(UploadItem {
            filename: "index.html".to_string(),
            data: html,
        });
    }

    let total_size = total_stored_size(&items)?;
    let file_count = items.len() as i64;

    for item in items {
        storage
            .store_file(storage_path, &item.filename, &item.data)
            .await
            .map_err(|e| AppError::Storage(e.to_string()))?;
    }

    Ok(ProcessedUpload {
        file_count,
        total_size,
    })
}

async fn collect_upload_items(multipart: &mut Multipart) -> Result<Vec<UploadItem>> {
    let mut items = Vec::new();
    let mut total_size = 0u64;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart data: {}", e)))?
    {
        let filename = match field.file_name() {
            Some(name) => name.to_string(),
            None => continue,
        };

        let sanitized_filename = sanitize_filename(&filename)?;
        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(format!("Failed to read file data: {}", e)))?;

        if data.len() as u64 > MAX_FILE_SIZE {
            return Err(AppError::BadRequest(format!(
                "File '{}' exceeds maximum size of 100MB",
                sanitized_filename
            )));
        }

        total_size += data.len() as u64;
        if total_size > MAX_TOTAL_SIZE {
            return Err(AppError::BadRequest(
                "Total upload size exceeds maximum of 500MB".to_string(),
            ));
        }

        items.push(UploadItem {
            filename: sanitized_filename,
            data: data.to_vec(),
        });

        if items.len() as i64 > MAX_FILE_COUNT {
            return Err(AppError::BadRequest(
                "Too many files (maximum 1000)".to_string(),
            ));
        }
    }

    Ok(items)
}

fn is_single_markdown_deploy(items: &[UploadItem]) -> bool {
    items.len() == 1 && items[0].filename == "index.md"
}

fn total_stored_size(items: &[UploadItem]) -> Result<u64> {
    items.iter().try_fold(0u64, |total, item| {
        let next = total
            .checked_add(item.data.len() as u64)
            .ok_or_else(|| AppError::BadRequest("Total upload size is too large".to_string()))?;
        if next > MAX_TOTAL_SIZE {
            return Err(AppError::BadRequest(
                "Total upload size exceeds maximum of 500MB".to_string(),
            ));
        }
        Ok(next)
    })
}

fn sanitize_filename(filename: &str) -> Result<String> {
    if filename.trim().is_empty() {
        return Err(AppError::BadRequest("Filename cannot be empty".to_string()));
    }

    if filename.contains("..") {
        return Err(AppError::BadRequest(format!(
            "Invalid filename: '{}' contains directory traversal",
            filename
        )));
    }

    if filename.starts_with('/') || filename.starts_with('\\') {
        return Err(AppError::BadRequest(format!(
            "Invalid filename: '{}' cannot be an absolute path",
            filename
        )));
    }

    let normalized = filename.replace('\\', "/");

    for component in normalized.split('/') {
        if component.starts_with('.') {
            return Err(AppError::BadRequest(format!(
                "Invalid filename: '{}' contains hidden file or directory",
                filename
            )));
        }
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename_valid() {
        assert!(sanitize_filename("index.html").is_ok());
        assert!(sanitize_filename("styles.css").is_ok());
        assert!(sanitize_filename("script.js").is_ok());
        assert!(sanitize_filename("css/styles.css").is_ok());
        assert!(sanitize_filename("js/app.js").is_ok());
        assert!(sanitize_filename("assets/images/logo.png").is_ok());
    }

    #[test]
    fn test_sanitize_filename_invalid() {
        assert!(sanitize_filename("../etc/passwd").is_err());
        assert!(sanitize_filename("..\\windows\\system32").is_err());
        assert!(sanitize_filename("dir/../file.txt").is_err());
        assert!(sanitize_filename("").is_err());
        assert!(sanitize_filename("   ").is_err());
        assert!(sanitize_filename(".htaccess").is_err());
        assert!(sanitize_filename("dir/.hidden").is_err());
    }
}
