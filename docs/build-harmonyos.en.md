# Building DBX on HarmonyOS (aarch64 / musl)

> Target environment: HarmonyOS (HongMeng Kernel), `aarch64`, musl libc.
> This document explains how to build `dbx-web` and `dbx` (CLI) from source on this system and run DBX in Web mode.

## 1. Summary

DBX can be ported to the current HarmonyOS system. The recommended deployment mode is **Web mode**:

- `dbx-web`: Rust Web backend that serves the frontend and the `/api/*` endpoints.
- `dbx`: Rust command-line client.

The Tauri desktop mode requires system WebView/GTK/WebKit components and is not recommended as a porting target on this HarmonyOS environment.

## 2. Prerequisites

| Item | Requirement |
|---|---|
| OS | HarmonyOS / HongMeng Kernel, `aarch64` |
| libc | musl (`/lib/ld-musl-aarch64.so.1`) |
| Rust | Homebrew Rust 1.97.1 (host: `aarch64-unknown-linux-ohos`) |
| Node.js | >= 22.13 (26.x in the current environment) |
| pnpm | 10.27.0 (matches the repository `packageManager`) |
| Toolchain | gcc / clang / make / cmake / python3 |

## 3. Install Dependencies

### 3.1 Rust

```bash
brew install rust
```

Verify:

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

### 3.3 Frontend dependencies

```bash
cd /storage/Users/currentUser/GitProject/dbx
pnpm install --frozen-lockfile
```

> Note: In this environment the rolldown OpenHarmony native binding may fail to load
> (`Permission denied`), so a local `pnpm build` may not complete.
> You can skip the frontend build and use the prebuilt `dist/` from the official static browser package (see Section 5).

## 4. Local Source Patches (Switch TLS Provider to ring)

The first `dbx-web` build fails to link because `aws-lc-rs` is missing AArch64 assembly symbols on the `aarch64-unknown-linux-ohos` target:

```
ld.lld: error: undefined symbol: aws_lc_0_43_0_ChaCha20_ctr32_neon
...
```

Fix: switch the TLS provider from `aws-lc-rs` to `ring`.

### 4.1 Cargo.toml changes

Files:

- `crates/dbx-core/Cargo.toml`
- `crates/dbx-web/Cargo.toml`
- `src-tauri/Cargo.toml`

Key changes:

```toml
# rustls: use the ring provider
rustls = { version = "0.23", default-features = false, features = ["ring", "std", "tls12", "logging"] }

# russh: disable default aws-lc-rs and use ring
russh = { version = "0.60", default-features = false, features = ["flate2", "ring", "rsa"] }

# mysql_async: use the ring-based rustls
mysql_async = { version = "0.37", default-features = false, features = ["default-rustls-ring", "client_ed25519", "chrono", "rust_decimal"] }
```

### 4.2 Source code changes

Replace every `rustls::crypto::aws_lc_rs` with `rustls::crypto::ring` in:

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

These are **local porting patches** and should not be submitted upstream directly.

## 5. Frontend Assets

Because local `pnpm build` is blocked by the rolldown binding issue, use the prebuilt `dist/` from the official static browser package:

```bash
mkdir -p .portable
# Download the v0.5.85 arm64 static browser package from GitHub Releases
curl -sSL -o /tmp/dbx-browser.tar.gz \
  https://github.com/t8y2/dbx/releases/download/v0.5.85/DBX_0.5.85_arm64-browser-static.tar.gz

# Extract and move dist into the project workspace
tar -xzf /tmp/dbx-browser.tar.gz
mv dbx-linux-arm64-browser-static/dist .portable/dist
```

The current project already has:

```
/storage/Users/currentUser/GitProject/dbx/.portable/dist
```

## 6. Build dbx-web

```bash
cd /storage/Users/currentUser/GitProject/dbx

cargo build --release -p dbx-web \
  --no-default-features \
  --features duckdb-sidecar,mq-admin
```

Artifact:

```
target/release/dbx-web
```

The first build takes a long time (about 30+ minutes in this environment) because the release profile uses `lto = true` and `codegen-units = 1`.

## 7. Build the CLI

```bash
cd /storage/Users/currentUser/GitProject/dbx

cargo build --release -p dbx-cli --no-default-features
```

Artifact:

```
target/release/dbx
```

## 8. Run

### 8.1 Run Web mode

```bash
DBX_STATIC_DIR=/storage/Users/currentUser/GitProject/dbx/.portable/dist \
DBX_DATA_DIR=/storage/Users/currentUser/GitProject/dbx/.portable/data \
DBX_DISABLE_PASSWORD=1 \
DBX_PORT=4224 \
/storage/Users/currentUser/GitProject/dbx/target/release/dbx-web
```

Open in a browser:

```
http://127.0.0.1:4224/
```

### 8.2 Run the CLI

```bash
# Help
/storage/Users/currentUser/GitProject/dbx/target/release/dbx --help

# Environment check
/storage/Users/currentUser/GitProject/dbx/target/release/dbx doctor

# Example query
/storage/Users/currentUser/GitProject/dbx/target/release/dbx query <connection> "select * from users limit 10;"
```

## 9. Verification

```bash
# Web page
curl -I http://127.0.0.1:4224/

# API example
curl http://127.0.0.1:4224/api/app-settings/pinned-tree-node-ids

# CLI
/storage/Users/currentUser/GitProject/dbx/target/release/dbx doctor --json
```

## 10. Troubleshooting

### 10.1 Port already in use

```text
Failed to bind address: Os { code: 98, kind: AddrInUse, message: "Address in use" }
```

Another process is already listening on port `4224`. Stop it first:

```bash
pgrep -a -f 'target/release/dbx-web'
kill <PID>
```

Or use another port:

```bash
DBX_PORT=4225 ... /storage/Users/currentUser/GitProject/dbx/target/release/dbx-web
```

### 10.2 The official static binary does not run

The `dbx-web-bin` inside `DBX_0.5.85_arm64-browser-static.tar.gz` is a generic Linux static musl binary.
The current HarmonyOS kernel requires static ELF binaries to carry `.note.ohos.ident` / OHOS-specific ELF structures.
Running the generic binary directly fails with `Permission denied` / `Operation not permitted`.

Therefore, the official static binary cannot be used directly; you must build from source with the local Rust toolchain.

### 10.3 Local frontend build fails

```text
Error loading shared library .../rolldown-binding.openharmony-arm64.node: Permission denied
```

The rolldown OpenHarmony native binding cannot be loaded by the current kernel.
Workaround: use the prebuilt `dist/` from the official static browser package.

### 10.4 `aws-lc-rs` link failure

```text
undefined symbol: aws_lc_0_43_0_ChaCha20_ctr32_neon
```

Make sure the ring patches from Section 4 have been applied.

## 11. Files Changed by This Port

```
Cargo.lock
crates/dbx-core/Cargo.toml
crates/dbx-core/src/db/postgres.rs
crates/dbx-web/Cargo.toml
crates/dbx-web/src/main.rs
src-tauri/Cargo.toml
src-tauri/src/lib.rs
```
