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
    RT_TYPE_DB = 4,
    RT_TYPE_FILE = 5,
    RT_TYPE_TLS_CONFIG = 6,
    RT_TYPE_TLS_CTX = 7,
    RT_TYPE_CRYPTO_KEY = 8
} LlmRtType;

typedef struct {
    unsigned int magic;
    unsigned short type;
    unsigned short ref_cnt;
} LlmRtHeader;

typedef struct {
    int type; // 1 = HttpServer
    int fd;
    void* tls_config;
} HttpServer;

typedef struct {
    int type; // 2 = HttpRequest
    int client_fd;
    char* method;
    char* path;
    char* body;
    void* tls_ctx;
} HttpRequest;

void* llm_rt_alloc(size_t size, LlmRtType type);
char* llm_rt_strdup(const char* s);
void register_json_root(void* cell);
void unregister_json_root(void* cell);
void llm_drop(long s);
long llm_dup(long s);

__attribute__((weak)) long llm_http_client(long method, long url, long body);
__attribute__((weak)) long llm_http_server(long op, long arg);
__attribute__((weak)) long llm_https_server(long port, long cert, long key, long legacy);
__attribute__((weak)) long llm_http_server_accept(HttpServer* server);
__attribute__((weak)) long llm_http_server_respond(HttpRequest* req, char* data_str);
__attribute__((weak)) void llm_drop_json(long s);
typedef struct {
    FILE* fp;
} LlmFile;

__attribute__((weak)) long llm_file_open(long path, long mode);
__attribute__((weak)) long file_open(long path, long mode);
__attribute__((weak)) long file_close(long handle);
__attribute__((weak)) void llm_drop_socket(long s);
__attribute__((weak)) void llm_drop_tls_config(long s);
__attribute__((weak)) void llm_drop_tls_ctx(long s);
__attribute__((weak)) void llm_drop_crypto_key(long s);

__attribute__((weak)) long crypto_sign(long key_ptr, long data_ptr);
__attribute__((weak)) long crypto_verify(long key_ptr, long sig_ptr, long data_ptr);
__attribute__((weak)) long crypto_encrypt(long key_ptr, long data_ptr);
__attribute__((weak)) long crypto_decrypt(long key_ptr, long enc_ptr);
__attribute__((weak)) long cms_unwrap(long env_ptr, long key_ptr);

__attribute__((weak)) long http_serve(long port_ptr);
__attribute__((weak)) long http_https_serve(long port_ptr, long cert_ptr, long key_ptr, long legacy);
__attribute__((weak)) long http_accept(long server_ptr);
__attribute__((weak)) long http_respond(long req_ptr, long data_ptr);

void* llm_tls_config_init(const char* cert_str, const char* key_str, int enable_legacy);
void* llm_tls_ctx_init(void* config_ptr, int* fd_ptr);
int llm_tls_handshake(void* ctx);
ssize_t llm_tls_read(void* ctx, unsigned char* buf, size_t len);
ssize_t llm_tls_write(void* ctx, const unsigned char* buf, size_t len);

#endif
