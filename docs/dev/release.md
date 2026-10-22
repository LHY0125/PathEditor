# 版本号与发布

> 本文件是 `CLAUDE.md` 的详细补充。日常开工只需读 `CLAUDE.md` 的发布速览；
> 准备发版时再读本文。

## 版本号升级清单

当前版本以 `package.json` 的 `version` 为准。升级时至少检查：

| 文件                  | 字段                                                            |
| --------------------- | --------------------------------------------------------------- |
| `package.json`        | `version`                                                       |
| `package-lock.json`   | `version`（`npm install --package-lock-only` 同步）             |
| `Cargo.toml`          | `[workspace.package] version`                                   |
| `Cargo.lock`          | 三个 workspace crate 的 `version`（`cargo update -p ...` 同步） |
| `gui/tauri.conf.json` | `version`、窗口 `title`                                         |
| `README.md`           | 版本徽章和安装包说明                                            |

其他位置通常从 `package.json` 或 `env!("CARGO_PKG_VERSION")` 动态读取，不要手工制造第二套版本源。

## 发布流程

**发布由 CI 自动完成，不要在本地手工构建或创建 Release。** `.github/workflows/release.yml`
在推送 `v*` tag 时触发，自动执行：校验版本号一致性 → `npx tauri build` →
`cargo build --release -p patheditor-cli` → 整理产物 → 生成发布日志 → 创建 GitHub Release。

### 唯一正确的发布步骤

```powershell
# 1. 确认版本号四处一致（package.json / Cargo.toml / gui/tauri.conf.json / README）
# 2. 在 CHANGELOG.md 顶部写入当前版本的段落（见下）
# 3. 提交并推送 main，确认该提交的 CI（.github/workflows/ci.yml）已绿
git push origin main
gh run list --branch main --limit 3

# 4. 打 annotated tag 并推送 —— 这一步触发 release.yml
git tag -a v<VERSION> -m "PathEditor v<VERSION>" -m "- 变更要点..."
git push origin v<VERSION>

# 5. 等 CI 跑完，用 gh 确认结果
gh run list --limit 3
gh release view v<VERSION>
```

**推送 tag 后不要做任何事，等 CI 完成。** 本地构建、手动创建 Release 都是重复劳动，且会与 CI 冲突。

### 禁止事项（2026-09-18 事故教训）

| 禁止                                                              | 原因                                                                                                                                         |
| ----------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| **推送 tag 后用 `gh release create` 手动建 Release**              | 会与 CI 抢同一个 Release。CI 走到最后一步会以 `a release with the same tag name already exists` 失败，留下红色的 CI 历史，且无法通过重跑消除 |
| **推送 tag 前手动跑 `npx tauri build` / `cargo build --release`** | CI 会做同样的事。本地构建只用于**验证能否构建通过**，产物不要用于发布                                                                        |
| **在没读 `.github/workflows/` 的情况下规划发布**                  | 仓库有 tag 触发的自动发布。不了解它就会重复构建、或与它冲突                                                                                  |

**若确实需要手动发布**（CI 不可用时）：先确认该 tag 的 Release 不存在，再执行 `gh release create`，
然后**不要**推送 tag（或推 tag 后接受 CI 会跳过）。两者只能选其一。

### CHANGELOG.md 是发布日志的来源

`release.yml` 的「生成发布日志」步骤**优先**用正则从 `CHANGELOG.md` 抽取当前版本段落：

```powershell
$pattern = "(?ms)^##\s+v?$([regex]::Escape($version))(?:\s|\(|$).*?(?=^##\s+|\z)"
```

匹配 `## <VERSION>` 或 `## <VERSION> (YYYY-MM-DD)` 开头的段落。**若找不到，会回退到
`git log <上一 tag>..<本 tag>` 逐条列 commit 标题**——日志质量明显下降。

所以发版前**必须在 CHANGELOG.md 顶部写好当前版本段落**，小节标题沿用最近几版的中文风格
（`### 新增` / `### 变更` / `### 修复` / `### 说明`）。

### CI 各步骤的前置依赖

| 步骤         | 依赖                                                                                                                                                                                                  |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 校验项目版本 | `package.json`、`gui/tauri.conf.json`、`Cargo.toml` 三处版本必须与 tag 一致，否则整条流水线失败                                                                                                       |
| Tauri Build  | 需要 `npm ci` 与 Node 20                                                                                                                                                                              |
| 整理发布产物 | 需要 `target\release\bundle\nsis\PathEditor_<VERSION>_x64-setup.exe`、`target\release\PathEditor.exe`（GUI 本体，portable zip 用）与 `target\cli\release\patheditor.exe`（CLI，独立 target 目录构建） |
| 生成发布日志 | 读 `CHANGELOG.md`（缺失则回退 git log）                                                                                                                                                               |

工作流使用 MSVC 工具链（覆盖 `rust-toolchain.toml` 的 GNU 设置），因为 GitHub Windows runner 上
MSVC 更稳定。这是刻意的，不要“修正”它。

### 产物命名

| 文件                                 | 说明                                                                                                            |
| ------------------------------------ | --------------------------------------------------------------------------------------------------------------- |
| `PathEditor_<VERSION>_x64-setup.exe` | NSIS 安装包（GUI），产物目录中原始名                                                                            |
| `PathEditor.exe`                     | GUI 本体（tauri bin 产物，portable zip 装的就是它）                                                             |
| `patheditor-cli_<VERSION>_x64.exe`   | CLI 二进制，CI 在整理产物时重命名——**本地产物为 `target\cli\release\patheditor.exe`，不要据此判断发布包的名称** |

### Release 已存在时的行为

工作流有两道检查：开头的「检查 Release 是否已存在」（`exists` 输出）与末尾「创建 GitHub Release」
步骤内的幂等再确认。已存在时后续步骤全部跳过、CI 正常变绿。

但**开头检查与末尾创建之间隔着数分钟的构建**，期间状态可能变化——这正是 2026-09-18 事故的成因。
末尾的二次确认是为此加的防护。

## 已发布的 v6.0.0（参考）

- tag `v6.0.0` → `2899462`；Release 流水线 run `39698498731` success（25m20s），
  MSVC 下的 `npx tauri build` + NSIS 打包通过。
- 产物：`PathEditor_6.0.0_x64-setup.exe`（4.6 MB）、`PathEditor_6.0.0_x64-portable.zip`（6.3 MB）、
  `patheditor-cli_6.0.0_x64.exe`（8.4 MB）。
