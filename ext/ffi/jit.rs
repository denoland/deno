// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::v8;
use std::ffi::c_void;
use std::ffi::CString;
use std::os::raw::c_int as int;
use std::ptr::null_mut;

use crate::NativeType;

#[repr(C)]
#[derive(Debug)]
pub struct TCCState {
  _unused: [u8; 0],
}
pub const TCC_OUTPUT_MEMORY: int = 1;

extern "C" {
  pub fn tcc_new() -> *mut TCCState;
  pub fn tcc_delete(s: *mut TCCState);
  pub fn tcc_set_options(s: *mut TCCState, str: *const ::std::os::raw::c_char);
  pub fn tcc_compile_string(
    s: *mut TCCState,
    buf: *const ::std::os::raw::c_char,
  ) -> ::std::os::raw::c_int;
  pub fn tcc_add_symbol(
    s: *mut TCCState,
    name: *const ::std::os::raw::c_char,
    val: *const ::std::os::raw::c_void,
  ) -> ::std::os::raw::c_int;
  pub fn tcc_set_output_type(
    s: *mut TCCState,
    output_type: ::std::os::raw::c_int,
  ) -> ::std::os::raw::c_int;
  pub fn tcc_relocate(
    s1: *mut TCCState,
    ptr: *mut ::std::os::raw::c_void,
  ) -> ::std::os::raw::c_int;
  pub fn tcc_get_symbol(
    s: *mut TCCState,
    name: *const ::std::os::raw::c_char,
  ) -> *mut ::std::os::raw::c_void;
}

macro_rules! cstr {
  ($st:expr) => {
    &CString::new($st).unwrap()
  };
}

fn native_to_c(ty: &crate::NativeType) -> &'static str {
  match ty {
    crate::NativeType::Void => "void",
    crate::NativeType::U8 => "unsigned char",
    crate::NativeType::U16 => "unsigned short",
    crate::NativeType::U32 => "unsigned int",
    crate::NativeType::U64 => "unsigned long",
    crate::NativeType::USize => "unsigned long",
    crate::NativeType::I8 => "char",
    crate::NativeType::I16 => "short",
    crate::NativeType::I32 => "int",
    crate::NativeType::I64 => "long",
    crate::NativeType::ISize => "long",
    crate::NativeType::F32 => "float",
    crate::NativeType::F64 => "double",
    crate::NativeType::Pointer => "void*",
    crate::NativeType::Function => "void*",
  }
}

fn native_to_converter(ty: &crate::NativeType) -> &'static str {
  match ty {
    crate::NativeType::Void => unreachable!(),
    crate::NativeType::U8 => "deno_ffi_u8",
    crate::NativeType::U16 => "deno_ffi_u16",
    crate::NativeType::U32 => "deno_ffi_u32",
    crate::NativeType::U64 => "deno_ffi_u64",
    crate::NativeType::USize => "deno_ffi_usize",
    crate::NativeType::I8 => "deno_ffi_i8",
    crate::NativeType::I16 => "deno_ffi_i16",
    crate::NativeType::I32 => "deno_ffi_i32",
    crate::NativeType::I64 => "deno_ffi_i64",
    crate::NativeType::ISize => "deno_ffi_isize",
    crate::NativeType::F32 => "deno_ffi_f32",
    crate::NativeType::F64 => "deno_ffi_f64",
    crate::NativeType::Pointer => "deno_ffi_pointer",
    crate::NativeType::Function => "deno_ffi_function",
  }
}

unsafe extern "C" fn deno_ffi_u8(
  info: *const v8::FunctionCallbackInfo,
  i: int,
) -> u8 {
  let scope = unsafe { &mut v8::CallbackScope::new(&*info) };
  let info = v8::FunctionCallbackArguments::from_function_callback_info(info);
  info.get(i).uint32_value(scope).unwrap() as u8
}

macro_rules! impl_set_int32 {
  ($ty: ty) => {
    unsafe extern "C" fn deno_ffi_$ty(
      info: *const v8::FunctionCallbackInfo,
      val: $ty,
    ) {
      let mut rv = v8::ReturnValue::from_function_callback_info(info);
      rv.set_uint32(i, value as i32);
    }
  };
}

unsafe extern "C" fn deno_rv_u8(
  info: *const v8::FunctionCallbackInfo,
  value: u8,
) {
  let scope = unsafe { &mut v8::CallbackScope::new(&*info) };
  let mut rv = v8::ReturnValue::from_function_callback_info(info);
  rv.set(v8::Integer::new(scope, value as i32).into());
}

pub(crate) unsafe fn create_func<'s>(
  _scope: &mut v8::HandleScope<'s>,
  sym: &crate::Symbol,
) -> extern "C" fn(*const v8::FunctionCallbackInfo) {
  let ctx = tcc_new();

  tcc_set_options(ctx, cstr!("-nostdlib").as_ptr());
  tcc_set_output_type(ctx, TCC_OUTPUT_MEMORY);

  tcc_add_symbol(
    ctx,
    cstr!("deno_ffi_u8").as_ptr(),
    deno_ffi_u8 as *const c_void,
  );
  tcc_add_symbol(
    ctx,
    cstr!("deno_rv_u8").as_ptr(),
    deno_rv_u8 as *const c_void,
  );

  tcc_add_symbol(ctx, cstr!("func").as_ptr(), sym.ptr.0);

  let mut code = String::from(include_str!("jit.c"));

  code += "extern ";
  let result_c_type = native_to_c(&sym.result_type);
  code += result_c_type;
  code += " func(";
  for (i, param) in sym.parameter_types.iter().enumerate() {
    if i != 0 {
      code += ", ";
    }
    let ty = native_to_c(param);
    code += ty;
    code += &format!(" p{i}");
  }
  code += ");\n\n";

  code += "void main(void* info) {\n";

  match sym.result_type {
    crate::NativeType::Void => {
      code += "  func(";
    }
    _ => {
      code += "  ";
      code += result_c_type;
      code += " r = func(";
    }
  }

  for (i, ty) in sym.parameter_types.iter().enumerate() {
    if i != 0 {
      code += ", ";
    }
    code += &format!("{}(info, {i})", native_to_converter(ty));
  }

  code += ");\n";

  if sym.result_type != crate::NativeType::Void {
    match sym.result_type {
      NativeType::I8
      | NativeType::U8
      | NativeType::I32
      | NativeType::I16
      | NativeType::U16 => {
        code += "  deno_rv_u8(info, r);\n";
      }
      NativeType::U32 => {
        code += "  v8__ReturnValue__Set__UInt32(rv, r);\n";
      }
      NativeType::I64
      | NativeType::U64
      | NativeType::F32
      | NativeType::F64
      | NativeType::ISize
      | NativeType::USize => {
        code += "  v8__ReturnValue__Set__Double(rv, r);\n";
      }
      _ => todo!(),
    }
  }

  code += "}\n";

  println!("{}", code);
  tcc_compile_string(ctx, cstr!(code).as_ptr());
  // pass null ptr to get required length
  let len = tcc_relocate(ctx, null_mut());
  assert!(len != -1);
  dbg!(len);
  let mut bin = Vec::with_capacity(len as usize);
  let ret = tcc_relocate(ctx, bin.as_mut_ptr() as *mut c_void);
  assert!(ret == 0);
  bin.set_len(len as usize);

  let addr = tcc_get_symbol(ctx, cstr!("main").as_ptr());

  let func = std::mem::transmute(addr);
  Box::leak(Box::new(ctx));
  tcc_delete(ctx);
  func
}
