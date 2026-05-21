use statichub_server::storage::{Storage, FilesystemStorage, StorageError};
use tempfile::TempDir;
use std::fs;

#[tokio::test]
async fn test_store_and_retrieve_deploy() {
    let temp = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp.path().to_path_buf());

    let deploy_id = "test-project/deploy-1";
    let content = b"hello world";

    // Store file
    storage.store_file(deploy_id, "index.html", content).await.unwrap();

    // Retrieve file
    let retrieved = storage.get_file(deploy_id, "index.html").await.unwrap();
    assert_eq!(retrieved, content);
}

#[tokio::test]
async fn test_list_files_in_deploy() {
    let temp = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp.path().to_path_buf());

    let deploy_id = "test-project/deploy-1";

    storage.store_file(deploy_id, "index.html", b"<html>").await.unwrap();
    storage.store_file(deploy_id, "app.js", b"console.log()").await.unwrap();

    let files = storage.list_files(deploy_id).await.unwrap();
    assert_eq!(files.len(), 2);
}

#[tokio::test]
async fn test_delete_deploy() {
    let temp = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp.path().to_path_buf());

    let deploy_id = "test-project/deploy-1";
    storage.store_file(deploy_id, "index.html", b"<html>").await.unwrap();

    storage.delete_deploy(deploy_id).await.unwrap();

    let result = storage.get_file(deploy_id, "index.html").await;
    assert!(result.is_err());
}

// Security Tests - Path Traversal Prevention

#[tokio::test]
async fn test_path_traversal_in_file_path_rejected() {
    let temp = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp.path().to_path_buf());

    let deploy_id = "test-project/deploy-1";

    // Attempt to traverse up with .. in path
    let result = storage.store_file(deploy_id, "../../../etc/passwd", b"malicious").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));
}

#[tokio::test]
async fn test_path_traversal_in_deploy_id_rejected() {
    let temp = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp.path().to_path_buf());

    // Attempt to traverse up with .. in deploy_id
    let malicious_deploy_id = "../../etc";

    let result = storage.store_file(malicious_deploy_id, "passwd", b"malicious").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));
}

#[tokio::test]
async fn test_delete_deploy_with_path_traversal_rejected() {
    let temp = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp.path().to_path_buf());

    // This is the most dangerous attack - trying to delete arbitrary directories
    let malicious_deploy_id = "../../../tmp/important";

    let result = storage.delete_deploy(malicious_deploy_id).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));
}

#[tokio::test]
async fn test_absolute_path_in_deploy_id_rejected() {
    let temp = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp.path().to_path_buf());

    // Attempt to use absolute path
    let result = storage.store_file("/etc/passwd", "index.html", b"malicious").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));
}

#[tokio::test]
async fn test_absolute_path_in_file_path_rejected() {
    let temp = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp.path().to_path_buf());

    let deploy_id = "test-project/deploy-1";

    // Attempt to use absolute path in file path
    let result = storage.store_file(deploy_id, "/etc/passwd", b"malicious").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));
}

#[tokio::test]
async fn test_empty_deploy_id_rejected() {
    let temp = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp.path().to_path_buf());

    let result = storage.store_file("", "index.html", b"content").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));
}

#[tokio::test]
async fn test_get_file_with_traversal_rejected() {
    let temp = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp.path().to_path_buf());

    let result = storage.get_file("../../etc", "passwd").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));
}

#[tokio::test]
async fn test_list_files_with_traversal_rejected() {
    let temp = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp.path().to_path_buf());

    let result = storage.list_files("../../etc").await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), StorageError::InvalidPath(_)));
}

#[tokio::test]
async fn test_canonical_path_validation() {
    let temp = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp.path().to_path_buf());

    // Create a directory outside the base path
    let outside_dir = temp.path().parent().unwrap().join("outside_base");
    fs::create_dir_all(&outside_dir).unwrap();

    // Try to use a deploy_id that would escape through path normalization
    // Even if we somehow bypass the simple checks, canonical validation should catch it
    let deploy_id = format!("valid/../{}", outside_dir.file_name().unwrap().to_str().unwrap());

    let result = storage.store_file(&deploy_id, "test.txt", b"content").await;
    // Should be rejected either by the .. check or canonical validation
    assert!(result.is_err());

    // Clean up
    fs::remove_dir_all(&outside_dir).ok();
}

#[tokio::test]
async fn test_valid_nested_deploy_ids_allowed() {
    let temp = TempDir::new().unwrap();
    let storage = FilesystemStorage::new(temp.path().to_path_buf());

    // These should all be valid - nested paths within base_path
    let valid_deploy_ids = vec![
        "project-1",
        "project-1/deploy-1",
        "org/project/deploy",
        "deeply/nested/project/structure",
    ];

    for deploy_id in valid_deploy_ids {
        let result = storage.store_file(deploy_id, "index.html", b"content").await;
        assert!(result.is_ok(), "Valid deploy_id '{}' should be allowed", deploy_id);
    }
}
