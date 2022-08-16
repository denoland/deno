// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::cmp::max;
use std::ffi::c_void;
use std::iter::once;

use deno_core::v8::fast_api;
use dynasmrt::dynasm;
use dynasmrt::DynasmApi;
use dynasmrt::ExecutableBuffer;

use crate::NativeType;
use crate::Symbol;

pub(crate) fn is_compatible(sym: &Symbol) -> bool {
  cfg!(any(
    all(target_arch = "x86_64", target_family = "unix"),
    all(target_arch = "x86_64", target_family = "windows"),
    all(target_arch = "aarch64", target_vendor = "apple")
  )) && !sym.can_callback
    && is_fast_api_rv(sym.result_type)
}

pub(crate) fn compile_trampoline(sym: &Symbol) -> Trampoline {
  #[cfg(all(target_arch = "x86_64", target_family = "unix"))]
  return SysVAmd64::compile(sym);
  #[cfg(all(target_arch = "x86_64", target_family = "windows"))]
  return Win64::compile(sym);
  #[cfg(all(target_arch = "aarch64", target_vendor = "apple"))]
  return Aarch64Apple::compile(sym);
  #[allow(unreachable_code)]
  {
    unimplemented!("fast API is not implemented for the current target");
  }
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
/// Calls the FFI function without the first argument (the receiver)
pub(crate) struct Trampoline(ExecutableBuffer);

impl Trampoline {
  fn ptr(&self) -> *const c_void {
    &self.0[0] as *const u8 as *const c_void
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

struct SysVAmd64 {
  // Reference: https://refspecs.linuxfoundation.org/elf/x86_64-abi-0.99.pdf
  assmblr: dynasmrt::x64::Assembler,
  // Parameter counters
  integer_params: u32,
  float_params: u32,
  // Stack offset accumulators
  offset_trampoline: u32,
  offset_callee: u32,
  allocated_stack: u32,
}

#[cfg_attr(
  not(all(target_aarch = "x86_64", target_family = "unix")),
  allow(dead_code)
)]
impl SysVAmd64 {
  // Integer arguments go to the following GPR, in order: rdi, rsi, rdx, rcx, r8, r9
  const INTEGER_REGISTERS: u32 = 6;
  // SSE arguments go to the first 8 SSE registers: xmm0-xmm7
  const FLOAT_REGISTERS: u32 = 8;

  fn new() -> Self {
    Self {
      assmblr: dynasmrt::x64::Assembler::new().unwrap(),
      integer_params: 0,
      float_params: 0,
      // Start at 8 to account for trampoline caller's return address
      offset_trampoline: 8,
      offset_callee: 8,
      allocated_stack: 0,
    }
  }

  fn compile(sym: &Symbol) -> Trampoline {
    let mut compiler = Self::new();

    let can_tailcall = !compiler.must_cast_return_value(sym.result_type);
    if !can_tailcall {
      compiler.allocate_stack(&sym.parameter_types);
    }

    for param in &sym.parameter_types {
      compiler.move_left(param)
    }
    if !compiler.is_recv_arg_overridden() {
      // the receiver object should never be expected. Avoid its unexpected or deliberate leak
      compiler.zero_first_arg();
    }

    if !can_tailcall {
      compiler.call(sym.ptr.as_ptr());
      if compiler.must_cast_return_value(sym.result_type) {
        compiler.cast_return_value(sym.result_type);
      }
      compiler.deallocate_stack();
      compiler.ret();
    } else {
      compiler.tailcall(sym.ptr.as_ptr());
    }

    Trampoline(compiler.finalize())
  }

  fn move_left(&mut self, param: &NativeType) {
    // Section 3.2.3 of the SysV ABI spec, on argument classification:
    // - INTEGER:
    //    > Arguments of types (signed and unsigned) _Bool, char, short, int,
    //    > long, long long, and pointers are in the INTEGER class.
    // - SSE:
    //    > Arguments of types float, double, _Decimal32, _Decimal64 and
    //    > __m64 are in class SSE.
    //
    match param {
      NativeType::F32 => self.move_float(Single),
      NativeType::F64 => self.move_float(Double),
      NativeType::U8 => self.move_integer(U(B)),
      NativeType::U16 => self.move_integer(U(W)),
      NativeType::U32 | NativeType::Void => self.move_integer(U(DW)),
      NativeType::U64
      | NativeType::USize
      | NativeType::Function
      | NativeType::Pointer => self.move_integer(U(QW)),
      NativeType::I8 => self.move_integer(I(B)),
      NativeType::I16 => self.move_integer(I(W)),
      NativeType::I32 => self.move_integer(I(DW)),
      NativeType::I64 | NativeType::ISize => self.move_integer(I(QW)),
    }
  }

  fn move_float(&mut self, param: Floating) {
    // Section 3.2.3 of the SysV AMD64 ABI:
    // > If the class is SSE, the next available vector register is used, the registers
    // > are taken in the order from %xmm0 to %xmm7.
    // [...]
    // > Once registers are assigned, the arguments passed in memory are pushed on
    // > the stack in reversed (right-to-left) order

    let param_i = self.float_params + 1;
    self.float_params = param_i;
    let is_in_stack = param_i > Self::FLOAT_REGISTERS;
    // floats are only moved to accommodate integer movement in the stack
    let stack_has_moved = self.allocated_stack > 0
      || self.integer_params >= Self::INTEGER_REGISTERS;

    if is_in_stack && stack_has_moved {
      match param {
        Single => dynasm!(self.assmblr
          ; .arch x64
          ; movss xmm8, [rsp + self.offset_trampoline as i32]
          ; movss [rsp + self.offset_callee as i32], xmm8
        ),
        Double => dynasm!(self.assmblr
          ; .arch x64
          ; movsd xmm8, [rsp + self.offset_trampoline as i32]
          ; movsd [rsp + self.offset_callee as i32], xmm8
        ),
      }

      // Section 3.2.3 of the SysV AMD64 ABI:
      // > The size of each argument gets rounded up to eightbytes. [...] Therefore the stack will always be eightbyte aligned.
      self.offset_trampoline += 8;
      self.offset_callee += 8;

      debug_assert!(
        self.allocated_stack == 0 || self.offset_callee <= self.allocated_stack
      );
    }
  }

  fn move_integer(&mut self, arg: Integer) {
    // Section 3.2.3 of the SysV AMD64 ABI:
    // > If the class is INTEGER, the next available register of the sequence %rdi,
    // > %rsi, %rdx, %rcx, %r8 and %r9 is used
    // [...]
    // > Once registers are assigned, the arguments passed in memory are pushed on
    // > the stack in reversed (right-to-left) order
    let arg_i = self.integer_params + 1;
    self.integer_params = arg_i;

    // move each argument one position to the left. The first argument in the stack moves to the last integer register (r9).
    // If the FFI function is called with a new stack frame, the arguments remaining in the stack are copied to the new stack frame.
    // Otherwise, they are copied 8 bytes lower in the same frame
    match (arg_i, arg) {
      // u8 and u16 parameters are defined as u32 parameters in the V8's fast API function. The trampoline takes care of the cast.
      // Conventionally, many compilers expect 8 and 16 bit arguments to be sign/zero extended by the caller
      // See https://stackoverflow.com/a/36760539/2623340
      (1, U(B)) => dynasm!(self.assmblr; .arch x64; movzx edi, sil),
      (1, I(B)) => dynasm!(self.assmblr; .arch x64; movsx edi, sil),
      (1, U(W)) => dynasm!(self.assmblr; .arch x64; movzx edi, si),
      (1, I(W)) => dynasm!(self.assmblr; .arch x64; movsx edi, si),
      (1, U(DW) | I(DW)) => dynasm!(self.assmblr; .arch x64; mov edi, esi),
      (1, U(QW) | I(QW)) => dynasm!(self.assmblr; .arch x64; mov rdi, rsi),

      (2, U(B)) => dynasm!(self.assmblr; .arch x64; movzx esi, dl),
      (2, I(B)) => dynasm!(self.assmblr; .arch x64; movsx esi, dl),
      (2, U(W)) => dynasm!(self.assmblr; .arch x64; movzx esi, dx),
      (2, I(W)) => dynasm!(self.assmblr; .arch x64; movsx esi, dx),
      (2, U(DW) | I(DW)) => dynasm!(self.assmblr; .arch x64; mov esi, edx),
      (2, U(QW) | I(QW)) => dynasm!(self.assmblr; .arch x64; mov rsi, rdx),

      (3, U(B)) => dynasm!(self.assmblr; .arch x64; movzx edx, cl),
      (3, I(B)) => dynasm!(self.assmblr; .arch x64; movsx edx, cl),
      (3, U(W)) => dynasm!(self.assmblr; .arch x64; movzx edx, cx),
      (3, I(W)) => dynasm!(self.assmblr; .arch x64; movsx edx, cx),
      (3, U(DW) | I(DW)) => dynasm!(self.assmblr; .arch x64; mov edx, ecx),
      (3, U(QW) | I(QW)) => dynasm!(self.assmblr; .arch x64; mov rdx, rcx),

      (4, U(B)) => dynasm!(self.assmblr; .arch x64; movzx ecx, r8b),
      (4, I(B)) => dynasm!(self.assmblr; .arch x64; movsx ecx, r8b),
      (4, U(W)) => dynasm!(self.assmblr; .arch x64; movzx ecx, r8w),
      (4, I(W)) => dynasm!(self.assmblr; .arch x64; movsx ecx, r8w),
      (4, U(DW) | I(DW)) => dynasm!(self.assmblr; .arch x64; mov ecx, r8d),
      (4, U(QW) | I(QW)) => dynasm!(self.assmblr; .arch x64; mov rcx, r8),

      (5, U(B)) => dynasm!(self.assmblr; .arch x64; movzx r8d, r9b),
      (5, I(B)) => dynasm!(self.assmblr; .arch x64; movsx r8d, r9b),
      (5, U(W)) => dynasm!(self.assmblr; .arch x64; movzx r8d, r9w),
      (5, I(W)) => dynasm!(self.assmblr; .arch x64; movsx r8d, r9w),
      (5, U(DW) | I(DW)) => dynasm!(self.assmblr; .arch x64; mov r8d, r9d),
      (5, U(QW) | I(QW)) => dynasm!(self.assmblr; .arch x64; mov r8, r9),

      (6, param) => {
        // First argument in stack goes to last register (r9)
        match param {
          U(B) => {
            dynasm!(self.assmblr; .arch x64; movzx r9d, BYTE [rsp + self.offset_trampoline as i32])
          }
          I(B) => {
            dynasm!(self.assmblr; .arch x64; movsx r9d, BYTE [rsp + self.offset_trampoline as i32])
          }
          U(W) => {
            dynasm!(self.assmblr; .arch x64; movzx r9d, WORD [rsp + self.offset_trampoline as i32])
          }
          I(W) => {
            dynasm!(self.assmblr; .arch x64; movsx r9d, WORD [rsp + self.offset_trampoline as i32])
          }
          U(DW) | I(DW) => {
            dynasm!(self.assmblr; .arch x64; mov r9d, [rsp + self.offset_trampoline as i32])
          }
          U(QW) | I(QW) => {
            dynasm!(self.assmblr; .arch x64; mov r9, [rsp + self.offset_trampoline as i32])
          }
        }
        // Section 3.2.3 of the SysV AMD64 ABI:
        // > The size of each argument gets rounded up to eightbytes. [...] Therefore the stack will always be eightbyte aligned.
        self.offset_trampoline += 8;
      }

      (_, param) => {
        match param {
          U(B) => dynasm!(self.assmblr
            ; .arch x64
            ; movzx eax, BYTE [rsp + self.offset_trampoline as i32]
            ; mov [rsp + self.offset_callee as i32], eax
          ),
          I(B) => dynasm!(self.assmblr
            ; .arch x64
            ; movsx eax, BYTE [rsp + self.offset_trampoline as i32]
            ; mov [rsp + self.offset_callee as i32], eax
          ),
          U(W) => dynasm!(self.assmblr
            ; .arch x64
            ; movzx eax, WORD [rsp + self.offset_trampoline as i32]
            ; mov [rsp + self.offset_callee as i32], eax
          ),
          I(W) => dynasm!(self.assmblr
            ; .arch x64
            ; movsx eax, WORD [rsp + self.offset_trampoline as i32]
            ; mov [rsp + self.offset_callee as i32], eax
          ),
          U(DW) | I(DW) => dynasm!(self.assmblr
            ; .arch x64
            ; mov eax, [rsp + self.offset_trampoline as i32]
            ; mov [rsp + self.offset_callee as i32], eax
          ),
          U(QW) | I(QW) => dynasm!(self.assmblr
            ; .arch x64
            ; mov rax, [rsp + self.offset_trampoline as i32]
            ; mov [rsp + self.offset_callee as i32], rax
          ),
        }
        // Section 3.2.3 of the SysV AMD64 ABI:
        // > The size of each argument gets rounded up to eightbytes. [...] Therefore the stack will always be eightbyte aligned.
        self.offset_trampoline += 8;
        self.offset_callee += 8;

        debug_assert!(
          self.allocated_stack == 0
            || self.offset_callee <= self.allocated_stack
        );
      }
    }
  }

  fn zero_first_arg(&mut self) {
    dynasm!(self.assmblr
      ; .arch x64
      ; xor edi, edi
    );
  }

  fn cast_return_value(&mut self, rv: NativeType) {
    // V8 only supports 32bit integers. We support 8 and 16 bit integers casting them to 32bits.
    // In SysV-AMD64 the convention dictates that the unused bits of the return value contain garbage, so we
    // need to zero/sign extend the return value explicitly
    match rv {
      NativeType::U8 => dynasm!(self.assmblr; .arch x64; movzx eax, al),
      NativeType::I8 => dynasm!(self.assmblr; .arch x64; movsx eax, al),
      NativeType::U16 => dynasm!(self.assmblr; .arch x64; movzx eax, ax),
      NativeType::I16 => dynasm!(self.assmblr; .arch x64; movsx eax, ax),
      _ => (),
    }
  }

  fn allocate_stack(&mut self, params: &[NativeType]) {
    let mut int_params = 0u32;
    let mut sse_params = 0u32;
    for param in params {
      match param {
        NativeType::F32 | NativeType::F64 => sse_params += 1,
        _ => int_params += 1,
      }
    }
    let mut stack_size = (int_params.saturating_sub(Self::INTEGER_REGISTERS)
      + sse_params.saturating_sub(Self::FLOAT_REGISTERS))
      * 8;

    // Section 3.2.2 of the SysV AMD64 ABI:
    // > The end of the input argument area shall be aligned on a 16 (32 or 64, if
    // > __m256 or __m512 is passed on stack) byte boundary. In other words, the value
    // > (%rsp + 8) is always a multiple of 16 (32 or 64) when control is transferred to
    // > the function entry point. The stack pointer, %rsp, always points to the end of the
    // > latest allocated stack frame.
    stack_size += (16 - stack_size % 16) % 16;

    dynasm!(self.assmblr
      ; .arch x64
      ; sub rsp, stack_size as i32
    );
    self.allocated_stack = stack_size;
    // new frame + trampoline caller's return address
    self.offset_trampoline = stack_size + 8;
    // offset is 0 because new frame does not have return address yet
    self.offset_callee = 0;
  }

  fn deallocate_stack(&mut self) {
    dynasm!(self.assmblr
      ; .arch x64
      ; add rsp, self.allocated_stack as i32
    );
  }

  fn call(&mut self, ptr: *const c_void) {
    // the stack has been aligned during stack allocation
    dynasm!(self.assmblr
      ; .arch x64
      ; mov rax, QWORD ptr as _
      ; call rax
    );
  }

  fn tailcall(&mut self, ptr: *const c_void) {
    // stack pointer is never modified and remains aligned
    // return address remains the one provided by the trampoline's caller (V8)
    dynasm!(self.assmblr
      ; .arch x64
      ; mov rax, QWORD ptr as _
      ; jmp rax
    );
  }

  fn ret(&mut self) {
    // the stack has been deallocated before ret is called
    dynasm!(self.assmblr
      ; .arch x64
      ; ret
    );
  }

  fn is_recv_arg_overridden(&self) -> bool {
    // V8 receiver is the first parameter of the trampoline function and is a pointer
    self.integer_params > 0
  }

  fn must_cast_return_value(&self, rv: NativeType) -> bool {
    // V8 only supports i32 and u32 return types for integers
    // We support 8 and 16 bit integers by extending them to 32 bits in the trampoline before returning
    matches!(
      rv,
      NativeType::U8 | NativeType::I8 | NativeType::U16 | NativeType::I16
    )
  }

  fn finalize(self) -> ExecutableBuffer {
    self.assmblr.finalize().unwrap()
  }
}

struct Aarch64Apple {
  // Reference https://github.com/ARM-software/abi-aa/blob/main/aapcs64/aapcs64.rst
  assmblr: dynasmrt::aarch64::Assembler,
  // Parameter counters
  integer_params: u32,
  float_params: u32,
  // Stack offset accumulators
  offset_trampoline: u32,
  offset_callee: u32,
}

#[cfg_attr(
  not(all(target_aarch = "aarch64", target_vendor = "apple")),
  allow(dead_code)
)]
impl Aarch64Apple {
  // Integer arguments go to the first 8 GPR: x0-x7
  const INTEGER_REGISTERS: u32 = 8;
  // Floating-point arguments go to the first 8 SIMD & Floating-Point registers: v0-v1
  const FLOAT_REG: u32 = 8;

  fn new() -> Self {
    Self {
      assmblr: dynasmrt::aarch64::Assembler::new().unwrap(),
      integer_params: 0,
      float_params: 0,
      offset_trampoline: 0,
      offset_callee: 0,
    }
  }

  fn compile(sym: &Symbol) -> Trampoline {
    let mut compiler = Self::new();

    for argument in &sym.parameter_types {
      compiler.move_left(argument)
    }
    if !compiler.is_recv_arg_overridden() {
      // the receiver object should never be expected. Avoid its unexpected or deliberate leak
      compiler.zero_first_arg();
    }
    // In Apple the return value is sign/zero extended to 32 bit by the callee. Therefore, casting
    // is implicit and the trampoline can always tail-call.
    compiler.tailcall(sym.ptr.as_ptr());

    Trampoline(compiler.finalize())
  }

  fn move_left(&mut self, param: &NativeType) {
    // Section 6.4.2 of the Aarch64 Procedure Call Standard (PCS), on argument classification:
    // - INTEGRAL or POINTER:
    //    > If the argument is an Integral or Pointer Type, the size of the argument is less than or equal to 8 bytes
    //    > and the NGRN is less than 8, the argument is copied to the least significant bits in x[NGRN].
    //
    // - Floating-Point or Vector:
    //    > If the argument is a Half-, Single-, Double- or Quad- precision Floating-point or short vector type
    //    > and the NSRN is less than 8, then the argument is allocated to the least significant bits of register v[NSRN]
    match param {
      NativeType::F32 => self.move_float(Single),
      NativeType::F64 => self.move_float(Double),
      NativeType::U8 => self.move_integer(U(B)),
      NativeType::U16 => self.move_integer(U(W)),
      NativeType::U32 | NativeType::Void => self.move_integer(U(DW)),
      NativeType::U64
      | NativeType::USize
      | NativeType::Function
      | NativeType::Pointer => self.move_integer(U(QW)),
      NativeType::I8 => self.move_integer(I(B)),
      NativeType::I16 => self.move_integer(I(W)),
      NativeType::I32 => self.move_integer(I(DW)),
      NativeType::I64 | NativeType::ISize => self.move_integer(I(QW)),
    }
  }

  fn move_float(&mut self, param: Floating) {
    // Section 6.4.2 of the Aarch64 PCS:
    // > If the argument is a Half-, Single-, Double- or Quad- precision Floating-point or short vector type and the NSRN is less than 8, then the
    // > argument is allocated to the least significant bits of register v[NSRN]. The NSRN is incremented by one. The argument has now been allocated.
    // > [if NSRN is equal or more than 8]
    // > The argument is copied to memory at the adjusted NSAA. The NSAA is incremented by the size of the argument. The argument has now been allocated.

    let param_i = self.float_params + 1;
    self.float_params = param_i;

    let is_in_stack = param_i > Self::FLOAT_REG;
    if is_in_stack {
      // https://developer.apple.com/documentation/xcode/writing-arm64-code-for-apple-platforms:
      // > Function arguments may consume slots on the stack that are not multiples of 8 bytes.
      // (i.e. natural alignment instead of eightbyte alignment)
      let padding_trampl =
        (param.size() - self.offset_trampoline % param.size()) % param.size();
      let padding_callee =
        (param.size() - self.offset_callee % param.size()) % param.size();

      // floats are only moved to accommodate integer movement in the stack
      let stack_has_moved = self.integer_params >= Self::INTEGER_REGISTERS;
      if stack_has_moved {
        match param {
          Single => dynasm!(self.assmblr
            ; .arch aarch64
            // 6.1.2 Aarch64 PCS:
            // > Registers v8-v15 must be preserved by a callee across subroutine calls;
            // > the remaining registers (v0-v7, v16-v31) do not need to be preserved (or should be preserved by the caller).
            ; ldr s16, [sp, self.offset_trampoline + padding_trampl]
            ; str s16, [sp, self.offset_callee + padding_callee]
          ),
          Double => dynasm!(self.assmblr
            ; .arch aarch64
            ; ldr d16, [sp, self.offset_trampoline + padding_trampl]
            ; str d16, [sp, self.offset_callee + padding_callee]
          ),
        }
      }
      self.offset_trampoline += padding_trampl + param.size();
      self.offset_callee += padding_callee + param.size();
    }
  }

  fn move_integer(&mut self, param: Integer) {
    // Section 6.4.2 of the Aarch64 PCS:
    // > If the argument is an Integral or Pointer Type, the size of the argument is less than or equal to 8 bytes and the NGRN is less than 8,
    // > the argument is copied to the least significant bits in x[NGRN]. The NGRN is incremented by one. The argument has now been allocated.
    // > [if NGRN is equal or more than 8]
    // > The argument is copied to memory at the adjusted NSAA. The NSAA is incremented by the size of the argument. The argument has now been allocated.

    let param_i = self.integer_params + 1;
    self.integer_params = param_i;

    // move each argument one position to the left. The first argument in the stack moves to the last integer register (x7).
    match (param_i, param) {
      // From https://developer.apple.com/documentation/xcode/writing-arm64-code-for-apple-platforms:
      // > The caller of a function is responsible for signing or zero-extending any argument with fewer than 32 bits.
      // > The standard ABI expects the callee to sign or zero-extend those arguments.
      // (this applies to register parameters, as stack parameters are not eightbyte aligned in Apple)
      (1, I(B)) => dynasm!(self.assmblr; .arch aarch64; sxtb w0, w1),
      (1, U(B)) => dynasm!(self.assmblr; .arch aarch64; and w0, w1, 0xFF),
      (1, I(W)) => dynasm!(self.assmblr; .arch aarch64; sxth w0, w1),
      (1, U(W)) => dynasm!(self.assmblr; .arch aarch64; and w0, w1, 0xFFFF),
      (1, I(DW) | U(DW)) => dynasm!(self.assmblr; .arch aarch64; mov w0, w1),
      (1, I(QW) | U(QW)) => dynasm!(self.assmblr; .arch aarch64; mov x0, x1),

      (2, I(B)) => dynasm!(self.assmblr; .arch aarch64; sxtb w1, w2),
      (2, U(B)) => dynasm!(self.assmblr; .arch aarch64; and w1, w2, 0xFF),
      (2, I(W)) => dynasm!(self.assmblr; .arch aarch64; sxth w1, w2),
      (2, U(W)) => dynasm!(self.assmblr; .arch aarch64; and w1, w2, 0xFFFF),
      (2, I(DW) | U(DW)) => dynasm!(self.assmblr; .arch aarch64; mov w1, w2),
      (2, I(QW) | U(QW)) => dynasm!(self.assmblr; .arch aarch64; mov x1, x2),

      (3, I(B)) => dynasm!(self.assmblr; .arch aarch64; sxtb w2, w3),
      (3, U(B)) => dynasm!(self.assmblr; .arch aarch64; and w2, w3, 0xFF),
      (3, I(W)) => dynasm!(self.assmblr; .arch aarch64; sxth w2, w3),
      (3, U(W)) => dynasm!(self.assmblr; .arch aarch64; and w2, w3, 0xFFFF),
      (3, I(DW) | U(DW)) => dynasm!(self.assmblr; .arch aarch64; mov w2, w3),
      (3, I(QW) | U(QW)) => dynasm!(self.assmblr; .arch aarch64; mov x2, x3),

      (4, I(B)) => dynasm!(self.assmblr; .arch aarch64; sxtb w3, w4),
      (4, U(B)) => dynasm!(self.assmblr; .arch aarch64; and w3, w4, 0xFF),
      (4, I(W)) => dynasm!(self.assmblr; .arch aarch64; sxth w3, w4),
      (4, U(W)) => dynasm!(self.assmblr; .arch aarch64; and w3, w4, 0xFFFF),
      (4, I(DW) | U(DW)) => dynasm!(self.assmblr; .arch aarch64; mov w3, w4),
      (4, I(QW) | U(QW)) => dynasm!(self.assmblr; .arch aarch64; mov x3, x4),

      (5, I(B)) => dynasm!(self.assmblr; .arch aarch64; sxtb w4, w5),
      (5, U(B)) => dynasm!(self.assmblr; .arch aarch64; and w4, w5, 0xFF),
      (5, I(W)) => dynasm!(self.assmblr; .arch aarch64; sxth w4, w5),
      (5, U(W)) => dynasm!(self.assmblr; .arch aarch64; and w4, w5, 0xFFFF),
      (5, I(DW) | U(DW)) => dynasm!(self.assmblr; .arch aarch64; mov w4, w5),
      (5, I(QW) | U(QW)) => dynasm!(self.assmblr; .arch aarch64; mov x4, x5),

      (6, I(B)) => dynasm!(self.assmblr; .arch aarch64; sxtb w5, w6),
      (6, U(B)) => dynasm!(self.assmblr; .arch aarch64; and w5, w6, 0xFF),
      (6, I(W)) => dynasm!(self.assmblr; .arch aarch64; sxth w5, w6),
      (6, U(W)) => dynasm!(self.assmblr; .arch aarch64; and w5, w6, 0xFFFF),
      (6, I(DW) | U(DW)) => dynasm!(self.assmblr; .arch aarch64; mov w5, w6),
      (6, I(QW) | U(QW)) => dynasm!(self.assmblr; .arch aarch64; mov x5, x6),

      (7, I(B)) => dynasm!(self.assmblr; .arch aarch64; sxtb w6, w7),
      (7, U(B)) => dynasm!(self.assmblr; .arch aarch64; and w6, w7, 0xFF),
      (7, I(W)) => dynasm!(self.assmblr; .arch aarch64; sxth w6, w7),
      (7, U(W)) => dynasm!(self.assmblr; .arch aarch64; and w6, w7, 0xFFFF),
      (7, I(DW) | U(DW)) => dynasm!(self.assmblr; .arch aarch64; mov w6, w7),
      (7, I(QW) | U(QW)) => dynasm!(self.assmblr; .arch aarch64; mov x6, x7),

      (8, param) => {
        match param {
          I(B) => {
            dynasm!(self.assmblr; .arch aarch64; ldrsb w7, [sp, self.offset_trampoline])
          }
          U(B) => {
            dynasm!(self.assmblr; .arch aarch64; ldrb w7, [sp, self.offset_trampoline])
          }
          I(W) => {
            dynasm!(self.assmblr; .arch aarch64; ldrsh w7, [sp, self.offset_trampoline])
          }
          U(W) => {
            dynasm!(self.assmblr; .arch aarch64; ldrh w7, [sp, self.offset_trampoline])
          }
          I(DW) | U(DW) => {
            dynasm!(self.assmblr; .arch aarch64; ldr w7, [sp, self.offset_trampoline])
          }
          I(QW) | U(QW) => {
            dynasm!(self.assmblr; .arch aarch64; ldr x7, [sp, self.offset_trampoline])
          }
        }
        // 16 and 8 bit integers are 32 bit integers in v8
        self.offset_trampoline += max(param.size(), 4);
      }

      (_, param) => {
        let size_original = param.size();
        // 16 and 8 bit integers are 32 bit integers in v8
        let size_trampl = max(size_original, 4);
        // https://developer.apple.com/documentation/xcode/writing-arm64-code-for-apple-platforms:
        // > Function arguments may consume slots on the stack that are not multiples of 8 bytes.
        // (i.e. natural alignment instead of eightbyte alignment)
        //
        // TODO: V8 does not currently follow this Apple's policy, and instead aligns all arguments to 8 Byte boundaries.
        // A decision needs to be taken:
        // 1. leave it broken and wait for v8 to fix the bug
        // 2. Adapt to v8 bug and follow its ABI instead of Apple's. When V8 fixes the implementation, we'll have to fix it here as well
        let padding_trampl =
          (size_trampl - self.offset_trampoline % size_trampl) % size_trampl;
        let padding_callee =
          (size_original - self.offset_callee % size_original) % size_original;
        match param {
          I(B) | U(B) => dynasm!(self.assmblr
            ; .arch aarch64
            ; ldr w8, [sp, self.offset_trampoline + padding_trampl]
            ; strb w8, [sp, self.offset_callee + padding_callee]
          ),
          I(W) | U(W) => dynasm!(self.assmblr
            ; .arch aarch64
            ; ldr w8, [sp, self.offset_trampoline + padding_trampl]
            ; strh w8, [sp, self.offset_callee + padding_callee]
          ),
          I(DW) | U(DW) => dynasm!(self.assmblr
            ; .arch aarch64
            ;  ldr w8, [sp, self.offset_trampoline + padding_trampl]
            ; str w8, [sp, self.offset_callee + padding_callee]
          ),
          I(QW) | U(QW) => dynasm!(self.assmblr
            ; .arch aarch64
            ; ldr x8, [sp, self.offset_trampoline + padding_trampl]
            ; str x8, [sp, self.offset_callee + padding_callee]
          ),
        }
        self.offset_trampoline += padding_trampl + size_trampl;
        self.offset_callee += padding_callee + size_original;
      }
    };
  }

  fn zero_first_arg(&mut self) {
    dynasm!(self.assmblr
      ; .arch aarch64
      ; mov x0, xzr
    );
  }

  fn tailcall(&mut self, ptr: *const c_void) {
    // stack pointer is never modified and remains aligned
    // frame pointer remains the one provided by the trampoline's caller (V8)

    // Like all ARM instructions, move instructions are 32bit long and can fit at most 16bit immediates.
    // bigger immediates are loaded in multiple steps applying a left-shift modifier
    let mut address = ptr as u64;
    let mut imm16 = address & 0xFFFF;
    dynasm!(self.assmblr
      ; .arch aarch64
      ; movz x8, imm16 as u32
    );
    address >>= 16;
    let mut shift = 16;
    while address > 0 {
      imm16 = address & 0xFFFF;
      dynasm!(self.assmblr
        ; .arch aarch64
        ; movk x8, imm16 as u32, lsl shift
      );
      address >>= 16;
      shift += 16;
    }
    dynasm!(self.assmblr
        ; .arch aarch64
        ; br x8
    );
  }

  fn is_recv_arg_overridden(&self) -> bool {
    // V8 receiver is the first parameter of the trampoline function and is a pointer
    self.integer_params > 0
  }

  fn finalize(self) -> ExecutableBuffer {
    self.assmblr.finalize().unwrap()
  }
}

struct Win64 {
  // Reference: https://github.com/MicrosoftDocs/cpp-docs/blob/main/docs/build/x64-calling-convention.md
  assmblr: dynasmrt::x64::Assembler,
  // Params counter (Windows does not distinguish by type with regards to parameter position)
  params: u32,
  // Stack offset accumulators
  offset_trampoline: u32,
  offset_callee: u32,
  allocated_stack: u32,
}

#[cfg_attr(
  not(all(target_aarch = "x86_64", target_family = "windows")),
  allow(dead_code)
)]
impl Win64 {
  // Section "Parameter Passing" of the Windows x64 calling convention:
  // > By default, the x64 calling convention passes the first four arguments to a function in registers.
  const REGISTERS: u32 = 4;

  fn new() -> Self {
    Self {
      assmblr: dynasmrt::x64::Assembler::new().unwrap(),
      params: 0,
      // trampoline caller's return address + trampoline's shadow space
      offset_trampoline: 8 + 32,
      offset_callee: 8 + 32,
      allocated_stack: 0,
    }
  }

  fn compile(sym: &Symbol) -> Trampoline {
    let mut compiler = Self::new();

    let can_tailcall = !compiler.must_cast_return_value(sym.result_type);
    if !can_tailcall {
      compiler.allocate_stack(&sym.parameter_types);
    }

    for argument in &sym.parameter_types {
      compiler.move_left(argument)
    }
    if !compiler.is_recv_arg_overridden() {
      // the receiver object should never be expected. Avoid its unexpected or deliberate leak
      compiler.zero_first_arg();
    }

    if !can_tailcall {
      compiler.call(sym.ptr.as_ptr());
      if compiler.must_cast_return_value(sym.result_type) {
        compiler.cast_return_value(sym.result_type);
      }
      compiler.deallocate_stack();
      compiler.ret();
    } else {
      compiler.tailcall(sym.ptr.as_ptr());
    }

    Trampoline(compiler.finalize())
  }

  fn move_left(&mut self, arg: &NativeType) {
    match arg {
      NativeType::F32 | NativeType::F64 => self.move_arg(Float),
      NativeType::U8 | NativeType::I8 => self.move_arg(Int(B)),
      NativeType::U16 | NativeType::I16 => self.move_arg(Int(W)),
      NativeType::U32 | NativeType::I32 | NativeType::Void => {
        self.move_arg(Int(DW))
      }
      NativeType::U64
      | NativeType::USize
      | NativeType::Function
      | NativeType::Pointer
      | NativeType::I64
      | NativeType::ISize => self.move_arg(Int(QW)),
    }
  }

  fn move_arg(&mut self, param: Param) {
    // Section "Parameter Passing" of the Windows x64 calling convention:
    // > By default, the x64 calling convention passes the first four arguments to a function in registers.
    // > The registers used for these arguments depend on the position and type of the argument.
    // > Remaining arguments get pushed on the stack in right-to-left order.
    // > [...]
    // > Integer valued arguments in the leftmost four positions are passed in left-to-right order in RCX, RDX, R8, and R9
    // > [...]
    // > Any floating-point and double-precision arguments in the first four parameters are passed in XMM0 - XMM3, depending on position
    let param_i = self.params + 1;
    self.params = param_i;

    // move each argument one position to the left. The first argument in the stack moves to the last register (r9 or xmm3).
    // If the FFI function is called with a new stack frame, the arguments remaining in the stack are copied to the new stack frame.
    // Otherwise, they are copied 8 bytes lower in the same frame
    match (param_i, param) {
      // Section "Parameter Passing" of the Windows x64 calling convention:
      // > All integer arguments in registers are right-justified, so the callee can ignore the upper bits of the register
      // > and access only the portion of the register necessary.
      // (i.e. unlike in SysV or Aarch64-Apple, 8/16 bit integers are not expected to be zero/sign extended)
      (1, Int(B | W | DW)) => dynasm!(self.assmblr; .arch x64; mov ecx, edx),
      (1, Int(QW)) => dynasm!(self.assmblr; .arch x64; mov rcx, rdx),
      // Use movaps for singles and doubles, benefits of smaller encoding outweigh those of using the correct instruction for the type,
      // which for doubles should technically be movapd
      (1, Float) => {
        dynasm!(self.assmblr; .arch x64; movaps xmm0, xmm1);
        self.zero_first_arg();
      }

      (2, Int(B | W | DW)) => dynasm!(self.assmblr; .arch x64; mov edx, r8d),
      (2, Int(QW)) => dynasm!(self.assmblr; .arch x64; mov rdx, r8),
      (2, Float) => dynasm!(self.assmblr; .arch x64; movaps xmm1, xmm2),

      (3, Int(B | W | DW)) => dynasm!(self.assmblr; .arch x64; mov r8d, r9d),
      (3, Int(QW)) => dynasm!(self.assmblr; .arch x64; mov r8, r9),
      (3, Float) => dynasm!(self.assmblr; .arch x64; movaps xmm2, xmm3),

      (4, param) => {
        match param {
          Int(B | W | DW) => {
            dynasm!(self.assmblr; .arch x64; mov r9d, [rsp + self.offset_trampoline as i32])
          }
          Int(QW) => {
            dynasm!(self.assmblr; .arch x64; mov r9, [rsp + self.offset_trampoline as i32])
          }
          Float => {
            // parameter 4 is always 16-byte aligned, so we can use movaps instead of movups
            dynasm!(self.assmblr; .arch x64; movaps xmm3, [rsp + self.offset_trampoline as i32])
          }
        }
        // Section "x64 Aggregate and Union layout" of the windows x64 software conventions doc:
        // > The alignment of the beginning of a structure or a union is the maximum alignment of any individual member
        // Ref: https://github.com/MicrosoftDocs/cpp-docs/blob/main/docs/build/x64-software-conventions.md#x64-aggregate-and-union-layout
        self.offset_trampoline += 8;
      }
      (_, param) => {
        match param {
          Int(B | W | DW) => {
            dynasm!(self.assmblr
              ; .arch x64
              ; mov eax, [rsp + self.offset_trampoline as i32]
              ; mov [rsp + self.offset_callee as i32], eax
            )
          }
          Int(QW) => {
            dynasm!(self.assmblr
              ; .arch x64
              ; mov rax, [rsp + self.offset_trampoline as i32]
              ; mov [rsp + self.offset_callee as i32], rax
            )
          }
          Float => {
            dynasm!(self.assmblr
              ; .arch x64
              ; movups xmm4, [rsp + self.offset_trampoline as i32]
              ; movups [rsp + self.offset_callee as i32], xmm4
            )
          }
        }
        // Section "x64 Aggregate and Union layout" of the windows x64 software conventions doc:
        // > The alignment of the beginning of a structure or a union is the maximum alignment of any individual member
        // Ref: https://github.com/MicrosoftDocs/cpp-docs/blob/main/docs/build/x64-software-conventions.md#x64-aggregate-and-union-layout
        self.offset_trampoline += 8;
        self.offset_callee += 8;

        debug_assert!(
          self.allocated_stack == 0
            || self.offset_callee <= self.allocated_stack
        );
      }
    }
  }

  fn zero_first_arg(&mut self) {
    dynasm!(self.assmblr
      ; .arch x64
      ; xor ecx, ecx
    );
  }

  fn cast_return_value(&mut self, rv: NativeType) {
    // V8 only supports 32bit integers. We support 8 and 16 bit integers casting them to 32bits.
    // Section "Return Values" of the Windows x64 Calling Convention doc:
    // > The state of unused bits in the value returned in RAX or XMM0 is undefined.
    match rv {
      NativeType::U8 => dynasm!(self.assmblr; .arch x64; movzx eax, al),
      NativeType::I8 => dynasm!(self.assmblr; .arch x64; movsx eax, al),
      NativeType::U16 => dynasm!(self.assmblr; .arch x64; movzx eax, ax),
      NativeType::I16 => dynasm!(self.assmblr; .arch x64; movsx eax, ax),
      _ => (),
    }
  }

  fn allocate_stack(&mut self, params: &[NativeType]) {
    let mut stack_size = 0;
    // Section "Calling Convetion Defaults" of the x64-calling-convention and Section "Stack Allocation" of the stack-usage docs:
    // > The x64 Application Binary Interface (ABI) uses a four-register fast-call calling convention by default.
    // > Space is allocated on the call stack as a shadow store for callees to save those registers.
    // > [...]
    // > Any parameters beyond the first four must be stored on the stack after the shadow store before the call
    // > [...]
    // > Even if the called function has fewer than 4 parameters, these 4 stack locations are effectively owned by the called function,
    // > and may be used by the called function for other purposes besides saving parameter register values
    stack_size += max(params.len() as u32, 4) * 8;

    // Align stack (including the 8 bytes of the trampoline caller's return address)
    // Section "Stack Allocation" of stack-usage docs:
    // > The stack will always be maintained 16-byte aligned, except within the prolog (for example, after the return address is pushed)
    stack_size += (16 - (stack_size + 8) % 16) % 16;

    dynasm!(self.assmblr
      ; .arch x64
      ; sub rsp, stack_size as i32
    );
    self.allocated_stack = stack_size;
    // New stack frame + trampoline caller's return address + trampoline's shadow space
    self.offset_trampoline = stack_size + 8 + 32;
    // callee's shadow space
    self.offset_callee = 32;
  }

  fn deallocate_stack(&mut self) {
    dynasm!(self.assmblr
      ; .arch x64
      ; add rsp, self.allocated_stack as i32
    );
  }

  fn call(&mut self, ptr: *const c_void) {
    // the stack has been aligned during stack allocation
    dynasm!(self.assmblr
      ; .arch x64
      ; mov rax, QWORD ptr as _
      ; call rax
    );
  }

  fn tailcall(&mut self, ptr: *const c_void) {
    // stack pointer is never modified and remains aligned
    // return address remains the one provided by the trampoline's caller (V8)
    dynasm!(self.assmblr
      ; .arch x64
      ; mov rax, QWORD ptr as _
      ; jmp rax
    );
  }

  fn ret(&mut self) {
    // the stack has been deallocated before ret is called
    dynasm!(self.assmblr
      ; .arch x64
      ; ret
    );
  }

  fn is_recv_arg_overridden(&self) -> bool {
    self.params > 0
  }

  fn must_cast_return_value(&self, rv: NativeType) -> bool {
    // V8 only supports i32 and u32 return types for integers
    // We support 8 and 16 bit integers by extending them to 32 bits in the trampoline before returning
    matches!(
      rv,
      NativeType::U8 | NativeType::I8 | NativeType::U16 | NativeType::I16
    )
  }

  fn finalize(self) -> ExecutableBuffer {
    self.assmblr.finalize().unwrap()
  }
}

#[derive(Clone, Copy, Debug)]
enum Floating {
  Single = 4,
  Double = 8,
}

impl Floating {
  fn size(self) -> u32 {
    self as u32
  }
}

use Floating::*;

#[derive(Clone, Copy, Debug)]
enum Integer {
  I(Size),
  U(Size),
}

impl Integer {
  fn size(self) -> u32 {
    match self {
      I(size) | U(size) => size as u32,
    }
  }
}

use Integer::*;

#[derive(Clone, Copy, Debug)]
enum Size {
  B = 1,
  W = 2,
  DW = 4,
  QW = 8,
}
use Size::*;

#[derive(Clone, Copy, Debug)]
enum Param {
  Int(Size),
  Float,
}

use Param::*;

// TODO: on ice. Decide what todo with this
// trait Abi {
//   fn compile(sym: &Symbol) -> Trampoline
//   where
//     Self: Default,
//   {
//     let mut compiler = Self::new(sym);

//     if !compiler.can_tailcall() {
//       compiler.allocate_stack(&sym.parameter_types);
//     }

//     for argument in &sym.parameter_types {
//       compiler.move_left(argument)
//     }
//     if !compiler.is_recv_overridden() {
//       // the receiver object should never be expected. Avoid its unexpected or deliberate leak
//       compiler.zero_first_arg();
//     }

//     if !compiler.can_tailcall() {
//       compiler.call(sym.ptr.as_ptr());
//       if compiler.must_cast_return_value(sym.result_type) {
//         compiler.cast_return_value(sym.result_type);
//       }
//       compiler.deallocate_stack();
//       compiler.ret();
//     } else {
//       compiler.tailcall(sym.ptr.as_ptr());
//     }

//     Trampoline(compiler.finalize())
//   }

//   fn move_left(&mut self, param: &NativeType) {
//     match param {
//       NativeType::F32 => self.process_float(Single),
//       NativeType::F64 => self.process_float(Double),
//       NativeType::U8 => self.move_integer(Unsigned(B)),
//       NativeType::U16 => self.move_integer(Unsigned(W)),
//       NativeType::U32 | NativeType::Void => self.move_integer(Unsigned(DW)),
//       NativeType::U64
//       | NativeType::USize
//       | NativeType::Function
//       | NativeType::Pointer => self.move_integer(Unsigned(QW)),
//       NativeType::I8 => self.move_integer(Signed(B)),
//       NativeType::I16 => self.move_integer(Signed(W)),
//       NativeType::I32 => self.move_integer(Signed(DW)),
//       NativeType::I64 | NativeType::ISize => self.move_integer(Signed(QW)),
//     }
//   }

//   fn new(sym: &Symbol) -> Self;

//   fn process_float(&mut self, param: Float);

//   fn move_integer(&mut self, arg: Integer);

//   fn zero_first_arg(&mut self);

//   fn cast_return_value(&mut self, rv: NativeType);

//   fn allocate_stack(&mut self, params: &[NativeType]);

//   fn deallocate_stack(&mut self);

//   fn call(&mut self, ptr: *const c_void);

//   fn tailcall(&mut self, ptr: *const c_void);

//   fn ret(&mut self);

//   fn can_tailcall(&self) -> bool;

//   fn is_recv_overridden(&self) -> bool;

//   fn must_cast_return_value(&self, rv: NativeType) -> bool;

//   fn finalize(self) -> ExecutableBuffer;
// }

#[cfg(test)]
mod tests {
  // TODO: add more test cases
  use std::ptr::null_mut;

  use libffi::middle::Type;

  use crate::NativeType;
  use crate::Symbol;

  fn symbol(parameters: Vec<NativeType>, ret: NativeType) -> Symbol {
    Symbol {
      cif: libffi::middle::Cif::new(vec![], Type::void()),
      ptr: libffi::middle::CodePtr(null_mut()),
      parameter_types: parameters,
      result_type: ret,
      can_callback: false,
    }
  }

  mod sysv_amd64 {
    use std::ops::Deref;

    use dynasmrt::dynasm;
    use dynasmrt::DynasmApi;

    use super::super::SysVAmd64;
    use super::symbol;
    use crate::NativeType::*;

    #[test]
    fn tailcall() {
      let trampoline = SysVAmd64::compile(&symbol(
        vec![
          U8, U16, I16, I8, U32, U64, Pointer, Function, I64, I32, I16, I8,
          F32, F32, F32, F32, F64, F64, F64, F64, F32, F64,
        ],
        Void,
      ));

      let mut assembler = dynasmrt::x64::Assembler::new().unwrap();
      // See https://godbolt.org/z/Gr1Mcbch5
      dynasm!(assembler
        ; .arch x64
        ; movzx edi, sil     // u8
        ; movzx esi, dx      // u16
        ; movsx edx, cx      // i16
        ; movsx ecx, r8b     // i8
        ; mov r8d, r9d       // u32
        ; mov r9, [DWORD rsp + 8]   // u64
        ; mov rax, [DWORD rsp + 16] // Pointer
        ; mov [DWORD rsp + 8], rax // ..
        ; mov rax, [DWORD rsp + 24] // Function
        ; mov [DWORD rsp + 16], rax // ..
        ; mov rax, [DWORD rsp + 32]    // i64
        ; mov [DWORD rsp + 24], rax // ..
        ; mov eax, [DWORD rsp + 40]   // i32
        ; mov [DWORD rsp + 32], eax    // ..
        ; movsx eax, WORD [DWORD rsp + 48]   // i16
        ; mov [DWORD rsp + 40], eax   // ..
        ; movsx eax, BYTE [DWORD rsp + 56]   // i8
        ; mov [DWORD rsp + 48], eax   // ..
        ; movss xmm8, [DWORD rsp + 64]  // f32
        ; movss [DWORD rsp + 56], xmm8  // ..
        ; movsd xmm8, [DWORD rsp + 72]  // f64
        ; movsd [DWORD rsp + 64], xmm8  // ..
        ; mov rax, QWORD 0
        ; jmp rax
      );
      let expected = assembler.finalize().unwrap();
      assert_eq!(trampoline.0.deref(), expected.deref());
    }
  }
  mod aarch64_apple {
    use std::ops::Deref;

    use dynasmrt::dynasm;

    use super::super::Aarch64Apple;
    use super::symbol;
    use crate::NativeType::*;

    #[test]
    fn tailcall() {
      let trampoline = Aarch64Apple::compile(&symbol(
        vec![
          U8, U16, I16, I8, U32, U64, Pointer, Function, I64, I32, I16, I8,
          F32, F32, F32, F32, F64, F64, F64, F64, F32, F64,
        ],
        Void,
      ));

      let mut assembler = dynasmrt::aarch64::Assembler::new().unwrap();
      // See https://godbolt.org/z/Gr1Mcbch5
      dynasm!(assembler
        ; .arch aarch64
        ; and w0, w1, 0xFF   // u8
        ; and w1, w2, 0xFFFF // u16
        ; sxth w2, w3        // i16
        ; sxtb w3, w4        // i8
        ; mov w4, w5         // u32
        ; mov x5, x6         // u64
        ; mov x6, x7         // Pointer
        ; ldr x7, [sp]       // Function
        ; ldr x8, [sp, 8]    // i64
        ; str x8, [sp]       // ..
        ; ldr w8, [sp, 16]   // i32
        ; str w8, [sp, 8]    // ..
        ; ldr w8, [sp, 20]   // i16
        ; strh w8, [sp, 12]   // ..
        ; ldr w8, [sp, 24]   // i8
        ; strb w8, [sp, 14]   // ..
        ; ldr s16, [sp, 28]  // f32
        ; str s16, [sp, 16]  // ..
        ; ldr d16, [sp, 32]  // f64
        ; str d16, [sp, 24]  // ..
        ; movz x8, 0
        ; br x8
      );
      let expected = assembler.finalize().unwrap();
      assert_eq!(trampoline.0.deref(), expected.deref());
    }
  }

  mod x64_windows {
    use std::ops::Deref;

    use dynasmrt::{dynasm, DynasmApi};

    use super::super::Win64;
    use super::symbol;
    use crate::NativeType::*;

    #[test]
    fn tailcall() {
      let trampoline = Win64::compile(&symbol(
        vec![U8, I16, F64, F32, U32, I8, Pointer],
        Void,
      ));

      let mut assembler = dynasmrt::x64::Assembler::new().unwrap();
      dynasm!(assembler
        ; .arch x64
        ; mov ecx, edx          // u8
        ; mov edx, r8d         // i16
        ; movaps xmm2, xmm3      // f64
        ; movaps xmm3, [DWORD rsp + 40] // f32
        ; mov eax, [DWORD rsp + 48]    // u32
        ; mov [DWORD rsp + 40], eax    // ..
        ; mov eax, [DWORD rsp + 56]    // i8
        ; mov [DWORD rsp + 48], eax     // ..
        ; mov rax, [DWORD rsp + 64]    // Pointer
        ; mov [DWORD rsp + 56], rax    // ..
        ; mov rax, QWORD 0
        ; jmp rax
      );
      let expected = assembler.finalize().unwrap();
      assert_eq!(trampoline.0.deref(), expected.deref());
    }
  }
}
