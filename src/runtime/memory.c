#include "common.h"

void llm_drop(long s) {
    if (s > 1000) { 
        free((void*)s);
    }
}
