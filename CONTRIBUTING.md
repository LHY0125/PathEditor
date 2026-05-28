# 贡献指南

## 本地开发环境

- **Node.js** 22+
- **Rust** 1.95+ (stable-x86_64-pc-windows-gnu)
- **MinGW-w64** (GCC 15.x 需 `-lmcfgthread` 链接标志)
- **Windows 10+** (自带 WebView2)

## 开发流程

1. Fork 本仓库
2. `git clone <你的 fork>`
3. `git checkout -b feature/xxx`
4. 开发 + 测试
5. `git commit` (遵循约定式提交格式)
6. `git push`
7. 提交 Pull Request

## 运行测试

```bash
# 前端单元测试
npm test

# Rust 测试
cargo test

# E2E 测试 (需要先 npm run dev)
npx playwright test

# Clippy 检查
cargo clippy -- -D warnings
```

## 代码规范

### TypeScript

- `strict: true`，零编译错误
- 核心逻辑在 `src/core/`，纯函数，零框架依赖
- 不可变操作优先

### Rust

- 所有 `pub fn` 必须有 `///` 文档注释
- 所有 `unsafe` 块必须有 `// SAFETY:` 注释
- `cargo clippy -- -D warnings` 零警告
- `cargo fmt` 统一格式

## 提交格式

```
<类型>: <描述>
```

类型：`feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`

## 项目结构

```
core/        # Rust 核心库（零 Tauri 依赖）
gui/         # Tauri 桌面应用
cli/         # 命令行工具
src/         # React 前端
tests/unit/  # 前端单元测试
e2e/         # Playwright E2E 测试
```

## 开始贡献前

- 大改动建议先开 Issue 讨论
- 新功能需要对应的测试
- 不要引入新的 clippy 警告
