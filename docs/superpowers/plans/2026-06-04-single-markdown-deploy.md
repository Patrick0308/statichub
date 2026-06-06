# Single Markdown File Deploy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Support `statichub deploy README.md` by uploading Markdown as `index.md`, rendering it on the server during deployment, and storing generated `index.html` beside the original source.

**Architecture:** The CLI only recognizes single Markdown files and uploads them as `index.md`. Server deploy handling moves multipart processing into a shared upload module used by anonymous and authenticated deploys; that module expands the special single-`index.md` upload into stored `index.md` plus rendered `index.html`. Existing static serving remains unchanged because `/` already resolves to `index.html`.

**Tech Stack:** Rust, Axum multipart, existing `Storage` trait, `pulldown-cmark` for Markdown rendering, `cargo test` for validation.

---

## File Structure

- Modify `cli/src/upload.rs`: allow single `.md` and `.markdown` inputs and add unit tests.
- Modify `server/Cargo.toml`: add `pulldown-cmark`.
- Create `server/src/markdown.rs`: render Markdown bytes into a deterministic HTML document.
- Create `server/src/api/upload.rs`: shared multipart collection, limits, filename sanitization, Markdown expansion, and storage.
- Modify `server/src/api/mod.rs`: register the new upload module.
- Modify `server/src/api/deploys.rs`: use shared upload processing for anonymous deploys.
- Modify `server/src/api/projects.rs`: use shared upload processing for authenticated deploys and remove duplicated upload code.
- Modify `server/src/lib.rs`: expose `markdown` to integration tests if needed.
- Modify `server/tests/api_test.rs`: cover anonymous Markdown deploy storage and invalid UTF-8 failure.
- Modify `server/tests/serve_tests.rs`: cover serving generated HTML at `/` and original Markdown at `/index.md`.
- Update `Cargo.lock` through Cargo.

### Task 1: CLI Single Markdown Collection

**Files:**
- Modify: `cli/src/upload.rs`

- [x] **Step 1: Write failing CLI tests**

Add tests inside `cli/src/upload.rs`:

```rust
#[test]
fn test_collect_single_md_file() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("README.md");
    fs::write(&file_path, b"# Hello\n\nMarkdown body").unwrap();

    let files = collect_files(&file_path).unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "index.md");
    assert_eq!(files[0].content, b"# Hello\n\nMarkdown body");
}

#[test]
fn test_collect_single_markdown_file() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("README.markdown");
    fs::write(&file_path, b"# Long Extension").unwrap();

    let files = collect_files(&file_path).unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "index.md");
    assert_eq!(files[0].content, b"# Long Extension");
}
```

Update unsupported-extension assertions to check for:

```rust
"only supports .html, .htm, .md, and .markdown files"
```

- [x] **Step 2: Run failing CLI test**

Run:

```bash
cargo test -p statichub upload::tests::test_collect_single_md_file
```

Expected: fail because single `.md` files are still rejected.

- [x] **Step 3: Implement CLI extension mapping**

Update the single-file branch in `collect_files()` to map extensions:

```rust
let upload_path = match extension {
    "html" | "htm" => "index.html",
    "md" | "markdown" => "index.md",
    _ => anyhow::bail!(
        "Single file deployment only supports .html, .htm, .md, and .markdown files. Got: .{}",
        extension
    ),
};
```

Use `upload_path.to_string()` when pushing the `UploadFile`.

- [x] **Step 4: Run CLI package tests**

Run:

```bash
cargo test -p statichub
```

Expected: pass.

- [x] **Step 5: Commit CLI change**

Run:

```bash
git add cli/src/upload.rs
git commit -m "Support single Markdown file collection"
```

### Task 2: Markdown Renderer

**Files:**
- Modify: `server/Cargo.toml`
- Modify: `server/src/lib.rs`
- Create: `server/src/markdown.rs`

- [x] **Step 1: Add renderer tests**

Create `server/src/markdown.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_heading_and_paragraph() {
        let html = render_markdown_document("# Hello\n\nThis is **bold**.").unwrap();

        assert!(html.contains("<title>Hello</title>"));
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.starts_with("<!doctype html>"));
    }

    #[test]
    fn rejects_non_utf8_markdown() {
        let err = render_markdown_document(&[0xff, 0xfe]).unwrap_err();

        assert!(err.contains("Markdown file must be valid UTF-8"));
    }
}
```

- [x] **Step 2: Run failing renderer tests**

Run:

```bash
cargo test -p statichub-server markdown::tests
```

Expected: fail because the module is not wired and the dependency is missing.

- [x] **Step 3: Add dependency and module**

Add to `server/Cargo.toml`:

```toml
pulldown-cmark = "0.10"
```

Add to `server/src/lib.rs`:

```rust
pub mod markdown;
```

- [x] **Step 4: Implement renderer**

Implement `render_markdown_document(content: &[u8]) -> std::result::Result<Vec<u8>, String>` using `pulldown_cmark::{html, Options, Parser}`. Extract the first line beginning with `# ` as the title, escape title text with a small helper, and wrap rendered body in a deterministic HTML shell with embedded CSS.

- [x] **Step 5: Run renderer tests**

Run:

```bash
cargo test -p statichub-server markdown::tests
```

Expected: pass.

- [x] **Step 6: Commit renderer**

Run:

```bash
git add server/Cargo.toml Cargo.lock server/src/lib.rs server/src/markdown.rs
git commit -m "Add server Markdown renderer"
```

### Task 3: Shared Server Upload Processing

**Files:**
- Create: `server/src/api/upload.rs`
- Modify: `server/src/api/mod.rs`
- Modify: `server/src/api/deploys.rs`
- Modify: `server/src/api/projects.rs`

- [x] **Step 1: Add anonymous Markdown deploy tests**

Add tests in `server/tests/api_test.rs` that post one multipart file named `index.md`, assert `StatusCode::OK`, find the created project/deploy from the response, and verify storage contains both `index.md` and `index.html`. Assert deploy metadata has `file_count == 2`.

- [x] **Step 2: Run failing anonymous deploy test**

Run:

```bash
cargo test -p statichub-server test_anonymous_markdown_deploy_renders_and_stores_source
```

Expected: fail because the server only stores `index.md`.

- [x] **Step 3: Create shared upload module**

Create `server/src/api/upload.rs` with:

```rust
pub struct ProcessedUpload {
    pub file_count: i64,
    pub total_size: u64,
}
```

Add `process_multipart_files(multipart, storage, storage_path) -> Result<ProcessedUpload>`. It should collect fields into an internal `UploadItem { filename: String, data: bytes::Bytes }`, enforce current limits, sanitize filenames, and then call an expansion helper. If the collected files are exactly one item named `index.md`, render HTML and store both `index.md` and `index.html`; otherwise store the original list unchanged.

- [x] **Step 4: Register module and use it**

Add `mod upload;` to `server/src/api/mod.rs`.

In `server/src/api/deploys.rs` and `server/src/api/projects.rs`, replace local `process_multipart_files()` calls with:

```rust
let upload_result = super::upload::process_multipart_files(
    &mut multipart,
    &state.storage,
    &storage_path,
)
.await;
```

Use returned `ProcessedUpload` for `Deploy::update_status()`.

Remove duplicated private `process_multipart_files()` and `sanitize_filename()` from both files.

- [x] **Step 5: Run anonymous deploy test**

Run:

```bash
cargo test -p statichub-server test_anonymous_markdown_deploy_renders_and_stores_source
```

Expected: pass.

- [x] **Step 6: Commit shared upload processing**

Run:

```bash
git add server/src/api/upload.rs server/src/api/mod.rs server/src/api/deploys.rs server/src/api/projects.rs server/tests/api_test.rs
git commit -m "Render single Markdown uploads during deploy"
```

### Task 4: Failure and Non-Special Cases

**Files:**
- Modify: `server/tests/api_test.rs`
- Modify: `server/src/api/upload.rs`

- [x] **Step 1: Add focused tests**

Add tests for:

```rust
#[tokio::test]
async fn test_markdown_deploy_rejects_non_utf8_source() { /* post index.md with bytes [0xff, 0xfe] and expect 400 */ }

#[tokio::test]
async fn test_markdown_in_multi_file_deploy_is_not_rendered() { /* post index.md and app.css; expect no generated index.html unless it was uploaded */ }
```

- [x] **Step 2: Run focused tests**

Run:

```bash
cargo test -p statichub-server markdown_deploy
```

Expected: pass after Task 3, or expose small fixes in error mapping or special-case detection.

- [x] **Step 3: Fix behavior if needed**

If needed, ensure Markdown render errors become `AppError::BadRequest` and special handling requires `items.len() == 1 && items[0].filename == "index.md"`.

- [x] **Step 4: Commit edge tests**

Run:

```bash
git add server/src/api/upload.rs server/tests/api_test.rs
git commit -m "Cover Markdown deploy edge cases"
```

### Task 5: Serving Verification

**Files:**
- Modify: `server/tests/serve_tests.rs`

- [x] **Step 1: Add serving test**

Add a test that creates a project/deploy, stores `index.md` and generated `index.html`, points the project at the deploy, requests `/`, and asserts `content-type` is `text/html` and the body contains rendered HTML. Request `/index.md` and assert the original Markdown bytes are returned.

- [x] **Step 2: Run serving tests**

Run:

```bash
cargo test -p statichub-server --test serve_tests
```

Expected: pass because serving already resolves `index.html`.

- [x] **Step 3: Commit serving coverage**

Run:

```bash
git add server/tests/serve_tests.rs
git commit -m "Verify serving Markdown deploy artifacts"
```

### Task 6: Workspace Validation

**Files:**
- Verify all touched files.

- [x] **Step 1: Format Rust code**

Run:

```bash
cargo fmt
```

- [x] **Step 2: Run targeted tests**

Run:

```bash
cargo test -p statichub
cargo test -p statichub-server markdown_deploy
cargo test -p statichub-server --test serve_tests
```

Expected: pass.

- [x] **Step 3: Run workspace tests**

Run:

```bash
cargo test --workspace
```

Expected: pass.

- [x] **Step 4: Commit any formatting or lockfile drift**

Run if `git status --short` shows tracked changes:

```bash
git add Cargo.lock cli/src/upload.rs server/Cargo.toml server/src server/tests
git commit -m "Polish single Markdown deploy support"
```

## Self-Review

- Spec coverage: CLI single-file Markdown support is Task 1; server deploy-time render and source preservation are Tasks 2-4; serving through existing `index.html` is Task 5; validation is Task 6.
- Placeholder scan: no placeholders remain; tests and commands are concrete.
- Type consistency: `ProcessedUpload { file_count, total_size }` is introduced before use, and both deploy endpoints consume the same upload helper.
