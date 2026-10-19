# PathEditor v6.0.0 SQLite spec — 核对轮交接

**提交窗口**：开发窗口
**日期**：2026-10-19
**交付对象**：审核窗口
**核对对象**：`docs/superpowers/specs/2026-10-19-v6-sqlite-design.md`（506 行）
**基线**：`main @ aae0909`
**结论**：**总体方向成立**（D1–D15 与核对轮 G1–G5 全部落实）。但 **7 项阻塞、7 项高风险**必须先解决或明确，其中 **M0 有一个本机可复现的硬障碍（NASM 缺失）**、**schema 有 4 处自洽性问题**（user_version、source_path、lamport、软删除唯一索引）、**备份明文策略存在 D7 与 §5.3 的直接矛盾**。**本轮未改代码、未提交任何文件。**

---

## 0. 核对方法

逐节核对：M0 构建前提（本机实测 + 上游源码）、schema 与 §6.3 同步语义的自洽性、迁移路径（含幂等键）、与现有 core 契约的接缝（`save_path_with_sidecar`、`ErrorCode`、`RESERVED_NAMES`）、以及 D7 与 §5.3 的一致性。关键结论附命令输出或 file:line。

---

## 1. M0 前置实测：本机缺 NASM ⛔（B1 的证据）

```
$ nsam -v  → bash: nasm: command not found
$ perl --version → perl 5, version 42, subversion 3 (v42.3) built for x86_64-cygwin-thread-multi
```

`rusqlite = { features = ["bundled-sqlcipher-vendored-openssl"] }` 已在 rusqlite 0.40.1 的 `Cargo.toml` 中确认存在（feature 链：`bundled-sqlcipher-vendored-openssl → libsqlite3-sys/bundled-sqlcipher-vendored-openssl → bundled-sqlcipher → openssl-sys/vendored`）。**openssl-sys 的 vendored 构建在 Windows x86_64 上必须调用 NASM 汇编器**；本机没有 NASM，M0 探针在当前环境**必然失败**。

**建议**：

- M0 开始前由用户安装 NASM（Scoop `main` 桶有 `nasm`；属系统变更，需用户执行）。
- Cygwin Perl 是否被 openssl-sys 接受需实测；若报错，改用 Strawberry Perl。
- CI：`release.yml` 用 `windows-latest`，构建前必须显式确认/安装 NASM 与 Perl，**不要假设 runner 自带**；M0 应把 CI 构建也纳入探针（本地过 ≠ CI 过）。

---

## 2. 阻塞项

### B1 —— M0 构建前提未验证，且本机已确认缺 NASM

见 §1。M0 是硬门，spec 已写对，但**必须在动任何业务代码前实测**；本机当前状态跑不通。这是 spec 的第一个可执行动作，建议 plan 的 M0 第一个 Task 就是它。

### B2 —— `PRAGMA user_version = 1` 放在每次启动的 PRAGMA 块里，会重置迁移版本 ⛔

spec §3.3：

```sql
PRAGMA key = ...;
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA user_version = 1;   ← 每次启动都执行
```

`user_version` 是**写操作**。首次建库时设 1 没问题；但将来 schema 迁移到 v2 后，这一行会在**下一次启动时把版本重置回 1**，导致迁移判断错乱（重跑迁移 / 拒绝启动 / 写入错误结构）。正确做法：`user_version` 只在**建库与迁移脚本内**设置，启动时只**读**。

### B3 —— 迁移幂等键 `source_path` 在 schema 中不存在

spec §7.3 写「`source_path + content_hash` 判重」，但 §4.1 的 `snapshots` 表**没有 `source_path` 列**（`backups` 表有 `file_path`，snapshots 没有）。迁移重跑时无法按来源判重，会重复导入或只能靠 uid 随机生成后无法识别。

**建议**：`snapshots` 增加 `source_path TEXT`（可空）+ `UNIQUE(source_path, content_hash) WHERE source_path IS NOT NULL` 的部分唯一索引；或单独建 `migration_imports(source_path, content_hash)` 表。

### B4 —— `changes` 表缺 `lamport`，与「LWW（lamport + device_id）」合并规则不匹配

spec §6.3 定义字段级 LWW 用 `lamport` 掐尖，但 §4.1 的 `changes` 只有 `seq/created_at/applied_at`，**没有 `lamport` 列**；`app_state.lamport` 只是单值计数器。导入时必须知道每条变更的逻辑时间，否则无法比较。

**建议**：`changes` 增加 `lamport INTEGER NOT NULL`（导出时写入，导入时参与比较），`app_state.lamport` 仅作本地单调源。

### B5 —— `profiles.name UNIQUE` 与软删除冲突

`profiles` 有 `deleted_at`（墓碑），但 `name TEXT NOT NULL UNIQUE` 是全表唯一。用户删除配置 A 后，用同名再建一个新配置会**唯一约束冲突**（墓碑行仍占名）。

**建议**：改为部分唯一索引 `CREATE UNIQUE INDEX uq_profiles_name ON profiles(name) WHERE deleted_at IS NULL;`（SQLite 支持 partial index）。

### B6 —— D7（保留明文备份文件）与 §5.3（新代码不得再生成明文敏感文件）直接矛盾 ⛔

- D7 / §6.1：备份文件保留，用户可手工打开。
- §5.3：「新代码不得再生成新的明文敏感文件（迁移后备份写加密包或加密文件）」。

两条同时成立时，**v6 之后 `patheditor env backup` 写出的文件到底是什么**没有定义：

- 若仍是明文 JSON → 违反 §5.3；
- 若改为加密文件 → 「用户可手工打开」失效，且需要定义加密密钥（DPAPI 本地密钥？口令？）与 `env restore <FILE>` 的兼容性。
- 迁移期（M1–M3）旧代码路径还在写明文，时间边界也要说明。

**必须裁断**：新备份的格式与密钥，或明确接受明文并删掉 §5.3 的承诺。

### B7 —— DB 打开失败时的应用行为未定义（sidecar 应否 fail-open）

spec 只写了「密钥丢失 → 明确报错，不自动重建空库」（§5.1），但没有说清楚是**整个应用拒绝启动**还是**仅历史功能不可用**。DB 是 sidecar、注册表是真相源——如果因为历史库读不了就让用户无法编辑 PATH，与定位矛盾；但 fail-open 又可能在密钥错误（篡改/损坏）时被误以为「一切正常」。

**建议**：明确区分——注册表/环境变量编辑与保存**继续可用**（degraded 模式，常驻错误条幅）；历史/同步面板报错并提供「导出诊断/重建（需显式确认）」；密钥错误与 DB 损坏都不得静默重建。

---

## 3. 高风险

### H1 —— 服务层如何拿到 `Db` 未定义，直接影响 227 个既有测试

`core::service::save_path_with_sidecar(system, user)`（`core/src/service.rs:326`）没有 DB 参数；调用点：`service.rs:314/315/481`、`gui/src/commands/service.rs:19`、`cli/src/runtime.rs:78`，另有 `core` 内部大量测试。spec §3.2 说「Db 由 GUI/CLI 启动时创建，显式传入服务层」，但没有给出接缝形态：

- 给每个服务函数加 `db: &Db` 参数？（所有调用点与测试都要改）
- 还是定义 `HistorySink` trait，`None` 表示无历史？（测试可传 no-op，改动面小）
- 还是 `service` 内部按全局？**禁止全局单例**（spec 自己写了）。

**建议**：在 spec 里定死一种（推荐 `HistorySink` trait + `NoopHistory`，M1 测试零改动，生产注入 `Db`），否则 plan 无法拆分。

### H2 —— 同步包与本地整数主键的映射规则缺失

跨设备同步时，以下引用都是**本地整数 FK**，不能直接随包传递：

- `snapshots.parent_id INTEGER REFERENCES snapshots(id)`
- `profile_revisions.snapshot_id`
- `events.snapshot_id`

bundle payload 必须用 **uid** 表示引用，导入时按 uid 查本地 id 再写入；spec §6.3 未定义 bundle schema（只写了「JSON（changes/tombstones/manifest）」）。**建议**在 spec 补一节「Bundle Schema」：manifest、每条 change 的字段、引用用 uid、导入的映射与失败回滚规则。

### H3 —— 迁移遗漏 `pending_path_snapshot.json`

`disabled.rs:27-31` 的 pending 文件是「注册表已写、快照失败」的补偿文件。若迁移时它存在，说明 v5 还有一个未提交的 sidecar 状态。§7 没有处理它。

**建议**：迁移事务开始前先调用既有 `retry_pending_path_state`（或把 pending 内容作为 `kind=current` 的最高优先级来源导入），再删/归档 pending 文件。

### H4 —— 首次启动的密钥文件创建存在多进程竞争

GUI 与 CLI 可能同时首次启动：两边都发现无密钥 → 各自生成 32B → 各自 `CryptProtectData` 写 `keys/db.key.dpapi` → 后写覆盖，先创建 DB 的进程用旧密钥、后开库的用新密钥 → 数据不可读。

**建议**：`create_new(true)` 原子创建 + 失败时读取对方已写密钥重试；或对 `keys/` 目录加跨进程锁（现有代码零锁依赖，需新增策略）。同时核对 windows-sys feature 是否够用：设置 DACL 通常需要 `Win32_Security_Authorization`/`Win32_Storage_FileSystem`（spec 只列了 Cryptography/Security/Memory）。

### H5 —— `content_hash` 的算法与依赖未定义

全库去重、迁移幂等、备份索引都依赖 `content_hash`，但 §3.4 新增依赖里**没有任何哈希 crate**。现有 `revision` 是手写 FNV-1a（弱）。若 `content_hash` 也用 FNV，碰撞与安全性都差；若用 BLAKE3/SHA-256，需要新增依赖并锁定版本。

**建议**：明确算法（推荐 BLAKE3 或 `sha2`）并加入依赖表；同时说明「内容哈希只用于去重/判等，不用于安全」。

### H6 —— M4 删除文件后，CLI 的 `env backups` / `env restore` 语义未收口

现有 CLI 命令 `env backups` / `env restore <FILE>`（CLAUDE.md）直接枚举/读取 `env_backup_*.json`。M4 删除这些文件后：

- `env backups` 读什么？（DB 索引？）
- `env restore <FILE>` 是否还接受文件路径？迁移后旧路径是否失效？
- `backup_env_vars` 写什么（见 B6）？

spec 未定义这些命令在 v6 的形态。**建议**：明确「文件为产物、DB 为索引」下 CLI 的三条命令语义，以及 `--from-db` 之类的新入口（或保持不变、只是文件来源变化）。

### H7 —— v6 是否继续写 `disabled.json` 未定义

迁移后 `disabled.json` 被归档为 `.v5.bak`；但 `save_path_with_sidecar` 的 sidecar 提交若仍写该文件，就会形成**双写**；若不写，v5 降级看到的是过期文件（D12 已接受降级不受支持，但需写明「v6 不再更新 v5 文件」）。建议在 §7 明确：**v6 迁移成功后停止写 v5 文件**，`.v5.bak` 只读。

---

## 4. 中风险与事实问题

| #   | 项                                | 说明与建议                                                                                                                                     |
| --- | --------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| M1  | bundle header 缺 salt             | §5.2 KDF 写了「salt 16 B」，但包格式 `magic + version + kdf params + nonce + ciphertext + tag` 没有 salt 字段；补进 header。                   |
| M2  | `VACUUM INTO` 与 SQLCipher        | M0 要实测 `VACUUM INTO` 产物是否仍是加密的；若产物为明文，**禁止**用于备份。SQLCipher 文档推荐的备份路径是 `sqlcipher_export`（§5.1 已写对）。 |
| M3  | `PRAGMA cipher_memory_security`   | 有性能成本，M0 应测冷启动/查询影响，必要时降级为可选。                                                                                         |
| M4  | `changes` / `sync_conflicts` 清理 | 已应用的 changes 与已解决的冲突没有保留策略，长期膨胀；R8 只泛泛提到保留策略，建议给出具体上限。                                               |
| M5  | 同步冲突值的 UI 掩码              | `sync_conflicts.local_value/remote_value` 可能含敏感明文（库内加密），但 UI 展示规则未写；补一条「按 `sensitive` 掩码」。                      |
| M6  | `app_state.version` 的 CAS 对象   | 单行表的 `version` 用来 CAS 什么（current_snapshot_id？lamport？）未写清，建议明确。                                                           |
| M7  | 依赖特性名核对                    | `windows-sys` 的 DPAPI/DACL 所需 feature 名、`uuid` v7 所需 feature、`argon2` 默认特性需在 M0 逐一编译验证。                                   |

---

## 5. 已核实正确的部分（无需改动）

| 项                             | 证据                                                                                                              |
| ------------------------------ | ----------------------------------------------------------------------------------------------------------------- |
| D13 `Path` 永不入 `env_values` | `RESERVED_NAMES = ["Path"]`（`core/src/env_var.rs:13`）；spec 的 `CHECK (lower(name) <> 'path')` + 采集层排除一致 |
| G2 事件无明文                  | D6 + §4.2.7 明确，且 `events` 只有 `changed_fields/before_revision/after_revision`                                |
| G3 CAS                         | D14 + §3.2/§4.2.3                                                                                                 |
| G4 降级时序                    | D12 明确「v5 无法识别 DB；守卫需在 v6 前发」                                                                      |
| G5 revision 措辞               | D15                                                                                                               |
| ErrorCode 扩展点               | 现有 `ErrorCode` 在 `core/src/error.rs:11`，新增 4 个错误码与退出码 4/5 不冲突                                    |
| 撤销操作类型                   | 现有 `OperationType` 为 **9 种**（DELETE..IMPORT_BOTH），spec「9–10 种」准确                                      |
| core lib target                | `core/src/lib.rs` 存在，`cargo test -p path-editor-core --lib` 可用（§1.3）                                       |
| rusqlite feature 存在性        | rusqlite 0.40.1 `Cargo.toml` 确认 `bundled-sqlcipher-vendored-openssl` 及其依赖链                                 |
| SQLCipher PRAGMA 顺序          | §3.3 中 key 在 journal/foreign_keys 之前，正确；`cipher_version` 验收正确                                         |
| 备份文件保留思路               | D7 与调研 B3 一致；用户可手工打开                                                                                 |

---

## 6. 待裁断（请审核窗口/用户逐条作答）

1. **B1**：NASM 由用户安装还是改用系统 OpenSSL/vcpkg？M0 是否把 CI 构建也列为探针？
2. **B2**：`user_version` 改为「启动只读、迁移时写」，确认。
3. **B3/B4/B5**：schema 四处修订（source_path、lamport、partial unique index）是否采纳？
4. **B6**：v6 新备份的格式与密钥（明文 / DPAPI 加密文件 / 加密包）？这是 v6 数据模型之外最影响用户体验的决定。
5. **B7**：DB 不可用时是否 fail-open（注册表编辑照常，历史降级）？
6. **H1**：`HistorySink` trait + `NoopHistory` 的接缝是否采纳？
7. **H2**：是否在 spec 增设「Bundle Schema」一节？
8. **H3/H6/H7**：pending 文件、CLI 备份命令、是否停写 v5 文件，是否写入 spec？

---

## 7. 结论

**方向、决策与核对轮 G1–G5 的落实全部认可。** 但当前 spec 处于「可讨论」而非「可写 plan」的状态：

- **M0 尚未开始**，且本机已确认缺 NASM——这不是纸面风险，是下一步会立刻撞上的墙；
- **schema 有 4 处自洽性缺口**（B2–B5），任何一处带进 plan 都会在 M1 变成返工；
- **B6 的备份明文矛盾**是产品级决定，影响 CLI、恢复、用户可读性与安全模型，不能留给 plan 猜；
- **H1/H2** 是接缝与协议设计，必须在 plan 之前定死，否则 M1/M3 无法拆分任务。

**本文件不改动 spec、不写 plan、不改代码、未提交任何文件。**

**签章**：以上结论基于 2026-10-19 在 `main @ aae0909` 上实际执行的命令输出与对 rusqlite 上游 `Cargo.toml` 的在线核实。
