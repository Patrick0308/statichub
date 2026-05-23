use anyhow::{Context, Result};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use walkdir::WalkDir;

/// Represents a file to be uploaded
#[derive(Debug)]
pub struct UploadFile {
    pub path: String,
    pub content: Vec<u8>,
}

/// Collect files from a directory, excluding common patterns
pub fn collect_files(dir: &Path) -> Result<Vec<UploadFile>> {
    let mut files = Vec::new();

    // Detect if the path is a single file
    if dir.is_file() {
        // Check file extension
        let extension = dir.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        if extension != "html" && extension != "htm" {
            anyhow::bail!(
                "Single file deployment only supports .html and .htm files. Got: .{}",
                extension
            );
        }

        let mut content = Vec::new();
        File::open(dir)
            .with_context(|| format!("Failed to open file: {}", dir.display()))?
            .read_to_end(&mut content)
            .with_context(|| format!("Failed to read file: {}", dir.display()))?;

        files.push(UploadFile {
            path: "index.html".to_string(),
            content,
        });

        return Ok(files);
    }

    // Continue with existing directory processing logic...
    for entry in WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_excluded(e.path(), dir))
    {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let relative_path = path
                .strip_prefix(dir)
                .context("Failed to get relative path")?
                .to_string_lossy()
                .to_string();

            let mut content = Vec::new();
            File::open(path)?.read_to_end(&mut content)?;

            files.push(UploadFile {
                path: relative_path,
                content,
            });
        }
    }

    Ok(files)
}

/// Check if a path should be excluded from upload
fn is_excluded(path: &Path, base_dir: &Path) -> bool {
    // Don't exclude the base directory itself
    if path == base_dir {
        return false;
    }

    // Exclude hidden files and directories
    if path.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
    {
        return true;
    }

    // Exclude common directories
    // Note: .git, .svn, .hg are already caught by hidden-file check above
    let excluded_dirs = [
        "node_modules",
        "target",
        "dist",
        "build",
    ];

    // Check if the directory name itself is excluded
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if excluded_dirs.contains(&name) {
            return true;
        }
    }

    false
}

/// Create a gzipped tarball from files
#[allow(dead_code)]
pub fn create_tarball(files: &[UploadFile]) -> Result<Vec<u8>> {
    use flate2::write::GzEncoder;
    use flate2::Compression;

    let mut buffer = Vec::new();
    let encoder = GzEncoder::new(&mut buffer, Compression::default());
    let mut archive = tar::Builder::new(encoder);

    for file in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(file.content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();

        archive
            .append_data(&mut header, &file.path, &file.content[..])
            .with_context(|| format!("Failed to add file to archive: {}", file.path))?;
    }

    let encoder = archive
        .into_inner()
        .context("Failed to finish tar archive")?;

    encoder.finish().context("Failed to finish gzip compression")?;

    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_collect_files() {
        let temp = TempDir::new().unwrap();

        // Create test files
        fs::write(temp.path().join("index.html"), b"<html>").unwrap();
        fs::write(temp.path().join("style.css"), b"body {}").unwrap();
        fs::create_dir(temp.path().join("js")).unwrap();
        fs::write(temp.path().join("js/app.js"), b"console.log()").unwrap();

        let files = collect_files(temp.path()).unwrap();

        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|f| f.path == "index.html"));
        assert!(files.iter().any(|f| f.path == "style.css"));
        assert!(files.iter().any(|f| f.path == "js/app.js"));
    }

    #[test]
    fn test_exclude_hidden_files() {
        let temp = TempDir::new().unwrap();

        fs::write(temp.path().join("index.html"), b"<html>").unwrap();
        fs::write(temp.path().join(".gitignore"), b"node_modules").unwrap();
        fs::create_dir(temp.path().join(".git")).unwrap();
        fs::write(temp.path().join(".git/config"), b"").unwrap();

        let files = collect_files(temp.path()).unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "index.html");
    }

    #[test]
    fn test_exclude_named_directories() {
        let temp = TempDir::new().unwrap();

        fs::write(temp.path().join("index.html"), b"<html>").unwrap();

        // Create directories that should be excluded
        fs::create_dir(temp.path().join("node_modules")).unwrap();
        fs::write(temp.path().join("node_modules/package.json"), b"{}").unwrap();

        fs::create_dir(temp.path().join("target")).unwrap();
        fs::write(temp.path().join("target/build.txt"), b"build").unwrap();

        fs::create_dir(temp.path().join("dist")).unwrap();
        fs::write(temp.path().join("dist/bundle.js"), b"code").unwrap();

        let files = collect_files(temp.path()).unwrap();

        // Should only include index.html, all excluded directories should be skipped
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "index.html");
    }

    #[test]
    fn test_create_tarball() {
        let files = vec![
            UploadFile {
                path: "index.html".to_string(),
                content: b"<html>".to_vec(),
            },
            UploadFile {
                path: "app.js".to_string(),
                content: b"console.log()".to_vec(),
            },
        ];

        let tarball = create_tarball(&files).unwrap();

        // Tarball should be compressed (non-empty and reasonable size)
        assert!(!tarball.is_empty());
        assert!(tarball.len() < 500); // Reasonable size for small files with headers
    }

    #[test]
    fn test_collect_single_html_file() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("page.html");
        fs::write(&file_path, b"<html><body>Hello World</body></html>").unwrap();

        let files = collect_files(&file_path).unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "index.html");
        assert_eq!(files[0].content, b"<html><body>Hello World</body></html>");
    }

    #[test]
    fn test_collect_single_htm_file() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("page.htm");
        fs::write(&file_path, b"<html><body>HTM file</body></html>").unwrap();

        let files = collect_files(&file_path).unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "index.html");
        assert_eq!(files[0].content, b"<html><body>HTM file</body></html>");
    }

    #[test]
    fn test_reject_non_html_file() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("document.pdf");
        fs::write(&file_path, b"PDF content").unwrap();

        let result = collect_files(&file_path);

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("only supports .html and .htm files"));
        assert!(error_msg.contains("Got: .pdf"));
    }

    #[test]
    fn test_reject_txt_file() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("readme.txt");
        fs::write(&file_path, b"text content").unwrap();

        let result = collect_files(&file_path);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("only supports .html and .htm files"));
    }

    #[test]
    fn test_reject_js_file() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("app.js");
        fs::write(&file_path, b"console.log('test')").unwrap();

        let result = collect_files(&file_path);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("only supports .html and .htm files"));
    }
}
