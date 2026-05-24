#!/bin/bash
# Comprehensive Comparative Benchmark Suite
# llmlang vs C (-O3)

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$DIR"

# Increase stack size to 256MB to allow deep recursion for high-iteration benchmarks
ulimit -s 262144

echo "================================================="
echo "    llmlang vs C (-O3) Comparative Benchmarks    "
echo "================================================="
echo ""

# Build C baselines
echo "Building C Baselines (-O3)..."
gcc -O3 01_memory_layout/aos_sum.c -o 01_memory_layout/aos_sum
gcc -O3 01_memory_layout/soa_sum.c -o 01_memory_layout/soa_sum
gcc -O3 02_vector_math/vector_add.c -o 02_vector_math/vector_add
gcc -O3 03_recursion/fib.c -o 03_recursion/fib
gcc -O3 04_string_split/split.c -o 04_string_split/split

# Build llmlang implementations
echo "Building llmlang implementations..."
../llm-clang 01_memory_layout/soa_sum.llm -o 01_memory_layout/soa_sum_llm
../llm-clang 02_vector_math/vector_add.llm -o 02_vector_math/vector_add_llm
../llm-clang 03_recursion/fib.llm -o 03_recursion/fib_llm
../llm-clang 04_string_split/split.llm -o 04_string_split/split_llm

echo "Running benchmarks..."

AOS_C=$(./01_memory_layout/aos_sum | grep "Sum Time" | awk '{print $3}')
SOA_C=$(./01_memory_layout/soa_sum | grep "Sum Time" | awk '{print $3}')
SOA_L=$(./01_memory_layout/soa_sum_llm | grep "Sum Time" | awk '{print $3}')

VEC_C=$(./02_vector_math/vector_add | grep "Time" | awk '{print $4}')
VEC_L=$(./02_vector_math/vector_add_llm | grep "Time" | awk '{print $4}')

FIB_C=$(./03_recursion/fib | grep "Time" | awk '{print $3}')
FIB_L=$(./03_recursion/fib_llm | grep "Time" | awk '{print $3}')

SPL_C=$(./04_string_split/split | grep "Time" | awk '{print $3}')
SPL_L=$(./04_string_split/split_llm | grep "Time" | awk '{print $3}')

echo ""
echo "| Benchmark Test                   | C (-O3) | llmlang |"
echo "|----------------------------------|---------|---------|"
echo "| 01 Memory Layout (AoS) [2M]      | $AOS_C    | N/A     |"
echo "| 01 Memory Layout (SoA) [2M]      | $SOA_C    | $SOA_L    |"
echo "| 02 Vector Math [2M]              | $VEC_C    | $VEC_L    |"
echo "| 03 Recursion (Fibonacci 40)      | $FIB_C    | $FIB_L    |"
echo "| 04 String Parsing [2M]           | $SPL_C    | $SPL_L    |"
echo ""

echo "================================================="
echo "Done."
