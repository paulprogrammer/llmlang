#define _GNU_SOURCE
#include "common.h"
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <fcntl.h>
#include <curl/curl.h>

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
    return (long)server;
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

long llm_http_server_accept(HttpServer* server) {
    struct sockaddr_in client_addr;
    socklen_t client_len = sizeof(client_addr);
    int client_fd = accept(server->fd, (struct sockaddr*)&client_addr, &client_len);
    if (client_fd < 0) {
        return 0;
    }

    size_t buf_size = 4096;
    char* buf = malloc(buf_size);
    size_t total_read = 0;
    while (1) {
        ssize_t bytes = read(client_fd, buf + total_read, buf_size - total_read - 1);
        if (bytes <= 0) {
            break;
        }
        total_read += bytes;
        buf[total_read] = '\0';

        char* header_end = strstr(buf, "\r\n\r\n");
        if (header_end) {
            char* content_length_ptr = strcasestr(buf, "Content-Length:");
            if (content_length_ptr) {
                int content_len = atoi(content_length_ptr + 15);
                size_t header_len = (header_end + 4) - buf;
                if (total_read >= header_len + content_len) {
                    break;
                }
            } else {
                break;
            }
        }

        if (total_read >= buf_size - 1) {
            buf_size *= 2;
            buf = realloc(buf, buf_size);
        }
    }

    char* first_line_end = strchr(buf, '\r');
    if (!first_line_end) first_line_end = strchr(buf, '\n');

    char* method_str = "";
    char* path_str = "";
    char* body_str = "";

    if (first_line_end) {
        size_t first_line_len = first_line_end - buf;
        char* first_line = malloc(first_line_len + 1);
        memcpy(first_line, buf, first_line_len);
        first_line[first_line_len] = '\0';

        char* p1 = strchr(first_line, ' ');
        if (p1) {
            *p1 = '\0';
            method_str = first_line;
            char* path_start = p1 + 1;
            char* p2 = strchr(path_start, ' ');
            if (p2) {
                *p2 = '\0';
            }
            path_str = path_start;
        }
        
        char* double_crlf = strstr(buf, "\r\n\r\n");
        if (double_crlf) {
            body_str = double_crlf + 4;
        } else {
            char* double_lf = strstr(buf, "\n\n");
            if (double_lf) {
                body_str = double_lf + 2;
            }
        }

        HttpRequest* req = (HttpRequest*)llm_rt_alloc(sizeof(HttpRequest), RT_TYPE_SOCKET);
        req->type = 2;
        req->client_fd = client_fd;
        req->method = llm_rt_strdup(method_str);
        req->path = llm_rt_strdup(path_str);
        req->body = llm_rt_strdup(body_str);

        free(first_line);
        free(buf);
        return (long)req;
    }

    free(buf);
    close(client_fd);
    return 0;
}

long llm_http_server_respond(HttpRequest* req, char* data_str) {
    if (!req || req->client_fd < 0) return 0;

    if (!data_str) data_str = "";

    if (strncmp(data_str, "HTTP/1.", 7) == 0) {
        write(req->client_fd, data_str, strlen(data_str));
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

        write(req->client_fd, header_buf, header_len);
        write(req->client_fd, data_str, strlen(data_str));
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
    } else if (*sub_type == 2) { // HttpRequest
        HttpRequest* req = (HttpRequest*)s;
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

