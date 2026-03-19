cmake_minimum_required(VERSION 3.10)
project(PathEditor C)

# Compile flags and definitions
add_compile_options(-Wall -O2 -fexec-charset=UTF-8)
add_compile_definitions(_WIN32 UNICODE _UNICODE)

# Include directories
include_directories(
    ${CMAKE_CURRENT_SOURCE_DIR}/include
    ${CMAKE_CURRENT_SOURCE_DIR}/libs/iup-3.31_Win64_dllw6_lib/include
)

# Source files
set(SOURCES
    src/main.c
    src/utils.c
    src/registry.c
    src/callbacks.c
    src/ui.c
    src/globals.c
)

# Resource file
set(RESOURCES
    ico/resources.rc
)

# Library directories
link_directories(
    ${CMAKE_CURRENT_SOURCE_DIR}/libs/iup-3.31_Win64_dllw6_lib
)

# Create WIN32 executable (automatically adds -mwindows and handles .rc files)
add_executable(${PROJECT_NAME} WIN32 ${SOURCES} ${RESOURCES})

# Link libraries
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

# Copy DLLs to the build directory after building
add_custom_command(TARGET ${PROJECT_NAME} POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_directory
    "${CMAKE_CURRENT_SOURCE_DIR}/libs/iup-3.31_Win64_dllw6_lib/"
    "$<TARGET_FILE_DIR:${PROJECT_NAME}>"
    COMMENT "Copying DLLs to build directory..."
)