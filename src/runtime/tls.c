#include "common.h"
#include <mbedtls/ssl.h>
#include <mbedtls/net_sockets.h>
#include <mbedtls/entropy.h>
#include <mbedtls/ctr_drbg.h>
#include <mbedtls/x509_crt.h>
#include <mbedtls/pk.h>
#include <mbedtls/version.h>
#include <mbedtls/error.h>
#include <errno.h>

// I/O Callbacks for non-blocking sockets
int bio_send_cb(void *ctx, const unsigned char *buf, size_t len) {
    int fd = *(int*)ctx;
    ssize_t ret = write(fd, buf, len);
    if (ret < 0) {
        if (errno == EAGAIN || errno == EWOULDBLOCK) {
            return MBEDTLS_ERR_SSL_WANT_WRITE;
        }
        return MBEDTLS_ERR_NET_SEND_FAILED;
    }
    return (int)ret;
}

int bio_recv_cb(void *ctx, unsigned char *buf, size_t len) {
    int fd = *(int*)ctx;
    ssize_t ret = read(fd, buf, len);
    if (ret < 0) {
        if (errno == EAGAIN || errno == EWOULDBLOCK) {
            return MBEDTLS_ERR_SSL_WANT_READ;
        }
        return MBEDTLS_ERR_NET_RECV_FAILED;
    }
    return (int)ret;
}

// Global RNG structure for TLS
typedef struct {
    mbedtls_entropy_context entropy;
    mbedtls_ctr_drbg_context ctr_drbg;
} TlsRng;

typedef struct {
    mbedtls_ssl_config conf;
    mbedtls_x509_crt cert;
    mbedtls_pk_context key;
    TlsRng rng;
} LlmTlsConfig;

// Initialize a TLS configuration
void* llm_tls_config_init(const char* cert_str, const char* key_str, int enable_legacy) {
    LlmTlsConfig* config = (LlmTlsConfig*)llm_rt_alloc(sizeof(LlmTlsConfig), RT_TYPE_TLS_CONFIG);
    
    mbedtls_ssl_config_init(&config->conf);
    mbedtls_x509_crt_init(&config->cert);
    mbedtls_pk_init(&config->key);
    mbedtls_entropy_init(&config->rng.entropy);
    mbedtls_ctr_drbg_init(&config->rng.ctr_drbg);

    const char* pers = "llm_tls_server";
    mbedtls_ctr_drbg_seed(&config->rng.ctr_drbg, mbedtls_entropy_func, &config->rng.entropy,
                          (const unsigned char *)pers, strlen(pers));

    mbedtls_ssl_config_defaults(&config->conf,
                                MBEDTLS_SSL_IS_SERVER,
                                MBEDTLS_SSL_TRANSPORT_STREAM,
                                MBEDTLS_SSL_PRESET_DEFAULT);

    if (!enable_legacy) {
        mbedtls_ssl_conf_min_version(&config->conf, MBEDTLS_SSL_MAJOR_VERSION_3, MBEDTLS_SSL_MINOR_VERSION_3); // TLS 1.2
    } else {
        mbedtls_ssl_conf_min_version(&config->conf, MBEDTLS_SSL_MAJOR_VERSION_3, MBEDTLS_SSL_MINOR_VERSION_3); // TLS 1.2 minimum for v3.x
    }

    mbedtls_ssl_conf_rng(&config->conf, mbedtls_ctr_drbg_random, &config->rng.ctr_drbg);

    if (mbedtls_x509_crt_parse(&config->cert, (const unsigned char*)cert_str, strlen(cert_str) + 1) != 0) {
        return NULL; // Failed to parse cert
    }

#if MBEDTLS_VERSION_NUMBER >= 0x03000000
    if (mbedtls_pk_parse_key(&config->key, (const unsigned char*)key_str, strlen(key_str) + 1, NULL, 0,
                             mbedtls_ctr_drbg_random, &config->rng.ctr_drbg) != 0) {
#else
    if (mbedtls_pk_parse_key(&config->key, (const unsigned char*)key_str, strlen(key_str) + 1, NULL, 0) != 0) {
#endif
        return NULL; // Failed to parse key
    }

    mbedtls_ssl_conf_own_cert(&config->conf, &config->cert, &config->key);

    return config;
}

// Setup a TLS context for a specific accepted socket
void* llm_tls_ctx_init(void* config_ptr, int* fd_ptr) {
    LlmTlsConfig* config = (LlmTlsConfig*)config_ptr;
    mbedtls_ssl_context* ssl = (mbedtls_ssl_context*)llm_rt_alloc(sizeof(mbedtls_ssl_context), RT_TYPE_TLS_CTX);
    
    mbedtls_ssl_init(ssl);
    if (mbedtls_ssl_setup(ssl, &config->conf) != 0) {
        return NULL;
    }

    mbedtls_ssl_set_bio(ssl, fd_ptr, bio_send_cb, bio_recv_cb, NULL);
    return ssl;
}

void llm_drop_tls_config(long s) {
    LlmTlsConfig* config = (LlmTlsConfig*)s;
    if (config) {
        mbedtls_x509_crt_free(&config->cert);
        mbedtls_pk_free(&config->key);
        mbedtls_ssl_config_free(&config->conf);
        mbedtls_ctr_drbg_free(&config->rng.ctr_drbg);
        mbedtls_entropy_free(&config->rng.entropy);
    }
}

void llm_drop_tls_ctx(long s) {
    mbedtls_ssl_context* ssl = (mbedtls_ssl_context*)s;
    if (ssl) {
        mbedtls_ssl_free(ssl);
    }
}

int llm_tls_handshake(void* ctx) {
    mbedtls_ssl_context* ssl = (mbedtls_ssl_context*)ctx;
    int ret;
    while ((ret = mbedtls_ssl_handshake(ssl)) != 0) {
        if (ret != MBEDTLS_ERR_SSL_WANT_READ && ret != MBEDTLS_ERR_SSL_WANT_WRITE) {
            return -1;
        }
        // Basic non-blocking loop (in a real event loop we would return EAGAIN)
        // For simplicity in our accept loop timeout we retry locally.
        usleep(1000);
    }
    return 0;
}

ssize_t llm_tls_read(void* ctx, unsigned char* buf, size_t len) {
    mbedtls_ssl_context* ssl = (mbedtls_ssl_context*)ctx;
    int ret = mbedtls_ssl_read(ssl, buf, len);
    if (ret == MBEDTLS_ERR_SSL_WANT_READ || ret == MBEDTLS_ERR_SSL_WANT_WRITE) {
        errno = EAGAIN;
        return -1;
    }
    if (ret < 0) return -1;
    return ret;
}

ssize_t llm_tls_write(void* ctx, const unsigned char* buf, size_t len) {
    mbedtls_ssl_context* ssl = (mbedtls_ssl_context*)ctx;
    int ret = mbedtls_ssl_write(ssl, buf, len);
    if (ret == MBEDTLS_ERR_SSL_WANT_READ || ret == MBEDTLS_ERR_SSL_WANT_WRITE) {
        errno = EAGAIN;
        return -1;
    }
    if (ret < 0) return -1;
    return ret;
}
