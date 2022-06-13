// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use core::ptr::NonNull;
use deno_core::anyhow::anyhow;
use deno_core::error::generic_error;
use deno_core::error::range_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::Future;
use deno_core::include_js_files;
use deno_core::op;

use deno_core::serde_json::json;
use deno_core::serde_json::Value;
//use deno_core::serde_json::json;
//use deno_core::serde_json::Value;
use deno_core::serde_v8;
use deno_core::v8;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use dlopen::raw::Library;
use libffi::middle::Arg;
use libffi::middle::Cif;
use libffi::raw::*;
use serde::Deserialize;
use serde::Serialize;
//use serde::Serialize;
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

thread_local! {
  static IS_EVENT_LOOP_THREAD: RefCell<bool> = RefCell::new(false);
}

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

#[derive(Clone)]
struct PtrSymbol {
  cif: libffi::middle::Cif,
  ptr: libffi::middle::CodePtr,
}

impl PtrSymbol {
  fn new(fn_ptr: u64, def: &ForeignFunction) -> Self {
    let ptr = libffi::middle::CodePtr::from_ptr(fn_ptr as _);
    let cif = libffi::middle::Cif::new(
      def
        .parameters
        .clone()
        .into_iter()
        .map(libffi::middle::Type::from),
      def.result.into(),
    );

    Self { cif, ptr }
  }
}

#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl Send for PtrSymbol {}
unsafe impl Sync for PtrSymbol {}

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
    IS_EVENT_LOOP_THREAD.with(|s| s.replace(true));
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
      op_ffi_call_ptr::decl::<P>(),
      op_ffi_call_ptr_nonblocking::decl::<P>(),
      op_ffi_ptr_of::decl::<P>(),
      op_ffi_ptr_of_cb::decl::<P>(),
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
      op_ffi_register_callback::decl(),
      op_ffi_deregister_callback::decl(),
      op_ffi_call_registered_callback::decl(),
    ])
    .state(move |state| {
      // Stolen from deno_webgpu, is there a better option?
      state.put(Unstable(unstable));
      Ok(())
    })
    .build()
}

/// Defines the accepted types that can be used as
/// parameters and return values in FFI.
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
  Function {},
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
      NativeType::Function {} => libffi::middle::Type::pointer(),
    }
  }
}

/// Intermediate format for easy translation from NativeType + V8 value
/// to libffi argument types.
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
  pointer: *const u8,
}

impl NativeValue {
  unsafe fn as_arg(&self, native_type: NativeType) -> Arg {
    match native_type {
      NativeType::Void => unreachable!(),
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
      NativeType::Pointer | NativeType::Function {} => Arg::new(&self.pointer),
    }
  }

  fn to_value(&self, native_type: NativeType) -> Value {
    match native_type {
      NativeType::Void => Value::Null,
      NativeType::U8 => Value::from(unsafe { self.u8_value }),
      NativeType::I8 => Value::from(unsafe { self.i8_value }),
      NativeType::U16 => Value::from(unsafe { self.u16_value }),
      NativeType::I16 => Value::from(unsafe { self.i16_value }),
      NativeType::U32 => Value::from(unsafe { self.u32_value }),
      NativeType::I32 => Value::from(unsafe { self.i32_value }),
      NativeType::U64 => {
        json!(U32x2::from(unsafe { self.u64_value }))
      }
      NativeType::I64 => {
        json!(U32x2::from(unsafe { self.i64_value } as u64))
      }
      NativeType::USize => {
        json!(U32x2::from(unsafe { self.usize_value } as u64))
      }
      NativeType::ISize => {
        json!(U32x2::from(unsafe { self.isize_value } as u64))
      }
      NativeType::F32 => Value::from(unsafe { self.f32_value }),
      NativeType::F64 => Value::from(unsafe { self.f64_value }),
      NativeType::Pointer | NativeType::Function {} => {
        json!(U32x2::from(unsafe { self.pointer } as u64))
      }
    }
  }

  fn to_v8<'scope>(
    &self,
    scope: &mut v8::HandleScope<'scope>,
    native_type: NativeType,
  ) -> serde_v8::Value<'scope> {
    match native_type {
      NativeType::Void => {
        let local_value: v8::Local<v8::Value> = v8::undefined(scope).into();
        local_value.into()
      }
      NativeType::U8 => {
        let local_value: v8::Local<v8::Value> =
          v8::Number::new(scope, unsafe { self.u8_value } as f64).into();
        local_value.into()
      }
      NativeType::I8 => {
        let local_value: v8::Local<v8::Value> =
          v8::Number::new(scope, unsafe { self.i8_value } as f64).into();
        local_value.into()
      }
      NativeType::U16 => {
        let local_value: v8::Local<v8::Value> =
          v8::Number::new(scope, unsafe { self.u16_value } as f64).into();
        local_value.into()
      }
      NativeType::I16 => {
        let local_value: v8::Local<v8::Value> =
          v8::Number::new(scope, unsafe { self.i16_value } as f64).into();
        local_value.into()
      }
      NativeType::U32 => {
        let local_value: v8::Local<v8::Value> =
          v8::Number::new(scope, unsafe { self.u32_value } as f64).into();
        local_value.into()
      }
      NativeType::I32 => {
        let local_value: v8::Local<v8::Value> =
          v8::Number::new(scope, unsafe { self.i32_value } as f64).into();
        local_value.into()
      }
      NativeType::U64 => {
        let local_value: v8::Local<v8::Value> =
          v8::BigInt::new_from_u64(scope, unsafe { self.u64_value }).into();
        local_value.into()
      }
      NativeType::I64 => {
        let local_value: v8::Local<v8::Value> =
          v8::BigInt::new_from_i64(scope, unsafe { self.i64_value }).into();
        local_value.into()
      }
      NativeType::USize => {
        let local_value: v8::Local<v8::Value> =
          v8::BigInt::new_from_u64(scope, unsafe { self.usize_value } as u64)
            .into();
        local_value.into()
      }
      NativeType::ISize => {
        let local_value: v8::Local<v8::Value> =
          v8::BigInt::new_from_i64(scope, unsafe { self.isize_value } as i64)
            .into();
        local_value.into()
      }
      NativeType::F32 => {
        let local_value: v8::Local<v8::Value> =
          v8::Number::new(scope, unsafe { self.f32_value } as f64).into();
        local_value.into()
      }
      NativeType::F64 => {
        let local_value: v8::Local<v8::Value> =
          v8::Number::new(scope, unsafe { self.f64_value }).into();
        local_value.into()
      }
      NativeType::Pointer | NativeType::Function {} => {
        let local_value: v8::Local<v8::Value> =
          v8::BigInt::new_from_u64(scope, unsafe { self.pointer } as u64)
            .into();
        local_value.into()
      }
    }
  }
}

unsafe impl Send for NativeValue {}

#[derive(Serialize, Debug, Clone, Copy)]
struct U32x2(u32, u32);

impl From<u64> for U32x2 {
  fn from(value: u64) -> Self {
    Self((value >> 32) as u32, value as u32)
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

fn ffi_parse_args<'scope>(
  scope: &mut v8::HandleScope<'scope>,
  args: serde_v8::Value<'scope>,
  parameter_types: &[NativeType],
  resource_table: &deno_core::ResourceTable,
) -> Result<Vec<NativeValue>, AnyError>
where
  'scope: 'scope,
{
  if parameter_types.is_empty() {
    return Ok(vec![]);
  }

  let args = v8::Local::<v8::Array>::try_from(args.v8_value)
    .map_err(|_| type_error("Invalid FFI parameters, expected Array"))?;
  let mut ffi_args: Vec<NativeValue> =
    Vec::with_capacity(parameter_types.len());

  for (index, native_type) in parameter_types.iter().enumerate() {
    let value = args.get_index(scope, index as u32).unwrap();
    match native_type {
      NativeType::Void => {
        unreachable!();
      }
      NativeType::U8 => {
        let value = value
          .number_value(scope)
          .ok_or_else(|| type_error("Invalid FFI u8 type, expected number"))?
          as u8;
        ffi_args.push(NativeValue { u8_value: value });
      }
      NativeType::I8 => {
        let value = value
          .number_value(scope)
          .ok_or_else(|| type_error("Invalid FFI i8 type, expected number"))?
          as i8;
        ffi_args.push(NativeValue { i8_value: value });
      }
      NativeType::U16 => {
        let value = value
          .number_value(scope)
          .ok_or_else(|| type_error("Invalid FFI u16 type, expected number"))?
          as u16;
        ffi_args.push(NativeValue { u16_value: value });
      }
      NativeType::I16 => {
        let value = value
          .number_value(scope)
          .ok_or_else(|| type_error("Invalid FFI i16 type, expected number"))?
          as i16;
        ffi_args.push(NativeValue { i16_value: value });
      }
      NativeType::U32 => {
        let value = value
          .number_value(scope)
          .ok_or_else(|| type_error("Invalid FFI u32 type, expected number"))?
          as u32;
        ffi_args.push(NativeValue { u32_value: value });
      }
      NativeType::I32 => {
        let value = value
          .number_value(scope)
          .ok_or_else(|| type_error("Invalid FFI i32 type, expected number"))?
          as i32;
        ffi_args.push(NativeValue { i32_value: value });
      }
      NativeType::U64 => {
        let value: u64 = if value.is_big_int() {
          let value = v8::Local::<v8::BigInt>::try_from(value)?;
          value.u64_value().0
        } else {
          value.integer_value(scope).ok_or_else(|| {
            type_error("Invalid FFI u64 type, expected number")
          })? as u64
        };
        ffi_args.push(NativeValue { u64_value: value });
      }
      NativeType::I64 => {
        let value: i64 = if value.is_big_int() {
          let value = v8::Local::<v8::BigInt>::try_from(value)?;
          value.i64_value().0
        } else {
          value.integer_value(scope).ok_or_else(|| {
            type_error("Invalid FFI i64 type, expected number")
          })? as i64
        };
        ffi_args.push(NativeValue { i64_value: value });
      }
      NativeType::USize => {
        let value: usize = if value.is_big_int() {
          let value = v8::Local::<v8::BigInt>::try_from(value)?;
          value.u64_value().0 as usize
        } else {
          value.integer_value(scope).ok_or_else(|| {
            type_error("Invalid FFI usize type, expected number")
          })? as usize
        };
        ffi_args.push(NativeValue { usize_value: value });
      }
      NativeType::ISize => {
        let value: isize = if value.is_big_int() {
          let value = v8::Local::<v8::BigInt>::try_from(value)?;
          value.i64_value().0 as isize
        } else {
          value.integer_value(scope).ok_or_else(|| {
            type_error("Invalid FFI isize type, expected number")
          })? as isize
        };
        ffi_args.push(NativeValue { isize_value: value });
      }
      NativeType::F32 => {
        let value = value
          .number_value(scope)
          .ok_or_else(|| type_error("Invalid FFI f32 type, expected number"))?
          as f32;
        ffi_args.push(NativeValue { f32_value: value });
      }
      NativeType::F64 => {
        let value = value
          .number_value(scope)
          .ok_or_else(|| type_error("Invalid FFI f64 type, expected number"))?
          as f64;
        ffi_args.push(NativeValue { f64_value: value });
      }
      NativeType::Pointer => {
        if value.is_null() {
          let value: *const u8 = ptr::null();
          ffi_args.push(NativeValue { pointer: value })
        } else if value.is_big_int() {
          let value = v8::Local::<v8::BigInt>::try_from(value).unwrap();
          let value = value.u64_value().0 as *const u8;
          ffi_args.push(NativeValue { pointer: value });
        } else if value.is_array_buffer() || value.is_array_buffer_view() {
          let value: ZeroCopyBuf = serde_v8::from_v8(scope, value)?;
          let value: &[u8] = &value[..];
          ffi_args.push(NativeValue {
            pointer: value.as_ptr(),
          });
        } else {
          return Err(type_error("Invalid FFI pointer type, expected null, BigInt, ArrayBuffer, or ArrayBufferView"));
        }
      }
      NativeType::Function {} => {
        if value.is_null() {
          let value: *const u8 = ptr::null();
          ffi_args.push(NativeValue { pointer: value })
        } else if value.is_number() {
          let value: ResourceId = value.number_value(scope).unwrap() as u32;
          let rc = resource_table.get::<RegisteredCallbackResource>(value)?;
          let function = *rc.closure.code_ptr();
          ffi_args.push(NativeValue {
            pointer: function as usize as *const u8,
          });
        } else if value.is_big_int() {
          // Do we support this? This would be a foreign function pointer given to us by an FFI library.
          let value = v8::Local::<v8::BigInt>::try_from(value).unwrap();
          let value = value.u64_value().0 as *const u8;
          ffi_args.push(NativeValue { pointer: value });
        } else {
          return Err(type_error("Invalid FFI function type, expected null, RegisteredCallback, or BigInt"));
        }
      }
    }
  }

  Ok(ffi_args)
}

fn ffi_call(
  call_args: Vec<NativeValue>,
  cif: &libffi::middle::Cif,
  fun_ptr: libffi::middle::CodePtr,
  parameter_types: &[NativeType],
  result_type: NativeType,
) -> Result<NativeValue, AnyError> {
  let call_args: Vec<Arg> = call_args
    .iter()
    .enumerate()
    .map(|(index, ffi_arg)| unsafe {
      ffi_arg.as_arg(*parameter_types.get(index).unwrap())
    })
    .collect();

  Ok(match result_type {
    NativeType::Void => NativeValue {
      void_value: unsafe { cif.call::<()>(fun_ptr, &call_args) },
    },
    NativeType::U8 => NativeValue {
      u8_value: unsafe { cif.call::<u8>(fun_ptr, &call_args) },
    },
    NativeType::I8 => NativeValue {
      i8_value: unsafe { cif.call::<i8>(fun_ptr, &call_args) },
    },
    NativeType::U16 => NativeValue {
      u16_value: unsafe { cif.call::<u16>(fun_ptr, &call_args) },
    },
    NativeType::I16 => NativeValue {
      i16_value: unsafe { cif.call::<i16>(fun_ptr, &call_args) },
    },
    NativeType::U32 => NativeValue {
      u32_value: unsafe { cif.call::<u32>(fun_ptr, &call_args) },
    },
    NativeType::I32 => NativeValue {
      i32_value: unsafe { cif.call::<i32>(fun_ptr, &call_args) },
    },
    NativeType::U64 => NativeValue {
      u64_value: unsafe { cif.call::<u64>(fun_ptr, &call_args) },
    },
    NativeType::I64 => NativeValue {
      i64_value: unsafe { cif.call::<i64>(fun_ptr, &call_args) },
    },
    NativeType::USize => NativeValue {
      usize_value: unsafe { cif.call::<usize>(fun_ptr, &call_args) },
    },
    NativeType::ISize => NativeValue {
      isize_value: unsafe { cif.call::<isize>(fun_ptr, &call_args) },
    },
    NativeType::F32 => NativeValue {
      f32_value: unsafe { cif.call::<f32>(fun_ptr, &call_args) },
    },
    NativeType::F64 => NativeValue {
      f64_value: unsafe { cif.call::<f64>(fun_ptr, &call_args) },
    },
    NativeType::Pointer | NativeType::Function {} => NativeValue {
      pointer: unsafe { cif.call::<*const u8>(fun_ptr, &call_args) },
    },
  })
}

struct RegisteredCallbackResource {
  closure: libffi::middle::Closure<'static>,
  info: *const CallbackInfo,
}

impl Resource for RegisteredCallbackResource {
  fn name(&self) -> Cow<str> {
    "registeredcallback".into()
  }

  fn close(self: Rc<Self>) {
    let info = unsafe { Box::from_raw(self.info as *mut CallbackInfo) };
    let isolate = unsafe { info.isolate.as_mut().unwrap() };
    unsafe { v8::Global::from_raw(isolate, info.callback) };
    unsafe { v8::Global::from_raw(isolate, info.context) };
    drop(self)
  }
}

struct CallbackInfo {
  pub callback: NonNull<v8::Function>,
  pub context: NonNull<v8::Context>,
  pub isolate: *mut v8::Isolate,
  pub parameters: Vec<NativeType>,
  pub result: NativeType,
}

unsafe extern "C" fn deno_ffi_callback(
  cif: &libffi::low::ffi_cif,
  result: &mut c_void,
  args: *const *const c_void,
  info: &CallbackInfo,
) {
  let isolate = &mut *info.isolate;
  let callback = v8::Global::from_raw(isolate, info.callback);
  let context = std::mem::transmute::<
    NonNull<v8::Context>,
    v8::Local<v8::Context>,
  >(info.context);
  IS_EVENT_LOOP_THREAD.with(|is_event_loop_thread| {
    if !(*is_event_loop_thread.borrow()) {
      // Call from another thread, PANIC IN THE DISCO!
      todo!("Call from another thread");
    }
  });
  // Call from main thread. If this callback is being triggered due to a
  // function call coming from Deno itself, then this callback will build
  // ontop of that stack.
  // If this callback is being triggered outside of Deno (for example from a
  // signal handler) then this will either create an empty new stack if
  // Deno currently has nothing running and is waiting for promises to resolve,
  // or will (very incorrectly) build ontop of whatever stack exists.
  // The callback will even be called through from a `while (true)` liveloop, but
  // it somehow cannot change the values that the loop sees, even if they both
  // refer the same `let bool_value`.
  let mut cb_scope = v8::CallbackScope::new(context);
  let mut scope = v8::TryCatch::new(&mut cb_scope);
  let func = callback.open(&mut scope);
  let result = result as *mut c_void;
  let repr: &[*mut ffi_type] =
    std::slice::from_raw_parts(cif.arg_types, cif.nargs as usize);
  let vals: &[*const c_void] =
    std::slice::from_raw_parts(args, cif.nargs as usize);

  let mut params: Vec<v8::Local<v8::Value>> = vec![];
  for (repr, val) in repr.iter().zip(vals) {
    let value = match (*(*repr)).type_ as _ {
      FFI_TYPE_INT => serde_v8::to_v8(&mut scope, *((*val) as *const i32)),
      FFI_TYPE_FLOAT => serde_v8::to_v8(&mut scope, *((*val) as *const f32)),
      FFI_TYPE_DOUBLE => serde_v8::to_v8(&mut scope, *((*val) as *const f64)),
      FFI_TYPE_SINT8 => serde_v8::to_v8(&mut scope, *((*val) as *const i8)),
      FFI_TYPE_UINT8 => serde_v8::to_v8(&mut scope, *((*val) as *const u8)),
      FFI_TYPE_SINT16 => serde_v8::to_v8(&mut scope, *((*val) as *const i16)),
      FFI_TYPE_UINT16 => serde_v8::to_v8(&mut scope, *((*val) as *const u16)),
      FFI_TYPE_SINT32 => serde_v8::to_v8(&mut scope, *((*val) as *const i32)),
      FFI_TYPE_UINT32 => serde_v8::to_v8(&mut scope, *((*val) as *const u32)),
      FFI_TYPE_SINT64 => {
        let result = *((*val) as *const i64);
        Ok(v8::BigInt::new_from_i64(&mut scope, result).into())
      }
      FFI_TYPE_POINTER | FFI_TYPE_STRUCT | FFI_TYPE_UINT64 => {
        let result = *((*val) as *const u64);
        Ok(v8::BigInt::new_from_u64(&mut scope, result).into())
      }
      FFI_TYPE_VOID => serde_v8::to_v8(&mut scope, ()),
      _ => {
        unreachable!()
      }
    };
    params.push(value.expect("Unable to serialize callback parameter."));
  }

  let recv = v8::undefined(&mut scope);
  let call_result = func.call(&mut scope, recv.into(), &params);
  std::mem::forget(callback);

  if call_result.is_none() {
    // JS function threw an exception. Set the return value to zero and return.
    match (*cif.rtype).type_ as _ {
      FFI_TYPE_INT | FFI_TYPE_SINT32 | FFI_TYPE_UINT32 => {
        // zero is equal for signed and unsigned alike
        *(result as *mut u32) = 0;
      }
      FFI_TYPE_FLOAT => {
        *(result as *mut f32) = 0.0;
      }
      FFI_TYPE_DOUBLE => {
        *(result as *mut f64) = 0.0;
      }
      FFI_TYPE_SINT8 | FFI_TYPE_UINT8 => {
        // zero is equal for signed and unsigned alike
        *(result as *mut u8) = 0;
      }
      FFI_TYPE_SINT16 | FFI_TYPE_UINT16 => {
        // zero is equal for signed and unsigned alike
        *(result as *mut u16) = 0;
      }
      FFI_TYPE_POINTER | FFI_TYPE_STRUCT | FFI_TYPE_UINT64
      | FFI_TYPE_SINT64 => {
        *(result as *mut u64) = 0;
      }
      FFI_TYPE_VOID => {
        // nop
      }
      _ => {
        unreachable!();
      }
    };

    if scope.has_caught() {
      // Rethrow exception to pass it up the call chain, if one exists.
      scope.rethrow();
    }

    return;
  }
  let value = call_result.unwrap();

  match (*cif.rtype).type_ as _ {
    FFI_TYPE_INT | FFI_TYPE_SINT32 => {
      *(result as *mut i32) = value
        .number_value(&mut scope)
        .expect("Unable to deserialize result parameter.")
        as i32;
    }
    FFI_TYPE_FLOAT => {
      *(result as *mut f32) = value
        .number_value(&mut scope)
        .expect("Unable to deserialize result parameter.")
        as f32;
    }
    FFI_TYPE_DOUBLE => {
      *(result as *mut f64) = value
        .number_value(&mut scope)
        .expect("Unable to deserialize result parameter.");
    }
    FFI_TYPE_POINTER | FFI_TYPE_STRUCT => {
      if value.is_array_buffer() | value.is_array_buffer_view() {
        let value: ZeroCopyBuf = serde_v8::from_v8(&mut scope, value)
          .expect("Unable to deserialize result parameter.");
        let value: &[u8] = &value[..];
        *(result as *mut *const u8) = value.as_ptr();
      } else if value.is_big_int() | value.is_big_int_object() {
        let value = v8::Local::<v8::BigInt>::try_from(value).unwrap();
        *(result as *mut u64) = value.u64_value().0;
      } else if value.is_null() {
        *(result as *mut *const c_void) = ptr::null();
      } else if value.is_object() {
        let value = v8::Local::<v8::Object>::try_from(value).unwrap();
        let value_key = v8::String::new(&mut scope, "value").unwrap();
        let value_field = value
          .get(&mut scope, value_key.into())
          .expect("Unable to deserialize result parameter.");
        let value = v8::Local::<v8::BigInt>::try_from(value_field)
          .expect("Unable to deserialize result parameter.");
        *(result as *mut u64) = value.u64_value().0;
      } else {
        // Fallthrough: Probably someone returned a number but this could
        // also be eg. a string. This is essentially UB.
        *(result as *mut u64) = value
          .number_value(&mut scope)
          .expect("Unable to deserialize result parameter.")
          as u64;
      }
    }
    FFI_TYPE_SINT8 => {
      *(result as *mut i8) = value
        .number_value(&mut scope)
        .expect("Unable to deserialize result parameter.")
        as i8;
    }
    FFI_TYPE_UINT8 => {
      *(result as *mut u8) = value
        .number_value(&mut scope)
        .expect("Unable to deserialize result parameter.")
        as u8;
    }
    FFI_TYPE_SINT16 => {
      *(result as *mut i16) = value
        .number_value(&mut scope)
        .expect("Unable to deserialize result parameter.")
        as i16;
    }
    FFI_TYPE_UINT16 => {
      *(result as *mut u16) = value
        .number_value(&mut scope)
        .expect("Unable to deserialize result parameter.")
        as u16;
    }
    FFI_TYPE_UINT32 => {
      *(result as *mut u32) = value
        .number_value(&mut scope)
        .expect("Unable to deserialize result parameter.")
        as u32;
    }
    FFI_TYPE_SINT64 => {
      if value.is_big_int() | value.is_big_int_object() {
        let value = v8::Local::<v8::BigInt>::try_from(value).unwrap();
        *(result as *mut u64) = value.u64_value().0;
      } else {
        *(result as *mut i64) = value
          .number_value(&mut scope)
          .expect("Unable to deserialize result parameter.")
          as i64;
      }
    }
    FFI_TYPE_UINT64 => {
      *(result as *mut u64) = value
        .number_value(&mut scope)
        .expect("Unable to deserialize result parameter.")
        as u64;
    }
    FFI_TYPE_VOID => {
      // nop
    }
    _ => {
      unreachable!();
    }
  };
}

#[derive(Deserialize)]
struct RegisterCallbackArgs {
  parameters: Vec<NativeType>,
  result: NativeType,
}

#[op(v8)]
fn op_ffi_register_callback(
  state: &mut deno_core::OpState,
  scope: &mut v8::HandleScope,
  args: RegisterCallbackArgs,
  cb: serde_v8::Value<'_>,
) -> Result<(ResourceId, U32x2), AnyError> {
  let v8_value = cb.v8_value;
  let cb = v8::Local::<v8::Function>::try_from(v8_value)?;

  let info = {
    let isolate_ptr: *mut v8::Isolate = {
      let isolate: &mut v8::Isolate = &mut *scope;
      isolate
    };
    let isolate = unsafe { &mut *isolate_ptr };
    let callback = v8::Global::new(isolate, cb).into_raw();
    let context =
      v8::Global::new(isolate, scope.get_current_context()).into_raw();

    Box::leak(Box::new(CallbackInfo {
      callback,
      context,
      isolate,
      parameters: args.parameters.clone(),
      result: args.result,
    }))
  };
  let cif = Cif::new(
    args.parameters.into_iter().map(libffi::middle::Type::from),
    libffi::middle::Type::from(args.result),
  );

  let closure = libffi::middle::Closure::new(cif, deno_ffi_callback, info);
  let u3x2 = U32x2::from(*closure.code_ptr() as usize as u64);

  let resource = RegisteredCallbackResource { closure, info };

  Ok((state.resource_table.add(resource), u3x2))
}

#[op]
fn op_ffi_deregister_callback(
  state: &mut deno_core::OpState,
  rid: ResourceId,
) -> Result<(), AnyError> {
  state
    .resource_table
    .take::<RegisteredCallbackResource>(rid)?
    .close();
  Ok(())
}

#[op(v8)]
fn op_ffi_call_registered_callback<'a>(
  scope: &mut v8::HandleScope<'a>,
  state: Rc<RefCell<deno_core::OpState>>,
  rid: ResourceId,
  args: serde_v8::Value<'a>,
) -> Result<serde_v8::Value<'a>, AnyError>
where
  'a: 'a,
{
  let (fn_ptr, info, call_args) = {
    let state = &mut state.borrow();
    let resource = state
      .resource_table
      .get::<RegisteredCallbackResource>(rid)?;
    let info: &CallbackInfo = unsafe { &*resource.info };

    let fn_ptr = *resource.closure.code_ptr() as *mut c_void;
    let call_args =
      ffi_parse_args(scope, args, &info.parameters, &state.resource_table)?;
    (fn_ptr, info, call_args)
  };
  let cif = libffi::middle::Cif::new(
    info
      .parameters
      .clone()
      .into_iter()
      .map(libffi::middle::Type::from),
    info.result.into(),
  );
  let fun_ptr = libffi::middle::CodePtr::from_ptr(fn_ptr);
  let result =
    ffi_call(call_args, &cif, fun_ptr, &info.parameters, info.result)?;
  Ok(result.to_v8(scope, info.result))
}

#[op(v8)]
fn op_ffi_call_ptr<FP, 'scope>(
  scope: &mut v8::HandleScope<'scope>,
  state: Rc<RefCell<deno_core::OpState>>,
  pointer: u64,
  def: ForeignFunction,
  parameters: serde_v8::Value<'scope>,
) -> Result<serde_v8::Value<'scope>, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable2(&state, "Deno.UnsafeFnPointer#call");

  let symbol = PtrSymbol::new(pointer, &def);
  let call_args = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<FP>();
    permissions.check(None)?;

    ffi_parse_args(scope, parameters, &def.parameters, &state.resource_table)?
  };

  let result = ffi_call(
    call_args,
    &symbol.cif,
    symbol.ptr,
    &def.parameters,
    def.result,
  )?;
  let result = result.to_v8(scope, def.result);
  Ok(result)
}

#[op(v8)]
fn op_ffi_call_ptr_nonblocking<'scope, FP>(
  scope: &mut v8::HandleScope<'scope>,
  state: Rc<RefCell<deno_core::OpState>>,
  pointer: u64,
  def: ForeignFunction,
  parameters: serde_v8::Value<'scope>,
) -> Result<impl Future<Output = Result<Value, AnyError>>, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable2(&state, "Deno.UnsafeFnPointer#call");

  let symbol = PtrSymbol::new(pointer, &def);
  let call_args = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<FP>();
    permissions.check(None)?;

    ffi_parse_args(scope, parameters, &def.parameters, &state.resource_table)?
  };

  let join_handle = tokio::task::spawn_blocking(move || {
    let PtrSymbol { cif, ptr } = symbol.clone();
    ffi_call(call_args, &cif, ptr, &def.parameters, def.result)
  });

  Ok(async move {
    let result = join_handle
      .await
      .map_err(|err| anyhow!("Nonblocking FFI call failed: {}", err))??;
    Ok(result.to_value(def.result))
  })
}

#[op(v8)]
fn op_ffi_get_static<'scope>(
  scope: &mut v8::HandleScope<'scope>,
  state: &mut deno_core::OpState,
  rid: ResourceId,
  name: String,
  static_type: NativeType,
) -> Result<serde_v8::Value<'scope>, AnyError> {
  let resource = state.resource_table.get::<DynamicLibraryResource>(rid)?;

  let data_ptr = resource.get_static(name)? as *const u8;

  Ok(match static_type {
    NativeType::Void => {
      return Err(type_error("Invalid FFI static type 'void'"));
    }
    NativeType::U8 => {
      let result = unsafe { ptr::read_unaligned(data_ptr as *const u8) };
      let number = v8::Number::new(scope, result as f64);
      serde_v8::from_v8(scope, number.into())?
    }
    NativeType::I8 => {
      let result = unsafe { ptr::read_unaligned(data_ptr as *const i8) };
      let number = v8::Number::new(scope, result as f64);
      serde_v8::from_v8(scope, number.into())?
    }
    NativeType::U16 => {
      let result = unsafe { ptr::read_unaligned(data_ptr as *const u16) };
      let number = v8::Number::new(scope, result as f64);
      serde_v8::from_v8(scope, number.into())?
    }
    NativeType::I16 => {
      let result = unsafe { ptr::read_unaligned(data_ptr as *const i16) };
      let number = v8::Number::new(scope, result as f64);
      serde_v8::from_v8(scope, number.into())?
    }
    NativeType::U32 => {
      let result = unsafe { ptr::read_unaligned(data_ptr as *const u32) };
      let number = v8::Number::new(scope, result as f64);
      serde_v8::from_v8(scope, number.into())?
    }
    NativeType::I32 => {
      let result = unsafe { ptr::read_unaligned(data_ptr as *const i32) };
      let number = v8::Number::new(scope, result as f64);
      serde_v8::from_v8(scope, number.into())?
    }
    NativeType::U64 => {
      let result = unsafe { ptr::read_unaligned(data_ptr as *const u64) };
      let big_int = v8::BigInt::new_from_u64(scope, result);
      serde_v8::from_v8(scope, big_int.into())?
    }
    NativeType::I64 => {
      let result = unsafe { ptr::read_unaligned(data_ptr as *const i64) };
      let big_int = v8::BigInt::new_from_i64(scope, result);
      serde_v8::from_v8(scope, big_int.into())?
    }
    NativeType::USize => {
      let result = unsafe { ptr::read_unaligned(data_ptr as *const usize) };
      let big_int = v8::BigInt::new_from_u64(scope, result as u64);
      serde_v8::from_v8(scope, big_int.into())?
    }
    NativeType::ISize => {
      let result = unsafe { ptr::read_unaligned(data_ptr as *const isize) };
      let big_int = v8::BigInt::new_from_i64(scope, result as i64);
      serde_v8::from_v8(scope, big_int.into())?
    }
    NativeType::F32 => {
      let result = unsafe { ptr::read_unaligned(data_ptr as *const f32) };
      let number = v8::Number::new(scope, result as f64);
      serde_v8::from_v8(scope, number.into())?
    }
    NativeType::F64 => {
      let result = unsafe { ptr::read_unaligned(data_ptr as *const f64) };
      let number = v8::Number::new(scope, result as f64);
      serde_v8::from_v8(scope, number.into())?
    }
    NativeType::Pointer | NativeType::Function {} => {
      let result = data_ptr as *const u8 as u64;
      let big_int = v8::BigInt::new_from_u64(scope, result);
      serde_v8::from_v8(scope, big_int.into())?
    }
  })
}

#[op(v8)]
fn op_ffi_call<'scope>(
  scope: &mut v8::HandleScope<'scope>,
  state: Rc<RefCell<deno_core::OpState>>,
  rid: ResourceId,
  symbol: String,
  parameters: serde_v8::Value<'scope>,
) -> Result<serde_v8::Value<'scope>, AnyError> {
  let (symbol, call_args) = {
    let state = &mut state.borrow();
    let resource = state.resource_table.get::<DynamicLibraryResource>(rid)?;

    let symbol = resource
      .symbols
      .get(&symbol)
      .ok_or_else(|| type_error("Invalid FFI symbol name"))?
      .clone();

    let call_args = ffi_parse_args(
      scope,
      parameters,
      &symbol.parameter_types,
      &state.resource_table,
    )?;
    (symbol, call_args)
  };

  let result = ffi_call(
    call_args,
    &symbol.cif,
    symbol.ptr,
    &symbol.parameter_types,
    symbol.result_type,
  )?;
  let result = result.to_v8(scope, symbol.result_type);
  Ok(result)
}

/// A non-blocking FFI call.
#[op(v8)]
fn op_ffi_call_nonblocking<'scope>(
  scope: &mut v8::HandleScope<'scope>,
  state: Rc<RefCell<deno_core::OpState>>,
  rid: ResourceId,
  symbol: String,
  parameters: serde_v8::Value<'scope>,
) -> Result<impl Future<Output = Result<Value, AnyError>> + 'static, AnyError> {
  let (symbol, call_args) = {
    let state = state.borrow();
    let resource = state.resource_table.get::<DynamicLibraryResource>(rid)?;
    let symbols = &resource.symbols;
    let symbol = symbols
      .get(&symbol)
      .ok_or_else(|| type_error("Invalid FFI symbol name"))?
      .clone();
    let call_args = ffi_parse_args(
      scope,
      parameters,
      &symbol.parameter_types,
      &state.resource_table,
    )?;
    (symbol, call_args)
  };

  let result_type = symbol.result_type;
  let join_handle = tokio::task::spawn_blocking(move || {
    let Symbol {
      cif,
      ptr,
      parameter_types,
      ..
    } = symbol.clone();
    ffi_call(call_args, &cif, ptr, &parameter_types, result_type)
  });

  Ok(async move {
    let result = join_handle
      .await
      .map_err(|err| anyhow!("Nonblocking FFI call failed: {}", err))??;
    Ok(result.to_value(result_type))
  })
}

#[op(v8)]
fn op_ffi_ptr_of<FP, 'scope>(
  scope: &mut v8::HandleScope<'scope>,
  state: &mut deno_core::OpState,
  buf: ZeroCopyBuf,
) -> Result<serde_v8::Value<'scope>, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointer#of");
  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  let big_int: v8::Local<v8::Value> =
    v8::BigInt::new_from_u64(scope, buf.as_ptr() as u64).into();
  Ok(big_int.into())
}

#[op(v8)]
fn op_ffi_ptr_of_cb<FP, 'scope>(
  scope: &mut v8::HandleScope<'scope>,
  state: &mut deno_core::OpState,
  rid: ResourceId,
) -> Result<serde_v8::Value<'scope>, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.RegisterableCallback.value");
  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  let resource = state
    .resource_table
    .get::<RegisteredCallbackResource>(rid)?;

  let big_int: v8::Local<v8::Value> = v8::BigInt::new_from_u64(
    scope,
    *resource.closure.code_ptr() as usize as u64,
  )
  .into();
  Ok(big_int.into())
}

#[op]
fn op_ffi_buf_copy_into<FP>(
  state: &mut deno_core::OpState,
  src: u64,
  mut dst: ZeroCopyBuf,
  len: usize,
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
    let src = src as *const u8;
    unsafe { ptr::copy(src, dst.as_mut_ptr(), len) };
    Ok(())
  }
}

#[op]
fn op_ffi_cstr_read<FP>(
  state: &mut deno_core::OpState,
  ptr: u64,
) -> Result<String, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getCString");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  let ptr = ptr as *const c_char;
  Ok(unsafe { CStr::from_ptr(ptr) }.to_str()?.to_string())
}

#[op]
fn op_ffi_read_u8<FP>(
  state: &mut deno_core::OpState,
  ptr: u64,
) -> Result<u8, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getUint8");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(ptr as *const u8) })
}

#[op]
fn op_ffi_read_i8<FP>(
  state: &mut deno_core::OpState,
  ptr: u64,
) -> Result<i8, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getInt8");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(ptr as *const i8) })
}

#[op]
fn op_ffi_read_u16<FP>(
  state: &mut deno_core::OpState,
  ptr: u64,
) -> Result<u16, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getUint16");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(ptr as *const u16) })
}

#[op]
fn op_ffi_read_i16<FP>(
  state: &mut deno_core::OpState,
  ptr: u64,
) -> Result<i16, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getInt16");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(ptr as *const i16) })
}

#[op]
fn op_ffi_read_u32<FP>(
  state: &mut deno_core::OpState,
  ptr: u64,
) -> Result<u32, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getUint32");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(ptr as *const u32) })
}

#[op]
fn op_ffi_read_i32<FP>(
  state: &mut deno_core::OpState,
  ptr: u64,
) -> Result<i32, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getInt32");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(ptr as *const i32) })
}

#[op(v8)]
fn op_ffi_read_u64<FP, 'scope>(
  scope: &mut v8::HandleScope<'scope>,
  state: &mut deno_core::OpState,
  ptr: u64,
) -> Result<serde_v8::Value<'scope>, AnyError>
where
  FP: FfiPermissions + 'static,
  'scope: 'scope,
{
  check_unstable(state, "Deno.UnsafePointerView#getBigUint64");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  let result = unsafe { ptr::read_unaligned(ptr as *const u64) };

  let big_int: v8::Local<v8::Value> =
    v8::BigInt::new_from_u64(scope, result).into();
  Ok(big_int.into())
}

#[op]
fn op_ffi_read_f32<FP>(
  state: &mut deno_core::OpState,
  ptr: u64,
) -> Result<f32, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getFloat32");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(ptr as *const f32) })
}

#[op]
fn op_ffi_read_f64<FP>(
  state: &mut deno_core::OpState,
  ptr: u64,
) -> Result<f64, AnyError>
where
  FP: FfiPermissions + 'static,
{
  check_unstable(state, "Deno.UnsafePointerView#getFloat64");

  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(ptr as *const f64) })
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
