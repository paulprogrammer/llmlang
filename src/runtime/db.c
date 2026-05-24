#include "db.h"
#include <string.h>

typedef struct {
    int type; // 4 = RT_TYPE_DB
    DatabaseDriver* driver;
    long conn; // Underlying driver connection representation
    char* last_error;
} LlmDbConn;

static DatabaseDriver* g_drivers = NULL;

void db_register_driver(DatabaseDriver* driver) {
    driver->next = g_drivers;
    g_drivers = driver;
}

DatabaseDriver* db_find_driver(const char* name) {
    DatabaseDriver* curr = g_drivers;
    while (curr) {
        if (strcmp(curr->name, name) == 0) {
            return curr;
        }
        curr = curr->next;
    }
    return NULL;
}

void llm_drop_db(long s) {
    LlmDbConn* conn = (LlmDbConn*)s;
    if (!conn) return;
    if (conn->driver && conn->driver->close) {
        conn->driver->close(conn->conn);
    }
    if (conn->last_error) {
        free(conn->last_error);
    }
}

long llm_db_connect(long driver_name_ptr, long conn_str_ptr) {
    char* driver_name = (char*)driver_name_ptr;
    char* conn_str = (char*)conn_str_ptr;
    if (!driver_name || !conn_str) return 0;
    
    DatabaseDriver* driver = db_find_driver(driver_name);
    if (!driver) return 0;
    
    long raw_conn = driver->connect(conn_str);
    if (!raw_conn) return 0;
    
    LlmDbConn* conn = (LlmDbConn*)llm_rt_alloc(sizeof(LlmDbConn), RT_TYPE_DB);
    conn->type = 4;
    conn->driver = driver;
    conn->conn = raw_conn;
    conn->last_error = NULL;
    return (long)conn;
}

long llm_db_query(long conn_ptr, long sql_ptr, long fields_csv_ptr, long params_soa) {
    LlmDbConn* conn = (LlmDbConn*)conn_ptr;
    char* sql = (char*)sql_ptr;
    char* fields_csv = (char*)fields_csv_ptr;
    if (!conn || conn->type != 4 || !sql || !fields_csv) return 0;
    
    long res = conn->driver->query(conn->conn, sql, fields_csv, params_soa);
    if (!res) {
        const char* err = conn->driver->get_error(conn->conn);
        if (conn->last_error) free(conn->last_error);
        conn->last_error = err ? strdup(err) : NULL;
    }
    return res;
}

long llm_db_exec(long conn_ptr, long sql_ptr, long params_soa) {
    LlmDbConn* conn = (LlmDbConn*)conn_ptr;
    char* sql = (char*)sql_ptr;
    if (!conn || conn->type != 4 || !sql) return -1;
    
    long res = conn->driver->exec(conn->conn, sql, params_soa);
    if (res < 0) {
        const char* err = conn->driver->get_error(conn->conn);
        if (conn->last_error) free(conn->last_error);
        conn->last_error = err ? strdup(err) : NULL;
    }
    return res;
}

long llm_db_error(long conn_ptr) {
    LlmDbConn* conn = (LlmDbConn*)conn_ptr;
    if (!conn || conn->type != 4) return (long)llm_rt_strdup("");
    if (conn->last_error) {
        return (long)llm_rt_strdup(conn->last_error);
    }
    return (long)llm_rt_strdup("");
}

static char* read_binding_file(const char* dir, const char* key) {
    char path[512];
    snprintf(path, sizeof(path), "%s/%s", dir, key);
    FILE* fp = fopen(path, "r");
    if (!fp) return NULL;
    
    char buf[1024];
    if (!fgets(buf, sizeof(buf), fp)) {
        fclose(fp);
        return NULL;
    }
    fclose(fp);
    
    // Trim trailing newlines and spaces
    size_t len = strlen(buf);
    while (len > 0 && (buf[len - 1] == '\n' || buf[len - 1] == '\r' || buf[len - 1] == ' ' || buf[len - 1] == '\t')) {
        buf[len - 1] = '\0';
        len--;
    }
    return strdup(buf);
}

long llm_db_connect_binding(long driver_name_ptr, long binding_name_ptr) {
    char* driver_name = (char*)driver_name_ptr;
    char* binding_name = (char*)binding_name_ptr;
    if (!driver_name || !binding_name) return 0;
    
    const char* root = getenv("SERVICE_BINDING_ROOT");
    if (!root) {
        root = "/bindings";
    }
    
    char dir[512];
    snprintf(dir, sizeof(dir), "%s/%s", root, binding_name);
    
    char* conn_str = NULL;
    
    // 1. Try "url" or "connection-string" first
    char* url = read_binding_file(dir, "url");
    if (!url) {
        url = read_binding_file(dir, "connection-string");
    }
    
    if (url) {
        conn_str = url;
    } else {
        // If not found, construct based on credentials
        if (strcmp(driver_name, "sqlite") == 0) {
            char* db_path = read_binding_file(dir, "database");
            if (!db_path) {
                db_path = read_binding_file(dir, "path");
            }
            if (db_path) {
                // sqlite://path
                size_t len = 9 + strlen(db_path) + 1;
                conn_str = malloc(len);
                snprintf(conn_str, len, "sqlite://%s", db_path);
                free(db_path);
            }
        } else {
            // General driver credentials
            char* host = read_binding_file(dir, "host");
            char* port = read_binding_file(dir, "port");
            char* username = read_binding_file(dir, "username");
            if (!username) username = read_binding_file(dir, "user");
            char* password = read_binding_file(dir, "password");
            char* database = read_binding_file(dir, "database");
            
            // Build: driver://username:password@host:port/database
            if (host) {
                size_t len = strlen(driver_name) + 4; // driver://
                if (username) len += strlen(username) + 1; // username: or username@
                if (password) len += strlen(password) + 1; // :password
                len += strlen(host);
                if (port) len += strlen(port) + 1; // :port
                if (database) len += strlen(database) + 1; // /database
                len += 1;
                
                conn_str = malloc(len);
                char* p = conn_str;
                p += sprintf(p, "%s://", driver_name);
                if (username) {
                    p += sprintf(p, "%s", username);
                    if (password) {
                        p += sprintf(p, ":%s", password);
                    }
                    p += sprintf(p, "@");
                }
                p += sprintf(p, "%s", host);
                if (port) {
                    p += sprintf(p, ":%s", port);
                }
                if (database) {
                    p += sprintf(p, "/%s", database);
                }
                
                if (host) free(host);
                if (port) free(port);
                if (username) free(username);
                if (password) free(password);
                if (database) free(database);
            }
        }
    }
    
    if (!conn_str) {
        conn_str = strdup("");
    }
    
    long res = llm_db_connect((long)driver_name, (long)conn_str);
    free(conn_str);
    return res;
}

// Compiler-compatible Wrappers (without llm_ prefix)
long db_connect(long driver_name_ptr, long conn_str_ptr) {
    return llm_db_connect(driver_name_ptr, conn_str_ptr);
}

long db_connect_binding(long driver_name_ptr, long binding_name_ptr) {
    return llm_db_connect_binding(driver_name_ptr, binding_name_ptr);
}

long db_query(long conn_ptr, long sql_ptr, long fields_csv_ptr, long params_soa) {
    return llm_db_query(conn_ptr, sql_ptr, fields_csv_ptr, params_soa);
}

long db_exec(long conn_ptr, long sql_ptr, long params_soa) {
    return llm_db_exec(conn_ptr, sql_ptr, params_soa);
}

long db_error(long conn_ptr) {
    return llm_db_error(conn_ptr);
}

