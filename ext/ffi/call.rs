// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::ffi::c_void;
use std::future::Future;
use std::rc::Rc;

use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::ToV8;
use deno_core::convert::BigInt as ConvertBigInt;
use deno_core::convert::ExternalPointer;
use deno_core::op2;
use deno_core::unsync::spawn_blocking;
use deno_core::v8;
use deno_permissions::PermissionsContainer;
use libffi::middle::Arg;
use num_bigint::BigInt;

use crate::ForeignFunction;
use crate::callback::PtrSymbol;
use crate::dlfcn::DynamicLibraryResource;
use crate::ir::*;
use crate::symbol::NativeType;
use crate::symbol::Symbol;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CallError {
  #[class(type)]
  #[error(transparent)]
  IR(#[from] IRError),
  #[class(generic)]
  #[error("Nonblocking FFI call failed: {0}")]
  NonblockingCallFailure(#[source] tokio::task::JoinError),
  #[class(type)]
  #[error("Invalid FFI symbol name: '{0}'")]
  InvalidSymbol(String),
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] deno_permissions::PermissionCheckError),
  #[class(inherit)]
  #[error(transparent)]
  Resource(#[from] deno_core::error::ResourceError),
  #[class(inherit)]
  #[error(transparent)]
  Callback(#[from] super::CallbackError),
}

// SAFETY: Makes an FFI call
unsafe fn ffi_call_rtype_struct(
  cif: &libffi::middle::Cif,
  fn_ptr: &libffi::middle::CodePtr,
  call_args: Vec<Arg>,
  out_buffer: *mut u8,
) {
  #[allow(
    clippy::undocumented_unsafe_blocks,
    reason = "safety comment on the containing block"
  )]
  unsafe {
    libffi::raw::ffi_call(
      cif.as_raw_ptr(),
      Some(*fn_ptr.as_safe_fun()),
      out_buffer as *mut c_void,
      call_args.as_ptr() as *mut *mut c_void,
    );
  }
}

fn validate_struct_out_buffer(
  cif: &libffi::middle::Cif,
  out_buffer: Option<OutBuffer>,
) -> Result<OutBuffer, IRError> {
  // SAFETY: `Cif::new` prepares a non-null result type that remains owned by
  // `cif`. libffi populates its ABI size while preparing the CIF.
  let expected = unsafe { (*(*cif.as_raw_ptr()).rtype).size };
  out_buffer
    .ok_or(IRError::MissingStructReturnBuffer)?
    .validate_size(expected)
}

// A one-off synchronous FFI call.
pub(crate) fn ffi_call_sync<'scope>(
  scope: &mut v8::PinScope<'scope, '_>,
  args: v8::FunctionCallbackArguments,
  symbol: &Symbol,
  out_buffer: Option<OutBuffer>,
) -> Result<NativeValue, CallError>
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
        ffi_args.push(ffi_parse_buffer_arg(value)?);
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
          validate_struct_out_buffer(&symbol.cif, out_buffer)?.as_ptr(),
        ),
      },
    })
  }
}

#[derive(ToV8)]
#[to_v8(untagged)]
pub enum FfiValue {
  Null,
  Bool(bool),
  Number(f64),
  BigInt(ConvertBigInt),
  External(ExternalPointer),
}

fn ffi_call(
  call_args: Vec<NativeValue>,
  cif: &libffi::middle::Cif,
  fun_ptr: libffi::middle::CodePtr,
  parameter_types: &[NativeType],
  result_type: NativeType,
  out_buffer: Option<OutBuffer>,
) -> FfiValue {
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
    match result_type {
      NativeType::Void => {
        cif.call::<()>(fun_ptr, &call_args);
        FfiValue::Null
      }
      NativeType::Bool => FfiValue::Bool(cif.call::<bool>(fun_ptr, &call_args)),
      NativeType::U8 => {
        FfiValue::Number(cif.call::<u8>(fun_ptr, &call_args) as f64)
      }
      NativeType::I8 => {
        FfiValue::Number(cif.call::<i8>(fun_ptr, &call_args) as f64)
      }
      NativeType::U16 => {
        FfiValue::Number(cif.call::<u16>(fun_ptr, &call_args) as f64)
      }
      NativeType::I16 => {
        FfiValue::Number(cif.call::<i16>(fun_ptr, &call_args) as f64)
      }
      NativeType::U32 => {
        FfiValue::Number(cif.call::<u32>(fun_ptr, &call_args) as f64)
      }
      NativeType::I32 => {
        FfiValue::Number(cif.call::<i32>(fun_ptr, &call_args) as f64)
      }
      NativeType::U64 => FfiValue::BigInt(ConvertBigInt::from(BigInt::from(
        cif.call::<u64>(fun_ptr, &call_args),
      ))),
      NativeType::I64 => FfiValue::BigInt(ConvertBigInt::from(BigInt::from(
        cif.call::<i64>(fun_ptr, &call_args),
      ))),
      NativeType::USize => FfiValue::BigInt(ConvertBigInt::from(BigInt::from(
        cif.call::<usize>(fun_ptr, &call_args),
      ))),
      NativeType::ISize => FfiValue::BigInt(ConvertBigInt::from(BigInt::from(
        cif.call::<isize>(fun_ptr, &call_args),
      ))),
      NativeType::F32 => {
        FfiValue::Number(cif.call::<f32>(fun_ptr, &call_args) as f64)
      }
      NativeType::F64 => FfiValue::Number(cif.call::<f64>(fun_ptr, &call_args)),
      NativeType::Pointer | NativeType::Function | NativeType::Buffer => {
        FfiValue::External(ExternalPointer::from(
          cif.call::<*mut c_void>(fun_ptr, &call_args),
        ))
      }
      NativeType::Struct(_) => {
        ffi_call_rtype_struct(
          cif,
          &fun_ptr,
          call_args,
          out_buffer
            .expect("struct return buffer was validated")
            .as_ptr(),
        );
        FfiValue::Null
      }
    }
  }
}

#[op2(stack_trace)]
pub fn op_ffi_call_ptr_nonblocking(
  scope: &mut v8::PinScope<'_, '_>,
  state: Rc<RefCell<OpState>>,
  pointer: *mut c_void,
  #[serde] def: ForeignFunction,
  parameters: v8::Local<v8::Array>,
  out_buffer: Option<v8::Local<v8::TypedArray>>,
) -> Result<impl Future<Output = Result<FfiValue, CallError>> + use<>, CallError>
where
{
  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions.check_ffi_partial_no_path()?;
  };

  let symbol = PtrSymbol::new(pointer, &def)?;
  let mut backing_store_holder = BackingStoreHolder::new();
  let call_args = ffi_parse_args_nonblocking(
    scope,
    parameters,
    &def.parameters,
    &mut backing_store_holder,
  )?;
  let out_buffer =
    out_buffer_as_ptr_nonblocking(out_buffer, &mut backing_store_holder)?;
  let out_buffer_ptr = if matches!(&def.result, NativeType::Struct(_)) {
    Some(validate_struct_out_buffer(&symbol.cif, out_buffer)?)
  } else {
    None
  };

  let join_handle = spawn_blocking(move || {
    let PtrSymbol { cif, ptr } = symbol.clone();
    let result = ffi_call(
      call_args,
      &cif,
      ptr,
      &def.parameters,
      def.result,
      out_buffer_ptr,
    );
    // prevent backing stores from being dropped before the FFI call completes
    drop(backing_store_holder);
    result
  });

  Ok(async move {
    let result = join_handle
      .await
      .map_err(CallError::NonblockingCallFailure)?;
    // SAFETY: Same return type declared to libffi; trust user to have it right beyond that.
    Ok(result)
  })
}

/// A non-blocking FFI call.
#[op2]
pub fn op_ffi_call_nonblocking(
  scope: &mut v8::PinScope<'_, '_>,
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[string] symbol: String,
  parameters: v8::Local<v8::Array>,
  out_buffer: Option<v8::Local<v8::TypedArray>>,
) -> Result<impl Future<Output = Result<FfiValue, CallError>> + use<>, CallError>
{
  let symbol = {
    let state = state.borrow();
    let resource = state.resource_table.get::<DynamicLibraryResource>(rid)?;
    let symbols = &resource.symbols;
    *symbols
      .get(&symbol)
      .ok_or_else(|| CallError::InvalidSymbol(symbol))?
      .clone()
  };

  let mut backing_store_holder = BackingStoreHolder::new();
  let call_args = ffi_parse_args_nonblocking(
    scope,
    parameters,
    &symbol.parameter_types,
    &mut backing_store_holder,
  )?;
  let out_buffer =
    out_buffer_as_ptr_nonblocking(out_buffer, &mut backing_store_holder)?;
  let out_buffer_ptr = if matches!(&symbol.result_type, NativeType::Struct(_)) {
    Some(validate_struct_out_buffer(&symbol.cif, out_buffer)?)
  } else {
    None
  };

  let join_handle = spawn_blocking(move || {
    let Symbol {
      cif,
      ptr,
      parameter_types,
      result_type,
      ..
    } = symbol.clone();
    let result = ffi_call(
      call_args,
      &cif,
      ptr,
      &parameter_types,
      result_type,
      out_buffer_ptr,
    );
    // prevent backing stores from being dropped before the FFI call completes
    drop(backing_store_holder);
    result
  });

  Ok(async move {
    let result = join_handle
      .await
      .map_err(CallError::NonblockingCallFailure)?;
    // SAFETY: Same return type declared to libffi; trust user to have it right beyond that.
    Ok(result)
  })
}

#[op2(reentrant, stack_trace)]
pub fn op_ffi_call_ptr(
  scope: &mut v8::PinScope<'_, '_>,
  state: Rc<RefCell<OpState>>,
  pointer: *mut c_void,
  #[serde] def: ForeignFunction,
  parameters: v8::Local<v8::Array>,
  out_buffer: Option<v8::Local<v8::TypedArray>>,
) -> Result<FfiValue, CallError> {
  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions.check_ffi_partial_no_path()?;
  };

  let symbol = PtrSymbol::new(pointer, &def)?;
  let call_args = ffi_parse_args(scope, parameters, &def.parameters)?;

  let out_buffer = out_buffer_as_ptr(out_buffer);
  let out_buffer_ptr = if matches!(&def.result, NativeType::Struct(_)) {
    Some(validate_struct_out_buffer(&symbol.cif, out_buffer)?)
  } else {
    None
  };

  let result = ffi_call(
    call_args,
    &symbol.cif,
    symbol.ptr,
    &def.parameters,
    def.result.clone(),
    out_buffer_ptr,
  );
  // SAFETY: Same return type declared to libffi; trust user to have it right beyond that.
  Ok(result)
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;
  use std::sync::atomic::AtomicUsize;
  use std::sync::atomic::Ordering;

  use deno_core::JsRuntime;
  use deno_core::RuntimeOptions;
  use deno_permissions::PermissionsContainer;
  use deno_permissions::RuntimePermissionDescriptorParser;

  use super::op_ffi_call_ptr;
  use super::op_ffi_call_ptr_nonblocking;
  use crate::repr::op_ffi_ptr_create;

  deno_core::extension!(
    test_ffi_call_ops,
    ops = [
      op_ffi_call_ptr,
      op_ffi_call_ptr_nonblocking,
      op_ffi_ptr_create,
    ],
  );

  #[derive(Clone, Copy)]
  #[repr(C)]
  struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
  }

  static MAKE_RECT_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

  extern "C" fn make_rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    MAKE_RECT_CALL_COUNT.fetch_add(1, Ordering::Relaxed);
    Rect {
      x,
      y,
      width,
      height,
    }
  }

  #[tokio::test]
  async fn caller_provided_struct_return_views() {
    MAKE_RECT_CALL_COUNT.store(0, Ordering::Relaxed);

    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ffi_call_ops::init()],
      ..Default::default()
    });
    let parser = Arc::new(RuntimePermissionDescriptorParser::new(
      sys_traits::impls::RealSys,
    ));
    runtime
      .op_state()
      .borrow_mut()
      .put(PermissionsContainer::allow_all(parser));

    let function_pointer = make_rect as *const () as usize;
    let source = r#"
      (async () => {
        const {
          op_ffi_call_ptr,
          op_ffi_call_ptr_nonblocking,
          op_ffi_ptr_create,
        } = Deno.core.ops;
        const pointer = op_ffi_ptr_create(__FUNCTION_POINTER__n);
        const definition = {
          parameters: ["f64", "f64", "f64", "f64"],
          result: { struct: ["f64", "f64", "f64", "f64"] },
        };
        const fill = 0xee;

        function assertEquals(actual, expected) {
          if (actual.length !== expected.length) {
            throw new Error(`length mismatch: ${actual.length} !== ${expected.length}`);
          }
          for (let i = 0; i < actual.length; i++) {
            if (!Object.is(actual[i], expected[i])) {
              throw new Error(`value mismatch at ${i}: ${actual[i]} !== ${expected[i]}`);
            }
          }
        }

        function assertInvalidBufferError(error, actual) {
          if (!(error instanceof TypeError)) {
            throw new Error(`expected TypeError, got ${error?.constructor?.name}`);
          }
          const expected =
            `Invalid FFI struct return buffer: expected at least 32 bytes, got ${actual}`;
          if (error.message !== expected) {
            throw new Error(`unexpected error: ${error.message}`);
          }
        }

        function assertMissingBufferError(error) {
          if (!(error instanceof TypeError)) {
            throw new Error(`expected TypeError, got ${error?.constructor?.name}`);
          }
          if (error.message !== "Missing FFI struct return buffer") {
            throw new Error(`unexpected error: ${error.message}`);
          }
        }

        const syncBacking = new Uint8Array(64);
        syncBacking.fill(fill);
        const syncOut = new Uint8Array(syncBacking.buffer, 16, 32);
        op_ffi_call_ptr(pointer, definition, [1, 2, 3, 4], syncOut);
        assertEquals(syncBacking.subarray(0, 16), new Array(16).fill(fill));
        assertEquals(
          new Float64Array(syncBacking.buffer, 16, 4),
          [1, 2, 3, 4],
        );
        assertEquals(syncBacking.subarray(48), new Array(16).fill(fill));

        const asyncBacking = new Uint8Array(64);
        asyncBacking.fill(fill);
        const asyncOut = new Uint8Array(asyncBacking.buffer, 16, 32);
        await op_ffi_call_ptr_nonblocking(
          pointer,
          { ...definition, nonblocking: true },
          [5, 6, 7, 8],
          asyncOut,
        );
        assertEquals(asyncBacking.subarray(0, 16), new Array(16).fill(fill));
        assertEquals(
          new Float64Array(asyncBacking.buffer, 16, 4),
          [5, 6, 7, 8],
        );
        assertEquals(asyncBacking.subarray(48), new Array(16).fill(fill));

        const undersizedOut = new Uint8Array(31);
        try {
          op_ffi_call_ptr(pointer, definition, [9, 10, 11, 12], undersizedOut);
          throw new Error("synchronous call accepted an undersized buffer");
        } catch (error) {
          assertInvalidBufferError(error, 31);
        }
        try {
          await op_ffi_call_ptr_nonblocking(
            pointer,
            { ...definition, nonblocking: true },
            [9, 10, 11, 12],
            undersizedOut,
          );
          throw new Error("nonblocking call accepted an undersized buffer");
        } catch (error) {
          assertInvalidBufferError(error, 31);
        }

        try {
          op_ffi_call_ptr(pointer, definition, [9, 10, 11, 12]);
          throw new Error("synchronous call accepted a missing buffer");
        } catch (error) {
          assertMissingBufferError(error);
        }

        try {
          await op_ffi_call_ptr_nonblocking(
            pointer,
            { ...definition, nonblocking: true },
            [9, 10, 11, 12],
          );
          throw new Error("nonblocking call accepted a missing buffer");
        } catch (error) {
          assertMissingBufferError(error);
        }

        const zeroLengthOut = new Uint8Array(new ArrayBuffer(32), 0, 0);
        try {
          op_ffi_call_ptr(
            pointer,
            definition,
            [9, 10, 11, 12],
            zeroLengthOut,
          );
          throw new Error("synchronous call accepted a zero-length buffer");
        } catch (error) {
          assertInvalidBufferError(error, 0);
        }

        const detachedOut = new Uint8Array(32);
        detachedOut.buffer.transfer();
        try {
          await op_ffi_call_ptr_nonblocking(
            pointer,
            { ...definition, nonblocking: true },
            [9, 10, 11, 12],
            detachedOut,
          );
          throw new Error("nonblocking call accepted a detached buffer");
        } catch (error) {
          assertInvalidBufferError(error, 0);
        }
      })()
    "#
    .replace("__FUNCTION_POINTER__", &function_pointer.to_string());

    let promise = runtime.execute_script("ffi_call_test.js", source).unwrap();
    #[allow(deprecated, reason = "test code")]
    runtime.resolve_value(promise).await.unwrap();

    assert_eq!(MAKE_RECT_CALL_COUNT.load(Ordering::Relaxed), 2);
  }
}
