// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::NativeType;
use crate::{tcc::Compiler, Symbol};
use std::ffi::c_void;
use std::ffi::CString;
use std::fmt::Write as _;
use std::mem::size_of;
use std::rc::Rc;

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

fn async_func_wrap(sym: &crate::Symbol) -> *const c_void {
  match sym.result_type {
    NativeType::Void => async_func::<()> as *const c_void,
    NativeType::U8 => async_func::<u8> as *const c_void,
    NativeType::U16 => async_func::<u16> as *const c_void,
    NativeType::U32 => async_func::<u32> as *const c_void,
    NativeType::I8 => async_func::<i8> as *const c_void,
    NativeType::I16 => async_func::<i16> as *const c_void,
    NativeType::I32 => async_func::<i32> as *const c_void,
    NativeType::F32 => async_func::<f32> as *const c_void,
    NativeType::F64 => async_func::<f64> as *const c_void,
    NativeType::U64 => async_func::<u64> as *const c_void,
    NativeType::I64 => async_func::<i64> as *const c_void,
    NativeType::ISize => async_func::<isize> as *const c_void,
    NativeType::USize => async_func::<usize> as *const c_void,
    NativeType::Pointer | NativeType::Function => {
      async_func::<usize> as *const c_void
    }
  }
}

#[repr(transparent)]
struct AsyncArgs(*const c_void);
unsafe impl Send for AsyncArgs {}

extern "C" fn async_func<
  T: Into<deno_core::serde_v8::SerializablePkg> + Send + 'static,
>(
  op_state: *const c_void,
  promise_id: i32,
  func: extern "C" fn(AsyncArgs) -> T,
  args: AsyncArgs,
) {
  dbg!(op_state as u64);
  deno_core::_ops::queue_async_op2(
    unsafe { Rc::from_raw(op_state as _) },
    async move {
      let value: T = tokio::task::spawn_blocking(move || func(args))
        .await
        .unwrap();
      (promise_id, None, deno_core::OpResult::Ok(value.into()))
    },
  );
}

extern "C" fn rust_malloc(size: usize) -> *mut u8 {
  unsafe {
    std::alloc::alloc(std::alloc::Layout::from_size_align(size, 8).unwrap())
  }
}

pub(crate) fn codegen(
  sym: &crate::Symbol,
  nonblocking: bool,
  state_ptr: u64,
) -> String {
  let mut c = String::from(include_str!("prelude.h"));
  let needs_unwrap = crate::needs_unwrap(sym.result_type);

  // Return type of the FFI call.
  let ffi_ret = native_to_c(&sym.result_type);
  // Return type of the trampoline.
  let ret = if needs_unwrap || nonblocking {
    "void"
  } else {
    ffi_ret
  };

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

  let mut call_s = String::from("func(");
  {
    for (i, ty) in sym.parameter_types.iter().enumerate() {
      if i > 0 {
        call_s += ", ";
      }
      if nonblocking {
        let _ = write!(call_s, "p->");
      }
      if matches!(ty, NativeType::Pointer) {
        let _ = write!(call_s, "p{i}->data");
      } else {
        let _ = write!(call_s, "p{i}");
      }
    }
    call_s += ");\n";
  }

  if nonblocking {
    let _ = writeln!(c, "uintptr_t op_state = {state_ptr};");
    c += "extern void async_func(uintptr_t state, int32_t promise_id, void* func, void* args);\n";
    if !sym.parameter_types.is_empty() {
      c += "extern void* rust_malloc(uintptr_t size);\n\n";
      c += "struct Args {\n";
      for (i, ty) in sym.parameter_types.iter().enumerate() {
        c += "  ";
        c += native_arg_to_c(ty);
        let _ = write!(c, " p{i}");
        c += ";\n";
      }
      c += "};\n";
      let _ =
        write!(c, "{ffi_ret} f(struct Args* p) {{\n  return {call_s}}};\n");
    }
  }

  // void* recv, <param_type> p0, <param_type> p1, ...);
  c += ret;
  c += " func_trampoline(";
  c += "void* recv";
  if nonblocking {
    c += ", int32_t promise_id";
  }
  for (i, ty) in sym.parameter_types.iter().enumerate() {
    c += ", ";
    c += native_arg_to_c(ty);
    let _ = write!(c, " p{i}");
  }
  if needs_unwrap {
    let _ = write!(c, ", struct FastApiTypedArray* const p_ret");
  }
  c += ") {\n";
  // This block of code defines how the actual FFI call is made
  // and how the return value is handled. It can be divided into
  // the following variants:
  // 1. Synchronous call, non 64-bit return value.
  //    This is the simplest case:
  //
  //      return func(p0, p1, ...);
  // 2. Synchronous call, 64-bit return value.
  //    64-bit return values need 'unwrap'ing. The value is copied into
  //    JS typed array passed as the last parameter `p_ret`.
  //
  //      <return_type> r = func(p0, p1, ...);
  //      ((<return_type>*)p_ret->data)[0] = r;
  // 3. Non-blocking call.
  //    Things get a little more complicated here. The call outlives the
  //    fast function call, so we need to copy over the arguments and send it to
  //    the `async_func` function.
  //    `async_func` runs the FFI call in a tokio blocking thread & registers a
  //    new unresolved op with the associated promise id. `Args*` allocation is
  //    free'd after the FFI call happens.
  //    64-bit return values that need 'unwrap'ing are handled in the `async_func`.
  //
  //      extern void async_func(void* op_state, int32_t promise_id, void* func, Args* args);
  //      struct Args {
  //        <param_type> p0; <param_type> p1; ...
  //      };
  //      struct Args *args = rust_malloc(sizeof(struct Args));
  //      args->p0 = p0; args->p1 = p1; ...
  //      <return_type> f(Args* p) {
  //        return func(p->p0, p->p1, ...);
  //      }
  //      async_func(op_state, promise_id, f, &args);
  if nonblocking {
    if !sym.parameter_types.is_empty() {
      c += "  struct Args *args = rust_malloc(sizeof(struct Args));\n";
      for (i, _) in sym.parameter_types.iter().enumerate() {
        let _ = writeln!(c, "  args->p{i} = p{i};");
      }
      c += "  async_func(op_state, promise_id, f, &args);\n";
    } else {
      c += "  async_func(op_state, promise_id, func, 0);\n";
    }
  } else if needs_unwrap {
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
  op_state: *const c_void,
  sym: Box<crate::Symbol>,
  nonblocking: bool,
) -> Result<Box<Allocation>, ()> {
  let mut ctx = Compiler::new()?;
  ctx.set_options(cstr!("-nostdlib"));
  // SAFETY: symbol satisfies ABI requirement.
  unsafe { ctx.add_symbol(cstr!("func"), sym.ptr.0 as *const c_void) };
  if nonblocking {
    dbg!(op_state as u64);
    // SAFETY: symbol satisfies ABI requirement.
    unsafe {
      ctx.add_symbol(cstr!("async_func"), async_func_wrap(&sym));
      ctx.add_symbol(cstr!("rust_malloc"), rust_malloc as *const c_void);
    };
  }
  let c = codegen(&sym, nonblocking, op_state as u64);
  if nonblocking {
    println!("{}", c)
  };
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
    super::codegen(&sym, false, 0)
  }

  fn codegen_async(parameters: Vec<NativeType>, ret: NativeType) -> String {
    let sym = Box::new(crate::Symbol {
      cif: libffi::middle::Cif::new(vec![], Type::void()),
      ptr: libffi::middle::CodePtr(null_mut()),
      parameter_types: parameters,
      result_type: ret,
      can_callback: false,
    });
    super::codegen(&sym, true, 0)
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
  fn test_gen_trampoline_async() {
    assert_codegen(
      codegen_async(vec![], NativeType::Void),
      "extern void func();\n\n\
      uintptr_t op_state = 0;\n\
      extern void async_func(uintptr_t state, int32_t promise_id, void* func, void* args);\n\
      void func_trampoline(void* recv, int32_t promise_id) {\
        \n  async_func(op_state, promise_id, func, 0);\n\
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
