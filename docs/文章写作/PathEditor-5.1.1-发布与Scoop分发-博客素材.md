# PathEditor 5.1.1 发布与 Scoop 分发：博客素材包

> 用途：交给专门写博客的窗口使用。本文不是最终博客，而是尽量保留技术细节、时间线、踩坑过程、验证证据和可复用经验的素材稿。
>
> 素材整理日期：2026-09-15（Asia/Shanghai）
>
> 项目目录：`D:\Code\doing_exercises\programs\PathEditor`
>
> 当前版本：`5.1.1`
>
> 建议成稿方向：一次从“CI 红灯和复审 P0”到“自动发布 + Scoop 分发”的完整工程复盘。

---

## 0. 快速事实卡

| 项目                  | 内容                                                                           |
| --------------------- | ------------------------------------------------------------------------------ |
| 项目                  | PathEditor                                                                     |
| 定位                  | Windows PATH 环境变量编辑器，Tauri GUI + Rust CLI                              |
| 发布版本              | `5.1.1`                                                                        |
| 主仓库                | https://github.com/LHY0125/PathEditor                                          |
| Release               | https://github.com/LHY0125/PathEditor/releases/tag/v5.1.1                      |
| 个人 Scoop bucket     | https://github.com/LHY0125/scoop-bucket                                        |
| Scoop manifest        | https://github.com/LHY0125/scoop-bucket/blob/master/bucket/patheditor-cli.json |
| 技术栈                | Tauri 2.x、React 19、TypeScript strict、Rust workspace、NSIS、Scoop            |
| 发布标签              | `v5.1.1`                                                                       |
| 关键发布提交          | `58c7a80 fix: 修复 Release 存在性检查`                                         |
| Scoop bucket 关键提交 | `4670c4f feat: add patheditor-cli manifest`                                    |
| 发布 workflow         | `.github/workflows/release.yml`                                                |
| Release workflow run  | GitHub Actions run `34852968548`，成功                                         |
| Scoop CI run          | run `34913998372`，成功                                                        |
| Scoop Excavator run   | run `34914410121`，成功                                                        |

GitHub Release 资产：

| 资产                             |            大小 |
| -------------------------------- | --------------: |
| `PathEditor_5.1.1_x64-setup.exe` | 2,251,586 bytes |
| `patheditor-cli_5.1.1_x64.exe`   | 1,273,344 bytes |

---

## 1. 这轮工作从哪里开始

PathEditor 原来已经能构建、能测试，但质量门、状态语义和分发流程都没有完全收口。

一开始的问题不是“功能完全不能用”，而是：

- CI 门禁不是绿色，不能作为重构保护网。
- 非管理员模式下，用户 PATH 被错误锁死。
- 导入和配置应用会丢失 `enabled=false`。
- PATH 超过 20 条之后，验证和变量展开停止。
- 禁用状态和注册表保存之间缺少一致的事务边界。
- IPC 类型契约靠手写复制，真实运行和 mock 测试已经出现偏差。
- GUI 和 CLI 的导入导出、禁用语义开始漂移。
- 发布仍然是手工流程，没有形成 tag 触发的自动 Release。

这决定了后面的工作不是简单“加一个功能”，而是先修质量门和状态模型，再统一契约，最后把发布和分发打通。

---

## 2. 第一轮审核：13 项问题与 P0/P1/P2/P3

第一轮审核文档：

```text
docs/审核和开发/2026.09.14/PathEditor-重构优化审核.md
```

审核结论的核心是：**单元测试数量不少，但 CI 还不能作为可靠的重构保护网。**

### 2.1 当时的验证基线

| 检查项                   | 结果         | 关键细节                                                    |
| ------------------------ | ------------ | ----------------------------------------------------------- |
| `npm run build`          | 通过         | Vite 主 JS 约 319.08 kB，gzip 约 99.06 kB                   |
| `npm run lint`           | 通过但有警告 | 0 error，2 个 TanStack Virtual 与 React Compiler 兼容性警告 |
| `npm run test:coverage`  | 失败         | 105 个前端测试通过，但 Lines 66.83%，低于 80%               |
| `cargo test --workspace` | 通过         | 57 个 Rust 测试通过                                         |
| `cargo fmt --check`      | 失败         | `core/src/backup.rs`、`core/src/fs.rs` 有格式差异           |
| Clippy                   | 失败         | `core/src/system.rs:132` 的布尔比较触发 `bool_comparison`   |
| `npm run test:e2e`       | 未完成       | 本机缺少 Playwright Chromium                                |
| `npm run format:check`   | 通过         | 前端格式一致                                                |

这只是第一轮基线。后面复审又发现了“测试看起来通过、真实运行却可能失败”的问题。

### 2.2 问题清单概览

| 编号 | 优先级 | 问题                                       | 后续处理方向                                           |
| ---- | ------ | ------------------------------------------ | ------------------------------------------------------ |
| F-01 | P0     | CI 门禁不是绿色                            | 恢复覆盖率、格式、Clippy 和构建门禁                    |
| F-02 | P0     | 非管理员模式把用户 PATH 编辑也锁死         | 按 HKLM/HKCU 分别判断能力                              |
| F-03 | P0     | 导入和应用配置丢失 `enabled=false`         | 引入并保留 `PathEntry { path, enabled }`               |
| F-04 | P1     | PATH 超过 20 条后停止验证/展开             | 改成有界并发队列，不再按前 20 条截断                   |
| F-05 | P1     | GUI 清理不检查目录是否存在                 | 统一走 Rust `clean_path_entries`                       |
| F-06 | P1     | TS/Rust 导入导出双实现漂移                 | 运行时统一走 Rust，TS 保留为兼容夹具                   |
| F-07 | P1     | 禁用状态先写侧车、再写注册表，缺少事务边界 | 注册表成功后才提交快照，失败保留 pending               |
| F-08 | P2     | 分析界面重复扫描 PATH 目录                 | `scan_paths` 单次枚举，最多 8 个扫描线程               |
| F-09 | P1     | IPC 调用散落，类型契约手工复制             | 集中到 `src/services/backend.ts` 并做运行时形状校验    |
| F-10 | P1     | 测试结构不平衡，E2E 不覆盖真实闭环         | mock E2E 保留，真实 Tauri 验证单独记录                 |
| F-11 | P2     | 核心模块过长                               | 拆分 app-store、AnalyzeDialog、ProfileDialog、CLI 模块 |
| F-12 | P2     | CSP、文件读取和防御性边界需收紧            | 不允许 CSP 为 `null`，限制导入文件读取范围             |
| F-13 | P3     | 文档、国际化、错误提示漂移                 | 同步文档；错误码 + 本地化仍待后续                      |

### 2.3 P0 的三个问题为什么重要

#### F-01：CI 不是绿色

如果覆盖率、格式和 Clippy 本身就失败，那么“测试通过”不能证明重构安全。重构前必须先让质量门可执行、可复现。

#### F-02：用户 PATH 被错误锁死

原实现用一个全局 `isAdmin` 推导两个 PATH 是否可写。真实 Windows 权限模型不是这样：

- 系统 PATH 在 `HKLM`，通常需要管理员。
- 用户 PATH 在 `HKCU`，普通用户通常可以编辑。
- 两个 hive 的读写能力必须分别探测。

这个错误的后果不是界面问题，而是普通用户明明能改自己的 PATH，却被应用锁死。

#### F-03：配置导入丢失禁用状态

如果只导入路径字符串，`enabled=false` 的信息就会丢失。一个“禁用但保留”的路径可能在导入后重新变成启用状态，或者直接消失。

后续统一契约：

```ts
type PathEntry = {
  path: string;
  enabled: boolean;
};
```

导入、配置、注册表、撤销重做、侧车快照都围绕这个契约工作。

---

## 3. 第一轮整改与复审：修了不等于真的对了

第一轮整改后，复审文档给出的结论是 **Request Changes**。

复审报告：

```text
docs/审核和开发/2026.09.14/PathEditor-复审报告.md
```

这轮复审发现，部分代码“看起来修了”，但真实运行时仍然可能出错。最典型的是 `PathCapabilities` 字段命名问题。

复审窗口的独立验证数据是：

- 前端测试：124 passed
- Rust 测试：58 passed
- 覆盖率：Lines 87.09%
- E2E：13 passed
- 构建和 Clippy：通过

这组数据说明第一轮整改已经明显改善了质量门，但还没有覆盖真实 Tauri 运行时契约。后面的 `PathCapabilities` 问题就是在这个阶段被重新抓出来的。

### 3.1 REV-01：`PathCapabilities` 的真实契约错误

Rust 结构体用 `Serialize` 序列化后，字段仍然是：

```text
can_read_system
can_write_system
can_read_user
can_write_user
```

前端按 camelCase 读取：

```text
canReadSystem
canWriteSystem
canReadUser
canWriteUser
```

问题在于：Unit 和 E2E mock 恰好返回了 camelCase，于是测试通过；真实 Tauri IPC 返回的却是 snake_case，普通用户权限修复在真实环境仍可能失效。

这个问题的修复方向：

- Rust 侧增加 `#[serde(rename_all = "camelCase")]`。
- 增加共享 fixture：`tests/fixtures/path-capabilities.json`。
- 前端 `backend.ts` 对能力对象做运行时形状校验。
- 不再只依赖 TypeScript 类型断言。

这个案例非常适合写进博客，因为它是“mock 测试掩盖真实运行时错误”的典型案例。

### 3.2 REV-02：验证队列可能永久 pending

验证任务在列表 rerender 时会取消旧批次。新批次又跳过 in-flight 项，导致部分路径可能再也没有人处理，永久停在 `pending`。

修复方向：

- 使用共享 in-flight Promise。
- 让 rerender 后的新批次复用正在进行的验证。
- 增加 deferred rerender 回归测试。

### 3.3 REV-03：禁用路径跨重启丢失

禁用路径保存后会从注册表移除，但重启时 `disabled.json` 里的孤儿记录不会重新合并。结果是：

- 当前会话看起来正常。
- 重启后禁用项消失。
- CLI `disable` 只写 sidecar，语义和 GUI 不一致。

修复方向：

- `disabled.json` 保存完整 `systemSnapshot` 和 `userSnapshot`。
- 启动时以注册表为“启用路径真相来源”，再用快照恢复禁用项和顺序。
- 兼容旧格式的禁用字符串数组。
- CLI `disable` 也使用完整快照语义。

### 3.4 REV-04：保存成功状态不真实

原有逻辑中，注册表写入成功但 `disabled.json` 写入失败时，Store 仍然把最新快照标记为已保存。用户看到“已保存”，但下一次保存不会补写侧车文件。

修复方向：

- 引入 `_pendingSys` / `_pendingUser`。
- 注册表写入成功但快照写入失败时，保留 `isModified = true`。
- 下次保存只补写侧车，不重复覆盖注册表。
- 分别报告 system/user 两个 hive 的成功或失败。

---

## 4. 第二轮整改后的验证结果

复审回执：

```text
docs/审核和开发/2026.09.14/PathEditor-复审回执.md
```

最终 `npm run verify` 结果：

| 检查项              | 结果                        |
| ------------------- | --------------------------- |
| Prettier            | 通过                        |
| ESLint              | 0 error，2 个非阻断 warning |
| 前端单元测试        | 127 passed                  |
| 覆盖率              | Lines 86.33%                |
| 生产构建            | 通过                        |
| `cargo fmt --check` | 通过                        |
| Clippy              | 通过                        |
| Rust 测试           | 63 passed                   |

另外：

```text
npm run test:e2e → 13 passed
```

E2E 使用 mock IPC，不写真实注册表。它能验证前端流程，但不能替代真实 Tauri + 真实注册表集成测试。

### 4.1 仍未完成的真实环境验证

后续必须单独验证：

- 普通用户实际启动 Tauri，编辑并保存用户 PATH。
- 禁用路径后退出、重启、重新启用。
- 注册表与 `disabled.json` 的异常/权限失败组合。
- 操作前后注册表快照、侧车文件和回滚结果。

这部分没有被包装成“已完成”，因为 mock E2E 不等于真实 Windows 集成测试。

### 4.2 非阻断残余风险

- 注册表已有重复 PATH 项时，快照合并可能按路径键去重并折叠重复项。
- CLI 若注册表成功但 sidecar 写入失败，仍是部分成功，暂无 GUI 等价的持久化补写机制。
- 旧 TS `import-export.ts`、`validation.ts` 仍作为兼容/测试夹具保留。
- Rust 错误码化和前端本地化映射尚未完成。
- `ts-rs` / `specta` 自动类型生成尚未引入。

---

## 5. 从 5.1.0 到 5.1.1：版本同步与本地产物

版本没有随意跳级，仍然围绕 `5.1.1` 收口。升级时检查并同步了这些位置：

| 文件                               | 字段                          |
| ---------------------------------- | ----------------------------- |
| `package.json`                     | `version`                     |
| `package-lock.json`                | 顶层和依赖版本信息            |
| `Cargo.toml`                       | `[workspace.package] version` |
| `Cargo.lock`                       | workspace crate 版本          |
| `gui/tauri.conf.json`              | `version`、窗口标题           |
| `gui/Cargo.lock`                   | GUI crate 版本                |
| `README.md`                        | 版本徽章和安装说明            |
| `index.html`                       | 页面版本信息                  |
| `tests/unit/import-export.test.ts` | 测试中的版本断言              |
| `CLAUDE.md` / `AGENTS.md`          | 开发指南版本说明              |

Rust workspace 三个 crate 都同步为：

```text
path-editor-core  5.1.1
patheditor        5.1.1
patheditor-cli    5.1.1
```

本地构建产物：

```text
GUI:
D:\Code\doing_exercises\programs\PathEditor\target\release\bundle\nsis\PathEditor_5.1.1_x64-setup.exe

CLI:
D:\Code\doing_exercises\programs\PathEditor\target\release\patheditor.exe
```

本地 CLI 是 GNU 工具链构建，SHA-256：

```text
4de0b898981c7f532d918fe917d0fae9dd2f0b855970130f66a9ce7360cffec2
```

GitHub Release 上的 CLI 是 MSVC 工具链构建，SHA-256：

```text
9622b1d04d7d5022e2fcc75acd41a1e1dbd0ae69580ecf122cc226bff3d2c3c7
```

两者 hash 不同是正常的：**同一份源码、不同工具链、不同构建产物**。Scoop manifest 必须引用 Release 资产的 hash，不能引用本地 GNU 构建的 hash。

---

## 6. GitHub Release 自动化

### 6.1 删除旧 CI workflow

原来的 `.github/workflows/ci.yml` 已经不再承担发布职责，后来按要求删除，只保留：

```text
.github/workflows/release.yml
```

这里的思路是：

- 普通 push 和 PR 的质量门由本地 `npm run verify`、Husky、lint-staged、Dependabot、CODEOWNERS、Issue/PR 模板等分担。
- 发布由 tag 触发，避免每次 push 都跑完整安装包构建。
- 发布 workflow 与版本号强校验，防止 tag 和项目版本不一致。

### 6.2 Release workflow 的完整流程

`release.yml` 的核心步骤：

1. `on: push: tags: ['v*']`
2. 完整 checkout，`fetch-depth: 0`，保留历史用于生成 release notes。
3. 校验 tag 格式：
   ```text
   vMAJOR.MINOR.PATCH
   可带预发布后缀
   ```
4. 检查 Release 是否已经存在：
   - `200`：已存在，跳过构建。
   - `404`：继续构建。
5. Node 20 + npm cache。
6. Rust 工具链覆盖本机 GNU 工具链配置，改用 MSVC：
   ```text
   stable-x86_64-pc-windows-msvc
   ```
7. 校验三个版本的单一来源：
   ```text
   package.json
   gui/tauri.conf.json
   Cargo.toml workspace version
   ```
8. `npm ci`
9. `npx tauri build`
10. `cargo build --release -p patheditor-cli`
11. 整理资产：
    ```text
    PathEditor_<version>_x64-setup.exe
    patheditor-cli_<version>_x64.exe
    ```
12. 生成 release notes：
    - 优先读取 `CHANGELOG.md` 中对应版本段落。
    - 如果没有，则取上一个 tag 到当前 tag 的 `git log`。
13. 调用 `gh release create` 创建 Release 并上传资产。

### 6.3 第一次 workflow 为什么失败

第一次运行 `#9` 失败。原因很典型：

- PowerShell 把 `gh release view` 在 Release 不存在时的非零退出码当成了失败。
- 在 PowerShell 的 `$ErrorActionPreference = 'Stop'` 下，命令退出码非零会直接抛错，导致 workflow 还没进入构建就终止。

修复：

```powershell
$response = Invoke-WebRequest -Uri $uri -Headers $headers -SkipHttpErrorCheck
```

用 `Invoke-WebRequest -SkipHttpErrorCheck` 直接读取 HTTP 状态码，而不是依赖 `gh release view` 的退出码。

修复提交：

```text
58c7a80 fix: 修复 Release 存在性检查
```

因为 `v5.1.1` 的 Release 当时还没有发布成功，所以把这个尚未发布的 tag force-update 到了 `58c7a80`，再运行 workflow `#10`，结果成功。

### 6.4 Release 页面和资产

Release：

```text
https://github.com/LHY0125/PathEditor/releases/tag/v5.1.1
```

资产：

```text
PathEditor_5.1.1_x64-setup.exe     2,251,586 bytes
patheditor-cli_5.1.1_x64.exe       1,273,344 bytes
```

Release 正文来自 `CHANGELOG.md` 的 5.1.1 段落，主要覆盖：

- 非管理员权限判断修复
- pending 验证队列修复
- 禁用状态跨重启修复
- 侧车写入失败重试
- 导入权限检查
- 超过 20 条路径不再截断
- 清理目录存在性检查
- `PathCapabilities` camelCase 契约修复
- TS/Rust 导入导出语义统一
- CLI 禁用语义统一

---

## 7. Scoop 分发：从同名冲突到个人 bucket

### 7.1 为什么不能直接叫 `patheditor`

本地执行：

```powershell
scoop search patheditor
```

发现 Scoop Extras 已经有一个同名应用：

```text
Name        : patheditor
Version     : 1.0
Description : A convenient GUI for editing the PATH environment variable
Website     : https://archive.codeplex.com/?p=patheditor2
Binaries    : PathEditor.exe
```

这是一个 2020 年左右的旧 GUI，不是当前 PathEditor。

冲突点有两个：

1. manifest 名称冲突：不能再往官方 Extras 里塞一个同名 `patheditor`。
2. shim 冲突：旧应用的可执行名是 `PathEditor.exe`，新 CLI 是 `patheditor.exe`。Windows 文件系统大小写不敏感，同名 shim 会互相覆盖或抢占。

因此推荐方案是：

- manifest 名称：`patheditor-cli`
- 实际命令名：仍然是 `patheditor`
- 发布位置：个人 bucket `LHY0125/scoop-bucket`

### 7.2 创建个人 bucket

使用官方模板：

```text
ScoopInstaller/BucketTemplate
```

创建：

```text
https://github.com/LHY0125/scoop-bucket
```

仓库配置：

- 默认分支：`master`
- topic：`scoop-bucket`
- Actions：允许全部
- workflow 默认权限：`read`
- 保留模板自带的：
  - `.github/workflows/ci.yml`
  - `.github/workflows/excavator.yml`
  - `.github/workflows/issues.yml`
  - `.github/workflows/pull_request.yml`

模板自带 CI 和 Excavator，后者每 4 小时检查一次 manifest 更新。

### 7.3 manifest 内容

文件：

```text
bucket/patheditor-cli.json
```

核心内容：

```json
{
  "version": "5.1.1",
  "description": "Command-line editor for the Windows PATH environment variable",
  "homepage": "https://github.com/LHY0125/PathEditor",
  "license": "MIT",
  "architecture": {
    "64bit": {
      "url": "https://github.com/LHY0125/PathEditor/releases/download/v5.1.1/patheditor-cli_5.1.1_x64.exe#/patheditor.exe",
      "hash": "sha256:9622b1d04d7d5022e2fcc75acd41a1e1dbd0ae69580ecf122cc226bff3d2c3c7"
    }
  },
  "bin": "patheditor.exe",
  "checkver": {
    "url": "https://api.github.com/repos/LHY0125/PathEditor/releases/latest",
    "jsonpath": "$.tag_name",
    "regex": "v?([\\d.]+(?:-[0-9A-Za-z.-]+)?)"
  },
  "autoupdate": {
    "architecture": {
      "64bit": {
        "url": "https://github.com/LHY0125/PathEditor/releases/download/v$version/patheditor-cli_$version_x64.exe#/patheditor.exe",
        "hash": {
          "url": "https://api.github.com/repos/LHY0125/PathEditor/releases/tags/v$version",
          "jsonpath": "$.assets[?(@.name == 'patheditor-cli_$version_x64.exe')].digest"
        }
      }
    }
  }
}
```

知识点：

- URL 后面的 `#/patheditor.exe` 是 Scoop 的下载重命名语法。
- `bin` 是安装后创建 shim 的可执行名，所以用户实际输入的是 `patheditor`，不是 `patheditor-cli`。
- `autoupdate.hash.jsonpath` 直接读取 GitHub Release API 返回的 `digest` 字段。
- `checkver` 使用 `api.github.com`，而不是 `github.com/.../releases/latest`，这是为了避开本地网络对 GitHub 主站的不稳定访问。

### 7.4 为什么 checkver 改成 API

最初写成：

```json
"checkver": {
  "github": "https://github.com/LHY0125/PathEditor"
}
```

本机执行 Scoop 的 checkver 时出现：

```text
The SSL connection could not be established
URL https://github.com/LHY0125/PathEditor/releases/latest is not valid
```

这不是 manifest 语法错误，而是本地 GitHub 主站连接不稳定。后来改成：

```json
"checkver": {
  "url": "https://api.github.com/repos/LHY0125/PathEditor/releases/latest",
  "jsonpath": "$.tag_name",
  "regex": "v?([\\d.]+(?:-[0-9A-Za-z.-]+)?)"
}
```

在 GitHub Actions 的 Windows runner 上，Excavator 运行成功，说明这个 manifest 的自动检查逻辑是可用的。

### 7.5 bucket 仓库的自动化验证

bucket 创建并推送后：

```text
CI        run 34913998372   success
Excavator run 34914410121   success
```

CI 在 `master` push 后运行，Excavator 手动触发后也通过。

### 7.6 本地添加 bucket 和安装

HTTPS 添加 bucket 时，本机访问 `github.com:443` 超时：

```text
Checking repo... ERROR 'https://github.com/LHY0125/scoop-bucket' doesn't look like a valid git repository
fatal: unable to access 'https://github.com/LHY0125/scoop-bucket/': Failed to connect to github.com:443
```

改用 SSH：

```powershell
scoop bucket add lhy git@github.com:LHY0125/scoop-bucket.git
```

成功。

随后 `scoop install lhy/patheditor-cli` 的 aria2 下载出现 0 B/s 循环，失败原因是本地到 `github.com` 的 Release 资产连接超时。

因为此前已经用 `curl` 成功下载过 Release 资产并验证过 hash，于是把有效文件放入 Scoop 缓存，再执行安装：

```powershell
scoop install lhy/patheditor-cli -u
```

结果：

```text
Checking hash of patheditor-cli_5.1.1_x64.exe ... ok.
Creating shim for 'patheditor'.
'patheditor-cli' (5.1.1) was installed successfully!
```

安装信息：

```text
Name        : patheditor-cli
Description : Command-line editor for the Windows PATH environment variable
Version     : 5.1.1
Source      : lhy
Website     : https://github.com/LHY0125/PathEditor
License     : MIT
Binaries    : patheditor.exe
```

这段可以写成博客里的“本地网络环境不等于 CI 网络环境”案例：manifest 和 hash 都是对的，失败发生在本地网络链路，而不是包定义本身。

---

## 8. PATH 优先级：Scoop 装了，但未必真的接管

Scoop 安装成功后，一开始 `patheditor` 实际并没有走 Scoop 版本，因为手工放进 Cargo bin 的 GNU 构建排在 Scoop shims 前面。

当时结果：

```text
Get-Command patheditor
→ D:\settings\Language\Rust\.cargo\bin\patheditor.exe

where.exe patheditor
→ D:\settings\Language\Rust\.cargo\bin\patheditor.exe
→ D:\settings\settings\Scoop\shims\patheditor.exe

scoop which patheditor
→ D:\settings\Language\Rust\.cargo\bin\patheditor.exe
```

三个 hash：

```text
Cargo 手工版 GNU：
4de0b898981c7f532d918fe917d0fae9dd2f0b855970130f66a9ce7360cffec2

Scoop 应用目录里的 MSVC 版：
9622b1d04d7d5022e2fcc75acd41a1e1dbd0ae69580ecf122cc226bff3d2c3c7

Scoop shim：
140e3801d8adeda639a21b14e62b93a4c7d26b7a758421f43c82be59753be49b
```

注意：shim 的 hash 和真实应用 binary 的 hash 不同是正常的。shim 只是一个启动器，实际应用在：

```text
D:\settings\settings\Scoop\apps\patheditor-cli\current\patheditor.exe
```

这意味着：

- `patheditor --version` 仍然输出 `5.1.1`。
- 但运行的是手工 GNU 构建。
- 以后 Scoop 更新到新版本时，手工版会继续遮挡，用户实际不会用到更新后的版本。

最终按用户明确要求，删除手工可执行文件：

```text
D:\settings\Language\Rust\.cargo\bin\patheditor.exe
```

随后又按用户明确要求删除旧版本备份：

```text
D:\settings\Language\Rust\.cargo\bin\patheditor.exe.5.1.0.20260914-213526.bak
```

最终验证：

```text
Get-Command patheditor
→ D:\settings\settings\Scoop\shims\patheditor.exe

where.exe patheditor
→ D:\settings\settings\Scoop\shims\patheditor.exe

patheditor --version
→ patheditor 5.1.1

scoop which patheditor
→ D:\settings\settings\Scoop\apps\patheditor-cli\current\patheditor.exe
```

Cargo bin 目录下已经没有 `patheditor*` 文件。CLI 现在完全由 Scoop 管理。

这个细节很适合作为博客的“最后一个坑”：**一个包安装成功，不等于 PATH 上实际生效的是这个包。** 必须检查：

```powershell
Get-Command <命令>
where.exe <命令>
scoop which <命令>
```

---

## 9. 验证证据索引

### 9.1 PathEditor 主仓库

- HEAD：
  ```text
  58c7a80 fix: 修复 Release 存在性检查
  ```
- tag：
  ```text
  v5.1.1
  ```
- Release workflow：
  ```text
  run 34852968548  成功
  ```
- Release：
  ```text
  https://github.com/LHY0125/PathEditor/releases/tag/v5.1.1
  ```

### 9.2 Scoop bucket

- 仓库：
  ```text
  https://github.com/LHY0125/scoop-bucket
  ```
- 关键提交：
  ```text
  4670c4f feat: add patheditor-cli manifest
  ```
- CI：
  ```text
  run 34913998372  成功
  ```
- Excavator：
  ```text
  run 34914410121  成功
  ```

### 9.3 本机验证

- `scoop info lhy/patheditor-cli`：
  ```text
  Name        : patheditor-cli
  Version     : 5.1.1
  Source      : lhy
  Binaries    : patheditor.exe
  ```
- 安装 hash 校验：
  ```text
  Checking hash of patheditor-cli_5.1.1_x64.exe ... ok.
  ```
- 最终命令：
  ```text
  patheditor --version
  → patheditor 5.1.1
  ```

### 9.4 本地网络问题的时间点

- `github.com:443` 本地间歇性超时。
- `ssh.github.com:443` 可以通过 SSH 建立连接。
- `api.github.com` 在某些工具里工作，但 .NET WebClient / Invoke-WebRequest 曾出现 TLS EOF。
- CI/Excavator 在 GitHub Actions runner 上运行正常。
- 这类问题应记录为“本地网络环境差异”，不要误判为 manifest 或代码问题。

---

## 10. 可复用的工程经验

### 10.1 mock 通过不代表真实契约正确

`PathCapabilities` 是最典型的案例：

- mock 返回 camelCase。
- 真实 Rust Serialize 返回 snake_case。
- 前端按 camelCase 读取。
- 测试全通过，真实环境可能失效。

结论：跨语言边界必须跑契约测试，不能只靠 mock。

### 10.2 状态保存必须有事务语义

“注册表写入成功”和“侧车快照写入成功”是两个步骤。任何一个失败都要能表达为部分成功，并允许下一次补写。

GUI 的 `_pending` 机制解决的是这个语义问题，不只是“失败重试”的 UI 细节。

### 10.3 质量门本身也要被验证

覆盖率阈值、Clippy、fmt、E2E 不是装饰。质量门失败时，先修门，再做重构，否则测试通过没有意义。

### 10.4 单一版本源之外的同步点要列清单

虽然 `package.json` 和 `Cargo.toml` 是主要版本源，但实际还要同步：

- `package-lock.json`
- `Cargo.lock`
- `gui/Cargo.lock`
- `gui/tauri.conf.json`
- `README.md`、`index.html`
- `CLAUDE.md` / `AGENTS.md`
- 测试中的版本断言

### 10.5 Release 资产 hash 和本地构建 hash 不能混用

本地 GNU 构建：

```text
4de0b898981c7f532d918fe917d0fae9dd2f0b855970130f66a9ce7360cffec2
```

GitHub Release MSVC 构建：

```text
9622b1d04d7d5022e2fcc75acd41a1e1dbd0ae69580ecf122cc226bff3d2c3c7
```

Scoop manifest 必须使用 Release 资产 hash。不同工具链的产物 hash 不同是正常现象。

### 10.6 Scoop 的 manifest 名称和命令名可以不同

- manifest：`patheditor-cli`
- 安装命令：`scoop install lhy/patheditor-cli`
- 实际命令：`patheditor`

这是通过 `bin` 字段实现的，不需要把 manifest 名和命令名绑定在一起。

### 10.7 同名冲突要同时考虑 registry 名称和 Windows shim

旧 `patheditor` 的可执行名是 `PathEditor.exe`。新 CLI 是 `patheditor.exe`。Windows 文件系统大小写不敏感，安装在一起会冲突。

所以个人 bucket 方案使用 `patheditor-cli` 作为 manifest 名称，避免和官方 Extras 里的旧 GUI 混淆。

### 10.8 GitHub 主站、API、SSH 可能是三条不同链路

这次实际遇到：

- `github.com:443`：本地不稳定。
- `ssh.github.com:443`：可连接。
- `api.github.com`：部分工具可连接，部分 .NET 工具 TLS 失败。
- GitHub Actions runner：正常。

排查时要区分：

```text
git clone 失败
≠ API 失败
≠ Actions 失败
≠ manifest 错误
```

---

## 11. 给博客写作窗口的建议

### 11.1 推荐标题

可选方向：

- 《从 CI 红灯到 Scoop 分发：PathEditor 5.1.1 的一次完整工程复盘》
- 《修了 13 个问题之后，我才发现 mock 测试掩盖了真实 bug》
- 《PathEditor 5.1.1：Windows PATH 编辑器的重构、自动发布与 Scoop 上架》
- 《一次 Windows 工具链项目的发布闭环：Tauri、Rust、GitHub Actions 与 Scoop》

### 11.2 推荐叙事线

建议按“问题 -> 复审 -> 修复 -> 发布 -> 分发 -> 验证”的顺序写：

1. 项目背景：PathEditor 是什么，为什么需要同时做 GUI 和 CLI。
2. 第一轮审核：CI 红灯、P0/P1、状态模型问题。
3. 第一轮整改：看起来修完了。
4. 复审反转：mock 测试掩盖了 `PathCapabilities` 字段契约错误。
5. 第二轮修复：权限、验证队列、禁用快照、保存事务。
6. 质量门恢复：127 前端、63 Rust、86.33% 覆盖率、13 E2E。
7. 版本发布：同步 5.1.1，改造 Release workflow。
8. 第一次 workflow 失败：PowerShell 退出码和 `gh release view`。
9. 再次运行成功：Release 资产、版本说明、tag 规则。
10. Scoop 分发：同名冲突、个人 bucket、BucketTemplate、CI 和 Excavator。
11. 本地安装踩坑：HTTPS 超时、SSH bucket、aria2 0 B、缓存注入。
12. 最后一个坑：Scoop 安装成功但 PATH 仍被手工版遮挡。
13. 结论：真正的工程闭环不是“代码能跑”，而是质量门、发布、分发、实际运行路径都能验证。

### 11.3 建议保留的细节

博客里可以保留：

- `PathCapabilities` 的 snake_case / camelCase 例子。
- “mock 返回 camelCase，真实 IPC 返回 snake_case”的反转。
- `PathEntry { path, enabled }` 契约。
- `disabled.json` 的完整快照和重启合并。
- `_pendingSys` / `_pendingUser` 的事务重试语义。
- 105 → 127 个前端测试、57 → 63 个 Rust 测试、66.83% → 86.33% 覆盖率的变化。
- Release workflow 的 tag、版本校验、MSVC、双资产。
- `#/patheditor.exe`、`bin`、`checkver`、`autoupdate` 这几个 Scoop 知识点。
- `Get-Command`、`where.exe`、`scoop which` 的区别。
- Release 资产 hash 和本地构建 hash 不同的原因。

### 11.4 建议弱化的细节

博客不必展开：

- 所有 13 个问题的完整表格。
- 每个 workflow 步骤的完整 YAML。
- 每个 commit 的完整 hash。
- TanStack Virtual / React Compiler 的 warning 细节，除非文章主题是前端性能。
- 大量本地网络日志，只要总结成“GitHub 主站、API、SSH 是三条链路”即可。

### 11.5 适合放文章结尾的一句话

可以围绕这个主题收束：

> 这次发布真正修掉的，不只是几个失败测试，而是“测试通过、构建成功、安装成功、实际运行路径生效”之间的断点。

---

## 12. 尚未完成和需要如实说明的事项

以下内容不能在博客里写成“已经完全验证”：

- 真实 Tauri 环境中的普通用户用户 PATH 编辑。
- 禁用路径保存、退出、重启、重新启用的真实注册表闭环。
- 注册表重复 PATH 项被快照合并时的保留语义。
- CLI 注册表成功但 sidecar 写入失败时的等价重试机制。
- 旧 TS `import-export.ts`、`validation.ts` 兼容夹具的长期维护。
- Rust 错误码 + GUI/CLI 本地化的统一错误契约。
- `ts-rs` / `specta` 自动类型生成。
- 官方 Scoop Main/Extras 提交；当前使用的是个人 bucket。

---

## 13. 可直接引用的链接

- 主仓库：
  https://github.com/LHY0125/PathEditor
- Release：
  https://github.com/LHY0125/PathEditor/releases/tag/v5.1.1
- Scoop bucket：
  https://github.com/LHY0125/scoop-bucket
- Scoop manifest：
  https://github.com/LHY0125/scoop-bucket/blob/master/bucket/patheditor-cli.json
- 第一轮审核：
  `docs/审核和开发/2026.09.14/PathEditor-重构优化审核.md`
- 复审报告：
  `docs/审核和开发/2026.09.14/PathEditor-复审报告.md`
- 复审回执：
  `docs/审核和开发/2026.09.14/PathEditor-复审回执.md`

---

## 14. 交接备注

- 本文是博客素材，不是最终发布稿。
- 所有数字和命令来自本轮实际上下文与仓库/Release/Scoop 验证。
- 写博客时建议保留“复审发现 mock 掩盖真实契约错误”这个反转点，它比单纯的版本发布更有技术叙事价值。
- 如果博客面向普通读者，可以减少代码；如果面向开发者，建议保留 `PathCapabilities`、`PathEntry`、`disabled.json`、Release workflow 和 Scoop manifest 的细节。
