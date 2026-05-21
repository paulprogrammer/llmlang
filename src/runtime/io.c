#include "common.h"

long llm_read(long handle) {
    char stack_buf[4096];
    if (!fgets(stack_buf, sizeof(stack_buf), fdopen((int)handle, "r"))) {
        return 0;
    }
    stack_buf[strcspn(stack_buf, "\n")] = 0;
    return (long)llm_rt_strdup(stack_buf);
}

long llm_write(long handle, long s) {
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
    if (llm_trap_stack) {
        longjmp(llm_trap_stack->buf, 1);
    }
    char* s = (char*)msg;
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
