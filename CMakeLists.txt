cmake_minimum_required(VERSION 3.15)
project(PathEditor LANGUAGES C)

set(IUP_DIR "${CMAKE_SOURCE_DIR}/libs/iup-3.31_Win64_dllw6_lib")

add_executable(PathEditor
    src/main.c
    src/utils.c
    src/registry.c
    src/callbacks.c
    ico/resources.rc
)

target_include_directories(PathEditor PRIVATE
    ${IUP_DIR}/include
    include
)

target_link_directories(PathEditor PRIVATE ${IUP_DIR})

target_link_libraries(PathEditor
    iup iupcd gdi32 comdlg32 comctl32 uuid ole32 advapi32
)

target_compile_definitions(PathEditor PRIVATE _WIN32 UNICODE _UNICODE)

target_compile_options(PathEditor PRIVATE -Wall -O2 -fexec-charset=UTF-8)

target_link_options(PathEditor PRIVATE -mwindows)

set_target_properties(PathEditor PROPERTIES
    RUNTIME_OUTPUT_DIRECTORY "${CMAKE_SOURCE_DIR}/bin"
)
