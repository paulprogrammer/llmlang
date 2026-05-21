#include "common.h"

void* llm_rt_alloc(size_t size, LlmRtType type) {
    LlmRtHeader* header = malloc(sizeof(LlmRtHeader) + size);
    if (!header) return NULL;
    header->magic = 0x4C4C4D52;
    header->type = type;
    header->ref_cnt = 1;
    return (void*)((char*)header + sizeof(LlmRtHeader));
}

char* llm_rt_strdup(const char* s) {
    if (!s) return NULL;
    size_t len = strlen(s);
    char* copy = llm_rt_alloc(len + 1, RT_TYPE_STRING);
    if (copy) {
        strcpy(copy, s);
    }
    return copy;
}

void* llm_alloc(long size) {
    return malloc((size_t)size);
}

void llm_drop(long s) {
    if (s <= 1000) return;
    LlmRtHeader* header = (LlmRtHeader*)(s - sizeof(LlmRtHeader));
    if (header->magic == 0x4C4C4D52) {
        header->magic = 0; // Prevent double drop!
        switch (header->type) {
            case RT_TYPE_JSON: {
                if (llm_drop_json) {
                    llm_drop_json(s);
                }
                break;
            }
            case RT_TYPE_SOCKET: {
                if (llm_drop_socket) {
                    llm_drop_socket(s);
                }
                break;
            }
            case RT_TYPE_STRING:
            case RT_TYPE_RAW:
            default:
                break;
        }
        free(header);
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
