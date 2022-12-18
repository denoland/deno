// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::check_unstable;
use crate::symbol::NativeType;
use crate::symbol::Symbol;
use crate::turbocall;
use crate::FfiPermissions;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::serde_v8;
use deno_core::v8;
use deno_core::Resource;
use deno_core::ResourceId;
use dlopen::raw::Library;
use serde::Deserialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::PathBuf;
use std::rc::Rc;

pub struct DynamicLibraryResource {
  lib: Library,
  pub symbols: HashMap<String, Box<Symbol>>,
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
  pub fn get_static(&self, symbol: String) -> Result<*const c_void, AnyError> {
    // By default, Err returned by this function does not tell
    // which symbol wasn't exported. So we'll modify the error
    // message to include the name of symbol.
    //
    // SAFETY: The obtained T symbol is the size of a pointer.
    match unsafe { self.lib.symbol::<*const c_void>(&symbol) } {
      Ok(value) => Ok(Ok(value)),
      Err(err) => Err(generic_error(format!(
        "Failed to register symbol {}: {}",
        symbol, err
      ))),
    }?
  }
}

pub fn needs_unwrap(rv: NativeType) -> bool {
  matches!(
    rv,
    NativeType::Function
      | NativeType::Pointer
      | NativeType::Buffer
      | NativeType::I64
      | NativeType::ISize
      | NativeType::U64
      | NativeType::USize
  )
}

fn is_i64(rv: NativeType) -> bool {
  matches!(rv, NativeType::I64 | NativeType::ISize)
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ForeignFunction {
  name: Option<String>,
  pub parameters: Vec<NativeType>,
  pub result: NativeType,
  #[serde(rename = "nonblocking")]
  non_blocking: Option<bool>,
  #[serde(rename = "callback")]
  #[serde(default = "default_callback")]
  callback: bool,
}

fn default_callback() -> bool {
  false
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
pub struct FfiLoadArgs {
  path: String,
  symbols: HashMap<String, ForeignSymbol>,
}

#[op(v8)]
pub fn op_ffi_load<FP, 'scope>(
  scope: &mut v8::HandleScope<'scope>,
  state: &mut deno_core::OpState,
  args: FfiLoadArgs,
) -> Result<(ResourceId, serde_v8::Value<'scope>), AnyError>
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
  let obj = v8::Object::new(scope);

  for (symbol_key, foreign_symbol) in args.symbols {
    match foreign_symbol {
      ForeignSymbol::ForeignStatic(_) => {
        // No-op: Statics will be handled separately and are not part of the Rust-side resource.
      }
      ForeignSymbol::ForeignFunction(foreign_fn) => {
        let symbol = match &foreign_fn.name {
          Some(symbol) => symbol,
          None => &symbol_key,
        };
        // By default, Err returned by this function does not tell
        // which symbol wasn't exported. So we'll modify the error
        // message to include the name of symbol.
        let fn_ptr =
          // SAFETY: The obtained T symbol is the size of a pointer.
          match unsafe { resource.lib.symbol::<*const c_void>(symbol) } {
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

        let func_key = v8::String::new(scope, &symbol_key).unwrap();
        let sym = Box::new(Symbol {
          cif,
          ptr,
          parameter_types: foreign_fn.parameters,
          result_type: foreign_fn.result,
          can_callback: foreign_fn.callback,
        });

        resource.symbols.insert(symbol_key, sym.clone());
        match foreign_fn.non_blocking {
          // Generate functions for synchronous calls.
          Some(false) | None => {
            let function = make_sync_fn(scope, sym);
            obj.set(scope, func_key.into(), function.into());
          }
          // This optimization is not yet supported for non-blocking calls.
          _ => {}
        };
      }
    }
  }

  let rid = state.resource_table.add(resource);
  Ok((
    rid,
    serde_v8::Value {
      v8_value: obj.into(),
    },
  ))
}

// Create a JavaScript function for synchronous FFI call to
// the given symbol.
fn make_sync_fn<'s>(
  scope: &mut v8::HandleScope<'s>,
  sym: Box<Symbol>,
) -> v8::Local<'s, v8::Function> {
  let sym = Box::leak(sym);
  let builder = v8::FunctionTemplate::builder(
    |scope: &mut v8::HandleScope,
     args: v8::FunctionCallbackArguments,
     mut rv: v8::ReturnValue| {
      let external: v8::Local<v8::External> = args.data().try_into().unwrap();
      // SAFETY: The pointer will not be deallocated until the function is
      // garbage collected.
      let symbol = unsafe { &*(external.value() as *const Symbol) };
      let needs_unwrap = match needs_unwrap(symbol.result_type) {
        true => Some(args.get(symbol.parameter_types.len() as i32)),
        false => None,
      };
      match crate::call::ffi_call_sync(scope, args, symbol) {
        Ok(result) => {
          match needs_unwrap {
            Some(v) => {
              let view: v8::Local<v8::ArrayBufferView> = v.try_into().unwrap();
              let backing_store =
                view.buffer(scope).unwrap().get_backing_store();

              if is_i64(symbol.result_type) {
                // SAFETY: v8::SharedRef<v8::BackingStore> is similar to Arc<[u8]>,
                // it points to a fixed continuous slice of bytes on the heap.
                let bs = unsafe {
                  &mut *(&backing_store[..] as *const _ as *mut [u8]
                    as *mut i64)
                };
                // SAFETY: We already checked that type == I64
                let value = unsafe { result.i64_value };
                *bs = value;
              } else {
                // SAFETY: v8::SharedRef<v8::BackingStore> is similar to Arc<[u8]>,
                // it points to a fixed continuous slice of bytes on the heap.
                let bs = unsafe {
                  &mut *(&backing_store[..] as *const _ as *mut [u8]
                    as *mut u64)
                };
                // SAFETY: We checked that type == U64
                let value = unsafe { result.u64_value };
                *bs = value;
              }
            }
            None => {
              // SAFETY: Same return type declared to libffi; trust user to have it right beyond that.
              let result = unsafe { result.to_v8(scope, symbol.result_type) };
              rv.set(result.v8_value);
            }
          }
        }
        Err(err) => {
          deno_core::_ops::throw_type_error(scope, err.to_string());
        }
      };
    },
  )
  .data(v8::External::new(scope, sym as *mut Symbol as *mut _).into());

  let mut fast_call_alloc = None;

  let func = if turbocall::is_compatible(sym) {
    let trampoline = turbocall::compile_trampoline(sym);
    let func = builder.build_fast(
      scope,
      &turbocall::make_template(sym, &trampoline),
      None,
    );
    fast_call_alloc = Some(Box::into_raw(Box::new(trampoline)));
    func
  } else {
    builder.build(scope)
  };
  let func = func.get_function(scope).unwrap();

  let weak = v8::Weak::with_finalizer(
    scope,
    func,
    Box::new(move |_| {
      // SAFETY: This is never called twice. pointer obtained
      // from Box::into_raw, hence, satisfies memory layout requirements.
      let _ = unsafe { Box::from_raw(sym) };
      if let Some(fast_call_ptr) = fast_call_alloc {
        // fast-call compiled trampoline is unmapped when the MMAP handle is dropped
        // SAFETY: This is never called twice. pointer obtained
        // from Box::into_raw, hence, satisfies memory layout requirements.
        let _ = unsafe { Box::from_raw(fast_call_ptr) };
      }
    }),
  );

  weak.to_local(scope).unwrap()
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
        // SAFETY:
        // winapi call to format the error message
        let length = unsafe {
          FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_ARGUMENT_ARRAY,
            std::ptr::null_mut(),
            err_num as DWORD,
            lang_id as DWORD,
            buf.as_mut_ptr(),
            buf.len() as DWORD,
            arguments.as_ptr() as _,
          )
        };

        if length == 0 {
          // SAFETY:
          // winapi call to get the last error message
          let err_num = unsafe { GetLastError() };
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
    _ => e.to_string(),
  }
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
