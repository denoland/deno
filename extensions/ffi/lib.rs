// Copyright 2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op_sync;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::Extension;
use deno_core::Resource;
use deno_core::ResourceId;
use dlopen::raw::Library;
use libffi::middle::Cif;
use serde::Deserialize;
use std::borrow::Cow;
use std::ffi::c_void;
use std::rc::Rc;

struct LibraryResource(Library);

impl Resource for LibraryResource {
  fn name(&self) -> Cow<str> {
    "library".into()
  }

  fn close(self: Rc<Self>) {
    drop(self)
  }
}

pub fn init() -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:extensions/ffi",
      "00_ffi.js",
    ))
    .ops(vec![
      ("op_dlopen", op_sync(op_dlopen)),
      ("op_dlcall", op_sync(op_dlcall)),
    ])
    .build()
}

fn op_dlopen(
  state: &mut deno_core::OpState,
  path: String,
  _: (),
) -> Result<ResourceId, AnyError> {
  Ok(
    state
      .resource_table
      .add(LibraryResource(Library::open(path)?)),
  )
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct FFIArg {
  ffi_type: FFIType,
  value: Value,
}

impl From<FFIArg> for libffi::middle::Arg {
  fn from(arg: FFIArg) -> Self {
    match arg.ffi_type {
      FFIType::Void => libffi::middle::Arg::new(&()),
      FFIType::U8 => libffi::middle::Arg::new(&(arg.as_u64() as u8)),
      FFIType::I8 => libffi::middle::Arg::new(&(arg.as_i64() as i8)),
      FFIType::U16 => libffi::middle::Arg::new(&(arg.as_u64() as u16)),
      FFIType::I16 => libffi::middle::Arg::new(&(arg.as_i64() as i16)),
      FFIType::U32 => libffi::middle::Arg::new(&(arg.as_u64() as u32)),
      FFIType::I32 => libffi::middle::Arg::new(&(arg.as_i64() as i32)),
      FFIType::U64 => libffi::middle::Arg::new(&arg.as_u64()),
      FFIType::I64 => libffi::middle::Arg::new(&arg.as_i64()),
      FFIType::USize => libffi::middle::Arg::new(&(arg.as_u64() as usize)),
      FFIType::ISize => libffi::middle::Arg::new(&(arg.as_i64() as isize)),
      FFIType::F32 => libffi::middle::Arg::new(&(arg.as_f64() as f32)),
      FFIType::F64 => libffi::middle::Arg::new(&arg.as_f64()),
    }
  }
}

impl FFIArg {
  fn as_u64(self) -> u64 {
    self
      .value
      .as_u64()
      .expect("Expected ffi arg value to be an unsigned integer")
  }

  fn as_i64(self) -> i64 {
    self
      .value
      .as_i64()
      .expect("Expected ffi arg value to be a signed integer")
  }

  fn as_f64(self) -> f64 {
    self
      .value
      .as_f64()
      .expect("Expected ffi arg value to be a float")
  }
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum FFIType {
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
  //  Ptr,
  //  CStr,
  //  Struct(Vec<FFIType>),
}

impl From<FFIType> for libffi::middle::Type {
  fn from(r#type: FFIType) -> Self {
    match r#type {
      FFIType::Void => libffi::middle::Type::void(),
      FFIType::U8 => libffi::middle::Type::u8(),
      FFIType::I8 => libffi::middle::Type::i8(),
      FFIType::U16 => libffi::middle::Type::u16(),
      FFIType::I16 => libffi::middle::Type::i16(),
      FFIType::U32 => libffi::middle::Type::u32(),
      FFIType::I32 => libffi::middle::Type::i32(),
      FFIType::U64 => libffi::middle::Type::u64(),
      FFIType::I64 => libffi::middle::Type::i64(),
      FFIType::USize => libffi::middle::Type::usize(),
      FFIType::ISize => libffi::middle::Type::isize(),
      FFIType::F32 => libffi::middle::Type::f32(),
      FFIType::F64 => libffi::middle::Type::f64(),
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DlcallArgs {
  rid: ResourceId,
  sym: String,
  args: Vec<FFIArg>,
  return_type: FFIType,
}

fn op_dlcall(
  state: &mut deno_core::OpState,
  dlcall_args: DlcallArgs,
  _: (),
) -> Result<Value, AnyError> {
  let library = state
    .resource_table
    .get::<LibraryResource>(dlcall_args.rid)
    .ok_or_else(bad_resource_id)?;
  let fn_ptr = unsafe { library.0.symbol::<*const c_void>(&dlcall_args.sym) }?;
  let fn_code_ptr = libffi::middle::CodePtr::from_ptr(fn_ptr as _);
  let types = dlcall_args
    .args
    .clone()
    .into_iter()
    .map(|arg| arg.ffi_type.into());
  let cif = Cif::new(types, dlcall_args.return_type.into());
  let args: Vec<libffi::middle::Arg> =
    dlcall_args.args.into_iter().map(|arg| arg.into()).collect();

  Ok(match dlcall_args.return_type {
    FFIType::Void => json!(unsafe { cif.call::<()>(fn_code_ptr, &args) }),
    FFIType::U8 => json!(unsafe { cif.call::<u8>(fn_code_ptr, &args) }),
    FFIType::I8 => json!(unsafe { cif.call::<i8>(fn_code_ptr, &args) }),
    FFIType::U16 => json!(unsafe { cif.call::<u16>(fn_code_ptr, &args) }),
    FFIType::I16 => json!(unsafe { cif.call::<i16>(fn_code_ptr, &args) }),
    FFIType::U32 => json!(unsafe { cif.call::<u32>(fn_code_ptr, &args) }),
    FFIType::I32 => json!(unsafe { cif.call::<i32>(fn_code_ptr, &args) }),
    FFIType::U64 => json!(unsafe { cif.call::<u64>(fn_code_ptr, &args) }),
    FFIType::I64 => json!(unsafe { cif.call::<i64>(fn_code_ptr, &args) }),
    FFIType::USize => json!(unsafe { cif.call::<usize>(fn_code_ptr, &args) }),
    FFIType::ISize => json!(unsafe { cif.call::<isize>(fn_code_ptr, &args) }),
    FFIType::F32 => json!(unsafe { cif.call::<f32>(fn_code_ptr, &args) }),
    FFIType::F64 => json!(unsafe { cif.call::<f64>(fn_code_ptr, &args) }),
  })
}
