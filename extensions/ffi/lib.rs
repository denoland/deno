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
use serde::Deserialize;
use std::borrow::Cow;
use std::collections::HashMap;
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

fn value_as_u64(value: Value) -> u64 {
  value
    .as_u64()
    .expect("Expected ffi arg value to be an unsigned integer")
}

fn value_as_i64(value: Value) -> i64 {
  value
    .as_i64()
    .expect("Expected ffi arg value to be a signed integer")
}

fn value_as_f64(value: Value) -> f64 {
  value
    .as_f64()
    .expect("Expected ffi arg value to be a float")
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
    let parameter_types =
      foreign_fn.parameters.into_iter().map(NativeType::from);
    let result_type = NativeType::from(foreign_fn.result);
    let cif = libffi::middle::Cif::new(
      parameter_types.clone().map(libffi::middle::Type::from),
      result_type.into(),
    );

    self.symbols.insert(
      symbol,
      Symbol {
        cif,
        ptr,
        parameter_types: parameter_types.collect(),
        result_type,
      },
    );

    Ok(())
  }
}

pub fn init<P: FfiPermissions + 'static>(unstable: bool) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:extensions/ffi",
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

#[derive(Deserialize, Clone, Copy)]
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

impl From<String> for NativeType {
  fn from(string: String) -> Self {
    match string.as_str() {
      "void" => NativeType::Void,
      "u8" => NativeType::U8,
      "i8" => NativeType::I8,
      "u16" => NativeType::U16,
      "i16" => NativeType::I16,
      "u32" => NativeType::U32,
      "i32" => NativeType::I32,
      "u64" => NativeType::U64,
      "i64" => NativeType::I64,
      "usize" => NativeType::USize,
      "isize" => NativeType::ISize,
      "f32" => NativeType::F32,
      "f64" => NativeType::F64,
      _ => unimplemented!(),
    }
  }
}

#[derive(Deserialize)]
struct ForeignFunction {
  parameters: Vec<String>,
  result: String,
}

#[derive(Deserialize)]
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

#[derive(Deserialize)]
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
    .get::<DynamicLibraryResource>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let symbol = resource
    .symbols
    .get(&args.symbol)
    .ok_or_else(bad_resource_id)?;

  let call_args: Vec<libffi::middle::Arg> = symbol
    .parameter_types
    .iter()
    .zip(args.parameters.into_iter())
    .map(|(parameter_type, value)| match parameter_type {
      NativeType::Void => libffi::middle::Arg::new(&()),
      NativeType::U8 => libffi::middle::Arg::new(&(value_as_u64(value) as u8)),
      NativeType::I8 => libffi::middle::Arg::new(&(value_as_i64(value) as i8)),
      NativeType::U16 => {
        libffi::middle::Arg::new(&(value_as_u64(value) as u16))
      }
      NativeType::I16 => {
        libffi::middle::Arg::new(&(value_as_i64(value) as i16))
      }
      NativeType::U32 => {
        libffi::middle::Arg::new(&(value_as_u64(value) as u32))
      }
      NativeType::I32 => {
        libffi::middle::Arg::new(&(value_as_i64(value) as i32))
      }
      NativeType::U64 => libffi::middle::Arg::new(&value_as_u64(value)),
      NativeType::I64 => libffi::middle::Arg::new(&value_as_i64(value)),
      NativeType::USize => {
        libffi::middle::Arg::new(&(value_as_u64(value) as usize))
      }
      NativeType::ISize => {
        libffi::middle::Arg::new(&(value_as_i64(value) as isize))
      }
      NativeType::F32 => {
        libffi::middle::Arg::new(&(value_as_f64(value) as f32))
      }
      NativeType::F64 => libffi::middle::Arg::new(&value_as_f64(value)),
    })
    .collect();

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
