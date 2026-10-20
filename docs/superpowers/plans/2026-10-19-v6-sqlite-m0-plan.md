# PathEditor v6.0.0 M0 风险探针 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在任何生产代码之前，用一组可失败的测试证明 SQLCipher + `rusqlite` + DPAPI + FTS5/JSON1 在本机（GNU）与 CI（MSVC）上可构建、可加密、可导出。

**Architecture:** 新建独立的 `probes/v6-sqlcipher` crate（**不加入 workspace members**），用 `#[test]` 作为探针断言；CI 用 `windows-latest` + 显式 NASM 跑同一组测试。所有结果写入 M0 结果文档，作为 M1 的放行门。

**Tech Stack:** Rust 2021、`rusqlite 0.40.1`（`bundled-sqlcipher-vendored-openssl`）、`windows-sys 0.61.2`、SQLCipher、FTS5、JSON1、`tempfile`（仅测试）。

**Spec:** `docs/superpowers/specs/2026-10-19-v6-sqlite-design.md`（M0 见 §十三；安全模型见 §五）

---

## Global Constraints

- **只做探针**：不改 `core/`、`gui/`、`cli/`、`src/` 的生产代码；`probes/` 不加入 workspace members。
- **不写真实注册表**；不删除任何用户文件；探针数据库只放 `tempfile::TempDir`，测试结束自动清理。
- **不提交、不推送**，除非用户明确授权；未授权时只落文件。
- 中文注释；Rust 4 空格；提交前跑 `cargo fmt --check` 与 `cargo clippy -- -D warnings`（在 probe 目录内）。
- **失败即停**：任一探针失败，不进入 M1；把失败输出写进 M0 结果文档，按 §七 的 fallback 重新拍板。
- 本机工具链是 GNU（`rust-toolchain.toml`）；CI 用 MSVC（与 `release.yml` 相同方式 override）。两边的结果都要记录。

## 前置（用户执行，系统变更）

- [x] **安装 GNU make**：`scoop install make`（main 桶，4.4.1）——**已完成**
- [x] **Perl 模块补树**：Strawberry Perl 提供的 `Locale`/`I18N`/`ExtUtils`/`Pod` 已复制到 `probes/v6-sqlcipher/perl5lib/`（不入库）
- [ ] **CI NASM**：由 workflow 安装，本地不需要。

> **M0 实测补充（已知）**：Windows GNU 下 `cargo test` 无论多线程还是 `--test-threads=1` 都会 `STATUS_STACK_OVERFLOW`；**唯一可行方式是每个测试单独进程**（`cargo test --exact <name>`）。详见 `PathEditor-v6-M0-探针结果.md` §五。

---

## File Structure

| 文件                                                      | 责任                                                                       |
| --------------------------------------------------------- | -------------------------------------------------------------------------- |
| `Cargo.toml`（根）                                        | 增加 `exclude = ["probes/v6-sqlcipher"]`，避免 probe 被当成 workspace 成员 |
| `probes/v6-sqlcipher/Cargo.toml`                          | probe crate 依赖与 features                                                |
| `probes/v6-sqlcipher/src/lib.rs`                          | `open_encrypted` + DPAPI `protect`/`unprotect`                             |
| `probes/v6-sqlcipher/tests/sqlcipher.rs`                  | SQLCipher / FTS5 / JSON1 / export / VACUUM INTO 测试                       |
| `probes/v6-sqlcipher/tests/dpapi.rs`                      | DPAPI roundtrip + DACL 符号编译测试                                        |
| `.github/workflows/v6-sqlcipher-probe.yml`                | CI（MSVC + NASM）跑同一组测试                                              |
| `docs/审核和开发/2026.10.19/PathEditor-v6-M0-探针结果.md` | 实际命令/输出/结论                                                         |

---

### Task 1: Probe crate 与加密库打开

**Files:**

- Modify: `Cargo.toml`（根，`[workspace]` 段）
- Create: `probes/v6-sqlcipher/Cargo.toml`
- Create: `probes/v6-sqlcipher/src/lib.rs`
- Test: `probes/v6-sqlcipher/tests/sqlcipher.rs`

**Interfaces:**

- Consumes: 无
- Produces:
  - `pub fn open_with_key(path: &Path, key_hex: &str) -> rusqlite::Result<Connection>`（只设 key）
  - `pub fn open_encrypted_with(path: &Path, key_hex: &str, memory_security: bool) -> rusqlite::Result<Connection>`
  - `pub fn open_encrypted(path: &Path, key_hex: &str) -> rusqlite::Result<Connection>`（= `open_encrypted_with(..., true)`）

- [ ] **Step 1: 写失败测试**

创建 `probes/v6-sqlcipher/tests/sqlcipher.rs`：

```rust
use tempfile::TempDir;

#[test]
fn sqlcipher_builds_and_opens() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("probe.db");
    let key = "01".repeat(32); // 32 字节 = 64 hex
    let conn = v6_sqlcipher_probe::open_encrypted(&path, &key).expect("SQLCipher 打开失败");

    let version: String = conn
        .query_row("PRAGMA cipher_version", [], |row| row.get(0))
        .expect("cipher_version 查询失败");
    assert!(!version.is_empty(), "cipher_version 不能为空");

    conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT NOT NULL)", [])
        .expect("建表失败");
    conn.execute("INSERT INTO t (v) VALUES ('ok')", []).expect("插入失败");
    let value: String = conn
        .query_row("SELECT v FROM t WHERE id = 1", [], |row| row.get(0))
        .expect("查询失败");
    assert_eq!(value, "ok");
}
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `cd probes/v6-sqlcipher && cargo test sqlcipher_builds_and_opens -- --nocapture`
Expected: FAIL —— crate 不存在 / `open_encrypted` 未定义。

- [ ] **Step 3: 建 crate 与最小实现**

根 `Cargo.toml` 的 `[workspace]` 段改为：

```toml
[workspace]
resolver = "2"
members = [
    "core",
    "gui",
    "cli",
]
exclude = ["probes/v6-sqlcipher"]
```

创建 `probes/v6-sqlcipher/Cargo.toml`：

```toml
[package]
name = "v6-sqlcipher-probe"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
rusqlite = { version = "0.40", features = ["bundled-sqlcipher-vendored-openssl"] }

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.61.2", features = [
    "Win32_Foundation",
    "Win32_Security",
    "Win32_Security_Authorization",
    "Win32_Security_Cryptography",
    "Win32_Storage_FileSystem",
    "Win32_System_Memory",
] }

[dev-dependencies]
tempfile = "3"
```

创建 `probes/v6-sqlcipher/src/lib.rs`：

```rust
//! v6 SQLCipher 探针：只验证构建与加密行为，不进入生产代码。

use rusqlite::Connection;
use std::path::Path;

/// 只打开连接并设置 key，不触碰数据库页；用于错误密钥测试。
pub fn open_with_key(path: &Path, key_hex: &str) -> rusqlite::Result<Connection> {
    assert_eq!(key_hex.len(), 64, "密钥必须是 32 字节（64 个 hex 字符）");
    let conn = Connection::open(path)?;
    // SQLCipher 的 key PRAGMA 必须用双引号包住 x'...'，不能用参数绑定。
    conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", key_hex))?;
    Ok(conn)
}

/// 打开、校验密钥，然后设置 v6 的 PRAGMA。
pub fn open_encrypted_with(
    path: &Path,
    key_hex: &str,
    memory_security: bool,
) -> rusqlite::Result<Connection> {
    let conn = open_with_key(path, key_hex)?;
    // 先做一次只读查询，让 SQLCipher 用 HMAC 校验密钥；错误密钥在这里返回 Err，
    // 不会先被 journal_mode=WAL 的写操作触发 panic。
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
        row.get::<_, i64>(0)
    })?;
    let memory = if memory_security { "ON" } else { "OFF" };
    conn.execute_batch(&format!(
        "PRAGMA cipher_memory_security = {memory};
         PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;"
    ))?;
    Ok(conn)
}

/// 默认开启 cipher_memory_security 的便捷入口。
pub fn open_encrypted(path: &Path, key_hex: &str) -> rusqlite::Result<Connection> {
    open_encrypted_with(path, key_hex, true)
}
```

- [ ] **Step 4: 运行测试，确认通过**

Run: `cd probes/v6-sqlcipher && cargo test sqlcipher_builds_and_opens -- --nocapture`
Expected: PASS。**若构建失败**，先记录错误，跳到 §七 的决策门，不要继续 Task 2。

- [ ] **Step 5: 格式化并提交**

```bash
cargo fmt
git add Cargo.toml probes/v6-sqlcipher
git commit -m "test(v6): add SQLCipher probe crate and encrypted open"
```

---

### Task 2: 错误密钥必须失败

**Files:**

- Test: `probes/v6-sqlcipher/tests/sqlcipher.rs`

**Interfaces:**

- Consumes: `open_encrypted`
- Produces: 无

- [ ] **Step 1: 写失败测试**

在 `tests/sqlcipher.rs` 追加：

```rust
#[test]
fn wrong_key_is_rejected_and_original_survives() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("wrong-key.db");
    let key_a = "11".repeat(32);
    let key_b = "22".repeat(32);

    {
        let conn = v6_sqlcipher_probe::open_encrypted(&path, &key_a).unwrap();
        conn.execute("CREATE TABLE t (v TEXT)", []).unwrap();
        conn.execute("INSERT INTO t VALUES ('keep')", []).unwrap();
    }

    // 只设 key，不应用会触碰页面的 PRAGMA；错误密钥在查询时被 HMAC 拒绝。
    let wrong = v6_sqlcipher_probe::open_with_key(&path, &key_b).unwrap();
    let result = wrong.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
        row.get::<_, i64>(0)
    });
    assert!(result.is_err(), "错误密钥必须无法查询");
    drop(wrong);

    // 错误尝试后，原库仍可用正确密钥打开并读到数据。
    let ok = v6_sqlcipher_probe::open_encrypted(&path, &key_a).unwrap();
    let value: String = ok.query_row("SELECT v FROM t", [], |row| row.get(0)).unwrap();
    assert_eq!(value, "keep");
}
```

- [ ] **Step 2: 运行测试**

Run: `cd probes/v6-sqlcipher && cargo test wrong_key_is_rejected_and_original_survives -- --nocapture`
Expected: **PASS** —— 错误密钥由 SQLCipher 的 HMAC 在首次读取时拒绝；原库仍可正常打开。若 FAIL，这是重大发现：记录并停止，检查 feature / PRAGMA 是否正确。

- [ ] **Step 3: 无需实现**

这是行为探针，不是 TDD 红绿；测试保留为回归守卫。

- [ ] **Step 4: 提交**

```bash
git add probes/v6-sqlcipher/tests/sqlcipher.rs
git commit -m "test(v6): reject wrong SQLCipher key"
```

---

### Task 3: FTS5 与 JSON1 可用

**Files:**

- Test: `probes/v6-sqlcipher/tests/sqlcipher.rs`

**Interfaces:**

- Consumes: `open_encrypted`
- Produces: 无

- [ ] **Step 1: 写失败测试**

在 `tests/sqlcipher.rs` 追加：

```rust
#[test]
fn fts5_and_json1_are_available() {
    let dir = TempDir::new().unwrap();
    let conn =
        v6_sqlcipher_probe::open_encrypted(&dir.path().join("fts.db"), &"33".repeat(32))
            .unwrap();

    let fts: i64 = conn
        .query_row("SELECT sqlite_compileoption_used('ENABLE_FTS5')", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(fts, 1, "bundled SQLCipher 必须启用 FTS5");

    conn.execute_batch(
        "CREATE VIRTUAL TABLE ft USING fts5(content);
         INSERT INTO ft(content) VALUES ('hello path editor');",
    )
    .unwrap();
    let hits: i64 = conn
        .query_row("SELECT count(*) FROM ft WHERE ft MATCH 'path'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(hits, 1, "FTS5 查询应命中一条");

    // SQLite >= 3.38 内建 JSON，ENABLE_JSON1 编译选项可能返回 0，故用 json_valid 验收。
    let json: i64 = conn
        .query_row("SELECT json_valid('{\"a\":1}')", [], |row| row.get(0))
        .unwrap();
    assert_eq!(json, 1, "JSON1 函数必须可用");
}
```

- [ ] **Step 2: 运行测试，确认失败或记录结果**

Run: `cd probes/v6-sqlcipher && cargo test fts5_and_json1_are_available -- --nocapture`
Expected: PASS（若 FAIL，记录 `sqlite_compileoption_used` 与 `json_valid` 的实际输出，进入决策门）。

- [ ] **Step 3: 无需实现**

这是编译选项探针，不是代码任务。

- [ ] **Step 4: 提交**

```bash
git add probes/v6-sqlcipher/tests/sqlcipher.rs
git commit -m "test(v6): verify FTS5 and JSON1 compile options"
```

---

### Task 4: `sqlcipher_export` 与 VACUUM/性能诊断

**Files:**

- Test: `probes/v6-sqlcipher/tests/sqlcipher.rs`

**Interfaces:**

- Consumes: `open_encrypted`
- Produces: 无

- [ ] **Step 1: 写失败测试**

在 `tests/sqlcipher.rs` 追加：

```rust
#[test]
fn sqlcipher_export_roundtrip() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("main.db");
    let backup = dir.path().join("backup.db");
    let key = "44".repeat(32);

    let conn = v6_sqlcipher_probe::open_encrypted(&db, &key).unwrap();
    conn.execute_batch("CREATE TABLE t (v TEXT); INSERT INTO t VALUES ('x');")
        .unwrap();
    conn.execute_batch(&format!(
        "ATTACH DATABASE '{}' AS backup KEY \"x'{}'\";
         SELECT sqlcipher_export('backup');
         DETACH DATABASE backup;",
        backup.display(),
        key
    ))
    .unwrap();

    let restored = v6_sqlcipher_probe::open_encrypted(&backup, &key).unwrap();
    let value: String = restored
        .query_row("SELECT v FROM t", [], |row| row.get(0))
        .unwrap();
    assert_eq!(value, "x");
}

#[test]
fn vacuum_into_parameters_are_recorded() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("v.db");
    let out = dir.path().join("v-out.db");
    let key = "55".repeat(32);

    let conn = v6_sqlcipher_probe::open_encrypted(&db, &key).unwrap();
    conn.execute_batch("CREATE TABLE t (v TEXT); INSERT INTO t VALUES ('x');")
        .unwrap();
    conn.execute_batch(&format!("VACUUM INTO '{}'", out.display()))
        .unwrap();

    // 诊断：记录产物是否加密与 cipher 参数；不作为 CI 门，也不作为备份路径。
    let header = std::fs::read(&out).unwrap();
    let encrypted = !header.starts_with(b"SQLite format 3\0");
    let page_size: i64 = conn
        .query_row("PRAGMA cipher_page_size", [], |row| row.get(0))
        .unwrap_or(0);
    let kdf_iter: i64 = conn
        .query_row("PRAGMA cipher_kdf_iter", [], |row| row.get(0))
        .unwrap_or(0);
    println!(
        "VACUUM INTO diagnostic: encrypted={encrypted} header={:02x?} cipher_page_size={page_size} cipher_kdf_iter={kdf_iter}",
        &header[..16.min(header.len())]
    );

    if let Ok(copy) = v6_sqlcipher_probe::open_encrypted(&out, &key) {
        let n: i64 = copy
            .query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get(0))
            .unwrap();
        assert!(n >= 1);
    }
}

#[test]
fn cipher_memory_security_benchmark_is_recorded() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("bench.db");
    let key = "77".repeat(32);
    {
        let conn = v6_sqlcipher_probe::open_encrypted(&path, &key).unwrap();
        conn.execute_batch("CREATE TABLE t (v TEXT); INSERT INTO t VALUES ('x');")
            .unwrap();
    }
    for (label, on) in [("ON", true), ("OFF", false)] {
        let conn = v6_sqlcipher_probe::open_encrypted_with(&path, &key, on).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..100 {
            let _: String = conn
                .query_row("SELECT v FROM t", [], |row| row.get(0))
                .unwrap();
        }
        println!(
            "cipher_memory_security={label}: 100 queries in {:?}",
            start.elapsed()
        );
    }
    // 诊断，不设时间断言（CI 噪声）。
}
```

- [ ] **Step 2: 运行测试**

Run: `cd probes/v6-sqlcipher && cargo test sqlcipher_export_roundtrip vacuum_into_parameters_are_recorded cipher_memory_security_benchmark_is_recorded -- --nocapture`
Expected: PASS。`sqlcipher_export` 是硬门；VACUUM 与 benchmark 只打印诊断信息（是否加密、cipher 参数、ON/OFF 耗时），**不因明文或耗时失败**。把打印结果写入 M0 结果文档。

- [ ] **Step 3: 提交**

```bash
git add probes/v6-sqlcipher/tests/sqlcipher.rs
git commit -m "test(v6): verify SQLCipher export and VACUUM INTO encryption"
```

---

### Task 5: DPAPI roundtrip 与 DACL 符号编译

**Files:**

- Modify: `probes/v6-sqlcipher/src/lib.rs`
- Create: `probes/v6-sqlcipher/tests/dpapi.rs`

**Interfaces:**

- Produces: `pub fn protect(plain: &[u8]) -> Result<Vec<u8>, String>`、`pub fn unprotect(cipher: &[u8]) -> Result<Vec<u8>, String>`（`#[cfg(windows)]`）

- [ ] **Step 1: 写失败测试**

创建 `probes/v6-sqlcipher/tests/dpapi.rs`：

```rust
#![cfg(windows)]

#[test]
fn dpapi_roundtrip() {
    let plain = b"path-editor-secret-probe";
    let cipher = v6_sqlcipher_probe::dpapi::protect(plain).expect("CryptProtectData 失败");
    assert_ne!(&cipher[..], &plain[..], "密文不能等于明文");
    let back = v6_sqlcipher_probe::dpapi::unprotect(&cipher).expect("CryptUnprotectData 失败");
    assert_eq!(back, plain);
}

#[test]
fn dacl_api_symbols_compile() {
    use windows_sys::Win32::Security::Authorization::{
        GetNamedSecurityInfoW, SetNamedSecurityInfoW, SE_FILE_OBJECT,
    };
    let _ = SE_FILE_OBJECT;
    let _ = GetNamedSecurityInfoW;
    let _ = SetNamedSecurityInfoW;
}
```

- [ ] **Step 2: 运行测试，确认失败**

Run: `cd probes/v6-sqlcipher && cargo test --test dpapi -- --nocapture`
Expected: FAIL —— `dpapi` 模块不存在 / 依赖 feature 未启用。

- [ ] **Step 3: 实现 DPAPI 模块**

在 `src/lib.rs` 追加：

```rust
#[cfg(windows)]
pub mod dpapi {
    //! Windows DPAPI 探针：验证 v6 的本地密钥/备份加密路径可用。

    use windows_sys::Win32::Foundation::{LocalFree, HLOCAL};
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN,
    };

    fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        }
    }

    /// 用当前 Windows 用户作用域的 DPAPI 加密。
    pub fn protect(plain: &[u8]) -> Result<Vec<u8>, String> {
        let input = blob(plain);
        let mut output = CRYPT_INTEGER_BLOB::default();
        // SAFETY: input 指向有效切片；output 由 DPAPI 填充，随后用 LocalFree 释放。
        let ok = unsafe {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err(format!(
                "CryptProtectData 失败: {}",
                std::io::Error::last_os_error()
            ));
        }
        let bytes = unsafe {
            std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec()
        };
        unsafe {
            LocalFree(output.pbData as HLOCAL);
        }
        Ok(bytes)
    }

    /// 解密 DPAPI 密文。
    pub fn unprotect(cipher: &[u8]) -> Result<Vec<u8>, String> {
        let input = blob(cipher);
        let mut output = CRYPT_INTEGER_BLOB::default();
        // SAFETY: 同上；ppszdatadescr 允许为 null。
        let ok = unsafe {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err(format!(
                "CryptUnprotectData 失败: {}",
                std::io::Error::last_os_error()
            ));
        }
        let bytes = unsafe {
            std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec()
        };
        unsafe {
            LocalFree(output.pbData as HLOCAL);
        }
        Ok(bytes)
    }
}
```

- [ ] **Step 4: 运行测试，确认通过**

Run: `cd probes/v6-sqlcipher && cargo test --test dpapi -- --nocapture`
Expected: PASS（2 个测试）。

- [ ] **Step 5: 提交**

```bash
cargo fmt
git add probes/v6-sqlcipher/src/lib.rs probes/v6-sqlcipher/tests/dpapi.rs
git commit -m "test(v6): verify DPAPI roundtrip and DACL symbols"
```

---

### Task 6: CI（MSVC + NASM）跑同一组探针

**Files:**

- Create: `.github/workflows/v6-sqlcipher-probe.yml`

**Interfaces:**

- Consumes: probe crate
- Produces: CI 结果（记录到 M0 结果文档）

- [ ] **Step 1: 写 workflow**

```yaml
name: v6-sqlcipher-probe

on:
  push:
    branches: [main, 'feat/**', 'v6/**']
    paths:
      - 'probes/v6-sqlcipher/**'
      - '.github/workflows/v6-sqlcipher-probe.yml'
  pull_request:
    paths:
      - 'probes/v6-sqlcipher/**'
      - '.github/workflows/v6-sqlcipher-probe.yml'
  # workflow_dispatch 只在 workflow 进入默认分支后可用；分支上用 push/PR 触发。

jobs:
  probe-msvc:
    runs-on: windows-latest
    defaults:
      run:
        working-directory: probes/v6-sqlcipher
    steps:
      - uses: actions/checkout@v4

      - name: Use MSVC toolchain
        shell: pwsh
        working-directory: .
        run: |
          rustup toolchain install stable-x86_64-pc-windows-msvc --profile minimal
          rustup override set stable-x86_64-pc-windows-msvc
          rustc -Vv

      - name: Install NASM and Perl
        shell: pwsh
        working-directory: .
        run: |
          choco install nasm strawberryperl -y --no-progress
          echo "C:\ProgramData\chocolatey\bin" >> $env:GITHUB_PATH
          nasm -v
          perl --version

      - name: Run probes (MSVC)
        run: cargo +stable-x86_64-pc-windows-msvc test -- --nocapture
```

- [ ] **Step 2: 本地无法验证 CI 时，先本地跑 GNU**

Run: `cd probes/v6-sqlcipher && cargo test -- --nocapture`
Expected: PASS（GNU 工具链）。记录本机是否有 NASM（当前无 → 这是 no-asm 实测）。

- [ ] **Step 3: 触发 CI（需用户授权推送/PR）**

推送分支（需用户授权）由 `push` 触发，或开 PR 由 `pull_request` 触发。
`gh workflow run v6-sqlcipher-probe.yml` 在分支上会 **404**——`workflow_dispatch` 要求 workflow 已在默认分支。
Expected: 绿色；记录 `rustc -Vv`、`nasm -v`、`cargo test` 输出。

- [ ] **Step 4: 提交**

```bash
git add .github/workflows/v6-sqlcipher-probe.yml
git commit -m "ci(v6): add SQLCipher probe workflow (MSVC + NASM)"
```

---

### Task 7: 写 M0 结果与放行门

**Files:**

- Create: `docs/审核和开发/2026.10.19/PathEditor-v6-M0-探针结果.md`

- [ ] **Step 1: 写结果文档**

模板（实际输出由执行者粘贴）：

```markdown
# PathEditor v6.0.0 M0 探针结果

- 日期：
- 本机：Windows / Rust `rustc -Vv` / NASM：有/无 / Perl：
- CI：run 链接 / rustc / NASM：

| 探针                            | 本机（GNU） | CI（MSVC） | 结论 |
| ------------------------------- | ----------- | ---------- | ---- |
| SQLCipher 打开 + cipher_version |             |            |      |
| 错误密钥拒绝                    |             |            |      |
| FTS5                            |             |            |      |
| JSON1 (`json_valid`)            |             |            |      |
| `sqlcipher_export`              |             |            |      |
| `VACUUM INTO` 加密性            |             |            |      |
| DPAPI roundtrip                 |             |            |      |
| DACL 符号                       |             |            |      |

## 结论

- [ ] 全部通过 → **放行 M1**
- [ ] 失败 → 记录错误并选择 fallback（系统 OpenSSL / 调整 feature / 重新拍板加密方案），更新 spec 后再决定
```

- [ ] **Step 2: 运行全部探针**

Run: `cd probes/v6-sqlcipher && cargo test -- --nocapture`
Expected: 全部 PASS；把输出粘贴到结果文档。

- [ ] **Step 3: 提交**

```bash
git add docs/审核和开发/2026.10.19/PathEditor-v6-M0-探针结果.md
git commit -m "docs(v6): record M0 SQLCipher probe results"
```

---

## 七、决策门（M0 失败时）

任一 SQLCipher/FTS5/JSON1/DPAPI 探针失败，**停止**，按顺序尝试并记录：

1. **no-asm**：确认失败是否与 NASM 有关（本机无 NASM 时应自动 `no-asm`）；对比 CI（有 NASM）。
2. **系统 OpenSSL**：`bundled-sqlcipher` + vcpkg 的 OpenSSL；记录运行时 DLL 依赖。
3. **换加密 provider**：如果 SQLCipher 在 MSVC 上不可行，回到用户重新拍板（不要静默改用明文 SQLite）。
4. 更新 `docs/superpowers/specs/2026-10-19-v6-sqlite-design.md` 的 §3.4/§十三，并取得用户确认后才进入 M1。

---

## 八、M0 计划核对轮裁断（2026-10-19）

| 项                             | 裁断                                                                                                                                 | 落点               |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------ | ------------------ | --------------------------------- | -------------- |
| **B1b** 本机缺 `make`          | 确认。`openssl-src` GNU 路径硬编码 `make`；本机只有 `mingw32-make`。**用户执行 `scoop install make`**                                | 前置               |
| **B2** 错误密钥测试 panic      | 采纳：拆 `open_with_key`（只设 key）与 `open_encrypted`（先查询校验再 PRAGMA）；测试验证原库仍可用                                   | Task 1/2           |
| **B3** 同步三态                | 采纳：spec 的 `changes` 增加 `origin` / `applied_at NOT NULL` / `exported_at`；导出条件改为 `origin='local' AND exported_at IS NULL` | spec §4.1/§6.3     |
| **B4** CI 触发                 | 采纳：workflow 用 `push` + `pull_request`；`workflow_dispatch` 需默认分支，分支上 404                                                | Task 6             |
| **H** `.pebak` hash            | 采纳：`payload_hash = BLAKE3(hash_salt                                                                                               |                    | payload)`；salt 存 DPAPI 密钥文件 | spec §5.1/§5.3 |
| **H** `migration_imports` 主键 | 采纳：`PRIMARY KEY (source_path, content_hash)`                                                                                      | spec §4.1          |
| **H** `VACUUM INTO`            | 采纳：诊断项，记录 header/参数，不作 CI 门；备份只用 `sqlcipher_export`                                                              | Task 4 / spec §5.1 |
| **H** `cipher_memory_security` | 采纳：ON/OFF benchmark 诊断，不设时间断言                                                                                            | Task 4             |

> **精度更正**：开发窗口说「NASM 分支只对 MSVC 生效、GNU 不查 NASM」不完全准确——`openssl-src` 的 `is_nasm_ready()` 是 `#[cfg(windows)]`，对 Windows GNU 也会检测；MSVC 专属的是 `nmake` 分支。实用结论不变：GNU 需要 `make`，NASM 可选。

---

## Self-Review

- **Spec coverage**：M0 覆盖 spec §十三 的构建、DPAPI、FTS5/JSON1、`sqlcipher_export`/`VACUUM INTO`；M1–M4 不在本计划内（按里程碑另写）。
- **Placeholder scan**：无 TBD/TODO；结果文档是执行后填写的记录，不是代码占位。
- **Type consistency**：`open_encrypted(&Path, &str) -> rusqlite::Result<Connection>`、`protect/unprotect(&[u8]) -> Result<Vec<u8>, String>` 在测试与实现中一致。
- **已知限制**：`windows-sys` 0.61.2 的 `CryptProtectData` 返回 `BOOL`（i32），`LocalFree` 参数是 `HLOCAL = *mut c_void`；若锁定版本不同，`cargo check` 会暴露签名差异，按编译器提示调整，不改测试意图。

---

## Execution Notes

（留给开发窗口回填：实际命令、失败与 fallback、CI run 链接、未覆盖项）
