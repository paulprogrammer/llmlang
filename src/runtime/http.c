#include "common.h"
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

long http_get(long url_ptr) {
    char* url = (char*)url_ptr;
    if (!url) return (long)strdup("");

    CURL* curl = curl_easy_init();
    if (!curl) return (long)strdup("");

    struct ResponseBuffer chunk;
    chunk.data = malloc(1);
    if (!chunk.data) {
        curl_easy_cleanup(curl);
        return (long)strdup("");
    }
    chunk.data[0] = '\0';
    chunk.size = 0;

    curl_easy_setopt(curl, CURLOPT_URL, url);
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, write_callback);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, (void*)&chunk);
    curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 1L);
    curl_easy_setopt(curl, CURLOPT_USERAGENT, "llmlang-http-client/1.0");
    curl_easy_setopt(curl, CURLOPT_TIMEOUT, 30L);

    CURLcode res = curl_easy_perform(curl);
    curl_easy_cleanup(curl);

    if (res != CURLE_OK) {
        free(chunk.data);
        return (long)strdup("");
    }

    return (long)chunk.data;
}

long http_post(long url_ptr, long body_ptr) {
    char* url = (char*)url_ptr;
    char* body = (char*)body_ptr;
    if (!url) return (long)strdup("");

    CURL* curl = curl_easy_init();
    if (!curl) return (long)strdup("");

    struct ResponseBuffer chunk;
    chunk.data = malloc(1);
    if (!chunk.data) {
        curl_easy_cleanup(curl);
        return (long)strdup("");
    }
    chunk.data[0] = '\0';
    chunk.size = 0;

    curl_easy_setopt(curl, CURLOPT_URL, url);
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, write_callback);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, (void*)&chunk);
    curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 1L);
    curl_easy_setopt(curl, CURLOPT_USERAGENT, "llmlang-http-client/1.0");
    curl_easy_setopt(curl, CURLOPT_TIMEOUT, 30L);

    // Set POST options
    curl_easy_setopt(curl, CURLOPT_POST, 1L);
    if (body) {
        curl_easy_setopt(curl, CURLOPT_POSTFIELDS, body);
    } else {
        curl_easy_setopt(curl, CURLOPT_POSTFIELDS, "");
    }

    CURLcode res = curl_easy_perform(curl);
    curl_easy_cleanup(curl);

    if (res != CURLE_OK) {
        free(chunk.data);
        return (long)strdup("");
    }

    return (long)chunk.data;
}
