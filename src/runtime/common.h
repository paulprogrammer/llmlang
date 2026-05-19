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

#endif
