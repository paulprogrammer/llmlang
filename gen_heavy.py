import os

DEPTH = 500

c_code = f"""// High Arithmetic Intensity Benchmark (C)
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

int main() {{
    long count = 2000000;
    long* data = malloc(count * sizeof(long));

    for (long i = 0; i < count; i++) {{
        data[i] = i;
    }}

    clock_t start = clock();

    for (long i = 0; i < count; i++) {{
        long x = data[i];
        for (int j = 0; j < {DEPTH}; j++) {{
            x = (x * x + x) / 2;
        }}
        data[i] = x;
    }}

    clock_t end = clock();
    printf("Compute Time: %.0fms\\n", (double)(end - start) / CLOCKS_PER_SEC * 1000.0);

    free(data);
    return 0;
}}
"""

llm_code = f"""// High Arithmetic Intensity Benchmark (llmlang)
# Data val

: heavy_compute x
"""

for i in range(DEPTH):
    if i == 0:
        llm_code += f"    L x{i} / + * $ x $ x $ x 2\n"
    else:
        llm_code += f"    L x{i} / + * $ x{i-1} $ x{i-1} $ x{i-1} 2\n"

llm_code += f"    $ x{DEPTH-1}\n\n"

llm_code += """: main
    L count 2000000
    L data N Data $ count

    L start tns
    L dummy1 map $ data "val" heavy_compute
    L end tns

    . ) 1 sc "Compute Time: " sc str / - $ end $ start 1000000 "ms\\n"
    0
"""

with open("benchmarks/05_gpu_hoisting/heavy_compute.c", "w") as f:
    f.write(c_code)

with open("benchmarks/05_gpu_hoisting/heavy_compute.llm", "w") as f:
    f.write(llm_code)

print("Generated files.")
