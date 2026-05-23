# 单文件部署支持

**设计规范**
**日期：** 2026-05-24
**状态：** 草稿

## 概述

支持用户直接部署单个 HTML 文件，而不需要创建目录结构。单个 HTML 文件会被保存为 `index.html`，使其可以通过域名根路径直接访问。

## 问题陈述

### 当前问题

当用户执行 `statichub deploy ~/Downloads/file.html` 时：

1. CLI 将路径传递给 `collect_files()`
2. `collect_files()` 使用 `WalkDir::new(dir)` - 假定输入是目录
3. 如果是文件，`WalkDir` 遍历失败或产生空结果
4. 服务器收到空文件名的请求，返回 400 错误：`{"error":"bad_request","message":"Filename cannot be empty"}`

### 根本原因

`cli/src/upload.rs` 的 `collect_files()` 函数只设计用于处理目录，没有处理单文件路径的逻辑。

## 需求

### 功能性需求

1. **支持单个 HTML 文件部署**
   - 用户可以传递单个 `.html` 文件路径给 `statichub deploy` 命令
   - 文件会被保存为 `index.html`
   - 可以通过域名根路径（`https://xxx.statichub.dev/`）直接访问

2. **文件类型限制**
   - 仅支持 `.html` 和 `.htm` 扩展名的文件（小写）
   - 其他文件类型（`.txt`, `.pdf`, `.js` 等）应返回清晰的错误信息

3. **向后兼容**
   - 不影响现有的目录部署功能
   - 所有现有命令和工作流保持不变

### 非功能性需求

1. **错误消息清晰**
   - 非 HTML 文件应返回明确的错误："Single file deployment only supports .html and .htm files"

2. **性能**
   - 单文件部署应与小型目录部署性能相当

## 解决方案

### 架构设计

**修改范围：** 仅修改 CLI 层，服务器端无需改动。

**核心逻辑：**
- 在 `collect_files()` 函数开始时检测路径是文件还是目录
- 如果是文件：
  - 检查扩展名是否为 `.html` 或 `.htm`
  - 如果不是 `.html` 或 `.htm`，返回错误
  - 如果是 `.html` 或 `.htm`，读取文件内容，创建一个路径为 `index.html` 的 `UploadFile`
- 如果是目录：使用现有的 `WalkDir` 逻辑

### 实现细节

#### 修改文件

**`cli/src/upload.rs`**

修改 `collect_files()` 函数：

```rust
pub fn collect_files(dir: &Path) -> Result<Vec<UploadFile>> {
    let mut files = Vec::new();

    // 新增：检测是否为单文件
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

    // 现有目录处理逻辑保持不变
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
```

#### 不需要修改的文件

- `cli/src/main.rs` - 已经接受 `Option<String>` 作为目录参数
- `cli/src/client.rs` - 已经处理文件数组
- 所有服务器端代码 - 无需修改
- 数据库 schema - 无需修改

## 错误处理

### 错误情况

1. **文件不存在**
   - `File::open(dir)?` 会返回错误
   - 现有的错误传播机制已处理

2. **文件读取失败（权限等）**
   - `read_to_end()` 会返回错误
   - 错误传播处理

3. **非 .html/.htm 文件**
   - 返回明确的错误信息："Single file deployment only supports .html and .htm files. Got: .{extension}"
   - 用户需要部署目录而不是单个文件

4. **无扩展名的文件**
   - 视为非 .html/.htm 文件，返回错误

5. **空 HTML 文件**
   - 允许部署，服务器会提供空的 `index.html`

### 边缘情况

1. **符号链接**
   - `Path::is_file()` 会跟随符号链接，正常处理

2. **大写扩展名（.HTML/.HTM）**
   - 不匹配，返回错误
   - 本次只支持小写扩展名

3. **部署到已有项目**
   - 替换整个项目（创建新版本，只有 index.html）
   - 这是标准的部署行为

## 测试策略

### 单元测试

在 `cli/src/upload.rs` 的 `tests` 模块中添加：

1. **测试单个 HTML 文件部署**

```rust
#[test]
fn test_collect_single_html_file() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("page.html");
    fs::write(&file_path, b"<html><body>Hello</body></html>").unwrap();

    let files = collect_files(&file_path).unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "index.html");
    assert_eq!(files[0].content, b"<html><body>Hello</body></html>");
}
```

2. **测试非 HTML 文件拒绝**

```rust
#[test]
fn test_reject_non_html_file() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("document.pdf");
    fs::write(&file_path, b"PDF content").unwrap();

    let result = collect_files(&file_path);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("only supports .html and .htm files"));
}
```

3. **测试 .htm 文件**

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

4. **测试无扩展名文件**

```rust
#[test]
fn test_reject_no_extension_file() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("noextension");
    fs::write(&file_path, b"content").unwrap();

    let result = collect_files(&file_path);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("only supports .html and .htm files"));
}
```

5. **测试现有目录逻辑不受影响**
   - 使用现有的目录测试用例
   - 确保所有现有测试仍然通过

### 集成测试（可选）

端到端测试：
- 部署单个 HTML 文件到测试服务器
- 验证可以通过域名根路径访问
- 验证内容正确

## 用户体验

### 成功场景

```bash
$ statichub deploy ~/Downloads/page.html
📦 Collecting files from /Users/patrick/Downloads/page.html...
   Found 1 files
🚀 Deploying to https://statichub.dev...
✅ Deploy successful!
   URL: https://x7k2m9.statichub.dev
   Subdomain: x7k2m9
```

访问 `https://x7k2m9.statichub.dev/` 会显示该 HTML 文件。

### 错误场景

```bash
$ statichub deploy ~/Downloads/document.pdf
📦 Collecting files from /Users/patrick/Downloads/document.pdf...
Error: Single file deployment only supports .html and .htm files. Got: .pdf
```

### 部署到已有项目

```bash
$ statichub deploy ~/Downloads/new-page.html --name my-app
📦 Collecting files from /Users/patrick/Downloads/new-page.html...
   Found 1 files
🚀 Deploying to project 'my-app' on https://statichub.dev...
✅ Deploy successful!
   URL: https://my-app.statichub.dev
   Subdomain: my-app
   Version: 5
```

这会创建版本 5，该版本只包含一个文件 `index.html`，替换之前的所有文件。

## 影响范围

### 修改的代码

- **1 个文件修改：** `cli/src/upload.rs`
- **约 20 行新代码**（单文件检测逻辑）
- **4-5 个新单元测试**

### 不影响的部分

- CLI 其他模块
- 服务器端所有代码
- 数据库 schema
- 现有的目录部署功能
- API 端点
- 文件服务逻辑

### 向后兼容性

- ✅ 完全兼容现有目录部署行为
- ✅ 所有现有命令和工作流保持不变
- ✅ 只是新增了单文件支持
- ✅ 无需数据库迁移
- ✅ 无需服务器更新

## 实现计划

1. **修改 `collect_files()` 函数**
   - 添加单文件检测逻辑
   - 添加扩展名验证
   - 返回适当的错误信息

2. **添加单元测试**
   - 单个 HTML 文件成功场景
   - 非 HTML 文件拒绝场景
   - 无扩展名文件场景
   - 确保现有测试通过

3. **手动测试**
   - 部署单个 HTML 文件（匿名）
   - 部署单个 HTML 文件（已认证项目）
   - 尝试部署非 HTML 文件（验证错误）
   - 验证现有目录部署仍然工作

4. **文档更新**（可选）
   - 更新 README.md 示例包含单文件部署
   - 更新 CLI 帮助文本（如果需要）

## 未来扩展（本次不实现）

1. **大小写不敏感支持**
   - 可以扩展为支持 `.HTML`、`.HTM` 等大写扩展名
   - 或支持混合大小写如 `.Html`

2. **智能文件名处理**
   - 如果部署 `about.html`，也可以在 `/about` 访问

3. **自动包装非 HTML 文件**
   - 为 PDF、图片等生成简单的查看页面

这些功能可以在后续迭代中根据用户反馈添加。
