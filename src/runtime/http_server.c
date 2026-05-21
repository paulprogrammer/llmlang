#define _GNU_SOURCE
#include "common.h"
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <fcntl.h>
#include <curl/curl.h>
#include <poll.h>
#include "picohttpparser.h"

struct ResponseBuffer {
    char* data;
    size_t size;
};

static size_t write_callback(void* contents, size_t size, size_t nmemb, void* userp) {
    size_t realsize = size * nmemb;
    struct ResponseBuffer* mem = (struct ResponseBuffer*)userp;
    char* ptr = realloc(mem->data, mem->size + realsize + 1);
    if (!ptr) {
        return 0; // out of memory
    }
    mem->data = ptr;
    memcpy(&(mem->data[mem->size]), contents, realsize);
    mem->size += realsize;
    mem->data[mem->size] = 0;
    return realsize;
}

long llm_http_client(long method_ptr, long url_ptr, long body_ptr) {
    char* method = (char*)method_ptr;
    char* url = (char*)url_ptr;
    char* body = (char*)body_ptr;
    if (!url) return (long)llm_rt_strdup("");
    if (!method) method = "GET";

    CURL* curl = curl_easy_init();
    if (!curl) return (long)llm_rt_strdup("");

    struct ResponseBuffer chunk;
    chunk.data = malloc(1);
    if (!chunk.data) {
        curl_easy_cleanup(curl);
        return (long)llm_rt_strdup("");
    }
    chunk.data[0] = '\0';
    chunk.size = 0;

    curl_easy_setopt(curl, CURLOPT_URL, url);
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, write_callback);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, (void*)&chunk);
    curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 1L);
    curl_easy_setopt(curl, CURLOPT_USERAGENT, "llmlang-http-client/1.0");
    curl_easy_setopt(curl, CURLOPT_TIMEOUT, 30L);

    if (strcasecmp(method, "POST") == 0) {
        curl_easy_setopt(curl, CURLOPT_POST, 1L);
        if (body) {
            curl_easy_setopt(curl, CURLOPT_POSTFIELDS, body);
        } else {
            curl_easy_setopt(curl, CURLOPT_POSTFIELDS, "");
        }
    } else if (strcasecmp(method, "GET") != 0) {
        curl_easy_setopt(curl, CURLOPT_CUSTOMREQUEST, method);
        if (body) {
            curl_easy_setopt(curl, CURLOPT_POSTFIELDS, body);
        }
    }

    CURLcode res = curl_easy_perform(curl);
    curl_easy_cleanup(curl);

    if (res != CURLE_OK) {
        free(chunk.data);
        return (long)llm_rt_strdup("");
    }

    long managed_res = (long)llm_rt_strdup(chunk.data);
    free(chunk.data);
    return managed_res;
}

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
    size_t total_read = 0;
    
    HttpRequest* req = (HttpRequest*)llm_rt_alloc(sizeof(HttpRequest), RT_TYPE_SOCKET);
    req->type = 2;
    req->client_fd = client_fd;
    req->method = NULL;
    req->path = NULL;
    req->body = NULL;
    req->tls_ctx = NULL;

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
        if (bytes <= 0) {
            break;
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
                if (total_read < header_len + content_len) {
                    if (header_len + content_len >= buf_size - 1) {
                        buf_size = header_len + content_len + 1024;
                        buf = realloc(buf, buf_size);
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
            buf_size *= 2;
            buf = realloc(buf, buf_size);
        }
    }

    free(buf);
    llm_drop((long)req);
    return 0;
}

static void http_write(HttpRequest* req, const char* data, size_t len) {
    if (req->tls_ctx) {
        llm_tls_write(req->tls_ctx, (const unsigned char*)data, len);
    } else {
        write(req->client_fd, data, len);
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
    llm_drop((long)req);
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

