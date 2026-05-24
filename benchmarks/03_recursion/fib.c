#include <stdio.h>
#include <time.h>

long fib(long n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}

int main() {
    long n = 40;
    clock_t start = clock();
    long result = fib(n);
    clock_t end = clock();

    printf("Fib(%ld) = %ld\n", n, result);
    printf("Fib Time: %.0fms\n", (double)(end - start) / CLOCKS_PER_SEC * 1000.0);
    return 0;
}
