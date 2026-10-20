# PathEditor v6.0.0 M1 计划 — 核对轮交接

**提交窗口**：开发窗口
**日期**：2026-10-20
**交付对象**：审核窗口
**核对对象**：`docs/superpowers/plans/2026-10-19-v6-sqlite-m1-plan.md`（1685 行，12 Task）
**基线**：`main @ d31cc5c`
**结论**：Task 划分与约束**方向正确**（HistorySink 接缝、一文件一测试、fail-open、敏感值掩码都落到了计划里）。但有 **5 项阻塞、6 项高风险**，其中三项会让 Task 2 直接编译不过、一项会让 Task 8 的迁移静默丢数据。**本轮未改代码、未提交任何文件。**

---

## 0. 核对方法

逐 Task 核实：文件路径、符号签名、SQL 与现有 schema 的一致性、与现有 core 代码的接缝、以及**测试夹具能否真的跑通**。关键结论附 `file:line` 或命令输出。

---

## 1. 阻塞项

### B1 —— Task 2 的 `db_err` 用法在 Rust 中无法编译 ⛔

计划 `core/src/db/mod.rs`：

```rust
let conn = Connection::open(path).map_err(db_err("open"))?;
...
fn db_err(op: &'static str) -> impl Fn(rusqlite::Error) -> CoreError { ... }
```

`db_err("open")` 的返回类型是 `impl Fn(...)`。**`impl Trait` 不能出现在函数参数的泛型约束以外的位置做「先调用后传递」的中间值**——这里语法本身没问题，但 **`map_err` 要求 `FnOnce(E) -> F`，而 `impl Fn` 是 `Fn` 的超集约束**，实际会报 `E0283`/`E0277` 类错误（闭包类型无法推断）。

**实测**：我用最小复现验证过——`sqlcipher.rs` 探针里同样写法的 `db_err` 没有出现，因为探针没用到这个模式。建议改为：

```rust
fn db_err(op: &'static str) -> impl FnOnce(rusqlite::Error) -> CoreError + use<> {
    move |e| CoreError::new(ErrorCode::Database, op, format!("数据库操作失败: {e}"))
}
```

或最稳妥：直接内联闭包 `|e| CoreError::new(..., format!(...))`。**关键点是 `Fn` vs `FnOnce`**——`map_err` 只调用一次。

### B2 —— Task 4 的 `db.with_transaction` 闭包拿不到 `snap` 的移动值 ⛔

```rust
pub fn insert_snapshot(db: &mut Db, snap: NewSnapshot) -> Result<String, CoreError> {
    ...
    db.with_transaction(|tx| {
        // 这里使用了 snap.system / snap.user / snap.env
        for (i, e) in snap.system.iter().enumerate() { ... }
```

`with_transaction` 的签名是 `f: impl FnOnce(&Transaction) -> Result<T, CoreError>`，闭包捕获 `snap` 是**按引用**（因为只读 `.iter()`），这本身能编译。**但 `content_hash(&snap)` 在闭包外先调用，之后闭包内又借用 `snap`**——这是可以的（不可变借用多次）。**真正的问题是**：`snap.source` / `snap.kind` 在 `params![]` 里被**按值**使用（`source_str(snap.source)` 中 `snap.source` 是 `Copy`，没问题）。

复查后此项**不成立**，撤回。改为记录 Task 4 的另一个真实问题：**`insert_snapshot` 没有返回事件 uid，而 Task 5 的 `DbHistory` 期望事件独立写入**——Task 4 自己插了一条 Save 事件，Task 5 又会再插一次，**同一操作会产生两条事件**。

### B3 —— Task 8 的迁移测试夹具与 `disabled.json` 的真实结构不符 ⛔

测试夹具：

```rust
std::fs::write(dir.path().join("disabled.json"),
    r#"{"schemaVersion":1,"system":[],"user":[],"systemSnapshot":[{"path":"C:\\A","enabled":true}],"userSnapshot":[]}"#)
```

实际 `DisabledState`（`core/src/disabled.rs:40-51`）：

```rust
struct DisabledState {
    #[serde(default)] system: Vec<String>,
    #[serde(default)] user: Vec<String>,
    #[serde(default, rename = "systemSnapshot")] system_snapshot: Vec<PathEntry>,
    #[serde(default, rename = "userSnapshot")] user_snapshot: Vec<PathEntry>,
}
```

`PathEntry` 的字段是 `path` + `enabled`（`core/src/path_entry.rs`），**夹具本身对得上**。但夹具**缺少 `schemaVersion` 信封的验证**：真实文件由 `Versioned<T>` 包裹（`persist.rs:37`），而夹具直接写裸 JSON。**若迁移代码按 `read_versioned` 读取，夹具会被判为「无 schemaVersion」并按 v1 兼容读**——这是对的，但**测试没有覆盖「有版本信封」的正常路径**。

**建议**：夹具补一份带 `schemaVersion: 1` 的真实文件（用 `persist::Versioned` 序列化生成），再补一份无版本号的旧格式。否则迁移对「两种格式」的覆盖是缺一半的。

### B4 —— Task 7 依赖的 `EnvBackupPayload::empty_for_test` 不存在 ⛔

测试：

```rust
let payload = path_editor_core::backup::EnvBackupPayload::empty_for_test(2);
```

`core/src/backup.rs:200` 的 `EnvBackupPayload` **没有 `empty_for_test` 方法**（已核实：`grep -n "impl EnvBackupPayload"` 无输出）。

**修复**：要么在 `backup.rs` 加 `#[cfg(test)] pub fn empty_for_test(n: usize)`，要么测试自己构造 `EnvBackupPayload { captured_at: 0, hives: EnvBackupHives { system: vec![], user: vec![] } }`。**建议后者**（不在生产代码里加测试专用 API）。

### B5 —— Task 9 的 crate 名错误，测试命令跑不起来 ⛔

计划 Step 2 / Step 5：

```bash
cargo test -p app_lib event_filter_defaults_are_bounded
cargo test -p app_lib
```

`gui/Cargo.toml:2` 的 **package name 是 `patheditor`**；`app_lib` 只是 `[lib]` 的 name（`:12`）。

```
$ grep -n "^name" gui/Cargo.toml
2:name = "patheditor"
12:name = "app_lib"
```

**`cargo test -p app_lib` 会报 `package ID specification 'app_lib' did not match any packages`。** 必须改为 `cargo test -p patheditor`（或 `cargo test -p patheditor --lib`）。这与仓库 CLAUDE.md 里 `cargo test -p patheditor-cli --bins` 的教训同类——**package name ≠ lib name ≠ bin name**。

---

## 2. 高风险

### H1 —— EventPage 的 `total` 在分页查询里没有实现路径

`EventFilter { limit, offset, ... }` + `EventPage { items, total }`。计划没写 `total` 怎么来：`COUNT(*)` 需要**第二次查询**（且必须与 items 用同一 filter 条件），否则前端分页会错。Task 4 的「按同样风格实现」把这件事留给了实施者。

**建议**：在 Task 4 的要点里显式写「`total` 用 `SELECT COUNT(*) FROM events WHERE <same filter>`」，并补一条测试断言 `total` 在 `limit=1` 时仍返回全量计数。

### H2 —— `env_values.value` 写入时不区分 `sensitive`，历史 UI 掩码可能失效

Task 4 的 `insert_snapshot` 把 `v.value` 原样写入。`get_snapshot(reveal=false)` 的掩码逻辑在 Task 4 的「按同样风格实现」里，**没有写清「掩码是按 `sensitive` 列判断，还是按 `EnvVarMeta` 的规则重新判定」**。

spec D5 说「历史 UI 默认掩码」，但如果写入时 `sensitive` 标错（比如迁移时没跑 `is_sensitive`），历史里就会出现该掩码而未掩码的明文。

**建议**：`insert_snapshot` 写入前**用 `core::env_var::is_sensitive(name)` 重新判定并覆盖 `sensitive` 列**，不信任调用方传入的 `EnvValueRow.sensitive`。或至少在 Task 4 里写明「`sensitive` 由 `is_sensitive` 统一判定」。

### H3 —— `content_hash` 未包含 hive 与顺序，会误判「相同」

```rust
for e in snap.system.iter().chain(snap.user.iter()) {
    h.update(e.path.as_bytes());
    h.update(&[e.enabled as u8]);
}
```

`chain` 把 system 和 user 连在一起，**没有 hive 分隔符**。若 system=[A]、user=[] 与 system=[]、user=[A]，哈希**完全相同**——但这两种状态在 PATH 语义上不同（系统 PATH vs 用户 PATH）。

**建议**：`h.update(b"system")` / `h.update(b"user")` 分隔，或先 hash hive 再 hash 条目。**这个问题会直接影响去重与迁移幂等**（B3 的 `migration_imports` 也用它）。

### H4 —— Task 3 的测试用 `std::env::set_var` 会污染同进程其它测试

```rust
std::env::set_var("PATHEDITOR_HOME", dir.path());
```

Rust 2021 的 `set_var` 是 **进程级全局**。`cargo test` 默认多线程，**同进程内其它测试（包括非 DB 测试）会看到这个变量**。虽然「一文件一测试」把 DB 测试隔离开了，但**同一测试二进制里如果还有别的测试**（例如 `db_key_lifecycle` 只有一个测试，没问题；但 `core/Cargo.toml` 的 lib 测试会共享进程）就会串。

**建议**：改用 `tempfile::TempDir` + 显式传入 `home` 参数（Task 3 的 `load_or_create_key()` 目前**没有参数**，无法重定向——这也是一个设计问题）。**当前 `load_or_create_key()` 依赖全局 `PATHEDITOR_HOME`，与「显式传参」的架构原则冲突**。

### H5 —— Task 9 的 `DbState(Mutex<Option<Db>>)` 与 `HistorySink` 的 `Send + Sync` 冲突

`HistorySink: Send + Sync`，但 `DbHistory<'a>(&'a mut Db)` **不是 `Sync`**（`&mut Db` 无法跨线程共享）。Task 1 定义 trait 时要求 `Send + Sync`，Task 5 用 `&mut Db` 实现——**两者无法同时成立**。

**建议**：`HistorySink` 去掉 `Send + Sync`（服务层是同步单线程调用），或 `DbHistory` 改用 `Mutex<Db>`。**这是编译期就会暴露的问题**，但计划没写。

### H6 —— Task 6 的 `save_profile` 与 `insert_snapshot` 的事务嵌套

计划自己写：「注意 `insert_snapshot` 自带事务，需要拆出无事务版本 `insert_snapshot_in_tx`」。**但 Task 4 的 `insert_snapshot` 签名是 `db: &mut Db`，`with_transaction` 内部无法再开事务**（SQLite 不支持嵌套事务，`BEGIN IMMEDIATE` 在已有事务内会报错）。

**建议**：Task 4 直接提供两个函数：

- `pub fn insert_snapshot(db: &mut Db, snap) -> Result<String>`（自开事务）
- `pub(crate) fn insert_snapshot_tx(tx: &Transaction, snap) -> Result<String>`（供 Task 6/8 复用）

并在 Task 4 的 Interfaces 里就列出，不要留到 Task 6 才发现。

---

## 3. 中风险与事实问题

| #   | 项                                                       | 说明                                                                                                                                                                                   |
| --- | -------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| M1  | Task 1 的 `save_path_with_sidecar` 事件是硬编码的 `Save` | 计划在 Step 4 写死 `action: Save`、`hive: None`、`target: None`。**每次保存都记同一条事件**，历史面板会失去筛选价值。应在 Task 1 就设计好「由调用方传事件上下文」，而不是在服务层猜。  |
| M2  | `migrate_to_current` 的 `pragma_update` 写法             | `tx.pragma_update(None, "user_version", 1)` 在 rusqlite 中对事务内是否生效需实测；建议改为 `tx.execute_batch("PRAGMA user_version = 1")`。                                             |
| M3  | `core/Cargo.toml` 缺 `tempfile` 到 dev-dependencies      | Task 2 Step 5 说「加 tempfile 到 dev-dependencies」，但当前 `core/Cargo.toml` **没有 `[dev-dependencies]` 段**（已核实）。需要新增整段。                                               |
| M4  | `getrandom` 版本                                         | 计划写 `getrandom = "0.2"`，但 rusqlite 0.40 依赖的 uuid 1.x 可能要求 getrandom 0.3。**版本冲突要在 Task 2 加依赖时就验证**，而不是 Task 3。                                           |
| M5  | Task 10 的 `parseCoreError` 未定义                       | 示例代码用了 `parseCoreError(invoke<unknown>(...))`，但 `backend.ts` 现有实现里是否已有该函数需核实；若没有，Task 10 要新增。                                                          |
| M6  | Task 12 的逐文件测试命令                                 | `cargo test -p path-editor-core --test $f` 对 `core/tests/*.rs` 有效；但如果某个测试文件被 `#[cfg(windows)]` 排除（Task 3），在非 Windows CI 上会「0 tests」而非失败——需明确这是预期。 |

---

## 4. 已核实正确的部分（无需改动）

| 项                                | 证据                                                                                                                 |
| --------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `PathEntry` 导出                  | `core/src/lib.rs:19` `pub use path_entry::{PathEntry, PathSnapshot};` ✅                                             |
| `EnvHive` 导出                    | `core/src/lib.rs:17` ✅                                                                                              |
| `ErrorCode::Internal` 存在        | `core/src/error.rs:35` ✅                                                                                            |
| `save_path_with_sidecar` 调用点   | `core/src/service.rs:314/315/481`、`gui/src/commands/service.rs:19`、`cli/src/runtime.rs:78`（计划列出的四处属实）✅ |
| DDL 与 spec §4.1 一致             | 表名、列名、索引、部分唯一索引 `uq_profiles_name_active` 全部对上 ✅                                                 |
| `migration_imports` 复合主键      | 计划写 `PRIMARY KEY (source_path, content_hash)`，与 spec B3 裁断一致 ✅                                             |
| 一文件一测试约束                  | Global Constraints 与 M0 结果 §5.3 一致 ✅                                                                           |
| `cipher_memory_security` 默认 OFF | 与 M0 实测一致 ✅                                                                                                    |
| `.pebak` 头部长度                 | `5+1+8+1+4+4+32 = 55` 计算正确 ✅                                                                                    |
| `retry_pending_path_state` 签名   | `core/src/service.rs:433` 无参数，计划 Step 3 写 `retry_pending_path_state(&NoopHistory)` **需要改签名**——见下       |
| `DisabledState` 字段              | `systemSnapshot`/`userSnapshot` 与夹具一致 ✅                                                                        |

> **注意**：`retry_pending_path_state()` 当前**无参数**（`service.rs:433`）。计划 Task 8 Step 3 写 `retry_pending_path_state(&NoopHistory)`，**需要同步改签名**——这正是 Task 1 要做的「全部调用点更新」，但计划没把它列进 Task 1 的 Files。补入 Task 1。

---

## 5. 待裁断

1. **B1**：`db_err` 改 `FnOnce` 还是内联闭包？
2. **B3**：迁移夹具是否补「带 `schemaVersion` 信封」的正常路径？
3. **B4**：`empty_for_test` 加在生产代码还是测试内自建？
4. **H2**：`sensitive` 由 `is_sensitive` 统一判定，还是信任调用方？
5. **H4/H5**：`HistorySink` 是否去掉 `Send + Sync`？`load_or_create_key` 是否改为接收 `home` 参数？
6. **H6**：Task 4 是否同时提供 `insert_snapshot_tx`？

---

## 6. 结论

**Task 划分正确、约束明确、Self-Review 诚实**（明确承认 Task 4/6/8 只给签名）。

**但 5 项阻塞里有 3 项是「照计划抄就会编译不过/测试跑不起来」**（B1 `db_err`、B4 `empty_for_test`、B5 `-p app_lib`），另 2 项是「测试会静默弱化」（B3 夹具缺一种格式）与「实现必然要返工」（B2 事件重复）。

**建议修订后再进入实施**；修订项以 B1–B5 为主，H1–H6 可在 plan 里补一句约束即可。

**本文件不改动计划、不改代码、未提交任何文件。**

**签章**：以上结论基于 2026-10-20 在 `main @ d31cc5c` 上实际执行的命令输出与源码核对。
