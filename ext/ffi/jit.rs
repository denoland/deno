// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::tcc::Context;
use crate::NativeType;
use deno_core::error::AnyError;
use deno_core::v8;
use std::ffi::c_void;
use std::ffi::CString;

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

unsafe extern "C" fn deno_ffi_u8(
  info: *const v8::FunctionCallbackInfo,
  index: i32,
) -> u8 {
  let scope = &mut v8::CallbackScope::new(&*info);
  let info = v8::FunctionCallbackArguments::from_function_callback_info(info);
  info.get(index).uint32_value(scope).unwrap() as u8
}

macro_rules! impl_rv_int32 {
  ($ctx: ident, $ty: ty) => {
    paste::paste! {
      unsafe extern "C" fn [<deno_rv_ $ty>] (
        info: *const v8::FunctionCallbackInfo,
        val: $ty,
      ) {
        let mut rv = v8::ReturnValue::from_function_callback_info(info);
        rv.set_int32(val as i32);
      }

      $ctx.add_symbol(
        cstr!(stringify!([<deno_rv_ $ty>])),
        [<deno_rv_ $ty>] as *const c_void,
      );
    }
  };
}

pub struct Allocation {
  _raw: Vec<u8>,
  _sym: Box<crate::Symbol>,
  pub(crate) func: extern "C" fn(*const v8::FunctionCallbackInfo),
}

pub(crate) struct Compiler {
  ctx: Context,
  c: String,
}

impl Compiler {
  pub unsafe fn new() -> Result<Self, ()> {
    let mut ctx = Context::new()?;
    ctx.set_options(cstr!("-nostdlib"));

    ctx.add_symbol(cstr!("deno_ffi_u8"), deno_ffi_u8 as *const c_void);

    impl_rv_int32!(ctx, i8);
    impl_rv_int32!(ctx, u8);
    impl_rv_int32!(ctx, i16);
    impl_rv_int32!(ctx, u16);
    impl_rv_int32!(ctx, i32);
    Ok(Compiler {
      ctx,
      c: String::from(include_str!("jit.c")),
    })
  }

  pub unsafe fn compile(
    mut self,
    sym: Box<crate::Symbol>,
  ) -> Result<Allocation, ()> {
    self
      .ctx
      .add_symbol(cstr!("func"), sym.ptr.0 as *const c_void);

    // extern <return_type> func(
    self.c += "\nextern ";
    self.c += native_to_c(&sym.result_type);
    self.c += " func(";
    // <param_type> p0, <param_type> p1, ...);
    for (i, ty) in sym.parameter_types.iter().enumerate() {
      if i > 0 {
        self.c += ", ";
      }
      self.c += native_to_c(ty);
      self.c += &format!(" p{i}");
    }
    self.c += ");\n\n";

    // void main(void* info) {
    self.c += "void main(void* info) {\n";

    //  [<return_type> r =] func(
    match sym.result_type {
      crate::NativeType::Void => {
        self.c += "  func(";
      }
      _ => {
        self.c += "  ";
        self.c += native_to_c(&sym.result_type);
        self.c += " r = func(";
      }
    };

    // deno_ffi_<ty>(info, 0), deno_ffi_<ty>(info, 1), ...);
    for (i, ty) in sym.parameter_types.iter().enumerate() {
      if i != 0 {
        self.c += ", ";
      }
      let conv = match ty {
        NativeType::Void => unreachable!(),
        NativeType::U8 => "deno_ffi_u8",
        NativeType::U16 => "deno_ffi_u16",
        NativeType::U32 => "deno_ffi_u32",
        NativeType::U64 => "deno_ffi_u64",
        NativeType::USize => "deno_ffi_usize",
        NativeType::I8 => "deno_ffi_i8",
        NativeType::I16 => "deno_ffi_i16",
        NativeType::I32 => "deno_ffi_i32",
        NativeType::I64 => "deno_ffi_i64",
        NativeType::ISize => "deno_ffi_isize",
        NativeType::F32 => "deno_ffi_f32",
        NativeType::F64 => "deno_ffi_f64",
        NativeType::Pointer => "deno_ffi_pointer",
        NativeType::Function => "deno_ffi_function",
      };
      self.c += &format!("{conv}(info, {i})");
    }
    self.c += ");\n";

    // deno_rv_<ty>(info, r);
    if sym.result_type != crate::NativeType::Void {
      match sym.result_type {
        NativeType::I8 => self.c += "  deno_rv_i8(info, r);\n",
        NativeType::U8 => self.c += "  deno_rv_u8(info, r);\n",
        NativeType::I16 => self.c += "  deno_rv_i16(info, r);\n",
        NativeType::U16 => self.c += "  deno_rv_u16(info, r);\n",
        NativeType::I32 => self.c += "  deno_rv_i32(info, r);\n",
        NativeType::U32 => self.c += "  deno_rv_u32(info, r);\n",
        NativeType::I64 | NativeType::ISize => {
          self.c += "  deno_rv_i64(info, r);\n"
        }
        NativeType::U64 | NativeType::USize => {
          self.c += "  deno_rv_u64(info, r);\n"
        }
        NativeType::F32 => self.c += "  deno_rv_f32(info, r);\n",
        NativeType::F64 => self.c += "  deno_rv_f64(info, r);\n",
        NativeType::Pointer => self.c += "  deno_rv_pointer(info, r);\n",
        NativeType::Function => self.c += "  deno_rv_function(info, r);\n",
        NativeType::Void => unreachable!(),
      };
    };

    println!("{}", &self.c);
    self.ctx.compile_string(cstr!(self.c))?;
    let addr = self.ctx.relocate_and_get_symbol(cstr!("main"))?;
    Ok(Allocation {
      func: std::mem::transmute::<
        *mut c_void,
        extern "C" fn(*const v8::FunctionCallbackInfo),
      >(addr),
      _raw: self.ctx.bin.take().ok_or(())?,
      _sym: sym,
    })
  }
}
