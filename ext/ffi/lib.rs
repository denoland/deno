// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::generic_error;
use deno_core::error::range_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op;

use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::serde_v8;
use deno_core::v8;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use dlopen::raw::Library;
use libffi::middle::Arg;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::Path;
use std::path::PathBuf;
use std::ptr;
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

pub fn check_unstable2(state: &Rc<RefCell<OpState>>, api_name: &str) {
  let state = state.borrow();
  check_unstable(&state, api_name)
}

pub trait FfiPermissions {
  fn check(&mut self, path: Option<&Path>) -> Result<(), AnyError>;
}

#[derive(Clone)]
struct Symbol {
  cif: libffi::middle::Cif,
  ptr: libffi::middle::CodePtr,
  parameter_types: Vec<NativeType>,
  result_type: NativeType,
}

#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl Send for Symbol {}
unsafe impl Sync for Symbol {}

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
    name: String,
    foreign_fn: ForeignFunction,
  ) -> Result<(), AnyError> {
    let symbol = match &foreign_fn.name {
      Some(symbol) => symbol,
      None => &name,
    };
    // By default, Err returned by this function does not tell
    // which symbol wasn't exported. So we'll modify the error
    // message to include the name of symbol.
    let fn_ptr = match unsafe { self.lib.symbol::<*const c_void>(symbol) } {
      Ok(value) => Ok(value),
      Err(err) => Err(generic_error(format!(
        "Failed to register symbol {}: {}",
        symbol, err
      ))),
    }?;
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
      name,
      Symbol {
        cif,
        ptr,
        parameter_types: foreign_fn.parameters,
        result_type: foreign_fn.result,
      },
    );

    Ok(())
  }

  fn get_static(&self, symbol: String) -> Result<*const c_void, AnyError> {
    // By default, Err returned by this function does not tell
    // which symbol wasn't exported. So we'll modify the error
    // message to include the name of symbol.
    match unsafe { self.lib.symbol::<*const c_void>(&symbol) } {
      Ok(value) => Ok(Ok(value)),
      Err(err) => Err(generic_error(format!(
        "Failed to register symbol {}: {}",
        symbol, err
      ))),
    }?
  }
}

pub fn init<P: FfiPermissions + 'static>(unstable: bool) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/ffi",
      "00_ffi.js",
    ))
    .ops(vec![
      op_ffi_load::decl::<P>(),
      op_ffi_get_static::decl(),
      op_ffi_call::decl(),
      op_ffi_call_nonblocking::decl(),
      op_ffi_call_ptr::decl(),
      op_ffi_call_ptr_nonblocking::decl(),
      op_ffi_ptr_of::decl::<P>(),
      op_ffi_buf_copy_into::decl::<P>(),
      op_ffi_cstr_read::decl::<P>(),
      op_ffi_read_u8::decl::<P>(),
      op_ffi_read_i8::decl::<P>(),
      op_ffi_read_u16::decl::<P>(),
      op_ffi_read_i16::decl::<P>(),
      op_ffi_read_u32::decl::<P>(),
      op_ffi_read_i32::decl::<P>(),
      op_ffi_read_u64::decl::<P>(),
      op_ffi_read_f32::decl::<P>(),
      op_ffi_read_f64::decl::<P>(),
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
  Pointer,
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
      NativeType::Pointer => libffi::middle::Type::pointer(),
    }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
struct U32x2(u32, u32);

impl From<u64> for U32x2 {
  fn from(value: u64) -> Self {
    Self((value >> 32) as u32, value as u32)
  }
}

impl From<U32x2> for u64 {
  fn from(value: U32x2) -> Self {
    (value.0 as u64) << 32 | value.1 as u64
  }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ForeignFunction {
  name: Option<String>,
  parameters: Vec<NativeType>,
  result: NativeType,
}

// ForeignStatic's name and type fields are read and used by
// serde_v8 to determine which variant a ForeignSymbol is.
// They are not used beyond that and are thus marked with underscores.
#[derive(Deserialize, Debug)]
struct ForeignStatic {
  #[serde(rename(deserialize = "name"))]
  _name: Option<String>,
  #[serde(rename(deserialize = "type"))]
  _type: String,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum ForeignSymbol {
  ForeignFunction(ForeignFunction),
  ForeignStatic(ForeignStatic),
}

#[derive(Deserialize, Debug)]
struct FfiLoadArgs {
  path: String,
  symbols: HashMap<String, ForeignSymbol>,
}

// `path` is only used on Windows.
#[allow(unused_variables)]
pub(crate) fn format_error(e: dlopen::Error, path: String) -> String {
  match e {
    #[cfg(target_os = "windows")]
    // This calls FormatMessageW with library path
    // as replacement for the insert sequences.
    // Unlike libstd which passes the FORMAT_MESSAGE_IGNORE_INSERTS
    // flag without any arguments.
    //
    // https://github.com/denoland/deno/issues/11632
    dlopen::Error::OpeningLibraryError(e) => {
      use std::ffi::OsStr;
      use std::os::windows::ffi::OsStrExt;
      use winapi::shared::minwindef::DWORD;
      use winapi::shared::winerror::ERROR_INSUFFICIENT_BUFFER;
      use winapi::um::errhandlingapi::GetLastError;
      use winapi::um::winbase::FormatMessageW;
      use winapi::um::winbase::FORMAT_MESSAGE_ARGUMENT_ARRAY;
      use winapi::um::winbase::FORMAT_MESSAGE_FROM_SYSTEM;
      use winapi::um::winnt::LANG_SYSTEM_DEFAULT;
      use winapi::um::winnt::MAKELANGID;
      use winapi::um::winnt::SUBLANG_SYS_DEFAULT;

      let err_num = match e.raw_os_error() {
        Some(err_num) => err_num,
        // This should never hit unless dlopen changes its error type.
        None => return e.to_string(),
      };

      // Language ID (0x0800)
      let lang_id =
        MAKELANGID(LANG_SYSTEM_DEFAULT, SUBLANG_SYS_DEFAULT) as DWORD;

      let mut buf = vec![0; 500];

      let path = OsStr::new(&path)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect::<Vec<_>>();

      let arguments = [path.as_ptr()];

      loop {
        unsafe {
          let length = FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_ARGUMENT_ARRAY,
            std::ptr::null_mut(),
            err_num as DWORD,
            lang_id as DWORD,
            buf.as_mut_ptr(),
            buf.len() as DWORD,
            arguments.as_ptr() as _,
          );

          if length == 0 {
            let err_num = GetLastError();
            if err_num == ERROR_INSUFFICIENT_BUFFER {
              buf.resize(buf.len() * 2, 0);
              continue;
            }

            // Something went wrong, just return the original error.
            return e.to_string();
          }

          let msg = String::from_utf16_lossy(&buf[..length as usize]);
          return msg;
        }
      }
    }
    _ => e.to_string(),
  }
}

#[op]
fn op_ffi_load<FP>(
  state: &mut deno_core::OpState,
  args: FfiLoadArgs,
) -> Result<ResourceId, AnyError>
where
  FP: FfiPermissions + 'static,
{
  let path = args.path;

  check_unstable(state, "Deno.dlopen");
  let permissions = state.borrow_mut::<FP>();
  permissions.check(Some(&PathBuf::from(&path)))?;

  let lib = Library::open(&path).map_err(|e| {
    dlopen::Error::OpeningLibraryError(std::io::Error::new(
      std::io::ErrorKind::Other,
      format_error(e, path),
    ))
  })?;

  let mut resource = DynamicLibraryResource {
    lib,
    symbols: HashMap::new(),
  };

  for (symbol, foreign_symbol) in args.symbols {
    match foreign_symbol {
      ForeignSymbol::ForeignStatic(_) => {
        // No-op: Statics will be handled separately and are not part of the Rust-side resource.
      }
      ForeignSymbol::ForeignFunction(foreign_fn) => {
        resource.register(symbol, foreign_fn)?;
      }
    }
  }

  Ok(state.resource_table.add(resource))
}

fn get_symbol(fn_ptr: u64, def: ForeignFunction) -> Symbol {
  let ptr = libffi::middle::CodePtr::from_ptr(fn_ptr as _);
  let cif = libffi::middle::Cif::new(
    def
      .parameters
      .clone()
      .into_iter()
      .map(libffi::middle::Type::from),
    def.result.into(),
  );

  Symbol {
    cif,
    ptr,
    parameter_types: def.parameters.clone(),
    result_type: def.result,
  }
}

#[inline]
fn value_as_arg(
  scope: &mut v8::HandleScope,
  value: v8::Local<v8::Value>,
  native_type: NativeType,
) -> Result<Arg, AnyError> {
  let value = match native_type {
    NativeType::Void => Arg::new(&()),
    NativeType::U8 => {
      let value = value
        .integer_value(scope)
        .ok_or_else(|| generic_error("Expected u8"))?;
      Arg::new(&u8::try_from(value).map_err(|_| generic_error("Expected u8"))?)
    }
    NativeType::I8 => {
      let value = value
        .integer_value(scope)
        .ok_or_else(|| generic_error("Expected i8"))?;
      Arg::new(&i8::try_from(value).map_err(|_| generic_error("Expected i8"))?)
    }
    NativeType::U16 => {
      let value = value
        .integer_value(scope)
        .ok_or_else(|| generic_error("Expected u16"))?;
      Arg::new(
        &u16::try_from(value).map_err(|_| generic_error("Expected u16"))?,
      )
    }
    NativeType::I16 => {
      let value = value
        .integer_value(scope)
        .ok_or_else(|| generic_error("Expected i16"))?;
      Arg::new(
        &i16::try_from(value).map_err(|_| generic_error("Expected i16"))?,
      )
    }
    NativeType::U32 => {
      let value = value
        .integer_value(scope)
        .ok_or_else(|| generic_error("Expected u32"))?;
      Arg::new(&value)
    }
    NativeType::I32 => {
      let value = value
        .integer_value(scope)
        .ok_or_else(|| generic_error("Expected i32"))?;
      Arg::new(&value)
    }
    NativeType::U64 | NativeType::USize => {
      let value = value
        .integer_value(scope)
        .ok_or_else(|| generic_error("Expected u64"))?;
      Arg::new(
        &u64::try_from(value).map_err(|_| generic_error("Expected u64"))?,
      )
    }
    NativeType::I64 | NativeType::ISize => {
      let value = value
        .integer_value(scope)
        .ok_or_else(|| generic_error("Expected i64"))?;
      Arg::new(&value)
    }
    NativeType::F32 => {
      let value = value
        .number_value(scope)
        .ok_or_else(|| generic_error("Expected f32"))?;
      Arg::new(&(value as f32))
    }
    NativeType::F64 => {
      let value = value
        .number_value(scope)
        .ok_or_else(|| generic_error("Expected f64"))?;
      Arg::new(&value)
    }
    NativeType::Pointer => {
      if let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(value) {
        let bs = view
          .buffer(scope)
          .ok_or_else(|| generic_error("Expected ArrayBuffer"))?
          .get_backing_store();
        let ptr = bs
          .data()
          .ok_or_else(|| generic_error("ArrayBuffer of zero length"))?
          .as_ptr() as *const u8;
        Arg::new(&ptr)
      } else if value.is_null() {
        Arg::new::<*const u8>(&ptr::null())
      } else {
        // U32x2 -> pointer
        let u32x2: U32x2 = serde_v8::from_v8(scope, value)?;
        let value: u64 = u32x2.into();
        Arg::new(&(value as *const u8))
      }
    }
  };
  Ok(value)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FfiGetArgs {
  rid: ResourceId,
  name: String,
  r#type: NativeType,
}

#[op]
fn op_ffi_get_static(
  state: &mut deno_core::OpState,
  args: FfiGetArgs,
) -> Result<Value, AnyError> {
  let resource = state
    .resource_table
    .get::<DynamicLibraryResource>(args.rid)?;

  let data_ptr = resource.get_static(args.name)? as *const u8;

  Ok(match args.r#type {
    NativeType::Void => {
      unreachable!();
    }
    NativeType::U8 => {
      json!(unsafe { ptr::read_unaligned(data_ptr as *const u8) })
    }
    NativeType::I8 => {
      json!(unsafe { ptr::read_unaligned(data_ptr as *const i8) })
    }
    NativeType::U16 => {
      json!(unsafe { ptr::read_unaligned(data_ptr as *const u16) })
    }
    NativeType::I16 => {
      json!(unsafe { ptr::read_unaligned(data_ptr as *const i16) })
    }
    NativeType::U32 => {
      json!(unsafe { ptr::read_unaligned(data_ptr as *const u32) })
    }
    NativeType::I32 => {
      json!(unsafe { ptr::read_unaligned(data_ptr as *const i32) })
    }
    NativeType::U64 => {
      json!(unsafe { ptr::read_unaligned(data_ptr as *const u64) })
    }
    NativeType::I64 => {
      json!(unsafe { ptr::read_unaligned(data_ptr as *const i64) })
    }
    NativeType::USize => {
      json!(unsafe { ptr::read_unaligned(data_ptr as *const usize) })
    }
    NativeType::ISize => {
      json!(unsafe { ptr::read_unaligned(data_ptr as *const isize) })
    }
    NativeType::F32 => {
      json!(unsafe { ptr::read_unaligned(data_ptr as *const f32) })
    }
    NativeType::F64 => {
      json!(unsafe { ptr::read_unaligned(data_ptr as *const f64) })
    }
    NativeType::Pointer => {
      json!(U32x2::from(data_ptr as *const u8 as u64))
    }
  })
}

macro_rules! impl_ffi_op {
  ($name: ident, $fn: ident, $is_async: ident) => {
    #[allow(non_camel_case_types)]
    pub struct $name;
    impl $name {
      pub fn decl() -> deno_core::OpDecl {
        use v8::MapFnTo;
        deno_core::OpDecl {
          name: stringify!($name),
          v8_fn_ptr: $fn.map_fn_to(),
          enabled: true,
          is_async: $is_async,
          is_unstable: true,
        }
      }
    }
  };
}

pub fn op_ffi_call_ptr_fn(
  scope: &mut deno_core::v8::HandleScope,
  args: deno_core::v8::FunctionCallbackArguments,
  mut rv: deno_core::v8::ReturnValue,
) {
  let pointer: U32x2 = serde_v8::from_v8(scope, args.get(0)).unwrap();
  let fn_def: ForeignFunction = serde_v8::from_v8(scope, args.get(1)).unwrap();
  let symbol = get_symbol(pointer.into(), fn_def);
  let args = v8::Local::<v8::Array>::try_from(args.get(2)).unwrap();
  let args_len = args.length() as usize;
  let mut call_args = Vec::with_capacity(args_len);
  for idx in 0..args_len {
    let arg = args.get_index(scope, idx as u32).unwrap();
    call_args
      .push(value_as_arg(scope, arg, symbol.parameter_types[idx]).unwrap());
  }

  let value = match symbol.result_type {
    NativeType::Void => {
      unsafe { symbol.cif.call::<()>(symbol.ptr, &call_args) };
      return;
    }
    NativeType::U8 => {
      let value = unsafe { symbol.cif.call::<u8>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::I8 => {
      let value = unsafe { symbol.cif.call::<i8>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::U16 => {
      let value = unsafe { symbol.cif.call::<u16>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::I16 => {
      let value = unsafe { symbol.cif.call::<i16>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::U32 => {
      let value = unsafe { symbol.cif.call::<u32>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::I32 => {
      let value = unsafe { symbol.cif.call::<i32>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::U64 => {
      let value = unsafe { symbol.cif.call::<u64>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::I64 => {
      let value = unsafe { symbol.cif.call::<i64>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::USize => {
      let value = unsafe { symbol.cif.call::<usize>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::ISize => {
      let value = unsafe { symbol.cif.call::<isize>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::F32 => {
      let value = unsafe { symbol.cif.call::<f32>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::F64 => {
      let value = unsafe { symbol.cif.call::<f64>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::Pointer => {
      let value =
        unsafe { symbol.cif.call::<*const u8>(symbol.ptr, &call_args) } as u64;
      let ptr = U32x2::from(value);
      serde_v8::to_v8(scope, ptr).unwrap()
    }
  };
  rv.set(value);
}

pub fn op_ffi_call_fn(
  scope: &mut deno_core::v8::HandleScope,
  args: deno_core::v8::FunctionCallbackArguments,
  mut rv: deno_core::v8::ReturnValue,
) {
  // SAFETY: deno_core guarantees args.data() is a v8 External pointing to an OpCtx for the isolates lifetime
  let ctx = unsafe {
    &*(v8::Local::<v8::External>::cast(args.data().unwrap_unchecked()).value()
      as *const deno_core::_ops::OpCtx)
  };
  let rid = args.get(0).uint32_value(scope).unwrap();
  let symbol = args.get(1).to_string(scope).unwrap();
  let symbol = symbol.to_rust_string_lossy(scope);

  let state = ctx.state.borrow();
  let resource = state
    .resource_table
    .get::<DynamicLibraryResource>(rid)
    .unwrap();

  let symbol = resource
    .symbols
    .get(&symbol)
    .ok_or_else(bad_resource_id)
    .unwrap();

  let args = v8::Local::<v8::Array>::try_from(args.get(2)).unwrap();
  let args_len = args.length() as usize;
  let mut call_args = Vec::with_capacity(args_len);
  for idx in 0..args_len {
    let arg = args.get_index(scope, idx as u32).unwrap();
    call_args
      .push(value_as_arg(scope, arg, symbol.parameter_types[idx]).unwrap());
  }

  let value = match symbol.result_type {
    NativeType::Void => {
      unsafe { symbol.cif.call::<()>(symbol.ptr, &call_args) };
      return;
    }
    NativeType::U8 => {
      let value = unsafe { symbol.cif.call::<u8>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::I8 => {
      let value = unsafe { symbol.cif.call::<i8>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::U16 => {
      let value = unsafe { symbol.cif.call::<u16>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::I16 => {
      let value = unsafe { symbol.cif.call::<i16>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::U32 => {
      let value = unsafe { symbol.cif.call::<u32>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::I32 => {
      let value = unsafe { symbol.cif.call::<i32>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::U64 => {
      let value = unsafe { symbol.cif.call::<u64>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::I64 => {
      let value = unsafe { symbol.cif.call::<i64>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::USize => {
      let value = unsafe { symbol.cif.call::<usize>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::ISize => {
      let value = unsafe { symbol.cif.call::<isize>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::F32 => {
      let value = unsafe { symbol.cif.call::<f32>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::F64 => {
      let value = unsafe { symbol.cif.call::<f64>(symbol.ptr, &call_args) };
      serde_v8::to_v8(scope, value).unwrap()
    }
    NativeType::Pointer => {
      let value =
        unsafe { symbol.cif.call::<*const u8>(symbol.ptr, &call_args) } as u64;
      let ptr = U32x2::from(value);
      serde_v8::to_v8(scope, ptr).unwrap()
    }
  };
  rv.set(value);
}

pub fn op_ffi_call_ptr_nonblocking_fn(
  scope: &mut deno_core::v8::HandleScope,
  args: deno_core::v8::FunctionCallbackArguments,
  mut rv: deno_core::v8::ReturnValue,
) {
  // SAFETY: deno_core guarantees args.data() is a v8 External pointing to an OpCtx for the isolates lifetime
  let ctx = unsafe {
    &*(v8::Local::<v8::External>::cast(args.data().unwrap_unchecked()).value()
      as *const deno_core::_ops::OpCtx)
  };
  let op_id = ctx.id;
  let promise_id = args.get(0).int32_value(scope).unwrap();

  let state = ctx.state.clone();

  // Track async call & get copy of get_error_class_fn
  let get_class = {
    let state = state.borrow();
    state.tracker.track_async(op_id);
    state.get_error_class_fn
  };

  let pointer: U32x2 = serde_v8::from_v8(scope, args.get(1)).unwrap();
  let fn_def: ForeignFunction = serde_v8::from_v8(scope, args.get(2)).unwrap();
  let symbol = get_symbol(pointer.into(), fn_def);

  let args = v8::Local::<v8::Array>::try_from(args.get(3)).unwrap();
  let args_len = args.length() as usize;
  let mut call_args = Vec::with_capacity(args_len);
  for idx in 0..args_len {
    let arg = args.get_index(scope, idx as u32).unwrap();
    call_args
      .push(value_as_arg(scope, arg, symbol.parameter_types[idx]).unwrap());
  }
  deno_core::_ops::queue_async_op(scope, async move {
    match symbol.result_type {
      NativeType::Void => {
        unsafe { symbol.cif.call::<()>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(().into()))
      }
      NativeType::U8 => {
        let value = unsafe { symbol.cif.call::<u8>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::I8 => {
        let value = unsafe { symbol.cif.call::<i8>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::U16 => {
        let value = unsafe { symbol.cif.call::<u16>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::I16 => {
        let value = unsafe { symbol.cif.call::<i16>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::U32 => {
        let value = unsafe { symbol.cif.call::<u32>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::I32 => {
        let value = unsafe { symbol.cif.call::<i32>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::U64 => {
        let value = unsafe { symbol.cif.call::<u64>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::I64 => {
        let value = unsafe { symbol.cif.call::<i64>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::USize => {
        let value = unsafe { symbol.cif.call::<usize>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::ISize => {
        let value = unsafe { symbol.cif.call::<isize>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::F32 => {
        let value = unsafe { symbol.cif.call::<f32>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::F64 => {
        let value = unsafe { symbol.cif.call::<f64>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::Pointer => {
        let value =
          unsafe { symbol.cif.call::<*const u8>(symbol.ptr, &call_args) }
            as u64;
        let ptr = U32x2::from(value);
        (promise_id, op_id, deno_core::OpResult::Ok(ptr.into()))
      }
    }
  });
}

/// A non-blocking FFI call.
pub fn op_ffi_call_nonblocking_fn(
  scope: &mut deno_core::v8::HandleScope,
  args: deno_core::v8::FunctionCallbackArguments,
  mut rv: deno_core::v8::ReturnValue,
) {
  // SAFETY: deno_core guarantees args.data() is a v8 External pointing to an OpCtx for the isolates lifetime
  let ctx = unsafe {
    &*(v8::Local::<v8::External>::cast(args.data().unwrap_unchecked()).value()
      as *const deno_core::_ops::OpCtx)
  };
  let op_id = ctx.id;
  let promise_id = args.get(0).int32_value(scope).unwrap();
  let rid = args.get(1).uint32_value(scope).unwrap();
  let symbol = args.get(2).to_string(scope).unwrap();
  let symbol = symbol.to_rust_string_lossy(scope);

  let state = ctx.state.clone();

  // Track async call & get copy of get_error_class_fn
  let get_class = {
    let state = state.borrow();
    state.tracker.track_async(op_id);
    state.get_error_class_fn
  };
  let state = state.borrow();
  let resource = state
    .resource_table
    .get::<DynamicLibraryResource>(rid)
    .unwrap();
  let symbols = resource.symbols.clone();
  let symbol = symbols
    .get(&symbol)
    .ok_or_else(bad_resource_id)
    .unwrap()
    .clone();

  let args = v8::Local::<v8::Array>::try_from(args.get(3)).unwrap();
  let args_len = args.length() as usize;
  let mut call_args = Vec::with_capacity(args_len);
  for idx in 0..args_len {
    let arg = args.get_index(scope, idx as u32).unwrap();
    call_args
      .push(value_as_arg(scope, arg, symbol.parameter_types[idx]).unwrap());
  }
  deno_core::_ops::queue_async_op(scope, async move {
    match symbol.result_type {
      NativeType::Void => {
        unsafe { symbol.cif.call::<()>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(().into()))
      }
      NativeType::U8 => {
        let value = unsafe { symbol.cif.call::<u8>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::I8 => {
        let value = unsafe { symbol.cif.call::<i8>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::U16 => {
        let value = unsafe { symbol.cif.call::<u16>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::I16 => {
        let value = unsafe { symbol.cif.call::<i16>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::U32 => {
        let value = unsafe { symbol.cif.call::<u32>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::I32 => {
        let value = unsafe { symbol.cif.call::<i32>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::U64 => {
        let value = unsafe { symbol.cif.call::<u64>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::I64 => {
        let value = unsafe { symbol.cif.call::<i64>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::USize => {
        let value = unsafe { symbol.cif.call::<usize>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::ISize => {
        let value = unsafe { symbol.cif.call::<isize>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::F32 => {
        let value = unsafe { symbol.cif.call::<f32>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::F64 => {
        let value = unsafe { symbol.cif.call::<f64>(symbol.ptr, &call_args) };
        (promise_id, op_id, deno_core::OpResult::Ok(value.into()))
      }
      NativeType::Pointer => {
        let value =
          unsafe { symbol.cif.call::<*const u8>(symbol.ptr, &call_args) }
            as u64;
        let ptr = U32x2::from(value);
        (promise_id, op_id, deno_core::OpResult::Ok(ptr.into()))
      }
    }
  });
}

impl_ffi_op!(op_ffi_call, op_ffi_call_fn, false);
impl_ffi_op!(op_ffi_call_ptr, op_ffi_call_ptr_fn, false);
impl_ffi_op!(op_ffi_call_nonblocking, op_ffi_call_nonblocking_fn, true);
impl_ffi_op!(
  op_ffi_call_ptr_nonblocking,
  op_ffi_call_ptr_nonblocking_fn,
  true
);

#[op]
fn op_ffi_ptr_of<FP>(
  state: &mut deno_core::OpState,
  buf: ZeroCopyBuf,
) -> Result<U32x2, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointer#of");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(U32x2::from(buf.as_ptr() as u64))
}

#[op]
fn op_ffi_buf_copy_into<FP>(
  state: &mut deno_core::OpState,
  (src, mut dst, len): (U32x2, ZeroCopyBuf, usize),
) -> Result<(), AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#copyInto");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  if dst.len() < len {
    Err(range_error(
      "Destination length is smaller than source length",
    ))
  } else {
    let src = u64::from(src) as *const u8;
    unsafe { ptr::copy(src, dst.as_mut_ptr(), len) };
    Ok(())
  }
}

#[op]
fn op_ffi_cstr_read<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
) -> Result<String, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getCString");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  let ptr = u64::from(ptr) as *const c_char;
  Ok(unsafe { CStr::from_ptr(ptr) }.to_str()?.to_string())
}

#[op]
fn op_ffi_read_u8<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
) -> Result<u8, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getUint8");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(u64::from(ptr) as *const u8) })
}

#[op]
fn op_ffi_read_i8<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
) -> Result<i8, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getInt8");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(u64::from(ptr) as *const i8) })
}

#[op]
fn op_ffi_read_u16<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
) -> Result<u16, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getUint16");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(u64::from(ptr) as *const u16) })
}

#[op]
fn op_ffi_read_i16<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
) -> Result<i16, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getInt16");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(u64::from(ptr) as *const i16) })
}

#[op]
fn op_ffi_read_u32<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
) -> Result<u32, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getUint32");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(u64::from(ptr) as *const u32) })
}

#[op]
fn op_ffi_read_i32<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
) -> Result<i32, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getInt32");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(u64::from(ptr) as *const i32) })
}

#[op]
fn op_ffi_read_u64<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
) -> Result<U32x2, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getBigUint64");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(U32x2::from(unsafe {
    ptr::read_unaligned(u64::from(ptr) as *const u64)
  }))
}

#[op]
fn op_ffi_read_f32<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
) -> Result<f32, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getFloat32");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(u64::from(ptr) as *const f32) })
}

#[op]
fn op_ffi_read_f64<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
) -> Result<f64, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getFloat64");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(u64::from(ptr) as *const f64) })
}

#[cfg(test)]
mod tests {
  #[cfg(target_os = "windows")]
  #[test]
  fn test_format_error() {
    use super::format_error;

    // BAD_EXE_FORMAT
    let err = dlopen::Error::OpeningLibraryError(
      std::io::Error::from_raw_os_error(0x000000C1),
    );
    assert_eq!(
      format_error(err, "foo.dll".to_string()),
      "foo.dll is not a valid Win32 application.\r\n".to_string(),
    );
  }
}
