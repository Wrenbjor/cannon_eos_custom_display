cmake_minimum_required(VERSION 3.24)
project(OpenEosVirtualCamera LANGUAGES CXX)
if(NOT WIN32 OR NOT MSVC)
  message(FATAL_ERROR "This prototype requires Windows 11 and Microsoft C++ Build Tools")
endif()
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
include(FetchContent)
FetchContent_Declare(wil
  URL https://api.nuget.org/v3-flatcontainer/microsoft.windows.implementationlibrary/1.0.260126.7/microsoft.windows.implementationlibrary.1.0.260126.7.nupkg
  URL_HASH SHA256=7d67bc71dd0edc342db44ea0334ac9b6a1776514f81694b0b5b3c7ac9d608e42
  DOWNLOAD_EXTRACT_TIMESTAMP TRUE)
FetchContent_MakeAvailable(wil)
add_library(OpenEosCameraSource SHARED
  source/dllmain.cpp source/winrtCommon.cpp source/EventHandler.cpp
  source/SimpleMediaSource.cpp source/SimpleMediaStream.cpp source/SimpleFrameGenerator.cpp
  source/VirtualCameraMediaSourceActivate.cpp source/VirtualCameraMediaSource.def)
target_include_directories(OpenEosCameraSource PRIVATE "${wil_SOURCE_DIR}/include" source)
target_precompile_headers(OpenEosCameraSource PRIVATE source/pch.h)
target_compile_definitions(OpenEosCameraSource PRIVATE UNICODE _UNICODE NOMINMAX)
target_compile_options(OpenEosCameraSource PRIVATE /EHsc /W3)
target_link_libraries(OpenEosCameraSource PRIVATE mfplat mf mfuuid mfreadwrite mfsensorgroup windowsapp cfgmgr32 shlwapi)
add_executable(eos-vcam controller.cpp)
target_compile_definitions(eos-vcam PRIVATE UNICODE _UNICODE NOMINMAX)
target_link_libraries(eos-vcam PRIVATE mfplat mf mfuuid mfreadwrite mfsensorgroup ole32 advapi32)
enable_testing()
add_test(NAME virtual_camera_stream COMMAND eos-vcam self-test "$<TARGET_FILE:OpenEosCameraSource>")
set_tests_properties(virtual_camera_stream PROPERTIES TIMEOUT 30)
