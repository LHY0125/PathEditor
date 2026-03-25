#include "core/lua_config.h"
#include <lua.h>
#include <lualib.h>
#include <lauxlib.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static lua_State* G_L = NULL;
static int G_loaded = 0;
static const char* G_config_path = "lua/config.lua";

static const char* get_string_default(const char* section, const char* key)
{
    if (strcmp(section, "app") == 0)
    {
        if (strcmp(key, "name") == 0) return "PathEditor";
        if (strcmp(key, "name_readonly") == 0) return "PathEditor (只读模式)";
    }
    else if (strcmp(section, "dialog") == 0)
    {
        if (strcmp(key, "size") == 0) return "800x800";
        if (strcmp(key, "minsize") == 0) return "800x800";
        if (strcmp(key, "select_dir") == 0) return "选择目录";
    }
    else if (strcmp(section, "list") == 0)
    {
        if (strcmp(key, "item_padding") == 0) return "5x5";
        if (strcmp(key, "backcolor") == 0) return "255 255 255";
    }
    else if (strcmp(section, "button") == 0)
    {
        if (strcmp(key, "rastersize") == 0) return "100x32";
        if (strcmp(key, "new") == 0) return "新建(N)";
        if (strcmp(key, "edit") == 0) return "编辑(E)";
        if (strcmp(key, "browse") == 0) return "浏览(B)...";
        if (strcmp(key, "del") == 0) return "删除(D)";
        if (strcmp(key, "up") == 0) return "上移(U)";
        if (strcmp(key, "down") == 0) return "下移(O)";
        if (strcmp(key, "clean") == 0) return "一键清理";
        if (strcmp(key, "ok") == 0) return "确定";
        if (strcmp(key, "cancel") == 0) return "取消";
        if (strcmp(key, "help") == 0) return "帮助(?)";
    }
    else if (strcmp(section, "label") == 0)
    {
        if (strcmp(key, "title") == 0) return "环境变量编辑器:";
        if (strcmp(key, "search_placeholder") == 0) return "输入关键词搜索...";
        if (strcmp(key, "tab_sys") == 0) return "系统变量 (System)";
        if (strcmp(key, "tab_user") == 0) return "用户变量 (User)";
    }
    else if (strcmp(section, "layout") == 0)
    {
        if (strcmp(key, "vbox_gap") == 0) return "5";
        if (strcmp(key, "vbox_margin") == 0) return "0x0";
        if (strcmp(key, "vbox_all_margin") == 0) return "10x10";
        if (strcmp(key, "vbox_all_gap") == 0) return "5";
        if (strcmp(key, "hbox_gap") == 0) return "10";
        if (strcmp(key, "hbox_margin") == 0) return "10x10";
        if (strcmp(key, "hbox_alignment") == 0) return "ACENTER";
    }
    else if (strcmp(section, "status") == 0)
    {
        if (strcmp(key, "normal") == 0) return "状态: 就绪";
        if (strcmp(key, "readonly") == 0) return "状态: ⚠️ 只读模式 (无管理员权限)";
        if (strcmp(key, "saving") == 0) return "状态: 保存中...";
        if (strcmp(key, "saved") == 0) return "状态: ✓ 保存成功";
        if (strcmp(key, "error") == 0) return "状态: ✗ 保存失败";
        if (strcmp(key, "deleted") == 0) return "状态: 已删除选中项";
        if (strcmp(key, "loaded") == 0) return "状态: 已加载系统和用户变量";
        if (strcmp(key, "drag_folder_only") == 0) return "提示: 只能拖拽文件夹添加到 PATH";
    }
    return "";
}

int lua_config_init(void)
{
    if (G_L != NULL)
    {
        return 0;
    }

    G_L = luaL_newstate();
    if (G_L == NULL)
    {
        return -1;
    }

    luaL_openlibs(G_L);

    if (luaL_dofile(G_L, G_config_path) != LUA_OK)
    {
        const char* err = lua_tostring(G_L, -1);
        if (err)
        {
            fprintf(stderr, "[Lua Config] 加载配置文件失败: %s\n", err);
        }
        lua_settop(G_L, 0);
        G_loaded = 0;
        return 0;
    }

    lua_settop(G_L, 0);
    G_loaded = 1;
    return 0;
}

void lua_config_destroy(void)
{
    if (G_L != NULL)
    {
        lua_close(G_L);
        G_L = NULL;
    }
    G_loaded = 0;
}

const char* lua_config_get_string(const char* section, const char* key)
{
    if (G_L == NULL || section == NULL || key == NULL)
    {
        return get_string_default(section, key);
    }

    lua_getglobal(G_L, "config");
    if (!lua_istable(G_L, -1))
    {
        lua_settop(G_L, 0);
        return get_string_default(section, key);
    }

    lua_getfield(G_L, -1, section);
    if (!lua_istable(G_L, -1))
    {
        lua_settop(G_L, 0);
        return get_string_default(section, key);
    }

    lua_getfield(G_L, -1, key);
    if (!lua_isstring(G_L, -1))
    {
        lua_settop(G_L, 0);
        return get_string_default(section, key);
    }

    const char* value = lua_tostring(G_L, -1);
    lua_settop(G_L, 0);

    return value ? value : get_string_default(section, key);
}

int lua_config_get_int(const char* section, const char* key, int default_value)
{
    if (G_L == NULL || section == NULL || key == NULL)
    {
        return default_value;
    }

    lua_getglobal(G_L, "config");
    if (!lua_istable(G_L, -1))
    {
        lua_settop(G_L, 0);
        return default_value;
    }

    lua_getfield(G_L, -1, section);
    if (!lua_istable(G_L, -1))
    {
        lua_settop(G_L, 0);
        return default_value;
    }

    lua_getfield(G_L, -1, key);
    if (!lua_isnumber(G_L, -1))
    {
        lua_settop(G_L, 0);
        return default_value;
    }

    int value = (int)lua_tointeger(G_L, -1);
    lua_settop(G_L, 0);

    return value;
}

int lua_config_reload(void)
{
    if (G_L != NULL)
    {
        lua_close(G_L);
        G_L = NULL;
    }

    return lua_config_init();
}

int lua_config_is_loaded(void)
{
    return G_loaded;
}