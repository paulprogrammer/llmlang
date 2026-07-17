#include "db.h"
#include <sqlite3.h>
#include <string.h>
#include <stdlib.h>

typedef struct {
    sqlite3* db;
    char* err_msg;
} SqliteConn;

static int is_string_cell(long val) {
    if (val <= RT_MIN_HANDLE) return 0;
    LlmRtHeader* header = (LlmRtHeader*)(val - sizeof(LlmRtHeader));
    return (header->type == RT_TYPE_STRING && (header->magic == RT_MAGIC || header->magic == 0));
}

static void bind_stmt_params(sqlite3_stmt* stmt, long params_soa) {
    if (!params_soa) return;
    long* instance = (long*)params_soa;
    long count = instance[0];
    if (count <= 0) return;
    
    int param_count = sqlite3_bind_parameter_count(stmt);
    for (int i = 0; i < param_count; i++) {
        long* col = (long*)instance[i + 1];
        long val = col[0];
        if (is_string_cell(val)) {
            sqlite3_bind_text(stmt, i + 1, (char*)val, -1, SQLITE_TRANSIENT);
        } else {
            sqlite3_bind_int64(stmt, i + 1, (sqlite3_int64)val);
        }
    }
}

static long sqlite_connect_impl(const char* conn_str) {
    sqlite3* db = NULL;
    // Skip protocol prefix if present, e.g. "sqlite://test.db" -> "test.db"
    const char* path = conn_str;
    if (strncmp(conn_str, "sqlite://", 9) == 0) {
        path = conn_str + 9;
    }
    
    int rc = sqlite3_open_v2(path, &db, SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE | SQLITE_OPEN_FULLMUTEX, NULL);
    if (rc != SQLITE_OK) {
        if (db) sqlite3_close(db);
        return 0;
    }
    
    SqliteConn* conn = malloc(sizeof(SqliteConn));
    conn->db = db;
    conn->err_msg = NULL;
    return (long)conn;
}

static long sqlite_query_impl(long conn_val, const char* sql, const char* fields_csv, long params_soa) {
    SqliteConn* conn = (SqliteConn*)conn_val;
    if (!conn) return 0;
    
    if (conn->err_msg) {
        sqlite3_free(conn->err_msg);
        conn->err_msg = NULL;
    }
    
    sqlite3_stmt* stmt = NULL;
    int rc = sqlite3_prepare_v2(conn->db, sql, -1, &stmt, NULL);
    if (rc != SQLITE_OK) {
        conn->err_msg = strdup(sqlite3_errmsg(conn->db));
        return 0;
    }
    
    bind_stmt_params(stmt, params_soa);
    
    // Parse the fields
    char* fields = strdup(fields_csv);
    char* field_names[32];
    int field_count = 0;
    char* saveptr = NULL;
    char* token = strtok_r(fields, ",", &saveptr);
    while (token && field_count < 32) {
        field_names[field_count++] = token;
        token = strtok_r(NULL, ",", &saveptr);
    }
    
    // Step through the rows and buffer data
    long capacity = 16;
    long count = 0;
    long** buffered_rows = malloc(capacity * sizeof(long*));
    
    while ((rc = sqlite3_step(stmt)) == SQLITE_ROW) {
        if (count >= capacity) {
            capacity *= 2;
            buffered_rows = realloc(buffered_rows, capacity * sizeof(long*));
        }
        
        long* row_data = malloc(field_count * sizeof(long));
        for (int c = 0; c < field_count; c++) {
            int col_type = sqlite3_column_type(stmt, c);
            if (col_type == SQLITE_INTEGER) {
                row_data[c] = (long)sqlite3_column_int64(stmt, c);
            } else if (col_type == SQLITE_FLOAT) {
                row_data[c] = (long)sqlite3_column_double(stmt, c);
            } else if (col_type == SQLITE_TEXT) {
                const unsigned char* text = sqlite3_column_text(stmt, c);
                row_data[c] = (long)llm_rt_strdup(text ? (char*)text : "");
            } else if (col_type == SQLITE_NULL) {
                row_data[c] = 0;
            } else {
                const unsigned char* text = sqlite3_column_text(stmt, c);
                row_data[c] = (long)llm_rt_strdup(text ? (char*)text : "");
            }
        }
        buffered_rows[count++] = row_data;
    }
    
    sqlite3_finalize(stmt);
    free(fields);
    
    if (rc != SQLITE_DONE && rc != SQLITE_OK && rc != SQLITE_ROW) {
        conn->err_msg = strdup(sqlite3_errmsg(conn->db));
        for (long i = 0; i < count; i++) {
            // Free any allocated strings to avoid leaks on error
            for (int c = 0; c < field_count; c++) {
                if (is_string_cell(buffered_rows[i][c])) {
                    llm_drop(buffered_rows[i][c]);
                }
            }
            free(buffered_rows[i]);
        }
        free(buffered_rows);
        return 0;
    }
    
    if (count == 0) {
        free(buffered_rows);
        long* instance = malloc((field_count + 1) * sizeof(long));
        instance[0] = 0;
        for (int i = 0; i < field_count; i++) {
            instance[i + 1] = (long)malloc(0);
        }
        return (long)instance;
    }
    
    // Allocate the Struct-of-Arrays (SoA) layout
    long* instance = malloc((field_count + 1) * sizeof(long));
    instance[0] = count;
    for (int c = 0; c < field_count; c++) {
        instance[c + 1] = (long)malloc(count * sizeof(long));
        long* col = (long*)instance[c + 1];
        for (long r = 0; r < count; r++) {
            col[r] = buffered_rows[r][c];
        }
    }
    
    // Free the row buffers (SoA now owns the string pointers)
    for (long r = 0; r < count; r++) {
        free(buffered_rows[r]);
    }
    free(buffered_rows);
    
    return (long)instance;
}

static long sqlite_exec_impl(long conn_val, const char* sql, long params_soa) {
    SqliteConn* conn = (SqliteConn*)conn_val;
    if (!conn) return -1;
    
    if (conn->err_msg) {
        sqlite3_free(conn->err_msg);
        conn->err_msg = NULL;
    }
    
    sqlite3_stmt* stmt = NULL;
    int rc = sqlite3_prepare_v2(conn->db, sql, -1, &stmt, NULL);
    if (rc != SQLITE_OK) {
        conn->err_msg = strdup(sqlite3_errmsg(conn->db));
        return -1;
    }
    
    bind_stmt_params(stmt, params_soa);
    
    rc = sqlite3_step(stmt);
    sqlite3_finalize(stmt);
    
    if (rc != SQLITE_DONE && rc != SQLITE_OK && rc != SQLITE_ROW) {
        conn->err_msg = strdup(sqlite3_errmsg(conn->db));
        return -1;
    }
    
    return (long)sqlite3_changes(conn->db);
}

static void sqlite_close_impl(long conn_val) {
    SqliteConn* conn = (SqliteConn*)conn_val;
    if (!conn) return;
    if (conn->db) {
        sqlite3_close_v2(conn->db);
    }
    if (conn->err_msg) {
        free(conn->err_msg);
    }
    free(conn);
}

static const char* sqlite_error_impl(long conn_val) {
    SqliteConn* conn = (SqliteConn*)conn_val;
    if (!conn) return "Invalid connection";
    return conn->err_msg ? conn->err_msg : "";
}

static DatabaseDriver sqlite_driver = {
    .name = "sqlite",
    .connect = sqlite_connect_impl,
    .query = sqlite_query_impl,
    .exec = sqlite_exec_impl,
    .close = sqlite_close_impl,
    .get_error = sqlite_error_impl,
    .next = NULL
};

__attribute__((constructor))
static void register_sqlite() {
    db_register_driver(&sqlite_driver);
}
