# CLI 环境变量管理 — 真实注册表闭环测试记录

**日期**: 2026-09-18
**分支**: `worktree-all-env-vars`
**被测版本**: 5.1.3
**测试对象**: `patheditor env` 子命令组（`add` / `set` / `remove` / CAS 冲突路径）

## 1. 结论

**4 项真实注册表写入验证全部通过。** 测试结束后注册表回到操作前状态，零残留、零污染。

前置的零风险验证（`env list` / `env get` 只读路径、clap 参数校验拒绝路径）已在开发阶段完成，本记录只覆盖**真实写入**部分。

## 2. 授权与前置准备

用户于 2026-09-18 显式授权执行真实注册表写入验证，要求逐步进行、遇异常即停下求证。

| 准备项             | 结果                                                                                           |
| ------------------ | ---------------------------------------------------------------------------------------------- |
| 注册表备份         | `C:\Users\33644\.patheditor\backups\path_backup_20260918_141256_062.txt`（退出码 0）           |
| 操作前快照         | `.tmp-verify/before-user.json`（user 29 项）、`.tmp-verify/before-system.json`（system 23 项） |
| 临时变量名冲突检查 | 两个 hive 均无 `PATHEDITOR_*`，临时名可用                                                      |
| `Path` 隔离确认    | 通用通路中不出现 `Path`（`RESERVED_NAMES` 过滤生效）                                           |

**测试范围限定**：只使用自建临时变量 `PATHEDITOR_VERIFY_TMP`，不触碰任何既有变量，尤其不触碰 `Path`。

## 3. 验证项与证据

### 3.1 `env add` 全流程（HKCU）

```text
$ patheditor env add PATHEDITOR_VERIFY_TMP hello-verify --kind expand
已新建用户变量: PATHEDITOR_VERIFY_TMP
退出码: 0
```

读回验证：

```text
$ patheditor env get PATHEDITOR_VERIFY_TMP
hello-verify
退出码: 0

$ patheditor env list --user | grep PATHEDITOR
PATHEDITOR_VERIFY_TMP        expand  hello-verify
```

JSON 元数据：

```json
{
  "canDelete": true,
  "canEdit": true,
  "hive": "user",
  "kind": "expandString",
  "name": "PATHEDITOR_VERIFY_TMP",
  "preview": "hello-verify",
  "revision": "61cd97af7f62b965",
  "sensitive": false
}
```

**结论**：`--kind expand` 正确写入 `REG_EXPAND_SZ`（JSON `kind: expandString`），**未被降级为 `REG_SZ`**——即 Issue #26 的回归点未复现。JSON 契约字段为 camelCase，**不含 `value` 字段**（明文只从 `env get` 输出）。

### 3.2 `env set --force`

```text
set 前：hello-verify

$ patheditor env set PATHEDITOR_VERIFY_TMP --value hello-verify-2 --force
已更新用户变量: PATHEDITOR_VERIFY_TMP
退出码: 0
```

读回验证：

```text
$ patheditor env get PATHEDITOR_VERIFY_TMP
hello-verify-2

$ patheditor env list --user | grep PATHEDITOR
PATHEDITOR_VERIFY_TMP        expand  hello-verify-2
```

revision 变化：`61cd97af7f62b965` → `a877a9d563aaa31e`（内容变更后重算）。

**结论**：值更新成功；**类型列仍为 `expand`**（`set` 只改值不改类型，符合设计）。`--force` 的「现读现写」语义正确——core 的 `update_env_var` 签名恒要求 `expected_revision`，CLI 侧通过重新读取当前 revision 并传入来表达「跳过校验」。

### 3.3 退出码 3 真实冲突路径（CAS 拒写）

构造方式：取 revision 后由另一进程（本测试中以另一次 `env set --force` 调用模拟外部修改）改变量值，再用**已过期的 revision** 执行 `env set`。

```text
步骤 1：取当前 revision
当前 revision: a877a9d563aaa31e

步骤 2：外部改动（模拟另一进程）
$ patheditor env set PATHEDITOR_VERIFY_TMP --value hello-verify-3 --force
已更新用户变量: PATHEDITOR_VERIFY_TMP
退出码: 0
新 revision: a877aad563aaa4d1（已与旧值不同）

步骤 3：用过期 revision 执行写入
$ patheditor env set PATHEDITOR_VERIFY_TMP --value SHOULD-NOT-BE-WRITTEN --revision a877a9d563aaa31e
错误: [E_CONFLICT] 变量已被其他进程修改，请重新加载
退出码: 3
```

写入拒斥验证（关键）：

```text
$ patheditor env get PATHEDITOR_VERIFY_TMP
hello-verify-3
退出码: 0

✅ 值未被覆盖为 SHOULD-NOT-BE-WRITTEN，CAS 拒写生效
```

**结论**：stderr 保留 `[E_CONFLICT]` 前缀原文；**退出码为 3**（与一般错误的 1 区分开，脚本可据此判断「重试可恢复」）；**冲突被拒后变量值原封不动**——这是 CAS 机制的核心保证，已获真实注册表实证。

### 3.4 `env remove`（CAS 模式）

```text
删除前：hello-verify-3，revision a877aad563aaa4d1

$ patheditor env remove PATHEDITOR_VERIFY_TMP --revision a877aad563aaa4d1
已删除用户变量: PATHEDITOR_VERIFY_TMP
退出码: 0
```

删除后验证：

```text
$ patheditor env get PATHEDITOR_VERIFY_TMP
错误: 无法读取环境变量 PATHEDITOR_VERIFY_TMP: 系统找不到指定的文件。 (os error 2)
退出码: 1
```

**结论**：删除成功；变量已不存在（`get` 以非零退出码报错）。`remove` 的 `--revision` CAS 校验路径与 `set` 对称工作。

## 4. 快照对比 — 零污染证明

测试结束后重新采集快照，与操作前逐项对比（比对键为 `name|kind|revision`）：

```text
user:   操作前 29 → 操作后 29  ✅ 一致
system: 操作前 23 → 操作后 23  ✅ 一致

user hive 新增项: 无
user hive 消失项: 无
```

**结论**：注册表完全回到操作前状态。无非预期的新增、消失或修改。

## 5. 未覆盖项

以下情形本轮未验证，需要时另立授权测试：

- `env add` / `env set` / `env remove` 在 **HKLM（系统 hive）** 的行为——本轮全部在 HKCU 进行
- `--stdin` 与 `--value-file` 两个值输入通道的端到端读数（本轮只用位置参数与 `--value`）
- 敏感变量（命中 `is_sensitive`）的 `env get` 读回
- 保护名单变量被写入时的拒绝错误透传
- 并发创建同名变量的极端竞态（`create_env_var` 的检查与写入是两步操作，存在窗口）

## 6. 回滚资源

| 资源       | 位置                                                                     |
| ---------- | ------------------------------------------------------------------------ |
| 注册表备份 | `C:\Users\33644\.patheditor\backups\path_backup_20260918_141256_062.txt` |
| 操作前快照 | `.tmp-verify/before-user.json`、`.tmp-verify/before-system.json`         |

注：`.tmp-verify/` 为测试临时目录，验证通过后按用户指示删除，其内容已在本记录中留存关键数据。
