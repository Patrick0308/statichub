# GitHub Release Workflow 设计文档

## 背景

StaticHub 是一个 Rust CLI 工具项目，需要为用户提供跨平台的二进制文件下载。目前项目没有自动化发布流程，需要设计一个完整的 GitHub Actions workflow 来自动化构建和发布过程。

## 目标

创建一个自动化的 GitHub Release 流程，满足以下需求：

1. **多平台支持**：为 macOS (Intel/Apple Silicon)、Linux (x86_64)、Windows (x86_64) 构建二进制文件
2. **自动触发**：通过创建 git tag (v*.*.* 格式) 自动触发发布流程
3. **质量保证**：发布前运行完整的测试套件，测试通过才继续构建
4. **用户友好**：提供清晰的安装说明和下载链接

## 架构设计

### 整体流程

```
创建 tag (v*.*.*)
    ↓
Job 1: 运行测试 (ubuntu-latest)
    ↓ (测试通过)
Job 2: 并行构建 (矩阵策略)
    ├─ macOS Intel (x86_64-apple-darwin)
    ├─ macOS Apple Silicon (aarch64-apple-darwin)
    ├─ Linux x86_64 (musl 静态链接)
    └─ Windows x86_64 (msvc)
    ↓ (所有平台构建成功)
Job 3: 创建 GitHub Release
    ↓
上传所有平台的二进制包 + checksums
```

### 关键设计决策

1. **矩阵构建策略**：使用 GitHub Actions 的 matrix 功能，为每个平台并行构建，缩短总构建时间
2. **测试先行**：独立的测试 job，只在 Linux 运行一次（节省时间），测试失败则整个 workflow 停止
3. **静态链接**：Linux 使用 musl 目标实现静态链接，确保在各种 Linux 发行版上运行
4. **命名规范**：`statichub-{version}-{platform}.tar.gz` 或 `.zip`

## 详细设计

### Job 1: 测试 (test)

**运行环境**：`ubuntu-latest`

**步骤**：
1. Checkout 代码
2. 安装 Rust stable toolchain
3. 缓存 Cargo 依赖和构建产物
4. 运行 `cargo test --workspace`
5. 失败则停止整个 workflow

**优化**：
- 使用 actions/cache 缓存 `~/.cargo` 和 `target/`
- 只在一个平台运行测试（而非每个构建平台都测试）

### Job 2: 构建 (build)

**依赖**：`needs: test`

**矩阵配置**：

| 平台 | os | target | 产物名称 | 压缩格式 |
|------|-------|--------|----------|----------|
| macOS Intel | macos-latest | x86_64-apple-darwin | statichub | .tar.gz |
| macOS Apple Silicon | macos-latest | aarch64-apple-darwin | statichub | .tar.gz |
| Linux x86_64 | ubuntu-latest | x86_64-unknown-linux-musl | statichub | .tar.gz |
| Windows x86_64 | windows-latest | x86_64-pc-windows-msvc | statichub.exe | .zip |

**每个平台的构建步骤**：
1. Checkout 代码
2. 安装 Rust toolchain
3. 添加目标平台：`rustup target add ${{ matrix.target }}`
4. Linux 特殊处理：安装 `musl-tools`
5. 构建：`cargo build --release --target ${{ matrix.target }} -p statichub`
6. Strip 二进制文件（移除调试符号，减小体积）
   - Linux/macOS: `strip target/${{ matrix.target }}/release/statichub`
   - Windows: 跳过（MSVC 自动优化）
7. 打包：
   - Linux/macOS: 创建 `.tar.gz`
   - Windows: 创建 `.zip`
8. 上传为 GitHub Actions artifact

**产物命名**：
- macOS Intel: `statichub-$VERSION-x86_64-apple-darwin.tar.gz`
- macOS Apple Silicon: `statichub-$VERSION-aarch64-apple-darwin.tar.gz`
- Linux: `statichub-$VERSION-x86_64-linux-musl.tar.gz`
- Windows: `statichub-$VERSION-x86_64-windows.zip`

### Job 3: 发布 (release)

**依赖**：`needs: build`

**运行环境**：`ubuntu-latest`

**步骤**：
1. Checkout 代码
2. 下载所有平台的 artifacts
3. 生成 checksums.txt（SHA256）
4. 创建 GitHub Release：
   - Tag: 从触发 workflow 的 tag 获取
   - Title: `StaticHub $VERSION`
   - Body: 使用 GitHub 自动生成的 changelog（commits 和 PRs）
5. 上传所有二进制包和 checksums.txt

**Release Notes 格式**：
```markdown
## Changes

[GitHub 自动生成的 commits 列表]

## Assets
- statichub-0.1.0-x86_64-apple-darwin.tar.gz
- statichub-0.1.0-aarch64-apple-darwin.tar.gz
- statichub-0.1.0-x86_64-linux-musl.tar.gz
- statichub-0.1.0-x86_64-windows.zip
- checksums.txt
```

## 错误处理

### 失败场景

1. **测试失败**：
   - 整个 workflow 停止
   - 不进行任何构建
   - 开发者收到 GitHub 通知

2. **某个平台构建失败**：
   - 其他平台继续构建
   - 但不创建 Release（需要所有平台都成功）
   - 矩阵策略配置：`fail-fast: false`（允许其他平台继续）

3. **Release 创建失败**：
   - 保留所有构建产物（artifacts 保留 90 天）
   - 可以手动重新触发 workflow
   - 或手动创建 Release 并上传 artifacts

### 版本回滚

如果发布的版本有问题：

1. 删除有问题的 tag：
   ```bash
   git tag -d v0.1.0
   git push origin :refs/tags/v0.1.0
   ```

2. 在 GitHub 上删除对应的 Release

3. 修复问题后重新创建 tag

## 触发条件

```yaml
on:
  push:
    tags:
      - 'v*.*.*'
```

**版本号格式**：
- 语义化版本：v0.1.0, v1.0.0, v1.2.3
- 支持预发布：v1.0.0-beta.1, v1.0.0-rc.1

## 使用方法

### 开发者发布新版本

```bash
# 1. 确保代码已提交并测试通过
git add .
git commit -m "release: prepare v0.1.0"

# 2. 创建并推送 tag
git tag v0.1.0
git push origin v0.1.0

# 3. GitHub Actions 自动运行
# - 运行测试
# - 构建所有平台
# - 创建 Release
# - 上传二进制文件

# 4. 发布完成后，用户可以在 Releases 页面下载
```

### 用户安装

**macOS (Intel)**：
```bash
curl -L https://github.com/Patrick0308/statichub/releases/download/v0.1.0/statichub-0.1.0-x86_64-apple-darwin.tar.gz | tar xz
sudo mv statichub /usr/local/bin/
```

**macOS (Apple Silicon)**：
```bash
curl -L https://github.com/Patrick0308/statichub/releases/download/v0.1.0/statichub-0.1.0-aarch64-apple-darwin.tar.gz | tar xz
sudo mv statichub /usr/local/bin/
```

**Linux**：
```bash
curl -L https://github.com/Patrick0308/statichub/releases/download/v0.1.0/statichub-0.1.0-x86_64-linux-musl.tar.gz | tar xz
sudo mv statichub /usr/local/bin/
```

**Windows**：
1. 下载 `statichub-0.1.0-x86_64-windows.zip`
2. 解压到任意目录
3. 将目录添加到 PATH 环境变量

## 优化考虑

### 构建时间优化

1. **缓存策略**：
   - 缓存 Cargo registry 和 git 数据库
   - 缓存编译产物（target/）
   - 使用 `Swatinem/rust-cache` action

2. **并行构建**：
   - 所有平台并行构建（matrix 策略）
   - 最大并行度取决于 GitHub Actions 限制

3. **增量编译**：
   - 缓存 target/ 目录
   - 只重新编译修改的代码

### 二进制文件大小优化

1. **Strip 调试符号**：使用 `strip` 命令
2. **编译器优化**：`--release` 模式
3. **静态链接**：Linux 使用 musl（体积稍大但无依赖）

### 安全考虑

1. **Checksums**：提供 SHA256 校验和
2. **GITHUB_TOKEN**：使用内置 token，自动授权
3. **签名**：未来可考虑 GPG 签名（当前版本不包含）

## 未来扩展

1. **更多平台**：ARM Linux、FreeBSD 等
2. **Homebrew Tap**：自动更新 Homebrew formula
3. **包管理器**：发布到 crates.io（`cargo install`）
4. **Docker 镜像**：发布 server 组件的容器镜像
5. **自动更新检查**：CLI 工具内置版本检查

## 验收标准

- ✅ 创建 tag 后自动触发 workflow
- ✅ 测试失败时停止构建
- ✅ 所有平台成功构建二进制文件
- ✅ 自动创建 GitHub Release
- ✅ Release 包含所有平台的二进制包
- ✅ 提供 checksums.txt
- ✅ Release Notes 包含 changelog
- ✅ 用户可以通过简单命令安装

## 参考资料

- [GitHub Actions 文档](https://docs.github.com/en/actions)
- [Rust Cross-Compilation](https://rust-lang.github.io/rustup/cross-compilation.html)
- [actions-rs/toolchain](https://github.com/actions-rs/toolchain)
- [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache)
