#include "common.h"

// Defined as global constants in the LLVM module
extern const long llm_max_threads;
extern const long llm_queue_size;

typedef struct {
    void* (*fn)(void*);
    void* arg;
    void** result_ptr;
    pthread_mutex_t* task_mutex;
    pthread_cond_t* task_cond;
    int* done;
} llm_work_item_t;

typedef struct {
    llm_work_item_t* queue;
    int head, tail, count;
    pthread_mutex_t mutex;
    pthread_cond_t cond;
    pthread_t* threads;
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
        pool->head = (pool->head + 1) % llm_queue_size;
        pool->count--;
        pthread_mutex_unlock(&pool->mutex);

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
    pool->queue = calloc(llm_queue_size, sizeof(llm_work_item_t));
    pool->threads = calloc(llm_max_threads, sizeof(pthread_t));
    
    pthread_mutex_init(&pool->mutex, NULL);
    pthread_cond_init(&pool->cond, NULL);
    for (int i = 0; i < llm_max_threads; i++) {
        pthread_create(&pool->threads[i], NULL, llm_worker, NULL);
    }
}

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
    if (pool->count < llm_queue_size) {
        llm_work_item_t* work = &pool->queue[pool->tail];
        work->fn = (void* (*)(void*))fn_ptr;
        work->arg = (void*)arg_ptr;
        work->result_ptr = &handle->result;
        work->task_mutex = &handle->mutex;
        work->task_cond = &handle->cond;
        work->done = &handle->done;
        pool->tail = (pool->tail + 1) % llm_queue_size;
        pool->count++;
        pthread_cond_signal(&pool->cond);
        pthread_mutex_unlock(&pool->mutex);
    } else {
        pthread_mutex_unlock(&pool->mutex);
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
