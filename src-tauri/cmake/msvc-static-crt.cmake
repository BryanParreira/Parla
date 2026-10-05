# Windows only: makes llama.cpp use the static C runtime, matching the prebuilt speech
# library it is linked with. llama.cpp's CMake setup picks the runtime from this
# variable and ignores the /MT flag the Rust build passes, so the CI build points
# CMAKE_TOOLCHAIN_FILE here.
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded")
