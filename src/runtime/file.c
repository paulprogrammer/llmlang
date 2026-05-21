#include "common.h"

long llm_file_open(long path, long mode) {
    const char* path_str = (const char*)path;
    const char* mode_str = (const char*)mode;
    if (!path_str || !mode_str) {
        return 0;
    }

    FILE* fp = fopen(path_str, mode_str);
    if (!fp) {
        return 0;
    }

    LlmFile* lf = (LlmFile*)llm_rt_alloc(sizeof(LlmFile), RT_TYPE_FILE);
    if (!lf) {
        fclose(fp);
        return 0;
    }
    lf->fp = fp;
    return (long)lf;
}

long file_open(long path, long mode) {
    return llm_file_open(path, mode);
}

long file_close(long handle) {
    llm_drop(handle);
    return 0;
}
