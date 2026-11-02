cmake_minimum_required(VERSION 3.18)
project(libjxl_rust_oracle LANGUAGES C CXX)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# The Rust palette sort follows the recorded libc++ specialization. Select
# that same standard library explicitly on Linux instead of silently using
# libstdc++, whose equivalent-element order differs. This is oracle-only.
if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
  if(NOT CMAKE_CXX_COMPILER_ID MATCHES "Clang")
    message(FATAL_ERROR "The Linux differential oracle requires Clang with libc++")
  endif()
  string(APPEND CMAKE_CXX_FLAGS " -stdlib=libc++")
endif()

get_filename_component(PORT_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.." ABSOLUTE)
set(UPSTREAM "${PORT_ROOT}/upstream/libjxl")
if(NOT EXISTS "${UPSTREAM}/third_party/highway/hwy/base.h")
  message(FATAL_ERROR "Initialize upstream/libjxl and its third_party/highway submodule first")
endif()
if(NOT EXISTS "${UPSTREAM}/third_party/brotli/c/include/brotli/encode.h")
  message(FATAL_ERROR "Initialize upstream/libjxl/third_party/brotli for frame reference headers")
endif()
if(NOT EXISTS "${UPSTREAM}/third_party/skcms/skcms.h")
  message(FATAL_ERROR "Initialize upstream/libjxl/third_party/skcms for the full image reference")
endif()

option(ORACLE_SANITIZERS "Instrument the complete C++ reference with ASan and UBSan" OFF)
add_compile_definitions(HWY_COMPILE_ONLY_SCALAR=1)
if(ORACLE_SANITIZERS)
  add_compile_options(-fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer)
  add_link_options(-fsanitize=address,undefined)
endif()

set(HWY_ENABLE_TESTS OFF CACHE BOOL "" FORCE)
set(HWY_ENABLE_EXAMPLES OFF CACHE BOOL "" FORCE)
set(HWY_ENABLE_CONTRIB OFF CACHE BOOL "" FORCE)
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
set(BUILD_TESTING OFF CACHE BOOL "" FORCE)
foreach(feature TOOLS DEVTOOLS DOXYGEN MANPAGES BENCHMARK EXAMPLES JNI SJPEG
                OPENEXR VIEWERS TCMALLOC PLUGINS COVERAGE FUZZERS LTO)
  set(JPEGXL_ENABLE_${feature} OFF CACHE BOOL "" FORCE)
endforeach()
set(JPEGXL_BUNDLE_LIBPNG OFF CACHE BOOL "" FORCE)
set(JPEGXL_ENABLE_SKCMS ON CACHE BOOL "" FORCE)
foreach(dependency BROTLI HWY LCMS2 GTEST)
  set(JPEGXL_FORCE_SYSTEM_${dependency} OFF CACHE BOOL "" FORCE)
endforeach()

# Build the complete original static library through its own CMake project.
# Adapter TUs include selected pristine .cc files to reach private functions;
# the archive supplies the remaining original objects, without replacement code.
add_subdirectory("${UPSTREAM}" upstream EXCLUDE_FROM_ALL)
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
  # Keep Quantizer's original constructor/recomputation calls across plain and
  # sanitized builds. Inlining folds *1 into a raw copy only in the plain build,
  # changing signaling-NaN observations. The source routines stay unmodified.
  set_property(SOURCE "${UPSTREAM}/lib/jxl/quantizer.cc"
    DIRECTORY "${UPSTREAM}/lib" APPEND PROPERTY COMPILE_OPTIONS -fno-inline-functions)
endif()
find_package(Python3 REQUIRED COMPONENTS Interpreter)
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS
  "${CMAKE_CURRENT_SOURCE_DIR}/extract_source.py"
  "${CMAKE_CURRENT_SOURCE_DIR}/source_fragments.json"
  "${UPSTREAM}/lib/jxl/render_pipeline/render_pipeline_test.cc"
  "${UPSTREAM}/lib/jxl/test_utils.cc"
  "${UPSTREAM}/lib/jxl/dec_group.cc")
execute_process(COMMAND "${Python3_EXECUTABLE}"
  "${CMAKE_CURRENT_SOURCE_DIR}/extract_source.py" "${CMAKE_CURRENT_BINARY_DIR}/generated"
  RESULT_VARIABLE extraction_status)
if(NOT extraction_status STREQUAL "0")
  message(FATAL_ERROR "Pinned source helper extraction failed")
endif()
add_executable(jxl_oracle main.cc spot_color.cc epf.cc vardct_dc.cc group_schedule.cc dequant.cc group_render.cc block_context.cc dct.cc sample_conversion.cc noise.cc gaborish.cc xyb_stage.cc quant_ac.cc quantizer.cc ac_group_symbols.cc
  "${UPSTREAM}/lib/jxl/test_memory_manager.cc")
# Expose original internal state to the frame fixture adapter without changing
# source files, class layouts, or runtime algorithms. Limit this to its TU.
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
  # Match the original GetBlock class hierarchy in this traversal observer.
  # Ordinary and instrumented O3 EPF stages select different NaN payloads.
  # Retain pristine Highway scalar operator boundaries in this new TU only.
  set_source_files_properties(epf.cc PROPERTIES COMPILE_OPTIONS "-O0;-fno-rtti;-fno-access-control")
  set_source_files_properties(spot_color.cc PROPERTIES COMPILE_OPTIONS "-fno-rtti;-fno-access-control")
  if(CMAKE_SYSTEM_NAME STREQUAL "Linux" AND CMAKE_SYSTEM_PROCESSOR MATCHES "^(x86_64|amd64|AMD64)$")
    # Native O3 ordinary/ASan original Spot objects differ on 3,074 existing
    # exceptional requests. Retain pristine scalar expression boundaries only
    # in this observer. ARM contraction and original archive options stay intact.
    set_property(SOURCE spot_color.cc APPEND PROPERTY COMPILE_OPTIONS -O0)
  endif()
  set_source_files_properties(group_render.cc PROPERTIES COMPILE_OPTIONS -fno-rtti)
  set_source_files_properties(group_schedule.cc PROPERTIES COMPILE_OPTIONS -fno-access-control)
  if(CMAKE_SYSTEM_NAME STREQUAL "Linux" AND CMAKE_SYSTEM_PROCESSOR MATCHES "^(x86_64|amd64|AMD64)$")
    # Native x86 ordinary/ASan objects choose different competing NaN payloads
    # in the B correlation sum. Retain pristine scalar operator boundaries in
    # this isolated observer; original decoder objects and sanitizers stay intact.
    set_source_files_properties(dequant.cc PROPERTIES COMPILE_OPTIONS -O0)
  endif()
  # Pristine scalar compressed_dc.cc supplies the actual public DC symbols.
  # Keep its operand order reproducible with every sanitizer still active;
  # neither the original archive nor any earlier adapter changes flags.
  set_source_files_properties(vardct_dc.cc PROPERTIES COMPILE_OPTIONS "-O0;-fno-access-control;-fno-rtti")
  # Unrolling makes the ordinary AArch64 identity object use FMSUB while
  # ASan keeps FMADD with a negative scale, changing a NaN sign. Retain its
  # original loop shape in this new TU; contraction, O3 and sanitizers stay on.
  set_source_files_properties(dct.cc PROPERTIES COMPILE_OPTIONS -fno-unroll-loops)
  # Observe quantization cache masks and offsets without changing declarations.
  set_source_files_properties(quant_ac.cc PROPERTIES COMPILE_OPTIONS "-fno-access-control")
  # Match upstream's no-RTTI build when deriving fixture pipeline stages.
  set_source_files_properties(sample_conversion.cc PROPERTIES COMPILE_OPTIONS "-fno-access-control;-fno-rtti")
  # Clang's unrolled max-number reduction changes the early-return result when
  # a signaling NaN follows a value above the threshold. Retain the source loop.
  set_source_files_properties(noise.cc PROPERTIES COMPILE_OPTIONS -fno-unroll-loops)
  # O3 commutes the original scalar Gaborish operands differently with ASan.
  # Retain the pristine operator bodies in their own TU, with all sanitizer
  # flags active. The old adapter and original library flags stay unchanged.
  set_source_files_properties(gaborish.cc PROPERTIES COMPILE_OPTIONS "-O0;-fno-rtti")
  # Keep original scalar XYB operand order reproducible with active sanitizers.
  # Only this new adapter and its included pristine stage/helper use -O0;
  # neither the original codec library nor another fixture changes flags.
  set_source_files_properties(xyb_stage.cc PROPERTIES COMPILE_OPTIONS "-O0;-fno-access-control;-fno-rtti")
else()
  message(FATAL_ERROR "Frame state fixtures require Clang or GCC access-control support")
endif()
target_compile_features(jxl_oracle PRIVATE cxx_std_17)
target_include_directories(jxl_oracle PRIVATE "${UPSTREAM}" "${UPSTREAM}/lib/include"
  "${UPSTREAM}/third_party/brotli/c/include" "${CMAKE_CURRENT_BINARY_DIR}/generated")
target_compile_definitions(jxl_oracle PRIVATE HWY_COMPILE_ONLY_SCALAR=1 JXL_STATIC_DEFINE JXL_CMS_STATIC_DEFINE)
target_link_libraries(jxl_oracle PRIVATE jxl-internal)
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang" AND NOT WIN32)
  # Preserve upstream lib/CMakeLists.txt's numeric code-generation flags.
  # Auto-vectorizing scalar cost reductions changes their FMA rounding, and
  # sanitizer instrumentation can otherwise select a different reduction loop.
  target_compile_options(jxl_oracle PRIVATE -fno-slp-vectorize -fno-vectorize)
endif()
if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU")
  target_compile_options(jxl_oracle PRIVATE -ffunction-sections -fdata-sections)
  if(APPLE)
    target_link_options(jxl_oracle PRIVATE -Wl,-dead_strip)
  else()
    target_link_options(jxl_oracle PRIVATE -Wl,--gc-sections)
  endif()
endif()
