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
    char* res = llm_rt_alloc(len1 + len2 + 1, RT_TYPE_STRING);
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
    if (start >= src_len) return (long)llm_rt_strdup("");
    if (len < 0) len = 0;
    if (start + len > src_len) len = src_len - start;
    char* res = llm_rt_alloc(len + 1, RT_TYPE_STRING);
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
    char* buffer = llm_rt_alloc(32, RT_TYPE_STRING);
    sprintf(buffer, "%ld", n);
    return (long)buffer;
}

long llm_strdup(long s) {
    if (s == 0) return 0;
    return (long)llm_rt_strdup((char*)s);
}

// Splits on `delim` as a literal substring (not a character set — strtok_r's
// set semantics made `sp s ", " i` behave unexpectedly, e.g. treating ' '
// and ',' as interchangeable separators instead of the two-char sequence).
long llm_split(long s, long d, long index) {
    char* src = (char*)s;
    char* delim = (char*)d;
    if (!src || !delim || !*delim) return (long)llm_rt_strdup("");

    size_t delim_len = strlen(delim);
    const char* cursor = src;
    long current = 0;
    while (1) {
        const char* found = strstr(cursor, delim);
        size_t tok_len = found ? (size_t)(found - cursor) : strlen(cursor);
        if (current == index) {
            char* res = llm_rt_alloc(tok_len + 1, RT_TYPE_STRING);
            memcpy(res, cursor, tok_len);
            res[tok_len] = '\0';
            return (long)res;
        }
        if (!found) break;
        cursor = found + delim_len;
        current++;
    }
    return (long)llm_rt_strdup("");
}

long llm_money_format(long val) {
    char buf[64];
    long whole = val / LLM_MONEY_SCALE;
    long frac = val % LLM_MONEY_SCALE;
    if (frac < 0) frac = -frac;
    sprintf(buf, "%ld.%04ld", whole, frac);
    return (long)llm_rt_strdup(buf);
}
