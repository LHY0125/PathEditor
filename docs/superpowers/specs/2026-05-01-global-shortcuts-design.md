# 全局快捷键 — 设计文档

## 背景

Ctrl+Z/Y 撤销/重做已在功能 1 中实现（列表级 K_ANY）。新增 Ctrl+N/S/F 作为对话框级的全局快捷键。

## 目标

添加三个全局快捷键：`Ctrl+N` 新建、`Ctrl+S` 保存、`Ctrl+F` 聚焦搜索框。

## 改动文件

| 文件 | 改动 |
|------|------|
| `include/controller/callbacks.h` | 声明 `dlg_k_any_cb` |
| `src/controller/callbacks_sys.c` | 实现 `dlg_k_any_cb` |
| `src/ui/main_window.c` | 对话框注册 `K_ANY` 回调 |

## 核心逻辑

```
dlg_k_any_cb(dlg, c):
    if c == K_cN → btn_new_cb(dlg)
    if c == K_cS → btn_ok_cb(dlg)
    if c == K_cF → IupSetFocus(txt_search)
    else → IUP_DEFAULT
```

## 快捷键传播

IUP 键盘事件从子控件向父控件传播。列表的 `list_k_any_cb`（Ctrl+Z/Y/DEL）返回 `IUP_IGNORE` 阻止传播；未识别的键返回 `IUP_DEFAULT` 使事件继续传播到对话框的 `dlg_k_any_cb`。

## 不做的事

- 不新增翻译条目
- 不修改 Lua 配置
- 列表级 `K_ANY` 保持不变
