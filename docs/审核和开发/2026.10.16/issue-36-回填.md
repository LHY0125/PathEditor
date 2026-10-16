已实现，随 **v5.1.5** 发布（[Release](https://github.com/LHY0125/PathEditor/releases/tag/v5.1.5)）。

`env set` / `env add` / `env remove` 三个破坏性写命令现在都支持 `--dry-run` 与 `--json`。

### 用法

```bash
patheditor env set JAVA_HOME --value "C:\Java\jdk-21" --dry-run
# 将更新用户变量 JAVA_HOME：C:\Java\jdk-17 → C:\Java\jdk-21（未写入任何内容）

patheditor env set JAVA_HOME --value "C:\Java\jdk-21" --dry-run --json
# {"operation":"set","hive":"user","name":"JAVA_HOME","oldValue":"C:\\Java\\jdk-17","newValue":"C:\\Java\\jdk-21","kind":"string","writable":true,"sensitive":false}
```

### 语义（你要的「落盘前可 diff、机器可读」）

- **纯读**：不写注册表、不产生备份文件、不广播环境变更。已补进程级测试守护这一性质（含变异测试验证）。
- **不要求** `--revision` / `--force`：预览无并发风险，强制要 revision 只会制造摩擦。
- **变量不存在时照常报 `notFound`（退出码 1）**：预览与执行对「是否存在」的判定一致，不制造「预览说可以、执行失败」的落差。
- `--json` 需与 `--dry-run` 同用（单独用由 clap 拒绝、退出码 2），输出结构供脚本消费：`{operation, hive, name, oldValue, newValue, kind, writable, sensitive}`。

### 一处安全加固（与你这条同类：让自动化场景可控且安全）

dry-run 的输出会进终端 / shell 历史 / CI 日志，因此**不新开明文出口**：命中 `is_sensitive` 的变量不回传明文（`oldValue` 为 `null`、文本显示 `(敏感)`），非敏感值剔除控制字符（防 ANSI 注入）。

关闭本条。
