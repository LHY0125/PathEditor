cmake_minimum_required(VERSION 3.10)
project(PathEditor VERSION 3.0 LANGUAGES C)

# 启用资源编译器以处理 .rc 文件
enable_language(RC)

# 设置 C 标准
set(CMAKE_C_STANDARD 17)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF) # 禁用特定编译器的扩展（如 gnu17），强制使用标准 C17

# 定义源文件
set(SOURCES
    src/main.c
    src/utils.c
    src/registry.c
    src/ui.c
    src/globals.c
    src/ui_utils.c
    src/cb_edit.c
    src/cb_file.c
    src/cb_main.c
    ico/resources.rc
)

# 创建 GUI 可执行文件（WIN32 属性会自动添加 -mwindows 参数）
add_executable(${PROJECT_NAME} WIN32 ${SOURCES})

# 添加宏定义
target_compile_definitions(${PROJECT_NAME} PRIVATE
    _WIN32
    UNICODE
    _UNICODE
)

# 添加编译选项
if(CMAKE_C_COMPILER_ID MATCHES "GNU|Clang")
    target_compile_options(${PROJECT_NAME} PRIVATE
        -Wall
        -O2
        -fexec-charset=UTF-8
    )
endif()

# 设置头文件搜索路径
target_include_directories(${PROJECT_NAME} PRIVATE
    ${CMAKE_SOURCE_DIR}/include
    ${CMAKE_SOURCE_DIR}/libs/iup-3.31_Win64_dllw6_lib/include
)

# 设置库文件搜索路径
target_link_directories(${PROJECT_NAME} PRIVATE
    ${CMAKE_SOURCE_DIR}/libs/iup-3.31_Win64_dllw6_lib
)

# 链接所需库
target_link_libraries(${PROJECT_NAME} PRIVATE
    iup
    iupcd
    gdi32
    comdlg32
    comctl32
    uuid
    ole32
    advapi32
)

# 编译完成后，将 DLL 文件复制到构建输出目录
file(GLOB IUP_DLLS "${CMAKE_CURRENT_SOURCE_DIR}/libs/iup-3.31_Win64_dllw6_lib/*.dll")
add_custom_command(TARGET ${PROJECT_NAME} POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different
    ${IUP_DLLS}
    "$<TARGET_FILE_DIR:${PROJECT_NAME}>"
    COMMENT "Copying DLLs to build directory..."
)
