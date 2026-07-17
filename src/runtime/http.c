#include "common.h"
#include <curl/curl.h>

// curl_global_init is not thread-safe and must run exactly once before the
// first curl_easy_init anywhere in the process (requests may start
// concurrently on pool threads).
static pthread_once_t curl_init_once = PTHREAD_ONCE_INIT;
static void curl_do_global_init(void) {
    curl_global_init(CURL_GLOBAL_DEFAULT);
}
void llm_curl_ensure_init(void) {
    pthread_once(&curl_init_once, curl_do_global_init);
}

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

// The one curl request routine: http_get, http_post, and llm_http_client
// are thin wrappers, so behavior changes (timeouts, redirects, init) happen
// in exactly one place.
static long curl_request(const char* method, const char* url, const char* body) {
    if (!url) return (long)llm_rt_strdup("");
    if (!method) method = "GET";

    llm_curl_ensure_init();
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
        curl_easy_setopt(curl, CURLOPT_POSTFIELDS, body ? body : "");
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

long http_get(long url_ptr) {
    return curl_request("GET", (const char*)url_ptr, NULL);
}

long http_post(long url_ptr, long body_ptr) {
    return curl_request("POST", (const char*)url_ptr, (const char*)body_ptr);
}

long llm_http_client(long method_ptr, long url_ptr, long body_ptr) {
    return curl_request((const char*)method_ptr, (const char*)url_ptr, (const char*)body_ptr);
}

static int hex_val(char c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return 10 + c - 'a';
    if (c >= 'A' && c <= 'F') return 10 + c - 'A';
    return -1;
}

long http_decode(long s) {
    char* src = (char*)s;
    if (!src) return (long)llm_rt_strdup("");
    size_t len = strlen(src);
    char* dest = llm_rt_alloc(len + 1, RT_TYPE_STRING);
    size_t j = 0;
    for (size_t i = 0; i < len; i++) {
        if (src[i] == '+') {
            dest[j++] = ' ';
        } else if (src[i] == '%' && i + 2 < len) {
            int h1 = hex_val(src[i+1]);
            int h2 = hex_val(src[i+2]);
            if (h1 >= 0 && h2 >= 0) {
                dest[j++] = (char)((h1 << 4) | h2);
                i += 2;
            } else {
                dest[j++] = '%';
            }
        } else {
            dest[j++] = src[i];
        }
    }
    dest[j] = '\0';
    return (long)dest;
}

long llm_http_get_header(long req_ptr, long name_ptr) {
    HttpRequest* req = (HttpRequest*)req_ptr;
    char* name = (char*)name_ptr;
    if (!req || req->type != 2 || !name) {
        return (long)llm_rt_strdup("");
    }
    for (int i = 0; i < req->header_count; i++) {
        if (strcasecmp(req->headers[i].name, name) == 0) {
            return llm_dup((long)req->headers[i].value);
        }
    }
    return (long)llm_rt_strdup("");
}
