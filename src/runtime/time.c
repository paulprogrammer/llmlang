#include "common.h"

#define TAI_OFFSET 4611686018427387904LL // 2^62

long llm_tai_now() {
    return TAI_OFFSET + (long)time(NULL) + 37;
}

long llm_tai_get(long tai, long component) {
    long s = tai - TAI_OFFSET;
    if (s < 0) s = 0;

    long second = s % 60; s /= 60;
    long minute = s % 60; s /= 60;
    long hour = s % 24; s /= 24;

    long d = s;
    long y = (10000 * d + 14780) / 3652425;
    long d_rem = d - (365 * y + y / 4 - y / 100 + y / 400);
    if (d_rem < 0) {
        y--;
        d_rem = d - (365 * y + y / 4 - y / 100 + y / 400);
    }
    long mi = (100 * d_rem + 52) / 3060;
    long month = (mi + 2) % 12 + 1;
    y += (mi + 2) / 12;
    long day = d_rem - (306 * mi + 5) / 10 + 1;

    switch(component) {
        case 0: return y + 1970;
        case 1: return month;
        case 2: return day;
        case 3: return hour;
        case 4: return minute;
        case 5: return second;
        default: return 0;
    }
}

long llm_tai_set(long y, long m, long d, long h, long mn, long s) {
    long year = y - 1970;
    if (m <= 2) {
        year--;
        m += 12;
    }
    long days = (long)(365 * year + year / 4 - year / 100 + year / 400 + (306 * (m + 1)) / 10 - 428 + d - 1);
    return TAI_OFFSET + days * 86400 + h * 3600 + mn * 60 + s;
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
