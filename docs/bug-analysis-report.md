# PathEditor v4.2 代码审查 — 隐含 Bug 分析报告

> 审查日期：2026-05-28 | 审查范围：全部前端核心模块 + Rust 后端 + E2E 测试

---

## BUG 1 [HIGH] — undo/redo TOGGLE 后不持久化 disabled 状态

### 位置

`src/store/app-store.ts` — `togglePath()` (L213-237) vs `undo()` (L239-248) / `redo()` (L251-260)

### 现象

1. 用户勾选复选框禁用一个路径 → `togglePath` 立即调用 `invoke('save_disabled_state', ...)` 写入 `disabled.json`
2. 用户按 Ctrl+Z 撤销 → `undo()` 恢复内存中的 `enabled` 状态，但**不调用** `save_disabled_state`
3. `disabled.json` 中该路径仍标记为 disabled
4. 下次启动 `loadPaths` 时，从 `disabled.json` 读到的是旧数据，路径又被标记为 disabled

### 根因

`undo()` 和 `redo()` 中处理 TOGGLE 时，只更新了 Zustand store 中的 `sysPaths`/`userPaths`，没有像 `togglePath` 那样同步持久化 disabled 状态。

```typescript
// togglePath — 有持久化
invoke('save_disabled_state', { system: sysDisabled, user: usrDisabled }).catch(() => {});

// undo/redo — 没有持久化！
set({
  sysPaths: result[0], userPaths: result[1], selectedIndices: [],
  isModified: !(arraysEqual(result[0], _savedSys) && arraysEqual(result[1], _savedUser)),
});
```

### 影响

- 撤销 toggle 后刷新/重启应用，disabled 状态恢复为撤销前的值
- 数据和 UI 展示不一致

### 修复方向

在 `undo()` 和 `redo()` 中，检测当前/上一条记录是否为 TOGGLE 类型，如果是则同步调用 `save_disabled_state`。或者更通用的方案：在 `set()` 之后统一检查 disabled 状态是否有变化并持久化。

---

## BUG 2 [MEDIUM] — `expand_env_vars` 未检测缓冲区截断

### 位置

`src-tauri/src/commands/system.rs:40-58` — `expand_env_vars()`

### 现象

Windows API `ExpandEnvironmentStringsW` 的行为：

- 第 1 次调用（`lpDst = NULL, nSize = 0`）：返回所需缓冲区大小（TCHAR 数）
- 第 2 次调用（提供缓冲区）：若缓冲区足够，返回写入的 TCHAR 数（≤ nSize）；**若缓冲区不足，返回所需大小（> nSize），而非 0**

当前代码：

```rust
let required = unsafe {
    ExpandEnvironmentStringsW(wide_path.as_ptr(), std::ptr::null_mut(), 0)
};
// ...
let mut buffer: Vec<u16> = vec![0; required as usize];
let result = unsafe {
    ExpandEnvironmentStringsW(wide_path.as_ptr(), buffer.as_mut_ptr(), required)
};

if result == 0 {
    // 仅处理了 API 失败
    log::warn!("expand_env_vars: 展开失败, 返回原始路径: {path}");
    return path.to_string();
}
// result > 0 但可能 > required（截断！）— 未检测
```

### 触发条件（低概率但真实存在）

环境变量在两次 `ExpandEnvironmentStringsW` 调用之间被修改：
1. 第 1 次调用查询到 `%VAR%` 展开后为 50 字符，`required = 51`（含 null）
2. 在第 1 次和第 2 次调用之间，另一个进程修改了 `%VAR%` 使其变为 200 字符
3. 第 2 次调用时 51 大小的缓冲区不够，API 返回 201
4. `result = 201 > 51`，`result != 0`，代码未进入错误分支
5. 函数返回截断的不完整路径

### 修复方向

```rust
if result == 0 || result > required {
    log::warn!("expand_env_vars: 展开失败或缓冲区不足, 返回原始路径: {path}");
    return path.to_string();
}
```

---

## BUG 3 [MEDIUM] — E2E mock `load_disabled_state` 返回格式与 Rust 端不匹配

### 位置

- `e2e/mocks/ipc.ts:9`
- `src-tauri/src/commands/disabled.rs:44` — `load_disabled_state()`
- `src/store/app-store.ts:275` — `loadPaths()` 消费端

### 现象

**Rust 端返回类型**：`Result<(Vec<String>, Vec<String>), String>`

Tauri 将元组序列化为 JSON 数组：`[["disabled_sys_1"], ["disabled_usr_1"]]`

**Mock 返回**：
```javascript
case 'load_disabled_state': return { system: [], user: [] };
//                                    ^^^^^^^^^^^^^^^^^^^^^^^^ 这是一个对象！
```

**前端消费**：
```typescript
const result = await invoke<[string[], string[]]>('load_disabled_state');
sysDisabled = result[0];  // 从对象取 → undefined
usrDisabled = result[1];  // 从对象取 → undefined
new Set(sysDisabled);     // → TypeError: undefined is not iterable
```

try/catch 捕获了 TypeError，`sysDisabled`/`usrDisabled` 保持为初始空数组 `[]`。

### 影响

- 生产环境无影响（Rust 端正确返回数组）
- E2E 测试中**disabled state 加载路径完全未被测试**（被 try/catch 静默跳过）
- 如果将来 E2E 测试要覆盖 disabled state 的加载-合并逻辑，会得到错误结果

### 修复方向

```javascript
case 'load_disabled_state': return [];  // 返回空元组 [[], []]
// 或者如果要 mock 有禁用路径的场景:
case 'load_disabled_state': return [['C:\\disabled_path'], []];
```

---

## BUG 4 [LOW] — `savePaths` 双 hive 失败时丢失错误原因

### 位置

`src/store/app-store.ts:332-336`

### 现象

```typescript
const reason = (!sysOk && sysResult.status === 'rejected') ? String(sysResult.reason) :
               (!userOk && userResult.status === 'rejected') ? String(userResult.reason) : '';
const msg = sysOk ? '用户 PATH 保存失败' :
            userOk ? '系统 PATH 保存失败' :
            `保存失败: ${reason}`;
```

当**两个 hive 都保存失败**时：
- `sysOk` = false，跳过第 1 个三元分支
- `userOk` = false，跳过第 2 个三元分支
- `reason` 取的是 `sysResult.reason`（第 1 行），`userResult.reason` 被丢弃
- 最终消息：`保存失败: <仅系统 PATH 的错误原因>`

### 修复方向

```typescript
const sysErr = (!sysOk && sysResult.status === 'rejected') ? String(sysResult.reason) : '';
const usrErr = (!userOk && userResult.status === 'rejected') ? String(userResult.reason) : '';
const parts = [sysErr, usrErr].filter(Boolean);
const msg = parts.length > 0 ? `保存失败: ${parts.join('; ')}` : '保存失败';
```

或者更清晰地分别显示两个 hive 的状态。

---

## BUG 5 [LOW] — `handleImportSelect` 导入 both 产生两条 undo 记录

### 位置

`src/hooks/use-app-actions.ts:160-165`

### 现象

```typescript
const handleImportSelect = useCallback((target: 'system' | 'user' | 'both') => {
  const { system, user } = dialogs.importDialog;
  const flat = flattenImportResult({ system, user }, target);
  if (flat.system.length > 0)
    useAppStore.getState().replacePaths(TargetType.SYSTEM, flat.system.map(e => e.path));
  if (flat.user.length > 0)
    useAppStore.getState().replacePaths(TargetType.USER, flat.user.map(e => e.path));
  ...
}, ...);
```

当用户选择「导入到两者」时：
1. 调用 `replacePaths(TargetType.SYSTEM, ...)` → 创建一条 IMPORT undo 记录
2. 调用 `replacePaths(TargetType.USER, ...)` → 创建另一条 IMPORT undo 记录

用户按一次 Ctrl+Z 只能撤销一个 hive 的导入，需要按两次才能完全撤销。

### 修复方向

在 `app-store.ts` 中新增一个 `replaceBothPaths` 方法，将 system + user 的替换合并为一条 undo 记录，或者把 `replacePaths` 扩展为支持同时替换两个 hive。

---

## 非 Bug 问题（值得关注但暂不紧急）

| # | 位置 | 描述 |
|---|------|------|
| 1 | `PathTable.tsx:35-36` | `validatedRef` / `expandedRef` 随会话持续增长，不清理。Set 中的 key 是路径字符串，正常使用下数量有限（几十到几百），无实际性能影响 |
| 2 | `use-keyboard.ts:39-54` | 非管理员模式下 Ctrl+Z/Y/N/S/Delete 静默忽略，用户无任何反馈 |
| 3 | `app-store.ts:317` | `backup_registry` 失败时 set statusMessage 为警告，但若后续保存成功，`statusMessage` 被覆盖为「保存成功」，用户看不到备份失败的警告 |
| 4 | `MergePreview.tsx:57` | `key={${source}-${displayIndex}}` — key 由翻译后的 source 字符串和 displayIndex 拼接，理论上在极端场景（同一秒内两次渲染翻译变化）可能重复，实际概率极低 |
| 5 | `system.rs:44-45` | `ExpandEnvironmentStringsW` 返回 0 时只 `log::warn`，可通过 `GetLastError` 获取具体错误码来改进日志 |
| 6 | `disabled.rs:54-56` | `load_disabled_state` 中 `content.trim().is_empty()` 检查在 `serde_json::from_str` 之前，空文件返回空数组。但如果文件包含有效 JSON 空对象 `{}`，反序列化为 `DisabledState::default()` 也正确。逻辑完整 |

---

## 修复优先级建议

| 优先级 | Bug | 理由 |
|--------|-----|------|
| **P0** | BUG 1 — undo/redo 不持久化 disabled | 用户可感知的数据不一致 |
| **P1** | BUG 2 — expand_env_vars 截断 | 概率低但后果是静默数据损坏 |
| **P2** | BUG 3 — E2E mock 格式 | 不影响生产，但阻碍 E2E 测试扩展 |
| **P3** | BUG 4 — 双失败错误消息 | 边缘场景的 UX 问题 |
| **P3** | BUG 5 — 导入双 undo | 用户体验小瑕疵 |
