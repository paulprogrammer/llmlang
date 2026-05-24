#include <stdio.h>
#include <stdlib.h>
#include <time.h>

int main() {
    long count = 2000000; // 50M elements
    long* a = malloc(count * sizeof(long));
    long* b = malloc(count * sizeof(long));
    long* c = malloc(count * sizeof(long));

    for (long i = 0; i < count; i++) {
        a[i] = i;
        b[i] = i * 2;
    }

    clock_t start = clock();
    for (long i = 0; i < count; i++) {
        c[i] = a[i] + b[i];
    }
    clock_t end = clock();

    printf("Vector Add Time: %.0fms\n", (double)(end - start) / CLOCKS_PER_SEC * 1000.0);

    free(a);
    free(b);
    free(c);
    return 0;
}
