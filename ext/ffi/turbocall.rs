// Copyright 2018-2025 the Deno authors. MIT license.

use std::ffi::c_void;

use deno_core::v8::fast_api;

use crate::NativeType;
use crate::Symbol;

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

#[allow(unused)]
pub(crate) fn compile_trampoline(sym: &Symbol) -> Trampoline {
  use cranelift::prelude::*;

  let mut flag_builder = settings::builder();
  flag_builder.set("is_pic", "true").unwrap();
  flag_builder.set("opt_level", "speed_and_size").unwrap();
  let flags = settings::Flags::new(flag_builder);

  let isa = cranelift_native::builder().unwrap().finish(flags).unwrap();

  let mut wrapper_sig =
    cranelift::codegen::ir::Signature::new(isa.default_call_conv());
  let mut target_sig =
    cranelift::codegen::ir::Signature::new(isa.default_call_conv());

  #[cfg(target_pointer_width = "32")]
  const ISIZE: Type = types::I32;
  #[cfg(target_pointer_width = "64")]
  const ISIZE: Type = types::I64;

  // Local<Value> receiver
  wrapper_sig.params.push(AbiParam::new(ISIZE));

  fn convert(t: &NativeType) -> AbiParam {
    match t {
      NativeType::U8 => AbiParam::new(types::I8).uext(),
      NativeType::I8 => AbiParam::new(types::I8).sext(),
      NativeType::U16 => AbiParam::new(types::I16).uext(),
      NativeType::I16 => AbiParam::new(types::I16).sext(),
      NativeType::U32 => AbiParam::new(types::I32),
      NativeType::I32 => AbiParam::new(types::I32),
      NativeType::U64 => AbiParam::new(types::I64),
      NativeType::I64 => AbiParam::new(types::I64),
      NativeType::USize => AbiParam::new(ISIZE),
      NativeType::ISize => AbiParam::new(ISIZE),
      NativeType::F32 => AbiParam::new(types::F32),
      NativeType::F64 => AbiParam::new(types::F64),
      NativeType::Bool => AbiParam::new(types::I8).uext(),
      NativeType::Pointer => AbiParam::new(ISIZE),
      NativeType::Buffer => AbiParam::new(ISIZE),
      NativeType::Function => AbiParam::new(ISIZE),
      NativeType::Struct(_) => AbiParam::new(types::INVALID),
      NativeType::Void => AbiParam::new(types::INVALID),
    }
  }

  for pty in &sym.parameter_types {
    let param = convert(pty);

    target_sig.params.push(param);

    if param.value_type == types::I8 || param.value_type == types::I16 {
      wrapper_sig.params.push(AbiParam::new(types::I32));
    } else {
      wrapper_sig.params.push(param);
    }
  }

  let param = convert(&sym.result_type);
  if param.value_type != types::INVALID {
    target_sig.returns.push(param);

    if param.value_type == types::I8 || param.value_type == types::I16 {
      wrapper_sig.returns.push(AbiParam::new(types::I32));
    } else {
      wrapper_sig.returns.push(param);
    }
  }

  let mut ab_sig =
    cranelift::codegen::ir::Signature::new(isa.default_call_conv());
  ab_sig.params.push(AbiParam::new(ISIZE));
  ab_sig.returns.push(AbiParam::new(ISIZE));

  let mut ctx = cranelift::codegen::Context::new();
  let mut fn_builder_ctx = FunctionBuilderContext::new();

  ctx.func = cranelift::codegen::ir::Function::with_name_signature(
    cranelift::codegen::ir::UserFuncName::user(0, 0),
    wrapper_sig,
  );

  let mut f = FunctionBuilder::new(&mut ctx.func, &mut fn_builder_ctx);

  let target_sig = f.import_signature(target_sig);
  let ab_sig = f.import_signature(ab_sig);

  {
    let block = f.create_block();
    f.append_block_params_for_function_params(block);
    f.switch_to_block(block);

    let args = f.block_params(block);
    let mut args = (args[1..]).to_owned();

    for (arg, nty) in args.iter_mut().zip(&sym.parameter_types) {
      match nty {
        NativeType::U8 | NativeType::I8 | NativeType::Bool => {
          *arg = f.ins().ireduce(types::I8, *arg);
        }
        NativeType::U16 | NativeType::I16 => {
          *arg = f.ins().ireduce(types::I16, *arg);
        }
        NativeType::Buffer => {
          let callee = f.ins().iconst(
            ISIZE,
            Imm64::new(turbocall_ab_contents as usize as isize as i64),
          );
          let call = f.ins().call_indirect(ab_sig, callee, &[*arg]);
          let results = f.inst_results(call);
          *arg = results[0];
        }
        _ => {}
      }
    }

    let callee = f.ins().iconst(ISIZE, Imm64::new(sym.ptr.as_ptr() as i64));
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
  }

  f.seal_all_blocks();
  f.finalize();

  cranelift::codegen::verifier::verify_function(&ctx.func, isa.flags())
    .unwrap();

  let code_info = ctx.compile(&*isa, &mut Default::default()).unwrap();

  let data = code_info.buffer.data();
  let mut mutable = memmap2::MmapMut::map_anon(data.len()).unwrap();
  mutable.copy_from_slice(data);
  let buffer = mutable.make_exec().unwrap();

  Trampoline(buffer)
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
) -> *mut u8 {
  let v = v.cast::<deno_core::v8::ArrayBufferView>();
  const {
    // We don't keep `buffer` around when this function returns,
    // so assert that it will be unused.
    assert!(deno_core::v8::TYPED_ARRAY_MAX_SIZE_IN_HEAP == 0);
  }
  let mut buffer = [0; deno_core::v8::TYPED_ARRAY_MAX_SIZE_IN_HEAP];
  // SAFETY: `buffer` is unused due to above, returned pointer is not
  // dereferenced by rust code, and we keep it alive at least as long
  // as the turbocall.
  let (data, _) = unsafe { v.get_contents_raw_parts(&mut buffer) };
  data
}
