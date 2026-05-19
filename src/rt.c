#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <regex.h>
#include <pthread.h>
#include <unistd.h>

// --- String and System Runtime ---

long llm_len(long s) {
    if (s == 0) return 0;
    return (long)strlen((char*)s);
}

long llm_cat(long l, long r) {
    char* s1 = (char*)l;
    char* s2 = (char*)r;
    size_t len1 = s1 ? strlen(s1) : 0;
    size_t len2 = s2 ? strlen(s2) : 0;
    char* res = malloc(len1 + len2 + 1);
    if (s1) strcpy(res, s1);
    else res[0] = '\0';
    if (s2) strcpy(res + len1, s2);
    res[len1 + len2] = '\0';
    return (long)res;
}

long llm_sub(long s, long start, long len) {
    char* src = (char*)s;
    if (!src) return 0;
    size_t src_len = strlen(src);
    if (start < 0) start = 0;
    if (start >= src_len) return (long)strdup("");
    if (len < 0) len = 0;
    if (start + len > src_len) len = src_len - start;
    char* res = malloc(len + 1);
    strncpy(res, src + start, len);
    res[len] = '\0';
    return (long)res;
}

long llm_loc(long s, long p) {
    char* src = (char*)s;
    char* pat = (char*)p;
    if (!src || !pat) return -1;
    char* found = strstr(src, pat);
    if (!found) return -1;
    return (long)(found - src);
}

long llm_reg(long s, long r) {
    char* src = (char*)s;
    char* re = (char*)r;
    if (!src || !re) return 0;
    regex_t regex;
    int reti = regcomp(&regex, re, REG_EXTENDED);
    if (reti) return 0;
    reti = regexec(&regex, src, 0, NULL, 0);
    regfree(&regex);
    return reti == 0 ? 1 : 0;
}

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
    FILE* f = fdopen((int)handle, "w");
    if (!f) return 0;
    long ret = (long)fprintf(f, "%s", src);
    fflush(f);
    return ret;
}

void llm_drop(long s) {
    if (s > 1000) { 
        free((void*)s);
    }
}

long llm_itoa(long n) {
    char* buffer = malloc(32);
    sprintf(buffer, "%ld", n);
    return (long)buffer;
}

long llm_split(long s, long d, long index) {
    char* src = (char*)s;
    char* delim = (char*)d;
    if (!src || !delim) return (long)strdup("");
    char* copy = strdup(src);
    char* token = strtok(copy, delim);
    long current = 0;
    while (token != NULL) {
        if (current == index) {
            char* res = strdup(token);
            free(copy);
            return (long)res;
        }
        token = strtok(NULL, delim);
        current++;
    }
    free(copy);
    return (long)strdup("");
}

// --- Managed Thread Pool ---

#define MAX_THREADS 8
#define QUEUE_SIZE 64

typedef struct {
    void* (*fn)(void*);
    void* arg;
    void** result_ptr;
    pthread_mutex_t* task_mutex;
    pthread_cond_t* task_cond;
    int* done;
} llm_work_item_t;

typedef struct {
    llm_work_item_t queue[QUEUE_SIZE];
    int head, tail, count;
    pthread_mutex_t mutex;
    pthread_cond_t cond;
    pthread_t threads[MAX_THREADS];
    int shutdown;
} llm_pool_t;

static llm_pool_t* pool = NULL;

void* llm_worker(void* arg) {
    while (1) {
        pthread_mutex_lock(&pool->mutex);
        while (pool->count == 0 && !pool->shutdown) {
            pthread_cond_wait(&pool->cond, &pool->mutex);
        }
        if (pool->shutdown) {
            pthread_mutex_unlock(&pool->mutex);
            break;
        }
        llm_work_item_t work = pool->queue[pool->head];
        pool->head = (pool->head + 1) % QUEUE_SIZE;
        pool->count--;
        pthread_mutex_unlock(&pool->mutex);

        // Execute task
        void* result = work.fn(work.arg);

        pthread_mutex_lock(work.task_mutex);
        *work.result_ptr = result;
        *work.done = 1;
        pthread_cond_signal(work.task_cond);
        pthread_mutex_unlock(work.task_mutex);
    }
    return NULL;
}

void llm_init_pool() {
    if (pool) return;
    pool = calloc(1, sizeof(llm_pool_t));
    pthread_mutex_init(&pool->mutex, NULL);
    pthread_cond_init(&pool->cond, NULL);
    for (int i = 0; i < MAX_THREADS; i++) {
        pthread_create(&pool->threads[i], NULL, llm_worker, NULL);
    }
}

// Handle returned to LLVM
typedef struct {
    void* result;
    pthread_mutex_t mutex;
    pthread_cond_t cond;
    int done;
} llm_task_handle_t;

long llm_fork(long fn_ptr, long arg_ptr) {
    if (!pool) llm_init_pool();

    llm_task_handle_t* handle = calloc(1, sizeof(llm_task_handle_t));
    pthread_mutex_init(&handle->mutex, NULL);
    pthread_cond_init(&handle->cond, NULL);

    pthread_mutex_lock(&pool->mutex);
    if (pool->count < QUEUE_SIZE) {
        llm_work_item_t* work = &pool->queue[pool->tail];
        work->fn = (void* (*)(void*))fn_ptr;
        work->arg = (void*)arg_ptr;
        work->result_ptr = &handle->result;
        work->task_mutex = &handle->mutex;
        work->task_cond = &handle->cond;
        work->done = &handle->done;
        pool->tail = (pool->tail + 1) % QUEUE_SIZE;
        pool->count++;
        pthread_cond_signal(&pool->cond);
        pthread_mutex_unlock(&pool->mutex);
    } else {
        pthread_mutex_unlock(&pool->mutex);
        // Queue full, execute synchronously
        handle->result = ((void* (*)(void*))fn_ptr)((void*)arg_ptr);
        handle->done = 1;
    }
    return (long)handle;
}

long llm_join(long handle_ptr) {
    llm_task_handle_t* handle = (llm_task_handle_t*)handle_ptr;
    pthread_mutex_lock(&handle->mutex);
    while (!handle->done) {
        pthread_cond_wait(&handle->cond, &handle->mutex);
    }
    void* res = handle->result;
    pthread_mutex_unlock(&handle->mutex);
    
    pthread_mutex_destroy(&handle->mutex);
    pthread_cond_destroy(&handle->cond);
    free(handle);
    
    return (long)res;
}
