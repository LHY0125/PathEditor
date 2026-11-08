# 安全策略

## 报告漏洞

如果你发现安全漏洞，请**不要**在公开 Issue 中报告。请通过以下方式私下报告：

- GitHub: 在 [Security Advisories](https://github.com/LHY0125/PathEditor/security/advisories) 页面提交
- 邮件: 联系项目维护者

我们会在 **48 小时内**确认收到报告，并在 7 天内提供初步评估和修复计划。

## 安全最佳实践

### 作为用户

- 仅从 [Releases](https://github.com/LHY0125/PathEditor/releases) 页面下载安装包
- 保存前确认 PATH 内容正确，备份文件存放在 `~/.patheditor/backups/`
- 编辑系统 PATH 需要管理员权限

### 作为开发者

- 永远不要在源代码中硬编码密钥或凭据
- 所有 `unsafe` 块必须有 `// SAFETY:` 注释
- 输入验证在系统边界执行
- 敏感操作（注册表写入）使用最小权限原则

## 支持版本

| 版本   | 支持状态                                |
| ------ | --------------------------------------- |
| v7.x   | 活跃支持                                |
| v6.x   | 仅安全修复（加密库，需删除后升级到 v7） |
| v5.x   | 不再支持                                |
| v4.x   | 仅安全修复                              |
| < v4.0 | 不再支持                                |

## 已知问题

- 数据库 `~/.patheditor/patheditor.db` 是**明文 SQLite**；`.pebak` 备份仍经 DPAPI 加密，仅回落或遗留的 `.json` 备份是明文。如需额外保护，请将 `~/.patheditor/` 目录设为仅当前用户可读。
