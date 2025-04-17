// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;

use deno_core::op2;
use deno_core::v8;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::Resource;
use deno_error::JsErrorBox;
use deno_error::JsErrorClass;
use denort_helper::DenoRtNativeAddonLoaderRc;
use dlopen2::raw::Library;
use serde::Deserialize;
use serde_value::ValueDeserializer;

use crate::ir::out_buffer_as_ptr;
use crate::symbol::NativeType;
use crate::symbol::Symbol;
use crate::turbocall;
use crate::turbocall::Turbocall;
use crate::FfiPermissions;

deno_error::js_error_wrapper!(dlopen2::Error, JsDlopen2Error, |err| {
  match err {
    dlopen2::Error::NullCharacter(_) => "InvalidData".into(),
    dlopen2::Error::OpeningLibraryError(e) => e.get_class(),
    dlopen2::Error::SymbolGettingError(e) => e.get_class(),
    dlopen2::Error::AddrNotMatchingDll(e) => e.get_class(),
    dlopen2::Error::NullSymbol => "NotFound".into(),
  }
});

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum DlfcnError {
  #[class(generic)]
  #[error("Failed to register symbol {symbol}: {error}")]
  RegisterSymbol {
    symbol: String,
    #[source]
    error: dlopen2::Error,
  },
  #[class(generic)]
  #[error(transparent)]
  Dlopen(#[from] dlopen2::Error),
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] deno_permissions::PermissionCheckError),
  #[class(inherit)]
  #[error(transparent)]
  DenoRtLoad(#[from] denort_helper::LoadError),
  #[class(inherit)]
  #[error(transparent)]
  Other(#[from] JsErrorBox),
}

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
  pub fn get_static(&self, symbol: String) -> Result<*mut c_void, DlfcnError> {
    // By default, Err returned by this function does not tell
    // which symbol wasn't exported. So we'll modify the error
    // message to include the name of symbol.
    //
    // SAFETY: The obtained T symbol is the size of a pointer.
    match unsafe { self.lib.symbol::<*mut c_void>(&symbol) } {
      Ok(value) => Ok(Ok(value)),
      Err(error) => Err(DlfcnError::RegisterSymbol { symbol, error }),
    }?
  }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ForeignFunction {
  name: Option<String>,
  pub parameters: Vec<NativeType>,
  pub result: NativeType,
  #[serde(rename = "nonblocking")]
  non_blocking: Option<bool>,
  #[serde(rename = "optional")]
  #[serde(default = "default_optional")]
  optional: bool,
}

fn default_optional() -> bool {
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

#[derive(Debug)]
enum ForeignSymbol {
  ForeignFunction(ForeignFunction),
  ForeignStatic(#[allow(dead_code)] ForeignStatic),
}

impl<'de> Deserialize<'de> for ForeignSymbol {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let value = serde_value::Value::deserialize(deserializer)?;

    // Probe a ForeignStatic and if that doesn't match, assume ForeignFunction to improve error messages
    if let Ok(res) = ForeignStatic::deserialize(
      ValueDeserializer::<D::Error>::new(value.clone()),
    ) {
      Ok(ForeignSymbol::ForeignStatic(res))
    } else {
      ForeignFunction::deserialize(ValueDeserializer::<D::Error>::new(value))
        .map(ForeignSymbol::ForeignFunction)
    }
  }
}

#[derive(Deserialize, Debug)]
pub struct FfiLoadArgs {
  path: String,
  symbols: HashMap<String, ForeignSymbol>,
}

#[op2(stack_trace)]
pub fn op_ffi_load<'scope, FP>(
  scope: &mut v8::HandleScope<'scope>,
  state: Rc<RefCell<OpState>>,
  #[serde] args: FfiLoadArgs,
) -> Result<v8::Local<'scope, v8::Value>, DlfcnError>
where
  FP: FfiPermissions + 'static,
{
  let (path, denort_helper) = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<FP>();
    (
      permissions.check_partial_with_path(&args.path)?,
      state.try_borrow::<DenoRtNativeAddonLoaderRc>().cloned(),
    )
  };

  let real_path = match denort_helper {
    Some(loader) => loader.load_and_resolve_path(&path)?,
    None => Cow::Borrowed(path.as_ref()),
  };
  let lib = Library::open(real_path.as_ref()).map_err(|e| {
    dlopen2::Error::OpeningLibraryError(std::io::Error::new(
      std::io::ErrorKind::Other,
      format_error(e, &real_path),
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
      ForeignSymbol::ForeignFunction(foreign_fn) => 'register_symbol: {
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
            Err(error) => if foreign_fn.optional {
              let null: v8::Local<v8::Value> = v8::null(scope).into();
              let func_key = v8::String::new(scope, &symbol_key).unwrap();
              obj.set(scope, func_key.into(), null);
              break 'register_symbol;
            } else {
              Err(DlfcnError::RegisterSymbol {
                symbol: symbol.to_owned(),
                error,
              })
            },
          }?;

        let ptr = libffi::middle::CodePtr::from_ptr(fn_ptr as _);
        let cif = libffi::middle::Cif::new(
          foreign_fn
            .parameters
            .clone()
            .into_iter()
            .map(libffi::middle::Type::try_from)
            .collect::<Result<Vec<_>, _>>()?,
          foreign_fn.result.clone().try_into()?,
        );

        let func_key = v8::String::new(scope, &symbol_key).unwrap();
        let sym = Box::new(Symbol {
          name: symbol_key.clone(),
          cif,
          ptr,
          parameter_types: foreign_fn.parameters,
          result_type: foreign_fn.result,
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

  let mut state = state.borrow_mut();
  let out = v8::Array::new(scope, 2);
  let rid = state.resource_table.add(resource);
  let rid_v8 = v8::Integer::new_from_unsigned(scope, rid);
  out.set_index(scope, 0, rid_v8.into());
  out.set_index(scope, 1, obj.into());
  Ok(out.into())
}

struct FunctionData {
  // Held in a box to keep memory while function is alive.
  #[allow(unused)]
  symbol: Box<Symbol>,
  // Held in a box to keep inner data alive while function is alive.
  #[allow(unused)]
  turbocall: Option<Turbocall>,
}

impl GarbageCollected for FunctionData {}

// Create a JavaScript function for synchronous FFI call to
// the given symbol.
fn make_sync_fn<'s>(
  scope: &mut v8::HandleScope<'s>,
  symbol: Box<Symbol>,
) -> v8::Local<'s, v8::Function> {
  let turbocall = if turbocall::is_compatible(&symbol) {
    match turbocall::compile_trampoline(&symbol) {
      Ok(trampoline) => {
        let turbocall = turbocall::make_template(&symbol, trampoline);
        Some(turbocall)
      }
      Err(e) => {
        log::warn!("Failed to compile FFI turbocall: {e}");
        None
      }
    }
  } else {
    None
  };

  let c_function = turbocall.as_ref().map(|turbocall| {
    v8::fast_api::CFunction::new(
      turbocall.trampoline.ptr(),
      &turbocall.c_function_info,
    )
  });

  let data = FunctionData { symbol, turbocall };
  let data = deno_core::cppgc::make_cppgc_object(scope, data);

  let builder = v8::FunctionTemplate::builder(sync_fn_impl).data(data.into());

  let func = if let Some(c_function) = c_function {
    builder.build_fast(scope, &[c_function])
  } else {
    builder.build(scope)
  };
  func.get_function(scope).unwrap()
}

fn sync_fn_impl<'s>(
  scope: &mut v8::HandleScope<'s>,
  args: v8::FunctionCallbackArguments<'s>,
  mut rv: v8::ReturnValue,
) {
  let data = deno_core::cppgc::try_unwrap_cppgc_object::<FunctionData>(
    scope,
    args.data(),
  )
  .unwrap();
  let out_buffer = match data.symbol.result_type {
    NativeType::Struct(_) => {
      let argc = args.length();
      out_buffer_as_ptr(
        scope,
        Some(
          v8::Local::<v8::TypedArray>::try_from(args.get(argc - 1)).unwrap(),
        ),
      )
    }
    _ => None,
  };
  match crate::call::ffi_call_sync(scope, args, &data.symbol, out_buffer) {
    Ok(result) => {
      let result =
            // SAFETY: Same return type declared to libffi; trust user to have it right beyond that.
            unsafe { result.to_v8(scope, data.symbol.result_type.clone()) };
      rv.set(result);
    }
    Err(err) => deno_core::error::throw_js_error_class(scope, &err),
  };
}

// `path` is only used on Windows.
#[allow(unused_variables)]
pub(crate) fn format_error(
  e: dlopen2::Error,
  path: &std::path::Path,
) -> String {
  match e {
    #[cfg(target_os = "windows")]
    // This calls FormatMessageW with library path
    // as replacement for the insert sequences.
    // Unlike libstd which passes the FORMAT_MESSAGE_IGNORE_INSERTS
    // flag without any arguments.
    //
    // https://github.com/denoland/deno/issues/11632
    dlopen2::Error::OpeningLibraryError(e) => {
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

      let path = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
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
  use serde_json::json;

  use super::ForeignFunction;
  use super::ForeignSymbol;
  use crate::symbol::NativeType;

  #[cfg(target_os = "windows")]
  #[test]
  fn test_format_error() {
    use super::format_error;

    // BAD_EXE_FORMAT
    let err = dlopen2::Error::OpeningLibraryError(
      std::io::Error::from_raw_os_error(0x000000C1),
    );
    assert_eq!(
      format_error(err, &std::path::PathBuf::from("foo.dll")),
      "foo.dll is not a valid Win32 application.\r\n".to_string(),
    );
  }

  /// Ensure that our custom serialize for ForeignSymbol is working using `serde_json`.
  #[test]
  fn test_serialize_foreign_symbol() {
    let symbol: ForeignSymbol = serde_json::from_value(json! {{
      "name": "test",
      "type": "type is unused"
    }})
    .expect("Failed to parse");
    assert!(matches!(symbol, ForeignSymbol::ForeignStatic(..)));

    let symbol: ForeignSymbol = serde_json::from_value(json! {{
      "name": "test",
      "parameters": ["i64"],
      "result": "bool"
    }})
    .expect("Failed to parse");
    if let ForeignSymbol::ForeignFunction(ForeignFunction {
      name: Some(expected_name),
      parameters,
      ..
    }) = symbol
    {
      assert_eq!(expected_name, "test");
      assert_eq!(parameters, vec![NativeType::I64]);
    } else {
      panic!("Failed to parse ForeignFunction as expected");
    }
  }

  #[test]
  fn test_serialize_foreign_symbol_failures() {
    let error = serde_json::from_value::<ForeignSymbol>(json! {{
      "name": "test",
      "parameters": ["int"],
      "result": "bool"
    }})
    .expect_err("Expected this to fail");
    assert!(error.to_string().contains("expected one of"));

    let error = serde_json::from_value::<ForeignSymbol>(json! {{
      "name": "test",
      "parameters": ["i64"],
      "result": "int"
    }})
    .expect_err("Expected this to fail");
    assert!(error.to_string().contains("expected one of"));
  }
}
