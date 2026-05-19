#include "common.h"

// Simplified JSON for SoA: {"x": [1, 2], "y": [3, 4]}

long llm_pack(long* instance, const char* fields_csv) {
    if (!instance || !fields_csv) return (long)strdup("{}");

    long count = instance[0];
    char* fields = strdup(fields_csv);
    char* field_names[32];
    int field_count = 0;
    char* token = strtok(fields, ",");
    while (token && field_count < 32) {
        field_names[field_count++] = token;
        token = strtok(NULL, ",");
    }

    // Rough estimate for buffer size: 32 chars per number, 32 per field name, etc.
    size_t buf_size = field_count * count * 32 + 1024;
    char* res = malloc(buf_size);
    char* p = res;

    p += sprintf(p, "{");
    for (int i = 0; i < field_count; i++) {
        p += sprintf(p, "\"%s\": [", field_names[i]);
        long* col = (long*)instance[i + 1]; // +1 because index 0 is count
        for (long j = 0; j < count; j++) {
            p += sprintf(p, "%ld%s", col[j], (j == count - 1) ? "" : ", ");
        }
        p += sprintf(p, "]%s", (i == field_count - 1) ? "" : ", ");
    }
    sprintf(p, "}");

    free(fields);
    return (long)res;
}

long llm_unpack(const char* json, const char* fields_csv) {
    if (!json || !fields_csv) return 0;

    char* fields = strdup(fields_csv);
    char* field_names[32];
    int field_count = 0;
    char* token = strtok(fields, ",");
    while (token && field_count < 32) {
        field_names[field_count++] = token;
        token = strtok(NULL, ",");
    }

    // 1. Find count by counting commas in the first array
    char* first_bracket = strchr(json, '[');
    if (!first_bracket) { free(fields); return 0; }
    long count = 0;
    const char* p = first_bracket + 1;
    while (*p && *p != ']') {
        if (*p == ',') count++;
        p++;
    }
    if (p > first_bracket + 1) count++; // One more than commas

    // 2. Allocate SoA struct and columns (member 0 is count)
    long* instance = malloc((field_count + 1) * sizeof(long));
    instance[0] = count;
    for (int i = 0; i < field_count; i++) {
        instance[i + 1] = (long)malloc(count * sizeof(long));
    }

    // 3. Parse fields (very naive)
    for (int i = 0; i < field_count; i++) {
        char search[64];
        sprintf(search, "\"%s\": [", field_names[i]);
        char* start = strstr(json, search);
        if (start) {
            start += strlen(search);
            long* col = (long*)instance[i + 1];
            for (long j = 0; j < count; j++) {
                col[j] = strtol(start, &start, 10);
                while (*start == ',' || *start == ' ') start++;
            }
        }
    }

    free(fields);
    return (long)instance;
}
