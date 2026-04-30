# PathEditor 单元测试框架
cmake_minimum_required(VERSION 3.10)

# 测试子项目配置
project(PathEditorTests C)

# 设置 C 标准（与主项目一致）
set(CMAKE_C_STANDARD 17)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)

# 包含主项目的头文件路径
include_directories(
    ${CMAKE_SOURCE_DIR}/include
    ${CMAKE_SOURCE_DIR}/include/core
    ${CMAKE_SOURCE_DIR}/include/ui
    ${CMAKE_SOURCE_DIR}/include/controller
    ${CMAKE_SOURCE_DIR}/include/utils
    ${CMAKE_SOURCE_DIR}/libs/IUP/include
    ${CMAKE_SOURCE_DIR}/libs/lua/include
    ${CMAKE_SOURCE_DIR}/libs/gettext/include
)

# 获取 CMocka（如果系统没有则下载）
include(FetchContent)
FetchContent_Declare(
    cmocka
    GIT_REPOSITORY https://git.cryptomilk.org/projects/cmocka.git
    GIT_TAG cmocka-1.1.5
)
FetchContent_MakeAvailable(cmocka)

# 启用测试
enable_testing()
include(CTest)

# 添加各测试模块
add_subdirectory(unit/safe_string)
add_subdirectory(unit/string_ext)
add_subdirectory(unit/path_manager)
