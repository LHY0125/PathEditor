# 测试与质量门

> 本文件是 `CLAUDE.md` 的详细补充。日常开工只需读 `CLAUDE.md` 的质量门速览；
> 需要测试布局、CI 步骤或基线数字时再读本文。

## 测试布局

- **前端**：Vitest + jsdom，测试放在 `src/gui/web/tests/unit/*.test.ts` 或 `*.test.tsx`；行为变化必须有回归测试。
  - 单个文件：`npx vitest run src/gui/web/tests/unit/<file>.test.ts`
  - 按名字过滤：`npx vitest run -t "<name>"`
- **E2E**：Playwright，测试放在 `src/gui/web/e2e/tests/*.spec.ts`，运行 `npm run test:e2e`。它验证前端流程（mock IPC），不等同于真实 Tauri 集成验证。
  `playwright.config.ts` 的 `webServer.cwd` **必须显式指向仓库根**（本文件不在仓库根，默认 cwd 会找不到根 `package.json`）。
- **Rust**：测试放在模块内或 workspace test target，运行 `cargo test --workspace`。
  - core 集成测试按领域分组：`src/core/tests/{history,search,schema,backup,sync,contracts}.rs` 是 6 个聚合 root，用例文件在 `tests/suites/<group>/`，由 root 用 `#[path]` 挂载。**新增用例放进对应组**，不要新增顶层 target。
  - 单个测试：`cargo test -p <crate> <test_name>`
  - 单组集成测试：`cargo test -p path-editor-core --test history`（包名是连字符，不是下划线）
  - 共用建库夹具：`tests/common/mod.rs` 的 `new_db(name) -> (TempDir, Db)`
  - CLI 单测：`cargo test -p patheditor-cli --bins <test_name>`（bin-only crate，`--lib` 会报 `no library targets found`）

## 一文件一测试（已取消）

数据库是普通 SQLite，「一文件一测试」约束随 SQLCipher 退场而**取消**：栈溢出的根因是 SQLCipher
的全局状态（同进程第二个测试 `STATUS_STACK_OVERFLOW`），明文 SQLite 没有这个问题。

存量单测试文件保留原样（合并只是 churn）；新测试可以在同一文件放多个 `#[test]`。

CI 分两步跑（core/cli 用 `--exclude patheditor`，GUI 用 `--lib` + 清单开关），覆盖整个 workspace。

## 质量门

```powershell
npm run verify        # Prettier → ESLint → build → coverage → cargo fmt → clippy → cargo test
npm run verify:all    # verify + Playwright E2E
```

- 覆盖率门槛为 **80% 行覆盖**，统计范围是 `src/gui/web/src/{core,store,hooks}` 与 `services/backend/`（`components/` 不在门槛内）。`services/backend/` 是 IPC 形状校验的安全边界，纳入门槛以免新增命令漏测。
- 提交前优先运行 `npm run verify:all`。
- **当前基线（v7.0.0）**：Rust **356 passed / 0 failed / 2 ignored**；前端 **41 files / 448 passed**（行覆盖率 **80.36%**）；E2E **59 passed**。
  - 基线只在本节维护一份，其他文档（`AGENTS.md` / `README.md`）引用这里的数字，不要各写一套。
  - Rust 为 2026-11-08 本机实测（GNU 工具链，去掉 SQLCipher 后同一套用例）：
    `cargo test --workspace` → 356 passed / 2 ignored；前端为同日实测：
    `npx vitest run` → 41 files / 448 passed，行覆盖率 80.36%；E2E 为同日实测：
    `npm run test:e2e` → 59 passed（Playwright 1.60）。

## CI

### `.github/workflows/ci.yml`（质量门）

在 `push main` / `pull_request` / `workflow_dispatch` 时触发，`windows-latest` + **MSVC**：

1. `node scripts/check-consistency.mjs`（版本号 / schema 版本 / 测试基线 / 指令文件一致性）
2. `cargo fmt --all -- --check`
3. `cargo clippy --workspace --all-targets -- -D warnings`
4. `cargo test --workspace --exclude patheditor`（core / cli）
5. `PATHEDITOR_MSVC_TEST_MANIFEST=1 cargo test -p patheditor --lib`（GUI lib）
6. `npx tsc -b`
7. `npm run lint` / `npm run format:check`
8. `npm run test:coverage`（覆盖率门槛只有带 `--coverage` 才生效）

另有独立的 `audit` job（`ubuntu-latest`）：`rustsec/audit-check` 跑 `cargo audit`，
放行项与理由集中在 `.cargo/audit.toml`——只允许「经核实不可达」的告警（未启用特性的可选依赖、
仅 Linux GTK 目标的依赖），能编译进来的漏洞必须升级而非加白名单。

要点：

- 用 `rustup override set stable-x86_64-pc-windows-msvc` 覆盖 `rust-toolchain.toml` 的 GNU 设置。
- 不再需要 NASM / OpenSSL（`rusqlite` 只用 `bundled`）。
- CLI 的 `env restore` 夹具**不需要**预置宿主环境变量：差异夹具改用系统 hive（任何 Windows 都有
  `SystemRoot` 等十几个变量），进程级用例通过 `PATHEDITOR_KEY_DIR` / `PATHEDITOR_DATA_DIR`
  把密钥与数据库指向临时目录。
- **CI 不跑 E2E**——E2E 证据来自本地 `npm run verify:all`。

### `.github/workflows/release.yml`（发布）

只在 `v*` tag 触发；详见 `docs/dev/release.md`。

## 测试不得触碰用户真实目录

`dirs::home_dir()` 走 `SHGetKnownFolderPath(FOLDERID_Profile)` 读注册表，**`HOME` 与 `USERPROFILE`
两个环境变量都不读**——`config_dir()` 无法被任何环境变量重定向。

因此凡是走它的测试（密钥、备份、数据库），必须**显式传路径**
（`load_or_create_key(&tempdir)`、`write_env_backup_to_with_key(dir, payload, &material)`），
不能在测试里调用会内部走 `config_dir()` 的入口（`backup_env_vars()`、`open_default()`）。

**例外：子进程用例。** CLI 的进程级用例跑的是 `patheditor.exe` 子进程，拿不到参数，
只能靠环境变量。密钥解析走 `crypto::key_home()`（`PATHEDITOR_KEY_DIR` 优先），数据库路径走
`crypto::data_home()`（`PATHEDITOR_DATA_DIR` 优先），两者都回落 `config_dir()`（见 `crypto.rs` 的注释）。
测试里由 `cli_cmd()` 统一下发这两个变量，密钥与库都落在临时目录，**不读写**真实的
`~/.patheditor/keys/db.key.dpapi` 与 `~/.patheditor/patheditor.db`。

**不要在同进程测试里 `set_var` 打开它**：那是进程级全局状态，会污染同进程其它测试。
`PATHEDITOR_BACKUP_DIR` 是同一类可重定向变量（备份目录）。

## 其他

- **本地构建不再需要 OpenSSL 工具链**：`rusqlite` 只用 `bundled`（SQLite C 源码，由 `cc` 编译，
  FTS5/JSON1 默认开启），`OPENSSL_SRC_PERL` / `PERL5LIB` / NASM 全部不再需要。
  `probes/v6-sqlcipher/` 仅作历史探针证据保留，不参与构建。
- 前端全量跑曾出现 `use-path-validation.test.tsx` 的隔离抖动（期望 3 实得 11）。根因不是随机抖动：
  5 个测试文件各自挂了 `afterEach`、其余文件没挂，RTL 的 DOM 会跨用例残留。现在 `tests/setup.ts`
  统一注册 `afterEach(cleanup)`，单独跑与全量跑结果一致 —— 再遇到「单独跑绿、全量红」，按缺 cleanup 排查。

### 被 `#[ignore]` 的用例

两个用例因环境前提被显式忽略，`cargo test` 全量跑时列为 ignored 且**不会执行**；
改动注册表编解码后请手动跑一次：

| 用例                                         | 位置                            | 忽略原因                                                      |
| -------------------------------------------- | ------------------------------- | ------------------------------------------------------------- |
| `list_fails_when_decode_fails`               | `platform/registry/env_edit.rs` | winreg 0.52 对字符串类型按 lossy 解码，解码失败分支当前不可达 |
| `save_paths_keeps_expand_sz_in_isolated_key` | `platform/registry/path.rs`     | 需要真实注册表写权限（只用隔离测试键，不接触 PATH）           |

运行：`cargo test -- --ignored <名字片段>`。

- 代码风格：UTF-8、CRLF；TS 2 空格，Rust/TOML 4 空格；Prettier 使用单引号、尾逗号和 100 列。
- 从仓库根目录跑 `npm test` 时，`vitest.config.ts` 的 `exclude` 必须包含 `.claude/**`——否则会扫到
  `.claude/worktrees/` 下嵌套 worktree 的 e2e 文件并大面积假失败。
