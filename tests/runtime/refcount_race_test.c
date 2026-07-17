// Regression test for maturity finding #3: non-atomic refcounts.
// llm_fork hands the same managed pointers to pool threads, so llm_dup /
// llm_drop must be thread-safe. With the old plain `ref_cnt++/--`, racing
// threads lose updates: the count drifts, the object is freed while
// references remain (magic clobbered), or the final count is skewed.
//
// Built and run by llm-test:
//   clang tests/runtime/refcount_race_test.c src/runtime/memory.c \
//         -Isrc/runtime -lpthread
#include "common.h"

#define N_THREADS 8
#define N_ITERS 200000

static char* obj;

static void* worker(void* arg) {
    (void)arg;
    for (int i = 0; i < N_ITERS; i++) {
        llm_dup((long)obj);
        llm_drop((long)obj);
    }
    return NULL;
}

int main(void) {
    obj = llm_rt_strdup("refcount race test object");
    if (!obj) {
        fprintf(stderr, "FAIL: allocation failed\n");
        return 1;
    }

    pthread_t threads[N_THREADS];
    for (int i = 0; i < N_THREADS; i++) {
        if (pthread_create(&threads[i], NULL, worker, NULL) != 0) {
            fprintf(stderr, "FAIL: pthread_create\n");
            return 1;
        }
    }
    for (int i = 0; i < N_THREADS; i++) {
        pthread_join(threads[i], NULL);
    }

    LlmRtHeader* header = (LlmRtHeader*)((char*)obj - sizeof(LlmRtHeader));
    if (header->magic != 0x4C4C4D52) {
        fprintf(stderr, "FAIL: object was freed while references remained\n");
        return 1;
    }
    unsigned int rc = header->ref_cnt;
    if (rc != 1) {
        fprintf(stderr, "FAIL: refcount is %u after balanced dup/drop (expected 1)\n", rc);
        return 1;
    }

    llm_drop((long)obj);
    printf("refcount race test: OK (%d threads x %d dup/drop)\n", N_THREADS, N_ITERS);
    return 0;
}
