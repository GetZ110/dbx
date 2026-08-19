# DBX 在 HarmonyOS（aarch64 / musl）上的构建与运行说明

> 适用环境：HarmonyOS（HongMeng Kernel）、`aarch64`、musl libc。
> 本文档记录如何在本机从源码构建 `dbx-web` 和 `dbx`（CLI），并运行 Web 模式。

## 1. 结论

DBX 可以移植到当前 HarmonyOS 系统，推荐使用 **Web 模式**：

- `dbx-web`：Rust 编写的 Web 后端，提供前端页面和 `/api/*` 接口。
- `dbx`：Rust 编写的命令行客户端。

Tauri 桌面原生模式需要 WebView/GTK/WebKit 等系统组件，当前 HarmonyOS 环境不建议作为移植目标。

## 2. 环境要求

| 项目 | 要求 |
|---|---|
| 系统 | HarmonyOS / HongMeng Kernel，`aarch64` |
| libc | musl（`/lib/ld-musl-aarch64.so.1`） |
| Rust | Homebrew Rust 1.97.1（host: `aarch64-unknown-linux-ohos`） |
| Node.js | >= 22.13（当前环境为 26.x） |
| pnpm | 10.27.0（与仓库 `packageManager` 一致） |
| 工具链 | gcc / clang / make / cmake / python3 |

## 3. 安装依赖

### 3.1 Rust

```bash
brew install rust
```

验证：

```bash
rustc -Vv
# host: aarch64-unknown-linux-ohos
```

### 3.2 pnpm

```bash
npm install -g pnpm@10.27.0
export PATH="$HOME/.npm-global/bin:$PATH"
pnpm -v
```

### 3.3 项目前端依赖

```bash
cd /storage/Users/currentUser/GitProject/dbx
pnpm install --frozen-lockfile
```

> 注意：当前环境的 `rolldown` OpenHarmony binding 加载会被系统拒绝（`Permission denied`），
> 因此本地 `pnpm build` 可能无法完成。可跳过前端构建，使用官方静态包中的现成 `dist/`（见第 5 节）。

## 4. 本地源码补丁（TLS 后端改为 ring）

首次构建 `dbx-web` 时，`aws-lc-rs` 在 `aarch64-unknown-linux-ohos` target 上缺少 AArch64 汇编符号，导致链接失败：

```
ld.lld: error: undefined symbol: aws_lc_0_43_0_ChaCha20_ctr32_neon
...
```

解决方式：将 TLS provider 从 `aws-lc-rs` 切换到 `ring`。

### 4.1 修改的 Cargo.toml

- `crates/dbx-core/Cargo.toml`
- `crates/dbx-web/Cargo.toml`
- `src-tauri/Cargo.toml`

关键改动：

```toml
# rustls：使用 ring provider
rustls = { version = "0.23", default-features = false, features = ["ring", "std", "tls12", "logging"] }

# russh：禁用默认 aws-lc-rs，改用 ring
russh = { version = "0.60", default-features = false, features = ["flate2", "ring", "rsa"] }

# mysql_async：使用 ring 版 rustls
mysql_async = { version = "0.37", default-features = false, features = ["default-rustls-ring", "client_ed25519", "chrono", "rust_decimal"] }
```

### 4.2 修改的源码

将源码中所有 `rustls::crypto::aws_lc_rs` 替换为 `rustls::crypto::ring`：

- `crates/dbx-core/src/db/postgres.rs`
- `crates/dbx-web/src/main.rs`
- `src-tauri/src/lib.rs`

```bash
cd /storage/Users/currentUser/GitProject/dbx
sed -i 's/rustls::crypto::aws_lc_rs/rustls::crypto::ring/g' \
  crates/dbx-core/src/db/postgres.rs \
  crates/dbx-web/src/main.rs \
  src-tauri/src/lib.rs
```

这些是**本地移植补丁**，不应直接提交到上游。

## 5. 前端资源

由于本地 `pnpm build` 受 rolldown binding 限制无法完成，使用官方发布的静态浏览器包中的 `dist/`：

```bash
mkdir -p .portable
# 从 GitHub Releases 下载 v0.5.85 arm64 静态浏览器包
curl -sSL -o /tmp/dbx-browser.tar.gz \
  https://github.com/t8y2/dbx/releases/download/v0.5.85/DBX_0.5.85_arm64-browser-static.tar.gz

# 解压后把 dist 放到项目工作区
tar -xzf /tmp/dbx-browser.tar.gz
mv dbx-linux-arm64-browser-static/dist .portable/dist
```

当前项目已经迁移到：

```
/storage/Users/currentUser/GitProject/dbx/.portable/dist
```

## 6. 构建 dbx-web

```bash
cd /storage/Users/currentUser/GitProject/dbx

cargo build --release -p dbx-web \
  --no-default-features \
  --features duckdb-sidecar,mq-admin
```

产物：

```
target/release/dbx-web
```

首次构建耗时较长（本机约 30+ 分钟），因为 release profile 使用 `lto = true`、`codegen-units = 1`。

## 7. 构建 CLI

```bash
cd /storage/Users/currentUser/GitProject/dbx

cargo build --release -p dbx-cli --no-default-features
```

产物：

```
target/release/dbx
```

## 8. 运行

### 8.1 运行 Web 模式

```bash
DBX_STATIC_DIR=/storage/Users/currentUser/GitProject/dbx/.portable/dist \
DBX_DATA_DIR=/storage/Users/currentUser/GitProject/dbx/.portable/data \
DBX_DISABLE_PASSWORD=1 \
DBX_PORT=4224 \
/storage/Users/currentUser/GitProject/dbx/target/release/dbx-web
```

启动后浏览器访问：

```
http://127.0.0.1:4224/
```

### 8.2 运行 CLI

```bash
# 帮助
/storage/Users/currentUser/GitProject/dbx/target/release/dbx --help

# 环境自检
/storage/Users/currentUser/GitProject/dbx/target/release/dbx doctor

# 查询示例
/storage/Users/currentUser/GitProject/dbx/target/release/dbx query <connection> "select * from users limit 10;"
```

## 9. 验证

```bash
# Web 页面
curl -I http://127.0.0.1:4224/

# API 示例
curl http://127.0.0.1:4224/api/app-settings/pinned-tree-node-ids

# CLI
/storage/Users/currentUser/GitProject/dbx/target/release/dbx doctor --json
```

## 10. 常见问题

### 10.1 端口被占用

```text
Failed to bind address: Os { code: 98, kind: AddrInUse, message: "Address in use" }
```

说明 `4224` 已被占用，先停止旧进程：

```bash
pgrep -a -f 'target/release/dbx-web'
kill <PID>
```

或换端口：

```bash
DBX_PORT=4225 ... /storage/Users/currentUser/GitProject/dbx/target/release/dbx-web
```

### 10.2 官方静态二进制无法运行

官方 `DBX_0.5.85_arm64-browser-static.tar.gz` 里的 `dbx-web-bin` 是通用 Linux 静态 musl 二进制，
但当前 HarmonyOS 内核要求静态 ELF 带 `.note.ohos.ident` / OHOS 专属 ELF 结构，
直接执行会报 `Permission denied` / `Operation not permitted`。

因此不能直接使用官方静态二进制，必须使用本机 Rust 工具链从源码构建。

### 10.3 本地前端构建失败

```text
Error loading shared library .../rolldown-binding.openharmony-arm64.node: Permission denied
```

原因是 rolldown 的 OpenHarmony native binding 无法被当前内核加载。
目前绕过方案是使用官方静态包中的现成 `dist/`。

### 10.4 `aws-lc-rs` 链接失败

```text
undefined symbol: aws_lc_0_43_0_ChaCha20_ctr32_neon
```

请确认已应用第 4 节的 ring 补丁。

## 11. 本次构建涉及的本地改动文件

```
Cargo.lock
crates/dbx-core/Cargo.toml
crates/dbx-core/src/db/postgres.rs
crates/dbx-web/Cargo.toml
crates/dbx-web/src/main.rs
src-tauri/Cargo.toml
src-tauri/src/lib.rs
```
