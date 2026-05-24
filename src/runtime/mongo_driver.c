#include "db.h"
#include <string.h>
#include <stdlib.h>

typedef struct {
    char* conn_str;
    char* err_msg;
} MongoConn;

static long mongo_connect_impl(const char* conn_str) {
    MongoConn* conn = malloc(sizeof(MongoConn));
    if (!conn) return 0;
    conn->conn_str = strdup(conn_str);
    conn->err_msg = NULL;
    return (long)conn;
}

static long mongo_query_impl(long conn_val, const char* sql, const char* fields_csv, long params_soa) {
    MongoConn* conn = (MongoConn*)conn_val;
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
        if (strcmp(field_names[c], "id") == 0) {
            col[0] = (long)llm_rt_strdup("mock_id_123");
        } else {
            col[0] = (long)llm_rt_strdup("mongo_mock");
        }
    }
    
    free(fields);
    return (long)instance;
}

static long mongo_exec_impl(long conn_val, const char* sql, long params_soa) {
    MongoConn* conn = (MongoConn*)conn_val;
    if (!conn) return -1;
    return 1; // 1 document modified
}

static void mongo_close_impl(long conn_val) {
    MongoConn* conn = (MongoConn*)conn_val;
    if (!conn) return;
    if (conn->conn_str) free(conn->conn_str);
    if (conn->err_msg) free(conn->err_msg);
    free(conn);
}

static const char* mongo_error_impl(long conn_val) {
    MongoConn* conn = (MongoConn*)conn_val;
    if (!conn) return "Invalid connection";
    return conn->err_msg ? conn->err_msg : "";
}

static DatabaseDriver mongo_driver = {
    .name = "mongodb",
    .connect = mongo_connect_impl,
    .query = mongo_query_impl,
    .exec = mongo_exec_impl,
    .close = mongo_close_impl,
    .get_error = mongo_error_impl,
    .next = NULL
};

__attribute__((constructor))
static void register_mongo() {
    db_register_driver(&mongo_driver);
}
