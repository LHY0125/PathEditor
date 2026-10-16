已修复，随 **v5.1.5** 发布（[Release](https://github.com/LHY0125/PathEditor/releases/tag/v5.1.5)）。

### 核实更正

issue 说「`~/.patheditor/backups/` 目前没有任何轮转或上限」—— 这对 **env 备份是错的**：`env_backup_*.json` 早已有轮换（`ENV_BACKUP_KEEP = 20`，可被 `config.ini` 的 `env_backup_keep` 覆盖）。

**但你对 path 备份的判断完全正确**：`path_backup_*.txt` 此前**只被写入、无任何轮换**，所以你实测的约 250 个文件确实在无限累积。

### 本版的修复（只针对 path 侧）

- `path_backup_*.txt` 写后自动轮换，**默认保留最近 20 份**（与 env 侧对称，具名常量 `PATH_BACKUP_KEEP`）。
- 可被 `~/.patheditor/config.ini` 的 **`path_backup_keep`** 覆盖，回落语义与 `env_backup_keep` 一致。
- 轮换**只删除本工具生成的 `path_backup_*.txt`**：绝不碰 `env_backup_*.json` / `.bak` / `.corrupt-*` 或目录中任何其它文件 —— 有对抗性测试守护（放入这些"外来文件"后断言全部仍在）。
- **仅在默认备份目录生效**：用户指定的自定义目录不参与轮换（避免动到用户自己放在那里、可能是备份的文件）。

### 关于你肯定的「两类备份分得很干净」

这个观察是对的，而且**正是它导致了 path 侧漏轮换** —— `rotate_env_backups` 为不误删，严格只匹配 `env_backup_*.json`，于是 `path_backup_*.txt` 成了盲区。本版补上对称的 `rotate_path_backups`，两类仍各自隔离。

关闭本条。
