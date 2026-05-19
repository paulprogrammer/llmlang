#include "common.h"

long llm_read(long handle) {
    char* buffer = malloc(4096);
    if (!fgets(buffer, 4096, fdopen((int)handle, "r"))) {
        free(buffer);
        return 0;
    }
    buffer[strcspn(buffer, "\n")] = 0;
    return (long)buffer;
}

long llm_write(long handle, long s) {
    char* src = (char*)s;
    if (!src) return 0;
    return (long)write((int)handle, src, strlen(src));
}

long llm_getenv(long k) {
    char* key = (char*)k;
    if (!key) return (long)strdup("");
    char* val = getenv(key);
    if (!val) return (long)strdup("");
    return (long)strdup(val);
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
        // Upon longjmp, the stack has been unwound to this point.
        // We need to pop the trap frame because it was never popped.
        llm_pop_trap();
        return fallback(farg);
    }
}
