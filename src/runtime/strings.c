#include "common.h"

long llm_len(long s) {
    if (s == 0) return 0;
    return (long)strlen((char*)s);
}

long llm_cat(long l, long r) {
    char* s1 = (char*)l;
    char* s2 = (char*)r;
    size_t len1 = s1 ? strlen(s1) : 0;
    size_t len2 = s2 ? strlen(s2) : 0;
    char* res = malloc(len1 + len2 + 1);
    if (s1) strcpy(res, s1);
    else res[0] = '\0';
    if (s2) strcpy(res + len1, s2);
    res[len1 + len2] = '\0';
    return (long)res;
}

long llm_sub(long s, long start, long len) {
    char* src = (char*)s;
    if (!src) return 0;
    size_t src_len = strlen(src);
    if (start < 0) start = 0;
    if (start >= src_len) return (long)strdup("");
    if (len < 0) len = 0;
    if (start + len > src_len) len = src_len - start;
    char* res = malloc(len + 1);
    strncpy(res, src + start, len);
    res[len] = '\0';
    return (long)res;
}

long llm_loc(long s, long p) {
    char* src = (char*)s;
    char* pat = (char*)p;
    if (!src || !pat) return -1;
    char* found = strstr(src, pat);
    if (!found) return -1;
    return (long)(found - src);
}

long llm_reg(long s, long r) {
    char* src = (char*)s;
    char* re = (char*)r;
    if (!src || !re) return 0;
    regex_t regex;
    int reti = regcomp(&regex, re, REG_EXTENDED);
    if (reti) return 0;
    reti = regexec(&regex, src, 0, NULL, 0);
    regfree(&regex);
    return reti == 0 ? 1 : 0;
}

long llm_itoa(long n) {
    char* buffer = malloc(32);
    sprintf(buffer, "%ld", n);
    return (long)buffer;
}

long llm_strdup(long s) {
    if (s == 0) return 0;
    return (long)strdup((char*)s);
}

long llm_split(long s, long d, long index) {
    char* src = (char*)s;
    char* delim = (char*)d;
    if (!src || !delim) return (long)strdup("");
    char* copy = strdup(src);
    char* token = strtok(copy, delim);
    long current = 0;
    while (token != NULL) {
        if (current == index) {
            char* res = strdup(token);
            free(copy);
            return (long)res;
        }
        token = strtok(NULL, delim);
        current++;
    }
    free(copy);
    return (long)strdup("");
}

long llm_money_format(long val) {
    char buf[64];
    long whole = val / 10000;
    long frac = val % 10000;
    if (frac < 0) frac = -frac;
    sprintf(buf, "%ld.%04ld", whole, frac);
    return (long)strdup(buf);
}
