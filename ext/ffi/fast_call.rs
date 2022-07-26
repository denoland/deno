// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::ffi::c_void;
use std::iter::once;

use deno_core::v8::fast_api;
use dynasmrt::dynasm;
use dynasmrt::DynasmApi;
use dynasmrt::ExecutableBuffer;

use crate::NativeType;
use crate::Symbol;

pub(crate) fn is_compatible(sym: &Symbol) -> bool {
  cfg!(all(target_arch = "x86_64", target_family = "unix"))
    && !sym.can_callback
    && is_fast_api_rv(sym.result_type)
}

pub(crate) fn compile_trampoline(sym: &Symbol) -> Trampoline {
  Trampoline::compile(sym)
}

pub(crate) fn make_template(sym: &Symbol, trampoline: &Trampoline) -> Template {
  let args = once(fast_api::Type::V8Value)
    .chain(sym.parameter_types.iter().map(|t| t.into()))
    .collect::<Vec<_>>();

  Template {
    args: args.into_boxed_slice(),
    ret: (&fast_api::Type::from(&sym.result_type)).into(),
    symbol_ptr: trampoline.ptr(),
  }
}

/// Trampoline for fast-call FFI functions
///
/// Removes first argument (Javascript object receiver) and shifts the rest of arguments to the left
pub(crate) struct Trampoline(ExecutableBuffer);

impl Trampoline {
  fn ptr(&self) -> *const c_void {
    &self.0[0] as *const u8 as *const c_void
  }

  fn compile(sym: &Symbol) -> Self {
    /// Count the number of arguments passed, classified by argument class
    /// as defined in section 3.2.3 of the System V ABI spec
    /// https://refspecs.linuxfoundation.org/elf/x86_64-abi-0.99.pdf
    #[derive(Clone, Copy)]
    struct ArgCounter {
      // > Arguments of types (signed and unsigned) _Bool, char, short, int,
      // > long, long long, and pointers are in the INTEGER class.
      integer: i32,
      // > Arguments of types float, double, _Decimal32, _Decimal64 and
      // > __m64 are in class SSE.
      sse: i32,
    }
    let mut counter = ArgCounter {
      integer: -6, // rdi, rsi, rdx, rcx, r8, r9
      sse: -8,     // xmm0-xmm7
    };
    let args_to_shift: Vec<_> = sym
      .parameter_types
      .iter()
      .filter_map(|p| match p {
        NativeType::F32 | NativeType::F64 => {
          counter.sse += 1;
          if counter.sse > 0 && counter.integer >= 0 {
            // floats are only shifted to accomodate integer shift in the stack
            Some(counter)
          } else {
            None
          }
        }
        _ => {
          counter.integer += 1;
          Some(counter)
        }
      })
      .collect();

    let mut ops = dynasmrt::x64::Assembler::new().unwrap();
    if args_to_shift.is_empty() {
      dynasm!(ops
        ; .arch x64
        ; xor rdi, rdi
      );
    } else {
      for arg in args_to_shift {
        match arg {
          ArgCounter { integer: -5, .. } => dynasm!(ops
            ; .arch x64
            ; mov rdi, rsi
          ),
          ArgCounter { integer: -4, .. } => dynasm!(ops
            ; .arch x64
            ; mov rsi, rdx
          ),
          ArgCounter { integer: -3, .. } => dynasm!(ops
            ; .arch x64
            ; mov rdx, rcx
          ),
          ArgCounter { integer: -2, .. } => dynasm!(ops
            ; .arch x64
            ; mov rcx, r8
          ),
          ArgCounter { integer: -1, .. } => dynasm!(ops
            ; .arch x64
            ; mov r8, r9
          ),
          ArgCounter { integer: 0, sse } => dynasm!(ops
            ; .arch x64
            ; mov r9, [rsp + (1 + sse.max(0)) * 8]
          ),
          ArgCounter { integer, sse } => dynasm!(ops
            ; .arch x64
            ; mov rax, [rsp + (1 + integer + sse.max(0)) * 8]
            ; mov [rsp + (integer + sse.max(0)) * 8], rax
          ),
        }
      }
    }

    // tail-call
    // SAFETY: stack pointer is not modified, therefore stack remains (un)aligned.
    // Stack contains one extra parameter (the last one) when using >6 parameters, which the function will ignore
    dynasm!(ops
      ; .arch x64
      ; mov rax, QWORD sym.ptr.as_ptr() as _
      ; jmp rax
    );

    let executable_buf = ops.finalize().unwrap();
    Self(executable_buf)
  }
}

pub(crate) struct Template {
  args: Box<[fast_api::Type]>,
  ret: fast_api::CType,
  symbol_ptr: *const c_void,
}

impl fast_api::FastFunction for Template {
  fn function(&self) -> *const c_void {
    self.symbol_ptr
  }

  fn args(&self) -> &'static [fast_api::Type] {
    Box::leak(self.args.clone())
  }

  fn return_type(&self) -> fast_api::CType {
    self.ret
  }
}

impl From<&NativeType> for fast_api::Type {
  fn from(native_type: &NativeType) -> Self {
    match native_type {
      NativeType::U8 | NativeType::U16 | NativeType::U32 => {
        fast_api::Type::Uint32
      }
      NativeType::I8 | NativeType::I16 | NativeType::I32 => {
        fast_api::Type::Int32
      }
      NativeType::F32 => fast_api::Type::Float32,
      NativeType::F64 => fast_api::Type::Float64,
      NativeType::Void => fast_api::Type::Void,
      NativeType::I64 => fast_api::Type::Int64,
      NativeType::U64 => fast_api::Type::Uint64,
      NativeType::ISize => fast_api::Type::Int64,
      NativeType::USize | NativeType::Function | NativeType::Pointer => {
        fast_api::Type::Uint64
      }
    }
  }
}

fn is_fast_api_rv(rv: NativeType) -> bool {
  !matches!(
    rv,
    NativeType::Function
      | NativeType::Pointer
      | NativeType::I64
      | NativeType::ISize
      | NativeType::U64
      | NativeType::USize
  )
}
