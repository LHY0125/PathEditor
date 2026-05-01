# 撤销/重做 UI 集成 — 设计文档

## 背景

撤销/重做后端（`src/core/undo_redo.c`）已完整实现，支持 7 种操作类型的记录与回滚。所有 UI 操作（新建/编辑/删除/上移/下移/清理）均已调用 `push_undo_record()` 写入历史。但 `undo()` 和 `redo()` 函数未被任何 UI 代码调用——用户无法触发撤销或重做。

## 目标

在界面上添加撤销/重做按钮，并绑定 Ctrl+Z / Ctrl+Y 快捷键，让用户可以回退和恢复操作。

## 改动文件

| 文件 | 改动内容 |
|------|---------|
| `include/utils/ui_constants.h` | 新增 `CTRL_BTN_UNDO`、`CTRL_BTN_REDO` 常量 |
| `src/ui/main_window.c` | 创建撤销/重做按钮，绑定回调，调整布局 |
| `src/controller/callbacks_nav.c` | 新增 `btn_undo_cb`、`btn_redo_cb`，`list_k_any_cb` 增加 Ctrl+Z/Y 检测 |
| `lua/config.lua` | 新增 `button.undo`、`button.redo` 配置项 |
| `locale/` 翻译文件 | 同步新增按钮的中英文翻译 |

## 核心逻辑

```
btn_undo_cb(dlg):
    ctx = get_app_context_from_dlg(dlg)
    if !can_undo(ctx->undo_redo_mgr): return
    undo(ctx->undo_redo_mgr, &ctx->sys_paths, &ctx->user_paths)
    sync both lists to UI
    update undo/redo button enabled state

btn_redo_cb(dlg):
    同上，调用 redo()

list_k_any_cb:
    新增分支:
      if c == K_cZ → btn_undo_cb
      if c == K_cY → btn_redo_cb
```

## 按钮布局

撤销/重做按钮放在上移/下移按钮下方：

```
  [新建]      [编辑]
  [浏览]      [删除]
     (分隔)
  [一键清理]
     (分隔)
  [导入]      [导出]
  [上移]      [下移]
  [撤销]      [重做]    ← 新增
```

## 按钮状态

- `can_undo() == false` → 撤销按钮 `ACTIVE=NO`
- `can_redo() == false` → 重做按钮 `ACTIVE=NO`
- 每次 undo/redo 执行后刷新按钮状态

## 不做的事

- 不修改 `undo_redo.c` 后端代码（已完备）
- 不添加操作历史面板（保持简洁，通过按钮状态反馈即可）
- 不在保存后清空历史（当前设计由 `clear_undo_redo_history` 决定，保持现有行为）
