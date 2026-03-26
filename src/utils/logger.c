#include "utils/logger.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <time.h>

// 全局日志文件指针
static FILE *G_log_file = NULL;
// 全局日志级别
static LogLevel G_log_level = LOG_LEVEL_INFO;

// 将日志级别转换为字符串
static const char *level_to_string(LogLevel level)
{
    switch (level)
    {
    case LOG_LEVEL_DEBUG:
        return "DEBUG";
    case LOG_LEVEL_INFO:
        return "INFO ";
    case LOG_LEVEL_WARN:
        return "WARN ";
    case LOG_LEVEL_ERROR:
        return "ERROR";
    default:
        return "UNKN ";
    }
}

// 获取当前时间戳
static void get_timestamp(char *buffer, size_t size)
{
    time_t now = time(NULL);
    struct tm *tm_info = localtime(&now);
    strftime(buffer, size, "%Y-%m-%d %H:%M:%S", tm_info);
}

// 写入日志
static void log_write(LogLevel level, const char *fmt, va_list args)
{
    if (level < G_log_level)
        return;

    char timestamp[64];
    get_timestamp(timestamp, sizeof(timestamp));

    char message[1024];
    vsnprintf(message, sizeof(message), fmt, args);

    const char *level_str = level_to_string(level);

    if (G_log_file)
    {
        fprintf(G_log_file, "[%s] [%s] %s\n", timestamp, level_str, message);
        fflush(G_log_file);
    }

    fprintf(stdout, "[%s] [%s] %s\n", timestamp, level_str, message);
}

// 初始化日志系统
void log_init(const char *log_file, LogLevel level)
{
    if (log_file)
    {
        G_log_file = fopen(log_file, "a");
        if (!G_log_file)
        {
            fprintf(stderr, "Failed to open log file: %s\n", log_file);
        }
    }
    G_log_level = level;
}

// 销毁日志系统
void log_destroy(void)
{
    if (G_log_file)
    {
        fclose(G_log_file);
        G_log_file = NULL;
    }
}

// 设置日志级别
void log_set_level(LogLevel level)
{
    G_log_level = level;
}

// 调试日志
void log_debug(const char *fmt, ...)
{
    va_list args;
    va_start(args, fmt);
    log_write(LOG_LEVEL_DEBUG, fmt, args);
    va_end(args);
}

// 信息日志
void log_info(const char *fmt, ...)
{
    va_list args;
    va_start(args, fmt);
    log_write(LOG_LEVEL_INFO, fmt, args);
    va_end(args);
}

// 警告日志
void log_warn(const char *fmt, ...)
{
    va_list args;
    va_start(args, fmt);
    log_write(LOG_LEVEL_WARN, fmt, args);
    va_end(args);
}

// 错误日志
void log_error(const char *fmt, ...)
{
    va_list args;
    va_start(args, fmt);
    log_write(LOG_LEVEL_ERROR, fmt, args);
    va_end(args);
}