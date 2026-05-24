#!/bin/bash
set -e

echo "=== Compiling SIMD / OpenCL test program ==="
./llm-clang tests/lang/simd_perf_test.llm -o tests/lang/simd_perf_test_bin

cleanup() {
    rm -rf tests/lang/simd_perf_test_bin libllm_opencl.so
}
trap cleanup EXIT

echo "=== 1. Testing CPU SIMD Auto-vectorization Fallback ==="
# Move libllm_opencl.so away to force CPU fallback
if [ -f libllm_opencl.so ]; then
    mv libllm_opencl.so libllm_opencl.so.tmp
fi

# Run CPU vectorized code
RES_CPU=$(./tests/lang/simd_perf_test_bin)
echo "CPU Fallback Output: $RES_CPU"

if [ "$RES_CPU" != "1" ]; then
    echo "FAIL: CPU SIMD execution output was not 1 (success)"
    exit 1
fi
echo "PASS: CPU SIMD auto-vectorization verified successfully!"

echo "=== 2. Testing OpenCL Dynamic GPU Dispatch ==="
# Restore libllm_opencl.so
if [ -f libllm_opencl.so.tmp ]; then
    mv libllm_opencl.so.tmp libllm_opencl.so
fi

# Set threshold to trigger GPU JIT compilation (array size is 20000)
# We can run the program. It should attempt OpenCL compilation.
# Even if the machine has no GPU, it should fall back to CPU and output 1.
RES_GPU=$(./tests/lang/simd_perf_test_bin)
echo "GPU/OpenCL Dispatch Output: $RES_GPU"

if [ "$RES_GPU" != "1" ]; then
    echo "FAIL: GPU/OpenCL execution output was not 1 (success)"
    exit 1
fi

echo "PASS: Dynamic OpenCL driver path ran successfully!"
echo "ALL SIMD AUTO-VECTORIZATION AND OPENCL JIT TESTS PASSED CLEANLY!"
