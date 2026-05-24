#include "common.h"
#include <dlfcn.h>
#include <stddef.h>
#include <stdio.h>

typedef long (*OpenClMapFn)(long, long, long, const char*);
static OpenClMapFn g_opencl_map_fn = NULL;
static int g_opencl_checked = 0;

long llm_opencl_map(long input_col_ptr, long output_col_ptr, long count, const char* kernel_src) {
    if (!g_opencl_checked) {
        g_opencl_checked = 1;
        
        // Check SERVICE_BINDING_ROOT or standard paths first
        void* handle = dlopen("libllm_opencl.so", RTLD_LAZY);
        if (!handle) {
            handle = dlopen("./libllm_opencl.so", RTLD_LAZY);
        }
        if (handle) {
            g_opencl_map_fn = (OpenClMapFn)dlsym(handle, "llm_opencl_map_impl");
        }
    }
    
    if (g_opencl_map_fn) {
        return g_opencl_map_fn(input_col_ptr, output_col_ptr, count, kernel_src);
    }
    
    return 0; // Failure, fall back to CPU vectorization
}
