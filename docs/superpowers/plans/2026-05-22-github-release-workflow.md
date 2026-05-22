# GitHub Release Workflow 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 创建自动化的 GitHub Actions workflow，在创建 tag 时自动测试、构建多平台二进制文件并发布到 GitHub Releases

**Architecture:** 使用 GitHub Actions 矩阵策略并行构建 4 个平台（macOS Intel/ARM、Linux、Windows），测试通过后自动创建 Release 并上传二进制包

**Tech Stack:** GitHub Actions, Rust, cargo, tar/zip

---

## 文件结构

**需要创建的文件：**
- `.github/workflows/release.yml` - GitHub Actions workflow 配置文件

**工作流结构：**
1. Job: `test` - 在 ubuntu-latest 上运行测试
2. Job: `build` - 矩阵策略并行构建 4 个平台
3. Job: `release` - 创建 GitHub Release 并上传所有二进制包

---

### Task 1: 创建 workflow 文件和测试 Job

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: 创建 workflow 目录**

```bash
mkdir -p .github/workflows
```

- [ ] **Step 2: 创建 workflow 文件头部和测试 job**

创建 `.github/workflows/release.yml`：

```yaml
name: Release

on:
  push:
    tags:
      - 'v*.*.*'

permissions:
  contents: write

jobs:
  test:
    name: Run tests
    runs-on: ubuntu-latest
    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Cache dependencies
        uses: Swatinem/rust-cache@v2

      - name: Run tests
        run: cargo test --workspace --verbose
```

- [ ] **Step 3: 验证 YAML 语法**

Run: `cat .github/workflows/release.yml`
Expected: 文件内容正确显示，无语法错误

- [ ] **Step 4: 提交**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add release workflow with test job"
```

---

### Task 2: 添加构建 Job（矩阵策略）

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: 在 test job 后添加 build job**

在 `.github/workflows/release.yml` 的 `test` job 后添加：

```yaml
  build:
    name: Build ${{ matrix.target }}
    needs: test
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: macos-latest
            target: x86_64-apple-darwin
            artifact_name: statichub
            asset_name: statichub-x86_64-apple-darwin

          - os: macos-latest
            target: aarch64-apple-darwin
            artifact_name: statichub
            asset_name: statichub-aarch64-apple-darwin

          - os: ubuntu-latest
            target: x86_64-unknown-linux-musl
            artifact_name: statichub
            asset_name: statichub-x86_64-linux-musl

          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact_name: statichub.exe
            asset_name: statichub-x86_64-windows

    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Install musl tools (Linux only)
        if: matrix.target == 'x86_64-unknown-linux-musl'
        run: sudo apt-get update && sudo apt-get install -y musl-tools

      - name: Cache dependencies
        uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.target }}

      - name: Build release binary
        run: cargo build --release --target ${{ matrix.target }} -p statichub

      - name: Strip binary (Linux and macOS only)
        if: matrix.os != 'windows-latest'
        run: strip target/${{ matrix.target }}/release/${{ matrix.artifact_name }}

      - name: Extract version from tag
        shell: bash
        run: echo "VERSION=${GITHUB_REF#refs/tags/v}" >> $GITHUB_ENV

      - name: Create tarball (Linux and macOS)
        if: matrix.os != 'windows-latest'
        run: |
          cd target/${{ matrix.target }}/release
          tar czf ../../../${{ matrix.asset_name }}-${{ env.VERSION }}.tar.gz ${{ matrix.artifact_name }}
          cd ../../..

      - name: Create zip (Windows)
        if: matrix.os == 'windows-latest'
        shell: bash
        run: |
          cd target/${{ matrix.target }}/release
          7z a ../../../${{ matrix.asset_name }}-${{ env.VERSION }}.zip ${{ matrix.artifact_name }}
          cd ../../..

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.asset_name }}-${{ env.VERSION }}
          path: |
            ${{ matrix.asset_name }}-${{ env.VERSION }}.tar.gz
            ${{ matrix.asset_name }}-${{ env.VERSION }}.zip
          if-no-files-found: ignore
```

- [ ] **Step 2: 验证 YAML 语法**

Run: `cat .github/workflows/release.yml | grep -A 5 "build:"`
Expected: build job 配置正确显示

- [ ] **Step 3: 提交**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add build job with matrix strategy for 4 platforms"
```

---

### Task 3: 添加发布 Job

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: 在 build job 后添加 release job**

在 `.github/workflows/release.yml` 的 `build` job 后添加：

```yaml
  release:
    name: Create GitHub Release
    needs: build
    runs-on: ubuntu-latest
    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Extract version from tag
        run: echo "VERSION=${GITHUB_REF#refs/tags/v}" >> $GITHUB_ENV

      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts

      - name: Move artifacts to root
        run: |
          mkdir -p release-assets
          find artifacts -type f \( -name "*.tar.gz" -o -name "*.zip" \) -exec mv {} release-assets/ \;

      - name: Generate checksums
        run: |
          cd release-assets
          sha256sum * > checksums.txt
          cat checksums.txt

      - name: Create Release
        uses: softprops/action-gh-release@v1
        with:
          name: StaticHub v${{ env.VERSION }}
          generate_release_notes: true
          files: |
            release-assets/*
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

- [ ] **Step 2: 验证完整的 workflow 文件**

Run: `cat .github/workflows/release.yml`
Expected: 包含 test、build、release 三个完整的 jobs

- [ ] **Step 3: 检查 YAML 语法**

Run: `yamllint .github/workflows/release.yml || echo "yamllint not installed, skipping"`
Expected: 无语法错误（如果安装了 yamllint）

- [ ] **Step 4: 提交**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add release job to create GitHub Release with all binaries"
```

---

### Task 4: 测试 Workflow（创建测试 tag）

**Files:**
- None (testing only)

- [ ] **Step 1: 推送所有更改到 GitHub**

```bash
git push origin main
```

Expected: 成功推送到 main 分支

- [ ] **Step 2: 创建测试 tag**

```bash
git tag v0.1.0-test
git push origin v0.1.0-test
```

Expected: tag 成功推送，GitHub Actions 开始运行

- [ ] **Step 3: 检查 GitHub Actions 运行状态**

访问: `https://github.com/Patrick0308/statichub/actions`

Expected: 看到 "Release" workflow 正在运行，包含 test、build、release 三个 jobs

- [ ] **Step 4: 等待 workflow 完成并验证结果**

检查:
1. Test job 是否通过
2. Build job 是否为 4 个平台都成功构建
3. Release job 是否成功创建 GitHub Release
4. Release 是否包含 4 个二进制包 + checksums.txt

- [ ] **Step 5: 下载并测试一个二进制文件**

```bash
# macOS Intel 示例
curl -L https://github.com/Patrick0308/statichub/releases/download/v0.1.0-test/statichub-x86_64-apple-darwin-0.1.0-test.tar.gz -o test.tar.gz
tar xzf test.tar.gz
./statichub --version
```

Expected: 显示版本信息

- [ ] **Step 6: 清理测试 tag（如果测试失败）**

如果测试失败需要重新测试：

```bash
git tag -d v0.1.0-test
git push origin :refs/tags/v0.1.0-test
```

然后在 GitHub Releases 页面删除测试 release

- [ ] **Step 7: 文档化 workflow**

```bash
git commit --allow-empty -m "ci: complete release workflow implementation

Features:
- Automatic testing on tag push
- Multi-platform builds (macOS Intel/ARM, Linux, Windows)
- Automatic GitHub Release creation
- SHA256 checksums for all binaries"
```

---

### Task 5: 更新 README 添加安装说明

**Files:**
- Modify: `README.md`

- [ ] **Step 1: 在 README.md 的 Installation 部分添加预编译二进制安装说明**

在 `README.md` 的 `## Installation` 部分，在 "From Source" 之前添加：

```markdown
### Pre-built Binaries

Download the latest release for your platform from the [Releases page](https://github.com/Patrick0308/statichub/releases).

**macOS (Intel)**:
```bash
curl -L https://github.com/Patrick0308/statichub/releases/latest/download/statichub-x86_64-apple-darwin.tar.gz | tar xz
sudo mv statichub /usr/local/bin/
```

**macOS (Apple Silicon)**:
```bash
curl -L https://github.com/Patrick0308/statichub/releases/latest/download/statichub-aarch64-apple-darwin.tar.gz | tar xz
sudo mv statichub /usr/local/bin/
```

**Linux (x86_64)**:
```bash
curl -L https://github.com/Patrick0308/statichub/releases/latest/download/statichub-x86_64-linux-musl.tar.gz | tar xz
sudo mv statichub /usr/local/bin/
```

**Windows**:
1. Download `statichub-x86_64-windows.zip` from the releases page
2. Extract the archive
3. Add the extracted directory to your PATH

**Verify Installation**:
```bash
statichub --version
```

```

- [ ] **Step 2: 验证 markdown 格式**

Run: `head -100 README.md | grep -A 20 "Pre-built Binaries"`
Expected: 安装说明正确显示

- [ ] **Step 3: 提交**

```bash
git add README.md
git commit -m "docs: add pre-built binaries installation instructions"
```

---

## 验收标准检查

完成所有任务后，验证：

- ✅ `.github/workflows/release.yml` 文件存在且配置完整
- ✅ 创建 tag 后自动触发 workflow
- ✅ 测试 job 运行 `cargo test --workspace`
- ✅ Build job 为 4 个平台并行构建二进制文件
- ✅ Release job 创建 GitHub Release
- ✅ Release 包含所有平台的二进制包（.tar.gz 和 .zip）
- ✅ Release 包含 checksums.txt
- ✅ Release Notes 自动生成 changelog
- ✅ README 包含预编译二进制安装说明
- ✅ 用户可以通过简单命令下载和安装

## 故障排查

### 常见问题

**问题 1: 测试失败**
- 检查: `cargo test --workspace` 在本地是否通过
- 解决: 修复测试后重新推送 tag

**问题 2: Linux musl 构建失败**
- 检查: `musl-tools` 是否正确安装
- 解决: 确认 workflow 中 `apt-get install musl-tools` 步骤

**问题 3: macOS ARM 构建失败**
- 检查: Rust toolchain 是否支持 `aarch64-apple-darwin`
- 解决: 确认 `rustup target add` 步骤正确

**问题 4: Windows zip 创建失败**
- 检查: 7z 命令是否可用
- 解决: Windows runner 默认包含 7z

**问题 5: Release 创建失败**
- 检查: `GITHUB_TOKEN` 权限
- 解决: 确认 workflow 中设置了 `permissions: contents: write`

**问题 6: 产物未上传**
- 检查: artifact 名称和路径是否正确
- 解决: 确认 `upload-artifact` 和 `download-artifact` 步骤的 path 配置

## 未来优化

1. **缓存优化**: 调整 `Swatinem/rust-cache` 的缓存策略
2. **并行度**: GitHub Actions 免费账户有并发限制，可能需要调整
3. **构建时间**: 监控各平台构建时间，优化慢的平台
4. **二进制大小**: 使用 UPX 等工具进一步压缩（可选）
5. **签名**: 添加 GPG 签名增强安全性（可选）
