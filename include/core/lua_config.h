#ifndef LUA_CONFIG_H
#define LUA_CONFIG_H

#include <lua.h>

// 初始化 Lua 配置系统
// 返回值: 0 成功, -1 失败
int lua_config_init(void);

// 销毁 Lua 配置系统
void lua_config_destroy(void);

// 获取字符串配置值
// section: 配置章节名 (如 "app", "dialog", "button")
// key: 配置键名 (如 "name", "size", "rastersize")
// 返回值: 配置值字符串, 失败时返回 NULL
const char* lua_config_get_string(const char* section, const char* key);

// 获取整型配置值
// section: 配置章节名
// key: 配置键名
// default_value: 默认值 (当配置不存在或转换失败时返回)
// 返回值: 配置值或默认值
int lua_config_get_int(const char* section, const char* key, int default_value);

// 重新加载配置文件
// 返回值: 0 成功, -1 失败
int lua_config_reload(void);

// 获取配置加载状态
// 返回值: 1 已加载, 0 未加载
int lua_config_is_loaded(void);

#endif // LUA_CONFIG_H