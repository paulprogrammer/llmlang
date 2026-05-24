#ifndef LLM_DB_H
#define LLM_DB_H

#include "common.h"

typedef struct DatabaseDriver {
    const char* name;
    
    // Core Operations
    long (*connect)(const char* conn_str);
    long (*query)(long conn, const char* sql, const char* fields_csv, long params_soa);
    long (*exec)(long conn, const char* sql, long params_soa);
    void (*close)(long conn);
    const char* (*get_error)(long conn);
    
    struct DatabaseDriver* next;
} DatabaseDriver;

void db_register_driver(DatabaseDriver* driver);
DatabaseDriver* db_find_driver(const char* name);

// FFI Entry Points
long llm_db_connect(long driver_name_ptr, long conn_str_ptr);
long llm_db_connect_binding(long driver_name_ptr, long binding_name_ptr);
long llm_db_query(long conn_ptr, long sql_ptr, long fields_csv_ptr, long params_soa);
long llm_db_exec(long conn_ptr, long sql_ptr, long params_soa);
long llm_db_error(long conn_ptr);

long db_connect(long driver_name_ptr, long conn_str_ptr);
long db_connect_binding(long driver_name_ptr, long binding_name_ptr);
long db_query(long conn_ptr, long sql_ptr, long fields_csv_ptr, long params_soa);
long db_exec(long conn_ptr, long sql_ptr, long params_soa);
long db_error(long conn_ptr);

#endif // LLM_DB_H
