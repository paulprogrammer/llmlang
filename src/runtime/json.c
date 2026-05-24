#include "common.h"
#include "cJSON.h"

#define MAX_ACTIVE_ROOTS 256
static __thread void* active_json_roots[MAX_ACTIVE_ROOTS];
static __thread int active_json_roots_count = 0;

void register_json_root(void* cell) {
    if (active_json_roots_count < MAX_ACTIVE_ROOTS) {
        active_json_roots[active_json_roots_count++] = cell;
    }
}

void unregister_json_root(void* cell) {
    for (int i = 0; i < active_json_roots_count; i++) {
        if (active_json_roots[i] == cell) {
            active_json_roots[i] = active_json_roots[--active_json_roots_count];
            return;
        }
    }
}

void llm_drop_json(long s) {
    void** cell = (void**)s;
    unregister_json_root(cell);
    if (*cell) {
        cJSON_Delete(*cell);
    }
}

static int is_json_root(long handle) {
    for (int i = 0; i < active_json_roots_count; i++) {
        if ((long)active_json_roots[i] == handle) {
            return 1;
        }
    }
    return 0;
}

static cJSON* get_node(long handle) {
    if (handle <= 1000) return NULL;
    if (is_json_root(handle)) {
        return *(cJSON**)handle;
    }
    return (cJSON*)handle;
}

long llm_pack(long* instance, const char* fields_csv) {
    if (!instance || !fields_csv) return (long)llm_rt_strdup("{}");

    long count = instance[0];
    char* fields = strdup(fields_csv);
    char* field_names[32];
    int field_count = 0;
    char* saveptr = NULL;
    char* token = strtok_r(fields, ",", &saveptr);
    while (token && field_count < 32) {
        field_names[field_count++] = token;
        token = strtok_r(NULL, ",", &saveptr);
    }

    cJSON* root = cJSON_CreateObject();
    for (int i = 0; i < field_count; i++) {
        cJSON* arr = cJSON_CreateArray();
        long* col = (long*)instance[i + 1];
        for (long j = 0; j < count; j++) {
            cJSON_AddItemToArray(arr, cJSON_CreateNumber((double)col[j]));
        }
        cJSON_AddItemToObject(root, field_names[i], arr);
    }

    char* s = cJSON_PrintUnformatted(root);
    long res = (long)llm_rt_strdup(s);
    free(s);
    cJSON_Delete(root);
    free(fields);
    return res;
}

long llm_unpack(const char* json, const char* fields_csv) {
    if (!json || !fields_csv) return 0;

    cJSON* root = cJSON_Parse(json);
    if (!root) return 0;

    char* fields = strdup(fields_csv);
    char* field_names[32];
    int field_count = 0;
    char* saveptr = NULL;
    char* token = strtok_r(fields, ",", &saveptr);
    while (token && field_count < 32) {
        field_names[field_count++] = token;
        token = strtok_r(NULL, ",", &saveptr);
    }

    long count = 0;
    if (field_count > 0) {
        cJSON* arr = cJSON_GetObjectItemCaseSensitive(root, field_names[0]);
        if (cJSON_IsArray(arr)) {
            count = cJSON_GetArraySize(arr);
        }
    }

    if (count <= 0) {
        cJSON_Delete(root);
        free(fields);
        return 0;
    }

    long* instance = malloc((field_count + 1) * sizeof(long));
    instance[0] = count;
    for (int i = 0; i < field_count; i++) {
        instance[i + 1] = (long)malloc(count * sizeof(long));
        cJSON* arr = cJSON_GetObjectItemCaseSensitive(root, field_names[i]);
        long* col = (long*)instance[i + 1];
        if (cJSON_IsArray(arr)) {
            for (long j = 0; j < count; j++) {
                cJSON* item = cJSON_GetArrayItem(arr, (int)j);
                col[j] = cJSON_IsNumber(item) ? (long)item->valuedouble : 0;
            }
        } else {
            for (long j = 0; j < count; j++) {
                col[j] = 0;
            }
        }
    }

    cJSON_Delete(root);
    free(fields);
    return (long)instance;
}

long json_parse(long str_ptr) {
    char* json_str = (char*)str_ptr;
    if (!json_str) return 0;
    cJSON* root = cJSON_Parse(json_str);
    if (!root) return 0;
    cJSON** cell = llm_rt_alloc(sizeof(cJSON*), RT_TYPE_JSON);
    *cell = root;
    register_json_root(cell);
    return (long)cell;
}

long json_stringify(long handle) {
    cJSON* node = get_node(handle);
    if (!node) return (long)llm_rt_strdup("");
    char* s = cJSON_PrintUnformatted(node);
    long res = (long)llm_rt_strdup(s);
    free(s);
    return res;
}

long json_get_int(long handle, long key_ptr) {
    cJSON* node = get_node(handle);
    if (!node || !key_ptr) return 0;
    cJSON* item = cJSON_GetObjectItemCaseSensitive(node, (char*)key_ptr);
    if (cJSON_IsNumber(item)) {
        return (long)item->valuedouble;
    }
    return 0;
}

long json_get_float(long handle, long key_ptr) {
    return json_get_int(handle, key_ptr);
}

long json_get_str(long handle, long key_ptr) {
    cJSON* node = get_node(handle);
    if (!node || !key_ptr) return (long)llm_rt_strdup("");
    cJSON* item = cJSON_GetObjectItemCaseSensitive(node, (char*)key_ptr);
    if (cJSON_IsString(item) && item->valuestring) {
        return (long)llm_rt_strdup(item->valuestring);
    }
    return (long)llm_rt_strdup("");
}

long json_get_obj(long handle, long key_ptr) {
    cJSON* node = get_node(handle);
    if (!node || !key_ptr) return 0;
    cJSON* item = cJSON_GetObjectItemCaseSensitive(node, (char*)key_ptr);
    if (cJSON_IsObject(item)) {
        return (long)item;
    }
    return 0;
}

long json_get_arr(long handle, long key_ptr) {
    cJSON* node = get_node(handle);
    if (!node || !key_ptr) return 0;
    cJSON* item = cJSON_GetObjectItemCaseSensitive(node, (char*)key_ptr);
    if (cJSON_IsArray(item)) {
        return (long)item;
    }
    return 0;
}

long json_arr_len(long handle) {
    cJSON* node = get_node(handle);
    if (!node || !cJSON_IsArray(node)) return 0;
    return (long)cJSON_GetArraySize(node);
}

long json_arr_get(long handle, long index) {
    cJSON* node = get_node(handle);
    if (!node || !cJSON_IsArray(node)) return 0;
    cJSON* item = cJSON_GetArrayItem(node, (int)index);
    return (long)item;
}
