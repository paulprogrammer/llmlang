#ifndef LLM_RT_COMMON_H
#define LLM_RT_COMMON_H

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <regex.h>
#include <pthread.h>
#include <unistd.h>
#include <time.h>
#include <setjmp.h>

typedef struct llm_trap_frame {
    jmp_buf buf;
    struct llm_trap_frame* next;
} llm_trap_frame_t;

extern __thread llm_trap_frame_t* llm_trap_stack;

typedef enum {
    RT_TYPE_RAW = 0,
    RT_TYPE_STRING = 1,
    RT_TYPE_JSON = 2,
    RT_TYPE_SOCKET = 3,
    RT_TYPE_DB = 4
} LlmRtType;

typedef struct {
    unsigned int magic;
    unsigned short type;
    unsigned short ref_cnt;
} LlmRtHeader;

void* llm_rt_alloc(size_t size, LlmRtType type);
char* llm_rt_strdup(const char* s);
void register_json_root(void* cell);
void unregister_json_root(void* cell);

#endif
