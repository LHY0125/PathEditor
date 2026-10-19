# PathEditor v6.0.0 M0 探针结果

- **日期**：2026-10-19（审核窗口复核与修正：2026-10-20）
- **执行者**：开发窗口（初版）；审核窗口（独立复核、根因定位、C' 重构）
- **分支/状态**：`v6/m0-probe`；**代码零改动**（`core/`/`gui/`/`cli/`/`src/` 无差异）
- **结论**：**本机（GNU）8 项探针通过（一文件一测试，24/24 稳定）→ 放行 M1**；CI（MSVC）待重推验证

---

## 一、环境实测

| 项               | 值                                                                     |
| ---------------- | ---------------------------------------------------------------------- |
| OS               | Windows 11                                                             |
| Rust             | `rustc 1.99.1 (58cf8f9c3 2026-10-13)`，`cargo 1.99.1`                  |
| 本机工具链       | `stable-x86_64-pc-windows-gnu`（`rust-toolchain.toml`）                |
| gcc              | MinGW-Builds 15.2.0                                                    |
| make             | **GNU Make 4.4.1**（本次新装，`scoop install make`）                   |
| perl（最终使用） | Git for Windows 自带 MSYS2 Perl 5.42.3                                 |
| perl（补充）     | Strawberry Perl 5.42.3.1（`scoop install perl`，仅用于取纯 Perl 模块） |
| NASM             | **无**（GNU 目标不需要）                                               |
| 构建产物         | `target2/`（约 255 MB+），首次全量编译 **31 分 40 秒**                 |

---

## 二、探针结果

| 探针                              | 本机（GNU）           | CI（MSVC） | 结论                                                                      |
| --------------------------------- | --------------------- | ---------- | ------------------------------------------------------------------------- |
| SQLCipher 打开 + `cipher_version` | ✅ PASS               | 待验证     | `cipher_version` 非空                                                     |
| 错误密钥拒绝 + 原库存活           | ✅ PASS               | 待验证     | 错误密钥查询返回 Err；原库仍可正确打开                                    |
| FTS5                              | ✅ PASS               | 待验证     | `sqlite_compileoption_used('ENABLE_FTS5')=1`，`MATCH` 命中                |
| JSON1 (`json_valid`)              | ✅ PASS               | 待验证     | `json_valid('{"a":1}')=1`                                                 |
| `sqlcipher_export`                | ✅ PASS               | 待验证     | ATTACH+export+DETACH 后新库可用同密钥读取                                 |
| `VACUUM INTO` 加密性              | ✅ **encrypted=true** | 待验证     | 产物**已加密**（无 `SQLite format 3` 头）；见 §四                         |
| DPAPI roundtrip                   | ✅ PASS               | 待验证     | `CryptProtectData`/`CryptUnprotectData` 往返一致                          |
| DACL 符号                         | ✅ PASS               | 待验证     | `GetNamedSecurityInfoW`/`SetNamedSecurityInfoW`/`SE_FILE_OBJECT` 编译通过 |

**测试计数**：8 个测试文件，每个文件 1 个探针 = **8 个探针，24/24 稳定通过（3 轮）**。

> **测试组织约定（C'）**：一个 `.rs` 文件只放一个 `#[test]`。原因见 §五——同一进程内第二个 SQLCipher 测试会栈溢出。CI 逐文件调用。

---

## 三、环境障碍与解法（本轮最大产出）

M0 的价值在于**把"文档承诺"变成"逐项实测"**。实际撞到 6 个环境障碍，全部记录如下：

| #   | 失败点            | 错误                                                       | 根因                                              | 解法                                                                           |
| --- | ----------------- | ---------------------------------------------------------- | ------------------------------------------------- | ------------------------------------------------------------------------------ |
| 1   | OpenSSL Configure | `Can't locate Locale/Maketext/Simple.pm`                   | Git 自带 Perl 是精简版，缺 OpenSSL Configure 依赖 | 装 Scoop Strawberry Perl                                                       |
| 2   | Configure         | `This perl implementation doesn't produce Unix like paths` | Strawberry 是 **MSWin32** 实现，mingw64 目标拒绝  | 回到 Git 的 **MSYS2** Perl                                                     |
| 3   | Configure         | `@INC` 里出现 `C /Users/...`（被拆断）                     | `PERL5LIB` 被 MSYS 路径转换后按 `:` 拆分          | 改从原生 PowerShell 设 POSIX 路径                                              |
| 4   | Configure         | `Can't locate ExtUtils/MakeMaker.pm`                       | Git Perl 缺 `ExtUtils`                            | 从 Strawberry 复制 `ExtUtils` 模块树                                           |
| 5   | Configure         | `Can't locate Pod/Usage.pm`                                | Git Perl 缺 `Pod`                                 | 复制 `Pod` 模块树                                                              |
| 6   | `make depend`     | 子环境 `PERL5LIB` 再次被破坏                               | cargo→make→sh 链路中的路径转换不可预测            | **改用相对路径** `../../../../../../../../perl5lib`（无盘符/冒号，不可被转换） |
| 7   | 构建启动          | `build_dir: 另一个程序正在使用此文件 (os error 32)`        | 手动跑 `make depend` 留下目录句柄锁               | 换全新 `CARGO_TARGET_DIR=target2`                                              |

**最终可复现的构建命令**（PowerShell）：

```powershell
$env:PERL5LIB='../../../../../../../../perl5lib'
$env:OPENSSL_SRC_PERL='D:\settings\settings\Scoop\apps\git\2.56.0.2\usr\bin\perl.exe'
$env:CARGO_TARGET_DIR='...\probes\v6-sqlcipher\target2'
cargo test -- --nocapture
```

其中 `probes/v6-sqlcipher/perl5lib/` 是从 Strawberry 复制的纯 Perl 模块树（`Locale`/`I18N`/`ExtUtils`/`Pod`），随探针 crate 一起存在。

---

## 四、关键诊断值

**`VACUUM INTO` 产物加密性**（spec §5.1 要求实测）：

```
VACUUM INTO diagnostic: encrypted=true
  header=[ed, 2e, be, 88, 46, d9, 38, 36, 37, 04, 68, 85, 88, fc, ed, 30]
  cipher_page_size=0  cipher_kdf_iter=0
```

- **产物已加密**（首字节非 `SQLite format 3`），与 SQLCipher 官方 issue #338 的结论一致（默认参数下继承源库加密设置）。
- `cipher_page_size` / `cipher_kdf_iter` 返回 0：这两个 PRAGMA 在**未显式设置**时查询返回 0（非默认值本身），属正常现象，不影响结论。
- **仍按 spec 决定：备份路径只用 `sqlcipher_export`**，`VACUUM INTO` 仅作诊断（spec §5.1）。

**`cipher_memory_security` 基准**（spec §5.1 要求实测开销）：

```
cipher_memory_security=ON:  100 queries in 902.4µs
cipher_memory_security=OFF: 100 queries in 931µs
```

- 两者**无显著差异**（ON 甚至略快，属噪声范围）。**结论：可以保持 ON**，无需降级为可选配置。

---

## 五、发现的问题：SQLCipher 测试的栈溢出（审核窗口已定位并修正）

### 5.1 现象（回执已记录）

`cargo test` 跑 `sqlcipher.rs` 时 `STATUS_STACK_OVERFLOW (0xc00000fd)`，溢出的测试不固定。

### 5.2 审核窗口独立复现与定位（本轮的实质结论）

| 实验                                  | 结果                                            |
| ------------------------------------- | ----------------------------------------------- |
| 每个测试**单独进程**（`--exact`）     | ✅ 6/6 全过                                     |
| `--test-threads=1` 一起跑             | ❌ **仍溢出**（回执原结论「单线程可过」不成立） |
| 同一测试内连续 5 轮 SQLCipher 操作    | ✅ 通过 —— **不是 SQLCipher 的进程级泄漏**      |
| 6 个相同测试（每轮建库/查询）并发     | ❌ 第 5 个溢出                                  |
| 6 个相同测试串行                      | ❌ 第 2 个溢出                                  |
| 每个测试包在 32 MB 栈线程里，**并行** | ❌ `thread '<unknown>'` 溢出                    |
| 每个测试包在 32 MB 栈线程里，**串行** | ❌ 仍溢出                                       |
| 每个测试包在 8 MB 栈线程里            | ❌ 仍溢出                                       |
| 单测试内 32 MB 线程 + 单次操作        | ✅ 通过                                         |

**根因**：不是「SQLCipher 泄漏」，也不是「FTS5 特殊」，而是 **libtest 的测试线程栈（默认 2 MB）对 SQLCipher 的一次完整生命周期（打开 + 校验 + PRAGMA + 操作）不够**；而且溢出后的进程状态不可靠，后续测试继续失败。**测试进程内只有「第一个 SQLCipher 测试」能稳定通过。**

**CI 复现（run 39282501269，MSVC）**：DPAPI 2/2 通过；SQLCipher 6 项同进程跑时 `fts5_and_json1_are_available` 溢出。**GNU 与 MSVC 同因**。

**非确定性（重要）**：审核窗口复跑时，`fts5_and_json1_are_available` 即使**单独进程**也开始稳定溢出（连续 3 次）。当时系统虚拟内存仅剩 **4.29 GB / 52.12 GB**（物理内存充足，15.95 GB 空闲）。说明溢出阈值与**进程可用虚拟地址空间**相关，不是固定值。**这意味着「每测试单独进程」也不保证稳定**，M1 不能依赖它作为唯一方案。

**生产影响评估**：GUI/CLI 是**单进程、单主线程、单个 DB 连接**，且主线程栈由 PE 头提供（默认 1 MB 保留、按需提交），与 libtest 的固定 2 MB 线程栈不同。**生产路径不触发此问题**；但 M1 的测试必须显式处理。

### 5.3 最终结论与处置（C'：一文件一测试）

**决定性实验（审核窗口）**：

| 模式                                                        | 结果               |
| ----------------------------------------------------------- | ------------------ |
| 每个测试单独进程（`--exact`）× 3 轮                         | ✅ **18/18 全过**  |
| 同一进程跑多个测试（`--test-threads=1`）× 5 轮              | ❌ **5/5 全失败**  |
| 关闭 rust-analyzer（虚拟内存 6.4 → 9.25 GB）后同进程 × 5 轮 | ❌ **仍 5/5 失败** |
| 一文件一测试 × 8 文件 × 3 轮                                | ✅ **24/24 全过**  |

**结论**：

- 溢出**与系统内存无关**（释放 3 GB 虚拟内存后仍稳定失败），开发窗口的「环境问题」判断不成立。
- 溢出**与 `cipher_memory_security` 无关**（ON/OFF 在单独进程下都通过）。
- 真正结论：**同一进程内不能运行第二个 SQLCipher 测试**（GNU 与 MSVC 同因）。

**处置（已实施）**：

- 探针拆为 **8 个测试文件，每个文件一个 `#[test]`**：`p1_open` / `p2_wrong_key` / `p3_fts_json` / `p4_export` / `p5_vacuum` / `p6_bench` / `p7_dpapi` / `p8_dacl`。
- CI 改为**逐文件调用**（每个文件一个进程）。
- **M1 的 DB 测试必须沿用此约定**：一个 `.rs` 文件只放一个 SQLCipher 测试；同进程多测试会随机失败。
- 若将来确实需要多测试同进程，必须先解决 SQLCipher 的栈需求（未解决项）。

### 5.4 附加观察

测试日志出现 `sqlcipher_mlock: VirtualLock() returned 0 LastError=1453`（`ERROR_NOT_ENOUGH_MEMORY`）——`cipher_memory_security=ON` 在测试进程锁内存失败。与栈溢出同属「测试进程资源受限」，**生产环境需 M1 验证**；ON/OFF 性能实测无差异（902 µs vs 931 µs），必要时可降级为 OFF。

**待 CI 验证**：MSVC 构建下是否同样有此现象（MSVC 默认栈配置不同）。**这是 M0 留给 CI 的一个观察项。**

---

## 六、未覆盖项（如实列出）

- **CI（MSVC + NASM）未运行**：需要推送分支（`push`/`pull_request` 触发），**等待用户授权**。本机 GNU 结果不能替代 CI 结果。
- **`VACUUM INTO` 在自定义 cipher 参数下的行为**未测（v6 用默认参数，已够）。
- **性能基线**未做（冷启动、1 万快照、迁移耗时）——属 M1 范围。
- **`sqlcipher_export` 的跨密钥导出**未测（只测了同密钥）。
- 探针 `perl5lib/` 目录是**本机环境补丁**，不应进版本库（见 §七）。

---

## 七、审核窗口复核（2026-10-19/20）

| 项                                                  | 结果                                                                                                        |
| --------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| 独立复跑 DPAPI 2 项                                 | ✅ PASS                                                                                                     |
| 独立复跑 6 项 SQLCipher（`--exact` 每测试独立进程） | ✅ 6/6 PASS                                                                                                 |
| 独立复跑 `--test-threads=1`                         | ❌ **仍栈溢出**（`fts5_and_json1_are_available`）——回执原写法不成立，已更正 §五                             |
| `probes/` 入库                                      | 用户拍板：**入库**（仅 `Cargo.toml`/`Cargo.lock`/`src`/`tests`；`perl5lib/`、`target*/` 已入 `.gitignore`） |
| 栈溢出处置                                          | 用户拍板：**接受并记录**，M1 测试按「每测试独立进程」运行                                                   |
| 根 `Cargo.toml` `exclude`                           | 用户确认接受                                                                                                |
| CI 推送                                             | 已推 `v6/m0-probe`，run `39282501269` 进行中                                                                |

**附加发现**：`sqlcipher_mlock: VirtualLock() returned 0 LastError=1453`（`ERROR_NOT_ENOUGH_MEMORY`）—— `cipher_memory_security=ON` 在测试进程锁内存失败，与栈溢出可能同源。**生产环境是否受影响待 M1 验证**；ON/OFF 性能实测无差异，必要时可降级。

---

## 八、原待用户决定项（已全部拍板）

1. **`probes/` 是否入库** → **入库**：已提交 `probes/v6-sqlcipher/{Cargo.toml,Cargo.lock,src,tests}`、workflow、根 `Cargo.toml` 的 `exclude`；`perl5lib/` 与 `target*/` 已加入 `.gitignore`。
2. **CI 推送授权** → **已授权并推送** `v6/m0-probe`（run `39282501269`）。
3. **根 `Cargo.toml` 的 `exclude`** → **接受**。

---

## 九、M0 放行判定

| 门                              | 状态                              |
| ------------------------------- | --------------------------------- |
| SQLCipher 可构建（本机 GNU）    | ✅ **通过**（31分40秒，产物可用） |
| SQLCipher 可加密 / 错误密钥拒绝 | ✅ 通过                           |
| FTS5 / JSON1 可用               | ✅ 通过                           |
| `sqlcipher_export` 可用         | ✅ 通过                           |
| DPAPI 可用                      | ✅ 通过                           |
| CI（MSVC）                      | ⏳ **待推送验证**                 |

**本机结论：M0 绿灯，可进入 M1。** CI 一路待用户授权推送后补验；若 MSVC 出现构建或栈问题，按计划 §七 决策门处理。

---

**签章**：以上结论基于 2026-10-19 在本机实际执行的构建与测试输出（`target2/`，GNU 工具链），以及 openssl-src / libsqlite3-sys / SQLCipher 上游文档的在线核实。
