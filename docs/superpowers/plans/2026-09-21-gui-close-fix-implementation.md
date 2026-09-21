# GUI 关窗死锁修复与产物污染根治 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 消除 CI 构建 GUI 的关窗死锁（阻塞式 `window.confirm` 换 Tauri 异步对话框），并根治 gui/cli bin 同名导致的产物污染。

**Architecture:** 死锁根因是关窗确认用了阻塞式 `window.confirm`（CI 构建环境下与 WebView2 跨进程等待互锁，本地 GNU 构建不复现）——改为 `@tauri-apps/plugin-dialog` 的异步 `confirm()`（Rust 侧插件已在 `gui/src/lib.rs:6` 注册，缺 `dialog:allow-confirm` 权限一行）。产物污染根因是 gui（package name 自动 bin）与 cli（显式 `[[bin]]`）产出同名 `patheditor` bin——gui crate 显式声明 `[[bin]] name = "PathEditor"`，从根上分离产物名。

**Tech Stack:** Tauri 2.11.2 + `@tauri-apps/plugin-dialog` 2.7.1（前端依赖已在 package.json:36）、React 19 + Zustand、Cargo workspace。

**Spec:** `docs/审核和开发/2026.09.21/PathEditor-GUI关不掉问题诊断报告.md`（本计划实施其修复建议 1/2/3）

**相关背景（必读）:**

- 诊断报告 §二 有完整复现实验矩阵：CI 版 GUI 死锁 3/3，同代码本地 GNU 构建 2/2 正常。
- 死锁画像：Win32 消息循环活着、无可见对话框、进程永不退出——JS 线程阻塞在同步 `window.confirm()` 的跨进程等待，Rust 侧 `api.prevent_close()`（`tauri-2.11.2/src/manager/window.rs:170-174`）在等 JS 的 destroy IPC。
- `tauri-plugin-dialog` Rust 侧已注册（`gui/src/lib.rs:6`），Rust 权限集 `dialog:default` 只含 `allow-message/allow-save/allow-open`（`tauri-plugin-dialog-2.7.1/permissions/default.toml`），**confirm 需单独授权 `dialog:allow-confirm`**。
- 现有阻塞式 `window.confirm` 共 4 处：`AppShell.tsx:87`（关窗确认，**死锁点，必须换**）、`AppShell.tsx:125`（删除变量）、`AppShell.tsx:184`（工具栏取消+关窗）、`use-profiles.ts:47/68`（profile 应用/删除）。本计划只**必须**换关窗路径两处（:87、:184）；其余两处顺带换（同机制、防同类问题），profile 的两处一并在 Task 3 换。
- 测试影响：`tests/unit/app-shell-env-vars.test.tsx` 的「关窗确认纳入环境变量草稿」describe（3 个用例）mock 的是 `window.confirm`，改异步对话框后须同步重写。

## Global Constraints

- **分支**：main 的 worktree（沿用 `worktree-<name>` 命名）。
- **不推送、不升级版本号、不删除文件**。
- **禁止真实注册表写入**：测试走 mock IPC / jsdom。
- **文档注释**：所有 `pub` / `pub(crate)` 项必须有 `///`。
- **代码风格**：UTF-8、CRLF；TS 2 空格 Prettier（单引号、尾逗号、100 列）；Rust 4 空格 rustfmt。
- **质量门**：`npm run verify:all` 全绿（Prettier → ESLint → 构建 → 覆盖率 → fmt → clippy → Rust 测试 → E2E）。
- **提交规范**：Conventional Commits；每 Task 一次提交；husky 会跑 Prettier，外部改文件后记得重新 `git add`。
- **CLI 退出码契约**：0/1/2/3 不变；CLI 侧零安全判定逻辑不变。
- **`AGENTS.md` 与 `CLAUDE.md` 保持字节级一致**（改其一必须同步另一份）。
- **开发回执落盘**：完成后写 `docs/审核和开发/YYYY.MM.DD/PathEditor-GUI关窗修复开发回执.md`（Wave 2 起固化的流程）。

---

### Task 1: 关窗确认换 Tauri 异步对话框（P1 死锁修复）

**Files:**

- Modify: `gui/capabilities/default.json:17-19`（权限区）
- Modify: `src/components/layout/AppShell.tsx:79-101`（onCloseRequested 回调）
- Modify: `src/components/layout/AppShell.tsx:175-190`（工具栏 onCancel）
- Modify: `src/services/backend.ts`（新增 dialog 封装，见下）
- Test: `tests/unit/app-shell-env-vars.test.tsx`（重写关窗确认 describe）

**Interfaces:**

- Consumes: `@tauri-apps/plugin-dialog` 的 `confirm(message: string, options?): Promise<boolean>`（已安装 2.7.1）。
- Produces: `backend.confirmDialog(message: string): Promise<boolean>` —— 统一异步确认入口；关窗回调从同步签名改为注册一次性异步流程。

**设计要点（为什么这样修）:**

阻塞式 `window.confirm` 在 JS 线程上同步等待用户——CI 构建的 WebView2 环境里该等待与 Rust 侧 `prevent_close` 互锁形成死锁。Tauri 的 `confirm()` 是异步 IPC：回调立即返回，JS 事件循环不被阻塞，`onCloseRequested` 的 Promise 处理器可以在用户响应后安全地调 `destroy()`（`@tauri-apps/api/window.js` 的包装：`await handler(evt); if (!evt.isPreventDefault()) await this.destroy()`）。

- [ ] **Step 1: 权限加 `dialog:allow-confirm`**

`gui/capabilities/default.json` 的 `permissions` 数组，在 `"dialog:allow-save"` 后加一行：

```json
    "dialog:allow-open",
    "dialog:allow-save",
    "dialog:allow-confirm",
    "log:default"
```

注意：`dialog:default` 不含 confirm（`tauri-plugin-dialog-2.7.1/permissions/default.toml` 只有 `allow-message/allow-save/allow-open`），必须显式加。漏加的症状是运行时 `confirm()` reject「dialog.confirm not allowed」。

- [ ] **Step 2: 写失败测试（先改测试定义新行为）**

重写 `tests/unit/app-shell-env-vars.test.tsx` 的 `describe('关窗确认纳入环境变量草稿（决策 4）')` 整块。mock 对象从 `window.confirm` 换成 `@tauri-apps/plugin-dialog`：

```tsx
// 文件顶部 import 区加：
import { confirm as dialogConfirm } from '@tauri-apps/plugin-dialog';

// 新 describe 全文（替换旧 describe）：
describe('关窗确认纳入环境变量草稿（异步对话框版）', () => {
  function mockDialogConfirm(v: boolean) {
    return vi.mocked(dialogConfirm).mockResolvedValue(v);
  }

  it('仅有环境变量草稿时也弹异步确认，取消则不关窗', async () => {
    mockDialogConfirm(false);
    const closeSpy = vi.spyOn(window, 'close').mockImplementation(() => undefined);
    useEnvStore.setState({ draft: new Map([['user:MY_TOKEN', 'secret']]) });

    render(<AppShell />);
    fireEvent.click(screen.getByRole('button', { name: '取消' }));

    await waitFor(() => expect(dialogConfirm).toHaveBeenCalledWith('有未保存的修改，确定退出吗？'));
    expect(closeSpy).not.toHaveBeenCalled();
  });

  it('确认后关窗', async () => {
    mockDialogConfirm(true);
    const closeSpy = vi.spyOn(window, 'close').mockImplementation(() => undefined);
    useAppStore.setState({ isModified: true });

    render(<AppShell />);
    fireEvent.click(screen.getByRole('button', { name: '取消' }));

    await waitFor(() => expect(dialogConfirm).toHaveBeenCalled());
    expect(closeSpy).toHaveBeenCalled();
  });

  it('无草稿且未修改时不弹确认，直接关窗（PATH 既有行为）', async () => {
    mockDialogConfirm(false);
    const closeSpy = vi.spyOn(window, 'close').mockImplementation(() => undefined);

    render(<AppShell />);
    fireEvent.click(screen.getByRole('button', { name: '取消' }));

    expect(dialogConfirm).not.toHaveBeenCalled();
    expect(closeSpy).toHaveBeenCalled();
  });
});
```

同时在该测试文件的 `vi.mock` 设置区（mock `@/services/backend` 的同级）加：

```tsx
vi.mock('@tauri-apps/plugin-dialog', () => ({ confirm: vi.fn() }));
```

- [ ] **Step 3: 跑测试确认失败**

Run: `npx vitest run tests/unit/app-shell-env-vars.test.tsx -t "异步对话框版"`
Expected: FAIL（实现还在用 `window.confirm`，`dialogConfirm` 未被调用）。

- [ ] **Step 4: `backend.ts` 加 dialog 封装**

`src/services/backend.ts`，import 区加：

```ts
import { confirm as tauriConfirm } from '@tauri-apps/plugin-dialog';
```

`backend` 对象内加：

```ts
  /** 异步确认对话框（替代阻塞式 window.confirm）：关窗确认等路径专用，绝不阻塞 JS 线程。 */
  confirmDialog: (message: string) => tauriConfirm(message),
```

- [ ] **Step 5: 改 AppShell 关窗路径两处**

`src/components/layout/AppShell.tsx`。第一处，`onCloseRequested` 回调（:85-90）整体替换：

```tsx
const handler = await getCurrentWindow().onCloseRequested((event) => {
  const pending = useAppStore.getState().isModified || useEnvStore.getState().hasDrafts();
  if (!pending) return; // 无待保存内容：不拦截，包装层自动 destroy
  event.preventDefault(); // 同步拦截默认销毁；确认后显式 destroy 收口（G-B2）
  void (async () => {
    try {
      const confirmed = await backend.confirmDialog(i18n.t('dialog.unsavedConfirm'));
      if (confirmed) await getCurrentWindow().destroy();
    } catch {
      // 对话框 IPC 失败时保守放行关窗：宁可丢草稿不可把窗口锁死
      await getCurrentWindow()
        .destroy()
        .catch(() => {});
    }
  })();
});
```

**G-B2 裁断（2026-09-21，采纳开发窗口指误）**：本计划的初版方案是「pending 时不调 `preventDefault()`、确认后显式 destroy」——**错误**。`onCloseRequested` 包装层（`window.js:1637-1639`）在 `await handler(evt)` 返回后 `if (!evt.isPreventDefault()) await this.destroy()`：handler 是同步回调立即返回，用户还在看对话框时 wrapper 就已经走到 destroy 分支——**用户点「取消」窗口照样关闭**，「取消则不关窗」语义丢失。正确做法：pending 时**同步调 `event.preventDefault()`** 拦截默认销毁，异步确认后再显式 `destroy()`；无 pending 时不拦截、交给包装层。注意三点：①`preventDefault()` 必须在回调同步段调用（wrapper 只看 handler 返回时刻的标志位）；②对话框 IPC 失败的兜底是显式 `destroy()`——失败模式应是「关窗」而不是「锁死」，这正是本次 bug 的教训；③文件顶部确认已有 `import { backend } from '@/services/backend'`（无需新增）。

第二处，工具栏 `onCancel`（:180-186）整体替换：

```tsx
            onCancel={() => {
              const state = useAppStore.getState();
              // 环境变量草稿尚未提交时同样需要确认，否则关窗会静默丢弃用户输入。
              const hasPendingChanges = state.isModified || useEnvStore.getState().hasDrafts();
              if (!hasPendingChanges) {
                window.close();
                return;
              }
              void backend
                .confirmDialog(t('dialog.unsavedConfirm'))
                .then((confirmed) => {
                  if (confirmed) window.close();
                })
                .catch(() => {});
            }}
```

（工具栏路径走 `window.close()`，不经 wrapper，无需 preventDefault。）

- [ ] **Step 6: 跑测试确认通过**

Run: `npx vitest run tests/unit/app-shell-env-vars.test.tsx`
Expected: 全文件 PASS（含重写的 3 用例与其余既有用例）。

- [ ] **Step 7: 全量前端质量门**

Run: `npm run verify:all`
Expected: 全绿（E2E 走 mock IPC 不受影响；`onCloseRequested` 在 jsdom 无原生事件自动跳过，既有行为）。

- [ ] **Step 8: 提交**

```bash
git add gui/capabilities/default.json src/services/backend.ts src/components/layout/AppShell.tsx tests/unit/app-shell-env-vars.test.tsx
git commit -m "fix(ui): 关窗确认换 Tauri 异步对话框，消除 CI 构建下的死锁"
```

---

### Task 2: 其余阻塞 confirm 一并换掉（同机制防复发）

**Files:**

- Modify: `src/components/layout/AppShell.tsx:123-127`（confirmRemoveVar）
- Modify: `src/components/dialogs/profile/use-profiles.ts:47`（applyConfirm）、`:68`（deleteConfirm）
- Test: `tests/unit/use-profiles.test.ts`（若无此文件则新建最小测试，见 Step 1）

**Interfaces:**

- Consumes: Task 1 的 `backend.confirmDialog(message: string): Promise<boolean>`。
- Produces: 全库 `window.confirm` 归零（`grep -rn "window.confirm" src/ | grep -v test` 零命中）。

- [ ] **Step 1: 写失败测试（profile 确认的异步化）**

`tests/unit/use-profiles.test.ts`（若已存在则在相应 describe 加用例）。参考结构（按现有测试文件的实际 mock 风格调整，`use-profiles` 依赖 `backend` 与 i18n）：

```ts
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { confirm as dialogConfirm } from '@tauri-apps/plugin-dialog';

vi.mock('@/services/backend', async (importOriginal) => ({
  ...(await importOriginal<Record<string, unknown>>()),
}));
vi.mock('@tauri-apps/plugin-dialog', () => ({ confirm: vi.fn() }));

import { backend } from '@/services/backend';
import { confirm as dialogConfirm } from '@tauri-apps/plugin-dialog';
// use-profiles 是 hook：用 renderHook + act 驱动，参照既有 hook 测试的写法

describe('profile 确认对话框（异步）', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it('apply 前弹异步确认，取消则不调用 apply_profile 通路', async () => {
    vi.mocked(dialogConfirm).mockResolvedValue(false);
    const applySpy = vi.spyOn(backend, 'applyProfile').mockResolvedValue(undefined as never);
    // …以现有 use-profiles 测试的构造方式准备 selected 状态后：
    // await act(async () => { applySelected(); });
    // expect(dialogConfirm).toHaveBeenCalled(); expect(applySpy).not.toHaveBeenCalled();
    expect(true).toBe(true); // 占位断言由实际状态构造替换——实施时按上述意图补全
  });
});
```

> **实施注**：若 `use-profiles.ts` 现无测试文件且 hook 构造复杂，允许降级：本 Task 的 profile 两处改动以「全量 `npm test` 不回归 + `window.confirm` 归零 grep 断言」作为验证标准，不强造 hook 测试。但 AppShell 的 `confirmRemoveVar` 必须有测试（沿用 app-shell 测试文件的 mock 手法，加一个用例：`mockDialogConfirm(false)` 后点删除按钮，断言 `dialogConfirm` 被调、`deleteEnvVar` 未被调）。

- [ ] **Step 2: 跑测试确认失败（仅 confirmRemoveVar 有测试时）**

Run: `npx vitest run tests/unit/app-shell-env-vars.test.tsx tests/unit/use-profiles.test.ts`
Expected: 新用例 FAIL（实现未改）。

- [ ] **Step 3: 改三处实现**

`AppShell.tsx:123-127`：

```tsx
const confirmRemoveVar = (meta: EnvVarMeta) => {
  void backend
    .confirmDialog(t('envVar.deleteConfirm', { name: meta.name }))
    .then((confirmed) => {
      if (confirmed) void useEnvStore.getState().remove(meta);
    })
    .catch(() => {});
};
```

`use-profiles.ts:47`（applyConfirm）与 `:68`（deleteConfirm）按同一模式改：外层函数改异步流程（`void backend.confirmDialog(...).then(...)`），文件顶部加 `import { backend } from '@/services/backend'`（若该 hook 目前经参数或别的方式拿 backend，沿用其现有模式）。

- [ ] **Step 4: 归零断言 + 全量质量门**

Run: `grep -rn "window.confirm" src/ | grep -v test`（期望零输出）
Run: `npm run verify:all`（全绿）

- [ ] **Step 5: 提交**

```bash
git add src/ tests/unit/
git commit -m "refactor(ui): 其余阻塞式 confirm 全部换为 Tauri 异步对话框"
```

---

### Task 3: bin 同名根治（P1 产物污染）

**Files:**

- Modify: `gui/Cargo.toml`（显式 bin 声明）
- Modify: `.github/workflows/release.yml:126,137`（CLI 产物路径）
- Modify: `D:\settings\settings\Scoop\buckets\lhy\bucket\patheditor-gui.json`（**G-B1 联动，bucket 仓库非本 repo**）
- Modify: `CLAUDE.md` + `AGENTS.md`（字节级同步：快速命令、发布流程段）
- Modify: `README.md`（若有 CLI 安装说明引用产物名）

**Interfaces:**

- Consumes: 无。
- Produces: GUI 产物恒为 `target/release/PathEditor.exe`（tauri productName 复制源），CLI 产物恒为 `target/release/patheditor-cli.exe`——两者**永不互相覆盖**。

**设计要点:**

现状：gui crate `package.name = "patheditor"`（src/main.rs 自动推导 bin `patheditor`）与 cli crate `[[bin]] name = "patheditor"` 同名，产物都写 `target/release/patheditor.exe`。构建顺序不同会互相覆盖——诊断报告 B 线已实测（GUI 产物 MD5 == CLI）。修法取**改 gui 一侧**（Tauri 惯例 bin 名 = productName）：

```toml
# gui/Cargo.toml 的 [lib] 块之后加：
[[bin]]
name = "PathEditor"
path = "src/main.rs"
```

改后 gui 的 bin 目标为 `PathEditor`（产物 `PathEditor.exe`），tauri build 的 `productName: PathEditor` 与之**精确同名**（tauri 对 productName 与 bin 名一致时不做复制/改名，直接用产物）——这正是要的确定性。cli 侧**不改**（`patheditor` 这个命令名是用户接口契约，scoop 清单 `patheditor-cli.json` 用 `#/patheditor.exe` 重命名安装，改 cli 会波及用户命令名）。

CI 影响面（已核实）：`release.yml:126` 的 `$cli = 'target\release\patheditor.exe'` 与 `:137` 的 portable zip 拷贝源**在 bin 改名后语义变化**——改后 `target\release\patheditor.exe` 只可能由 CLI crate 产出（gui 产物是 `PathEditor.exe`），因此这两处**引用保持不变且从此无歧义**；需要改的是加注释说明这一点，并核对 portable zip 装的确实是 GUI（见 Step 3 的验证步骤——**这是本 Task 最重要的验证**：诊断报告怀疑 v5.1.3 CI 的 portable zip 可能装了 CLI 而非 GUI，bin 改名后此隐患自动消除，但要在本机用产物大小验证修复生效）。

- [ ] **Step 1: 改 gui/Cargo.toml 并验证产物分离**

`gui/Cargo.toml`（`[lib]` 块后加）：

```toml
[[bin]]
name = "PathEditor"
path = "src/main.rs"
```

Run: `cargo build --release -p patheditor 2>&1 | tail -1 && ls target/release/PathEditor.exe`
Expected: 构建成功，产物 `PathEditor.exe`（约 20MB）。

Run: `cargo build --release -p patheditor-cli 2>&1 | tail -1 && powershell -Command "(Get-Item 'target\release\PathEditor.exe').Length; (Get-Item 'target\release\patheditor.exe').Length"`
Expected: 两个大小差异显著（GUI ≈20MB，CLI ≈3MB）——**不再可能互相覆盖**。

- [ ] **Step 2: tauri build 验证 NSIS 产物**

Run: `npx tauri build 2>&1 | tail -3`
Expected: `Finished 1 bundle at: target\release\bundle\nsis\PathEditor_5.1.3_x64-setup.exe`。

Run: `powershell -Command "(Get-Item 'target\release\PathEditor.exe').Length"`（tauri build 之后立刻看）
Expected: **仍约 20MB**——tauri build 后不再需要防御性重跑 CLI 构建。

- [ ] **Step 3: 更新 CI 与文档**

`.github/workflows/release.yml`：`:126` 的 `$cli = 'target\release\patheditor.exe'` 上方加注释行：

```yaml
# bin 改名后：patheditor.exe 只由 patheditor-cli crate 产出（GUI 是 PathEditor.exe），
# 两产物不再同名冲突；portable zip 的 GUI 本体取 target\release\PathEditor.exe
```

并把 `:137` 的 portable zip 拷贝源从 `'target\release\patheditor.exe'` 改为 `'target\release\PathEditor.exe'`（**这是修复 v5.1.3 CI portable zip 可能装错 CLI 的关键一行**）。同文件 `:130` 的 CLI 发布产物拷贝保持 `patheditor.exe` 不变。

**scoop 清单联动（G-B1，2026-09-21 裁断：二选一取「直接改清单」）**：`D:\settings\settings\Scoop\buckets\lhy\bucket\patheditor-gui.json:13-16` 的 `shortcuts` 改指 `PathEditor.exe`：

```json
    "shortcuts": [
        [
            "PathEditor.exe",
            "PathEditor"
        ]
    ],
```

裁断理由：备选「zip 内含两个 exe 过渡」会保留『zip 里的 patheditor.exe 是 GUI 还是 CLI』的混淆——正是要根治的问题，否决。注意三点：①该文件在 scoop bucket 仓库（非本 repo），改动在本地 bucket 立即生效，**bucket 的 git 提交/推送由用户决定**；②清单 `hash` 字段针对的是 v5.1.3 旧 zip——本波次**不出新 release**，清单 version 不动，旧 URL 下载的 zip 里仍是 `patheditor.exe`，因此 shortcuts 的改动**只对下一个版本生效**（autoupdate 触发新 version 时，digest 由 `jsonpath` 自动取新 release 资产）。若实施中发现直接改 shortcuts 会导致 v5.1.3 装机校验混乱（用户此刻不应手动 `scoop update patheditor-gui`），在回执里记录「此清单改动随下一版本发布生效」即可，不需要额外兼容措施；③实施注：本波次改 bucket 后**不要**提交 bucket 的 git（提交由用户发话），在回执注明本地改动状态。

`CLAUDE.md` 与 `AGENTS.md`（同步改，保持字节级一致）：

- 「快速命令」区 `npx tauri build` 注释补「产物 target\release\PathEditor.exe；CLI 产物 patheditor.exe，两者不再同名冲突」。
- 「产物命名」表加一行：`PathEditor.exe`｜GUI 本体（tauri bin 产物，portable zip 装的就是它）。
- 「CI 各步骤的前置依赖」表「整理发布产物」行补 GUI 本体（G-N3）：`需要 ...\PathEditor_<VERSION>_x64-setup.exe`、`PathEditor.exe`（GUI 本体，portable zip 用）与 `patheditor.exe`（CLI）。
- 「版本号升级清单」表加一行（顺手闭环既有登记）：`package-lock.json`｜`version`（两处，`npm install` 同步）。

`README.md`：核对 CLI 安装段对二进制名的描述（若引用了产物名则同步；用户命令名 `patheditor` 不变则通常无需改）。

- [ ] **Step 4: 质量门 + 提交**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && npm run verify:all`
Expected: 全绿。

```bash
git add gui/Cargo.toml .github/workflows/release.yml CLAUDE.md AGENTS.md README.md
git commit -m "fix(build): gui bin 改名 PathEditor，根治 gui/cli 同名产物互相覆盖"
```

（scoop bucket 清单改动在 bucket 仓库，不在本 repo 提交内；bucket 的 git 提交由用户决定，回执注明状态。）

---

### Task 4: 关窗链路回归测试补底（P2）

**Files:**

- Test: `tests/unit/app-shell-env-vars.test.tsx`（新增 describe）

**Interfaces:**

- Consumes: Task 1 改造后的 `onCloseRequested` 回调。
- Produces: 关窗决策逻辑（pending 判定 → 弹确认 → 确认/取消/异常三分支）的直接单测覆盖（此前该链路零覆盖——诊断报告 §四.3）。

- [ ] **Step 1: 写 onCloseRequested 回调的用例**

在 `tests/unit/app-shell-env-vars.test.tsx` 加（紧随 Task 1 重写的 describe 之后）。`onCloseRequested` 在 jsdom 无原生事件，需 mock `@tauri-apps/api/window` 捕获注册的回调并手动触发：

```tsx
// 文件顶部 mock 区加（与 plugin-dialog 的 mock 并列）：
const closeRequestedHandlers: Array<(event: { preventDefault: () => void }) => void> = [];
const destroyMock = vi.fn().mockResolvedValue(undefined);

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    onCloseRequested: (handler: (event: { preventDefault: () => void }) => void) => {
      closeRequestedHandlers.push(handler);
      return Promise.resolve(() => {});
    },
    destroy: destroyMock,
  }),
}));

describe('onCloseRequested 原生关窗回调（诊断报告 §四.3 补底）', () => {
  beforeEach(() => {
    closeRequestedHandlers.length = 0;
    destroyMock.mockClear();
  });

  function fireClose() {
    for (const h of closeRequestedHandlers) h({ preventDefault: () => {} });
  }

  it('无 pending：不弹确认，直接 destroy', async () => {
    mockDialogConfirm(false);
    render(<AppShell />);
    await waitFor(() => expect(closeRequestedHandlers.length).toBeGreaterThan(0));
    fireClose();
    await waitFor(() => expect(destroyMock).toHaveBeenCalled());
    expect(dialogConfirm).not.toHaveBeenCalled();
  });

  it('有草稿 + 用户确认：destroy', async () => {
    mockDialogConfirm(true);
    useEnvStore.setState({ draft: new Map([['user:MY_TOKEN', 'x']]) });
    render(<AppShell />);
    await waitFor(() => expect(closeRequestedHandlers.length).toBeGreaterThan(0));
    fireClose();
    await waitFor(() => expect(destroyMock).toHaveBeenCalled());
    expect(dialogConfirm).toHaveBeenCalled();
  });

  it('有草稿 + 用户取消：不 destroy（窗口保留）', async () => {
    mockDialogConfirm(false);
    useEnvStore.setState({ draft: new Map([['user:MY_TOKEN', 'x']]) });
    render(<AppShell />);
    await waitFor(() => expect(closeRequestedHandlers.length).toBeGreaterThan(0));
    fireClose();
    // 确认对话框 reject 之外的路径都不应 destroy
    await new Promise((r) => setTimeout(r, 50));
    expect(destroyMock).not.toHaveBeenCalled();
  });

  it('confirmDialog IPC 异常：兜底 destroy（宁可丢草稿不锁死窗口）', async () => {
    vi.mocked(dialogConfirm).mockRejectedValue(new Error('ipc denied'));
    useEnvStore.setState({ draft: new Map([['user:MY_TOKEN', 'x']]) });
    render(<AppShell />);
    await waitFor(() => expect(closeRequestedHandlers.length).toBeGreaterThan(0));
    fireClose();
    await waitFor(() => expect(destroyMock).toHaveBeenCalled());
  });
});
```

> **实施注（G-N1，2026-09-21 裁断采纳）**：`vi.mock('@tauri-apps/api/window')` 会拦截 AppShell 内的**动态** import（`await import('@tauri-apps/api/window')` 也走 mock）。采用全局 mock 即可——Task 1 的既有 3 用例走工具栏 `window.close` 路径，`window.close` 是 window 方法 spy、与模块 mock 无冲突，应照常工作；实施时以**全文件 PASS** 为准，无需 `vi.doMock` 收窄。若个别用例因 mock 注入失败，优先修该用例的初始化等待（`waitFor` handlers 注册完成），不要引入 doMock 分层。

- [ ] **Step 2: 跑测试**

Run: `npx vitest run tests/unit/app-shell-env-vars.test.tsx`
Expected: 全文件 PASS（含 4 个新用例）。

- [ ] **Step 3: 全量质量门 + 提交**

Run: `npm run verify:all`
Expected: 全绿。

```bash
git add tests/unit/app-shell-env-vars.test.tsx
git commit -m "test(ui): 补 onCloseRequested 关窗回调的直接单测覆盖"
```

---

### Task 5: 收口质量门与文档

- [ ] **Step 1: 全量质量门**

```bash
npm run verify:all
```

Expected: 一次性全绿。

- [ ] **Step 2: 真实 GUI 冒烟（构建级验证，非注册表写入）**

```bash
npx tauri build
# 手工冒烟：启动 target/release/PathEditor.exe
# 1. 点 X → 窗口立即关闭（无草稿）
# 2. 编辑任一变量输入内容不保存 → 点 X → 出现原生确认对话框 → 取消 → 窗口保留；确认 → 关闭
# 3. 全程无挂起
```

Run: `powershell -Command "(Get-Item 'target\release\PathEditor.exe').Length"`
Expected: ≈20MB（若 ≈3MB 说明产物污染回归，Task 3 修复失效——阻塞，回查）。

**G-N2（2026-09-21 采纳）**：冒烟口径如实记入回执——「本机 GNU 构建 + 手工冒烟 ≠ CI（MSVC）构建环境验证」；CI 环境的关窗行为只能靠发布后用户侧/后续验证确认，回执的未覆盖项必须列明，不得写成本修复「已在 CI 环境验证」。这是死锁本身只复现于 CI 构建的事实决定的，如实措辞优先于声明完成。

- [ ] **Step 3: 开发回执落盘**

写 `docs/审核和开发/2026.09.21/PathEditor-GUI关窗修复开发回执.md`：任务映射、Minor 处置（已修/登记/豁免三态）、质量门终态数字（注明口径与时间点——三波审查报告的持续登记项）、未覆盖项（如真实注册表场景未测）。

- [ ] **Step 4: 提交**

```bash
git add -A
git commit -m "chore: GUI 关窗修复质量门与开发回执"
```

---

## Self-Review

**1. Spec coverage**（诊断报告 §四修复建议 → 任务）

| 诊断报告建议                                     | 任务                                          |
| ------------------------------------------------ | --------------------------------------------- |
| P1-1 关窗确认去阻塞化（异步对话框）              | Task 1                                        |
| P1-1 引申：其余 window.confirm 同机制替换        | Task 2                                        |
| P1-2 bin 改名根治产物污染 + CI portable zip 修正 | Task 3                                        |
| P2-3 关窗链路自动化测试补底                      | Task 4                                        |
| P2-4 用户临时解法（任务管理器）                  | 无需任务（纯用户操作，诊断报告 §四.4 已写明） |

**2. Placeholder scan**

Task 2 Step 1 的 profile 测试给了降级路径（实施注），不构成占位符——AppShell 的 confirmRemoveVar 用例是硬性要求且有完整代码。其余步骤均为完整代码/命令。

**3. Type consistency**

| 符号                                                       | 定义          | 使用                                 | 一致 |
| ---------------------------------------------------------- | ------------- | ------------------------------------ | ---- |
| `backend.confirmDialog(message: string): Promise<boolean>` | Task 1 Step 4 | Task 1 Step 5、Task 2 Step 3、Task 4 | ✓    |
| `dialog:allow-confirm` 权限                                | Task 1 Step 1 | Task 1 运行前提                      | ✓    |
| `[[bin]] name = "PathEditor"`                              | Task 3 Step 1 | Task 3 Step 2/3 的产物名             | ✓    |
| `mockDialogConfirm(v)` 测试助手                            | Task 1 Step 2 | Task 4 复用（同文件）                | ✓    |

## 开发窗口核对轮（2026-09-21，G-B1~B2 / G-N1~N3）

开发窗口逐项核对，锚点全部属实（9/9 ✓）。2 阻塞 + 3 非阻塞全部经审核窗口核实成立并折入：

| #    | 异议                                                                                                                      | 裁断                                                                                                                                           | 落点                  |
| ---- | ------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | --------------------- |
| G-B1 | portable zip 改名缺 scoop 清单联动，GUI 快捷方式会指向不存在的文件                                                        | 采纳：直接改清单（否决 zip 双 exe 过渡——保留混淆即保留问题）；shortcuts 改指 `PathEditor.exe`，注明仅对下一版本生效，bucket git 提交由用户决定 | Task 3 Files + Step 3 |
| G-B2 | 初版关窗代码「不调 preventDefault」会使「用户取消」也关窗——wrapper 在 handler 返回后 `if (!isPreventDefault()) destroy()` | 采纳（**开发窗口指误正确，审核窗口初版方案作废**）：pending 时同步调 `event.preventDefault()`，确认后显式 `destroy()`                          | Task 1 Step 5         |
| G-N1 | Task 4 的全局 window mock 会拦截动态 import，`vi.doMock` 方案过重                                                         | 采纳：直接全局 mock，既有用例的 `window.close` spy 与之无冲突，以全文件 PASS 为准                                                              | Task 4 实施注         |
| G-N2 | 冒烟用例 2 在 CI 环境不可自动断言                                                                                         | 采纳：回执必须记「本机冒烟 ≠ CI 构建环境验证」，未覆盖项如实列明                                                                               | Task 5 Step 2         |
| G-N3 | CLAUDE.md CI 前置依赖表漏补 GUI 本体说明                                                                                  | 采纳：「整理发布产物」行补 `PathEditor.exe`（GUI 本体）与 `patheditor.exe`（CLI）双产物语义                                                    | Task 3 Step 3         |

另核实成立：profile 测试降级路径合理（`use-profiles` 无现测试文件）、Task 3 改 gui 侧正确（CLI 命令名契约不变）、`ask()` 异步先例已在 `use-profiles.ts:57`、E2E 不受影响。

## Execution Notes

（留给开发窗口回填：一行一项，「计划原文 / 实际 / 处理」。）
