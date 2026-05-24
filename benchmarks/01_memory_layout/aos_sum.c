#include <stdio.h>
#include <stdlib.h>
#include <time.h>

typedef struct {
    long id;
    long x;
    long y;
    long z;
} Vector;

int main() {
    long count = 2000000;
    Vector* v = malloc(count * sizeof(Vector));

    // 1. Initialize
    clock_t start_init = clock();
    for (long i = 0; i < count; i++) {
        v[i].id = i;
        v[i].x = 1;
        v[i].y = 2;
        v[i].z = 3;
    }
    clock_t end_init = clock();
    printf("Init Time: %.0fms\n", (double)(end_init - start_init) / CLOCKS_PER_SEC * 1000.0);

    // 2. Sum Column X
    clock_t start_sum = clock();
    long total = 0;
    for (long i = 0; i < count; i++) {
        total += v[i].x;
    }
    clock_t end_sum = clock();
    printf("Sum: %ld\n", total);
    printf("Sum Time: %.0fms\n", (double)(end_sum - start_sum) / CLOCKS_PER_SEC * 1000.0);

    free(v);
    return 0;
}
