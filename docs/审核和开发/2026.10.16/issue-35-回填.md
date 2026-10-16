已处理，随 **v5.1.5** 发布（[Release](https://github.com/LHY0125/PathEditor/releases/tag/v5.1.5)）。

## 你问的三个答案：第 3 种（纯新建语义）是正确的那一个

代码核实：`env add` 确实**没有任何并发字段**（对比 `env set` / `env remove` 有 `--revision` / `--force`）。而 `create_env_var` 在 core 内**遇同名变量会拒绝**（`ErrorCode::NameExists`），**不会覆盖** —— 这正是你说的第 3 种。「新建没有『旧值』可被并发覆盖，CAS 不适用」，所以 `add` 不加 revision 守卫是**有意为之**。

### 已做的改进：把豁免理由写进 help

你指出的实质问题成立 —— 这个理由此前**只存在于源码注释**，用户看 `add --help` 无从得知。本版已在 help 里写明：

> **纯新建语义**：同名变量会被拒绝（`NameExists`），不会覆盖 —— 因此无需 revision 守卫。注意检查与写入是两步，存在毫秒级竞态窗口。

（竞态窗口如实标注 —— 与 `CLAUDE.md` 记录的「检查与写入是两步操作」一致。）

### 顺带

同一位置还按另一条 issue（#38）补了「省略 VALUE 会立即报错，不进入交互提示」，两处 help 文案一起收口。

关闭本条。
