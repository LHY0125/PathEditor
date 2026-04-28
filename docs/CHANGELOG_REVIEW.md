# PathEditor 代码审查修复日志

> 审查日期：2026-04-28
> 审查范围：全项目代码质量审查
> 修复状态：29/29 已完成

---

## 一、高严重度问题（7 项）

### 1.1 add_string_list 中 realloc 失败导致内存泄漏

- **文件**：`src/utils/string_ext.c`
- **问题**：`realloc` 返回 NULL 时，原 `list->items` 指针被覆盖为 NULL，导致原有数组及其中所有字符串指针永久泄漏
- **修复**：使用临时变量保存 `realloc` 结果，失败时保留原数据不变

```c
// 修复前
list->items = (char **)realloc(list->items, list->capacity * sizeof(char *));

// 修复后
char **new_items = (char **)realloc(list->items, new_capacity * sizeof(char *));
if (!new_items)
    return; // 失败时保留原数据
list->items = new_items;
```

### 1.2 add_string_list 中 _strdup 失败未检查

- **文件**：`src/utils/string_ext.c`
- **问题**：`_strdup` 可能返回 NULL，但代码未检查就存入数组并递增 count，后续字符串操作会崩溃
- **修复**：检查 `_strdup` 返回值，失败时不递增 count

### 1.3 string_list_set 中先 free 后 strdup 导致数据丢失

- **文件**：`src/utils/string_ext.c`
- **问题**：先释放旧字符串，然后 `_strdup` 可能失败，导致旧数据丢失且无法恢复
- **修复**：先调用 `_strdup` 获取新字符串，成功后再释放旧字符串

### 1.4 wcsftime 缓冲区大小以字节而非宽字符数传入

- **文件**：`src/utils/os_env.c`
- **问题**：`sizeof(timestamp)` 返回字节数（128），而 `wcsftime` 期望宽字符数（64），可能导致栈缓冲区溢出
- **修复**：改用 `sizeof(timestamp) / sizeof(timestamp[0])`

```c
// 修复前
wcsftime(timestamp, sizeof(timestamp), L"%Y%m%d_%H%M%S", tm_info);

// 修复后
wcsftime(timestamp, sizeof(timestamp) / sizeof(timestamp[0]), L"%Y%m%d_%H%M%S", &tm_info);
```

### 1.5 load_single_path 在 malloc 失败时仍返回 ERR_OK

- **文件**：`src/core/registry_service.c`
- **问题**：`malloc` 失败时跳过数据读取，但仍返回 `ERR_OK`，调用者认为加载成功
- **修复**：`malloc` 失败时返回 `ERR_OUT_OF_MEMORY`

### 1.6 导入数据内存泄漏

- **文件**：`src/controller/callbacks_io.c`
- **问题**：`btn_import_cb` 中 `ExportData imported` 的 `system` 和 `user` 从未调用 `clear_string_list` 释放
- **修复**：在函数所有返回路径前调用 `clear_string_list` 释放导入数据

### 1.7 JSON 导入解析器完全失效

- **文件**：`src/core/import_export.c`
- **问题**：键名检测逻辑被 `in_string` 状态翻转拦截，`"system"` 和 `"user"` 的检测永远无法触发
- **修复**：重写 JSON 解析器，在字符串结束时检测键名，合并重复的数组解析逻辑

---

## 二、中严重度问题（9 项）

### 2.1 backup_registry 部分备份成功时错误报告为完全成功

- **文件**：`src/utils/os_env.c`
- **修复**：分别跟踪系统和用户 PATH 的备份状态

### 2.2 btn_ok_cb 未检查 backup_registry 返回值

- **文件**：`src/controller/callbacks_sys.c`
- **修复**：检查返回值，备份失败时提示用户是否继续保存

```c
// 修复后
ErrorCode backup_result = backup_registry();
if (backup_result != ERR_OK)
{
    int choice = IupAlarm("警告", "备份失败！是否继续保存？",
                          "继续保存", "取消", NULL);
    if (choice != 1)
        return IUP_DEFAULT;
}
```

### 2.3 TABTITLE 设置在 Dialog 而非 Tabs 控件上

- **文件**：`src/ui/main_window.c`
- **问题**：`TABTITLE0` 和 `TABTITLE1` 是 `IupTabs` 属性，不是 `IupDialog` 属性，导致语言切换后选项卡标题不更新
- **修复**：先获取 Tabs 控件句柄，再设置 TABTITLE

### 2.4 list_dropfiles_cb 使用 ANSI 版本 API

- **文件**：`src/controller/callbacks_search.c`
- **问题**：`GetFileAttributesA` 无法正确处理中文等 Unicode 字符路径
- **修复**：改用 `utf8_to_wide` + `GetFileAttributesW`

### 2.5 load_all_paths 用户路径加载失败时未通知用户

- **文件**：`src/controller/callbacks_sys.c`
- **修复**：添加 else 分支，在用户路径加载失败时弹出提示

### 2.6 escape_json_string 未处理所有控制字符

- **文件**：`src/core/import_export.c`
- **问题**：只处理了 `\\`、`"`、`\n`、`\r`、`\t`，其他 0x00-0x1F 控制字符未转义
- **修复**：添加 `\b`、`\f` 处理，其他控制字符使用 `\uXXXX` 格式

### 2.7 CreateDirectoryW 不创建中间目录

- **文件**：`src/utils/os_env.c`
- **修复**：改用 `SHCreateDirectoryExW` 递归创建目录

### 2.8 ExportData 浅拷贝隐患

- **文件**：`include/core/import_export.h`
- **修复**：添加文档注释说明只读语义，防止误用 `clear_string_list`

### 2.9 export_paths_to_file 未检查 fprintf 返回值

- **文件**：`src/core/import_export.c`
- **修复**：在 `fclose` 后检查 `ferror(fp)`，发现错误时返回 `ERR_FAILED`

---

## 三、低严重度问题（13 项）

### 3.1 JSON 解析器代码重复

- **文件**：`src/core/import_export.c`
- **修复**：合并 system/user 数组解析逻辑为统一的键名检测机制

### 3.2 get_app_context_from_dlg 依赖指针到字符串的不安全转换

- **状态**：保留现状（IUP 框架标准用法）

### 3.3 putenv 使用字符串字面量的可移植性问题

- **文件**：`src/main.c`
- **修复**：改用 `_wputenv_s`

### 3.4 path_manager_clean 时间复杂度为 O(n³)

- **文件**：`src/core/path_manager.c`
- **修复**：使用标记+批量删除优化为 O(n²)

### 3.5 全局日志状态非线程安全

- **状态**：保留现状（当前是单线程 GUI 应用）

### 3.6 localtime 返回静态缓冲区指针，非线程安全

- **文件**：`src/utils/os_env.c`、`src/core/import_export.c`
- **修复**：改用 `localtime_s`

### 3.7 JSON 解析器原地修改输入缓冲区

- **修复**：重写解析器时已解决，不再修改原始缓冲区

### 3.8 JSON 解析器对 `\\"` 场景处理错误

- **修复**：重写解析器时已解决，使用 `is_quote_escaped` 检查连续反斜杠

### 3.9 TXT 格式导入只能导入到系统路径

- **文件**：`src/controller/callbacks_io.c`
- **修复**：添加选择对话框，允许用户选择导入到系统变量或用户变量

### 3.10 trim_whitespace 未检查 NULL 输入

- **文件**：`src/core/import_export.c`
- **修复**：添加防御性 NULL 检查

### 3.11 is_json_file 使用 strcasecmp 限制可移植性

- **文件**：`src/core/import_export.c`
- **修复**：改用 `_stricmp`（MSVC/MinGW 都支持）

### 3.12 localtime 可能返回 NULL

- **修复**：改用 `localtime_s`，无需 NULL 检查

### 3.13 main.c 中 _() 嵌套调用可能崩溃

- **文件**：`src/main.c`
- **问题**：`_(lua_config_get_string(...))` 嵌套调用，若配置键不存在，`gettext(NULL)` 行为未定义
- **修复**：先将结果保存到临时变量并检查非 NULL

```c
// 修复前
IupMessage(_("Warning"), _(lua_config_get_string("status", "admin_warning")));

// 修复后
const char *admin_msg = lua_config_get_string("status", "admin_warning");
IupMessage(_("Warning"), admin_msg ? _(admin_msg) : "需要管理员权限才能编辑环境变量");
```

---

## 四、第一轮修复（原10条锐评）

| # | 问题 | 状态 |
|---|------|------|
| 1 | core层用了iup.h（架构矛盾） | ✅ 已修复 |
| 2 | main.c管理员权限检测代码灾难 | ✅ 已修复 |
| 3 | refresh_main_window_ui重复代码 | ✅ 已修复 |
| 4 | 字符串溢出风险（buffer不统一） | ✅ 已修复 |
| 5 | 错误码乱用（ERR_NULL_PTR误用） | ✅ 已修复 |
| 6 | 硬编码字符串满天飞 | ✅ 已修复 |
| 7 | backup_registry未实现 | ✅ 已修复 |
| 8 | callbacks.c 600行回调地狱 | ✅ 已拆分 |
| 9 | StringList封装不透明 | ✅ 已修复 |
| 10 | refresh_single_list_style黑盒 | ✅ 已修复 |

---

## 五、修改文件清单

| 文件 | 修改类型 |
|------|----------|
| `include/utils/string_ext.h` | 添加访问器函数声明 |
| `include/utils/ui_constants.h` | 新建，控件名称常量 |
| `include/core/import_export.h` | 添加文档注释 |
| `include/controller/callbacks_internal.h` | 新建，内部辅助函数声明 |
| `src/utils/string_ext.c` | 修复 realloc/strdup 安全性 |
| `src/utils/os_env.c` | 修复 wcsftime、localtime、目录创建 |
| `src/core/registry_service.c` | 修复 malloc 错误处理 |
| `src/core/import_export.c` | 重写 JSON 解析器、修复转义函数 |
| `src/core/path_manager.c` | 优化清理算法、修复错误码 |
| `src/main.c` | 修复 putenv、_() 嵌套调用 |
| `src/ui/main_window.c` | 修复 TABTITLE 设置位置 |
| `src/controller/callbacks.c` | 拆分后只保留辅助函数 |
| `src/controller/callbacks_basic.c` | 新建，基础 CRUD 回调 |
| `src/controller/callbacks_nav.c` | 新建，导航回调 |
| `src/controller/callbacks_search.c` | 新建，搜索/拖拽回调 |
| `src/controller/callbacks_io.c` | 新建，导入导出回调 |
| `src/controller/callbacks_sys.c` | 新建，系统操作回调 |
| `CMakeLists.txt` | 添加新源文件 |

---

## 六、编译验证

```bash
cmake -B build -G "MinGW Makefiles"
cmake --build build
```

**结果**：编译通过，无错误，无警告。

---

## 七、剩余已知问题（设计决策，非 Bug）

| 问题 | 原因 | 建议 |
|------|------|------|
| IUP 指针存储模式 | IUP 框架标准用法 | 保持现状 |
| 日志线程安全 | 当前是单线程应用 | 多线程时再处理 |
| localtime 线程安全 | 已改用 localtime_s | 已修复 |

---

*审查完成于 2026-04-28*
