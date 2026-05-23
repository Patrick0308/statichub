# 单文件部署支持 - 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 支持直接部署单个 HTML 文件（.html 或 .htm），文件将被保存为 index.html 并可通过根路径访问。

**Architecture:** 仅修改 CLI 的 `collect_files()` 函数，在开始时添加单文件检测逻辑。检测到单文件时验证扩展名为 .html 或 .htm，然后将内容包装为路径为 "index.html" 的 UploadFile。服务器端无需改动。

**Tech Stack:** Rust, std::path::Path, std::fs::File

---

## 文件结构

**修改文件：**
- `cli/src/upload.rs` - 修改 `collect_files()` 函数，添加单文件支持和扩展名验证

**测试文件：**
- `cli/src/upload.rs` - 在现有的 `#[cfg(test)] mod tests` 模块中添加新测试

---

## 任务分解

### Task 1: 添加单个 .html 文件测试和实现

**Files:**
- Modify: `cli/src/upload.rs`
- Test: `cli/src/upload.rs` (tests 模块)

- [ ] **Step 1: 写失败的测试**

在 `cli/src/upload.rs` 的 `tests` 模块末尾添加：

```rust
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
```

- [ ] **Step 2: 运行测试验证失败**

```bash
cargo test -p statichub test_collect_single_html_file
```

Expected output: 测试失败，因为 `collect_files()` 尚未处理单文件情况

- [ ] **Step 3: 实现单文件检测逻辑**

在 `cli/src/upload.rs` 的 `collect_files()` 函数开始处（在 `let mut files = Vec::new();` 之后）添加：

```rust
pub fn collect_files(dir: &Path) -> Result<Vec<UploadFile>> {
    let mut files = Vec::new();

    // 检测是否为单文件
    if dir.is_file() {
        // 检查文件扩展名
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
        File::open(dir)?.read_to_end(&mut content)?;

        files.push(UploadFile {
            path: "index.html".to_string(),
            content,
        });

        return Ok(files);
    }

    // 现有目录处理逻辑保持不变...
    for entry in WalkDir::new(dir)
```

- [ ] **Step 4: 运行测试验证通过**

```bash
cargo test -p statichub test_collect_single_html_file
```

Expected output: 测试通过

- [ ] **Step 5: 确保现有测试仍然通过**

```bash
cargo test -p statichub upload
```

Expected output: 所有现有的 upload 测试通过

- [ ] **Step 6: 提交**

```bash
git add cli/src/upload.rs
git commit -m "feat(cli): add single HTML file deployment support

- Detect single file vs directory in collect_files()
- Validate .html and .htm extensions
- Save single files as index.html for root path access"
```

---

### Task 2: 添加 .htm 文件测试

**Files:**
- Modify: `cli/src/upload.rs`
- Test: `cli/src/upload.rs` (tests 模块)

- [ ] **Step 1: 写 .htm 文件测试**

在 `cli/src/upload.rs` 的 `tests` 模块中添加：

```rust
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
```

- [ ] **Step 2: 运行测试验证通过**

```bash
cargo test -p statichub test_collect_single_htm_file
```

Expected output: 测试应该通过（因为 Task 1 已经支持 .htm）

- [ ] **Step 3: 提交**

```bash
git add cli/src/upload.rs
git commit -m "test(cli): add test for .htm file deployment"
```

---

### Task 3: 添加非 HTML 文件拒绝测试

**Files:**
- Modify: `cli/src/upload.rs`
- Test: `cli/src/upload.rs` (tests 模块)

- [ ] **Step 1: 写拒绝非 HTML 文件的测试**

在 `cli/src/upload.rs` 的 `tests` 模块中添加：

```rust
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
```

- [ ] **Step 2: 运行测试验证通过**

```bash
cargo test -p statichub test_reject_non_html_file
```

Expected output: 测试应该通过（Task 1 已实现验证逻辑）

- [ ] **Step 3: 添加更多文件类型测试**

在同一测试模块添加：

```rust
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
```

- [ ] **Step 4: 运行新测试验证通过**

```bash
cargo test -p statichub test_reject_txt_file test_reject_js_file
```

Expected output: 两个测试都通过

- [ ] **Step 5: 提交**

```bash
git add cli/src/upload.rs
git commit -m "test(cli): add tests for rejecting non-HTML file types"
```

---

### Task 4: 添加无扩展名文件测试

**Files:**
- Modify: `cli/src/upload.rs`
- Test: `cli/src/upload.rs` (tests 模块)

- [ ] **Step 1: 写无扩展名文件测试**

在 `cli/src/upload.rs` 的 `tests` 模块中添加：

```rust
#[test]
fn test_reject_no_extension_file() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("noextension");
    fs::write(&file_path, b"some content").unwrap();

    let result = collect_files(&file_path);

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("only supports .html and .htm files"));
    assert!(error_msg.contains("Got: ."));
}
```

- [ ] **Step 2: 运行测试验证通过**

```bash
cargo test -p statichub test_reject_no_extension_file
```

Expected output: 测试通过

- [ ] **Step 3: 提交**

```bash
git add cli/src/upload.rs
git commit -m "test(cli): add test for rejecting files without extension"
```

---

### Task 5: 添加空 HTML 文件测试

**Files:**
- Modify: `cli/src/upload.rs`
- Test: `cli/src/upload.rs` (tests 模块)

- [ ] **Step 1: 写空 HTML 文件测试**

在 `cli/src/upload.rs` 的 `tests` 模块中添加：

```rust
#[test]
fn test_collect_empty_html_file() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("empty.html");
    fs::write(&file_path, b"").unwrap();

    let files = collect_files(&file_path).unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "index.html");
    assert_eq!(files[0].content, b"");
}
```

- [ ] **Step 2: 运行测试验证通过**

```bash
cargo test -p statichub test_collect_empty_html_file
```

Expected output: 测试通过（允许空文件）

- [ ] **Step 3: 提交**

```bash
git add cli/src/upload.rs
git commit -m "test(cli): add test for empty HTML file deployment"
```

---

### Task 6: 运行完整测试套件

**Files:**
- Test: 所有 CLI 测试

- [ ] **Step 1: 运行完整的 CLI 测试套件**

```bash
cargo test -p statichub
```

Expected output: 所有测试通过

- [ ] **Step 2: 运行 workspace 级别的测试**

```bash
cargo test --workspace
```

Expected output: 所有测试通过（确保没有破坏其他模块）

- [ ] **Step 3: 检查编译警告**

```bash
cargo clippy -p statichub
```

Expected output: 无警告或错误

---

### Task 7: 手动集成测试

**Files:**
- Test: 手动测试完整的部署流程

- [ ] **Step 1: 创建测试 HTML 文件**

```bash
echo '<html><body><h1>Test Single File Deploy</h1></body></html>' > /tmp/test-deploy.html
```

- [ ] **Step 2: 测试匿名单文件部署**

```bash
cargo run -p statichub -- deploy /tmp/test-deploy.html
```

Expected output:
```
📦 Collecting files from /tmp/test-deploy.html...
   Found 1 files
🚀 Deploying to https://statichub.dev...
✅ Deploy successful!
   URL: https://xxxxx.statichub.dev
   Subdomain: xxxxx
```

- [ ] **Step 3: 验证部署的内容可访问**

访问上一步返回的 URL（或使用 curl）：

```bash
curl https://xxxxx.statichub.dev/
```

Expected output: 返回 HTML 内容

- [ ] **Step 4: 测试 .htm 文件**

```bash
echo '<html><body><h1>HTM Test</h1></body></html>' > /tmp/test-deploy.htm
cargo run -p statichub -- deploy /tmp/test-deploy.htm
```

Expected output: 部署成功

- [ ] **Step 5: 测试拒绝非 HTML 文件**

```bash
echo 'Not HTML' > /tmp/test.txt
cargo run -p statichub -- deploy /tmp/test.txt
```

Expected output:
```
📦 Collecting files from /tmp/test.txt...
Error: Single file deployment only supports .html and .htm files. Got: .txt
```

- [ ] **Step 6: 测试已认证项目部署（如果已登录）**

如果已经登录：

```bash
cargo run -p statichub -- deploy /tmp/test-deploy.html --name test-single-file
```

Expected output: 部署成功到命名项目

- [ ] **Step 7: 测试现有目录部署仍然工作**

```bash
mkdir -p /tmp/test-dir
echo '<html><body>Dir deploy</body></html>' > /tmp/test-dir/index.html
cargo run -p statichub -- deploy /tmp/test-dir
```

Expected output: 目录部署成功（确保没有破坏现有功能）

- [ ] **Step 8: 清理测试文件**

```bash
rm /tmp/test-deploy.html /tmp/test-deploy.htm /tmp/test.txt
rm -rf /tmp/test-dir
```

---

### Task 8: 更新文档（可选）

**Files:**
- Modify: `README.md`

- [ ] **Step 1: 在 README 的 Quick Start 部分添加单文件部署示例**

在 "Deploy Anonymously" 部分添加：

```markdown
# Deploy a single HTML file
statichub deploy ~/my-page.html
```

位置：在 `README.md` 第 22-30 行的示例之后

- [ ] **Step 2: 提交文档更新**

```bash
git add README.md
git commit -m "docs: add single file deployment example to README"
```

---

## 自审清单

### 1. 规范覆盖检查

从设计规范 `docs/superpowers/specs/2026-05-24-single-file-deploy-design.md` 检查：

- ✅ **支持单个 HTML 文件部署** → Task 1 (test + implementation)
- ✅ **支持 .html 和 .htm 扩展名** → Task 1 (implementation), Task 2 (test)
- ✅ **拒绝非 HTML 文件** → Task 3 (tests)
- ✅ **拒绝无扩展名文件** → Task 4 (test)
- ✅ **允许空 HTML 文件** → Task 5 (test)
- ✅ **清晰的错误消息** → Task 1 (implementation), Task 3-4 (verification)
- ✅ **不影响现有目录部署** → Task 1 Step 5, Task 6, Task 7 Step 7
- ✅ **集成测试** → Task 7 (manual testing)
- ✅ **文档更新** → Task 8

无遗漏的需求。

### 2. Placeholder 扫描

搜索计划中的占位符模式：
- ❌ 无 "TBD", "TODO", "implement later"
- ❌ 无 "add appropriate error handling" 等模糊描述
- ❌ 无 "类似于 Task N" 的引用
- ✅ 所有代码步骤都包含完整代码
- ✅ 所有命令都有预期输出

### 3. 类型一致性检查

检查跨任务的一致性：
- ✅ `UploadFile` 结构体字段 `path` 和 `content` 在所有任务中一致
- ✅ 错误消息 "only supports .html and .htm files" 在所有任务中一致
- ✅ 文件路径 `"index.html"` 在所有任务中一致
- ✅ 扩展名检查 `extension != "html" && extension != "htm"` 在实现和测试中一致

无类型不一致问题。

---

## 实现计划完成

计划已保存到 `docs/superpowers/plans/2026-05-24-single-file-deploy.md`。

**预计时间：** 约 30-45 分钟（包括测试和手动验证）

**风险：** 低 - 改动范围小，测试覆盖完整，不影响服务器端
