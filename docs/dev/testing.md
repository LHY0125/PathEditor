# 测试与质量门

> 本文件是 `CLAUDE.md` 的详细补充。日常开工只需读 `CLAUDE.md` 的质量门速览；
> 需要测试布局、CI 步骤或基线数字时再读本文。

## 测试布局

- **前端**：Vitest + jsdom，测试放在 `src/gui/web/tests/unit/*.test.ts` 或 `*.test.tsx`；行为变化必须有回归测试。
  - 单个文件：`npx vitest run src/gui/web/tests/unit/<file>.test.ts`
  - 按名字过滤：`npx vitest run -t "<name>"`
- **E2E**：Playwright，测试放在 `src/gui/web/e2e/tests/*.spec.ts`，运行 `npm run test:e2e`。它验证前端流程（mock IPC），不等同于真实 Tauri 集成验证。
- **Rust**：测试放在模块内或 workspace test target，运行 `cargo test --workspace`。
  - 单个测试：`cargo test -p <crate> <test_name>`
  - core 集成测试：`cargo test -p path-editor-core --test <file>`（包名是连字符，不是下划线）
  - CLI 单测：`cargo test -p patheditor-cli --bins <test_name>`（bin-only crate，`--lib` 会报 `no library targets found`）

## 一文件一测试（v6 SQLCipher）

任何使用 SQLCipher 的 `.rs` 测试文件**只放一个 `#[test]`**。同一进程内跑第二个 SQLCipher 测试会
`STATUS_STACK_OVERFLOW`（GNU 与 MSVC 均已复现，`--test-threads=1` 无效）。

CI 分两步跑（core/cli 用 `--exclude patheditor`，GUI 用 `--lib` + 清单开关），覆盖整个 workspace——
规则靠文件布局保证，不靠 CI 逐文件调用。依据：`docs/审核和开发/2026.10.19/PathEditor-v6-M0-探针结果.md` §5.3。

## 质量门

```powershell
npm run verify        # Prettier → ESLint → build → coverage → cargo fmt → clippy → cargo test
npm run verify:all    # verify + Playwright E2E
```

- 覆盖率门槛为 **80% 行覆盖**。
- 提交前优先运行 `npm run verify:all`。
- **当前基线（v6.1.1）**：Rust **324 passed / 0 failed / 2 ignored**；前端 **37 files / 413 passed**；E2E **54 passed**。
  - 基线只在本节维护一份，其他文档（`AGENTS.md` / `README.md`）引用这里的数字，不要各写一套。
  - 三项均为 2026-10-24 本机实测（GNU 工具链）：`cargo test --workspace` → 324 passed / 2 ignored；
    `npx vitest run` → 37 files / 413 passed；`npm run test:e2e` → 54 passed。

## CI

### `.github/workflows/ci.yml`（质量门）

在 `push main` / `pull_request` / `workflow_dispatch` 时触发，`windows-latest` + **MSVC**：

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace --exclude patheditor`（core / cli）
4. `PATHEDITOR_MSVC_TEST_MANIFEST=1 cargo test -p patheditor --lib`（GUI lib）
5. `npx tsc --noEmit`
6. `npx vitest run`

要点：

- 用 `rustup override set stable-x86_64-pc-windows-msvc` 覆盖 `rust-toolchain.toml` 的 GNU 设置。
- OpenSSL 的 MSVC 构建需要 NASM；runner 缺失时 `choco install nasm`。
- CLI 的 `env restore` 夹具要求本机至少有 4 个用户环境变量；CI 用
  `[Environment]::SetEnvironmentVariable(..., "User")` 预置 4 个。
- **CI 不跑 E2E**——E2E 证据来自本地 `npm run verify:all`。

### `.github/workflows/release.yml`（发布）

只在 `v*` tag 触发；详见 `docs/dev/release.md`。

## 测试不得触碰用户真实目录

`dirs::home_dir()` 走 `SHGetKnownFolderPath(FOLDERID_Profile)` 读注册表，**`HOME` 与 `USERPROFILE`
两个环境变量都不读**——`config_dir()` 无法被任何环境变量重定向。

因此凡是走它的测试（密钥、备份、数据库），必须**显式传路径**
（`load_or_create_key(&tempdir)`、`write_env_backup_to_with_key(dir, payload, &material)`），
不能在测试里调用会内部走 `config_dir()` 的入口（`backup_env_vars()`、`open_default()`）。
只有 `PATHEDITOR_BACKUP_DIR` 这类项目自定义变量是可重定向的。

## 其他

- **Windows + GNU 目标本地构建需要三件事**，缺任一条都会在 `openssl-sys` 处失败：
  1. 能产出 POSIX 路径的 perl。原生 Windows perl（`osname=MSWin32`，如 Strawberry/Scoop）会被
     OpenSSL 的 `Configurations/unix-checker.pm`（`rel2abs('.') !~ m|/|`）拦下并中止。
     Git for Windows 自带的 perl 是 cygwin 版、能过检查：
     `$env:OPENSSL_SRC_PERL = "$(scoop prefix git)\usr\bin\perl.exe"`
  2. 补齐该 perl 被裁剪掉的模块（缺 `Locale::Maketext::Simple` 等，`IPC::Cmd` 因此加载失败）。
     仓库已把需要的模块树放在 `probes/v6-sqlcipher/perl5lib/`：
     `$env:PERL5LIB = "$PWD\probes\v6-sqlcipher\perl5lib"`
  3. 给 `make` 的 recipe 提供 `sh` / `cp` / `rm`：
     `$env:PATH += ";$(scoop prefix git)\usr\bin"`
     三行都是会话级环境变量，**不要写进 `.cargo/config.toml`**——CI 走 MSVC 也会读它，
     指向本机路径会直接把 CI 弄红。兜底方案：装 MSYS2 并 `pacman -S perl`，
     把 `OPENSSL_SRC_PERL` 指向其 `usr\bin\perl.exe`。
     验证记录：2026-10-24 本机 `cargo check -p path-editor-core` 通过（23m52s）。
- CI 与发布一律走 MSVC（原生 perl + NASM），不受上述限制。
- 前端全量跑曾出现 `use-path-validation.test.tsx` 的隔离抖动（期望 3 实得 11）；
  该文件单独跑 3/3 通过，属用例间 pending 未清理，与本轮改动无关，重跑即可判定。
- 代码风格：UTF-8、CRLF；TS 2 空格，Rust/TOML 4 空格；Prettier 使用单引号、尾逗号和 100 列。
- 从仓库根目录跑 `npm test` 时，`vitest.config.ts` 的 `exclude` 必须包含 `.claude/**`——否则会扫到
  `.claude/worktrees/` 下嵌套 worktree 的 e2e 文件并大面积假失败。
