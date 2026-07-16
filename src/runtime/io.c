#include "common.h"

long llm_read(long handle) {
    if (handle > 1000) {
        LlmRtHeader* header = (LlmRtHeader*)(handle - sizeof(LlmRtHeader));
        if (header->magic == 0x4C4C4D52 && header->type == RT_TYPE_SOCKET) {
            int* sub_type = (int*)handle;
            if (*sub_type == 1) { // HttpServer
                if (llm_http_server_accept) {
                    return llm_http_server_accept((HttpServer*)handle);
                }
                return 0;
            }
        }
        if (header->magic == 0x4C4C4D52 && header->type == RT_TYPE_FILE) {
            LlmFile* lf = (LlmFile*)handle;
            char stack_buf[4096];
            if (!fgets(stack_buf, sizeof(stack_buf), lf->fp)) {
                return 0;
            }
            stack_buf[strcspn(stack_buf, "\n")] = 0;
            return (long)llm_rt_strdup(stack_buf);
        }
    }
    char stack_buf[4096];
    if (!fgets(stack_buf, sizeof(stack_buf), fdopen((int)handle, "r"))) {
        return 0;
    }
    stack_buf[strcspn(stack_buf, "\n")] = 0;
    return (long)llm_rt_strdup(stack_buf);
}

long llm_write(long handle, long s) {
    if (handle > 1000) {
        LlmRtHeader* header = (LlmRtHeader*)(handle - sizeof(LlmRtHeader));
        if (header->magic == 0x4C4C4D52 && header->type == RT_TYPE_SOCKET) {
            int* sub_type = (int*)handle;
            if (*sub_type == 2) { // HttpRequest
                if (llm_http_server_respond) {
                    return llm_http_server_respond((HttpRequest*)handle, (char*)s);
                }
                return 0;
            }
        }
        if (header->magic == 0x4C4C4D52 && header->type == RT_TYPE_FILE) {
            LlmFile* lf = (LlmFile*)handle;
            char* src = (char*)s;
            if (!src) return 0;
            size_t len = strlen(src);
            size_t written = fwrite(src, 1, len, lf->fp);
            return (long)written;
        }
    }
    char* src = (char*)s;
    if (!src) return 0;
    return (long)write((int)handle, src, strlen(src));
}

long llm_getenv(long k) {
    char* key = (char*)k;
    if (!key) return (long)llm_rt_strdup("");
    char* val = getenv(key);
    if (!val) return (long)llm_rt_strdup("");
    return (long)llm_rt_strdup(val);
}

__thread llm_trap_frame_t* llm_trap_stack = NULL;

void llm_panic(long msg) {
    char* s = (char*)msg;
    // Record the message so trap fallbacks can recover it via `env "LLM_PANIC_MSG"`.
    setenv("LLM_PANIC_MSG", s ? s : "Unknown error", 1);
    if (llm_trap_stack) {
        longjmp(llm_trap_stack->buf, 1);
    }
    fprintf(stderr, "🚨 PANIC: %s\n", s ? s : "Unknown error");
    exit(1);
}

void llm_push_trap(llm_trap_frame_t* frame) {
    frame->next = llm_trap_stack;
    llm_trap_stack = frame;
}

void llm_pop_trap() {
    if (llm_trap_stack) {
        llm_trap_stack = llm_trap_stack->next;
    }
}

long llm_try(long (*body)(void*), void* arg, long (*fallback)(void*), void* farg) {
    llm_trap_frame_t frame;
    llm_push_trap(&frame);
    if (setjmp(frame.buf) == 0) {
        long res = body(arg);
        llm_pop_trap();
        return res;
    } else {
        llm_pop_trap();
        return fallback(farg);
    }
}
