// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::NativeType;
use crate::{tcc::Compiler, Symbol};
use std::ffi::c_void;
use std::ffi::CString;
use std::fmt::Write as _;

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
    NativeType::U64 | NativeType::USize => "uint64_t",
    NativeType::I64 | NativeType::ISize => "int64_t",
    NativeType::Void => "void",
    NativeType::F32 => "float",
    NativeType::F64 => "double",
    _ => unimplemented!(),
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
    NativeType::U64 | NativeType::USize => "uint64_t",
    NativeType::I64 | NativeType::ISize => "int64_t",
    _ => unimplemented!(),
  }
}

pub(crate) fn codegen(sym: &crate::Symbol) -> String {
  let mut c = String::from("#include <stdint.h>\n");
  let ret = native_to_c(&sym.result_type);

  // extern <return_type> func(
  c += "\nextern ";
  c += ret;
  c += " func(";
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
  c += ") {\n";
  // return func(p0, p1, ...);
  c += "  return func(";
  for (i, _) in sym.parameter_types.iter().enumerate() {
    if i > 0 {
      c += ", ";
    }
    let _ = write!(c, "p{i}");
  }
  c += ");\n}\n\n";
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

  #[test]
  fn test_gen_trampoline() {
    assert_eq!(
      codegen(vec![], NativeType::Void),
      "#include <stdint.h>\n\n\
      extern void func();\n\n\
      void func_trampoline(void* recv) {\
        \n  return func();\n\
      }\n\n"
    );
    assert_eq!(
      codegen(vec![NativeType::U32, NativeType::U32], NativeType::U32),
      "#include <stdint.h>\n\n\
      extern uint32_t func(uint32_t p0, uint32_t p1);\n\n\
      uint32_t func_trampoline(void* recv, uint32_t p0, uint32_t p1) {\
        \n  return func(p0, p1);\n\
      }\n\n"
    );
    assert_eq!(
      codegen(vec![NativeType::I32, NativeType::I32], NativeType::I32),
      "#include <stdint.h>\n\n\
      extern int32_t func(int32_t p0, int32_t p1);\n\n\
      int32_t func_trampoline(void* recv, int32_t p0, int32_t p1) {\
        \n  return func(p0, p1);\n\
      }\n\n"
    );
    assert_eq!(
      codegen(vec![NativeType::F32, NativeType::F32], NativeType::F32),
      "#include <stdint.h>\n\n\
      extern float func(float p0, float p1);\n\n\
      float func_trampoline(void* recv, float p0, float p1) {\
        \n  return func(p0, p1);\n\
      }\n\n"
    );
    assert_eq!(
      codegen(vec![NativeType::F64, NativeType::F64], NativeType::F64),
      "#include <stdint.h>\n\n\
      extern double func(double p0, double p1);\n\n\
      double func_trampoline(void* recv, double p0, double p1) {\
        \n  return func(p0, p1);\n\
      }\n\n"
    );
  }

  #[test]
  fn test_gen_trampoline_implicit_cast() {
    assert_eq!(
      codegen(vec![NativeType::I8, NativeType::U8], NativeType::I8),
      "#include <stdint.h>\n\n\
      extern int8_t func(int8_t p0, uint8_t p1);\n\n\
      int8_t func_trampoline(void* recv, int32_t p0, uint32_t p1) {\
        \n  return func(p0, p1);\n\
      }\n\n"
    );
    assert_eq!(
      codegen(vec![NativeType::ISize, NativeType::U64], NativeType::Void),
      "#include <stdint.h>\n\n\
      extern void func(int64_t p0, uint64_t p1);\n\n\
      void func_trampoline(void* recv, int64_t p0, uint64_t p1) {\
        \n  return func(p0, p1);\n\
      }\n\n"
    );
    assert_eq!(
      codegen(vec![NativeType::USize, NativeType::USize], NativeType::U32),
      "#include <stdint.h>\n\n\
      extern uint32_t func(uint64_t p0, uint64_t p1);\n\n\
      uint32_t func_trampoline(void* recv, uint64_t p0, uint64_t p1) {\
        \n  return func(p0, p1);\n\
      }\n\n"
    );
  }
}
