// Copyright 2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::range_error;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
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
      ("op_ffi_call_nonblocking", op_async(op_ffi_call_nonblocking)),
      ("op_ffi_ptr_of", op_sync(op_ffi_ptr_of::<P>)),
      ("op_ffi_buf_copy_into", op_sync(op_ffi_buf_copy_into::<P>)),
      ("op_ffi_cstr_read", op_sync(op_ffi_cstr_read::<P>)),
      ("op_ffi_read_u8", op_sync(op_ffi_read_u8::<P>)),
      ("op_ffi_read_i8", op_sync(op_ffi_read_i8::<P>)),
      ("op_ffi_read_u16", op_sync(op_ffi_read_u16::<P>)),
      ("op_ffi_read_i16", op_sync(op_ffi_read_i16::<P>)),
      ("op_ffi_read_u32", op_sync(op_ffi_read_u32::<P>)),
      ("op_ffi_read_i32", op_sync(op_ffi_read_i32::<P>)),
      ("op_ffi_read_u64", op_sync(op_ffi_read_u64::<P>)),
      ("op_ffi_read_f32", op_sync(op_ffi_read_f32::<P>)),
      ("op_ffi_read_f64", op_sync(op_ffi_read_f64::<P>)),
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
      NativeType::Pointer => {
        if value.is_null() {
          Self {
            pointer: ptr::null(),
          }
        } else {
          Self {
            pointer: u64::from(
              serde_json::from_value::<U32x2>(value)
              .expect("Expected ffi arg value to be a tuple of the low and high bits of a pointer address")
            ) as *const u8,
          }
        }
      }
    }
  }

  fn buffer(ptr: *const u8) -> Self {
    Self { pointer: ptr }
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
      NativeType::Pointer => Arg::new(&self.pointer),
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

#[derive(Serialize, Deserialize, Debug)]
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
  parameters: Vec<NativeType>,
  result: NativeType,
}

#[derive(Deserialize, Debug)]
struct FfiLoadArgs {
  path: String,
  symbols: HashMap<String, ForeignFunction>,
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

fn op_ffi_load<FP>(
  state: &mut deno_core::OpState,
  args: FfiLoadArgs,
  _: (),
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
  buffers: Vec<Option<ZeroCopyBuf>>,
}

fn ffi_call(args: FfiCallArgs, symbol: &Symbol) -> Result<Value, AnyError> {
  let buffers: Vec<Option<&[u8]>> = args
    .buffers
    .iter()
    .map(|buffer| buffer.as_ref().map(|buffer| &buffer[..]))
    .collect();

  let native_values = symbol
    .parameter_types
    .iter()
    .zip(args.parameters.into_iter())
    .map(|(&native_type, value)| {
      if let NativeType::Pointer = native_type {
        if let Some(idx) = value.as_u64() {
          if let Some(&Some(buf)) = buffers.get(idx as usize) {
            return NativeValue::buffer(buf.as_ptr());
          }
        }
      }

      NativeValue::new(native_type, value)
    })
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
    NativeType::Pointer => {
      json!(U32x2::from(unsafe {
        symbol.cif.call::<*const u8>(symbol.ptr, &call_args)
      } as u64))
    }
  })
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

  ffi_call(args, symbol)
}

/// A non-blocking FFI call.
async fn op_ffi_call_nonblocking(
  state: Rc<RefCell<deno_core::OpState>>,
  args: FfiCallArgs,
  _: (),
) -> Result<Value, AnyError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<DynamicLibraryResource>(args.rid)?;
  let symbols = &resource.symbols;
  let symbol = symbols
    .get(&args.symbol)
    .ok_or_else(bad_resource_id)?
    .clone();

  tokio::task::spawn_blocking(move || ffi_call(args, &symbol))
    .await
    .unwrap()
}

fn op_ffi_ptr_of<FP>(
  state: &mut deno_core::OpState,
  buf: ZeroCopyBuf,
  _: (),
) -> Result<U32x2, AnyError>
where
  FP: FfiPermissions + 'static,
{
  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(U32x2::from(buf.as_ptr() as u64))
}

fn op_ffi_buf_copy_into<FP>(
  state: &mut deno_core::OpState,
  (src, mut dst, len): (U32x2, ZeroCopyBuf, usize),
  _: (),
) -> Result<(), AnyError>
where
  FP: FfiPermissions + 'static,
{
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

fn op_ffi_cstr_read<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
  _: (),
) -> Result<String, AnyError>
where
  FP: FfiPermissions + 'static,
{
  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  let ptr = u64::from(ptr) as *const i8;
  Ok(unsafe { CStr::from_ptr(ptr) }.to_str()?.to_string())
}

fn op_ffi_read_u8<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
  _: (),
) -> Result<u8, AnyError>
where
  FP: FfiPermissions + 'static,
{
  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(u64::from(ptr) as *const u8) })
}

fn op_ffi_read_i8<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
  _: (),
) -> Result<i8, AnyError>
where
  FP: FfiPermissions + 'static,
{
  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(u64::from(ptr) as *const i8) })
}

fn op_ffi_read_u16<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
  _: (),
) -> Result<u16, AnyError>
where
  FP: FfiPermissions + 'static,
{
  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(u64::from(ptr) as *const u16) })
}

fn op_ffi_read_i16<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
  _: (),
) -> Result<i16, AnyError>
where
  FP: FfiPermissions + 'static,
{
  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(u64::from(ptr) as *const i16) })
}

fn op_ffi_read_u32<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
  _: (),
) -> Result<u32, AnyError>
where
  FP: FfiPermissions + 'static,
{
  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(u64::from(ptr) as *const u32) })
}

fn op_ffi_read_i32<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
  _: (),
) -> Result<i32, AnyError>
where
  FP: FfiPermissions + 'static,
{
  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(u64::from(ptr) as *const i32) })
}

fn op_ffi_read_u64<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
  _: (),
) -> Result<U32x2, AnyError>
where
  FP: FfiPermissions + 'static,
{
  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(U32x2::from(unsafe {
    ptr::read_unaligned(u64::from(ptr) as *const u64)
  }))
}

fn op_ffi_read_f32<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
  _: (),
) -> Result<f32, AnyError>
where
  FP: FfiPermissions + 'static,
{
  let permissions = state.borrow_mut::<FP>();
  permissions.check(None)?;

  Ok(unsafe { ptr::read_unaligned(u64::from(ptr) as *const f32) })
}

fn op_ffi_read_f64<FP>(
  state: &mut deno_core::OpState,
  ptr: U32x2,
  _: (),
) -> Result<f64, AnyError>
where
  FP: FfiPermissions + 'static,
{
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
