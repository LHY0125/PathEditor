# PathEditor 复审回执

## 1. 回执结论

- 回执日期：2026-09-14
- 复审对象：开发窗口针对 `PathEditor-复审报告.md` 的整改
- 回执结论：**代码级复审通过，进入待真实 Tauri 集成验证状态**
- 版本状态：仍为 `5.1.0`
- Git 状态：未 commit、未 push
- 注册表状态：本轮未执行任何真实系统/用户 PATH 写入

整改内容与复审报告中的 P0/P1/P2/P3 项基本对应；本地静态检查、前后端单元测试、覆盖率检查和 mock E2E 均通过。由于用户明确要求不要写入真实注册表，尚未证明“普通用户编辑用户 PATH”和“禁用后重启”在真实 Tauri 进程中的完整闭环。

## 2. 日期与归档核对

| 项目         | 结果                          |
| ------------ | ----------------------------- |
| 当前系统日期 | 2026-09-14（Asia/Shanghai）   |
| 审核目录     | `docs/审核和开发/2026.09.14/` |
| 原始审核文档 | `PathEditor-重构优化审核.md`  |
| 复审报告     | `PathEditor-复审报告.md`      |
| 本回执       | `PathEditor-复审回执.md`      |
| 日期一致性   | 通过；目录日期与当前日期一致  |

本轮整改文件仍处于当前工作树，尚未提交到 Git；不存在把整改内容写入其他日期目录的情况。

## 3. 整改核对

### 3.1 已确认修复

| 复审项                              | 核对结果           | 证据                                                                                      |
| ----------------------------------- | ------------------ | ----------------------------------------------------------------------------------------- |
| REV-01 能力字段契约                 | 通过               | `core/src/capabilities.rs:7` 增加 camelCase 序列化；Rust 契约测试和共享 fixture 已存在    |
| REV-02 验证队列竞态                 | 通过               | `use-path-validation.ts` 使用共享 in-flight Promise；deferred rerender 回归测试已通过     |
| REV-03 禁用状态跨重启               | 通过               | `core/src/disabled.rs` 新增完整 `systemSnapshot/userSnapshot`，启动合并时保留禁用孤儿条目 |
| REV-04 侧车写入失败后的 dirty 状态  | 通过               | Store 新增 `_pendingSys/_pendingUser`；测试覆盖失败后重试且只补写 `save_path_snapshot`    |
| REV-05 导入权限                     | 通过               | 单 hive 导入显式检查权限；`ImportDialog` 禁用无权限选项                                   |
| REV-06 旧 TS 导入导出实现           | 以兼容夹具方式收口 | 运行时 GUI/CLI 已统一走 Rust；旧 TS 实现保留为测试/兼容夹具，注释已纠正                   |
| REV-07 CLI 配置保存                 | 通过               | `cli/src/profile_ops.rs:17-21` 使用完整 `load_path_snapshot()` 保存配置                   |
| REV-08 `loadDisabledState` 静默回退 | 通过               | `backend.ts` 对元组和字符串数组执行严格形状校验，不符合契约直接抛错                       |
| REV-09 `verify` 缺少 ESLint         | 通过               | `package.json` 的 `verify` 已加入 `npm run lint`；ESLint 已忽略 `coverage/`               |

### 3.2 仍需保留的跨层项

以下不是本轮整改失败，但尚未达到完整发布验收：

- Rust 层仍返回中文/自由文本错误；错误码加参数、再由 GUI/CLI 本地化的统一契约尚未完成。
- Rust→TypeScript 自动类型生成尚未接入；`backend.ts` 已集中 IPC，但仍依赖手工类型和运行时校验。
- E2E 使用 mock IPC，不能替代真实 Tauri、真实注册表和真实 WebView 集成测试。

## 4. 验证证据

### 4.1 已执行

执行命令：

```powershell
npm run verify
```

结果：

- Prettier：通过
- ESLint：0 error；2 个 TanStack Virtual/React Compiler 非阻断 warning
- 前端单元测试：127 passed
- 覆盖率：Lines 86.33%
- 生产构建：通过
- `cargo fmt --check`：通过
- `cargo clippy --workspace --all-targets -- -D warnings`：通过
- Rust 测试：63 passed

另外独立执行了 mock E2E：

```powershell
npm run test:e2e
```

结果：13 passed。该测试通过 mock IPC 运行，不写入真实注册表。

### 4.2 未执行且不能声称完成

- 未以普通用户实际启动 Tauri 应用并编辑、保存用户 PATH。
- 未在真实注册表上验证禁用路径保存、退出、重启和重新启用。
- 未验证真实 Windows 注册表与 `disabled.json` 的所有异常/权限失败组合。

## 5. 残余风险

### R-01 [非阻断] 合并快照时会按路径键去重

`core/src/disabled.rs` 的 `merge_hive` 使用 `HashSet` 和 `HashMap` 按大小写不敏感的路径键去重。若注册表中已经存在重复 PATH 条目，启动加载时界面不会再完整展示重复项，后续保存可能将其折叠。该行为是否可接受取决于产品是否要求“重复项必须保留到用户显式执行一键清理”。当前没有看到覆盖该语义的测试。

### R-02 [非阻断] CLI 侧车写入失败仍是部分成功

CLI 的 `verify_and_save` 完成后才 `persist_snapshot`；若注册表写入成功而侧车写入失败，CLI 会报错退出，但注册表已经改变。GUI 已有 `_pending` 重试机制，CLI 尚无等价的持久化补写机制。

### R-03 [非阻断] 旧 TS 导入导出代码仍会产生维护成本

虽然生产运行时已经统一到 Rust，但 `src/core/import-export.ts` 和 `src/core/validation.ts` 仍作为兼容/测试夹具保留。只要它们继续被测试引用，就需要明确它们不是生产契约，否则后续开发者仍可能误改其中一套而以为影响生产。

## 6. 回执判定

**本轮代码级整改符合 2026-09-14 的复审目标和归档要求，可以进入真实 Tauri 集成验证阶段；在真实 PATH 闭环测试完成前，不建议标记为可发布或可提交终验。**

后续验证必须在独立、明确授权的终端环境中进行，并记录操作前后的注册表快照、`disabled.json` 内容、应用重启结果和回滚结果。
