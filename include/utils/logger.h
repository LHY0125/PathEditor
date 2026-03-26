#ifndef LOGGER_H
#define LOGGER_H

// 日志级别
typedef enum {
    LOG_LEVEL_DEBUG,    // 调试日志级别
    LOG_LEVEL_INFO,     // 信息日志级别
    LOG_LEVEL_WARN,     // 警告日志级别
    LOG_LEVEL_ERROR     // 错误日志级别
} LogLevel;

// 初始化日志系统
void log_init(const char *log_file, LogLevel level);

// 销毁日志系统
void log_destroy(void);

// 日志函数
void log_debug(const char *fmt, ...);

// 信息日志函数
void log_info(const char *fmt, ...);

// 警告日志函数
void log_warn(const char *fmt, ...);

// 错误日志函数
void log_error(const char *fmt, ...);

// 设置日志级别
void log_set_level(LogLevel level);

#endif // LOGGER_H