#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <dlfcn.h>

// OpenCL Type Definitions
typedef void* cl_platform_id;
typedef void* cl_device_id;
typedef void* cl_context;
typedef void* cl_command_queue;
typedef void* cl_program;
typedef void* cl_kernel;
typedef void* cl_mem;
typedef int cl_int;
typedef unsigned int cl_uint;
typedef unsigned long cl_ulong;
typedef unsigned long cl_properties;

#define CL_SUCCESS                                  0
#define CL_DEVICE_TYPE_GPU                          (1ULL << 2)
#define CL_MEM_READ_ONLY                            (1ULL << 2)
#define CL_MEM_WRITE_ONLY                           (1ULL << 3)
#define CL_MEM_COPY_HOST_PTR                        (1ULL << 5)

// Function pointer signatures for OpenCL APIs
typedef cl_int (*fn_clGetPlatformIDs)(cl_uint, cl_platform_id*, cl_uint*);
typedef cl_int (*fn_clGetDeviceIDs)(cl_platform_id, cl_ulong, cl_uint, cl_device_id*, cl_uint*);
typedef cl_context (*fn_clCreateContext)(const cl_properties*, cl_uint, const cl_device_id*, void ( * )(const char*, const void*, size_t, void*), void*, cl_int*);
typedef cl_command_queue (*fn_clCreateCommandQueue)(cl_context, cl_device_id, cl_ulong, cl_int*);
typedef cl_program (*fn_clCreateProgramWithSource)(cl_context, cl_uint, const char**, const size_t*, cl_int*);
typedef cl_int (*fn_clBuildProgram)(cl_program, cl_uint, const cl_device_id*, const char*, void ( * )(cl_program, void*), void*);
typedef cl_kernel (*fn_clCreateKernel)(cl_program, const char*, cl_int*);
typedef cl_mem (*fn_clCreateBuffer)(cl_context, cl_ulong, size_t, void*, cl_int*);
typedef cl_int (*fn_clEnqueueWriteBuffer)(cl_command_queue, cl_mem, cl_uint, size_t, size_t, const void*, cl_uint, const void*, void*);
typedef cl_int (*fn_clSetKernelArg)(cl_kernel, cl_uint, size_t, const void*);
typedef cl_int (*fn_clEnqueueNDRangeKernel)(cl_command_queue, cl_kernel, cl_uint, const size_t*, const size_t*, const size_t*, cl_uint, const void*, void*);
typedef cl_int (*fn_clEnqueueReadBuffer)(cl_command_queue, cl_mem, cl_uint, size_t, size_t, void*, cl_uint, const void*, void*);
typedef cl_int (*fn_clFinish)(cl_command_queue);
typedef cl_int (*fn_clReleaseMemObject)(cl_mem);
typedef cl_int (*fn_clReleaseKernel)(cl_kernel);
typedef cl_int (*fn_clReleaseProgram)(cl_program);
typedef cl_int (*fn_clReleaseCommandQueue)(cl_command_queue);
typedef cl_int (*fn_clReleaseContext)(cl_context);

static void* g_ocl_handle = NULL;
static fn_clGetPlatformIDs p_clGetPlatformIDs = NULL;
static fn_clGetDeviceIDs p_clGetDeviceIDs = NULL;
static fn_clCreateContext p_clCreateContext = NULL;
static fn_clCreateCommandQueue p_clCreateCommandQueue = NULL;
static fn_clCreateProgramWithSource p_clCreateProgramWithSource = NULL;
static fn_clBuildProgram p_clBuildProgram = NULL;
static fn_clCreateKernel p_clCreateKernel = NULL;
static fn_clCreateBuffer p_clCreateBuffer = NULL;
static fn_clEnqueueWriteBuffer p_clEnqueueWriteBuffer = NULL;
static fn_clSetKernelArg p_clSetKernelArg = NULL;
static fn_clEnqueueNDRangeKernel p_clEnqueueNDRangeKernel = NULL;
static fn_clEnqueueReadBuffer p_clEnqueueReadBuffer = NULL;
static fn_clFinish p_clFinish = NULL;
static fn_clReleaseMemObject p_clReleaseMemObject = NULL;
static fn_clReleaseKernel p_clReleaseKernel = NULL;
static fn_clReleaseProgram p_clReleaseProgram = NULL;
static fn_clReleaseCommandQueue p_clReleaseCommandQueue = NULL;
static fn_clReleaseContext p_clReleaseContext = NULL;

static int load_opencl() {
    if (g_ocl_handle) return 1;
    
    // Attempt standard Linux, macOS, and Windows library names
    g_ocl_handle = dlopen("libOpenCL.so.1", RTLD_LAZY);
    if (!g_ocl_handle) g_ocl_handle = dlopen("libOpenCL.so", RTLD_LAZY);
    if (!g_ocl_handle) g_ocl_handle = dlopen("/System/Library/Frameworks/OpenCL.framework/OpenCL", RTLD_LAZY);
    if (!g_ocl_handle) g_ocl_handle = dlopen("OpenCL.dll", RTLD_LAZY);
    
    if (!g_ocl_handle) return 0;
    
    p_clGetPlatformIDs = (fn_clGetPlatformIDs)dlsym(g_ocl_handle, "clGetPlatformIDs");
    p_clGetDeviceIDs = (fn_clGetDeviceIDs)dlsym(g_ocl_handle, "clGetDeviceIDs");
    p_clCreateContext = (fn_clCreateContext)dlsym(g_ocl_handle, "clCreateContext");
    p_clCreateCommandQueue = (fn_clCreateCommandQueue)dlsym(g_ocl_handle, "clCreateCommandQueue");
    p_clCreateProgramWithSource = (fn_clCreateProgramWithSource)dlsym(g_ocl_handle, "clCreateProgramWithSource");
    p_clBuildProgram = (fn_clBuildProgram)dlsym(g_ocl_handle, "clBuildProgram");
    p_clCreateKernel = (fn_clCreateKernel)dlsym(g_ocl_handle, "clCreateKernel");
    p_clCreateBuffer = (fn_clCreateBuffer)dlsym(g_ocl_handle, "clCreateBuffer");
    p_clEnqueueWriteBuffer = (fn_clEnqueueWriteBuffer)dlsym(g_ocl_handle, "clEnqueueWriteBuffer");
    p_clSetKernelArg = (fn_clSetKernelArg)dlsym(g_ocl_handle, "clSetKernelArg");
    p_clEnqueueNDRangeKernel = (fn_clEnqueueNDRangeKernel)dlsym(g_ocl_handle, "clEnqueueNDRangeKernel");
    p_clEnqueueReadBuffer = (fn_clEnqueueReadBuffer)dlsym(g_ocl_handle, "clEnqueueReadBuffer");
    p_clFinish = (fn_clFinish)dlsym(g_ocl_handle, "clFinish");
    p_clReleaseMemObject = (fn_clReleaseMemObject)dlsym(g_ocl_handle, "clReleaseMemObject");
    p_clReleaseKernel = (fn_clReleaseKernel)dlsym(g_ocl_handle, "clReleaseKernel");
    p_clReleaseProgram = (fn_clReleaseProgram)dlsym(g_ocl_handle, "clReleaseProgram");
    p_clReleaseCommandQueue = (fn_clReleaseCommandQueue)dlsym(g_ocl_handle, "clReleaseCommandQueue");
    p_clReleaseContext = (fn_clReleaseContext)dlsym(g_ocl_handle, "clReleaseContext");
    
    return (p_clGetPlatformIDs && p_clGetDeviceIDs && p_clCreateContext && 
            p_clCreateCommandQueue && p_clCreateProgramWithSource && p_clBuildProgram && 
            p_clCreateKernel && p_clCreateBuffer && p_clEnqueueWriteBuffer && 
            p_clSetKernelArg && p_clEnqueueNDRangeKernel && p_clEnqueueReadBuffer && 
            p_clFinish && p_clReleaseMemObject && p_clReleaseKernel && 
            p_clReleaseProgram && p_clReleaseCommandQueue && p_clReleaseContext);
}

// Pluggable driver entry point
long llm_opencl_map_impl(long input_col_ptr, long output_col_ptr, long count, const char* kernel_src) {
    if (!load_opencl()) return 0;
    if (count <= 0) return 1;
    
    cl_platform_id platform = NULL;
    cl_uint num_platforms = 0;
    cl_int err = p_clGetPlatformIDs(1, &platform, &num_platforms);
    if (err != CL_SUCCESS || num_platforms == 0) return 0;
    
    cl_device_id device = NULL;
    cl_uint num_devices = 0;
    err = p_clGetDeviceIDs(platform, CL_DEVICE_TYPE_GPU, 1, &device, &num_devices);
    if (err != CL_SUCCESS || num_devices == 0) {
        // Fall back to CPU device type if no GPU is found
        err = p_clGetDeviceIDs(platform, 1ULL << 1, 1, &device, &num_devices); // CL_DEVICE_TYPE_CPU
        if (err != CL_SUCCESS || num_devices == 0) return 0;
    }
    
    cl_context context = p_clCreateContext(NULL, 1, &device, NULL, NULL, &err);
    if (err != CL_SUCCESS || !context) return 0;
    
    cl_command_queue queue = p_clCreateCommandQueue(context, device, 0, &err);
    if (err != CL_SUCCESS || !queue) {
        p_clReleaseContext(context);
        return 0;
    }
    
    cl_program program = p_clCreateProgramWithSource(context, 1, &kernel_src, NULL, &err);
    if (err != CL_SUCCESS || !program) {
        p_clReleaseCommandQueue(queue);
        p_clReleaseContext(context);
        return 0;
    }
    
    err = p_clBuildProgram(program, 1, &device, NULL, NULL, NULL);
    if (err != CL_SUCCESS) {
        p_clReleaseProgram(program);
        p_clReleaseCommandQueue(queue);
        p_clReleaseContext(context);
        return 0;
    }
    
    cl_kernel kernel = p_clCreateKernel(program, "map_kernel", &err);
    if (err != CL_SUCCESS || !kernel) {
        p_clReleaseProgram(program);
        p_clReleaseCommandQueue(queue);
        p_clReleaseContext(context);
        return 0;
    }
    
    size_t size_bytes = count * sizeof(long);
    cl_mem d_input = p_clCreateBuffer(context, CL_MEM_READ_ONLY | CL_MEM_COPY_HOST_PTR, size_bytes, (void*)input_col_ptr, &err);
    if (err != CL_SUCCESS || !d_input) {
        p_clReleaseKernel(kernel);
        p_clReleaseProgram(program);
        p_clReleaseCommandQueue(queue);
        p_clReleaseContext(context);
        return 0;
    }
    
    cl_mem d_output = p_clCreateBuffer(context, CL_MEM_WRITE_ONLY, size_bytes, NULL, &err);
    if (err != CL_SUCCESS || !d_output) {
        p_clReleaseMemObject(d_input);
        p_clReleaseKernel(kernel);
        p_clReleaseProgram(program);
        p_clReleaseCommandQueue(queue);
        p_clReleaseContext(context);
        return 0;
    }
    
    p_clSetKernelArg(kernel, 0, sizeof(cl_mem), &d_input);
    p_clSetKernelArg(kernel, 1, sizeof(cl_mem), &d_output);
    p_clSetKernelArg(kernel, 2, sizeof(long), &count);
    
    size_t global_work_size = (size_t)count;
    err = p_clEnqueueNDRangeKernel(queue, kernel, 1, NULL, &global_work_size, NULL, 0, NULL, NULL);
    if (err != CL_SUCCESS) {
        p_clReleaseMemObject(d_output);
        p_clReleaseMemObject(d_input);
        p_clReleaseKernel(kernel);
        p_clReleaseProgram(program);
        p_clReleaseCommandQueue(queue);
        p_clReleaseContext(context);
        return 0;
    }
    
    err = p_clEnqueueReadBuffer(queue, d_output, 1, 0, size_bytes, (void*)output_col_ptr, 0, NULL, NULL);
    p_clFinish(queue);
    
    p_clReleaseMemObject(d_output);
    p_clReleaseMemObject(d_input);
    p_clReleaseKernel(kernel);
    p_clReleaseProgram(program);
    p_clReleaseCommandQueue(queue);
    p_clReleaseContext(context);
    
    return (err == CL_SUCCESS) ? 1 : 0;
}
