# PathEditor v6.0.0 M0 计划 — 核对轮交接

**提交窗口**：开发窗口
**日期**：2026-10-19
**交付对象**：审核窗口
**核对对象**：`docs/superpowers/specs/2026-10-19-v6-sqlite-design.md`（615 行，收口版）+ `docs/superpowers/plans/2026-10-19-v6-sqlite-m0-plan.md`（646 行）
**基线**：`main @ aae0909`
**结论**：spec 的 14 项裁断全部落实且自洽；M0 计划的 Task 划分与探针内容**方向正确**。但发现 **4 项阻塞、5 项高风险**，其中两项是本机实测出来的：**本机缺 `make`（GNU 路径必用）**、**Task 2 的错误密钥测试会因 `.unwrap()` 先 panic**；另有一项 **spec 级同步语义矛盾**（`applied_at` 过滤导致本地变更永远导不出去）。**本轮未改代码、未提交任何文件。**

---

## 0. 核对方法

- 逐 Task 核对路径、命令、crate/feature 名、windows-sys 符号签名（docs.rs 0.61.2 实查）。
- 实测本机工具链前提（`make` / `nasm` / `perl` / `rustc`）。
- 上游源码核实：`openssl-src` 的 NASM 检测逻辑、rusqlite 0.40 的 feature 链、SQLCipher 的 `VACUUM INTO` 行为。
- 跨节通读 spec：§4.1 schema × §6.3 同步语义 × §6.1 备份索引 × §7 迁移。

---

## 1. 先认错：B1 的「部分驳回」成立

**我上一轮写「缺 NASM → 构建必然失败」是错的。** 上游 `openssl-src` 源码（`src/lib.rs`）明确：

```rust
if target.contains("msvc") {
    if self.is_nasm_ready() { /* 启用汇编 */ } else { configure.arg("no-asm"); }
}
// is_nasm_ready() = `cmd /C where nasm` 成功，或 OPENSSL_RUST_USE_NASM 强制
```

缺 NASM 只是**失去汇编加速**，不是构建失败。审核窗口的裁断正确，计划里的「本地先 no-asm、CI 装 NASM」方案成立。

补充一个精度问题：该 `no-asm` 分支**只对 MSVC 目标生效**。本机是 GNU（`rust-toolchain.toml` = `stable-x86_64-pc-windows-gnu`），GNU 路径根本不查 NASM（OpenSSL 的 mingw64 目标用 gcc 汇编 .s）。所以「本地 no-asm 实测」的措辞只对 MSVC 有意义；对本机 GNU 而言，实际验证的是 n/a。建议在 spec/plan 里把这条写准确。

---

## 2. 阻塞项

### B1（新）—— 本机缺 `make`，GNU 路径的 vendored OpenSSL 构建必失败 ⛔

实测：

```
$ make -v
bash: make: command not found
$ gcc --version → gcc.exe (MinGW-Builds) 15.2.0 ✓
$ perl --version → 5.42.3 (cygwin) ✓
$ where nasm → 未找到
```

`openssl-src` 的 `cmd_make()` 在非 BSD 平台一律 `Command::new("make")`；GNU 目标构建 OpenSSL 时需要 `make`（以及 gcc、perl）。本机三者中缺 **make**。

**结论**：M0 本机探针在 Task 1 Step 4 就会卡在 OpenSSL 构建（`make` 不存在），与 NASM 无关。**这是当前第一优先级的环境前提**，必须先解决（Scoop `make`，或改用系统 OpenSSL）。

**建议**：plan 的 Global Constraints/前置条件里明确列出「本机：gcc + make + perl；CI：MSVC + NASM + Perl」，并在 Task 1 之前加一步 `make -v` 检查（失败即停，记录到结果文档）。

### B2 —— Task 2 的错误密钥测试会先 panic，永远到不了断言 ⛔

计划 `tests/sqlcipher.rs`：

```rust
let conn = v6_sqlcipher_probe::open_encrypted(&path, &key_b).unwrap();  // ← 这里
let result = conn.query_row("SELECT count(*) FROM sqlite_master", ...);
assert!(result.is_err());
```

而 `open_encrypted` 内部执行：

```rust
PRAGMA cipher_memory_security = ON;
PRAGMA journal_mode = WAL;   // ← 触碰数据库文件
```

`PRAGMA journal_mode = WAL` 需要读写数据库头，必然触发 SQLCipher 的 HMAC 校验；**错误密钥会在这一步返回 `SQLITE_NOTADB`**，`open_encrypted` 返回 `Err`，`.unwrap()` 直接 panic。测试以 panic 失败，而计划预期 PASS。

**修复（二选一）**：

- 测试接受「打开阶段即被拒绝」：`match open_encrypted(...) { Err(_) => return /* 符合预期 */, Ok(conn) => { assert!(query.is_err()) } }`；
- 或把 `open_encrypted` 拆成「key-only 打开」与「PRAGMA 设置」两步，让测试能区分打开成功但读取失败。

另建议补一条断言：**错误密钥尝试后，原库用正确密钥仍能打开**（对应 spec §12「错误密钥失败且不产生半开库」——目前计划没有验证「不产生半开库」）。

### B3 —— spec 同步语义矛盾：导出条件 `applied_at IS NULL` 会把本地变更全部排除 ⛔

- `changes.applied_at` 注释：「本地应用时间；NULL = 待同步」（spec §4.1）。
- `export_sync_bundle`：「收集 `changes` 中 `applied_at IS NULL` 的记录」（spec §6.3）。

**矛盾**：本地操作产生的变更在本地**已经应用**（`applied_at` 有值），按导出条件永远导不出去；若把本地变更写成 NULL，则「本地应用时间」的注释又是错的，且导入端的变更也会被置值——两边都没有「已导出」状态。按现文本，导出包要么为空，要么每次全量重复（无游标），两种都不对。

**建议**：把「待同步」与「本地应用」拆成两个字段，例如 `applied_at`（导入落地）+ `exported_at`（已进入导出包）或 `synced_at` + 每设备导出游标（`last_exported_seq`）。M3 之前必须先定死，否则 bundle 的幂等/增量语义无从实现。

### B4 —— `workflow_dispatch` 在 workflow 进入默认分支前不可用，Task 6 Step 3 的命令会 404

GitHub 规定：`workflow_dispatch` 只在 workflow 文件位于**默认分支**时可用。计划把 workflow 提交在 feature 分支后直接 `gh workflow run v6-sqlcipher-probe.yml`，会报「could not find any workflow」。

**修复**：Task 6 Step 3 改为「推送分支并开 PR（`pull_request` 触发）」，或明确需要用户授权先把 workflow 合入 main；`workflow_dispatch` 仅作为合入后的补充入口。

---

## 3. 高风险

### H1 —— `.pebak` 的 `content_hash` 未定义，且可能无法在不解密的情况下计算

spec §6.1：备份索引要记录 `kind/time/hash/size/path`，而 `.pebak` **从文件头读 `variable_count`，不解密**。`content_hash` 若指「明文 payload 的哈希」，就必须解密才能算；若指「文件字节的哈希」，则密文每次 DPAPI 加密都不同（nonce 随机），同一内容的不同备份哈希不同，失去去重意义。

**建议**：明确 `.pebak` 的索引字段——要么 `content_hash = blake3(文件字节)`（只用于完整性/变更检测，不做内容去重），要么在创建时把 `plaintext_hash` 写进**文件头**（明文，泄露内容指纹，需评估）。不要留着「hash 怎么算」让 M1 猜。

### H2 —— `migration_imports` 主键只有 `source_path`，与「source_path + content_hash 判重」的措辞不一致

schema：`source_path TEXT PRIMARY KEY`；§7.3 写「以 `migration_imports(source_path, content_hash)` 判重」。若同一路径的文件内容变了（用户手工改过备份），现主键会**跳过重新导入**；若想按二元组判重，主键应为 `(source_path, content_hash)`（并允许同路径多行）。**建议**：要么改复合主键，要么把措辞改成「source_path 唯一，content_hash 仅审计」。

### H3 —— `VACUUM INTO` 的加密性依赖「默认 cipher 设置」，探针需覆盖页面大小前提

上游证据（SQLCipher issue #338 + `src/vacuum.c` 说明）：加密源库的 `VACUUM INTO` 产物**继承源库的加密设置**；但 issue #338 的实际坑是**自定义 `cipher_default_page_size` 与源库不一致**时产物无法打开。v6 使用 SQLCipher 默认参数，计划里用默认参数探针是对的。

**建议**：在 Task 4 的结果记录里同时记下 `PRAGMA cipher_default_page_size` / `cipher_page_size`，并把「v6 只能用默认 cipher 参数」写进 spec 的安全/备份一节（或 M1 显式设置两者一致）。

### H4 —— Task 4 把「失败」当成「预期发现」，与 Task 6「CI 必须绿」冲突

Task 4 Step 2 写：「Expected: PASS。若 `vacuum_into` 失败（产物为明文），**这是预期的重要发现**……spec 的备份路径只用 `sqlcipher_export`」。但失败即 CI 红，而 Task 6 的整体期望是绿色。一处测试不能同时「失败是预期」和「CI 必须绿」。

**建议**：把 VACUUM INTO 探针改成**观察型**（读取文件头，把结论写入结果文档，断言只要求「可被同密钥打开」），把「必须使用 `sqlcipher_export`」作为 spec 的硬性要求单独保留；或失败时用 `#[ignore]` + 结果文档说明。按上游证据，默认参数下大概率是加密的，但计划措辞需要自洽。

### H5 —— M0 未测量 `cipher_memory_security` 的开销（spec 要求 M0 实测）

spec §5.1 写「`cipher_memory_security`（M3）：M0 实测冷启动/查询开销；若明显，降级为可选配置」，但 M0 计划只在 `open_encrypted` 里打开它，**没有任何计时/对比探针**。

**建议**：Task 1 或 Task 7 加一条简单基准（ON vs OFF 各插入/查询 N 次，记录耗时），结果写入 M0 文档。

---

## 4. 中风险与事实问题

| #   | 项                                                                                                | 说明                                                                                |
| --- | ------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| M1  | `open_encrypted` 只校验 key 长度，不校验 hex 字符                                                 | 非 hex 会在 SQL 层报错；建议加 `chars().all(is_ascii_hexdigit)`，错误信息更明确。   |
| M2  | 计划 Global Constraints 要求 probe 内跑 `cargo clippy -- -D warnings`，但没有任何 Task 步骤执行它 | 建议并入 Task 7 或每个提交步骤。                                                    |
| M3  | Task 1 Step 2 的「确认失败」是 `cd` 失败而非测试失败                                              | 可接受（crate 尚不存在），但结果文档里别把它当成「红灯验证」。                      |
| M4  | CI `choco install nasm strawberryperl`：windows-latest 通常已自带两者                             | 重复安装可能较慢；`choco install` 对已装包一般返回成功，但建议先 `where` 检测再装。 |
| M5  | 计划 Task 5 的 `dpapi` 模块只 `#[cfg(windows)]`，GNU 与 MSVC 都是 windows                         | 没问题；仅提醒 `dpapi.rs` 的 `#![cfg(windows)]` 与模块 cfg 重复但无害。             |
| M6  | `sync_conflicts` 表无 `uid`，与 `changes.uid` 的幂等风格不一致                                    | 若冲突也要跨设备同步/去重，建议补 uid；M3 前定。                                    |

---

## 5. 已核实正确的部分（无需改动）

| 项                                    | 证据                                                                                                                                                                                                                                                    |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| B1 的「openssl-src 自动 no-asm」      | 上游 `src/lib.rs`：MSVC 且 `where nasm` 失败 → `configure.arg("no-asm")`；README 亦载明 `OPENSSL_RUST_USE_NASM` 三态                                                                                                                                    |
| rusqlite 0.40.1 的 feature 链         | `bundled-sqlcipher-vendored-openssl → libsqlite3-sys/bundled-sqlcipher-vendored-openssl → bundled-sqlcipher → openssl-sys/vendored`                                                                                                                     |
| 根 `Cargo.toml` 片段                  | 实际就是 `resolver = "2"` + `members = ["core","gui","cli"]`，计划加 `exclude` 正确且不丢 `[workspace.package]`                                                                                                                                         |
| 工具链满足 MSRV                       | 本机 rustc **1.99.1** ≥ rusqlite 0.40 的 1.88；`rust-toolchain.toml` = GNU stable                                                                                                                                                                       |
| DPAPI 符号签名（docs.rs 0.61.2 实查） | `CryptProtectData` / `CryptUnprotectData` 参数类型与计划逐项匹配；`CRYPTPROTECT_UI_FORBIDDEN: u32`；`CRYPT_INTEGER_BLOB` 有 `Default`；`LocalFree` 在 `Win32::Foundation`；`GetNamedSecurityInfoW`/`SE_FILE_OBJECT` 在 `Win32::Security::Authorization` |
| FTS5 / JSON1 探针设计                 | `sqlite_compileoption_used('ENABLE_FTS5')` + 实际 MATCH；JSON 用 `json_valid`（正确规避 JSON1 编译选项在 ≥3.38 返回 0 的陷阱）                                                                                                                          |
| `sqlcipher_export` SQL                | `ATTACH … KEY "x'…'"` + `sqlcipher_export('backup')` + `DETACH`，语法正确                                                                                                                                                                               |
| probe crate 命名                      | `v6-sqlcipher-probe` → `v6_sqlcipher_probe` 与测试引用一致；`src/lib.rs` 默认 lib target                                                                                                                                                                |
| spec 收口                             | B2–B7、H1–H7、M1–M7 全部落入对应章节，且跨节自洽（除 B3 的同步语义外）                                                                                                                                                                                  |
| `VACUUM INTO` 产物倾向                | SQLCipher issue #338：加密到加密时与 `sqlcipher_export` 行为类似；DeepWiki 载明产物继承源库加密设置（默认参数）                                                                                                                                         |

---

## 6. 待裁断

1. **B1**：本机 `make` 由用户安装（Scoop `make`）还是改用系统 OpenSSL？M0 是否在 Task 1 前加环境检查步骤？
2. **B2**：错误密钥测试选哪种修复？是否补「错误尝试后正确密钥仍可开」断言？
3. **B3**：同步导出状态字段如何改（`synced_at` / `last_exported_seq`）？这是 M3 的前置，但请在 spec 收口时定。
4. **B4**：CI 触发方式（PR / 合入默认分支）？
5. **H1**：`.pebak` 的 `content_hash` 定义（文件字节哈希 vs 文件头明文哈希）？
6. **H2**：`migration_imports` 主键是否改复合？
7. **H4**：VACUUM INTO 探针改为观察型，还是失败即停？
8. **H5**：`cipher_memory_security` 基准是否加入 M0？

---

## 7. 结论

- **spec 收口质量高**：14 项裁断全部落位，B1–B7/H1–H7 的修复相互自洽；`VACUUM INTO`/JSON1 等细节处理得比我上一轮更准确。
- **M0 计划结构正确**：独立 probe crate + 可失败断言 + CI 对照 + 结果文档 + 决策门，是合理的「先撞墙再铺路」。
- **但 M0 还不能开工**：本机缺 `make`（B1），Task 2 的测试会先 panic（B2），CI 触发方式在默认分支前不可用（B4）。这三项都是执行第一步就会撞上的问题。
- **spec 还有一个 M3 级的语义洞**（B3），建议本轮一并修掉，避免 M3 时推倒重来。

**本文件不改动 spec/plan、不改代码、未提交任何文件。**

**签章**：以上结论基于 2026-10-19 在 `main @ aae0909` 上实际执行的命令输出，以及对 openssl-src / libsqlite3-sys / windows-sys 0.61.2 / SQLCipher 上游文档与源码的在线核实。
