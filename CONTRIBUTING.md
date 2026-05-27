# 贡献指南

感谢你对 PathEditor 的关注！

## 提交 Issue

- 使用清晰的标题描述问题
- 提供复现步骤
- 附上系统信息（Windows 版本、是否管理员）
- 如果是功能建议，说明使用场景

## 提交 Pull Request

1. Fork 仓库并从 `main` 创建功能分支
2. 运行 `npm test` 和 `cargo check` 确保通过
3. 遵循项目代码规范：
   - TypeScript `strict: true`，零编译错误
   - 前端核心逻辑在 `src/core/`，纯函数，零依赖
   - Rust `unsafe` 块必须有 `// SAFETY:` 注释
4. 新功能应包含测试

## 本地开发

```bash
npm install
npx tauri dev
```

详见 [README.md](./README.md#开发)。
