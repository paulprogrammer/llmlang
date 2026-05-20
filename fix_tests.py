import re

file_path = "/home/paul/PROJ/llmlang/tests/compiler_tests.rs"
with open(file_path, 'r') as f:
    content = f.read()

# 1. Update gen_function calls
# Skip the one in test_positive_export_sig for now as it needs special handling
# Matches codegen.gen_function(args) where it's not in the loop
def replace_gen_function(match):
    if "test_positive_export_sig" in content[max(0, match.start()-1000):match.start()]:
         # Inside test_positive_export_sig, let's do it separately or carefully
         return match.group(0)
    
    # Check if it already has 4 arguments (comma count)
    # This is a bit naive but works for these tests
    args = match.group(1)
    if args.count(',') < 3:
        return f"codegen.gen_function({args}, false)"
    return match.group(0)

# 2. Update gen_shape calls
def replace_gen_shape(match):
    if "test_positive_export_sig" in content[max(0, match.start()-1000):match.start()]:
         return match.group(0)
    args = match.group(1)
    if args.count(',') < 2:
        return f"codegen.gen_shape({args}, false)"
    return match.group(0)

# Actually, it's safer to just target the specific patterns found in the file
new_content = content

# test_positive_math, test_positive_div, test_positive_soa_shape, test_positive_string_literals
new_content = new_content.replace('codegen.gen_function("main", vec![], &ast);', 'codegen.gen_function("main", vec![], &ast, false);')

# test_positive_comparisons, test_positive_bitwise, test_positive_recursion, test_positive_expansion, test_positive_import, test_positive_multi_arity_codegen, test_positive_nested_multi_arity, test_positive_auto_parallelism, test_positive_temporal, test_positive_env, test_positive_money, test_positive_trap, test_integration_nested_traps, test_integration_json_filter, test_integration_complex_fault_tolerance
new_content = new_content.replace('codegen.gen_function(&name, params, &body);', 'codegen.gen_function(&name, params, &body, false);')

# test_positive_debruijn, test_positive_move_borrow, test_negative_double_move, test_positive_let, test_esoteric_parallel_recursion, test_esoteric_parallel_inside_trap, test_esoteric_multi_move_error
new_content = new_content.replace('codegen.gen_function(&name, params, &body);', 'codegen.gen_function(&name, params, &body, false);')

# test_positive_soa_shape
new_content = new_content.replace('codegen.gen_function("get_x", vec![], &body);', 'codegen.gen_function("get_x", vec![], &body, false);')
new_content = new_content.replace('codegen.gen_shape("Point", &["x".to_string(), "y".to_string()]);', 'codegen.gen_shape("Point", &["x".to_string(), "y".to_string()], false);')

# test_positive_expansion
new_content = new_content.replace('codegen.gen_shape("Point", &["x".to_string()]);', 'codegen.gen_shape("Point", &["x".to_string()], false);')
new_content = new_content.replace('codegen.gen_function("wrapper", vec![], &call_ast);', 'codegen.gen_function("wrapper", vec![], &call_ast, false);')

# test_positive_json
new_content = new_content.replace('codegen.gen_shape("User", &["id".to_string(), "age".to_string()]);', 'codegen.gen_shape("User", &["id".to_string(), "age".to_string()], false);')

# test_integration_json_filter
new_content = new_content.replace('codegen.gen_shape("User", &["id".to_string(), "active".to_string()]);', 'codegen.gen_shape("User", &["id".to_string(), "active".to_string()], false);')

# test_positive_string_ops
new_content = new_content.replace('codegen.gen_function(&name, params, &body);', 'codegen.gen_function(&name, params, &body, false);')

# test_positive_regex
new_content = new_content.replace('codegen.gen_function(&name, params, &body);', 'codegen.gen_function(&name, params, &body, false);')

# test_positive_system_ops
new_content = new_content.replace('codegen.gen_function(&name, params, &body);', 'codegen.gen_function(&name, params, &body, false);')

# test_positive_split_op
new_content = new_content.replace('codegen.gen_function(&name, params, &body);', 'codegen.gen_function(&name, params, &body, false);')

# test_positive_export_sig (SPECIAL)
new_content = new_content.replace('Expr::Shape(n, f, _) => codegen.gen_shape(&n, &f),', 'Expr::Shape(n, f, exported) => codegen.gen_shape(&n, &f, exported),')
new_content = new_content.replace('Expr::Define(n, p, b, _) => { codegen.gen_function(&n, p, &b); },', 'Expr::Define(n, p, b, exported) => { codegen.gen_function(&n, p, &b, exported); },')

with open(file_path, 'w') as f:
    f.write(new_content)
