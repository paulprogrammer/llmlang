#include <stdio.h>
#include <string.h>
#include <time.h>

int main() {
    long count = 2000000;
    clock_t start = clock();
    
    long dummy = 0;
    const char* str = "apple,banana,cherry,date,elderberry";
    
    for (long i = 0; i < count; i++) {
        int token_idx = 0;
        const char* ptr = str;
        while (*ptr && token_idx < 2) {
            if (*ptr == ',') token_idx++;
            ptr++;
        }
        if (*ptr) dummy++;
    }
    clock_t end = clock();

    printf("Split loops: %ld\n", count);
    printf("Split Time: %.0fms\n", (double)(end - start) / CLOCKS_PER_SEC * 1000.0);
    return 0;
}
