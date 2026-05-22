# StaticHub 本地测试指南

本指南介绍如何在本地环境中测试 StaticHub 的完整功能。

## 环境要求

- Rust 1.70+ (安装: https://rustup.rs/)
- SQLite 3
- Google OAuth 凭据（可选，用于测试认证功能）

## 快速开始

### 1. 克隆并构建项目

```bash
cd /Users/patrick/projects/statichub

# 构建所有组件
cargo build --workspace
```

### 2. 设置数据库

```bash
# 进入 server 目录
cd server

# 运行数据库迁移
sqlx migrate run --database-url sqlite:statichub.db
```

这会创建 `statichub.db` 文件并建立所有必需的表。

### 3. 配置环境变量（简化版）

创建 `server/.env` 文件：

```bash
# 最小配置 - 用于本地测试
DATABASE_URL=sqlite:statichub.db
BASE_URL=http://localhost:3000
JWT_SECRET=test-secret-key-for-local-development-only
STORAGE_PATH=./storage

# OAuth（暂时使用占位符，匿名部署不需要）
GOOGLE_CLIENT_ID=placeholder
GOOGLE_CLIENT_SECRET=placeholder
GOOGLE_REDIRECT_URI=http://localhost:3000/auth/callback/google
```

### 4. 启动服务器

```bash
# 在 server 目录中
cargo run --release

# 或者使用开发模式（更快的编译）
cargo run
```

服务器将在 `http://localhost:3000` 启动。

## 测试场景

### 场景 1: 匿名部署（无需登录）

这是最简单的测试场景，不需要 OAuth 配置。

```bash
# 在新终端中

# 1. 创建测试网站
mkdir -p /tmp/test-site
cat > /tmp/test-site/index.html << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>Test Site</title>
</head>
<body>
    <h1>Hello from StaticHub!</h1>
    <p>This is a test deployment.</p>
</body>
</html>
EOF

# 2. 部署
cargo run -p statichub -- deploy /tmp/test-site

# 输出示例:
# 📦 Preparing deployment...
#    Found 1 file
#    Total size: 234 bytes
#
# 🚀 Deploying...
# ✅ Deployment successful!
#    URL: http://x7k2m9.localhost:3000
#    Subdomain: x7k2m9.localhost:3000
```

**验证部署：**

```bash
# 方法1: 使用 curl
curl http://x7k2m9.localhost:3000/

# 方法2: 在浏览器中打开
# http://x7k2m9.localhost:3000/
```

### 场景 2: 带配置文件的部署

```bash
# 1. 创建更复杂的网站
mkdir -p /tmp/spa-test
cat > /tmp/spa-test/index.html << 'EOF'
<!DOCTYPE html>
<html>
<head><title>SPA Test</title></head>
<body>
    <h1>Single Page App</h1>
    <nav>
        <a href="/about">About</a>
        <a href="/contact">Contact</a>
    </nav>
</body>
</html>
EOF

cat > /tmp/spa-test/about.html << 'EOF'
<!DOCTYPE html>
<html>
<head><title>About</title></head>
<body><h1>About Page</h1></body>
</html>
EOF

# 2. 创建配置文件
cat > /tmp/spa-test/statichub.yaml << 'EOF'
# 启用 Clean URLs
clean_urls: true

# 启用 SPA 模式
spa: true

# 自定义重定向
redirects:
  - from: /old-page
    to: /about
    status: 301

# 自定义 Headers
headers:
  - path: /*
    headers:
      X-Custom-Header: StaticHub-Test
      Cache-Control: public, max-age=3600
EOF

# 3. 部署
cargo run -p statichub -- deploy /tmp/spa-test --config /tmp/spa-test/statichub.yaml
```

**验证配置功能：**

```bash
# 测试 Clean URLs
curl http://[subdomain].localhost:3000/about
# 应该返回 about.html 的内容

# 测试自定义 Headers
curl -I http://[subdomain].localhost:3000/
# 应该看到 X-Custom-Header

# 测试重定向
curl -I http://[subdomain].localhost:3000/old-page
# 应该看到 301 重定向到 /about
```

### 场景 3: 认证部署（需要 Google OAuth）

#### 3.1 配置 Google OAuth

1. 访问 [Google Cloud Console](https://console.cloud.google.com/)
2. 创建新项目或选择现有项目
3. 启用 Google+ API
4. 创建 OAuth 2.0 凭据：
   - 应用类型: Web 应用
   - 授权重定向 URI: `http://localhost:3000/auth/callback/google`
5. 获取 Client ID 和 Client Secret

#### 3.2 更新 .env 文件

```bash
# server/.env
DATABASE_URL=sqlite:statichub.db
BASE_URL=http://localhost:3000
JWT_SECRET=your-random-secret-key-here
STORAGE_PATH=./storage

# 真实的 OAuth 凭据
GOOGLE_CLIENT_ID=your-actual-client-id.apps.googleusercontent.com
GOOGLE_CLIENT_SECRET=your-actual-client-secret
GOOGLE_REDIRECT_URI=http://localhost:3000/auth/callback/google
```

#### 3.3 重启服务器并测试认证

```bash
# 重启 server (Ctrl+C 停止，然后重新运行)
cd server
cargo run

# 在新终端中登录
cargo run -p statichub -- login

# 浏览器会自动打开 Google OAuth 页面
# 授权后，CLI 会显示:
# ✅ Login successful!
#    Credentials saved to ~/.statichub/credentials.json
```

#### 3.4 测试命名项目部署

```bash
# 部署到命名项目
cargo run -p statichub -- deploy /tmp/test-site --name my-test-app

# 输出:
# 📦 Preparing deployment...
# 🚀 Deploying to project: my-test-app
# ✅ Deployment successful!
#    URL: http://my-test-app.localhost:3000
#    Version: 1
```

**验证：**

```bash
curl http://my-test-app.localhost:3000/
```

### 场景 4: 项目管理

```bash
# 列出所有项目
cargo run -p statichub -- list

# 查看项目详情
cargo run -p statichub -- info my-test-app

# 部署新版本
echo "<h1>Version 2</h1>" > /tmp/test-site/index.html
cargo run -p statichub -- deploy /tmp/test-site --name my-test-app

# 查看版本历史
cargo run -p statichub -- info my-test-app

# 回滚到版本 1
cargo run -p statichub -- rollback my-test-app 1

# 验证回滚
curl http://my-test-app.localhost:3000/
```

### 场景 5: 自定义域名

#### 5.1 添加域名

```bash
# 添加自定义域名
cargo run -p statichub -- domain add my-test-app example.test

# 输出会包含验证 token，例如:
# 🌐 Adding domain example.test to my-test-app...
# ✅ Domain added!
#    Status: pending_verification
#
# 📝 Verification instructions:
#    Upload a file named 'statichub-verify.txt' to your domain root containing: a1b2c3d4-e5f6-7890-abcd-ef1234567890
#
#    After uploading the file, run:
#    statichub domain verify my-test-app example.test
```

#### 5.2 创建验证文件并重新部署

```bash
# 创建验证文件（使用上面输出的 token）
echo "a1b2c3d4-e5f6-7890-abcd-ef1234567890" > /tmp/test-site/statichub-verify.txt

# 重新部署
cargo run -p statichub -- deploy /tmp/test-site --name my-test-app
```

#### 5.3 验证域名

```bash
# 验证域名所有权
cargo run -p statichub -- domain verify my-test-app example.test

# 输出:
# 🔍 Verifying domain example.test...
# ✅ Domain verified successfully!
#    example.test is now live
```

#### 5.4 配置本地 DNS 并测试

```bash
# 添加到 /etc/hosts（需要 sudo）
echo "127.0.0.1 example.test" | sudo tee -a /etc/hosts

# 测试自定义域名
curl http://example.test:3000/

# 应该返回您的网站内容
```

#### 5.5 域名管理

```bash
# 列出所有域名
cargo run -p statichub -- domain list my-test-app

# 删除域名
cargo run -p statichub -- domain remove my-test-app example.test

# 清理 /etc/hosts（如果需要）
sudo sed -i '' '/example.test/d' /etc/hosts
```

## 运行自动化测试

### 运行所有测试

```bash
# 在项目根目录
cargo test --workspace

# 预期结果: 所有 70+ 测试通过
```

### 运行特定测试套件

```bash
# 只运行 server 测试
cargo test -p statichub-server

# 只运行集成测试
cargo test --test '*'

# 运行特定测试
cargo test test_add_domain
cargo test test_authenticated_deploy
cargo test test_verify_domain_success
```

### 测试覆盖率

```bash
# 按模块查看测试
cargo test --workspace -- --nocapture

# 查看详细输出
cargo test --workspace -- --show-output
```

## 常见测试场景

### 测试 1: Clean URLs

```bash
# 创建带 .html 的文件
mkdir -p /tmp/clean-url-test
echo "<h1>About</h1>" > /tmp/clean-url-test/about.html

# 配置
cat > /tmp/clean-url-test/statichub.yaml << 'EOF'
clean_urls: true
EOF

# 部署
cargo run -p statichub -- deploy /tmp/clean-url-test

# 测试: 无需 .html 扩展名即可访问
curl http://[subdomain].localhost:3000/about
```

### 测试 2: SPA 模式

```bash
mkdir -p /tmp/spa-test
cat > /tmp/spa-test/index.html << 'EOF'
<!DOCTYPE html>
<html>
<body>
    <div id="app">SPA</div>
    <script>
        // 模拟 SPA 路由
        console.log(window.location.pathname);
    </script>
</body>
</html>
EOF

cat > /tmp/spa-test/statichub.yaml << 'EOF'
spa: true
EOF

cargo run -p statichub -- deploy /tmp/spa-test

# 测试: 任意路径都返回 index.html
curl http://[subdomain].localhost:3000/any/random/path
# 应该返回 index.html
```

### 测试 3: 重定向

```bash
mkdir -p /tmp/redirect-test
echo "<h1>Home</h1>" > /tmp/redirect-test/index.html
echo "<h1>New Page</h1>" > /tmp/redirect-test/new.html

cat > /tmp/redirect-test/statichub.yaml << 'EOF'
redirects:
  - from: /old
    to: /new
    status: 301
EOF

cargo run -p statichub -- deploy /tmp/redirect-test

# 测试重定向
curl -I http://[subdomain].localhost:3000/old
# 应该看到 301 Location: /new
```

## 调试技巧

### 查看服务器日志

```bash
# 启动 server 时会看到所有请求日志
# 默认输出到 stdout
```

### 检查数据库内容

```bash
# 使用 sqlite3 CLI
sqlite3 server/statichub.db

# 查看表
.tables

# 查看项目
SELECT * FROM projects;

# 查看部署
SELECT * FROM deploys;

# 查看域名
SELECT * FROM domains;

# 退出
.quit
```

### 清理测试数据

```bash
# 删除数据库并重新创建
cd server
rm statichub.db
sqlx migrate run --database-url sqlite:statichub.db

# 删除存储文件
rm -rf storage/*

# 删除 CLI 凭据
rm -rf ~/.statichub/
```

### 检查 CLI 配置

```bash
# 查看保存的凭据
cat ~/.statichub/credentials.json

# 检查格式
{
  "access_token": "eyJ...",
  "saved_at": "2024-01-15T10:30:00Z"
}
```

## 性能测试

### 测试大文件部署

```bash
# 创建包含多个文件的网站
mkdir -p /tmp/big-site
for i in {1..100}; do
    echo "<h1>Page $i</h1>" > /tmp/big-site/page$i.html
done

# 部署并计时
time cargo run -p statichub -- deploy /tmp/big-site --name big-site
```

### 测试并发部署

```bash
# 在多个终端中同时部署
# Terminal 1:
cargo run -p statichub -- deploy /tmp/test-site --name project1

# Terminal 2:
cargo run -p statichub -- deploy /tmp/test-site --name project2

# Terminal 3:
cargo run -p statichub -- deploy /tmp/test-site --name project3
```

## 故障排除

### 问题 1: 编译错误

```bash
# 清理并重新构建
cargo clean
cargo build --workspace
```

### 问题 2: 数据库迁移失败

```bash
# 检查 sqlx-cli 是否安装
cargo install sqlx-cli --no-default-features --features sqlite

# 删除数据库并重新运行迁移
cd server
rm statichub.db
sqlx migrate run --database-url sqlite:statichub.db
```

### 问题 3: OAuth 不工作

```bash
# 检查环境变量
cd server
cat .env | grep GOOGLE

# 确保重定向 URI 匹配
# Google Console: http://localhost:3000/auth/callback/google
# .env: GOOGLE_REDIRECT_URI=http://localhost:3000/auth/callback/google
```

### 问题 4: 端口已被占用

```bash
# 查找占用 3000 端口的进程
lsof -i :3000

# 杀死进程
kill -9 <PID>

# 或使用不同端口启动 server
BASE_URL=http://localhost:3001 cargo run
```

### 问题 5: 自定义域名无法访问

```bash
# 检查 /etc/hosts
cat /etc/hosts | grep example.test

# 应该有这一行:
# 127.0.0.1 example.test

# 验证域名已验证
cargo run -p statichub -- domain list my-test-app
# Status 应该是 "verified"

# 测试连接
curl -v http://example.test:3000/
```

## 完整测试流程

运行这个脚本来测试所有主要功能：

```bash
#!/bin/bash
set -e

echo "=== StaticHub 完整测试流程 ==="

# 1. 启动 server (后台)
echo "1. 启动 server..."
cd server
cargo run &
SERVER_PID=$!
sleep 3

cd ..

# 2. 匿名部署
echo "2. 测试匿名部署..."
mkdir -p /tmp/statichub-test
echo "<h1>Test</h1>" > /tmp/statichub-test/index.html
cargo run -p statichub -- deploy /tmp/statichub-test

# 3. 登录（需要手动完成 OAuth）
echo "3. 测试登录（请在浏览器中完成授权）..."
cargo run -p statichub -- login

# 4. 命名项目部署
echo "4. 测试命名项目部署..."
cargo run -p statichub -- deploy /tmp/statichub-test --name test-project

# 5. 项目管理
echo "5. 测试项目管理..."
cargo run -p statichub -- list
cargo run -p statichub -- info test-project

# 6. 版本管理
echo "6. 测试版本管理..."
echo "<h1>Version 2</h1>" > /tmp/statichub-test/index.html
cargo run -p statichub -- deploy /tmp/statichub-test --name test-project
cargo run -p statichub -- rollback test-project 1

# 7. 域名管理
echo "7. 测试域名管理..."
cargo run -p statichub -- domain add test-project example.test
# 手动步骤：添加验证文件、部署、验证

# 8. 运行测试套件
echo "8. 运行自动化测试..."
cargo test --workspace

# 清理
echo "=== 测试完成，清理中 ==="
kill $SERVER_PID
rm -rf /tmp/statichub-test

echo "✅ 所有测试通过！"
```

## 下一步

测试完成后，您可以：

1. **修改代码** - 在 `server/` 或 `cli/` 中进行更改
2. **添加测试** - 在 `server/tests/` 中添加新测试
3. **扩展功能** - 参考 README.md 中的 Roadmap
4. **部署到生产** - 配置真实域名和 SSL 证书

## 相关文档

- [README.md](../README.md) - 项目概览和功能说明
- [server/migrations/](../server/migrations/) - 数据库架构
- [docs/superpowers/specs/](./superpowers/specs/) - 设计规范
- [docs/superpowers/plans/](./superpowers/plans/) - 实现计划
