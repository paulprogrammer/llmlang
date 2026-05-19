#include <stdio.h>
#include <stdlib.h>
#include <time.h>

int main() {
    long count = 10000000;
    long* id = malloc(count * sizeof(long));
    long* x = malloc(count * sizeof(long));
    long* y = malloc(count * sizeof(long));
    long* z = malloc(count * sizeof(long));

    // 1. Initialize
    clock_t start_init = clock();
    for (long i = 0; i < count; i++) {
        id[i] = i;
        x[i] = 1;
        y[i] = 2;
        z[i] = 3;
    }
    clock_t end_init = clock();
    printf("Init Time: %f s\n", (double)(end_init - start_init) / CLOCKS_PER_SEC);

    // 2. Sum Column X
    clock_t start_sum = clock();
    long total = 0;
    for (long i = 0; i < count; i++) {
        total += x[i];
    }
    clock_t end_sum = clock();
    printf("Sum: %ld\n", total);
    printf("Sum Time: %f s\n", (double)(end_sum - start_sum) / CLOCKS_PER_SEC);

    free(id); free(x); free(y); free(z);
    return 0;
}
