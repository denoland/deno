// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::NativeType;
use crate::{tcc::Compiler, Symbol};
use std::ffi::c_void;
use std::ffi::CString;
use std::fmt::Write as _;
use std::mem::size_of;

const _: () = assert!(size_of::<fn()>() == size_of::<usize>());

pub(crate) struct Allocation {
  pub addr: *mut c_void,
  _ctx: Compiler,
  _sym: Box<Symbol>,
}

macro_rules! cstr {
  ($st:expr) => {
    &CString::new($st).unwrap()
  };
}

fn native_arg_to_c(ty: &NativeType) -> &'static str {
  match ty {
    NativeType::U8 | NativeType::U16 | NativeType::U32 => "uint32_t",
    NativeType::I8 | NativeType::I16 | NativeType::I32 => "int32_t",
    NativeType::Void => "void",
    NativeType::F32 => "float",
    NativeType::F64 => "double",
    NativeType::U64 => "uint64_t",
    NativeType::I64 => "int64_t",
    NativeType::ISize => "intptr_t",
    NativeType::USize => "uintptr_t",
    NativeType::Pointer => "struct FastApiTypedArray*",
    NativeType::Function => "void*",
  }
}

fn native_to_c(ty: &NativeType) -> &'static str {
  match ty {
    NativeType::U8 => "uint8_t",
    NativeType::U16 => "uint16_t",
    NativeType::U32 => "uint32_t",
    NativeType::I8 => "int8_t",
    NativeType::I16 => "uint16_t",
    NativeType::I32 => "int32_t",
    NativeType::Void => "void",
    NativeType::F32 => "float",
    NativeType::F64 => "double",
    NativeType::U64 => "uint64_t",
    NativeType::I64 => "int64_t",
    NativeType::ISize => "intptr_t",
    NativeType::USize => "uintptr_t",
    NativeType::Pointer | NativeType::Function => "void*",
  }
}

pub(crate) fn codegen(sym: &crate::Symbol) -> String {
  let mut c = String::from(include_str!("prelude.h"));
  let needs_unwrap = crate::needs_unwrap(sym.result_type);

  // Return type of the FFI call.
  let ffi_ret = native_to_c(&sym.result_type);
  // Return type of the trampoline.
  let ret = if needs_unwrap { "void" } else { ffi_ret };

  // extern <return_type> func(
  let _ = write!(c, "\nextern {ffi_ret} func(");
  // <param_type> p0, <param_type> p1, ...);
  for (i, ty) in sym.parameter_types.iter().enumerate() {
    if i > 0 {
      c += ", ";
    }
    c += native_to_c(ty);
    let _ = write!(c, " p{i}");
  }
  c += ");\n\n";

  // void* recv, <param_type> p0, <param_type> p1, ...);
  c += ret;
  c += " func_trampoline(";
  c += "void* recv";
  for (i, ty) in sym.parameter_types.iter().enumerate() {
    c += ", ";
    c += native_arg_to_c(ty);
    let _ = write!(c, " p{i}");
  }
  if needs_unwrap {
    let _ = write!(c, ", struct FastApiTypedArray* const p_ret");
  }
  c += ") {\n";
  // func(p0, p1, ...);
  let mut call_s = String::from("func(");
  {
    for (i, ty) in sym.parameter_types.iter().enumerate() {
      if i > 0 {
        call_s += ", ";
      }
      if matches!(ty, NativeType::Pointer) {
        let _ = write!(call_s, "p{i}->data");
      } else {
        let _ = write!(call_s, "p{i}");
      }
    }
    call_s += ");\n";
  }
  if needs_unwrap {
    // <return_type> r = func(p0, p1, ...);
    // ((<return_type>*)p_ret->data)[0] = r;
    let _ = write!(c, " {ffi_ret} r = {call_s}");
    let _ = writeln!(c, " (({ffi_ret}*)p_ret->data)[0] = r;");
  } else {
    // return func(p0, p1, ...);
    let _ = write!(c, "  return {call_s}");
  }
  c += "}\n\n";
  c
}

pub(crate) fn gen_trampoline(
  sym: Box<crate::Symbol>,
) -> Result<Box<Allocation>, ()> {
  let mut ctx = Compiler::new()?;
  ctx.set_options(cstr!("-nostdlib"));
  // SAFETY: symbol satisfies ABI requirement.
  unsafe { ctx.add_symbol(cstr!("func"), sym.ptr.0 as *const c_void) };
  let c = codegen(&sym);
  ctx.compile_string(cstr!(c))?;
  let alloc = Allocation {
    addr: ctx.relocate_and_get_symbol(cstr!("func_trampoline"))?,
    _ctx: ctx,
    _sym: sym,
  };
  Ok(Box::new(alloc))
}

#[cfg(test)]
mod tests {
  use super::*;
  use libffi::middle::Type;
  use std::ptr::null_mut;

  fn codegen(parameters: Vec<NativeType>, ret: NativeType) -> String {
    let sym = Box::new(crate::Symbol {
      cif: libffi::middle::Cif::new(vec![], Type::void()),
      ptr: libffi::middle::CodePtr(null_mut()),
      parameter_types: parameters,
      result_type: ret,
      can_callback: false,
    });
    super::codegen(&sym)
  }

  const PRELUDE: &str = include_str!("prelude.h");
  fn assert_codegen(expected: String, actual: &str) {
    assert_eq!(expected, format!("{PRELUDE}\n{}", actual))
  }

  #[test]
  fn test_gen_trampoline() {
    assert_codegen(
      codegen(vec![], NativeType::Void),
      "extern void func();\n\n\
      void func_trampoline(void* recv) {\
        \n  return func();\n\
      }\n\n",
    );
    assert_codegen(
      codegen(vec![NativeType::U32, NativeType::U32], NativeType::U32),
      "extern uint32_t func(uint32_t p0, uint32_t p1);\n\n\
      uint32_t func_trampoline(void* recv, uint32_t p0, uint32_t p1) {\
        \n  return func(p0, p1);\n\
      }\n\n",
    );
    assert_codegen(
      codegen(vec![NativeType::I32, NativeType::I32], NativeType::I32),
      "extern int32_t func(int32_t p0, int32_t p1);\n\n\
      int32_t func_trampoline(void* recv, int32_t p0, int32_t p1) {\
        \n  return func(p0, p1);\n\
      }\n\n",
    );
    assert_codegen(
      codegen(vec![NativeType::F32, NativeType::F32], NativeType::F32),
      "extern float func(float p0, float p1);\n\n\
      float func_trampoline(void* recv, float p0, float p1) {\
        \n  return func(p0, p1);\n\
      }\n\n",
    );
    assert_codegen(
      codegen(vec![NativeType::F64, NativeType::F64], NativeType::F64),
      "extern double func(double p0, double p1);\n\n\
      double func_trampoline(void* recv, double p0, double p1) {\
        \n  return func(p0, p1);\n\
      }\n\n",
    );
    assert_codegen(
      codegen(vec![NativeType::Pointer, NativeType::U32], NativeType::U32),
      "extern uint32_t func(void* p0, uint32_t p1);\n\n\
      uint32_t func_trampoline(void* recv, struct FastApiTypedArray* p0, uint32_t p1) {\
        \n  return func(p0->data, p1);\n\
      }\n\n",
    );
    assert_codegen(
      codegen(vec![NativeType::Pointer, NativeType::Pointer], NativeType::U32),
      "extern uint32_t func(void* p0, void* p1);\n\n\
      uint32_t func_trampoline(void* recv, struct FastApiTypedArray* p0, struct FastApiTypedArray* p1) {\
        \n  return func(p0->data, p1->data);\n\
      }\n\n",
    );
    assert_codegen(
      codegen(vec![], NativeType::U64),
      "extern uint64_t func();\n\n\
      void func_trampoline(void* recv, struct FastApiTypedArray* const p_ret) {\
        \n uint64_t r = func();\
        \n ((uint64_t*)p_ret->data)[0] = r;\n\
      }\n\n",
    );
    assert_codegen(
      codegen(vec![NativeType::Pointer, NativeType::Pointer], NativeType::U64),
      "extern uint64_t func(void* p0, void* p1);\n\n\
      void func_trampoline(void* recv, struct FastApiTypedArray* p0, struct FastApiTypedArray* p1, struct FastApiTypedArray* const p_ret) {\
        \n uint64_t r = func(p0->data, p1->data);\
        \n ((uint64_t*)p_ret->data)[0] = r;\n\
      }\n\n",
    );
  }

  #[test]
  fn test_gen_trampoline_implicit_cast() {
    assert_codegen(
      codegen(vec![NativeType::I8, NativeType::U8], NativeType::I8),
      "extern int8_t func(int8_t p0, uint8_t p1);\n\n\
      int8_t func_trampoline(void* recv, int32_t p0, uint32_t p1) {\
        \n  return func(p0, p1);\n\
      }\n\n",
    );
    assert_codegen(
      codegen(vec![NativeType::ISize, NativeType::U64], NativeType::Void),
      "extern void func(intptr_t p0, uint64_t p1);\n\n\
      void func_trampoline(void* recv, intptr_t p0, uint64_t p1) {\
        \n  return func(p0, p1);\n\
      }\n\n",
    );
    assert_codegen(
      codegen(vec![NativeType::USize, NativeType::USize], NativeType::U32),
      "extern uint32_t func(uintptr_t p0, uintptr_t p1);\n\n\
      uint32_t func_trampoline(void* recv, uintptr_t p0, uintptr_t p1) {\
        \n  return func(p0, p1);\n\
      }\n\n",
    );
  }
}
