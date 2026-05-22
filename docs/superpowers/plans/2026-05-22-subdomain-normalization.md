# Subdomain 存储标准化实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 统一数据库中 subdomain 字段格式，只存储标识符，完整 URL 在应用层动态构建

**Architecture:** 在 shared 模块添加 URL 构建辅助函数，修改所有创建项目和构建 URL 的代码使用该函数，简化查找逻辑移除 fallback，清空测试数据重新开始

**Tech Stack:** Rust, Axum, SQLx, SQLite

---

## 文件结构

**需要修改的文件：**
- `shared/src/lib.rs` - 添加 `build_project_url()` 公共函数
- `shared/src/types.rs` - 添加 URL 构建函数的测试
- `server/src/models/project.rs` - 修改创建项目时的 subdomain 赋值
- `server/src/api/deploys.rs` - 修改匿名部署 URL 构建
- `server/src/api/projects.rs` - 修改命名项目部署 URL 构建
- `server/src/api/serve.rs` - 简化查找逻辑
- `server/src/api/management.rs` - 修改项目列表 URL 构建

---

### Task 1: 添加 URL 构建辅助函数

**Files:**
- Modify: `shared/src/lib.rs`
- Test: `shared/src/lib.rs` (内联测试)

- [ ] **Step 1: 添加 build_project_url 函数到 shared/src/lib.rs**

在文件末尾添加：

```rust
/// Build full project URL from subdomain identifier and base URL
///
/// # Examples
///
/// ```
/// use statichub_shared::build_project_url;
///
/// // Development
/// let url = build_project_url("my-app", "http://localhost:3000");
/// assert_eq!(url, "https://my-app.localhost:3000");
///
/// // Production
/// let url = build_project_url("my-app", "https://statichub.io");
/// assert_eq!(url, "https://my-app.statichub.io");
/// ```
pub fn build_project_url(subdomain: &str, base_url: &str) -> String {
    let domain = base_url
        .trim_start_matches("http://")
        .trim_start_matches("https://");

    format!("https://{}.{}", subdomain, domain)
}

#[cfg(test)]
mod url_tests {
    use super::*;

    #[test]
    fn test_build_project_url_localhost() {
        assert_eq!(
            build_project_url("test-app", "http://localhost:3000"),
            "https://test-app.localhost:3000"
        );
    }

    #[test]
    fn test_build_project_url_production() {
        assert_eq!(
            build_project_url("my-project", "https://statichub.io"),
            "https://my-project.statichub.io"
        );
    }

    #[test]
    fn test_build_project_url_custom_domain() {
        assert_eq!(
            build_project_url("app", "https://custom.com"),
            "https://app.custom.com"
        );
    }

    #[test]
    fn test_build_project_url_with_http_prefix() {
        assert_eq!(
            build_project_url("app", "http://example.org"),
            "https://app.example.org"
        );
    }
}
```

- [ ] **Step 2: 导出函数**

在 `shared/src/lib.rs` 顶部确保公开导出：

```rust
// Shared types and utilities for StaticHub

pub mod types;

pub use types::*;

// URL building utility
pub use build_project_url;
```

- [ ] **Step 3: 运行测试验证**

Run: `cargo test -p statichub-shared`
Expected: 所有测试通过，包括新增的 4 个 URL 构建测试

- [ ] **Step 4: 提交**

```bash
git add shared/src/lib.rs
git commit -m "feat(shared): add build_project_url utility function

Add helper function to dynamically construct project URLs from
subdomain identifier and BASE_URL environment variable.

Supports development (localhost), production, and self-hosted
deployments with custom domains."
```

---

### Task 2: 修改项目创建逻辑 - 匿名部署

**Files:**
- Modify: `server/src/models/project.rs:85-100`

- [ ] **Step 1: 修改匿名项目创建函数**

找到 `Project::create_anonymous` 函数（约第 85 行），修改 subdomain 赋值：

修改前：
```rust
pub async fn create_anonymous(
    pool: &SqlitePool,
    config: Option<&ProjectConfig>,
) -> Result<Project, sqlx::Error> {
    // Generate random 6-character subdomain
    let subdomain = generate_random_subdomain();
    let name = subdomain.clone();
    let config_json = config.map(|c| serde_json::to_string(c).ok()).flatten();

    let project = sqlx::query_as::<_, Project>(
        r#"
        INSERT INTO projects (name, subdomain, is_anonymous, config)
        VALUES (?, ?, 1, ?)
        RETURNING *
        "#,
    )
    .bind(&name)
    .bind(&subdomain)
    .bind(config_json)
    .fetch_one(pool)
    .await?;

    Ok(project)
}
```

修改后：
```rust
pub async fn create_anonymous(
    pool: &SqlitePool,
    config: Option<&ProjectConfig>,
) -> Result<Project, sqlx::Error> {
    // Generate random 6-character identifier
    let identifier = generate_random_subdomain();
    let name = identifier.clone();
    let subdomain = identifier.clone(); // Store identifier only
    let config_json = config.map(|c| serde_json::to_string(c).ok()).flatten();

    let project = sqlx::query_as::<_, Project>(
        r#"
        INSERT INTO projects (name, subdomain, is_anonymous, config)
        VALUES (?, ?, 1, ?)
        RETURNING *
        "#,
    )
    .bind(&name)
    .bind(&subdomain)
    .bind(config_json)
    .fetch_one(pool)
    .await?;

    Ok(project)
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo build -p statichub-server`
Expected: 编译成功

- [ ] **Step 3: 提交**

```bash
git add server/src/models/project.rs
git commit -m "refactor(server): store identifier only for anonymous projects

匿名项目的 subdomain 字段现在只存储标识符（如 'totutk'），
不再包含域名后缀。"
```

---

### Task 3: 修改项目创建逻辑 - 命名项目

**Files:**
- Modify: `server/src/models/project.rs:39-61, 63-94`

- [ ] **Step 1: 修改 create_owned 函数**

找到 `Project::create_owned` 函数（约第 39 行）：

修改前：
```rust
pub async fn create_owned(
    pool: &SqlitePool,
    owner_id: i64,
    name: &str,
    config: Option<&ProjectConfig>,
) -> Result<Project, sqlx::Error> {
    let subdomain = format!("{}.statichub.io", name);
    let config_json = config.map(|c| serde_json::to_string(c).ok()).flatten();
```

修改后：
```rust
pub async fn create_owned(
    pool: &SqlitePool,
    owner_id: i64,
    name: &str,
    config: Option<&ProjectConfig>,
) -> Result<Project, sqlx::Error> {
    let subdomain = name.to_string(); // Store identifier only
    let config_json = config.map(|c| serde_json::to_string(c).ok()).flatten();
```

- [ ] **Step 2: 修改 create_owned_tx 函数**

找到 `Project::create_owned_tx` 函数（约第 63 行）：

修改前：
```rust
pub async fn create_owned_tx(
    tx: &mut Transaction<'_, Sqlite>,
    owner_id: i64,
    name: &str,
    config: Option<&ProjectConfig>,
) -> Result<Project, sqlx::Error> {
    let subdomain = format!("{}.statichub.io", name);
```

修改后：
```rust
pub async fn create_owned_tx(
    tx: &mut Transaction<'_, Sqlite>,
    owner_id: i64,
    name: &str,
    config: Option<&ProjectConfig>,
) -> Result<Project, sqlx::Error> {
    let subdomain = name.to_string(); // Store identifier only
```

- [ ] **Step 3: 编译验证**

Run: `cargo build -p statichub-server`
Expected: 编译成功

- [ ] **Step 4: 提交**

```bash
git add server/src/models/project.rs
git commit -m "refactor(server): store identifier only for owned projects

命名项目的 subdomain 字段现在只存储项目名（如 'test-config'），
不再拼接 '.statichub.io' 后缀。"
```

---

### Task 4: 修改匿名部署 API 响应

**Files:**
- Modify: `server/src/api/deploys.rs:55-66`

- [ ] **Step 1: 导入 build_project_url**

在文件顶部添加导入：

```rust
use statichub_shared::build_project_url;
```

- [ ] **Step 2: 修改 deploy_anonymous 函数的响应构建**

找到函数末尾的 `Ok(Json(DeployResponse {...}))` 部分（约第 55 行）：

修改前：
```rust
Ok(Json(DeployResponse {
    url: format!("https://{}.statichub.io", subdomain),
    subdomain: format!("{}.statichub.io", subdomain),
    version: None,
    deploy_id: deploy.id,
    project_id: Some(project.id),
}))
```

修改后：
```rust
Ok(Json(DeployResponse {
    url: build_project_url(&project.subdomain, &state.base_url),
    subdomain: project.subdomain.clone(),
    version: None,
    deploy_id: deploy.id,
    project_id: Some(project.id),
}))
```

- [ ] **Step 3: 编译验证**

Run: `cargo build -p statichub-server`
Expected: 编译成功

- [ ] **Step 4: 提交**

```bash
git add server/src/api/deploys.rs
git commit -m "refactor(server): use build_project_url for anonymous deploy response

使用辅助函数动态构建 URL，基于 BASE_URL 环境变量。"
```

---

### Task 5: 修改命名项目部署 API 响应

**Files:**
- Modify: `server/src/api/projects.rs:93-101`

- [ ] **Step 1: 导入 build_project_url**

在文件顶部添加导入：

```rust
use statichub_shared::build_project_url;
```

- [ ] **Step 2: 修改 deploy_to_project 函数的响应构建**

找到函数末尾的响应构建（约第 93 行）：

修改前：
```rust
let subdomain = format!("{}.statichub.io", project_name);

Ok(Json(DeployResponse {
    url: format!("https://{}", subdomain),
    subdomain,
    version: Some(deploy.version),
    deploy_id: deploy.id,
    project_id: Some(project.id),
}))
```

修改后：
```rust
Ok(Json(DeployResponse {
    url: build_project_url(&project.subdomain, &state.base_url),
    subdomain: project.subdomain.clone(),
    version: Some(deploy.version),
    deploy_id: deploy.id,
    project_id: Some(project.id),
}))
```

- [ ] **Step 3: 编译验证**

Run: `cargo build -p statichub-server`
Expected: 编译成功

- [ ] **Step 4: 提交**

```bash
git add server/src/api/projects.rs
git commit -m "refactor(server): use build_project_url for project deploy response

使用辅助函数动态构建 URL，基于 BASE_URL 环境变量。"
```

---

### Task 6: 简化静态文件服务查找逻辑

**Files:**
- Modify: `server/src/api/serve.rs:58-77`

- [ ] **Step 1: 简化查找逻辑**

找到 `serve_static_file` 函数中的项目查找部分（约第 58 行）：

修改前：
```rust
// Try custom domain first
let project = if let Some(proj) = try_custom_domain(&hostname, &state).await? {
    proj
} else {
    // Fall back to subdomain lookup
    let subdomain = extract_subdomain(&hostname, &state.base_url)?;

    // Try exact match first (works for both formats)
    if let Some(proj) = Project::find_by_subdomain(&state.pool, &subdomain).await? {
        proj
    } else {
        // For named projects, database stores "name.statichub.io" but hostname is "name.localhost:3000"
        // Try with statichub.io suffix
        let full_subdomain = format!("{}.statichub.io", subdomain);

        Project::find_by_subdomain(&state.pool, &full_subdomain)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Project not found: {}", subdomain)))?
    }
};
```

修改后：
```rust
// Try custom domain first
let project = if let Some(proj) = try_custom_domain(&hostname, &state).await? {
    proj
} else {
    // Fall back to subdomain lookup (now simple: just identifier)
    let subdomain = extract_subdomain(&hostname, &state.base_url)?;
    Project::find_by_subdomain(&state.pool, &subdomain)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Project not found: {}", subdomain)))?
};
```

- [ ] **Step 2: 编译验证**

Run: `cargo build -p statichub-server`
Expected: 编译成功

- [ ] **Step 3: 提交**

```bash
git add server/src/api/serve.rs
git commit -m "refactor(server): simplify project lookup logic

移除查找 fallback 逻辑，现在数据库统一存储标识符，
可以直接精确匹配。"
```

---

### Task 7: 修改项目管理 API 响应

**Files:**
- Modify: `server/src/api/management.rs:74, 134`

- [ ] **Step 1: 导入 build_project_url**

在文件顶部添加导入：

```rust
use statichub_shared::build_project_url;
```

- [ ] **Step 2: 修改 list_projects 函数**

找到 `list_projects` 函数中的 URL 构建（约第 74 行）：

修改前：
```rust
let url = format!("https://{}", p.subdomain);
```

修改后：
```rust
let url = build_project_url(&p.subdomain, &state.base_url);
```

- [ ] **Step 3: 修改 get_project_info 函数**

找到 `get_project_info` 函数中的 URL 构建（约第 134 行）：

修改前：
```rust
let url = format!("https://{}", project.subdomain);
```

修改后：
```rust
let url = build_project_url(&project.subdomain, &state.base_url);
```

- [ ] **Step 4: 编译验证**

Run: `cargo build -p statichub-server`
Expected: 编译成功

- [ ] **Step 5: 提交**

```bash
git add server/src/api/management.rs
git commit -m "refactor(server): use build_project_url in management API

项目列表和详情接口使用辅助函数动态构建 URL。"
```

---

### Task 8: 清理测试数据

**Files:**
- Delete: `server/statichub.db`
- Delete: `server/storage/*`
- Delete: `/tmp/test-site-*`

- [ ] **Step 1: 停止服务器**

Run: `ps aux | grep statichub-server | grep -v grep | awk '{print $2}' | xargs kill`
Expected: 服务器进程停止

- [ ] **Step 2: 删除数据库**

Run: `rm -f server/statichub.db`
Expected: 数据库文件删除

- [ ] **Step 3: 删除存储文件**

Run: `rm -rf server/storage/*`
Expected: 存储目录清空

- [ ] **Step 4: 删除测试网站**

Run: `rm -rf /tmp/test-site*`
Expected: 测试文件删除

- [ ] **Step 5: 删除 CLI 凭证**

Run: `rm -rf ~/.statichub`
Expected: CLI 凭证清空

- [ ] **Step 6: 重新运行迁移**

Run: `cd server && sqlx migrate run`
Expected: 成功运行 2 个迁移（001_initial_schema, 002_domains）

- [ ] **Step 7: 验证数据库**

Run: `sqlite3 server/statichub.db "SELECT name FROM sqlite_master WHERE type='table';"`
Expected: 显示所有表（users, projects, deploys, deploy_tokens, oauth_sessions, domains）

- [ ] **Step 8: 提交**

```bash
git add -A
git commit -m "chore: clean test data for subdomain normalization

准备测试新的 subdomain 存储格式。"
```

---

### Task 9: 测试匿名部署

**Files:**
- Create: `/tmp/test-anon/index.html`

- [ ] **Step 1: 启动服务器**

Run: `cd server && cargo run --release &`
Expected: 服务器启动，监听 3000 端口
Wait: 3 秒

- [ ] **Step 2: 创建测试网站**

```bash
mkdir -p /tmp/test-anon
cat > /tmp/test-anon/index.html <<'EOF'
<!DOCTYPE html>
<html>
<head><title>匿名部署测试</title></head>
<body><h1>匿名部署成功！</h1></body>
</html>
EOF
```

- [ ] **Step 3: 执行匿名部署**

Run: `cargo run -p statichub -- deploy /tmp/test-anon`
Expected: 部署成功，显示随机子域名（如 abc123）

保存输出中的 subdomain 值，例如：`abc123`

- [ ] **Step 4: 验证数据库**

Run: `sqlite3 server/statichub.db "SELECT name, subdomain, is_anonymous FROM projects;"`
Expected:
```
abc123|abc123|1
```
（name 和 subdomain 相同，都是标识符）

- [ ] **Step 5: 验证访问**

Run: `curl -H "Host: abc123.localhost:3000" http://localhost:3000/`
Expected: 返回 HTML 内容 `<h1>匿名部署成功！</h1>`

---

### Task 10: 测试命名项目部署

**Files:**
- Create: `/tmp/test-named/index.html`

- [ ] **Step 1: 登录**

Run: `cargo run -p statichub -- login`
Expected: 打开浏览器，完成 Google OAuth 登录
Wait: 等待用户完成登录

- [ ] **Step 2: 创建测试网站**

```bash
mkdir -p /tmp/test-named
cat > /tmp/test-named/index.html <<'EOF'
<!DOCTYPE html>
<html>
<head><title>命名项目测试</title></head>
<body>
  <h1>命名项目部署成功！</h1>
  <p>项目名: my-test-app</p>
</body>
</html>
EOF
```

- [ ] **Step 3: 执行命名部署**

Run: `cargo run -p statichub -- deploy /tmp/test-named --name my-test-app`
Expected: 部署成功，显示 `URL: https://my-test-app.localhost:3000`

- [ ] **Step 4: 验证数据库**

Run: `sqlite3 server/statichub.db "SELECT name, subdomain, is_anonymous FROM projects WHERE name='my-test-app';"`
Expected:
```
my-test-app|my-test-app|0
```
（name 和 subdomain 相同，都是 'my-test-app'）

- [ ] **Step 5: 验证访问**

Run: `curl -H "Host: my-test-app.localhost:3000" http://localhost:3000/`
Expected: 返回 HTML 内容 `<h1>命名项目部署成功！</h1>`

- [ ] **Step 6: 验证项目列表**

Run: `cargo run -p statichub -- list`
Expected: 显示 my-test-app 项目，URL 为 `https://my-test-app.localhost:3000`

---

### Task 11: 测试环境切换

**Files:**
- Modify: `server/.env`

- [ ] **Step 1: 停止服务器**

Run: `ps aux | grep statichub-server | grep -v grep | awk '{print $2}' | xargs kill`
Expected: 服务器停止

- [ ] **Step 2: 修改 BASE_URL**

修改 `server/.env`：

```bash
BASE_URL=http://example.com:8080
```

- [ ] **Step 3: 重启服务器**

Run: `cd server && cargo run --release &`
Expected: 服务器启动
Wait: 3 秒

- [ ] **Step 4: 验证匿名项目 URL**

Run: `cargo run -p statichub -- deploy /tmp/test-anon`
Expected: URL 显示 `https://{subdomain}.example.com:8080`

- [ ] **Step 5: 验证命名项目 URL**

Run: `cargo run -p statichub -- list`
Expected: my-test-app 的 URL 显示 `https://my-test-app.example.com:8080`

- [ ] **Step 6: 恢复原始配置**

修改 `server/.env` 回到：

```bash
BASE_URL=http://localhost:3000
```

Run: `ps aux | grep statichub-server | grep -v grep | awk '{print $2}' | xargs kill && cd server && cargo run --release &`
Expected: 服务器使用原始配置重启
Wait: 3 秒

---

### Task 12: 运行集成测试

**Files:**
- Test: `server/tests/*.rs`

- [ ] **Step 1: 运行所有服务器测试**

Run: `cargo test -p statichub-server`
Expected: 所有测试通过

- [ ] **Step 2: 运行部署测试**

Run: `cargo test -p statichub-server test_deploy`
Expected: 部署相关测试通过

- [ ] **Step 3: 运行域名测试**

Run: `cargo test -p statichub-server domain_tests`
Expected: 域名相关测试通过

---

### Task 13: 最终提交

**Files:**
- All modified files

- [ ] **Step 1: 检查所有修改**

Run: `git status`
Expected: 显示所有已提交的修改

- [ ] **Step 2: 查看提交历史**

Run: `git log --oneline -10`
Expected: 显示本次重构的所有提交

- [ ] **Step 3: 创建总结提交（如果需要）**

```bash
git commit --allow-empty -m "refactor: complete subdomain normalization

完成 subdomain 字段标准化重构：
- 数据库统一存储标识符（不含域名后缀）
- URL 在应用层动态构建（基于 BASE_URL）
- 简化查找逻辑
- 支持自部署和环境切换

Closes #[issue-number] (如果有)
"
```

- [ ] **Step 4: 验证最终状态**

Run: `sqlite3 server/statichub.db "SELECT name, subdomain, is_anonymous FROM projects;"`
Expected: 所有项目的 name 和 subdomain 格式一致

---

## 验收标准检查

完成所有任务后，验证：

- ✅ 数据库中所有 `subdomain` 字段只包含标识符
- ✅ 修改 `BASE_URL` 后 API 返回的 URL 自动适配
- ✅ 匿名部署正常工作
- ✅ 命名项目部署正常工作
- ✅ 静态文件访问正常
- ✅ 查找逻辑简化，代码更清晰
- ✅ 所有集成测试通过
