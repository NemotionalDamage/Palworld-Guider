cmake_minimum_required(VERSION 3.18)

project(PalworldGuider LANGUAGES CXX)

if(NOT WIN32)
    message(FATAL_ERROR "PalworldGuider requires Windows")
endif()

if(NOT CMAKE_SIZEOF_VOID_P EQUAL 8)
    message(FATAL_ERROR "PalworldGuider requires x64")
endif()

set(UE4SS_DLL "" CACHE FILEPATH "Path to the target UE4SS.dll")
if(NOT EXISTS "${UE4SS_DLL}")
    message(FATAL_ERROR "UE4SS_DLL must point to the target UE4SS.dll")
endif()

set(UE4SS_DLL_SHA256 "8AC18FBFFC1EF96B0662D4A2D537B3F224C26D65CAABA7989A9404C566102B26")
file(SHA256 "${UE4SS_DLL}" UE4SS_DLL_ACTUAL_SHA256)
string(TOUPPER "${UE4SS_DLL_ACTUAL_SHA256}" UE4SS_DLL_ACTUAL_SHA256_UPPERCASE)
if(NOT UE4SS_DLL_ACTUAL_SHA256_UPPERCASE STREQUAL "${UE4SS_DLL_SHA256}")
    message(FATAL_ERROR "UE4SS.dll SHA256 mismatch: expected '${UE4SS_DLL_SHA256}', got '${UE4SS_DLL_ACTUAL_SHA256_UPPERCASE}'")
endif()

# The UE4SS import library is derived from the verified UE4SS.dll exports
# so the checked-in UE4SS.def can remain the mod's own export definition
# (exactly start_mod and uninstall_mod).
find_program(UE4SS_DUMPBIN_TOOL NAMES dumpbin)
if(NOT UE4SS_DUMPBIN_TOOL)
    message(FATAL_ERROR "dumpbin.exe is required to derive the UE4SS import library")
endif()

execute_process(COMMAND "${UE4SS_DUMPBIN_TOOL}" /exports "${UE4SS_DLL}"
                OUTPUT_VARIABLE UE4SS_DUMPBIN_OUTPUT
                RESULT_VARIABLE ue4ss_dumpbin_result)
if(NOT ue4ss_dumpbin_result EQUAL 0)
    message(FATAL_ERROR "dumpbin /exports failed for UE4SS.dll")
endif()

string(REPLACE "\r\n" "\n" UE4SS_DUMPBIN_OUTPUT "${UE4SS_DUMPBIN_OUTPUT}")
string(REPLACE "\n" ";" UE4SS_DUMPBIN_LINES "${UE4SS_DUMPBIN_OUTPUT}")
set(UE4SS_IMPORT_NAMES "")
foreach(UE4SS_DUMPBIN_LINE IN LISTS UE4SS_DUMPBIN_LINES)
    if(UE4SS_DUMPBIN_LINE MATCHES "^[ ]*[0-9]+[ ]+[0-9A-Fa-f]+[ ]+[0-9A-Fa-f]+[ ]+(.+)$")
        string(APPEND UE4SS_IMPORT_NAMES "${CMAKE_MATCH_1}\n")
    endif()
endforeach()

set(UE4SS_IMPORTS_DEF "${CMAKE_BINARY_DIR}/UE4SS_imports.def")
file(WRITE "${UE4SS_IMPORTS_DEF}" "LIBRARY UE4SS\nEXPORTS\n${UE4SS_IMPORT_NAMES}")

find_program(UE4SS_LIB_TOOL NAMES lib)
if(NOT UE4SS_LIB_TOOL)
    message(FATAL_ERROR "lib.exe is required to generate the UE4SS import library")
endif()

set(UE4SS_IMPORT_LIB "${CMAKE_BINARY_DIR}/UE4SS.lib")
execute_process(COMMAND "${UE4SS_LIB_TOOL}"
                        "/def:${UE4SS_IMPORTS_DEF}"
                        /machine:X64
                        "/out:${UE4SS_IMPORT_LIB}"
                RESULT_VARIABLE ue4ss_lib_result)
if(NOT ue4ss_lib_result EQUAL 0)
    message(FATAL_ERROR "Failed to generate the UE4SS import library")
endif()

add_library(UE4SS STATIC IMPORTED)
set_target_properties(UE4SS PROPERTIES IMPORTED_LOCATION "${UE4SS_IMPORT_LIB}")

add_library(PalworldGuider SHARED
            src/palworld_guider.cpp
            src/pal_transport.cpp
            UE4SS.def)
target_include_directories(PalworldGuider PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_compile_features(PalworldGuider PRIVATE cxx_std_20)
set_target_properties(PalworldGuider PROPERTIES CXX_EXTENSIONS OFF)
set_property(TARGET PalworldGuider PROPERTY MSVC_RUNTIME_LIBRARY "MultiThreadedDLL")
target_link_libraries(PalworldGuider PRIVATE UE4SS winhttp)
set_target_properties(PalworldGuider PROPERTIES OUTPUT_NAME main)

get_filename_component(GUIDER_SCRIPTS_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../Scripts" ABSOLUTE)

add_custom_command(TARGET PalworldGuider POST_BUILD
                   COMMAND "${CMAKE_COMMAND}" -E make_directory
                           "${CMAKE_BINARY_DIR}/PalworldGuider/dlls"
                   COMMAND "${CMAKE_COMMAND}" -E make_directory
                           "${CMAKE_BINARY_DIR}/PalworldGuider/Scripts"
                   COMMAND "${CMAKE_COMMAND}" -E copy_if_different
                           "$<TARGET_FILE:PalworldGuider>"
                           "${CMAKE_BINARY_DIR}/PalworldGuider/dlls/main.dll"
                   COMMAND "${CMAKE_COMMAND}" -E copy_if_different
                           "${GUIDER_SCRIPTS_DIR}/main.lua"
                           "${CMAKE_BINARY_DIR}/PalworldGuider/Scripts/main.lua"
                   COMMAND "${CMAKE_COMMAND}" -E copy_if_different
                           "${GUIDER_SCRIPTS_DIR}/pal_transport.lua"
                           "${CMAKE_BINARY_DIR}/PalworldGuider/Scripts/pal_transport.lua"
                   COMMAND "${CMAKE_COMMAND}" -E copy_if_different
                           "${GUIDER_SCRIPTS_DIR}/pal_json.lua"
                           "${CMAKE_BINARY_DIR}/PalworldGuider/Scripts/pal_json.lua")
