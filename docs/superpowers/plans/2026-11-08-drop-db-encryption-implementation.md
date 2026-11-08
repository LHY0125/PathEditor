# 移除数据库加密 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 按任务逐步实施。步骤用 `- [ ]` 复选框跟踪。

**Goal:** 把 `src/core` 的 SQLCipher 全库加密替换为普通 SQLite（`rusqlite` 的 `bundled` 特性），删除由 DB 加密衍生的密钥代码，保留 `.pebak`（DPAPI）与同步 bundle（Argon2id + ChaCha20-Poly1305）不变。**不实现任何数据迁移**：旧加密库由用户手动删除。

**Architecture:** 加密只存在于 `Db` 的打开序列与 `db::crypto` 的密钥材料两处；表结构、事务、CAS、同步、备份索引全部与加密无关。因此改动集中在「打开库」与「密钥材料瘦身为盐」，其余模块只改注释与测试的打开方式。

**Tech Stack:** Rust 2021 workspace（core / cli / gui）、`rusqlite 0.40 bundled`、Windows（DPAPI 保留）。

**Spec:** `docs/superpowers/specs/2026-11-08-drop-db-encryption-design.md`

## Global Constraints

- **不写迁移代码**：不引入 `sqlcipher_export`、不读旧密文库、不做双版本兼容（用户裁断 2026-11-08）。
- **不删除文件**：`probes/v6-sqlcipher/`、`.github/workflows/v6-sqlcipher-probe.yml`、任何测试文件都保留。语义变化靠重写内容，不靠删文件。
- **`.pebak` 与同步 bundle 的加密不得削弱**：`dpapi.rs`、`sync/bundle.rs`、`argon2`、`chacha20poly1305` 一律不动。
- **密钥文件格式不变**：`~/.patheditor/keys/db.key.dpapi` 仍是 49 字节、`version(1)+reserved(32)+hash_salt(16)`、DPAPI 当前用户作用域。改了会让磁盘上既有 `.pebak` 的 `payload_hash` 全部失配。
- **`CURRENT_SCHEMA_VERSION` 保持 4**：表结构未变，不升版本。
- **不提交、不推送、不升 tag**：完成质量门后停下，由用户决定。版本号改写（Task 6）属于计划内改动，是否提交单独授权。
- **不写真实注册表、不打开真实 `~/.patheditor`**：所有验证用 `TempDir`。真实目录的清理由用户手动执行。
- **代码注释用中文**；新增 `pub fn` / `pub(crate) fn` 带中文 `///`。
- `AGENTS.md` 与 `CLAUDE.md` 改完做**字节级**比较（`git hash-object`），不用 `cmp`（忽略 CRLF 会假阳性）。
- 批量文本替换一律 `sed --binary -i`（裸 `sed -i` 会把 CRLF 翻成 LF），改完用 `git ls-files --eol src` 抽查。
- 每任务结束跑快速门：`cargo check --workspace --all-targets`。

---

### Task 1: 换依赖，断开 OpenSSL 构建链

**Files:**

- Modify: `src/core/Cargo.toml`（第 29 行）
- Modify: `.github/workflows/ci.yml`（第 53-58 行附近的 NASM 步骤）
- Verify only: `.github/workflows/release.yml`（无可改项）

- [ ] **Step 1: 换 feature**

`src/core/Cargo.toml`：

```toml
# v7：普通 SQLite（FTS5 内置），不再需要 SQLCipher 与 OpenSSL。
rusqlite = { version = "0.40", features = ["bundled"] }
```

- [ ] **Step 2: 证明 OpenSSL 退出依赖图**

```bash
cargo tree -e normal,build -i openssl-src    # 期望 nothing to print
cargo tree -e normal,build -i libsqlite3-sys # 期望只剩 rusqlite → libsqlite3-sys
```

- [ ] **Step 3: 删除 CI 的 NASM 安装步**

`ci.yml` 中 `choco install nasm` 的整段（含 `$nasm = Get-Command ...` 判断与注释）删除。保留 `rustup override set stable-x86_64-pc-windows-msvc` 与其余步骤。

- [ ] **Step 4: 构建可过**

```bash
cargo check --workspace --all-targets
```

> 从此本地 GNU 构建不再需要 `OPENSSL_SRC_PERL` / `PERL5LIB` / `PATH` 三行；本任务不要求立即清理文档（Task 6 处理）。若本机仍有旧 `target/` 缓存，`cargo check` 可能复用到旧产物——以 `cargo tree` 的输出为准。

---

### Task 2: `Db::open` 取代 `open_encrypted`

**Files:**

- Modify: `src/core/src/db/mod.rs`（`impl Db` 开头、`open_default`）

- [ ] **Step 1: 替换打开实现**

```rust
/// 打开（或创建）普通 SQLite 库。
///
/// 不加密：v7 起数据库文件是标准 SQLite。加密库（v6 及更早）无法打开，
/// 由 [`open_default`] 给出手动删除的指引。
pub fn open(path: &Path) -> Result<Self, CoreError> {
    let conn = Connection::open(path).map_err(|e| db_err("open", e))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )
    .map_err(|e| db_err("pragma", e))?;
    Ok(Self { conn })
}
```

删除原有的：64 位十六进制密钥校验、`PRAGMA key`、`SELECT count(*) FROM sqlite_master` 的密钥校验、`PRAGMA cipher_memory_security`。

- [ ] **Step 2: 旧库（密文）检测（放在 `Db::open` 内部）**

在 `db/mod.rs` 增加私有辅助：

```rust
/// 文件是否以 SQLite 的 16 字节 magic 开头。
///
/// 不存在或空文件视为「可以新建」，返回 `true`；读失败按 IO 错误上抛。
fn has_sqlite_header(path: &Path) -> Result<bool, CoreError> {
    use std::io::Read as _;
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(true),
        Err(e) => return Err(CoreError::new(ErrorCode::Io, "db_open", format!("读取数据库文件失败: {e}"))),
    };
    let mut buf = [0u8; 16];
    let n = f.read(&mut buf).map_err(|e| CoreError::new(ErrorCode::Io, "db_open", format!("读取数据库文件失败: {e}")))?;
    Ok(n == 0 || &buf[..n] == b"SQLite format 3\0")
}
```

`Db::open` 的第一行（在 `Connection::open` 之前）：

```rust
if !has_sqlite_header(path)? {
    return Err(CoreError::new(
        ErrorCode::Database,
        "db_open",
        format!(
            "检测到旧版加密数据库 {}（v6 及更早使用 SQLCipher），v7 不再支持。\
             请删除或重命名该文件（连同同目录的 patheditor.db-wal、patheditor.db-shm）后重启。\
             PATH 编辑不受影响；历史/同步/检索会从空库重建。",
            path.display()
        ),
    ));
}
```

> 检查放在 `Db::open` 而不是 `open_default`：集成测试直接调 `Db::open`，这样 D8 的行为可被单测覆盖，也不会出现「生产有检查、测试造不出」。

`open_default()` 只保留「拼路径 + 调 `Db::open`」。

- [ ] **Step 3: 清掉 `open_default` 的密钥与 DACL 块**

`open_default` 不再调用 `crypto::load_or_create_key`、不再做 DACL 校验（DACL 移到 Task 3 的 `load_or_create_key`）。保留函数签名与文档（GUI/CLI 调用点不变）。

- [ ] **Step 4: 模块文档更新**

`db/mod.rs:1` 「SQLCipher 数据库句柄」→「SQLite 数据库句柄（v7 起不加密）」；第 5-11 行的安全边界清单改为：`crypto.rs`（本机盐与 `.pebak` 密钥材料）、`dpapi.rs`（DPAPI）、`dacl.rs`（盐文件访问控制），并注明「DB 文件本身不加密」。

- [ ] **Step 5: 快速门**

```bash
cargo check --workspace --all-targets
```

此时 `src/core/tests/` 仍引用 `open_encrypted`，`--all-targets` 会报错——预期。本 Step 只要求 `cargo check -p path-editor-core --lib` 通过。

---

### Task 3: 密钥材料瘦身为盐（保留文件格式）

**Files:**

- Modify: `src/core/src/db/crypto.rs`

- [ ] **Step 1: 删字段与 `key_hex`**

```rust
/// 本机密钥材料。v7 起只承载备份去重盐——数据库不再加密。
#[derive(Clone)]
pub struct KeyMaterial {
    /// 备份去重盐：`payload_hash = BLAKE3(hash_salt ‖ canonical_json(payload))`
    pub hash_salt: [u8; 16],
}
```

删除 `db_key` 字段与 `key_hex()` 函数；删除对应单元测试（`key_hex_is_lowercase_64_chars_and_parse_validates`），改为断言 `parse_material` 对错误长度的拒绝。

- [ ] **Step 2: 保留 49 字节格式**

常量与布局不变：

```rust
/// 明文长度：`version(1) + reserved(32) + hash_salt(16)`。
/// 前 32 字节是 v6 的 `db_key` 遗留位置，v7 起不再使用，写入随机值以保持文件长度。
const KEY_PLAIN_LEN: usize = 49;
const SALT_OFFSET: usize = 33;
```

`create_key_locked`：仍 `KEY_FORMAT_VERSION` + 32 随机 + 16 随机盐。`parse_material`：校验 `len == KEY_PLAIN_LEN` 且 `plain[0] == KEY_FORMAT_VERSION`，只取 `plain[SALT_OFFSET..]`。

- [ ] **Step 3: DACL 校验迁到 `load_or_create_key`**

拿到（或创建）材料后：

```rust
#[cfg(windows)]
{
    let key_path = key_path(home);
    match super::dacl::key_dacl_is_restricted(&key_path) {
        Ok(true) => {}
        Ok(false) => log::warn!(
            "密钥文件 {} 的权限过宽（存在非当前用户/SYSTEM 的授权项）。\
             其它用户可能替换它，导致既有 .pebak 备份无法校验。",
            key_path.display()
        ),
        Err(e) => log::warn!("无法校验密钥文件权限: {}", e.message),
    }
}
```

- [ ] **Step 4: 更新模块文档**

`crypto.rs:5` 的文件内容说明改为 `version(1) + reserved(32) + hash_salt(16)`，并写明「数据库不加密；该文件只提供服务备份去重盐」。

- [ ] **Step 5: 快速门**

```bash
cargo check -p path-editor-core --lib
```

---

### Task 4: 迁移 51 个 core 集成测试

**Files:** `src/core/tests/*.rs`（51 个文件、56 处 `open_encrypted` 调用；4 个语义重写）

- [ ] **Step 0: 确认 `sed` 是 GNU sed**

```bash
sed --version | head -1          # 必须含 "GNU sed"
```

若 Shell 里的 `sed` 不是 GNU（例如某些环境预置了 Rust 版 `sed`，报 `unexpected argument '--binary'`），改用 Git 自带的那份绝对路径：`& "$(scoop prefix git)\usr\bin\sed.exe"`。`--binary` 只有 GNU sed 支持，缺了它 `sed -i` 会把 CRLF 整体翻成 LF。

- [ ] **Step 1: 机械替换（先看命中数，再改）**

```bash
grep -rl "Db::open_encrypted" src/core/tests | wc -l          # 记录基线（实测 51 个文件 / 56 处）
grep -rn "open_encrypted" src/core/tests | wc -l
```

```bash
# 1) 删 key_hex 导入（两种形态）
sed --binary -i -E 's/use path_editor_core::db::crypto::\{key_hex, load_or_create_key\};/use path_editor_core::db::crypto::load_or_create_key;/' src/core/tests/*.rs
sed --binary -i -E '/^use path_editor_core::db::crypto::key_hex;$/d' src/core/tests/*.rs

# 2) 打开方式：覆盖实测的三种第二参数形态
sed --binary -i -E 's/Db::open_encrypted\(([^,]+), &key_hex\(&[A-Za-z_][A-Za-z0-9_]*\)\)/Db::open(\1)/g' src/core/tests/*.rs
sed --binary -i -E 's/Db::open_encrypted\(([^,]+), &(key|kh|k)\)/Db::open(\1)/g' src/core/tests/*.rs
sed --binary -i -E 's/Db::open_encrypted\(([^,]+), &key_hex\)/Db::open(\1)/g' src/core/tests/*.rs
```

- [ ] **Step 2: 收尾残留**

```bash
grep -rn "open_encrypted\|key_hex" src/core/tests
```

**实测（2026-11-08，用等价正则对 51 个文件预演）**：三条规则分别命中 43 / 10 / 2 处，共 55 处；剩余 `open_encrypted`/`key_hex` 只出现在 Step 3–5 要整体重写的 3 个文件里：

```text
db_schema_open.rs            Db::open_encrypted(&path, &"cd".repeat(32))
db_schema_v1_to_v2.rs        let key_hex = key_hex(&key); + PRAGMA key 段
db_schema_v2_to_current.rs   let kh = key_hex(&key);
```

```bash
cargo check --workspace --all-targets               # 期望：只剩 unused variable 警告
```

对每个 `unused variable: key` 的文件，删除已经无用的 `let key = load_or_create_key(...);`（`load_or_create_key` 仍被 `.pebak` 相关测试与 `maintenance.rs` 内嵌测试使用，不要从这些文件删）。

- [ ] **Step 3: 重写 `db_schema_open.rs`（覆盖 D8）**

```rust
#[test]
fn db_opens_as_plain_sqlite_and_rejects_legacy_encrypted_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.db");

    let mut db = Db::open(&path).expect("打开失败");
    assert_eq!(db.schema_version().unwrap(), 0);
    db.migrate_to_current().expect("迁移失败");
    db.migrate_to_current().expect("二次迁移失败"); // 幂等
    assert_eq!(db.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);

    for table in ["snapshots", "app_state", "path_entries", "env_values", "events",
                  "profiles", "profile_revisions", "backups", "migration_imports"] {
        assert!(db.table_exists(table).unwrap(), "缺少表 {table}");
    }
    drop(db);

    // 新库是明文 SQLite：文件头必须是 16 字节 magic
    let head = std::fs::read(&path).unwrap()[..16].to_vec();
    assert_eq!(head, b"SQLite format 3\0", "v7 的库必须是明文 SQLite");

    // 重开可读
    assert_eq!(Db::open(&path).unwrap().schema_version().unwrap(), CURRENT_SCHEMA_VERSION);

    // 旧版加密库 → 明确报错，消息含删除指引
    let legacy = dir.path().join("legacy.db");
    std::fs::write(&legacy, [0x6au8, 0x40, 0x7f, 0x68, 0x51, 0xc0, 0x29, 0xe4,
                             0xb2, 0x02, 0x00, 0x6a, 0xb6, 0xd9, 0xb3, 0xaa]).unwrap();
    let err = Db::open(&legacy).map(|_| ()).expect_err("加密库必须被拒绝");
    assert!(err.message.contains("旧版加密数据库"), "实际: {}", err.message);
}
```

> 该用例依赖 Task 2 Step 2 的 `has_sqlite_header` 已在 `Db::open` 内生效；若先写测试，它会红——这正是期望的顺序（先见红，再补实现）。

- [ ] **Step 4: 重写 `db_key_lifecycle.rs`**

保留：盐稳定、非全零、不同目录不同、`<home>/keys/db.key.dpapi` 路径、DACL 断言。
删除：`db_key` 相关断言、「错误密钥打不开库」断言（DB 不再用密钥）。

```rust
let a = load_or_create_key(dir.path()).expect("首次创建失败");
let b = load_or_create_key(dir.path()).expect("二次读取失败");
assert_eq!(a.hash_salt, b.hash_salt, "salt 必须稳定");
assert_ne!(a.hash_salt, [0u8; 16], "salt 不能全零");

let mut db = Db::open(&dir.path().join("k.db")).expect("打开失败");
db.migrate_to_current().expect("迁移失败");
drop(db);
Db::open(&dir.path().join("k.db")).expect("重开后失败");

let d = load_or_create_key(TempDir::new().unwrap().path()).unwrap();
assert_ne!(a.hash_salt, d.hash_salt, "不同目录必须生成不同盐");
```

- [ ] **Step 5: 重写两个 schema 历史测试的建库方式**

`db_schema_v1_to_v2.rs`、`db_schema_v2_to_current.rs`：删除手写 `PRAGMA key` 与 `Connection::open` 的加密构造，改为

```rust
let conn = rusqlite::Connection::open(&path).unwrap();
conn.execute_batch(path_editor_core::db::schema::DDL_V1).unwrap();
conn.execute_batch("PRAGMA user_version = 1").unwrap();
drop(conn);
let mut db = Db::open(&path).unwrap();
db.migrate_to_current().unwrap();
```

- [ ] **Step 6: 行尾与全量测试**

```bash
git ls-files --eol src/core/tests | grep -c "w/crlf"   # 不得低于改动前记录值
cargo test --workspace
```

---

### Task 5: 源码注释与 CLI 注释

**Files:**

- Modify: `src/core/src/db/search.rs:5`
- Modify: `src/core/src/db/sync/merge.rs:3-4`
- Modify: `src/core/src/db/maintenance.rs:267`（内嵌测试注释）
- Modify: `src/cli/src/commands/maintenance.rs:40,66`
- Modify: `src/gui/src-tauri/src/lib.rs:46`（「并发写同一个 SQLCipher 库」→「同一个数据库」）

- [ ] **Step 1: 逐条改**

- `search.rs:5`：把「由 SQLCipher 保护」改为「env 值不写入 `search_fts`；v7 起库文件为明文 SQLite，敏感值的保护依赖 UI 掩码与 `reveal_env_var`」。
- `merge.rs:3-4`：删除「不受一进程一个 SQLCipher 测试约束」，改为「纯函数，不碰数据库」。
- `maintenance.rs:267`：「它用 DPAPI、不建 SQLCipher」→「它用 DPAPI，不建数据库」。
- `cli/commands/maintenance.rs:40,66`：`load_or_create_key` 的说明改为「加载本机盐（v7 起数据库不加密，该文件只服务 `.pebak`）」。

- [ ] **Step 2: 全文复查**

```bash
grep -rn "SQLCipher\|sqlcipher\|PRAGMA key\|cipher_memory_security\|open_encrypted\|key_hex" src/
# 期望：仅 probes 之外无输出
```

---

### Task 6: 文档、版本号、发布说明

**Files:** `CLAUDE.md`、`AGENTS.md`、`README.md`、`ROADMAP.md`、`CHANGELOG.md`、`SECURITY.md`、`docs/dev/{sqlite-storage,testing,architecture,data-and-security,interfaces}.md`、`package.json`、`Cargo.toml`、`src/gui/src-tauri/tauri.conf.json`

- [ ] **Step 1: 版本号四处一致 → 7.0.0**

```bash
grep -n '"version"' package.json src/gui/src-tauri/tauri.conf.json
grep -n '^version' Cargo.toml
```

全部改 `7.0.0`；README 徽章同步。

- [ ] **Step 2: `CLAUDE.md` 与 `AGENTS.md`（同一份内容）**

| 位置（2026-11-08 行号） | 改法                                                                                                                              |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| 15                      | 「SQLCipher 加密数据库（sidecar）」→「SQLite 数据库（sidecar，v7 起不加密）」                                                     |
| 57-63                   | 删除「本地构建环境（Windows + GNU）」整段三行环境变量，改为一句：「v7 起 `rusqlite` 只用 `bundled`，不再需要 OpenSSL/perl/NASM」  |
| 78-80                   | 目录注释 `v6 SQLCipher 数据层` → `数据层（SQLite）`                                                                               |
| 92                      | `probes/v6-sqlcipher` 保留，注明「v6 历史证据，不参与构建」                                                                       |
| 126-129                 | sidecar 行去掉「SQLCipher」；密钥行改为「`db.key.dpapi` 只存 `.pebak` 去重盐（v6 遗留 32 字节保留位）」；删除「一文件一测试」条目 |
| 158-159                 | 基线数字按 Task 7 实测更新（只改一行，数字唯一出处仍是 `docs/dev/testing.md`）；删掉「（SQLCipher）」括注                         |

- [ ] **Step 3: `README.md`**

114、149-152、396：去掉「SQLCipher 加密数据库」，明确「数据库是本地 SQLite，不加密；注册表仍是唯一真相源」。
570：技术表「数据库」行改 `SQLite（rusqlite bundled + FTS5）`。
596-597：目录注释改 `db/ # 数据层（SQLite）`。
651-652：删除 GNU 三行环境变量的整段说明。

- [ ] **Step 4: `docs/dev/`**

- `sqlite-storage.md`：8-9 改「sidecar 是明文 SQLite」+ 密钥文件只存盐；25 删除「一文件一测试」；55-60 整节「OpenSSL 构建成本」删除，替换为「v7 起依赖链 `rusqlite → libsqlite3-sys → SQLite(C)`，无 OpenSSL」。
- `testing.md`：17-22 删除一文件一测试节（改为「v7 起无此约束，存量文件不合并」）；94-101 删除三行环境变量；质量门基线改写（数字来自 Task 7）。
- `architecture.md`：25 改「db/ 数据层（SQLite）」；28 行 `CURRENT_SCHEMA_VERSION = 3` 更正为 **4**（既有文档滞后事实）。
- `data-and-security.md`：11、15 保留 `.pebak`/DPAPI 表述；新增一条：「v7 起 `~/.patheditor/patheditor.db` 不加密，内容与注册表同级的明文副本；`.pebak` 与同步 bundle 不受影响」。
- `interfaces.md`：76、93 退出码 5 的原因收窄为「`.pebak`/同步 bundle 口令/DPAPI 不可用」，删掉「数据库密钥」。

- [ ] **Step 5: `SECURITY.md`**

37 行现称「备份文件格式为纯文本，不加密」与 `.pebak` 事实不符，改为：「`.pebak` 备份经 DPAPI 加密；仅回落 `.json` 时为明文。数据库 `patheditor.db` 自 v7 起为明文 SQLite。」

- [ ] **Step 6: `CHANGELOG.md` + `ROADMAP.md`**

```markdown
## 7.0.0 (2026-11-XX)

### Breaking

- **数据库不再加密**：`~/.patheditor/patheditor.db` 从 SQLCipher 改为标准 SQLite。
  v6 及更早版本的加密库**无法读取**，升级前请删除（或重命名）
  `~/.patheditor/patheditor.db` 及同目录的 `-wal` / `-shm`。
  删除后历史、同步队列、检索与统计会从空库重建；PATH 编辑与注册表不受影响。
- 回退到 6.x 不可行：6.x 会对明文库执行 `PRAGMA key` 并失败。
- 移除 `rusqlite` 的 `bundled-sqlcipher-vendored-openssl`：不再需要 OpenSSL、perl、NASM；
  本地 GNU 构建无需三条环境变量。

### Changed

- `db::Db::open_encrypted` → `db::Db::open`；`db::crypto::KeyMaterial` 只保留 `hash_salt`，
  `key_hex` 删除。密钥文件路径与 49 字节格式不变，既有 `.pebak` 与备份索引继续有效。
- 「一文件一测试」约束随 SQLCipher 退场而取消（存量测试文件不合并）。

### Unchanged

- `.pebak` 备份仍是 DPAPI 加密；同步 bundle 仍是 Argon2id + ChaCha20-Poly1305。
```

`ROADMAP.md` 在 v6.2 段后加一条 `- ✅ **7.0.0**（日期）— 移除数据库加密…`。

- [ ] **Step 7: 格式与字节一致性**

```bash
npm run format:check
git hash-object CLAUDE.md AGENTS.md    # 两个哈希必须相同
```

---

### Task 7: 质量门、烟雾验证、基线

- [ ] **Step 1: 全量质量门**

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npx vitest run
npm run build
```

- [ ] **Step 2: 烟雾验证（仓库内，不碰真实目录）**

1. 新建库文件头：由 Task 4 Step 3 的断言覆盖（`b"SQLite format 3\0"`）。
2. 旧库拒绝：同一用例覆盖（错误消息含「旧版加密数据库」）。
3. 端到端降级：`cargo test -p patheditor-cli --bins` 全绿。
4. `cargo audit` 仍为 0 告警（放行项集中在 `.cargo/audit.toml`，本次未改白名单）。

- [ ] **Step 3: 更新基线（唯一出处）**

只改 `docs/dev/testing.md` 的「质量门」一节，写入实测数字；`CLAUDE.md` / `AGENTS.md` / `README.md` 引用同一组数字（README 徽章 = Rust + 前端之和）。

- [ ] **Step 4: 真实目录的最终验证（由用户手动执行，不属于本计划）**

```text
1) 关闭 PathEditor
2) 删除 %USERPROFILE%\.patheditor\patheditor.db、patheditor.db-wal、patheditor.db-shm
3) 启动 v7 → 历史为空、编辑正常
4) od -An -tx1 -N16 %USERPROFILE%\.patheditor\patheditor.db  → 53 51 4c 69 74 65 20 66 6f 72 6d 61 74 20 33 00
```

---

## 完成判定

```bash
grep -rn "bundled-sqlcipher" --include=Cargo.toml .   # 仅 probes/v6-sqlcipher/Cargo.toml
grep -rn "open_encrypted\|key_hex\|PRAGMA key\|cipher_memory_security" src/   # 无输出
cargo tree -e normal,build -i openssl-src             # nothing to print
cargo test --workspace && npx vitest run              # 全绿，数字已写入 docs/dev/testing.md
```

## 未覆盖项（如实列出）

- 不提供历史导出/迁移；旧库删除由用户完成。
- 不重命名 `db.key.dpapi`、不改 49 字节格式。
- 不合并存量单测试文件。
- 不新增「数据库未加密」的 UI 提示；沿用既有降级条幅。
- 不在本计划内执行 `git commit` / `git push` / 打 tag。
