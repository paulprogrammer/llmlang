#include "common.h"

void* llm_alloc(long size) {
    return malloc((size_t)size);
}

void llm_drop(long s) {
    if (s > 1000) { 
        free((void*)s);
    }
}

void llm_drop_soa(long* instance, long field_count) {
    if (!instance || (long)instance < 1000) return;
    
    // Free each column (index 1 to field_count)
    for (int i = 0; i < field_count; i++) {
        long col = instance[i + 1];
        if (col > 1000) {
            free((void*)col);
        }
    }
    // Free the struct itself
    free(instance);
}
