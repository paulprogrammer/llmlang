#define _GNU_SOURCE
#include "common.h"
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <fcntl.h>
#include <poll.h>
#include <errno.h>
#include "picohttpparser.h"

// llm_http_client lives in http.c (shared curl_request routine, finding #23).

// Upper bound on a buffered request (headers + body). Without this, a
// client-supplied Content-Length (or an endless stream of headers with no
// terminator) drives realloc growth with no ceiling.
#define HTTP_MAX_REQUEST_BYTES (16 * 1024 * 1024)

static long listen_server(const char* port_str) {
    int port = atoi(port_str);
    if (port <= 0) port = 8080;

    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd < 0) {
        perror("socket");
        return 0;
    }

    int opt = 1;
    if (setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt)) < 0) {
        perror("setsockopt");
        close(fd);
        return 0;
    }

    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = INADDR_ANY;
    addr.sin_port = htons(port);

    if (bind(fd, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        perror("bind");
        close(fd);
        return 0;
    }

    if (listen(fd, 128) < 0) {
        perror("listen");
        close(fd);
        return 0;
    }

    HttpServer* server = (HttpServer*)llm_rt_alloc(sizeof(HttpServer), RT_TYPE_SOCKET);
    server->type = 1;
    server->fd = fd;
    server->tls_config = NULL;
    return (long)server;
}

long llm_https_server(long port_ptr, long cert_ptr, long key_ptr, long legacy) {
    long server_ptr = listen_server((char*)port_ptr);
    if (!server_ptr) return 0;
    
    HttpServer* server = (HttpServer*)server_ptr;
    server->tls_config = llm_tls_config_init((char*)cert_ptr, (char*)key_ptr, (int)legacy);
    
    if (!server->tls_config) {
        close(server->fd);
        server->fd = -1;
        // The garbage collector will free the server object
        return 0;
    }
    
    return server_ptr;
}

long llm_http_server(long op, long arg) {
    if (op == 0) {
        return listen_server((char*)arg);
    } else if (op == 1) {
        HttpRequest* req = (HttpRequest*)arg;
        if (!req || req->type != 2) return (long)llm_rt_strdup("");
        return (long)llm_rt_strdup(req->method ? req->method : "");
    } else if (op == 2) {
        HttpRequest* req = (HttpRequest*)arg;
        if (!req || req->type != 2) return (long)llm_rt_strdup("");
        return (long)llm_rt_strdup(req->path ? req->path : "");
    } else if (op == 3) {
        HttpRequest* req = (HttpRequest*)arg;
        if (!req || req->type != 2) return (long)llm_rt_strdup("");
        return (long)llm_rt_strdup(req->body ? req->body : "");
    } else if (op == 4) {
        HttpServer* server = (HttpServer*)arg;
        if (server && server->type == 1) {
            llm_drop(arg);
        }
        return 0;
    }
    return 0;
}

static char* dup_token(const char* ptr, size_t len) {
    char* res = llm_rt_alloc(len + 1, RT_TYPE_STRING);
    memcpy(res, ptr, len);
    res[len] = '\0';
    return res;
}

long llm_http_server_accept(HttpServer* server) {
    struct sockaddr_in client_addr;
    socklen_t client_len = sizeof(client_addr);
    int client_fd = accept(server->fd, (struct sockaddr*)&client_addr, &client_len);
    if (client_fd < 0) {
        return 0;
    }

    struct pollfd pfd;
    pfd.fd = client_fd;
    pfd.events = POLLIN;

    void* tls_ctx = NULL;
    if (server->tls_config) {
        // Safe to cast client_fd directly but we pass a pointer since mbedtls bio needs it.
        // We will store client_fd in req and pass &req->client_fd to llm_tls_ctx_init.
    }

    size_t buf_size = 4096;
    char* buf = malloc(buf_size);
    if (!buf) {
        close(client_fd);
        return 0;
    }
    size_t total_read = 0;

    HttpRequest* req = (HttpRequest*)llm_rt_alloc(sizeof(HttpRequest), RT_TYPE_SOCKET);
    req->type = 2;
    req->client_fd = client_fd;
    req->method = NULL;
    req->path = NULL;
    req->body = NULL;
    req->tls_ctx = NULL;
    req->headers = NULL;
    req->header_count = 0;

    if (server->tls_config) {
        req->tls_ctx = llm_tls_ctx_init(server->tls_config, &req->client_fd);
        if (!req->tls_ctx || llm_tls_handshake(req->tls_ctx) != 0) {
            free(buf);
            llm_drop((long)req); // This safely cleans up the fd and tls_ctx
            return 0;
        }
    }

    while (1) {
        int poll_ret = poll(&pfd, 1, 5000); // 5 seconds timeout
        if (poll_ret <= 0) {
            free(buf);
            llm_drop((long)req);
            return 0;
        }

        ssize_t bytes;
        if (req->tls_ctx) {
            bytes = llm_tls_read(req->tls_ctx, (unsigned char*)(buf + total_read), buf_size - total_read - 1);
        } else {
            bytes = read(req->client_fd, buf + total_read, buf_size - total_read - 1);
        }
        if (bytes < 0) {
            if (errno == EAGAIN || errno == EWOULDBLOCK) {
                // Transient TLS WANT_READ/WANT_WRITE or a spurious wakeup:
                // the request isn't done, go back and poll again.
                continue;
            }
            break;
        }
        if (bytes == 0) {
            break; // real EOF
        }
        size_t prev_read = total_read;
        total_read += bytes;
        buf[total_read] = '\0';

        const char *method = NULL;
        size_t method_len = 0;
        const char *path = NULL;
        size_t path_len = 0;
        int minor_version = 0;
        struct phr_header headers[100];
        size_t num_headers = sizeof(headers) / sizeof(headers[0]);

        int pret = phr_parse_request(buf, total_read, &method, &method_len, &path, &path_len,
                                     &minor_version, headers, &num_headers, prev_read);
        if (pret > 0) {
            size_t content_len = 0;
            int has_content_length = 0;
            for (size_t i = 0; i < num_headers; ++i) {
                if (headers[i].name != NULL && strncasecmp(headers[i].name, "Content-Length", headers[i].name_len) == 0) {
                    char val_buf[64];
                    size_t val_len = headers[i].value_len < 63 ? headers[i].value_len : 63;
                    memcpy(val_buf, headers[i].value, val_len);
                    val_buf[val_len] = '\0';
                    content_len = strtoul(val_buf, NULL, 10);
                    has_content_length = 1;
                    break;
                }
            }

            if (has_content_length) {
                size_t header_len = (size_t)pret;
                if (header_len + content_len > HTTP_MAX_REQUEST_BYTES) {
                    free(buf);
                    llm_drop((long)req);
                    return 0;
                }
                if (total_read < header_len + content_len) {
                    if (header_len + content_len >= buf_size - 1) {
                        size_t new_size = header_len + content_len + 1024;
                        char* new_buf = realloc(buf, new_size);
                        if (!new_buf) {
                            free(buf);
                            llm_drop((long)req);
                            return 0;
                        }
                        buf = new_buf;
                        buf_size = new_size;
                    }
                    continue;
                }
            }

            char* method_str = dup_token(method, method_len);
            char* path_str = dup_token(path, path_len);
            char* body_str = NULL;
            if (has_content_length) {
                size_t header_len = (size_t)pret;
                body_str = dup_token(buf + header_len, content_len);
            } else {
                body_str = dup_token("", 0);
            }

            req->header_count = (int)num_headers;
            if (num_headers > 0) {
                req->headers = malloc(num_headers * sizeof(HttpHeader));
                for (size_t i = 0; i < num_headers; i++) {
                    req->headers[i].name = dup_token(headers[i].name, headers[i].name_len);
                    req->headers[i].value = dup_token(headers[i].value, headers[i].value_len);
                }
            } else {
                req->headers = NULL;
            }

            req->method = method_str;
            req->path = path_str;
            req->body = body_str;

            free(buf);
            return (long)req;

        } else if (pret == -1) {
            free(buf);
            llm_drop((long)req);
            return 0;
        }

        // pret == -2: partial request
        if (total_read >= buf_size - 1) {
            if (buf_size * 2 > HTTP_MAX_REQUEST_BYTES) {
                free(buf);
                llm_drop((long)req);
                return 0;
            }
            size_t new_size = buf_size * 2;
            char* new_buf = realloc(buf, new_size);
            if (!new_buf) {
                free(buf);
                llm_drop((long)req);
                return 0;
            }
            buf = new_buf;
            buf_size = new_size;
        }
    }

    free(buf);
    llm_drop((long)req);
    return 0;
}

// Writes exactly `len` bytes, looping through partial writes and transient
// EAGAIN/EINTR instead of silently truncating the response on the first
// short write.
static void http_write(HttpRequest* req, const char* data, size_t len) {
    size_t total_written = 0;
    while (total_written < len) {
        ssize_t n;
        if (req->tls_ctx) {
            n = llm_tls_write(req->tls_ctx, (const unsigned char*)(data + total_written), len - total_written);
        } else {
            n = write(req->client_fd, data + total_written, len - total_written);
        }
        if (n > 0) {
            total_written += (size_t)n;
            continue;
        }
        if (n < 0 && errno == EINTR) {
            continue;
        }
        if (n < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
            usleep(1000);
            continue;
        }
        break; // real error (e.g. peer closed the connection); give up
    }
}


long llm_http_server_respond(HttpRequest* req, char* data_str) {
    if (!req || req->client_fd < 0) return 0;

    if (!data_str) data_str = "";

    if (strncmp(data_str, "HTTP/1.", 7) == 0) {
        http_write(req, data_str, strlen(data_str));
    } else {
        const char* content_type = "text/plain";
        char* p = data_str;
        while (*p == ' ' || *p == '\t' || *p == '\r' || *p == '\n') {
            p++;
        }
        if (*p == '{' || *p == '[') {
            content_type = "application/json";
        }

        char header_buf[512];
        int header_len = snprintf(header_buf, sizeof(header_buf),
            "HTTP/1.1 200 OK\r\n"
            "Content-Type: %s; charset=utf-8\r\n"
            "Content-Length: %zu\r\n"
            "Connection: close\r\n"
            "\r\n",
            content_type, strlen(data_str));


        http_write(req, header_buf, header_len);
        http_write(req, data_str, strlen(data_str));
    }

    close(req->client_fd);
    req->client_fd = -1;
    return 1;
}

void llm_drop_socket(long s) {
    int* sub_type = (int*)s;
    if (*sub_type == 1) { // HttpServer
        HttpServer* server = (HttpServer*)s;
        if (server->fd >= 0) {
            close(server->fd);
            server->fd = -1;
        }
        if (server->tls_config) {
            llm_drop((long)server->tls_config);
            server->tls_config = NULL;
        }
    } else if (*sub_type == 2) { // HttpRequest
        HttpRequest* req = (HttpRequest*)s;
        if (req->tls_ctx) {
            llm_drop((long)req->tls_ctx);
            req->tls_ctx = NULL;
        }
        if (req->client_fd >= 0) {
            close(req->client_fd);
            req->client_fd = -1;
        }
        if (req->method) {
            llm_drop((long)req->method);
            req->method = NULL;
        }
        if (req->path) {
            llm_drop((long)req->path);
            req->path = NULL;
        }
        if (req->body) {
            llm_drop((long)req->body);
            req->body = NULL;
        }
        if (req->headers) {
            for (int i = 0; i < req->header_count; i++) {
                if (req->headers[i].name) {
                    llm_drop((long)req->headers[i].name);
                }
                if (req->headers[i].value) {
                    llm_drop((long)req->headers[i].value);
                }
            }
            free(req->headers);
            req->headers = NULL;
        }
        req->header_count = 0;
    }
}

long http_serve(long port_ptr) {
    return llm_http_server(0, port_ptr);
}

long http_https_serve(long port_ptr, long cert_ptr, long key_ptr, long legacy) {
    return llm_https_server(port_ptr, cert_ptr, key_ptr, legacy);
}

long http_accept(long server_ptr) {
    return llm_http_server_accept((HttpServer*)server_ptr);
}

long http_respond(long req_ptr, long data_ptr) {
    return llm_http_server_respond((HttpRequest*)req_ptr, (char*)data_ptr);
}

// --- OTEL Thread-Local Context ---
__thread long current_trace_id = 0;
__thread long current_span_id = 0;

typedef struct {
    long trace_id;
    long span_id;
} OtelContextState;

__thread OtelContextState context_stack[256];
__thread int context_stack_depth = 0;

long llm_otel_enter_span() {
    if (context_stack_depth < 256) {
        context_stack[context_stack_depth].trace_id = current_trace_id;
        context_stack[context_stack_depth].span_id = current_span_id;
        context_stack_depth++;
    }
    
    // Generate new trace ID if none exists (using clock nanoseconds for simple uniqueness)
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    long r = (long)ts.tv_sec * 1000000000LL + ts.tv_nsec; 
    
    if (current_trace_id == 0) {
        current_trace_id = r;
    }
    
    // Mix with thread id to ensure uniqueness for span
    current_span_id = r ^ (long)pthread_self();
    
    return current_span_id;
}

void llm_otel_exit_span() {
    if (context_stack_depth > 0) {
        context_stack_depth--;
        current_trace_id = context_stack[context_stack_depth].trace_id;
        current_span_id = context_stack[context_stack_depth].span_id;
    } else {
        current_trace_id = 0;
        current_span_id = 0;
    }
}

long llm_otel_get_context() {
    char buf[128];
    snprintf(buf, sizeof(buf), "%ld:%ld", current_trace_id, current_span_id);
    return (long)llm_rt_strdup(buf);
}

// --- Asynchronous Serialization MPSC Queue ---
typedef struct EmissionTask {
    long type; // 1 = DISCARDED_HTTP, 2 = OTEL_STDOUT, 3 = OTEL_HTTP, 4 = OTEL_FILE
    long arg1;
    long arg2;
    long arg3;
    struct EmissionTask* next;
} EmissionTask;

static pthread_mutex_t emission_mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t emission_cond = PTHREAD_COND_INITIALIZER;
static EmissionTask* emission_queue_head = NULL;
static EmissionTask* emission_queue_tail = NULL;
static int emission_shutdown = 0;
static pthread_t emission_thread_id;
static int emission_thread_started = 0;

static void* emission_flusher_thread(void* arg) {
    while (1) {
        pthread_mutex_lock(&emission_mutex);
        while (!emission_queue_head && !emission_shutdown) {
            pthread_cond_wait(&emission_cond, &emission_mutex);
        }
        
        EmissionTask* task = emission_queue_head;
        if (task) {
            emission_queue_head = task->next;
            if (!emission_queue_head) emission_queue_tail = NULL;
        }
        pthread_mutex_unlock(&emission_mutex);
        
        if (!task && emission_shutdown) break;
        
        if (task) {
            if (task->type == 1 || task->type == 3) {
                // HTTP (Discarded or OTEL HTTP)
                long res = llm_http_client(task->arg1, task->arg2, task->arg3);
                llm_drop(res); 
                
                llm_drop(task->arg1);
                llm_drop(task->arg2);
                if (task->arg3) llm_drop(task->arg3);
            } else if (task->type == 2) {
                // OTEL STDOUT
                char* msg = (char*)task->arg1;
                if (msg) {
                    fprintf(stdout, "%s\n", msg);
                    fflush(stdout);
                    llm_drop((long)msg);
                }
            } else if (task->type == 4) {
                // OTEL FILE
                char* path = (char*)task->arg1;
                char* msg = (char*)task->arg2;
                if (path && msg) {
                    FILE* f = fopen(path, "a");
                    if (f) {
                        fprintf(f, "%s\n", msg);
                        fclose(f);
                    }
                    llm_drop((long)path);
                    llm_drop((long)msg);
                }
            }
            free(task);
        }
    }
    return NULL;
}

long llm_emit_async(long type, long arg1, long arg2, long arg3) {
    EmissionTask* task = malloc(sizeof(EmissionTask));
    task->type = type;
    task->arg1 = arg1;
    task->arg2 = arg2;
    task->arg3 = arg3;
    task->next = NULL;
    
    pthread_mutex_lock(&emission_mutex);
    if (!emission_thread_started) {
        pthread_create(&emission_thread_id, NULL, emission_flusher_thread, NULL);
        emission_thread_started = 1;
    }
    if (emission_queue_tail) {
        emission_queue_tail->next = task;
        emission_queue_tail = task;
    } else {
        emission_queue_head = emission_queue_tail = task;
    }
    pthread_cond_signal(&emission_cond);
    pthread_mutex_unlock(&emission_mutex);
    return 0;
}

void llm_emit_wait_all() {
    pthread_mutex_lock(&emission_mutex);
    if (!emission_thread_started) {
        pthread_mutex_unlock(&emission_mutex);
        return;
    }
    emission_shutdown = 1;
    pthread_cond_signal(&emission_cond);
    pthread_mutex_unlock(&emission_mutex);
    
    pthread_join(emission_thread_id, NULL);
}

long llm_get_time_ns() {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (long)ts.tv_sec * 1000000000LL + ts.tv_nsec;
}

void llm_otel_emit_span(long name_ptr, long start, long end) {
    char* name = (char*)name_ptr;
    char buf[1024];
    snprintf(buf, sizeof(buf), "{\"trace_id\":\"%ld\",\"span_id\":\"%ld\",\"name\":\"%s\",\"start_time\":%ld,\"end_time\":%ld}", 
             current_trace_id, current_span_id, name ? name : "", start, end);
             
    char* payload = llm_rt_strdup(buf);
    
    char* endpoint = getenv("OTEL_EXPORTER_OTLP_ENDPOINT");
    if (endpoint && strlen(endpoint) > 0) {
        llm_emit_async(3, (long)llm_rt_strdup("POST"), (long)llm_rt_strdup(endpoint), (long)payload);
    } else {
        llm_emit_async(2, (long)payload, 0, 0);
    }
    llm_drop(name_ptr);
}
