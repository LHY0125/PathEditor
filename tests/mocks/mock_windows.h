/*
 * mock_windows.h
 * Windows API Mock 头文件
 * 用于单元测试中模拟 Windows API
 */
#ifndef MOCK_WINDOWS_H
#define MOCK_WINDOWS_H

#ifdef TESTING

#include <windows.h>
#include <wchar.h>

/* Mock 计数器，用于验证调用 */
extern int mock_MultiByteToWideChar_call_count;
extern int mock_WideCharToMultiByte_call_count;

/* 设置 Mock 返回值 */
void mock_set_MultiByteToWideChar_return(int ret);
void mock_set_WideCharToMultiByte_return(int ret);

/* Mock MultiByteToWideChar */
int mock_MultiByteToWideChar(
    UINT CodePage,
    DWORD dwFlags,
    LPCSTR lpMultiByteStr,
    int cbMultiByte,
    LPWSTR lpWideCharStr,
    int cchWideChar);

/* Mock WideCharToMultiByte */
int mock_WideCharToMultiByte(
    UINT CodePage,
    DWORD dwFlags,
    LPCWSTR lpWideCharStr,
    int cchWideChar,
    LPSTR lpMultiByteStr,
    int cbMultiByte,
    LPCSTR lpDefaultChar,
    LPBOOL lpUsedDefaultChar);

/* 替换宏（在测试源文件中定义） */
#ifdef REPLACE_WINDOWS_API
    #define MultiByteToWideChar mock_MultiByteToWideChar
    #define WideCharToMultiByte mock_WideCharToMultiByte
#endif

#else
/* 非测试模式下为空 */
#define REPLACE_WINDOWS_API 0

#endif /* TESTING */

#endif /* MOCK_WINDOWS_H */
