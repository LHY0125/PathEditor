已修复，随 **v5.1.5** 发布（[Release](https://github.com/LHY0125/PathEditor/releases/tag/v5.1.5)）。

### 修复内容

`env list --help` 的措辞由

```
列出环境变量元数据（不含明文）
```

改为

```
列出环境变量元数据（敏感变量不含明文，非敏感变量给出可能截断的 preview）
```

**只改文案，不改行为。** 核实时的更正：你在 issue 里提到的「方向 2：让 preview 真的做截断」**其实早已实现** —— `core/src/env_var.rs` 的 `sanitize_preview` 会对超过 `PREVIEW_MAX_CHARS` 的值截断并加 `…`。你举的两个例子（`.cargo` 路径、aliyun 镜像 URL）都短于阈值，所以看起来"完整"，那是长度不够、不是没有截断机制。因此本版只做方向 1（改文案）。

### 关联的加固

同一波里，dry-run 预览路径还补了敏感值打码与控制字符净化（`sanitize_preview` 的剔除范围扩到全部 C0 控制字符 + DEL，覆盖 ESC）。虽然你这条只讲 `list`，但预览不新开明文出口、净化 ANSI 转义是同一类问题的收口。

关闭本条。
