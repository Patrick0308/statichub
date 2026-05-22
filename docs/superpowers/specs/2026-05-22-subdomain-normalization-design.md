# Subdomain 存储标准化设计

## 背景

当前数据库中 `subdomain` 字段存储格式不一致：
- 匿名部署：存储标识符（如 "totutk"）
- 命名项目：存储完整域名（如 "test-config.statichub.io"）

这导致：
1. 数据格式不统一
2. 域名硬编码在数据库中，无法在环境间移植
3. 查询逻辑复杂，需要特殊处理
4. 不支持自部署场景（用户使用自己的域名）

## 目标

统一 `subdomain` 字段格式，只存储标识符，完整 URL 在应用层动态构建。

## 架构决策

### 1. 数据存储策略

**数据库只存储标识符：**
- 匿名部署：随机 6 字符 ID（如 "totutk"）
- 命名项目：用户提供的名称（如 "test-config"）

**不存储域名后缀：**
- ❌ 不存储：".statichub.io", ".localhost:3000" 等
- ✅ 只存储：纯标识符

### 2. URL 构建策略

**运行时动态构建完整 URL：**

```rust
pub fn build_project_url(subdomain: &str, base_url: &str) -> String {
    let domain = base_url
        .trim_start_matches("http://")
        .trim_start_matches("https://");

    format!("https://{}.{}", subdomain, domain)
}
```

**示例：**
- 开发环境（`BASE_URL=http://localhost:3000`）：
  - 输入：subdomain="test-config"
  - 输出：`https://test-config.localhost:3000`

- 生产环境（`BASE_URL=https://statichub.io`）：
  - 输入：subdomain="test-config"
  - 输出：`https://test-config.statichub.io`

- 自部署环境（`BASE_URL=https://custom.com`）：
  - 输入：subdomain="test-config"
  - 输出：`https://test-config.custom.com`

### 3. 数据模型

**Schema（无需修改）：**
```sql
CREATE TABLE projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id INTEGER,
    name TEXT NOT NULL UNIQUE,           -- "test-config" 或 "totutk"
    subdomain TEXT NOT NULL UNIQUE,      -- "test-config" 或 "totutk"
    is_anonymous BOOLEAN NOT NULL DEFAULT 0,
    ...
);
```

**数据一致性：**

当前状态：
```
name         subdomain                  is_anonymous
--------     ------------------------   ------------
totutk       totutk                     1
test-config  test-config.statichub.io   0  ← 不一致
```

目标状态：
```
name         subdomain       is_anonymous
--------     -------------   ------------
totutk       totutk          1
test-config  test-config     0  ← 统一
```

### 4. 查找逻辑简化

**修改前（复杂）：**
```rust
// 需要两次查找
let subdomain = extract_subdomain(&hostname, &state.base_url)?;
if let Some(proj) = Project::find_by_subdomain(&state.pool, &subdomain).await? {
    proj
} else {
    // Fallback: 尝试加 .statichub.io 后缀
    let full = format!("{}.statichub.io", subdomain);
    Project::find_by_subdomain(&state.pool, &full).await?...
}
```

**修改后（简单）：**
```rust
// 一次查找即可
let subdomain = extract_subdomain(&hostname, &state.base_url)?;
Project::find_by_subdomain(&state.pool, &subdomain)
    .await?
    .ok_or_else(|| AppError::NotFound(...))?
```

## 实施计划

### 影响范围

**需要修改的文件：**
1. `shared/src/lib.rs` - 添加 `build_project_url()` 辅助函数
2. `server/src/models/project.rs` - 创建项目时不拼接域名
3. `server/src/api/projects.rs` - URL 响应使用辅助函数
4. `server/src/api/deploys.rs` - URL 响应使用辅助函数
5. `server/src/api/serve.rs` - 简化查找逻辑
6. `server/src/api/management.rs` - URL 构建逻辑

### 数据迁移

**测试阶段策略：** 直接清空数据库重新开始

步骤：
1. 删除 `server/statichub.db`
2. 删除 `server/storage/*`
3. 删除 `/tmp/test-site-*` 测试文件
4. 重新运行 `sqlx migrate run`

**生产阶段策略（未来）：** 使用 SQL 迁移脚本

```sql
-- 003_normalize_subdomains.sql
UPDATE projects
SET subdomain = name
WHERE is_anonymous = 0;
```

### 测试计划

**测试场景：**

1. **匿名部署**
   - 创建：生成随机 ID
   - 访问：`{random}.localhost:3000` 成功
   - URL 响应包含正确的完整 URL

2. **命名项目**
   - 创建：使用用户名称
   - 访问：`{name}.localhost:3000` 成功
   - URL 响应包含正确的完整 URL

3. **环境切换**
   - 修改 `.env` 中的 `BASE_URL`
   - 重启服务器
   - URL 自动适配新域名

4. **数据库可移植性**
   - 复制数据库文件到新环境
   - 修改新环境 `BASE_URL`
   - 项目正常访问

## 验收标准

- ✅ 数据库中所有项目的 `subdomain` 字段只包含标识符
- ✅ 修改 `BASE_URL` 后，API 返回的 URL 自动适配
- ✅ 静态文件访问正常（匿名和命名项目）
- ✅ 查找逻辑简化，无需 fallback
- ✅ 所有集成测试通过

## 优势

1. **自部署友好：** 用户可以使用自己的域名部署实例
2. **环境无关：** 数据库可在开发、测试、生产环境间复制
3. **数据一致性：** 匿名和命名项目使用统一格式
4. **代码简化：** 查找逻辑更简单，URL 构建集中管理
5. **可维护性：** 域名配置集中在环境变量，修改方便

## 风险与缓解

**风险：** 如果已有生产数据，需要仔细处理迁移

**缓解：** 当前处于测试阶段，可以直接清空数据。未来生产环境使用 SQL 迁移脚本，并在迁移前备份数据库。
