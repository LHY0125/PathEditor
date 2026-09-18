# v5.1.3 发布事故复盘

**日期**: 2026-09-18
**涉及版本**: v5.1.3
**事故类型**: 发布流程操作失误 —— 手动创建 Release 与 CI 自动发布冲突
**影响**: CI run 失败（红色历史，无法通过重跑消除）；发布本身成功，产物正确
**责任**: 开发窗口（Claude Code）未先阅读 `.github/workflows/release.yml` 即执行发布操作

---

## 1. 结论摘要

推送 `v5.1.3` tag 后，CI（`release.yml`）已按设计启动自动发布流程。在此期间开发窗口又手动执行了 `gh release create`，抢先创建了同一 tag 的 Release，导致 CI 走到最后一步（`创建 GitHub Release`）时以 `a release with the same tag name already exists` 失败。

**发布结果是正确的**（Release 已发布、物产正确、notes 完整），但 CI 留下了无法消除的失败记录，且开发窗口在事后多次基于不完整信息做出错误判断，多轮才定位根因。

---

## 2. 事故时间线（全部时间为 UTC）

| 时间     | 事件                                                                                     | 来源                             |
| -------- | ---------------------------------------------------------------------------------------- | -------------------------------- |
| 06:41:5x | 开发窗口执行 `gh release create v5.1.3 ...`，**工具回执显示 rejected**，但命令实际已提交 | 开发窗口                         |
| 06:41:54 | Release `v5.1.3` 创建                                                                    | GitHub API `createdAt`           |
| 06:42:09 | CI Release run `35316027919` 启动（由 tag push 触发）                                    | `gh run list`                    |
| 06:42:38 | asset `PathEditor_5.1.3_x64-setup.exe` 上传                                              | GitHub API `assets[].created_at` |
| 06:43:03 | asset `patheditor.exe` 上传                                                              | 同上                             |
| 06:43:32 | Release published                                                                        | GitHub API `publishedAt`         |
| 06:48:21 | CI 第 8 步失败：`a release with the same tag name already exists: v5.1.3`                | CI 日志                          |

**关键观察**：两个 asset 的 `uploader` 均为 `LHY0125`（本地 gh CLI 身份），而非 `github-actions[bot]`；asset 名也与 CI 的重命名规则（`patheditor-cli_5.1.3_x64.exe`，见 `release.yml:130`）不符。这证明 asset 来自手动命令，不是 CI。

---

## 3. 根因分析

### 3.1 直接原因

开发窗口在**未阅读 `.github/workflows/`** 的情况下规划并执行了发布操作。仓库存在 tag 触发的自动发布工作流（`release.yml:6-9`），本地手动创建 Release 会与之争夺同一资源。

### 3.2 加剧因素：命令回执与实际状态不符

`gh release create` 的工具回执显示 `rejected`（"the user doesn't want to proceed with this tool use"），但命令**实际已提交到 GitHub**。开发窗口信任了回执，未做二次核实（一条 `gh release view` 即可确认），错失了在 CI 启动前纠正的机会。

### 3.3 工作流侧的固有窗口

`release.yml` 存在一个设计窗口：

- 「检查 Release 是否已存在」步骤在**构建之前**执行（第 38-58 行）
- 「创建 GitHub Release」步骤在**构建之后**执行（第 165-188 行）
- 两者之间相隔约 **6 分钟**（Tauri Build + CLI Build）

在此期间若有其他来源创建同名 Release，末尾步骤就会失败。虽然本次事故由手动抢跑触发，但这个窗口本身是可被并发场景（手动发布、重跑、并发 workflow）命中的。

---

## 4. 处置措施

### 4.1 已完成

| 措施                   | 提交      | 说明                                                                                                                           |
| ---------------------- | --------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `release.yml` 幂等保护 | `35679da` | 「创建 GitHub Release」步骤前增加二次确认，已存在则跳过并正常退出（`exit 0`），不再报错                                        |
| CHANGELOG 补充         | `10c6b4f` | 补 `## 5.1.3 (2026-09-18)` 段落。该段落是 `release.yml:141-148` 生成发布日志的首选来源，缺失时会回退到 `git log` 逐条列 commit |
| 发布流程文档化         | `aff4284` | `CLAUDE.md` / `AGENTS.md` 新增「发布流程」章节，含唯一正确步骤与三条禁止事项                                                   |
| 文档一致性修复         | `35e0468` | 修复 `aff4284` 中 lint-staged 只格式化 `AGENTS.md` 导致两份文件分叉的问题                                                      |

`release.yml` 的幂等保护实现：

```powershell
# 幂等保护：检查步骤发生在构建之前，构建耗时数分钟，期间可能有
# 其他来源（手动发布、重跑、并发 workflow）创建了同一 tag 的
# Release。此处再确认一次，已存在则跳过而不是报错。
$existing = gh release view "$env:RELEASE_TAG" --repo "$env:GITHUB_REPOSITORY" --json tagName 2>$null
if ($LASTEXITCODE -eq 0 -and $existing) {
  Write-Host "Release $env:RELEASE_TAG 已存在，跳过创建。"
  exit 0
}
```

### 4.2 未处置

- **失败的 CI run `35316027919`**：无法通过重跑消除（Release 已存在，末尾步骤仍会走到跳过分支，虽已修但仍会记录为历史）。用户决定不处理，保留为真实事件记录。
- 该 run 的失败使 `--latest` 标记可能未设置。已核实：`gh api repos/LHY0125/PathEditor/releases/latest` 返回 `v5.1.3`，标记正确，无需补。

---

## 5. 开发窗口的判断错误（自我记录）

事故处置过程中，开发窗口多次基于不完整信息下结论，延长了定位时间。如实记录以避免重演：

| #   | 错误判断                                   | 实际情况                                                                          | 正确做法                                                          |
| --- | ------------------------------------------ | --------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| 1   | 「Release 是 CI 建的」                     | Release 由手动命令创建，时间戳早于 CI 启动 15 秒                                  | 立即用 `gh release view --json author,createdAt` 核实创建者与时间 |
| 2   | 「notes 是 CI 回退生成的 commit 列表」     | notes 与手动写入的 `/tmp/release-notes-513.md` 字节级一致                         | 检查 `release.yml:141-161` 的两条分支逻辑，而非凭印象             |
| 3   | 「asset 是 CI 上传的」                     | `gh api ... --jq '.assets[].uploader'` 显示为 `LHY0125`，证明是手动上传           | 一条 API 查询即可定论，不应推测                                   |
| 4   | 提交数口径在 13 / 14 / 15 之间反复         | 三个数分别对应 `main..HEAD`、`8481a9f..dfa3dcf`、`origin/main..HEAD` 三个不同范围 | 报数时同时说明取值范围                                            |
| 5   | 说「合并后于 main 重跑质量门」未点明工作区 | 主检出与 worktree 是两个工作区，审核窗口在 worktree 看不到                        | 涉及路径的陈述必须写明绝对工作区                                  |

**共性问题**：先推测后验证。正确顺序是**先查证再下结论**，尤其涉及远端状态（Release、asset、CI run）时，`gh api` / `gh release view` / `gh run view` 都能给出确定性答案。

---

## 6. 预防措施

已通过 `aff4284` 写入 `CLAUDE.md` / `AGENTS.md` 的「发布流程」章节，要点：

1. **发布由 CI 自动完成**，推送 `v*` tag 即触发，不要在本地手工构建或创建 Release
2. **唯一正确的发布步骤**：改版本号 → 写 CHANGELOG → 推 main → 打 tag 并推送 → 等 CI
3. **推送 tag 后不要做任何事，等 CI 完成**
4. **三条禁止事项**：推 tag 后手动建 Release、推 tag 前手动构建、未读 workflows 就规划发布
5. **CHANGELOG 是发布日志来源**：必须写当前版本段落，否则日志质量下降
6. **产物命名对照**：CI 会重命名 CLI 二进制（`patheditor-cli_<VERSION>_x64.exe`），本地产物名为 `patheditor.exe`

---

## 7. 事故定级与影响评估

| 维度       | 评估                                                                                  |
| ---------- | ------------------------------------------------------------------------------------- |
| 用户影响   | **无**。Release 正常发布，安装包与 CLI 可正常下载                                     |
| 产物正确性 | **正确**。安装包 4,488,745 字节，与本地构建产物一致；SHA256 已在 Release notes 中提供 |
| CI 历史    | **受损**。run `35316027919` 永久显示失败                                              |
| 数据完整性 | **无损**。无注册表写入、无文件丢失                                                    |
| 回滚需求   | **无**。所有变更均已提交并推送，状态一致                                              |

**定级：低危操作事故**（影响限于 CI 历史美观度，不影响交付物）。

---

## 8. 相关文件

| 文件                                                               | 说明                               |
| ------------------------------------------------------------------ | ---------------------------------- |
| `.github/workflows/release.yml`                                    | 自动发布工作流（含新增的幂等保护） |
| `CHANGELOG.md`                                                     | 5.1.3 段落                         |
| `CLAUDE.md` / `AGENTS.md`                                          | 「发布流程」章节                   |
| `docs/审核和开发/2026.09.18/PathEditor-CLI环境变量闭环测试记录.md` | 同批次的其他验证记录               |
| `docs/审核和开发/2026.09.18/PathEditor-CLI环境变量开发回执.md`     | 同批次交付回执                     |
