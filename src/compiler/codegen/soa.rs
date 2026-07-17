use crate::compiler::ast::Expr;
use crate::compiler::codegen::{CodeGen, StackItem};
use inkwell::types::BasicTypeEnum;
use inkwell::values::{BasicValue, BasicValueEnum, IntValue, PointerValue};

// SoA (structure-of-arrays) codegen shared by the New/Map/Filter/Get/Set
// arms in expr.rs. Descriptor layout: [count, col_ptr_0, col_ptr_1, ...],
// one 8-byte slot per entry; each column is a `count`-element array of
// 8-byte slots.
impl<'ctx> CodeGen<'ctx> {
    /// Allocate a SoA instance for `n_fields` columns of `count` elements
    /// and store the populated descriptor. Returns the descriptor as
    /// (raw pointer, same pointer as i64).
    pub fn gen_soa_alloc(
        &self,
        count: IntValue<'ctx>,
        n_fields: usize,
    ) -> (PointerValue<'ctx>, IntValue<'ctx>) {
        let i64_type = self.context.i64_type();
        let alloc_fn_type = self
            .context
            .ptr_type(inkwell::AddressSpace::default())
            .fn_type(&[i64_type.into()], false);
        let alloc_fn = self.get_or_add_external_fn("llm_alloc", alloc_fn_type);

        let mut members: Vec<BasicValueEnum<'ctx>> = Vec::new();
        members.push(count.into());
        for _ in 0..n_fields {
            let size_bytes = self
                .builder
                .build_int_mul(count, i64_type.const_int(8, false), "size")
                .unwrap();
            let call = self
                .builder
                .build_call(alloc_fn, &[size_bytes.into()], "col_ptr_raw")
                .unwrap();
            let ptr_val = self.get_call_res(call);
            let ptr = self
                .builder
                .build_ptr_to_int(ptr_val.into_pointer_value(), i64_type, "col_ptr")
                .unwrap();
            members.push(ptr.into());
        }
        let struct_size = (members.len() as u64) * 8;
        let call = self
            .builder
            .build_call(alloc_fn, &[i64_type.const_int(struct_size, false).into()], "struct_ptr_raw")
            .unwrap();
        let struct_ptr_raw = self.get_call_res(call).into_pointer_value();
        let struct_ptr_int = self
            .builder
            .build_ptr_to_int(struct_ptr_raw, i64_type, "struct_ptr")
            .unwrap();
        for (i, val) in members.into_iter().enumerate() {
            let member_ptr = unsafe {
                self.builder
                    .build_gep(i64_type, struct_ptr_raw, &[i64_type.const_int(i as u64, false)], "member_ptr")
                    .unwrap()
            };
            self.builder.build_store(member_ptr, val).unwrap();
        }
        (struct_ptr_raw, struct_ptr_int)
    }

    /// Resolve `field_name` on the SoA instance `instance_expr` to its
    /// descriptor slot (1-based; slot 0 holds the count) and field type
    /// name. Uses the inferred shape when available, otherwise falls back
    /// to the first shape containing the field. Panics with E007 when no
    /// shape has it.
    pub fn resolve_soa_field(
        &self,
        instance_expr: &Expr,
        field_name: &str,
        stack: &[StackItem<'ctx>],
    ) -> (u64, String) {
        let shapes = self.shapes.borrow();
        let inferred = self.infer_shape(instance_expr, stack);
        if let Some(ref shape_name) = inferred {
            if let Some(fields) = shapes.get(shape_name) {
                if let Some(idx) = fields.iter().position(|f| f == field_name) {
                    return ((idx + 1) as u64, fields[idx].clone());
                }
            }
        } else {
            for (_, fields) in shapes.iter() {
                if let Some(idx) = fields.iter().position(|f| f == field_name) {
                    return ((idx + 1) as u64, fields[idx].clone());
                }
            }
        }
        panic!("E007: field '{}' not found in shape '{:?}'", field_name, inferred);
    }

    /// Load column `field_idx`'s base pointer out of a SoA descriptor.
    /// Returns (pointer, i64) forms of the column base.
    pub fn gen_soa_col_ptr(
        &self,
        struct_ptr: PointerValue<'ctx>,
        field_idx: u64,
    ) -> (PointerValue<'ctx>, IntValue<'ctx>) {
        let i64_type = self.context.i64_type();
        let col_ptr_ptr = unsafe {
            self.builder
                .build_gep(i64_type, struct_ptr, &[i64_type.const_int(field_idx, false)], "col_ptr_ptr")
                .unwrap()
        };
        let col_ptr_int_val = self.builder.build_load(i64_type, col_ptr_ptr, "col_ptr_int").unwrap();
        let col_ptr_int = self.as_int(col_ptr_int_val);
        let col_ptr = self
            .builder
            .build_int_to_ptr(col_ptr_int, self.context.ptr_type(inkwell::AddressSpace::default()), "col_ptr")
            .unwrap();
        (col_ptr, col_ptr_int)
    }

    /// 8-byte-aligned element load from `col_ptr[index]`.
    pub fn gen_soa_elem_load(
        &self,
        elem_type: BasicTypeEnum<'ctx>,
        col_ptr: PointerValue<'ctx>,
        index: IntValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let gep = unsafe { self.builder.build_gep(elem_type, col_ptr, &[index], "gep").unwrap() };
        let loaded = self.builder.build_load(elem_type, gep, "load").unwrap();
        if let Some(inst) = loaded.as_instruction_value() {
            inst.set_alignment(8).unwrap();
        }
        loaded
    }

    /// 8-byte-aligned element store to `col_ptr[index]`.
    pub fn gen_soa_elem_store(
        &self,
        elem_type: BasicTypeEnum<'ctx>,
        col_ptr: PointerValue<'ctx>,
        index: IntValue<'ctx>,
        value: BasicValueEnum<'ctx>,
    ) {
        let gep = unsafe { self.builder.build_gep(elem_type, col_ptr, &[index], "gep").unwrap() };
        let store = self.builder.build_store(gep, value).unwrap();
        store.set_alignment(8).unwrap();
    }

    /// Load one logical row — every field at `index` — from a SoA instance.
    pub fn gen_soa_row_load(
        &self,
        inst_ptr: PointerValue<'ctx>,
        fields: &[String],
        index: IntValue<'ctx>,
    ) -> Vec<BasicValueEnum<'ctx>> {
        fields
            .iter()
            .enumerate()
            .map(|(idx, field_type_name)| {
                let (col_ptr, _) = self.gen_soa_col_ptr(inst_ptr, (idx + 1) as u64);
                let llvm_type = self.get_llvm_type(field_type_name);
                self.gen_soa_elem_load(llvm_type, col_ptr, index)
            })
            .collect()
    }

    /// Evaluate a Filter predicate on one row: calls `func_expr` (an
    /// identifier naming a function) with the row values and returns an i1
    /// that is true when the row matches. Non-identifier predicates match
    /// every row.
    pub fn gen_soa_predicate(
        &self,
        func_expr: &Expr,
        row_vals: &[BasicValueEnum<'ctx>],
    ) -> IntValue<'ctx> {
        if let Expr::Identifier(ref name) = *func_expr {
            let resolved = self.resolve_func_name(name);
            let function = self.module.get_function(&resolved).expect("E010");
            let meta_vals: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
                row_vals.iter().map(|v| (*v).into()).collect();
            let res_val = self.get_call_res(self.builder.build_call(function, &meta_vals, "pred").unwrap());
            let res = self.as_int(res_val);
            self.builder
                .build_int_compare(
                    inkwell::IntPredicate::NE,
                    res,
                    self.context.i64_type().const_int(0, false),
                    "is_matched",
                )
                .unwrap()
        } else {
            self.context.bool_type().const_int(1, false)
        }
    }
}
