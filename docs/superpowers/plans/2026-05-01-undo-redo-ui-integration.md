# 撤销/重做 UI 集成 — 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将已完成的撤销/重做后端逻辑暴露到 UI 层——添加撤销/重做按钮及 Ctrl+Z/Ctrl+Y 快捷键

**Architecture:** 在现有 MVC 架构上，新增两个按钮回调（`btn_undo_cb`/`btn_redo_cb`），它们调用 `undo_redo.c` 中已有的 `undo()`/`redo()` 函数，然后同步 UI 列表。Ctrl+Z/Y 快捷键在 `list_k_any_cb` 中检测并分发到对应的按钮回调。

**Tech Stack:** C17, IUP GUI, GCC/MinGW-w64

---

## 文件结构

| 操作 | 文件 | 职责 |
|------|------|------|
| 修改 | `include/utils/ui_constants.h` | 新增 2 个按钮名称常量 |
| 修改 | `include/controller/callbacks.h` | 声明 `btn_undo_cb` / `btn_redo_cb` |
| 修改 | `src/controller/callbacks_nav.c` | 实现 `btn_undo_cb` / `btn_redo_cb`，`list_k_any_cb` 增加 Ctrl+Z/Y |
| 修改 | `src/ui/main_window.c` | 创建撤销/重做按钮，加入布局，绑定回调 |
| 修改 | `lua/config.lua` | 新增 `button.undo` / `button.redo` 配置 |
| 修改 | `po/zh_CN.po` | 新增 "Undo"→"撤销" / "Redo"→"重做" 翻译 |
| 修改 | `po/en_US.po` | 新增 "Undo"→"Undo" / "Redo"→"Redo" 翻译 |
| 修改 | `po/messages.pot` | 新增 msgid 条目 |
| 重新生成 | `locale/zh_CN/LC_MESSAGES/zh_CN.mo` | msgfmt 编译 |
| 重新生成 | `locale/en_US/LC_MESSAGES/en_US.mo` | msgfmt 编译 |

---

### Task 1: 添加 UI 常量

**Files:**
- Modify: `include/utils/ui_constants.h`

- [ ] **Step 1: 在 ui_constants.h 添加按钮常量**

在 `CTRL_BTN_LANG` 之后、`#endif` 之前添加：

```c
// 撤销/重做按钮
#define CTRL_BTN_UNDO "BTN_UNDO"
#define CTRL_BTN_REDO "BTN_REDO"
```

- [ ] **Step 2: 提交**

```bash
git add include/utils/ui_constants.h
git commit -m "feat(undo): 添加撤销/重做按钮的 UI 常量"
```

---

### Task 2: 声明回调函数

**Files:**
- Modify: `include/controller/callbacks.h`

- [ ] **Step 1: 在 callbacks.h 声明新回调函数**

在 `btn_lang_cb` 声明之后、搜索回调声明之前添加：

```c
// 撤销/重做回调
int btn_undo_cb(Ihandle *self);
int btn_redo_cb(Ihandle *self);
```

- [ ] **Step 2: 提交**

```bash
git add include/controller/callbacks.h
git commit -m "feat(undo): 声明撤销/重做按钮回调函数"
```

---

### Task 3: 实现撤销/重做回调逻辑

**Files:**
- Modify: `src/controller/callbacks_nav.c`

> 需要新增的 include：`#include "core/app_context.h"`（已存在）、`#include "ui/ui_utils.h"`（已存在）

- [ ] **Step 1: 在 callbacks_nav.c 头部添加 app_context 头文件引用**

检查第 3 行已有 `#include "core/undo_redo.h"`，第 10 行已有 `#include "utils/ui_constants.h"`。确认无需添加新的 include。

- [ ] **Step 2: 添加刷新撤销/重做按钮状态的辅助函数**

在文件末尾 `list_k_any_cb` 之前添加：

```c
// 刷新撤销/重做按钮的启用状态
static void refresh_undo_redo_buttons(Ihandle *dlg)
{
    AppContext *ctx = get_app_context_from_dlg(dlg);
    if (!ctx || !ctx->undo_redo_mgr)
        return;

    Ihandle *btn_undo = IupGetDialogChild(dlg, CTRL_BTN_UNDO);
    Ihandle *btn_redo = IupGetDialogChild(dlg, CTRL_BTN_REDO);

    if (btn_undo)
        IupSetAttribute(btn_undo, "ACTIVE", can_undo(ctx->undo_redo_mgr) ? "YES" : "NO");
    if (btn_redo)
        IupSetAttribute(btn_redo, "ACTIVE", can_redo(ctx->undo_redo_mgr) ? "YES" : "NO");
}
```

- [ ] **Step 3: 实现 btn_undo_cb**

在 `refresh_undo_redo_buttons` 之后添加：

```c
int btn_undo_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    AppContext *ctx = get_app_context_from_dlg(dlg);
    if (!ctx || !ctx->undo_redo_mgr)
        return IUP_DEFAULT;

    if (!can_undo(ctx->undo_redo_mgr))
        return IUP_DEFAULT;

    undo(ctx->undo_redo_mgr, &ctx->sys_paths, &ctx->user_paths);

    Ihandle *list_sys = IupGetDialogChild(dlg, CTRL_LIST_SYS);
    Ihandle *list_user = IupGetDialogChild(dlg, CTRL_LIST_USER);
    sync_string_list_to_ui(list_sys, &ctx->sys_paths);
    sync_string_list_to_ui(list_user, &ctx->user_paths);

    Ihandle *lbl_status = IupGetDialogChild(dlg, CTRL_LBL_STATUS);
    if (lbl_status)
        IupSetAttribute(lbl_status, "TITLE", _("Undo completed"));

    refresh_undo_redo_buttons(dlg);
    return IUP_DEFAULT;
}
```

- [ ] **Step 4: 实现 btn_redo_cb**

在 `btn_undo_cb` 之后添加：

```c
int btn_redo_cb(Ihandle *self)
{
    Ihandle *dlg = IupGetDialog(self);
    AppContext *ctx = get_app_context_from_dlg(dlg);
    if (!ctx || !ctx->undo_redo_mgr)
        return IUP_DEFAULT;

    if (!can_redo(ctx->undo_redo_mgr))
        return IUP_DEFAULT;

    redo(ctx->undo_redo_mgr, &ctx->sys_paths, &ctx->user_paths);

    Ihandle *list_sys = IupGetDialogChild(dlg, CTRL_LIST_SYS);
    Ihandle *list_user = IupGetDialogChild(dlg, CTRL_LIST_USER);
    sync_string_list_to_ui(list_sys, &ctx->sys_paths);
    sync_string_list_to_ui(list_user, &ctx->user_paths);

    Ihandle *lbl_status = IupGetDialogChild(dlg, CTRL_LBL_STATUS);
    if (lbl_status)
        IupSetAttribute(lbl_status, "TITLE", _("Redo completed"));

    refresh_undo_redo_buttons(dlg);
    return IUP_DEFAULT;
}
```

- [ ] **Step 5: 修改 list_k_any_cb 添加 Ctrl+Z / Ctrl+Y 检测**

将现有的 `list_k_any_cb` 函数（第 149-164 行）替换为：

```c
int list_k_any_cb(Ihandle *self, int c)
{
    if (IupGetInt(self, "ACTIVE") == 0)
        return IUP_DEFAULT;

    if (c == K_cZ)  // Ctrl+Z 撤销
    {
        btn_undo_cb(self);
        return IUP_IGNORE;
    }

    if (c == K_cY)  // Ctrl+Y 重做
    {
        btn_redo_cb(self);
        return IUP_IGNORE;
    }

    if (c == K_DEL)  // DEL 键
    {
        btn_del_cb(self);
        return IUP_IGNORE;
    }
    return IUP_DEFAULT;
}
```

- [ ] **Step 6: 提交**

```bash
git add src/controller/callbacks_nav.c
git commit -m "feat(undo): 实现撤销/重做按钮回调及 Ctrl+Z/Y 快捷键"
```

---

### Task 4: 在 UI 中添加撤销/重做按钮

**Files:**
- Modify: `src/ui/main_window.c`

- [ ] **Step 1: 在 main_window.c 头部添加 app_context 引用**

检查当前 include。如果尚未包含 `<iupkey.h>`，需要在 `#include <iup.h>` 之后添加（但 iup.h 通常已包含 iupkey.h）。确认无需新增 include。

- [ ] **Step 2: 创建撤销/重做按钮并绑定回调**

在 `btn_down` 按钮创建之后（第 67 行附近）、`btn_clean` 之前，添加新按钮：

```c
Ihandle *btn_undo = IupButton(_(lua_config_get_string("button", "undo")), NULL);
IupSetAttribute(btn_undo, "NAME", CTRL_BTN_UNDO);
IupSetAttribute(btn_undo, "ACTIVE", "NO");  // 初始无操作可撤销

Ihandle *btn_redo = IupButton(_(lua_config_get_string("button", "redo")), NULL);
IupSetAttribute(btn_redo, "NAME", CTRL_BTN_REDO);
IupSetAttribute(btn_redo, "ACTIVE", "NO");  // 初始无操作可重做
```

- [ ] **Step 3: 设置按钮回调**

在现有按钮回调设置区域（第 84-93 行），`btn_down` 回调之后添加：

```c
IupSetCallback(btn_undo, "ACTION", (Icallback)btn_undo_cb);
IupSetCallback(btn_redo, "ACTION", (Icallback)btn_redo_cb);
```

- [ ] **Step 4: 设置按钮大小**

在按钮大小设置区域（第 96-106 行），`btn_export` 之后添加：

```c
IupSetAttribute(btn_undo, "RASTERSIZE", btn_size);
IupSetAttribute(btn_redo, "RASTERSIZE", btn_size);
```

- [ ] **Step 5: 将按钮加入垂直布局**

修改 `vbox_btns` 布局（第 109-118 行），在 `btn_up, btn_down` 之后、`NULL` 之前加入 `btn_undo, btn_redo`：

```c
Ihandle *vbox_btns = IupVbox(
    btn_new, btn_edit, btn_browse, btn_del,
    IupFill(),
    btn_clean,
    IupFill(),
    btn_import, btn_export,
    btn_up, btn_down,
    btn_undo, btn_redo,
    NULL);
```

- [ ] **Step 6: 在 refresh_main_window_ui 中添加按钮文本刷新**

在 `refresh_main_window_ui` 函数的 `SET_CHILD_TITLE` 宏调用区域（第 198-209 行），`CTRL_BTN_EXPORT` 之后添加：

```c
SET_CHILD_TITLE(CTRL_BTN_UNDO, "undo");
SET_CHILD_TITLE(CTRL_BTN_REDO, "redo");
```

- [ ] **Step 7: 提交**

```bash
git add src/ui/main_window.c
git commit -m "feat(undo): 在 UI 中添加撤销/重做按钮并集成布局"
```

---

### Task 5: 更新 Lua 配置

**Files:**
- Modify: `lua/config.lua`

- [ ] **Step 1: 在 config.lua 的 button 表中添加 undo 和 redo**

在 `button` 表的 `help = "Help"` 之后添加：

```lua
undo = "Undo",
redo = "Redo",
```

- [ ] **Step 2: 提交**

```bash
git add lua/config.lua
git commit -m "feat(undo): 在 Lua 配置中添加撤销/重做按钮文本"
```

---

### Task 6: 更新翻译文件

**Files:**
- Modify: `po/zh_CN.po`
- Modify: `po/en_US.po`
- Modify: `po/messages.pot`

- [ ] **Step 1: 在 zh_CN.po 中添加翻译条目**

在 "Clean Invalid" 条目之后、其他条目之前插入：

```
#: src/ui/main_window.c
msgid "Undo"
msgstr "撤销"

#: src/ui/main_window.c
msgid "Redo"
msgstr "重做"

```

以及状态栏消息：

```
#: src/controller/callbacks_nav.c
msgid "Undo completed"
msgstr "已撤销"

#: src/controller/callbacks_nav.c
msgid "Redo completed"
msgstr "已重做"

```

- [ ] **Step 2: 在 en_US.po 中添加翻译条目**

```
#: src/ui/main_window.c
msgid "Undo"
msgstr "Undo"

#: src/ui/main_window.c
msgid "Redo"
msgstr "Redo"

#: src/controller/callbacks_nav.c
msgid "Undo completed"
msgstr "Undo completed"

#: src/controller/callbacks_nav.c
msgid "Redo completed"
msgstr "Redo completed"

```

- [ ] **Step 3: 在 messages.pot 中添加翻译模板条目**

```
#: src/ui/main_window.c
msgid "Undo"
msgstr ""

#: src/ui/main_window.c
msgid "Redo"
msgstr ""

#: src/controller/callbacks_nav.c
msgid "Undo completed"
msgstr ""

#: src/controller/callbacks_nav.c
msgid "Redo completed"
msgstr ""

```

- [ ] **Step 4: 重新编译 .mo 文件**

```bash
cd D:/Code/doing_exercises/programs/PathEditor
msgfmt po/zh_CN.po -o locale/zh_CN/LC_MESSAGES/zh_CN.mo
msgfmt po/en_US.po -o locale/en_US/LC_MESSAGES/en_US.mo
```

- [ ] **Step 5: 提交**

```bash
git add po/zh_CN.po po/en_US.po po/messages.pot locale/zh_CN/LC_MESSAGES/zh_CN.mo locale/en_US/LC_MESSAGES/en_US.mo
git commit -m "feat(undo): 添加撤销/重做的中英文翻译"
```

---

### Task 7: 编译验证

- [ ] **Step 1: 编译项目**

```bash
cd D:/Code/doing_exercises/programs/PathEditor
cmake --build build
```

预期输出：编译成功，无错误无警告。

- [ ] **Step 2: 功能验证清单**
  1. 启动程序，确认出现「撤销」「重做」按钮
  2. 新建一条路径 → 撤销按钮变可用 → 点击撤销 → 路径消失 → 重做按钮变可用
  3. 按 Ctrl+Z 撤销 → Ctrl+Y 重做
  4. 删除一条路径 → 撤销恢复 → 重做再次删除
  5. 无历史记录时，撤销/重做按钮灰色不可点击
  6. 语言切换后按钮文本正确切换
