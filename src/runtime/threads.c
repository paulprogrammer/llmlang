#include "common.h"

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
