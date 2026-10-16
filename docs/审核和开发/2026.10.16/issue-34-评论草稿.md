## 已修复（5.1.5，未发布）

你的两点建议**全部采纳**，且核实过程中发现了一个比 issue 描述更深的问题。

### 修复后的行为

```
$ patheditor env get THIS_VAR_DOES_NOT_EXIST
错误: 用户环境变量 THIS_VAR_DOES_NOT_EXIST 不存在
```

退出码仍为 1；`code` 为 `notFound`；**不再泄漏 `os error 2`**，变量名只出现一次。你指出的「保留带变量名和操作」这一优点保留了下来。

### 根因（比 issue 描述更深一层）

issue 的描述是「漏判 NotFound」。实际查下来，问题出在 `reg_store` 的端口边界：

- `EnvHiveStore::get_raw` 把 winreg 的 `io::Error` **格式化成 `String`** 返回（`core/src/reg_store.rs`），**结构化信息（`ErrorKind`）在这一步就丢了**——上层 `read_env_var` 拿到的只是一个字符串，无从判定「找不到」还是「其它 IO 错误」。

修复方式：新增 `get_raw_kind`（**不改既有 `get_raw` 签名**，遵守 W2-N2 端口边界裁断），按 `io::ErrorKind` 把错误分类为 `NotFound` / `Io`，`read_env_var` 据此产出正确的 `ErrorCode`。其它错误（类型不支持 / IO / 解码失败）的 message 与 code **逐字节不变**。

### 附带发现：一处「注释与实现不符」

`core/src/registry/env_var.rs` 的 `read_env_var` 上有一段 2026-09-20 的裁断注释（W2-B1）写着：

> 三类错误在函数内部分类完成 —— 读取失败 → `Io`、类型不支持 → `UnsupportedType`、解码失败 → `Parse`。调用方直接 `?` 传播，**不再整体包成 `Io`**。

但**紧接着的代码恰恰在整体包 `Io`**（`.map_err(|m| CoreError::new(ErrorCode::Io, "read_env_var", m))`）——即该注释描述的行为**从未被实现**（或被后续改动回退）。

这才是这个 bug 能长期存在的真正原因：注释声明了正确的设计，代码没有落实，而注释的"权威感"让后来者不再复核代码。修复时已把 `read_env_var` 改走 `get_raw_kind`，并在注释里补明了这一点。

（本条评论对应 5.1.5 的修复提交；版本号尚未升级，等发布时一并更新。）
