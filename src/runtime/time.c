#include "common.h"

#define TAI_OFFSET 4611686018427387914LL

long llm_tai_now() {
    return TAI_OFFSET + (long)time(NULL);
}

// Correct Gregorian calendar math
long llm_tai_get(long tai, long component) {
    long s = tai - TAI_OFFSET;
    struct tm tm;
    time_t t = (time_t)s;
    gmtime_r(&t, &tm);

    switch(component) {
        case 0: return (long)tm.tm_year + 1900;
        case 1: return (long)tm.tm_mon + 1;
        case 2: return (long)tm.tm_mday;
        case 3: return (long)tm.tm_hour;
        case 4: return (long)tm.tm_min;
        case 5: return (long)tm.tm_sec;
        default: return 0;
    }
}

long llm_tai_set(long y, long m, long d, long h, long mn, long s) {
    struct tm tm = {0};
    tm.tm_year = (int)y - 1900;
    tm.tm_mon = (int)m - 1;
    tm.tm_mday = (int)d;
    tm.tm_hour = (int)h;
    tm.tm_min = (int)mn;
    tm.tm_sec = (int)s;
    return TAI_OFFSET + (long)timegm(&tm);
}

long llm_timezone() {
    char* tz = getenv("TZ");
    if (tz) return (long)strdup(tz);

    FILE* f = fopen("/etc/timezone", "r");
    if (f) {
        char buf[128];
        if (fgets(buf, sizeof(buf), f)) {
            buf[strcspn(buf, "\n")] = 0;
            fclose(f);
            return (long)strdup(buf);
        }
        fclose(f);
    }

    char link[256];
    ssize_t len = readlink("/etc/localtime", link, sizeof(link)-1);
    if (len != -1) {
        link[len] = '\0';
        char* p = strstr(link, "zoneinfo/");
        if (p) return (long)strdup(p + 9);
    }

    return (long)strdup("UTC");
}
