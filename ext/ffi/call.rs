// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::callback::PtrSymbol;
use crate::dlfcn::DynamicLibraryResource;
use crate::ir::*;
use crate::symbol::NativeType;
use crate::symbol::Symbol;
use crate::FfiPermissions;
use crate::ForeignFunction;
use deno_core::anyhow::anyhow;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::serde_json::Value;
use deno_core::serde_v8::ExternalPointer;
use deno_core::unsync::spawn_blocking;
use deno_core::v8;
use deno_core::OpState;
use deno_core::ResourceId;
use libffi::middle::Arg;
use serde::Serialize;
use std::cell::RefCell;
use std::ffi::c_void;
use std::future::Future;
use std::rc::Rc;

// SAFETY: Makes an FFI call
unsafe fn ffi_call_rtype_struct(
  cif: &libffi::middle::Cif,
  fn_ptr: &libffi::middle::CodePtr,
  call_args: Vec<Arg>,
  out_buffer: *mut u8,
) {
  libffi::raw::ffi_call(
    cif.as_raw_ptr(),
    Some(*fn_ptr.as_safe_fun()),
    out_buffer as *mut c_void,
    call_args.as_ptr() as *mut *mut c_void,
  );
}

// A one-off synchronous FFI call.
pub(crate) fn ffi_call_sync<'scope>(
  scope: &mut v8::HandleScope<'scope>,
  args: v8::FunctionCallbackArguments,
  symbol: &Symbol,
  out_buffer: Option<OutBuffer>,
) -> Result<NativeValue, AnyError>
where
  'scope: 'scope,
{
  let Symbol {
    parameter_types,
    result_type,
    cif,
    ptr: fun_ptr,
    ..
  } = symbol;
  let mut ffi_args: Vec<NativeValue> =
    Vec::with_capacity(parameter_types.len());

  for (index, native_type) in parameter_types.iter().enumerate() {
    let value = args.get(index as i32);
    match native_type {
      NativeType::Bool => {
        ffi_args.push(ffi_parse_bool_arg(value)?);
      }
      NativeType::U8 => {
        ffi_args.push(ffi_parse_u8_arg(value)?);
      }
      NativeType::I8 => {
        ffi_args.push(ffi_parse_i8_arg(value)?);
      }
      NativeType::U16 => {
        ffi_args.push(ffi_parse_u16_arg(value)?);
      }
      NativeType::I16 => {
        ffi_args.push(ffi_parse_i16_arg(value)?);
      }
      NativeType::U32 => {
        ffi_args.push(ffi_parse_u32_arg(value)?);
      }
      NativeType::I32 => {
        ffi_args.push(ffi_parse_i32_arg(value)?);
      }
      NativeType::U64 => {
        ffi_args.push(ffi_parse_u64_arg(scope, value)?);
      }
      NativeType::I64 => {
        ffi_args.push(ffi_parse_i64_arg(scope, value)?);
      }
      NativeType::USize => {
        ffi_args.push(ffi_parse_usize_arg(scope, value)?);
      }
      NativeType::ISize => {
        ffi_args.push(ffi_parse_isize_arg(scope, value)?);
      }
      NativeType::F32 => {
        ffi_args.push(ffi_parse_f32_arg(value)?);
      }
      NativeType::F64 => {
        ffi_args.push(ffi_parse_f64_arg(value)?);
      }
      NativeType::Buffer => {
        ffi_args.push(ffi_parse_buffer_arg(scope, value)?);
      }
      NativeType::Struct(_) => {
        ffi_args.push(ffi_parse_struct_arg(scope, value)?);
      }
      NativeType::Pointer => {
        ffi_args.push(ffi_parse_pointer_arg(scope, value)?);
      }
      NativeType::Function => {
        ffi_args.push(ffi_parse_function_arg(scope, value)?);
      }
      NativeType::Void => {
        unreachable!();
      }
    }
  }
  let call_args: Vec<Arg> = ffi_args
    .iter()
    .enumerate()
    // SAFETY: Creating a `Arg` from a `NativeValue` is pretty safe.
    .map(|(i, v)| unsafe { v.as_arg(parameter_types.get(i).unwrap()) })
    .collect();
  // SAFETY: types in the `Cif` match the actual calling convention and
  // types of symbol.
  unsafe {
    Ok(match result_type {
      NativeType::Void => NativeValue {
        void_value: cif.call::<()>(*fun_ptr, &call_args),
      },
      NativeType::Bool => NativeValue {
        bool_value: cif.call::<bool>(*fun_ptr, &call_args),
      },
      NativeType::U8 => NativeValue {
        u8_value: cif.call::<u8>(*fun_ptr, &call_args),
      },
      NativeType::I8 => NativeValue {
        i8_value: cif.call::<i8>(*fun_ptr, &call_args),
      },
      NativeType::U16 => NativeValue {
        u16_value: cif.call::<u16>(*fun_ptr, &call_args),
      },
      NativeType::I16 => NativeValue {
        i16_value: cif.call::<i16>(*fun_ptr, &call_args),
      },
      NativeType::U32 => NativeValue {
        u32_value: cif.call::<u32>(*fun_ptr, &call_args),
      },
      NativeType::I32 => NativeValue {
        i32_value: cif.call::<i32>(*fun_ptr, &call_args),
      },
      NativeType::U64 => NativeValue {
        u64_value: cif.call::<u64>(*fun_ptr, &call_args),
      },
      NativeType::I64 => NativeValue {
        i64_value: cif.call::<i64>(*fun_ptr, &call_args),
      },
      NativeType::USize => NativeValue {
        usize_value: cif.call::<usize>(*fun_ptr, &call_args),
      },
      NativeType::ISize => NativeValue {
        isize_value: cif.call::<isize>(*fun_ptr, &call_args),
      },
      NativeType::F32 => NativeValue {
        f32_value: cif.call::<f32>(*fun_ptr, &call_args),
      },
      NativeType::F64 => NativeValue {
        f64_value: cif.call::<f64>(*fun_ptr, &call_args),
      },
      NativeType::Pointer | NativeType::Function | NativeType::Buffer => {
        NativeValue {
          pointer: cif.call::<*mut c_void>(*fun_ptr, &call_args),
        }
      }
      NativeType::Struct(_) => NativeValue {
        void_value: ffi_call_rtype_struct(
          &symbol.cif,
          &symbol.ptr,
          call_args,
          out_buffer.unwrap().0,
        ),
      },
    })
  }
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum FfiValue {
  Value(Value),
  External(ExternalPointer),
}

fn ffi_call(
  call_args: Vec<NativeValue>,
  cif: &libffi::middle::Cif,
  fun_ptr: libffi::middle::CodePtr,
  parameter_types: &[NativeType],
  result_type: NativeType,
  out_buffer: Option<OutBuffer>,
) -> Result<FfiValue, AnyError> {
  let call_args: Vec<Arg> = call_args
    .iter()
    .enumerate()
    .map(|(index, ffi_arg)| {
      // SAFETY: the union field is initialized
      unsafe { ffi_arg.as_arg(parameter_types.get(index).unwrap()) }
    })
    .collect();

  // SAFETY: types in the `Cif` match the actual calling convention and
  // types of symbol.
  unsafe {
    Ok(match result_type {
      NativeType::Void => {
        cif.call::<()>(fun_ptr, &call_args);
        FfiValue::Value(Value::from(()))
      }
      NativeType::Bool => {
        FfiValue::Value(Value::from(cif.call::<bool>(fun_ptr, &call_args)))
      }
      NativeType::U8 => {
        FfiValue::Value(Value::from(cif.call::<u8>(fun_ptr, &call_args)))
      }
      NativeType::I8 => {
        FfiValue::Value(Value::from(cif.call::<i8>(fun_ptr, &call_args)))
      }
      NativeType::U16 => {
        FfiValue::Value(Value::from(cif.call::<u16>(fun_ptr, &call_args)))
      }
      NativeType::I16 => {
        FfiValue::Value(Value::from(cif.call::<i16>(fun_ptr, &call_args)))
      }
      NativeType::U32 => {
        FfiValue::Value(Value::from(cif.call::<u32>(fun_ptr, &call_args)))
      }
      NativeType::I32 => {
        FfiValue::Value(Value::from(cif.call::<i32>(fun_ptr, &call_args)))
      }
      NativeType::U64 => {
        FfiValue::Value(Value::from(cif.call::<u64>(fun_ptr, &call_args)))
      }
      NativeType::I64 => {
        FfiValue::Value(Value::from(cif.call::<i64>(fun_ptr, &call_args)))
      }
      NativeType::USize => {
        FfiValue::Value(Value::from(cif.call::<usize>(fun_ptr, &call_args)))
      }
      NativeType::ISize => {
        FfiValue::Value(Value::from(cif.call::<isize>(fun_ptr, &call_args)))
      }
      NativeType::F32 => {
        FfiValue::Value(Value::from(cif.call::<f32>(fun_ptr, &call_args)))
      }
      NativeType::F64 => {
        FfiValue::Value(Value::from(cif.call::<f64>(fun_ptr, &call_args)))
      }
      NativeType::Pointer | NativeType::Function | NativeType::Buffer => {
        FfiValue::External(ExternalPointer::from(
          cif.call::<*mut c_void>(fun_ptr, &call_args),
        ))
      }
      NativeType::Struct(_) => {
        ffi_call_rtype_struct(cif, &fun_ptr, call_args, out_buffer.unwrap().0);
        FfiValue::Value(Value::Null)
      }
    })
  }
}

#[op2(async)]
#[serde]
pub fn op_ffi_call_ptr_nonblocking<FP>(
  scope: &mut v8::HandleScope,
  state: Rc<RefCell<OpState>>,
  pointer: *mut c_void,
  #[serde] def: ForeignFunction,
  parameters: v8::Local<v8::Array>,
  out_buffer: Option<v8::Local<v8::TypedArray>>,
) -> Result<impl Future<Output = Result<FfiValue, AnyError>>, AnyError>
where
  FP: FfiPermissions + 'static,
{
  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<FP>();
    permissions.check_partial_no_path()?;
  };

  let symbol = PtrSymbol::new(pointer, &def)?;
  let call_args = ffi_parse_args(scope, parameters, &def.parameters)?;
  let out_buffer_ptr = out_buffer_as_ptr(scope, out_buffer);

  let join_handle = spawn_blocking(move || {
    let PtrSymbol { cif, ptr } = symbol.clone();
    ffi_call(
      call_args,
      &cif,
      ptr,
      &def.parameters,
      def.result,
      out_buffer_ptr,
    )
  });

  Ok(async move {
    let result = join_handle
      .await
      .map_err(|err| anyhow!("Nonblocking FFI call failed: {}", err))??;
    // SAFETY: Same return type declared to libffi; trust user to have it right beyond that.
    Ok(result)
  })
}

/// A non-blocking FFI call.
#[op2(async)]
#[serde]
pub fn op_ffi_call_nonblocking(
  scope: &mut v8::HandleScope,
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[string] symbol: String,
  parameters: v8::Local<v8::Array>,
  out_buffer: Option<v8::Local<v8::TypedArray>>,
) -> Result<impl Future<Output = Result<FfiValue, AnyError>>, AnyError> {
  let symbol = {
    let state = state.borrow();
    let resource = state.resource_table.get::<DynamicLibraryResource>(rid)?;
    let symbols = &resource.symbols;
    *symbols
      .get(&symbol)
      .ok_or_else(|| {
        type_error(format!("Invalid FFI symbol name: '{symbol}'"))
      })?
      .clone()
  };

  let call_args = ffi_parse_args(scope, parameters, &symbol.parameter_types)?;
  let out_buffer_ptr = out_buffer_as_ptr(scope, out_buffer);

  let join_handle = spawn_blocking(move || {
    let Symbol {
      cif,
      ptr,
      parameter_types,
      result_type,
      ..
    } = symbol.clone();
    ffi_call(
      call_args,
      &cif,
      ptr,
      &parameter_types,
      result_type,
      out_buffer_ptr,
    )
  });

  Ok(async move {
    let result = join_handle
      .await
      .map_err(|err| anyhow!("Nonblocking FFI call failed: {}", err))??;
    // SAFETY: Same return type declared to libffi; trust user to have it right beyond that.
    Ok(result)
  })
}

#[op2(reentrant)]
#[serde]
pub fn op_ffi_call_ptr<FP>(
  scope: &mut v8::HandleScope,
  state: Rc<RefCell<OpState>>,
  pointer: *mut c_void,
  #[serde] def: ForeignFunction,
  parameters: v8::Local<v8::Array>,
  out_buffer: Option<v8::Local<v8::TypedArray>>,
) -> Result<FfiValue, AnyError>
where
  FP: FfiPermissions + 'static,
{
  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<FP>();
    permissions.check_partial_no_path()?;
  };

  let symbol = PtrSymbol::new(pointer, &def)?;
  let call_args = ffi_parse_args(scope, parameters, &def.parameters)?;

  let out_buffer_ptr = out_buffer_as_ptr(scope, out_buffer);

  let result = ffi_call(
    call_args,
    &symbol.cif,
    symbol.ptr,
    &def.parameters,
    def.result.clone(),
    out_buffer_ptr,
  )?;
  // SAFETY: Same return type declared to libffi; trust user to have it right beyond that.
  Ok(result)
}
