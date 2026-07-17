#include "common.h"
#include <dlfcn.h>
#include <stddef.h>
#include <stdio.h>

typedef long (*OpenClMapFn)(long, long, long, const char*);
static OpenClMapFn g_opencl_map_fn = NULL;
static pthread_once_t g_opencl_init_once = PTHREAD_ONCE_INIT;

static void llm_opencl_init(void) {
    // Check SERVICE_BINDING_ROOT or standard paths first
    void* handle = dlopen("libllm_opencl.so", RTLD_LAZY);
    if (!handle) {
        handle = dlopen("./libllm_opencl.so", RTLD_LAZY);
    }
    if (handle) {
        g_opencl_map_fn = (OpenClMapFn)dlsym(handle, "llm_opencl_map_impl");
    }
}

long llm_opencl_map(long input_col_ptr, long output_col_ptr, long count, const char* kernel_src) {
    // Was a plain `if (!g_opencl_checked)` check: two threads racing here
    // could both see it unset and call dlopen/dlsym concurrently, with no
    // guarantee the g_opencl_map_fn write from one is visible to the other.
    pthread_once(&g_opencl_init_once, llm_opencl_init);

    if (g_opencl_map_fn) {
        return g_opencl_map_fn(input_col_ptr, output_col_ptr, count, kernel_src);
    }

    return 0; // Failure, fall back to CPU vectorization
}
