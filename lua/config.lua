-- PathEditor 配置文件
-- 用于热更新 UI 参数，无需重新编译即可调整界面

local config = {
    -- 应用程序信息
    app = {
        name = "PathEditor",
        name_readonly = "PathEditor (Read-only)"
    },

    -- 对话框设置
    dialog = {
        size = "800x800",
        minsize = "800x800",
        select_dir = "Select Directory"
    },

    -- 备份设置
    backup = {
        dir = "",  -- 默认备份目录，留空使用 %APPDATA%/PathEditor/backups/
    },

    -- 列表控件设置
    list = {
        item_padding = "5x5",
        backcolor = "255 255 255"
    },

    -- 按钮设置（使用英文原文，供 gettext 翻译）
    button = {
        rastersize = "100x32",
        new = "New",
        edit = "Edit",
        browse = "Browse",
        del = "Delete",
        up = "Move Up",
        down = "Move Down",
        clean = "Clean Invalid",
        import = "Import",
        export = "Export",
        ok = "OK",
        cancel = "Cancel",
        help = "Help"
    },

    -- 标签文本（使用英文原文，供 gettext 翻译）
    label = {
        title = "Environment Variable Editor:",
        search_placeholder = "Search...",
        tab_sys = "System Variables",
        tab_user = "User Variables",
        export_title = "Export PATH",
        import_title = "Import PATH"
    },

    -- 布局设置
    layout = {
        vbox_gap = "5",
        vbox_margin = "0x0",
        vbox_all_margin = "10x10",
        vbox_all_gap = "5",
        hbox_gap = "10",
        hbox_margin = "10x10",
        hbox_alignment = "ACENTER"
    },

    -- 状态栏（使用英文原文，供 gettext 翻译）
    status = {
        normal = "Status: Ready",
        readonly = "Status: Read-only (No admin)",
        saving = "Status: Saving...",
        saved = "Status: Saved",
        error = "Status: Error",
        deleted = "Status: Deleted",
        loaded = "Status: Loaded",
        drag_folder_only = "Tip: Only folders can be added to PATH",
        admin_warning = "No admin rights. You can only view and export PATH."
    },

    -- 语言选择对话框
    language = {
        dialog_title = "Language",
        label = "Language",
        option_cn = "Chinese (Simplified)",
        option_en = "English",
        dialog_size = "250x150",
        list_size = "200x",
        margin = "15x15",
        gap = "10"
    },

    -- 输入对话框
    input_dialog = {
        text_size = "500x",
        margin = "15x15",
        gap = "10"
    }
}

return config
