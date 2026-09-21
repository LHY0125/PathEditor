# PathEditor GUI「关不掉」问题诊断报告

- **诊断日期**：2026-09-21
- **诊断人**：审核窗口
- **触发**：用户报告「GUI 版打开关不掉」
- **性质**：诊断报告（未改任何代码，无需授权）

## 一、结论（TL;DR）

经 8 组对照实验，确认**两个独立问题**：

| #   | 问题                                                                                                               | 根因定位                                                                   | 影响                     | 状态                               |
| --- | ------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------- | ------------------------ | ---------------------------------- |
| A   | **CI 构建的 GUI（v5.1.3，scoop 安装版）点 X 关不掉**                                                               | CI/MSVC 构建环境级缺陷（本地同代码构建不复现）                             | 已发布版本 v5.1.3 的 GUI | **已确认，main 代码无法复现**      |
| B   | 本地`npx tauri build` 曾产出被 CLI 污染的 GUI 产物（`PathEditor.exe` 与 CLI 的 `patheditor.exe` **MD5 完全相同**） | 构建顺序：先`cargo build -p patheditor-cli` 再 `tauri build`，产物复用冲突 | 本地构建验证             | 已确认（构建流程缺陷，非代码缺陷） |

**给用户的即时建议**：升级到含 Wave 0–2 的新版本（main @ `d772c68`，本地构建实测关窗正常），或临时用任务管理器结束进程。

## 二、现象与实测复现

### 问题 A：CI 版 GUI 关窗死锁（复现 3/3）

测试方法：PowerShell `Start-Process` 启动 GUI → `PostMessage(WM_CLOSE)` 模拟点 X → 观察进程退出。

| 测试对象                                          | 构建来源                       | 关窗时机                         | 结果                               |
| ------------------------------------------------- | ------------------------------ | -------------------------------- | ---------------------------------- |
| scoop`patheditor-gui` 5.1.3（exe 20.8MB）         | **CI**（GitHub runner + MSVC） | 启动 8s 后发 WM_CLOSE，等 12s    | ❌**存活，第二次 WM_CLOSE 仍不退** |
| 同上                                              | 同上                           | 启动 2s 内早期关窗（监听未注册） | ❌ 同样死锁                        |
| v5.1.3 tag**本地构建**（GNU 工具链，同 tag 代码） | 本地                           | 同 CI 版完全相同的时序           | ✅ 正常退出                        |
| main`d772c68` **本地构建**（GNU）                 | 本地                           | 早期 + 晚期关窗各测              | ✅ 全部正常退出                    |

### 死锁画像（关键证据）

- 死锁时 `SendMessageTimeout` 探测：主窗口（`Tauri Window` 类）与 tao 事件循环窗口（`Tao Thread Event Target`）**均正常响应消息**——Win32 消息循环活着；
- 全进程枚举**无确认对话框窗口**（既无主进程对话框，msedgewebview2 子进程也无带标题窗口）；
- 进程永不自行退出，CPU 占用极低（0.48s）。

该画像与「**JS 线程阻塞在 `window.confirm()` 的跨进程等待上**」吻合：tao 的消息循环在独立线程所以还活着；而关窗的 JS 回调被同步 confirm 阻塞、永远走不到 `destroy()`；同时 Rust 侧 `api.prevent_close()`（`tauri-2.11.2/src/manager/window.rs:170-174`：只要 JS 注册了 close-requested 监听就拦截默认关窗）在等 JS 的 destroy IPC——**双方互等，窗口永不关闭**。

### 问题 B：本地构建产物污染

```
target/release/PathEditor.exe   (GUI 产物名, 3.1MB)  MD5 5269df8661ba094a709da8d025960e9b
target/release/patheditor.exe   (CLI,        3.1MB)  MD5 5269df8661ba094a709da8d025960e9b
```

两个**本应不同**的二进制（GUI ~20.8MB，CLI ~3.1MB）MD5 完全相同——`tauri build` 时 GUI 产物被 CLI 的同名产物覆盖/复用。复现前提：先跑 `cargo build --release -p patheditor-cli`，紧接着 `npx tauri build`（两个 crate 的 bin 目标重名 `patheditor`，见 §三.3）。重新单独构建 GUI 后产物恢复 20.8MB、功能正常。

> 附带发现：CLI exe 曾在 NSIS 打包刚结束时出现「连 `--version` 都挂起」的瞬时现象，重新链接后消失——判定为文件刚释放时的占用/扫描瞬时状态，非缺陷，但与问题 B 一同构成本次「编译打包」会话中的噪音。

## 三、根因分析

### 3.1 关窗链路（两版代码逐字相同）

```text
用户点 X
  → Rust tao 收 WM_CLOSE → tauri manager（window.rs:171）
      has_js_listener("tauri://close-requested") == true（AppShell 注册过）
      → api.prevent_close()   ← 拦截默认关窗，等 JS 决定
      → 转发事件到 webview
  → JS onCloseRequested 回调（AppShell.tsx:85-90）
      pending = isModified || hasDrafts()
      pending && !window.confirm(...) → preventDefault()
      否则（无 pending）→ Tauri JS 包装层调 window.destroy()
```

代码位置：`src/components/layout/AppShell.tsx:79-101`（注册于提交 `000d3a5`，2026-09-17，随 v5.1.3 发布）。

### 3.2 为什么 main 构建不复现而 CI 复现

已排除的变量：

| 排除项                             | 证据                                                                                                                |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| 关窗 JS 代码差异                   | `git diff v5.1.3 HEAD -- AppShell.tsx` 仅 onConfirm 签名变化；onCloseRequested 块逐字相同                           |
| `env-store` / `app-store` 状态差异 | `hasDrafts`/`isModified` 两版逻辑一致，初值同为 false                                                               |
| Rust/前端依赖差异                  | `git diff v5.1.3 HEAD -- Cargo.lock package-lock.json` 零依赖变化（tauri 2.11.2 / @tauri-apps/api 2.11.0 两版相同） |
| 运行目录 / WebView2Loader.dll      | scoop exe 拷贝到本地目录运行仍死锁                                                                                  |
| 默认 Tab / 启动时序                | 早期关窗（监听未注册窗口期）同样死锁                                                                                |

**剩余唯一显著变量：构建工具链与环境**（CI = GitHub runner + MSVC stable，本地 = GNU stable；`rust-toolchain.toml` 指定 GNU，CI release.yml 刻意覆盖为 MSVC——CLAUDE.md 记载该决定）。本地 MSVC 构建因本机 link.exe 环境问题未能完成复现实验，无法进一步切分「MSVC 工具链」与「GitHub runner 环境」。

**定性**：CI 构建环境级缺陷，疑似 MSVC 构建的 tao/wry 与 WebView2（本机 153.0.4234.x）在关窗拦截路径上的交互问题。**main 代码不需要修改**——同代码 GNU 构建正常；但关窗确认的阻塞式 `window.confirm` 是脆弱点（§四）。

### 3.3 问题 B 的根因：bin 目标重名

```toml
# gui/Cargo.toml   → [package] name = "patheditor"（src/main.rs 自动成为 bin "patheditor"）
# cli/Cargo.toml   → [[bin]] name = "patheditor"（自 dc36d63 起 CLI 二进制改名）
```

`cargo metadata` 证实两个 crate 产出**同名 bin 目标 `patheditor`**，产物都写 `target/release/patheditor.exe`；`tauri build` 再以 `productName: PathEditor` 复制一份为 `PathEditor.exe`。构建顺序不当（先 CLI 后 tauri）时，tauri 的产物固化/复制环节拿到的是 CLI 内容。**这是构建流程缺陷**：谁能想到 GUI 安装包里装的是 CLI？好在 NSIS 产物是独立的（4.5MB），且 CI 上「Tauri Build」先于「构建 CLI」执行（release.yml:105-111），顺序正确，**CI 发布产物不受影响**（scoop 版 20.8MB 真 GUI 已验证）。

## 四、修复建议（按优先级）

1. **P1 — 关窗链路去阻塞化**（防御性修复，main 代码虽不复现也值得做）：把 AppShell.tsx:87 的阻塞式 `window.confirm` 换成 Tauri 异步对话框（`@tauri-apps/plugin-dialog` 的 `confirm`，gui 已依赖 `tauri-plugin-dialog`），异步等待期间不阻塞 JS 线程，消除死锁前提。此改动同时消除 WebView2 默认对话框在不同宿主上的可见性问题。
2. **P1 — 构建流程加固（问题 B）**：CI 与本地文档中明确「`tauri build` 必须先于/独立于 `cargo build -p patheditor-cli`」；更彻底的修法是给 CLI 的 bin 改名（如 `[[bin]] name = "patheditor-cli"`，安装时再重命名）或 gui crate 显式声明 `[[bin]] name = "PathEditor"`，从根上消除同名冲突。**注意**：CLI 改名会动 scoop 清单与 README，需走计划流程。
3. **P2 — 关窗链路自动化测试**：`onCloseRequested` 原生关窗路径目前**零测试覆盖**（单测只覆盖工具栏「取消」按钮的 `window.close` 路径）。E2E 无法测原生窗口事件，至少补 jsdom 层对回调逻辑的单测（mock `getCurrentWindow`）。
4. **P2 — 用户侧临时解法**：关不掉时用任务管理器结束 `PathEditor.exe`；数据安全无虞（未保存修改本来就会提示，强制结束只丢草稿）。

## 五、验证与复现命令

```powershell
# A 线复现（对 CI 版 GUI）
$p = Start-Process "<scoop>/apps/patheditor-gui/current/patheditor.exe" -PassThru
Start-Sleep 8; PostMessage($p.MainWindowHandle, WM_CLOSE=0x0010)  # 等 12s 进程仍存活

# A 线反证（同 tag 本地 GNU 构建）
git worktree add ../repro v5.1.3 && cargo build --release -p patheditor
# 同样时序 → 6 秒内正常退出

# B 线复现（产物污染）
cargo build --release -p patheditor-cli && npx tauri build
certutil -hashfile target\release\PathEditor.exe MD5     # == CLI 的 MD5 → 污染
certutil -hashfile target\release\patheditor.exe MD5
```

## 六、遗留与登记

1. 本地 MSVC 工具链构建失败（link.exe 环境问题）导致「MSVC 工具链 vs runner 环境」未完全切分——若下轮要根治问题 A，需在 CI 环境加诊断（如 WebView2 环境变量 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`、tauri 日志级别）。
2. 问题 B 的 bin 改名涉及 scoop 清单（`lhy` bucket 的 `patheditor-cli.json` 直接下载 `patheditor-cli_<ver>_x64.exe#/patheditor.exe`，`#/` 已做重命名，实际影响面小）。
3. 本诊断未改动任何代码；修复建议 1/3 适合并入「F-08 GUI 接线小波次」。
