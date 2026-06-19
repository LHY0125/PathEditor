# 获取帮助

## 📖 文档

- [README](README.md) — 项目简介、功能列表、安装指南
- [CONTRIBUTING](CONTRIBUTING.md) — 贡献指南
- [CHANGELOG](CHANGELOG.md) — 版本变更记录
- [ROADMAP](ROADMAP.md) — 未来规划
- [SECURITY](SECURITY.md) — 安全政策

## 🐛 报告 Bug

1. 先搜索 [Issues](https://github.com/LHY0125/PathEditor/issues) 确认未被报告
2. 使用 **Bug Report** 模板创建新 Issue
3. 提供系统信息（Windows 版本、PathEditor 版本）
4. 附上复现步骤和截图

## 💡 功能建议

1. 检查 [ROADMAP](ROADMAP.md) 确认不在已有计划中
2. 使用 **Feature Request** 模板创建新 Issue
3. 描述使用场景和期望行为

## ❓ 常见问题

### CLI 命令找不到？

```bash
patheditor --help
```

确保已通过 `cargo install --path cli` 安装，且 `~/.cargo/bin` 在 PATH 中。

### 提示权限不足？

编辑系统 PATH 需要管理员权限。右键以管理员身份运行，或使用 CLI `patheditor check-admin` 检测。

### 保存后环境变量未生效？

PathEditor 会自动广播 `WM_SETTINGCHANGE`，但部分程序需要手动重启才能识别新 PATH。

## 📧 联系

- GitHub Issues: [LHY0125/PathEditor](https://github.com/LHY0125/PathEditor/issues)
- 安全问题: 参见 [SECURITY.md](SECURITY.md)
