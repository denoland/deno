// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
use std::rc::Rc;
#[cfg(unix)]
use std::sync::Arc;
#[cfg(unix)]
use std::sync::LazyLock;
#[cfg(unix)]
use std::sync::Mutex;
#[cfg(unix)]
use std::sync::Weak;

use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::op2;
use deno_core::v8;
use deno_error::JsErrorBox;
use deno_error::JsErrorClass;
use deno_permissions::PermissionsContainer;
use denort_helper::DenoRtNativeAddonLoaderRc;
use dlopen2::raw::Library;
use serde::Deserialize;
use serde_value::ValueDeserializer;

use crate::ir::out_buffer_as_ptr;
use crate::symbol::NativeType;
use crate::symbol::Symbol;
use crate::turbocall;
use crate::turbocall::Turbocall;

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
  // On Unix, holds a strong reference to the temp file the library was loaded
  // from (see `open_library`). Keeps the temp file alive — and the entry in
  // `OPEN_TEMPS` reachable — for as long as this library is loaded, so
  // subsequent dlopens of the same source path reuse the same temp file (and
  // therefore the same OS-level mapping).
  #[cfg(unix)]
  #[allow(
    dead_code,
    reason = "RAII owner: kept alive for the lifetime of the library"
  )]
  temp: Option<Arc<tempfile::NamedTempFile>>,
}

impl Resource for DynamicLibraryResource {
  fn name(&self) -> Cow<'_, str> {
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
  ForeignStatic(
    #[allow(dead_code, reason = "variant data used by serde deserialization")]
    ForeignStatic,
  ),
}

impl<'de> Deserialize<'de> for ForeignSymbol {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let value = serde_value::Value::deserialize(deserializer)?;

    // Probe a ForeignStatic and if that doesn't match, assume ForeignFunction to improve error messages
    match ForeignStatic::deserialize(ValueDeserializer::<D::Error>::new(
      value.clone(),
    )) {
      Ok(res) => Ok(ForeignSymbol::ForeignStatic(res)),
      _ => {
        ForeignFunction::deserialize(ValueDeserializer::<D::Error>::new(value))
          .map(ForeignSymbol::ForeignFunction)
      }
    }
  }
}

/// Open a dynamic library.
///
/// On Unix, when the caller passes a real file path (one that contains a path
/// separator — bare `libfoo.so` style SONAMEs are forwarded to `dlopen` so
/// the dynamic loader can search `LD_LIBRARY_PATH`, `/etc/ld.so.cache`, etc.),
/// the library bytes are first copied into a temporary file with a fresh
/// inode and `dlopen(3)` is pointed at that copy. The temp file is unlinked
/// from the directory tree as soon as `dlopen` has mmap'd it: the kernel
/// keeps the inode (and therefore the backing pages) alive while the mapping
/// exists, so the loaded code is no longer affected by any later writes to
/// the user-supplied path.
///
/// Without this isolation, overwriting an in-use library (e.g. via
/// `Deno.copyFileSync` truncating the same path) invalidates the page-cache
/// pages backing the mapping, and any later page fault into that library —
/// including the finalizers run by ld.so at process exit — crashes. See
/// https://github.com/denoland/deno/issues/20956.
///
/// A process-wide cache (`OPEN_TEMPS`) keyed by the user-supplied path makes
/// sure two callers of `Deno.dlopen(samePath, ...)` — e.g. a main isolate and
/// a Worker — observe the same temp file and therefore the same OS-level
/// mapping. That matches the dedup `dlopen` would normally do based on path
/// identity, so library statics like `STORED_FUNCTION` remain shared across
/// isolates exactly as they would without the isolation.
fn open_library(
  path: &Path,
) -> Result<(Library, OpenLibraryHolder), dlopen2::Error> {
  #[cfg(unix)]
  {
    open_library_isolated(path)
  }
  #[cfg(not(unix))]
  {
    Library::open(path).map(|lib| (lib, ()))
  }
}

#[cfg(not(unix))]
type OpenLibraryHolder = ();

#[cfg(unix)]
type OpenLibraryHolder = Option<Arc<tempfile::NamedTempFile>>;

#[cfg(unix)]
static OPEN_TEMPS: LazyLock<
  Mutex<HashMap<PathBuf, Weak<tempfile::NamedTempFile>>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));

#[cfg(unix)]
fn open_library_isolated(
  path: &Path,
) -> Result<(Library, OpenLibraryHolder), dlopen2::Error> {
  use std::os::unix::ffi::OsStrExt;
  use std::os::unix::fs::PermissionsExt;

  // Names without a path separator are dlopen SONAMEs — let the dynamic
  // loader search the system library paths. We don't have a real file to
  // interpose on here, and the isolation problem only arises when the
  // caller controls (and can rewrite) the file on disk.
  if !path.as_os_str().as_bytes().contains(&b'/') {
    return Library::open(path).map(|lib| (lib, None));
  }

  fn copy_into_temp(path: &Path) -> std::io::Result<tempfile::NamedTempFile> {
    // Prefer the source directory so `$ORIGIN`-relative RPATH/RUNPATH and
    // sibling-library lookups keep working; fall back to the system temp
    // directory when the source directory isn't writable.
    let suffix = path
      .extension()
      .map(|e| format!(".{}", e.to_string_lossy()));
    let mut builder = tempfile::Builder::new();
    builder.prefix(".deno-ffi-");
    if let Some(ref s) = suffix {
      builder.suffix(s);
    }
    let mut temp = match path.parent() {
      Some(parent) if !parent.as_os_str().is_empty() => builder
        .tempfile_in(parent)
        .or_else(|_| builder.tempfile())?,
      _ => builder.tempfile()?,
    };

    // dlopen requires execute permission on the backing file.
    temp
      .as_file()
      .set_permissions(std::fs::Permissions::from_mode(0o700))?;

    let mut src = std::fs::File::open(path)?;
    std::io::copy(&mut src, temp.as_file_mut())?;
    temp.as_file_mut().sync_data()?;

    Ok(temp)
  }

  // Reuse an existing temp file if one already exists for this source path,
  // so multiple `Deno.dlopen` calls with the same path share one mapping.
  let temp = {
    let mut cache = OPEN_TEMPS.lock().unwrap();
    match cache.get(path).and_then(Weak::upgrade) {
      Some(existing) => existing,
      None => {
        let temp =
          copy_into_temp(path).map_err(dlopen2::Error::OpeningLibraryError)?;
        let temp = Arc::new(temp);
        cache.insert(path.to_path_buf(), Arc::downgrade(&temp));
        temp
      }
    }
  };

  // `Library::open` calls `dlopen(3)`, which opens the file, mmaps the ELF
  // segments and closes its file descriptor. The caller stores the returned
  // `Arc<NamedTempFile>` in `DynamicLibraryResource` so the temp file (and
  // therefore the cache entry) stays alive for as long as any library loaded
  // from it is still around.
  let lib = Library::open(temp.path())?;
  Ok((lib, Some(temp)))
}

#[op2(stack_trace)]
pub fn op_ffi_load<'scope>(
  scope: &mut v8::PinScope<'scope, '_>,
  state: Rc<RefCell<OpState>>,
  #[string] path: &str,
  #[serde] symbols: HashMap<String, ForeignSymbol>,
) -> Result<v8::Local<'scope, v8::Value>, DlfcnError> {
  let (path, denort_helper) = {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<PermissionsContainer>();
    (
      permissions
        .check_ffi_partial_with_path(Cow::Borrowed(Path::new(path)))?,
      state.try_borrow::<DenoRtNativeAddonLoaderRc>().cloned(),
    )
  };

  let real_path = match denort_helper {
    Some(loader) => loader.load_and_resolve_path(&path)?,
    None => Cow::Borrowed(path.as_ref()),
  };
  let (lib, _temp) = open_library(real_path.as_ref()).map_err(|e| {
    dlopen2::Error::OpeningLibraryError(std::io::Error::other(format_error(
      e, &real_path,
    )))
  })?;
  let mut resource = DynamicLibraryResource {
    lib,
    symbols: HashMap::new(),
    #[cfg(unix)]
    temp: _temp,
  };
  let obj = v8::Object::new(scope);

  for (symbol_key, foreign_symbol) in symbols {
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

pub struct FunctionData {
  // Held in a box to keep memory while function is alive.
  #[allow(unused, reason = "kept alive for the duration of the function")]
  pub symbol: Box<Symbol>,
  // Held in a box to keep inner data alive while function is alive.
  #[allow(unused, reason = "kept alive for the duration of the function")]
  turbocall: Option<Turbocall>,
}

// SAFETY: we're sure `FunctionData` can be GCed
unsafe impl GarbageCollected for FunctionData {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"FunctionData"
  }
}

// Create a JavaScript function for synchronous FFI call to
// the given symbol.
fn make_sync_fn<'s>(
  scope: &mut v8::PinScope<'s, '_>,
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
  scope: &mut v8::PinScope<'s, '_>,
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
#[allow(unused_variables, reason = "path is only used on Windows")]
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
      use winapi::um::winbase::FORMAT_MESSAGE_ARGUMENT_ARRAY;
      use winapi::um::winbase::FORMAT_MESSAGE_FROM_SYSTEM;
      use winapi::um::winbase::FormatMessageW;
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
