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

void llm_panic(long msg) {
    char* s = (char*)msg;
    fprintf(stderr, "🚨 PANIC: %s\n", s ? s : "Unknown error");
    exit(1);
}
