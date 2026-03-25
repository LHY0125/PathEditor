-- PathEditor 配置文件
-- 用于热更新 UI 参数，无需重新编译即可调整界面

local config = {
    -- 应用程序信息
    app = {
        name = "PathEditor",
        name_readonly = "PathEditor (只读模式)"
    },

    -- 对话框设置
    dialog = {
        size = "800x800",
        minsize = "800x800",
        select_dir = "选择目录"
    },

    -- 列表控件设置
    list = {
        item_padding = "5x5",
        backcolor = "255 255 255"
    },

    -- 按钮设置
    button = {
        rastersize = "100x32",
        new = "新建(N)",
        edit = "编辑(E)",
        browse = "浏览(B)...",
        del = "删除(D)",
        up = "上移(U)",
        down = "下移(O)",
        clean = "一键清理",
        ok = "确定",
        cancel = "取消",
        help = "帮助(?)"
    },

    -- 标签文本
    label = {
        title = "环境变量编辑器:",
        search_placeholder = "输入关键词搜索...",
        tab_sys = "系统变量 (System)",
        tab_user = "用户变量 (User)"
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

    -- 状态栏
    status = {
        normal = "状态: 就绪",
        readonly = "状态: ⚠️ 只读模式 (无管理员权限)",
        saving = "状态: 保存中...",
        saved = "状态: ✓ 保存成功",
        error = "状态: ✗ 保存失败",
        deleted = "状态: 已删除选中项",
        loaded = "状态: 已加载系统和用户变量",
        drag_folder_only = "提示: 只能拖拽文件夹添加到 PATH"
    }
}

return config