# PathEditor Issue #26 修复方案

## 1. 结论

**Bug 已确认存在。**

Issue：https://github.com/LHY0125/PathEditor/issues/26

标题：`[Bug] 写入 PATH 时注册表值类型被降级为 REG_SZ，导致 %VAR% 条目无法展开`

当前代码的写路径仍会复现该问题。问题不是 PATH 文本丢失，而是注册表值类型被从 `REG_EXPAND_SZ` 改为 `REG_SZ`。Windows 新进程读取环境变量时不会再展开其中的 `%VAR%`，因此 `%SystemRoot%`、`%USERPROFILE%` 等条目可能失效。

建议优先级：**P1 / High**。影响系统 PATH 和用户 PATH，且每次保存都可能再次改变注册表值类型。

## 2. 代码证据

当前基线：`main` 提交 `58c7a80`，版本 `5.1.1`。

`core/src/registry.rs` 的 `save_paths()` 仍使用字符串重载写入：

```rust
env_key
    .set_value(PATH_VALUE, &value)
    .map_err(|e| format!("无法写入{} PATH: {}", label, e))?;
```

依赖 `winreg 0.52.0` 的实现已经确认了类型推断行为：

```rust
// winreg-0.52.0/src/reg_key.rs
pub fn set_value<T: ToRegValue, N: AsRef<OsStr>>(&self, name: N, value: &T) -> io::Result<()> {
    self.set_raw_value(name, &value.to_reg_value())
}

// winreg-0.52.0/src/types.rs
to_reg_value_sz!(String);
to_reg_value_sz!(&'a str, 'a);
// to_reg_value_sz! 内部固定设置 vtype = REG_SZ
```

同时，`get_value::<String>()` 对 `REG_SZ | REG_EXPAND_SZ | REG_MULTI_SZ` 都按原始注册表字节解码，不会执行环境变量展开。因此读回时文本看起来正常，类型却已经变成 `REG_SZ`，问题非常隐蔽。

`save_paths()` 只被 `save_system_paths()` 和 `save_user_paths()` 调用。GUI 的 `gui/src/commands/registry.rs` 和 CLI 的 `cli/src/runtime.rs` 最终都走到这两个函数，所以以下写操作都受影响：

- GUI 保存系统/用户 PATH
- CLI `add`、`remove`、`edit`、`move-up`、`move-down`
- CLI `clean`、`enable`、`disable`
- CLI/GUI `import`、profile apply，以及其他最终复用 PATH 保存的事务

当前影响范围同时包括 HKLM 系统 PATH 和 HKCU 用户 PATH。

## 3. 修复目标

写回 PATH 时必须保持原有注册表值类型：

- 原值类型为 `REG_EXPAND_SZ`：继续写 `REG_EXPAND_SZ`。
- 原值类型为 `REG_SZ`：继续写 `REG_SZ`，除非产品明确决定做迁移，否则不要静默改变语义。
- 原值不存在或类型不是字符串类型：默认使用 `REG_EXPAND_SZ`，这是 PATH 的安全默认值。
- UTF-16 内容必须以 NUL 结尾，避免依赖库对原始注册表缓冲区长度的处理差异。

不要把“保持类型”实现成无条件 `REG_EXPAND_SZ`。这会让原本明确为 `REG_SZ` 的值发生变化，也会使包含字面量 `%` 的目录名产生不同解释。

## 4. 推荐修改

在 `core/src/registry.rs` 中引入 `RegValue`、`RegType` 和 `ToRegValue`，并让 `save_paths()` 使用 `get_raw_value()` + `set_raw_value()`：

```rust
use winreg::enums::{RegType, KEY_READ, KEY_WRITE, REG_EXPAND_SZ, REG_SZ};
use winreg::types::ToRegValue;
use winreg::{RegKey, RegValue};

fn path_value_type(env_key: &RegKey) -> RegType {
    match env_key.get_raw_value(PATH_VALUE).map(|raw| raw.vtype) {
        Ok(REG_EXPAND_SZ) => REG_EXPAND_SZ,
        Ok(REG_SZ) => REG_SZ,
        _ => REG_EXPAND_SZ,
    }
}

fn make_path_value(value: &str, vtype: RegType) -> RegValue {
    // 复用 winreg 的编码逻辑，确保 UTF-16LE 和结尾 NUL 与库契约一致。
    let mut raw = value.to_reg_value();
    raw.vtype = vtype;
    raw
}

fn save_paths(
    root: winreg::HKEY,
    sub_path: &str,
    label: &str,
    paths: &[String],
) -> Result<(), String> {
    let value = validate_and_join_paths(paths, label)?;

    let key = RegKey::predef(root);
    // 读取类型需要 KEY_QUERY_VALUE，因此必须同时请求 READ | WRITE。
    let env_key = key
        .open_subkey_with_flags(sub_path, KEY_READ | KEY_WRITE)
        .map_err(|e| format!("无法写入{}注册表（需要管理员权限）: {}", label, e))?;

    let vtype = path_value_type(&env_key);
    let raw = make_path_value(&value, vtype);

    env_key
        .set_raw_value(PATH_VALUE, &raw)
        .map_err(|e| format!("无法写入{} PATH: {}", label, e))?;

    log::info!("已保存{} PATH，{} 个条目", label, paths.len());
    Ok(())
}
```

实现时优先复用 `winreg::types::ToRegValue`，不要手工复制一串容易出错的 UTF-16 编码逻辑。若项目选择手写编码，必须显式补 NUL 结尾，并保证小端序。

### 兼容性注意

本修复会阻止 PathEditor 继续降级类型，但**不会自动修复已经被旧版本改成 `REG_SZ` 的机器**：下一次保存会看到 `REG_SZ` 并按“保持原类型”策略继续保存。

因此发布时需要二选一：

1. 提供显式修复命令，例如 `patheditor repair-registry-type`，由用户确认后把 PATH 写回 `REG_EXPAND_SZ`。
2. 在 release note/README 中给出 PowerShell 修复步骤，并明确提醒备份和重新登录/广播环境变更。

不建议在没有明确提示的情况下自动把用户现有的 `REG_SZ` 改成 `REG_EXPAND_SZ`。

## 5. 必须补充的测试

### Rust 单元测试

在 `core/src/registry.rs` 测试模块中至少增加：

1. `REG_EXPAND_SZ` 经保存后仍为 `REG_EXPAND_SZ`。
2. `REG_SZ` 经保存后仍为 `REG_SZ`。
3. 不存在的值默认写为 `REG_EXPAND_SZ`。
4. `%SystemRoot%\system32` 等文本保存后字面量不变且末尾 NUL 正确。
5. 空白、CJK 路径和超长路径校验仍然通过原有测试。

为覆盖真实注册表调用，建议增加一个隔离集成测试：

- 只在 `HKCU\Software\PathEditor\Tests\<唯一编号>` 下创建临时键。
- 先写入 `REG_EXPAND_SZ`，调用重构后的保存逻辑，再读取 `get_raw_value().vtype` 断言类型。
- 测试结束使用 RAII guard 删除临时键。
- **测试绝不能读写 `HKCU\Environment\Path` 或 HKLM PATH。**

如果测试环境不允许写注册表，至少必须保留纯函数测试，覆盖类型选择、UTF-16 编码和 NUL 结尾。

### 回归命令

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run verify
```

## 6. 人工验收建议

以下步骤会产生真实注册表写入，只能在明确授权、已备份且可恢复的测试机上执行：

1. 记录修改前两个 hive 的 `GetValueKind('Path')` 和原始值。
2. 执行一次 PathEditor 写操作。
3. 确认 HKLM/HKCU 的类型分别保持不变。
4. 确认文本、顺序、`enabled` 状态和禁用侧车快照没有被破坏。
5. 广播 `WM_SETTINGCHANGE` 或重新登录，确认新进程能够解析 `%VAR%` 条目。
6. 用备份恢复测试环境并再次核对类型。

本次代码审查未写入真实注册表，结论来自当前源码和 `winreg 0.52.0` 的实现。

## 7. 验收标准

- [ ] 所有 PATH 写操作统一经过修复后的 `save_paths()`。
- [ ] HKLM 和 HKCU 的 `REG_EXPAND_SZ` 在写回后不降级。
- [ ] `REG_SZ` 不被静默改成其他类型。
- [ ] 新创建值默认使用 `REG_EXPAND_SZ`。
- [ ] UTF-16 NUL 结尾和文本完整性有测试覆盖。
- [ ] 增加隔离注册表测试或等价的纯函数测试。
- [ ] `cargo test --workspace`、Clippy 和 `npm run verify` 通过。
- [ ] Release note 说明旧版本已受影响机器的修复方式。
