use statichub_server::storage::{Storage, FilesystemStorage};
use tempfile::TempDir;

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
