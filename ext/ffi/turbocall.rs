// Copyright 2018-2026 the Deno authors. MIT license.

use std::ffi::c_void;
use std::sync::Arc;
use std::sync::LazyLock;

use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::fast_api;

use crate::NativeType;
use crate::Symbol;
use crate::dlfcn::FunctionData;

static TRACE_TURBO: LazyLock<bool> = LazyLock::new(|| {
  std::env::var("DENO_UNSTABLE_FFI_TRACE_TURBO").as_deref() == Ok("1")
});

pub(crate) static CRANELIFT_ISA: LazyLock<
  Result<Arc<dyn cranelift::codegen::isa::TargetIsa>, TurbocallError>,
> = LazyLock::new(|| {
  let mut flag_builder = cranelift::prelude::settings::builder();
  flag_builder.set("is_pic", "true").unwrap();
  flag_builder.set("opt_level", "speed").unwrap();
  let flags = cranelift::prelude::settings::Flags::new(flag_builder);
  let isa = cranelift_native::builder()
    .map_err(TurbocallError::IsaError)?
    .finish(flags)?;
  Ok(isa)
});

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum TurbocallError {
  #[class(generic)]
  #[error(transparent)]
  SetError(#[from] cranelift::prelude::settings::SetError),

  #[class(generic)]
  #[error("Cranelift ISA error: {0}")]
  IsaError(&'static str),

  #[class(generic)]
  #[error(transparent)]
  CodegenError(#[from] cranelift::codegen::CodegenError),

  #[cfg(debug_assertions)]
  #[class(generic)]
  #[error(transparent)]
  VerifierError(#[from] cranelift::codegen::verifier::VerifierErrors),

  #[class(generic)]
  #[error("{0}")]
  CompileError(String),

  #[class(generic)]
  #[error(transparent)]
  Stdio(#[from] std::io::Error),
}

pub(crate) fn is_compatible(sym: &Symbol) -> bool {
  !matches!(sym.result_type, NativeType::Struct(_))
    && !sym
      .parameter_types
      .iter()
      .any(|t| matches!(t, NativeType::Struct(_)))
}

/// Trampoline for fast-call FFI functions
///
/// Calls the FFI function without the first argument (the receiver)
pub(crate) struct Trampoline(memmap2::Mmap);

impl Trampoline {
  pub(crate) fn ptr(&self) -> *const c_void {
    self.0.as_ptr() as *const c_void
  }
}

// Hand-emitted trivial trampolines.
//
// For signatures where all parameters and
// the result type need no conversion
#[cfg(any(
  target_arch = "aarch64",
  all(target_arch = "x86_64", not(target_os = "windows"))
))]
mod trivial_trampoline {
  use super::*;

  fn is_trivial_param(t: &NativeType) -> bool {
    matches!(
      t,
      NativeType::U32
        | NativeType::I32
        | NativeType::U64
        | NativeType::I64
        | NativeType::USize
        | NativeType::ISize
        | NativeType::F32
        | NativeType::F64
        | NativeType::Pointer
        | NativeType::Function
    )
  }
  fn is_trivial_result(t: &NativeType) -> bool {
    matches!(
      t,
      NativeType::Void
        | NativeType::U32
        | NativeType::I32
        | NativeType::U64
        | NativeType::I64
        | NativeType::USize
        | NativeType::ISize
        | NativeType::F32
        | NativeType::F64
        | NativeType::Pointer
        | NativeType::Function
    )
  }
  pub fn is_trivial(sym: &Symbol) -> bool {
    sym.parameter_types.iter().all(is_trivial_param)
      && is_trivial_result(&sym.result_type)
  }

  pub fn compile(sym: &Symbol) -> Result<Trampoline, TurbocallError> {
    let n_int = sym
      .parameter_types
      .iter()
      .filter(|t| !matches!(t, NativeType::F32 | NativeType::F64))
      .count();
    let n_float = sym
      .parameter_types
      .iter()
      .filter(|t| matches!(t, NativeType::F32 | NativeType::F64))
      .count();

    // Max integer params that fit in registers after removing the
    // receiver (float regs are unaffected)
    #[cfg(target_arch = "aarch64")]
    const MAX_INT: usize = 7; // x0..x7 - receiver
    #[cfg(all(target_arch = "x86_64", not(target_os = "windows")))]
    const MAX_INT: usize = 5; // rdi..r9 - receiver
    const MAX_FLOAT: usize = 8;

    if n_int > MAX_INT {
      return Err(TurbocallError::CompileError(
        "too many integer parameters for trivial trampoline".into(),
      ));
    }
    if n_float > MAX_FLOAT {
      return Err(TurbocallError::CompileError(
        "too many float parameters for trivial trampoline".into(),
      ));
    }

    let target_addr = sym.ptr.as_ptr() as usize;
    let code = emit_code(n_int, target_addr);

    let mut mmap = memmap2::MmapMut::map_anon(code.len())?;
    mmap.copy_from_slice(&code);
    let exec = mmap.make_exec()?;
    Ok(Trampoline(exec))
  }

  #[cfg(target_arch = "aarch64")]
  fn emit_code(n_int_params: usize, target_addr: usize) -> Vec<u8> {
    let mut code = Vec::with_capacity(n_int_params * 4 + 16);

    // Shift integer registers
    for i in 0..n_int_params {
      let rd = i as u32;
      let rm = (i + 1) as u32;
      // mov Xd, Xm
      let insn: u32 = 0xAA00_03E0 | (rm << 16) | rd;
      code.extend_from_slice(&insn.to_le_bytes());
    }

    // ldr X16, +8
    code.extend_from_slice(&0x5800_0050_u32.to_le_bytes());
    // br X16
    code.extend_from_slice(&0xD61F_0200_u32.to_le_bytes());
    code.extend_from_slice(&(target_addr as u64).to_le_bytes());

    code
  }

  #[cfg(all(target_arch = "x86_64", not(target_os = "windows")))]
  fn emit_code(n_int_params: usize, target_addr: usize) -> Vec<u8> {
    const SHIFTS: [[u8; 3]; 5] = [
      [0x48, 0x89, 0xF7], // mov rdi, rsi
      [0x48, 0x89, 0xD6], // mov rsi, rdx
      [0x48, 0x89, 0xCA], // mov rdx, rcx
      [0x4C, 0x89, 0xC1], // mov rcx, r8
      [0x4D, 0x89, 0xC8], // mov r8,  r9
    ];

    let mut code = Vec::with_capacity(n_int_params * 3 + 12);

    for shift in &SHIFTS[..n_int_params] {
      code.extend_from_slice(shift);
    }

    // movabs rax, <target_addr>
    code.push(0x48);
    code.push(0xB8);
    code.extend_from_slice(&(target_addr as u64).to_le_bytes());
    // jmp rax
    code.push(0xFF);
    code.push(0xE0);

    code
  }
}

#[allow(unused)]
pub(crate) fn compile_trampoline(
  sym: &Symbol,
) -> Result<Trampoline, TurbocallError> {
  // Try hand-emitting for trivial signatures
  #[cfg(any(
    target_arch = "aarch64",
    all(target_arch = "x86_64", not(target_os = "windows"))
  ))]
  if !*TRACE_TURBO && trivial_trampoline::is_trivial(sym) {
    match trivial_trampoline::compile(sym) {
      Ok(trampoline) => return Ok(trampoline),
      Err(e) => {
        log::debug!("Trivial trampoline emit failed for '{}': {e}", sym.name);
      }
    }
  }

  let isa = CRANELIFT_ISA
    .as_ref()
    .map_err(|e| TurbocallError::CompileError(format!("{e}")))?;
  let mut ctx = cranelift::codegen::Context::new();
  let mut fn_builder_ctx = cranelift::prelude::FunctionBuilderContext::new();
  compile_cranelift_trampoline(sym, isa, &mut ctx, &mut fn_builder_ctx)
}

pub(crate) type CraneliftState = Option<(
  Arc<dyn cranelift::codegen::isa::TargetIsa>,
  cranelift::codegen::Context,
  cranelift::prelude::FunctionBuilderContext,
)>;

/// Like `compile_trampoline` but accepts a lazily-initialised Cranelift
/// state so callers can amortise allocation across many symbols.
#[allow(unused)]
pub(crate) fn compile_trampoline_reuse(
  sym: &Symbol,
  cl_state: &mut CraneliftState,
) -> Result<Trampoline, TurbocallError> {
  #[cfg(any(
    target_arch = "aarch64",
    all(target_arch = "x86_64", not(target_os = "windows"))
  ))]
  if !*TRACE_TURBO && trivial_trampoline::is_trivial(sym) {
    match trivial_trampoline::compile(sym) {
      Ok(trampoline) => return Ok(trampoline),
      Err(e) => {
        log::debug!("Trivial trampoline emit failed for '{}': {e}", sym.name);
      }
    }
  }

  if cl_state.is_none() {
    let isa = CRANELIFT_ISA
      .as_ref()
      .map_err(|e| TurbocallError::CompileError(format!("{e}")))?
      .clone();
    *cl_state = Some((
      isa,
      cranelift::codegen::Context::new(),
      cranelift::prelude::FunctionBuilderContext::new(),
    ));
  }
  let (isa, ctx, fn_builder_ctx) = cl_state.as_mut().unwrap();
  compile_cranelift_trampoline(sym, isa, ctx, fn_builder_ctx)
}

use cranelift::prelude::*;

#[cfg(target_pointer_width = "32")]
const ISIZE: Type = types::I32;
#[cfg(target_pointer_width = "64")]
const ISIZE: Type = types::I64;

fn convert(t: &NativeType, wrapper: bool) -> AbiParam {
  match t {
    NativeType::U8 => {
      if wrapper {
        AbiParam::new(types::I32)
      } else {
        AbiParam::new(types::I8).uext()
      }
    }
    NativeType::I8 => {
      if wrapper {
        AbiParam::new(types::I32)
      } else {
        AbiParam::new(types::I8).sext()
      }
    }
    NativeType::U16 => {
      if wrapper {
        AbiParam::new(types::I32)
      } else {
        AbiParam::new(types::I16).uext()
      }
    }
    NativeType::I16 => {
      if wrapper {
        AbiParam::new(types::I32)
      } else {
        AbiParam::new(types::I16).sext()
      }
    }
    NativeType::Bool => {
      if wrapper {
        AbiParam::new(types::I32)
      } else {
        AbiParam::new(types::I8).uext()
      }
    }
    NativeType::U32 => AbiParam::new(types::I32),
    NativeType::I32 => AbiParam::new(types::I32),
    NativeType::U64 => AbiParam::new(types::I64),
    NativeType::I64 => AbiParam::new(types::I64),
    NativeType::USize => AbiParam::new(ISIZE),
    NativeType::ISize => AbiParam::new(ISIZE),
    NativeType::F32 => AbiParam::new(types::F32),
    NativeType::F64 => AbiParam::new(types::F64),
    NativeType::Pointer => AbiParam::new(ISIZE),
    NativeType::Buffer => AbiParam::new(ISIZE),
    NativeType::Function => AbiParam::new(ISIZE),
    NativeType::Struct(_) => AbiParam::new(types::INVALID),
    NativeType::Void => AbiParam::new(types::INVALID),
  }
}

/// Compile a Cranelift trampoline with reusable contexts, returning a
/// ready-to-execute `Trampoline`.
pub(crate) fn compile_cranelift_trampoline(
  sym: &Symbol,
  isa: &Arc<dyn cranelift::codegen::isa::TargetIsa>,
  ctx: &mut cranelift::codegen::Context,
  fn_builder_ctx: &mut FunctionBuilderContext,
) -> Result<Trampoline, TurbocallError> {
  let data = compile_cranelift_trampoline_bytes(sym, isa, ctx, fn_builder_ctx)?;
  let mut mutable = memmap2::MmapMut::map_anon(data.len())?;
  mutable.copy_from_slice(&data);
  let buffer = mutable.make_exec()?;
  Ok(Trampoline(buffer))
}

fn compile_cranelift_trampoline_bytes(
  sym: &Symbol,
  isa: &Arc<dyn cranelift::codegen::isa::TargetIsa>,
  ctx: &mut cranelift::codegen::Context,
  fn_builder_ctx: &mut FunctionBuilderContext,
) -> Result<Vec<u8>, TurbocallError> {
  let mut wrapper_sig =
    cranelift::codegen::ir::Signature::new(isa.default_call_conv());
  let mut target_sig =
    cranelift::codegen::ir::Signature::new(isa.default_call_conv());
  let mut raise_sig =
    cranelift::codegen::ir::Signature::new(isa.default_call_conv());

  // *const FastApiCallbackOptions
  raise_sig.params.push(AbiParam::new(ISIZE));

  // Local<Value> receiver
  wrapper_sig.params.push(AbiParam::new(ISIZE));

  for pty in &sym.parameter_types {
    target_sig.params.push(convert(pty, false));
    wrapper_sig.params.push(convert(pty, true));
  }

  // const FastApiCallbackOptions& options
  wrapper_sig.params.push(AbiParam::new(ISIZE));

  if !matches!(sym.result_type, NativeType::Struct(_) | NativeType::Void) {
    target_sig.returns.push(convert(&sym.result_type, false));
    wrapper_sig.returns.push(convert(&sym.result_type, true));
  }

  let mut ab_sig =
    cranelift::codegen::ir::Signature::new(isa.default_call_conv());
  ab_sig.params.push(AbiParam::new(ISIZE));
  ab_sig.returns.push(AbiParam::new(ISIZE));

  ctx.func = cranelift::codegen::ir::Function::with_name_signature(
    cranelift::codegen::ir::UserFuncName::user(0, 0),
    wrapper_sig,
  );

  let mut f = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);

  let target_sig = f.import_signature(target_sig);
  let ab_sig = f.import_signature(ab_sig);
  let raise_sig = f.import_signature(raise_sig);

  {
    // Define blocks

    let entry = f.create_block();
    f.append_block_params_for_function_params(entry);

    let error = f.create_block();
    f.set_cold_block(error);

    // Define variables

    let mut vidx = 0;
    for pt in &sym.parameter_types {
      let target_v = Variable::new(vidx);
      vidx += 1;

      let wrapper_v = Variable::new(vidx);
      vidx += 1;

      f.declare_var(target_v, convert(pt, false).value_type);
      f.declare_var(wrapper_v, convert(pt, true).value_type);
    }

    let options_v = Variable::new(vidx);
    #[allow(unused)]
    {
      vidx += 1;
    }
    f.declare_var(options_v, ISIZE);

    // Go!

    f.switch_to_block(entry);
    f.seal_block(entry);

    let args = f.block_params(entry).to_owned();

    let mut vidx = 1;
    let mut argx = 1;
    for _ in &sym.parameter_types {
      f.def_var(Variable::new(vidx), args[argx]);
      argx += 1;
      vidx += 2;
    }

    f.def_var(options_v, args[argx]);

    if *TRACE_TURBO {
      let options = f.use_var(options_v);
      let trace_fn = f.ins().iconst(ISIZE, turbocall_trace as usize as i64);
      f.ins().call_indirect(ab_sig, trace_fn, &[options]);
    }

    let mut next = f.create_block();

    let mut vidx = 0;
    for nty in &sym.parameter_types {
      let target_v = Variable::new(vidx);
      vidx += 1;
      let wrapper_v = Variable::new(vidx);
      vidx += 1;

      let arg = f.use_var(wrapper_v);

      match nty {
        NativeType::U8 | NativeType::I8 | NativeType::Bool => {
          let v = f.ins().ireduce(types::I8, arg);
          f.def_var(target_v, v);
        }
        NativeType::U16 | NativeType::I16 => {
          let v = f.ins().ireduce(types::I16, arg);
          f.def_var(target_v, v);
        }
        NativeType::Buffer => {
          let callee =
            f.ins().iconst(ISIZE, turbocall_ab_contents as usize as i64);
          let call = f.ins().call_indirect(ab_sig, callee, &[arg]);
          let result = f.inst_results(call)[0];
          f.def_var(target_v, result);

          let sentinel = f.ins().iconst(ISIZE, isize::MAX as i64);
          let condition = f.ins().icmp(IntCC::Equal, result, sentinel);
          f.ins().brif(condition, error, &[], next, &[]);

          // switch to new block
          f.switch_to_block(next);
          f.seal_block(next);
          next = f.create_block();
        }
        _ => {
          f.def_var(target_v, arg);
        }
      }
    }

    let mut args = Vec::with_capacity(sym.parameter_types.len());
    let mut vidx = 0;
    for _ in &sym.parameter_types {
      args.push(f.use_var(Variable::new(vidx)));
      vidx += 2; // skip wrapper arg
    }
    let callee = f.ins().iconst(ISIZE, sym.ptr.as_ptr() as i64);
    let call = f.ins().call_indirect(target_sig, callee, &args);
    let mut results = f.inst_results(call).to_owned();

    match sym.result_type {
      NativeType::U8 | NativeType::U16 | NativeType::Bool => {
        results[0] = f.ins().uextend(types::I32, results[0]);
      }
      NativeType::I8 | NativeType::I16 => {
        results[0] = f.ins().sextend(types::I32, results[0]);
      }
      _ => {}
    }

    f.ins().return_(&results);

    f.switch_to_block(error);
    f.seal_block(error);
    if !f.is_unreachable() {
      let options = f.use_var(options_v);
      let callee = f.ins().iconst(ISIZE, turbocall_raise as usize as i64);
      f.ins().call_indirect(raise_sig, callee, &[options]);
      let rty = convert(&sym.result_type, true);
      if rty.value_type.is_invalid() {
        f.ins().return_(&[]);
      } else {
        let zero = if rty.value_type == types::F32 {
          f.ins().f32const(0.0)
        } else if rty.value_type == types::F64 {
          f.ins().f64const(0.0)
        } else {
          f.ins().iconst(rty.value_type, 0)
        };
        f.ins().return_(&[zero]);
      }
    }
  }

  f.finalize();

  #[cfg(debug_assertions)]
  cranelift::codegen::verifier::verify_function(&ctx.func, isa.flags())?;

  let mut ctrl_plane = Default::default();
  ctx.optimize(&**isa, &mut ctrl_plane)?;

  log::trace!("Turbocall IR:\n{}", ctx.func.display());

  let code_info = ctx
    .compile(&**isa, &mut ctrl_plane)
    .map_err(|e| TurbocallError::CompileError(format!("{e:?}")))?;

  let bytes = code_info.buffer.data().to_vec();
  ctx.clear();
  Ok(bytes)
}

pub(crate) struct Turbocall {
  pub trampoline: Trampoline,
  // Held in a box to keep the memory alive for CFunctionInfo
  #[allow(unused)]
  pub param_info: Box<[fast_api::CTypeInfo]>,
  // Held in a box to keep the memory alive for V8
  #[allow(unused)]
  pub c_function_info: Box<fast_api::CFunctionInfo>,
}

pub(crate) fn make_template(sym: &Symbol, trampoline: Trampoline) -> Turbocall {
  let param_info = std::iter::once(fast_api::Type::V8Value.as_info()) // Receiver
    .chain(sym.parameter_types.iter().map(|t| t.into()))
    .chain(std::iter::once(fast_api::Type::CallbackOptions.as_info()))
    .collect::<Box<_>>();

  let ret = if sym.result_type == NativeType::Buffer {
    // Buffer can be used as a return type and converts differently than in parameters.
    fast_api::Type::Pointer.as_info()
  } else {
    (&sym.result_type).into()
  };

  let c_function_info = Box::new(fast_api::CFunctionInfo::new(
    ret,
    &param_info,
    fast_api::Int64Representation::BigInt,
  ));

  Turbocall {
    trampoline,
    param_info,
    c_function_info,
  }
}

impl From<&NativeType> for fast_api::CTypeInfo {
  fn from(native_type: &NativeType) -> Self {
    match native_type {
      NativeType::Bool => fast_api::Type::Bool.as_info(),
      NativeType::U8 | NativeType::U16 | NativeType::U32 => {
        fast_api::Type::Uint32.as_info()
      }
      NativeType::I8 | NativeType::I16 | NativeType::I32 => {
        fast_api::Type::Int32.as_info()
      }
      NativeType::F32 => fast_api::Type::Float32.as_info(),
      NativeType::F64 => fast_api::Type::Float64.as_info(),
      NativeType::Void => fast_api::Type::Void.as_info(),
      NativeType::I64 => fast_api::Type::Int64.as_info(),
      NativeType::U64 => fast_api::Type::Uint64.as_info(),
      NativeType::ISize => fast_api::Type::Int64.as_info(),
      NativeType::USize => fast_api::Type::Uint64.as_info(),
      NativeType::Pointer | NativeType::Function => {
        fast_api::Type::Pointer.as_info()
      }
      NativeType::Buffer => fast_api::Type::V8Value.as_info(),
      NativeType::Struct(_) => fast_api::Type::V8Value.as_info(),
    }
  }
}

extern "C" fn turbocall_ab_contents(
  v: deno_core::v8::Local<deno_core::v8::Value>,
) -> *mut c_void {
  super::ir::parse_buffer_arg(v).unwrap_or(isize::MAX as _)
}

unsafe extern "C" fn turbocall_raise(
  options: *const deno_core::v8::fast_api::FastApiCallbackOptions,
) {
  // SAFETY: This is called with valid FastApiCallbackOptions from within fast callback.
  v8::callback_scope!(unsafe scope, unsafe { &*options });
  let exception =
    deno_core::error::to_v8_error(scope, &crate::IRError::InvalidBufferType);
  scope.throw_exception(exception);
}

pub struct TurbocallTarget(String);

unsafe extern "C" fn turbocall_trace(
  options: *const deno_core::v8::fast_api::FastApiCallbackOptions,
) {
  // SAFETY: This is called with valid FastApiCallbackOptions from within fast callback.
  v8::callback_scope!(unsafe let scope, unsafe { &*options });
  let func_data = deno_core::cppgc::try_unwrap_cppgc_object::<FunctionData>(
    scope,
    // SAFETY: This is valid if the options are valid.
    unsafe { (&*options).data },
  )
  .unwrap();
  deno_core::JsRuntime::op_state_from(scope)
    .borrow_mut()
    .put(TurbocallTarget(func_data.symbol.name.clone()));
}

#[op2]
#[string]
pub fn op_ffi_get_turbocall_target(state: &mut OpState) -> Option<String> {
  state.try_take::<TurbocallTarget>().map(|t| t.0)
}
