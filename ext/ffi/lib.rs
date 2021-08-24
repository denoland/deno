// Copyright 2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op_sync;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use dlopen::raw::Library;
use libffi::middle::Arg;
use serde::Deserialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::ffi::c_void;
use std::rc::Rc;

pub struct Unstable(pub bool);

fn check_unstable(state: &OpState, api_name: &str) {
  let unstable = state.borrow::<Unstable>();

  if !unstable.0 {
    eprintln!(
      "Unstable API '{}'. The --unstable flag must be provided.",
      api_name
    );
    std::process::exit(70);
  }
}

pub trait FfiPermissions {
  fn check(&mut self, path: &str) -> Result<(), AnyError>;
}

pub struct NoFfiPermissions;

impl FfiPermissions for NoFfiPermissions {
  fn check(&mut self, _path: &str) -> Result<(), AnyError> {
    Ok(())
  }
}

struct Symbol {
  cif: libffi::middle::Cif,
  ptr: libffi::middle::CodePtr,
  parameter_types: Vec<NativeType>,
  result_type: NativeType,
}

struct DynamicLibraryResource {
  lib: Library,
  symbols: HashMap<String, Symbol>,
}

impl Resource for DynamicLibraryResource {
  fn name(&self) -> Cow<str> {
    "dynamicLibrary".into()
  }

  fn close(self: Rc<Self>) {
    drop(self)
  }
}

impl DynamicLibraryResource {
  fn register(
    &mut self,
    symbol: String,
    foreign_fn: ForeignFunction,
  ) -> Result<(), AnyError> {
    let fn_ptr = unsafe { self.lib.symbol::<*const c_void>(&symbol) }?;
    let ptr = libffi::middle::CodePtr::from_ptr(fn_ptr as _);
    let cif = libffi::middle::Cif::new(
      foreign_fn
        .parameters
        .clone()
        .into_iter()
        .map(libffi::middle::Type::from),
      foreign_fn.result.into(),
    );

    self.symbols.insert(
      symbol,
      Symbol {
        cif,
        ptr,
        parameter_types: foreign_fn.parameters,
        result_type: foreign_fn.result,
      },
    );

    Ok(())
  }
}

pub fn init<P: FfiPermissions + 'static>(unstable: bool) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/ffi",
      "00_ffi.js",
    ))
    .ops(vec![
      ("op_ffi_load", op_sync(op_ffi_load::<P>)),
      ("op_ffi_call", op_sync(op_ffi_call)),
    ])
    .state(move |state| {
      // Stolen from deno_webgpu, is there a better option?
      state.put(Unstable(unstable));
      Ok(())
    })
    .build()
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum NativeType {
  Void,
  U8,
  I8,
  U16,
  I16,
  U32,
  I32,
  U64,
  I64,
  USize,
  ISize,
  F32,
  F64,
}

impl From<NativeType> for libffi::middle::Type {
  fn from(native_type: NativeType) -> Self {
    match native_type {
      NativeType::Void => libffi::middle::Type::void(),
      NativeType::U8 => libffi::middle::Type::u8(),
      NativeType::I8 => libffi::middle::Type::i8(),
      NativeType::U16 => libffi::middle::Type::u16(),
      NativeType::I16 => libffi::middle::Type::i16(),
      NativeType::U32 => libffi::middle::Type::u32(),
      NativeType::I32 => libffi::middle::Type::i32(),
      NativeType::U64 => libffi::middle::Type::u64(),
      NativeType::I64 => libffi::middle::Type::i64(),
      NativeType::USize => libffi::middle::Type::usize(),
      NativeType::ISize => libffi::middle::Type::isize(),
      NativeType::F32 => libffi::middle::Type::f32(),
      NativeType::F64 => libffi::middle::Type::f64(),
    }
  }
}

#[repr(C)]
union NativeValue {
  void_value: (),
  u8_value: u8,
  i8_value: i8,
  u16_value: u16,
  i16_value: i16,
  u32_value: u32,
  i32_value: i32,
  u64_value: u64,
  i64_value: i64,
  usize_value: usize,
  isize_value: isize,
  f32_value: f32,
  f64_value: f64,
}

impl NativeValue {
  fn new(native_type: NativeType, value: Value) -> Self {
    match native_type {
      NativeType::Void => Self { void_value: () },
      NativeType::U8 => Self {
        u8_value: value_as_uint::<u8>(value),
      },
      NativeType::I8 => Self {
        i8_value: value_as_int::<i8>(value),
      },
      NativeType::U16 => Self {
        u16_value: value_as_uint::<u16>(value),
      },
      NativeType::I16 => Self {
        i16_value: value_as_int::<i16>(value),
      },
      NativeType::U32 => Self {
        u32_value: value_as_uint::<u32>(value),
      },
      NativeType::I32 => Self {
        i32_value: value_as_int::<i32>(value),
      },
      NativeType::U64 => Self {
        u64_value: value_as_uint::<u64>(value),
      },
      NativeType::I64 => Self {
        i64_value: value_as_int::<i64>(value),
      },
      NativeType::USize => Self {
        usize_value: value_as_uint::<usize>(value),
      },
      NativeType::ISize => Self {
        isize_value: value_as_int::<isize>(value),
      },
      NativeType::F32 => Self {
        f32_value: value_as_f32(value),
      },
      NativeType::F64 => Self {
        f64_value: value_as_f64(value),
      },
    }
  }

  unsafe fn as_arg(&self, native_type: NativeType) -> Arg {
    match native_type {
      NativeType::Void => Arg::new(&self.void_value),
      NativeType::U8 => Arg::new(&self.u8_value),
      NativeType::I8 => Arg::new(&self.i8_value),
      NativeType::U16 => Arg::new(&self.u16_value),
      NativeType::I16 => Arg::new(&self.i16_value),
      NativeType::U32 => Arg::new(&self.u32_value),
      NativeType::I32 => Arg::new(&self.i32_value),
      NativeType::U64 => Arg::new(&self.u64_value),
      NativeType::I64 => Arg::new(&self.i64_value),
      NativeType::USize => Arg::new(&self.usize_value),
      NativeType::ISize => Arg::new(&self.isize_value),
      NativeType::F32 => Arg::new(&self.f32_value),
      NativeType::F64 => Arg::new(&self.f64_value),
    }
  }
}

fn value_as_uint<T: TryFrom<u64>>(value: Value) -> T {
  value
    .as_u64()
    .and_then(|v| T::try_from(v).ok())
    .expect("Expected ffi arg value to be an unsigned integer")
}

fn value_as_int<T: TryFrom<i64>>(value: Value) -> T {
  value
    .as_i64()
    .and_then(|v| T::try_from(v).ok())
    .expect("Expected ffi arg value to be a signed integer")
}

fn value_as_f32(value: Value) -> f32 {
  value_as_f64(value) as f32
}

fn value_as_f64(value: Value) -> f64 {
  value
    .as_f64()
    .expect("Expected ffi arg value to be a float")
}

#[derive(Deserialize, Debug)]
struct ForeignFunction {
  parameters: Vec<NativeType>,
  result: NativeType,
}

#[derive(Deserialize, Debug)]
struct FfiLoadArgs {
  path: String,
  symbols: HashMap<String, ForeignFunction>,
}

fn op_ffi_load<FP>(
  state: &mut deno_core::OpState,
  args: FfiLoadArgs,
  _: (),
) -> Result<ResourceId, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.dlopen");
  let permissions = state.borrow_mut::<FP>();
  permissions.check(&args.path)?;

  let lib = Library::open(args.path)?;
  let mut resource = DynamicLibraryResource {
    lib,
    symbols: HashMap::new(),
  };

  for (symbol, foreign_fn) in args.symbols {
    resource.register(symbol, foreign_fn)?;
  }

  Ok(state.resource_table.add(resource))
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct FfiCallArgs {
  rid: ResourceId,
  symbol: String,
  parameters: Vec<Value>,
}

fn op_ffi_call(
  state: &mut deno_core::OpState,
  args: FfiCallArgs,
  _: (),
) -> Result<Value, AnyError> {
  let resource = state
    .resource_table
    .get::<DynamicLibraryResource>(args.rid)?;

  let symbol = resource
    .symbols
    .get(&args.symbol)
    .ok_or_else(bad_resource_id)?;

  let native_values = symbol
    .parameter_types
    .iter()
    .zip(args.parameters.into_iter())
    .map(|(&native_type, value)| NativeValue::new(native_type, value))
    .collect::<Vec<_>>();

  let call_args = symbol
    .parameter_types
    .iter()
    .zip(native_values.iter())
    .map(|(&native_type, native_value)| unsafe {
      native_value.as_arg(native_type)
    })
    .collect::<Vec<_>>();

  Ok(match symbol.result_type {
    NativeType::Void => {
      json!(unsafe { symbol.cif.call::<()>(symbol.ptr, &call_args) })
    }
    NativeType::U8 => {
      json!(unsafe { symbol.cif.call::<u8>(symbol.ptr, &call_args) })
    }
    NativeType::I8 => {
      json!(unsafe { symbol.cif.call::<i8>(symbol.ptr, &call_args) })
    }
    NativeType::U16 => {
      json!(unsafe { symbol.cif.call::<u16>(symbol.ptr, &call_args) })
    }
    NativeType::I16 => {
      json!(unsafe { symbol.cif.call::<i16>(symbol.ptr, &call_args) })
    }
    NativeType::U32 => {
      json!(unsafe { symbol.cif.call::<u32>(symbol.ptr, &call_args) })
    }
    NativeType::I32 => {
      json!(unsafe { symbol.cif.call::<i32>(symbol.ptr, &call_args) })
    }
    NativeType::U64 => {
      json!(unsafe { symbol.cif.call::<u64>(symbol.ptr, &call_args) })
    }
    NativeType::I64 => {
      json!(unsafe { symbol.cif.call::<i64>(symbol.ptr, &call_args) })
    }
    NativeType::USize => {
      json!(unsafe { symbol.cif.call::<usize>(symbol.ptr, &call_args) })
    }
    NativeType::ISize => {
      json!(unsafe { symbol.cif.call::<isize>(symbol.ptr, &call_args) })
    }
    NativeType::F32 => {
      json!(unsafe { symbol.cif.call::<f32>(symbol.ptr, &call_args) })
    }
    NativeType::F64 => {
      json!(unsafe { symbol.cif.call::<f64>(symbol.ptr, &call_args) })
    }
  })
}
