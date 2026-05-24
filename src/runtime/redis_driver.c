#include "db.h"
#include <string.h>
#include <stdlib.h>

typedef struct {
    char* conn_str;
    char* err_msg;
} RedisConn;

static long redis_connect_impl(const char* conn_str) {
    RedisConn* conn = malloc(sizeof(RedisConn));
    if (!conn) return 0;
    conn->conn_str = strdup(conn_str);
    conn->err_msg = NULL;
    return (long)conn;
}

static long redis_query_impl(long conn_val, const char* sql, const char* fields_csv, long params_soa) {
    RedisConn* conn = (RedisConn*)conn_val;
    if (!conn) return 0;
    
    // Parse fields
    char* fields = strdup(fields_csv);
    char* field_names[32];
    int field_count = 0;
    char* saveptr = NULL;
    char* token = strtok_r(fields, ",", &saveptr);
    while (token && field_count < 32) {
        field_names[field_count++] = token;
        token = strtok_r(NULL, ",", &saveptr);
    }
    
    // Let's create a single mock row
    long count = 1;
    long* instance = malloc((field_count + 1) * sizeof(long));
    instance[0] = count;
    
    for (int c = 0; c < field_count; c++) {
        instance[c + 1] = (long)malloc(count * sizeof(long));
        long* col = (long*)instance[c + 1];
        if (strcmp(field_names[c], "key") == 0) {
            col[0] = (long)llm_rt_strdup("mock_key");
        } else if (strcmp(field_names[c], "value") == 0) {
            col[0] = (long)llm_rt_strdup("mock_value");
        } else {
            col[0] = (long)llm_rt_strdup("redis_mock");
        }
    }
    
    free(fields);
    return (long)instance;
}

static long redis_exec_impl(long conn_val, const char* sql, long params_soa) {
    RedisConn* conn = (RedisConn*)conn_val;
    if (!conn) return -1;
    return 1; // 1 command executed successfully
}

static void redis_close_impl(long conn_val) {
    RedisConn* conn = (RedisConn*)conn_val;
    if (!conn) return;
    if (conn->conn_str) free(conn->conn_str);
    if (conn->err_msg) free(conn->err_msg);
    free(conn);
}

static const char* redis_error_impl(long conn_val) {
    RedisConn* conn = (RedisConn*)conn_val;
    if (!conn) return "Invalid connection";
    return conn->err_msg ? conn->err_msg : "";
}

static DatabaseDriver redis_driver = {
    .name = "redis",
    .connect = redis_connect_impl,
    .query = redis_query_impl,
    .exec = redis_exec_impl,
    .close = redis_close_impl,
    .get_error = redis_error_impl,
    .next = NULL
};

__attribute__((constructor))
static void register_redis() {
    db_register_driver(&redis_driver);
}
