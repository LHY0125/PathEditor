cmake_minimum_required(VERSION 3.10)
project(PathEditor C)

# 设置 C 标准
set(CMAKE_C_STANDARD 17)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF) # 禁用特定编译器的扩展（如 gnu17），强制使用标准 C17

# 编译选项和宏定义
add_compile_options(-Wall -O2 -fexec-charset=UTF-8)
add_compile_definitions(_WIN32 UNICODE _UNICODE)

# 头文件包含目录
include_directories(
    ${CMAKE_CURRENT_SOURCE_DIR}/include
    ${CMAKE_CURRENT_SOURCE_DIR}/libs/iup-3.31_Win64_dllw6_lib/include
)

# 源代码文件
set(SOURCES
    src/main.c
    src/utils.c
    src/registry.c
    src/callbacks.c
    src/ui.c
    src/globals.c
)

# 资源文件
set(RESOURCES
    ico/resources.rc
)

# 库文件目录
link_directories(
    ${CMAKE_CURRENT_SOURCE_DIR}/libs/iup-3.31_Win64_dllw6_lib
)

# 创建 WIN32 可执行文件 (自动添加 -mwindows 并处理 .rc 资源文件)
add_executable(${PROJECT_NAME} WIN32 ${SOURCES} ${RESOURCES})

# 链接依赖库
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
add_custom_command(TARGET ${PROJECT_NAME} POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_directory
    "${CMAKE_CURRENT_SOURCE_DIR}/libs/iup-3.31_Win64_dllw6_lib/"
    "$<TARGET_FILE_DIR:${PROJECT_NAME}>"
    COMMENT "Copying DLLs to build directory..."
)
