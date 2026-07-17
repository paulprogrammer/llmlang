#include "common.h"

void* llm_rt_alloc(size_t size, LlmRtType type) {
    LlmRtHeader* header = malloc(sizeof(LlmRtHeader) + size);
    if (!header) return NULL;
    header->magic = RT_MAGIC;
    header->type = type;
    atomic_init(&header->ref_cnt, 1);
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
    void* ptr = NULL;
    if (posix_memalign(&ptr, 64, (size_t)size) != 0) {
        return NULL;
    }
    return ptr;
}

void llm_drop(long s) {
    if (s <= RT_MIN_HANDLE) return;
    LlmRtHeader* header = (LlmRtHeader*)(s - sizeof(LlmRtHeader));
    if (header->magic == RT_MAGIC) {
        // fetch_sub returns the pre-decrement value: only the caller that
        // releases the last reference proceeds to destroy. acq_rel pairs
        // the release of every other drop with the acquire of the final
        // one, so writes made through the object are visible before free.
        if (atomic_fetch_sub_explicit(&header->ref_cnt, 1, memory_order_acq_rel) != 1) return;
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
            case RT_TYPE_FILE: {
                LlmFile* lf = (LlmFile*)s;
                if (lf->fp) {
                    fclose(lf->fp);
                    lf->fp = NULL;
                }
                break;
            }
            case RT_TYPE_TLS_CONFIG: {
                if (llm_drop_tls_config) {
                    llm_drop_tls_config(s);
                }
                break;
            }
            case RT_TYPE_TLS_CTX: {
                if (llm_drop_tls_ctx) {
                    llm_drop_tls_ctx(s);
                }
                break;
            }
            case RT_TYPE_CRYPTO_KEY: {
                if (llm_drop_crypto_key) {
                    llm_drop_crypto_key(s);
                }
                break;
            }
            case RT_TYPE_DB: {
                if (llm_drop_db) {
                    llm_drop_db(s);
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

long llm_dup(long s) {
    if (s <= RT_MIN_HANDLE) return s;
    LlmRtHeader* header = (LlmRtHeader*)(s - sizeof(LlmRtHeader));
    if (header->magic == RT_MAGIC) {
        // relaxed is enough: taking a new reference needs no ordering,
        // only that the increment itself is not lost.
        atomic_fetch_add_explicit(&header->ref_cnt, 1, memory_order_relaxed);
    }
    return s;
}


void llm_drop_soa(long* instance, long field_count) {
    if (!instance || (long)instance < RT_MIN_HANDLE) return;
    
    // Free each column (index 1 to field_count)
    for (int i = 0; i < field_count; i++) {
        long col = instance[i + 1];
        if (col > RT_MIN_HANDLE) {
            free((void*)col);
        }
    }
    // Free the struct itself
    free(instance);
}
