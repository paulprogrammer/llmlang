// High Arithmetic Intensity Benchmark (C)
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

int main() {
    long count = 2000000;
    long* data = malloc(count * sizeof(long));

    for (long i = 0; i < count; i++) {
        data[i] = i;
    }

    clock_t start = clock();

    for (long i = 0; i < count; i++) {
        long x = data[i];
        for (int j = 0; j < 500; j++) {
            x = (x * x + x) / 2;
        }
        data[i] = x;
    }

    clock_t end = clock();
    printf("Compute Time: %.0fms\n", (double)(end - start) / CLOCKS_PER_SEC * 1000.0);

    free(data);
    return 0;
}
