// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering;

use deno_core::GarbageCollected;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::ToV8;
use deno_core::op2;
#[cfg(feature = "sync_fs")]
use deno_core::unsync::spawn_blocking;
use deno_core::v8;
use deno_fs::FileSystemRc;
use deno_fs::FsFileType;
use deno_fs::OpenOptions;
use deno_io::fs::FsResult;
use deno_io::fs::FsStatFs;
use deno_permissions::CheckedPath;
use deno_permissions::CheckedPathBuf;
use deno_permissions::OpenAccessKind;
use deno_permissions::PermissionsContainer;
#[cfg(feature = "sync_fs")]
use tokio::task::JoinError;

use crate::ops::constant::UV_FS_COPYFILE_EXCL;

/// Virtual file descriptors for files without a real OS fd (e.g. VFS files
/// in `deno compile` binaries). These start at a high value to avoid
/// collisions with real OS fds.
const VIRTUAL_FD_START: i32 = 1_000_000_000;
static NEXT_VIRTUAL_FD: AtomicI32 = AtomicI32::new(VIRTUAL_FD_START);

fn next_virtual_fd() -> i32 {
  NEXT_VIRTUAL_FD
    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
      Some(v.saturating_add(1))
    })
    .unwrap()
}

#[cfg(windows)]
fn is_virtual_fd(fd: i32) -> bool {
  fd >= VIRTUAL_FD_START
}

/// Extract the real OS file descriptor from a File trait object.
/// On Unix, this is the raw fd (already an i32).
/// On Windows, this duplicates the OS HANDLE and converts it to a CRT
/// file descriptor. The duplicate ensures the CRT fd and the File trait
/// object own independent handles, avoiding double-close on cleanup.
/// Read CRT errno and map it to a Win32 error code for std::io::Error.
///
/// `open_osfhandle` (and other CRT functions) report failures via errno,
/// NOT GetLastError(). Calling `std::io::Error::last_os_error()` after a
/// CRT failure reads a stale Win32 error from a prior API call — e.g.
/// ERROR_ALREADY_EXISTS (183) left over from CreateFileW(CREATE_ALWAYS),
/// which would be misreported as EEXIST.
#[cfg(windows)]
fn crt_error() -> std::io::Error {
  // SAFETY: _errno() is a standard MSVC CRT function that returns a
  // pointer to the thread-local errno value. Always valid to call.
  unsafe extern "C" {
    fn _errno() -> *mut i32;
  }
  // SAFETY: _errno() returns a valid pointer to thread-local errno.
  let crt_errno = unsafe { *_errno() };
  let win32_code = match crt_errno {
    libc::EMFILE => 4,  // ERROR_TOO_MANY_OPEN_FILES
    libc::EBADF => 6,   // ERROR_INVALID_HANDLE
    libc::ENOMEM => 8,  // ERROR_NOT_ENOUGH_MEMORY
    libc::EINVAL => 87, // ERROR_INVALID_PARAMETER
    _ => 0,             // Unmapped → maps to UV "UNKNOWN"
  };
  std::io::Error::from_raw_os_error(win32_code)
}

/// Convert a backing OS fd/handle into a usable file descriptor.
/// On Unix this is a no-op. On Windows it duplicates the OS HANDLE and
/// converts it to a CRT file descriptor.
fn raw_fd_from_backing_fd(
  handle_fd: deno_core::ResourceHandleFd,
) -> Result<i32, FsError> {
  #[cfg(unix)]
  {
    Ok(handle_fd)
  }

  #[cfg(windows)]
  {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Foundation::DUPLICATE_SAME_ACCESS;
    use windows_sys::Win32::Foundation::DuplicateHandle;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    // Duplicate the OS handle so the CRT fd owns an independent copy.
    // This prevents double-close: dropping the Rc<dyn File> closes the
    // original handle, and libc::close(crt_fd) closes the duplicate.
    let mut dup_handle = std::ptr::null_mut();
    // SAFETY: `handle_fd` is a valid OS handle from the file. We duplicate
    // it into the current process with the same access rights.
    let ok = unsafe {
      DuplicateHandle(
        GetCurrentProcess(),
        handle_fd as _,
        GetCurrentProcess(),
        &mut dup_handle,
        0,
        0,
        DUPLICATE_SAME_ACCESS,
      )
    };
    if ok == 0 {
      return Err(FsError::Io(std::io::Error::last_os_error()));
    }

    // SAFETY: `dup_handle` is a valid duplicated OS handle.
    // `open_osfhandle` associates a CRT file descriptor with it so
    // that node:fs callers receive a POSIX-style fd.
    let crt_fd = unsafe { libc::open_osfhandle(dup_handle as isize, 0) };
    if crt_fd == -1 {
      // SAFETY: Clean up the duplicated handle on failure.
      unsafe { CloseHandle(dup_handle) };
      return Err(FsError::Io(crt_error()));
    }
    Ok(crt_fd)
  }
}

fn ebadf() -> FsError {
  FsError::Io(std::io::Error::from_raw_os_error(
    #[cfg(unix)]
    libc::EBADF,
    #[cfg(windows)]
    {
      // Win32 ERROR_INVALID_HANDLE, which maps to Node's EBADF
      6
    },
  ))
}

#[cfg(unix)]
const EBADF_ERRNO: i32 = libc::EBADF;
#[cfg(windows)]
const EBADF_ERRNO: i32 = 6;

// Maps an fd-op error to a node error carrying the syscall name and NO path
// (e.g. "EBADF: bad file descriptor, fsync") — matching how node reports
// errors for fd-based fs ops, which have no path component.
fn fd_syscall_err(e: deno_io::fs::FsError, syscall: &str) -> FsError {
  map_fs_error_to_node_fs_error(
    e,
    NodeFsErrorContext {
      syscall: Some(syscall.to_string()),
      ..Default::default()
    },
  )
}

// `EBADF` for an fd not present in the FdTable, node-formatted with the syscall.
fn ebadf_node(syscall: &str) -> FsError {
  fd_syscall_err(
    deno_io::fs::FsError::Io(std::io::Error::from_raw_os_error(EBADF_ERRNO)),
    syscall,
  )
}

// node's Windows `fs__write` (deps/uv/src/win/fs.c) remaps a write that fails
// with ERROR_ACCESS_DENIED to ERROR_INVALID_FLAGS -> UV_EBADF, so writing to a
// read-only fd reports EBADF (not EPERM/EACCES). Substitute the io error before
// the usual node mapping to match. No-op on Unix, where the kernel already
// returns EBADF for a write to an O_RDONLY fd.
fn remap_write_access_denied(e: deno_io::fs::FsError) -> deno_io::fs::FsError {
  #[cfg(windows)]
  if e.kind() == std::io::ErrorKind::PermissionDenied {
    return deno_io::fs::FsError::Io(std::io::Error::from_raw_os_error(
      EBADF_ERRNO,
    ));
  }
  e
}

/// Get the File trait object for an OS file descriptor from FdTable.
fn file_for_fd(
  state: &OpState,
  fd: i32,
) -> Result<Rc<dyn deno_io::fs::File>, FsError> {
  state
    .borrow::<deno_io::FdTable>()
    .get(fd)
    .cloned()
    .ok_or_else(ebadf)
}

/// When `sync_fs` is enabled, `FileSystemRc` is `Arc` (Send) and we can
/// offload work to a blocking thread. Otherwise, run inline.
macro_rules! maybe_spawn_blocking {
  ($f:expr) => {{
    #[cfg(feature = "sync_fs")]
    {
      spawn_blocking($f).await.unwrap()
    }
    #[cfg(not(feature = "sync_fs"))]
    {
      ($f)()
    }
  }};
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum FsError {
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] deno_permissions::PermissionCheckError),
  #[class(inherit)]
  #[error("{0}")]
  Io(
    #[from]
    #[inherit]
    std::io::Error,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Fs(
    #[from]
    #[inherit]
    deno_io::fs::FsError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  Cp(#[from] CpError),
  #[class(inherit)]
  #[error(transparent)]
  NodeFs(#[from] NodeFsError),
  #[class(inherit)]
  #[error(transparent)]
  NodeArg(#[from] NodeArgError),
  #[class(inherit)]
  #[error(transparent)]
  InvalidData(#[from] InvalidDataError),
  #[cfg(feature = "sync_fs")]
  #[class(inherit)]
  #[error(transparent)]
  JoinError(#[from] JoinError),
}

#[derive(Debug, Default)]
pub struct NodeFsErrorContext {
  message: Option<String>,
  path: Option<String>,
  dest: Option<String>,
  syscall: Option<String>,
}

impl NodeFsErrorContext {
  // Public constructor for ops outside this crate (runtime's fs.watch ops)
  // that need to build fully-formed node fs errors.
  pub fn new_syscall_path(syscall: &str, path: &str) -> Self {
    Self {
      syscall: Some(syscall.to_string()),
      path: Some(path.to_string()),
      ..Default::default()
    }
  }

  pub fn with_message(mut self, message: String) -> Self {
    self.message = Some(message);
    self
  }
}

// Maps a raw OS errno to its libuv error-code name (e.g. `ENOENT`). On unix
// the OS errno is matched directly via `libc`; on windows the value is a Win32
// error code translated by libuv's table. Returns `"UNKNOWN"` when unmapped.
#[cfg(unix)]
fn os_errno_to_uv_code(os_errno: i32) -> &'static str {
  match os_errno {
    libc::E2BIG => "E2BIG",
    libc::EACCES => "EACCES",
    libc::EADDRINUSE => "EADDRINUSE",
    libc::EADDRNOTAVAIL => "EADDRNOTAVAIL",
    libc::EAFNOSUPPORT => "EAFNOSUPPORT",
    libc::EAGAIN => "EAGAIN",
    libc::EALREADY => "EALREADY",
    libc::EBADF => "EBADF",
    libc::EBUSY => "EBUSY",
    libc::ECANCELED => "ECANCELED",
    libc::ECONNABORTED => "ECONNABORTED",
    libc::ECONNREFUSED => "ECONNREFUSED",
    libc::ECONNRESET => "ECONNRESET",
    libc::EDESTADDRREQ => "EDESTADDRREQ",
    libc::EEXIST => "EEXIST",
    libc::EFAULT => "EFAULT",
    libc::EFBIG => "EFBIG",
    libc::EHOSTUNREACH => "EHOSTUNREACH",
    libc::EINTR => "EINTR",
    libc::EINVAL => "EINVAL",
    libc::EIO => "EIO",
    libc::EISCONN => "EISCONN",
    libc::EISDIR => "EISDIR",
    libc::ELOOP => "ELOOP",
    libc::EMFILE => "EMFILE",
    libc::EMSGSIZE => "EMSGSIZE",
    libc::ENAMETOOLONG => "ENAMETOOLONG",
    libc::ENETDOWN => "ENETDOWN",
    libc::ENETUNREACH => "ENETUNREACH",
    libc::ENFILE => "ENFILE",
    libc::ENOBUFS => "ENOBUFS",
    libc::ENODEV => "ENODEV",
    libc::ENOENT => "ENOENT",
    libc::ENOMEM => "ENOMEM",
    libc::ENOPROTOOPT => "ENOPROTOOPT",
    libc::ENOSPC => "ENOSPC",
    libc::ENOSYS => "ENOSYS",
    libc::ENOTCONN => "ENOTCONN",
    libc::ENOTDIR => "ENOTDIR",
    libc::ENOTEMPTY => "ENOTEMPTY",
    libc::ENOTSOCK => "ENOTSOCK",
    libc::ENOTSUP => "ENOTSUP",
    libc::EPERM => "EPERM",
    libc::EPIPE => "EPIPE",
    libc::EPROTO => "EPROTO",
    libc::EPROTONOSUPPORT => "EPROTONOSUPPORT",
    libc::EPROTOTYPE => "EPROTOTYPE",
    libc::ERANGE => "ERANGE",
    libc::EROFS => "EROFS",
    libc::ESHUTDOWN => "ESHUTDOWN",
    libc::ESPIPE => "ESPIPE",
    libc::ESRCH => "ESRCH",
    libc::ETIMEDOUT => "ETIMEDOUT",
    libc::ETXTBSY => "ETXTBSY",
    libc::EXDEV => "EXDEV",
    libc::ENXIO => "ENXIO",
    libc::EMLINK => "EMLINK",
    libc::ENOTTY => "ENOTTY",
    libc::EILSEQ => "EILSEQ",
    _ => "UNKNOWN",
  }
}

#[cfg(windows)]
fn os_errno_to_uv_code(os_errno: i32) -> &'static str {
  crate::ops::winerror::sys_errno_to_uv_code(os_errno)
}

// Canonical libuv message and windows uv-errno value for a given uv code name,
// matching node's `internal_binding/uv.ts` tables. Messages are identical
// across platforms; only the numeric errno differs (handled by the caller).
fn uv_code_info(code: &str) -> (&'static str, i32) {
  match code {
    "E2BIG" => ("argument list too long", -4093),
    "EACCES" => ("permission denied", -4092),
    "EADDRINUSE" => ("address already in use", -4091),
    "EADDRNOTAVAIL" => ("address not available", -4090),
    "EAFNOSUPPORT" => ("address family not supported", -4089),
    "EAGAIN" => ("resource temporarily unavailable", -4088),
    "EALREADY" => ("connection already in progress", -4084),
    "EBADF" => ("bad file descriptor", -4083),
    "EBUSY" => ("resource busy or locked", -4082),
    "ECANCELED" => ("operation canceled", -4081),
    "ECHARSET" => ("invalid Unicode character", -4080),
    "ECONNABORTED" => ("software caused connection abort", -4079),
    "ECONNREFUSED" => ("connection refused", -4078),
    "ECONNRESET" => ("connection reset by peer", -4077),
    "EDESTADDRREQ" => ("destination address required", -4076),
    "EEXIST" => ("file already exists", -4075),
    "EFAULT" => ("bad address in system call argument", -4074),
    "EFBIG" => ("file too large", -4036),
    "EHOSTUNREACH" => ("host is unreachable", -4073),
    "EINTR" => ("interrupted system call", -4072),
    "EINVAL" => ("invalid argument", -4071),
    "EIO" => ("i/o error", -4070),
    "EISCONN" => ("socket is already connected", -4069),
    "EISDIR" => ("illegal operation on a directory", -4068),
    "ELOOP" => ("too many symbolic links encountered", -4067),
    "EMFILE" => ("too many open files", -4066),
    "EMSGSIZE" => ("message too long", -4065),
    "ENAMETOOLONG" => ("name too long", -4064),
    "ENETDOWN" => ("network is down", -4063),
    "ENETUNREACH" => ("network is unreachable", -4062),
    "ENFILE" => ("file table overflow", -4061),
    "ENOBUFS" => ("no buffer space available", -4060),
    "ENODEV" => ("no such device", -4059),
    "ENOENT" => ("no such file or directory", -4058),
    "ENOMEM" => ("not enough memory", -4057),
    "ENOPROTOOPT" => ("protocol not available", -4035),
    "ENOSPC" => ("no space left on device", -4055),
    "ENOSYS" => ("function not implemented", -4054),
    "ENOTCONN" => ("socket is not connected", -4053),
    "ENOTDIR" => ("not a directory", -4052),
    "ENOTEMPTY" => ("directory not empty", -4051),
    "ENOTSOCK" => ("socket operation on non-socket", -4050),
    "ENOTSUP" => ("operation not supported on socket", -4049),
    "EPERM" => ("operation not permitted", -4048),
    "EPIPE" => ("broken pipe", -4047),
    "EPROTO" => ("protocol error", -4046),
    "EPROTONOSUPPORT" => ("protocol not supported", -4045),
    "EPROTOTYPE" => ("protocol wrong type for socket", -4044),
    "ERANGE" => ("result too large", -4034),
    "EROFS" => ("read-only file system", -4043),
    "ESHUTDOWN" => ("cannot send after transport endpoint shutdown", -4042),
    "ESPIPE" => ("invalid seek", -4041),
    "ESRCH" => ("no such process", -4040),
    "ETIMEDOUT" => ("connection timed out", -4039),
    "ETXTBSY" => ("text file is busy", -4038),
    "EXDEV" => ("cross-device link not permitted", -4037),
    "EOF" => ("end of file", -4095),
    "ENXIO" => ("no such device or address", -4033),
    "EMLINK" => ("too many links", -4032),
    "ENOTTY" => ("inappropriate ioctl for device", -4029),
    "EILSEQ" => ("illegal byte sequence", -4027),
    _ => ("unknown error", -4094),
  }
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct NodeFsError {
  code: &'static str,
  errno: i32,
  message: String,
  syscall: Option<String>,
  path: Option<String>,
  dest: Option<String>,
}

impl deno_error::JsErrorClass for NodeFsError {
  fn get_class(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Borrowed(deno_error::builtin_classes::GENERIC_ERROR)
  }

  fn get_message(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Owned(self.message.clone())
  }

  fn get_additional_properties(&self) -> deno_error::AdditionalProperties {
    let mut props: Vec<(
      std::borrow::Cow<'static, str>,
      deno_error::PropertyValue,
    )> = vec![
      (std::borrow::Cow::Borrowed("errno"), self.errno.into()),
      (std::borrow::Cow::Borrowed("code"), self.code.into()),
    ];
    if let Some(syscall) = &self.syscall {
      props.push((
        std::borrow::Cow::Borrowed("syscall"),
        syscall.clone().into(),
      ));
    }
    if let Some(path) = &self.path {
      props.push((std::borrow::Cow::Borrowed("path"), path.clone().into()));
    }
    if let Some(dest) = &self.dest {
      props.push((std::borrow::Cow::Borrowed("dest"), dest.clone().into()));
    }
    Box::new(props.into_iter())
  }

  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
    self
  }
}

// Unix errno (positive) for a uv code name, used to synthesize errors that
// don't originate from a real OS error (e.g. EACCES from `access`).
#[cfg(unix)]
fn uv_code_to_unix_errno(code: &str) -> i32 {
  match code {
    "EACCES" => libc::EACCES,
    "ENOENT" => libc::ENOENT,
    "EEXIST" => libc::EEXIST,
    "ENOTEMPTY" => libc::ENOTEMPTY,
    "EPERM" => libc::EPERM,
    "EBADF" => libc::EBADF,
    "EINVAL" => libc::EINVAL,
    "ENOTDIR" => libc::ENOTDIR,
    "EISDIR" => libc::EISDIR,
    "ELOOP" => libc::ELOOP,
    _ => 0,
  }
}

impl NodeFsError {
  // Formats the node error message exactly like the `uvException` helper in
  // `ext/node/polyfills/internal/errors.ts`:
  //   `${code}: ${message || uvmsg}, ${syscall} '${path}' -> '${dest}'`
  fn build(
    code: &'static str,
    errno: i32,
    context: NodeFsErrorContext,
  ) -> Self {
    let (uvmsg, _) = uv_code_info(code);
    let base = context.message.as_deref().filter(|m| !m.is_empty());
    let mut message = format!("{code}: {}", base.unwrap_or(uvmsg));
    if let Some(syscall) = &context.syscall {
      message.push_str(", ");
      message.push_str(syscall);
    }
    if let Some(path) = &context.path {
      message.push_str(&format!(" '{path}'"));
    }
    if let Some(dest) = &context.dest {
      message.push_str(&format!(" -> '{dest}'"));
    }
    Self {
      code,
      errno,
      message,
      syscall: context.syscall,
      path: context.path,
      dest: context.dest,
    }
  }

  // Builds the fully-formed node error from a raw OS errno, so ops can throw
  // the final error without a JS round-trip.
  pub fn new(os_errno: i32, context: NodeFsErrorContext) -> Self {
    let code = os_errno_to_uv_code(os_errno);
    let (_, win_errno) = uv_code_info(code);
    let errno = if cfg!(windows) { win_errno } else { -os_errno };
    Self::build(code, errno, context)
  }

  // Builds a node error from a uv code name directly (for synthetic errors not
  // backed by a real OS error).
  pub fn from_code(code: &'static str, context: NodeFsErrorContext) -> Self {
    let (_, win_errno) = uv_code_info(code);
    #[cfg(windows)]
    let errno = win_errno;
    #[cfg(unix)]
    let errno = -uv_code_to_unix_errno(code);
    let _ = win_errno;
    Self::build(code, errno, context)
  }
}

fn map_fs_error_to_node_fs_error(
  err: deno_io::fs::FsError,
  context: NodeFsErrorContext,
) -> FsError {
  let os_errno = match err {
    deno_io::fs::FsError::Io(ref io_err) => io_err.raw_os_error(),
    _ => None,
  };

  if let Some(os_errno) = os_errno {
    return NodeFsError::new(os_errno, context).into();
  }

  FsError::Fs(err)
}

// --- node argument validators (ERR_INVALID_ARG_*) ported to Rust ---
//
// These reproduce node's `internal/errors` + `internal/validators` so fs ops
// can validate their own arguments and throw the exact node error, letting the
// JS wrappers collapse to thin re-exports.

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct NodeArgError {
  /// JS error constructor: "TypeError" or "RangeError".
  class: &'static str,
  /// node error `code`, e.g. "ERR_INVALID_ARG_TYPE".
  code: &'static str,
  message: String,
}

impl deno_error::JsErrorClass for NodeArgError {
  fn get_class(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Borrowed(self.class)
  }
  fn get_message(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Owned(self.message.clone())
  }
  fn get_additional_properties(&self) -> deno_error::AdditionalProperties {
    Box::new(std::iter::once((
      std::borrow::Cow::Borrowed("code"),
      deno_error::PropertyValue::String(std::borrow::Cow::Borrowed(self.code)),
    )))
  }
  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
    self
  }
}

// Interned one-byte (ASCII) v8 string, for property keys v8 can dedup.
fn intern_key<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  s: &str,
) -> v8::Local<'a, v8::String> {
  v8::String::new_from_one_byte(
    scope,
    s.as_bytes(),
    v8::NewStringType::Internalized,
  )
  .unwrap()
}

// node's `invalidArgTypeHelper`: describes the received value.
fn invalid_arg_type_helper(
  scope: &mut v8::PinScope<'_, '_>,
  actual: v8::Local<v8::Value>,
) -> String {
  if actual.is_null_or_undefined() {
    let word = if actual.is_null() {
      "null"
    } else {
      "undefined"
    };
    return format!(" Received {word}");
  }
  if actual.is_function() {
    let name = v8::Local::<v8::Function>::try_from(actual)
      .ok()
      .map(|f| f.get_name(scope).to_rust_string_lossy(scope))
      .unwrap_or_default();
    return format!(" Received function {name}");
  }
  if actual.is_object() {
    // Mirror node's `input.constructor?.name`.
    let obj = v8::Local::<v8::Object>::try_from(actual).unwrap();
    let ctor_key = intern_key(scope, "constructor");
    let ctor = obj.get(scope, ctor_key.into());
    let ctor_name = ctor
      .filter(|c| c.is_object() || c.is_function())
      .and_then(|c| v8::Local::<v8::Object>::try_from(c).ok())
      .and_then(|c| {
        let name_key = intern_key(scope, "name");
        c.get(scope, name_key.into())
      })
      .filter(|n| n.is_string())
      .map(|n| n.to_rust_string_lossy(scope))
      .unwrap_or_default();
    if !ctor_name.is_empty() {
      return format!(" Received an instance of {ctor_name}");
    }
    return " Received [Object: null prototype] {}".to_string();
  }
  // primitives: ` Received type <typeof> (<inspected, <=25 chars>)`
  let type_of = actual.type_of(scope).to_rust_string_lossy(scope);
  // `ToString` throws on a Symbol, so describe it like node's inspect.
  let mut inspected = if actual.is_symbol() {
    let sym = v8::Local::<v8::Symbol>::try_from(actual).unwrap();
    let desc = sym.description(scope);
    if desc.is_undefined() {
      "Symbol()".to_string()
    } else {
      format!("Symbol({})", desc.to_rust_string_lossy(scope))
    }
  } else if actual.is_big_int() {
    // inspect renders bigints with the literal `n` suffix.
    format!("{}n", actual.to_rust_string_lossy(scope))
  } else {
    actual.to_rust_string_lossy(scope)
  };
  if actual.is_string() {
    inspected = format!("'{inspected}'");
  }
  if inspected.chars().count() > 25 {
    inspected =
      format!("{}...", inspected.chars().take(25).collect::<String>());
  }
  format!(" Received type {type_of} ({inspected})")
}

// node's `createInvalidArgType` (simplified: handles the type/instance lists
// fs uses — "string", "Buffer", "URL", etc.).
fn create_invalid_arg_type(name: &str, expected: &[&str]) -> String {
  const KTYPES: &[&str] = &[
    "string", "function", "number", "object", "Function", "Object", "boolean",
    "bigint", "symbol",
  ];
  let mut types = Vec::new();
  let mut instances = Vec::new();
  let mut other = Vec::new();
  for &e in expected {
    if KTYPES.contains(&e) {
      types.push(e.to_lowercase());
    } else if e.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
      instances.push(e.to_string());
    } else {
      other.push(e.to_string());
    }
  }

  // Special-case `object` so allowed instances read distinctly.
  if !instances.is_empty()
    && let Some(pos) = types.iter().position(|t| t == "object")
  {
    types.remove(pos);
    instances.push("Object".to_string());
  }

  let mut msg = String::from("The ");
  if name.ends_with(" argument") {
    msg.push_str(&format!("{name} "));
  } else {
    let kind = if name.contains('.') {
      "property"
    } else {
      "argument"
    };
    msg.push_str(&format!("\"{name}\" {kind} "));
  }
  msg.push_str("must be ");

  if !types.is_empty() {
    if types.len() > 2 {
      let last = types.pop().unwrap();
      msg.push_str(&format!("one of type {}, or {last}", types.join(", ")));
    } else if types.len() == 2 {
      msg.push_str(&format!("one of type {} or {}", types[0], types[1]));
    } else {
      msg.push_str(&format!("of type {}", types[0]));
    }
    if !instances.is_empty() || !other.is_empty() {
      msg.push_str(" or ");
    }
  }

  if !instances.is_empty() {
    if instances.len() > 2 {
      let last = instances.pop().unwrap();
      msg.push_str(&format!(
        "an instance of {}, or {last}",
        instances.join(", ")
      ));
    } else {
      msg.push_str(&format!("an instance of {}", instances[0]));
      if instances.len() == 2 {
        msg.push_str(&format!(" or {}", instances[1]));
      }
    }
    if !other.is_empty() {
      msg.push_str(" or ");
    }
  }

  if !other.is_empty() {
    if other.len() > 2 {
      let last = other.pop().unwrap();
      msg.push_str(&format!("one of {}, or {last}", other.join(", ")));
    } else if other.len() == 2 {
      msg.push_str(&format!("one of {} or {}", other[0], other[1]));
    } else {
      if other[0].to_lowercase() != other[0] {
        msg.push_str("an ");
      }
      msg.push_str(&other[0]);
    }
  }
  msg
}

fn err_invalid_arg_type(
  scope: &mut v8::PinScope<'_, '_>,
  name: &str,
  expected: &[&str],
  actual: v8::Local<v8::Value>,
) -> NodeArgError {
  let message = format!(
    "{}.{}",
    create_invalid_arg_type(name, expected),
    invalid_arg_type_helper(scope, actual)
  );
  NodeArgError {
    class: deno_error::builtin_classes::TYPE_ERROR,
    code: "ERR_INVALID_ARG_TYPE",
    message,
  }
}

// node's exact `ERR_INVALID_ARG_VALUE` format: the name is quoted and the
// inspected received value is appended ("The argument 'encoding' is invalid
// encoding. Received 'bogus'").
fn err_invalid_arg_value_received(
  name: &str,
  reason: &str,
  received: &str,
) -> NodeArgError {
  let kind = if name.contains('.') {
    "property"
  } else {
    "argument"
  };
  NodeArgError {
    class: deno_error::builtin_classes::TYPE_ERROR,
    code: "ERR_INVALID_ARG_VALUE",
    message: format!("The {kind} '{name}' {reason}. Received {received}"),
  }
}

fn err_invalid_arg_value(name: &str, message_suffix: &str) -> NodeArgError {
  let kind = if name.contains('.') {
    "property"
  } else {
    "argument"
  };
  NodeArgError {
    class: deno_error::builtin_classes::TYPE_ERROR,
    code: "ERR_INVALID_ARG_VALUE",
    message: format!("The {name} {kind} {message_suffix}"),
  }
}

fn err_out_of_range(name: &str, range: &str, got: &str) -> NodeArgError {
  NodeArgError {
    class: deno_error::builtin_classes::RANGE_ERROR,
    code: "ERR_OUT_OF_RANGE",
    message: format!(
      "The value of \"{name}\" is out of range. It must be {range}. Received {got}"
    ),
  }
}

// Replicates `getValidatedPathToString`: accepts string | Buffer/Uint8Array |
// URL, rejects others (ERR_INVALID_ARG_TYPE) and null bytes
// (ERR_INVALID_ARG_VALUE), returning the path as a String.
fn validate_path_to_string(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
  name: &str,
) -> Result<String, FsError> {
  let path = if value.is_string() {
    value.to_rust_string_lossy(scope)
  } else if let Ok(buf) = v8::Local::<v8::Uint8Array>::try_from(value) {
    let len = buf.byte_length();
    let mut bytes = vec![0u8; len];
    buf.copy_contents(&mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
  } else if is_url(scope, value) {
    url_to_path(scope, value)?
  } else {
    return Err(
      err_invalid_arg_type(scope, name, &["string", "Buffer", "URL"], value)
        .into(),
    );
  };
  if path.as_bytes().contains(&0) {
    return Err(
      err_invalid_arg_value(
        name,
        "must be a string, Uint8Array, or URL without null bytes",
      )
      .into(),
    );
  }
  Ok(path)
}

fn is_url(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
) -> bool {
  let Ok(obj) = v8::Local::<v8::Object>::try_from(value) else {
    return false;
  };
  // Treat anything exposing a string `href` and `protocol` as URL-like, then
  // rely on `url_to_path` to convert via the `pathname`.
  let href = intern_key(scope, "href");
  let proto = intern_key(scope, "protocol");
  obj
    .get(scope, href.into())
    .map(|v| v.is_string())
    .unwrap_or(false)
    && obj
      .get(scope, proto.into())
      .map(|v| v.is_string())
      .unwrap_or(false)
}

// Replicates `decodeURIComponent`'s validation, which node's `fileURLToPath`
// runs on the URL pathname: percent-decode `%XX` to bytes (a `%` not followed by
// two hex digits is malformed) and require the result to be valid UTF-8. Returns
// `Err` (caller throws `URIError`, like node's "URI malformed") otherwise.
fn decode_uri_component_checked(s: &str) -> Result<(), ()> {
  let bytes = s.as_bytes();
  let mut out = Vec::with_capacity(bytes.len());
  let mut i = 0;
  while i < bytes.len() {
    if bytes[i] == b'%' {
      if i + 2 >= bytes.len() {
        return Err(());
      }
      let hi = (bytes[i + 1] as char).to_digit(16).ok_or(())?;
      let lo = (bytes[i + 2] as char).to_digit(16).ok_or(())?;
      out.push((hi * 16 + lo) as u8);
      i += 3;
    } else {
      out.push(bytes[i]);
      i += 1;
    }
  }
  std::str::from_utf8(&out).map(|_| ()).map_err(|_| ())
}

fn url_to_path(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
) -> Result<String, FsError> {
  let obj = v8::Local::<v8::Object>::try_from(value).unwrap();
  let href_key = intern_key(scope, "href");
  let href = obj
    .get(scope, href_key.into())
    .unwrap()
    .to_rust_string_lossy(scope);
  let url = url::Url::parse(&href)
    .map_err(|_| err_invalid_arg_value("path", "must be a file URL"))?;
  // node's fileURLToPath error semantics:
  // - a non-file URL is ERR_INVALID_URL_SCHEME;
  // - a non-localhost host is ERR_INVALID_FILE_URL_HOST (unix);
  // - encoded slashes in the path are ERR_INVALID_FILE_URL_PATH.
  if url.scheme() != "file" {
    return Err(
      NodeArgError {
        class: deno_error::builtin_classes::TYPE_ERROR,
        code: "ERR_INVALID_URL_SCHEME",
        message: "The URL must be of scheme file".to_string(),
      }
      .into(),
    );
  }
  let url_path = url.path();
  let has_encoded_slash = url_path.to_ascii_lowercase().contains("%2f")
    || (cfg!(windows) && url_path.to_ascii_lowercase().contains("%5c"));
  if has_encoded_slash {
    let suffix = if cfg!(windows) {
      "must not include encoded \\ or / characters"
    } else {
      "must not include encoded / characters"
    };
    return Err(
      NodeArgError {
        class: deno_error::builtin_classes::TYPE_ERROR,
        code: "ERR_INVALID_FILE_URL_PATH",
        message: format!("File URL path {suffix}"),
      }
      .into(),
    );
  }
  // node's `fileURLToPath` decodes the pathname with `decodeURIComponent`, which
  // throws `URIError` if the percent-encoded bytes aren't valid UTF-8 (e.g. a
  // Shift_JIS path). Validate before handing off to the lossy path conversion.
  if decode_uri_component_checked(url_path).is_err() {
    return Err(
      NodeArgError {
        class: "URIError",
        code: "ERR_INVALID_URI",
        message: "URI malformed".to_string(),
      }
      .into(),
    );
  }
  if !cfg!(windows) {
    let host = url.host_str().unwrap_or("");
    if !host.is_empty() && host != "localhost" {
      let platform = match std::env::consts::OS {
        "macos" => "darwin",
        os => os,
      };
      return Err(
        NodeArgError {
          class: deno_error::builtin_classes::TYPE_ERROR,
          code: "ERR_INVALID_FILE_URL_HOST",
          message: format!(
            "File URL host must be \"localhost\" or empty on {platform}"
          ),
        }
        .into(),
      );
    }
  }
  match deno_path_util::url_to_file_path(&url) {
    Ok(p) => Ok(p.to_string_lossy().into_owned()),
    Err(_) => Err(err_invalid_arg_value("path", "must be a file URL").into()),
  }
}

// node's `kMaxUserId` (2**32 - 1): upper bound for uid/gid.
const K_MAX_USER_ID: i64 = 4294967295;

// node's safe-integer bounds: the default range for `validateInteger`.
const MAX_SAFE_INTEGER: i64 = 9007199254740991;
const MIN_SAFE_INTEGER: i64 = -9007199254740991;

// Replicates node's `validateInteger(value, name, min, max)`: requires a
// safe-integer number within [min, max], else ERR_INVALID_ARG_TYPE /
// ERR_OUT_OF_RANGE.
fn validate_integer(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
  name: &str,
  min: i64,
  max: i64,
) -> Result<i64, FsError> {
  if !value.is_number() {
    return Err(err_invalid_arg_type(scope, name, &["number"], value).into());
  }
  let n = value.number_value(scope).unwrap_or(f64::NAN);
  if n.fract() != 0.0 || n.is_nan() {
    // node: validateInteger requires an integer.
    return Err(err_out_of_range(name, "an integer", &fmt_num(n)).into());
  }
  let i = n as i64;
  if i < min || i > max {
    return Err(
      err_out_of_range(name, &format!(">= {min} && <= {max}"), &fmt_num(n))
        .into(),
    );
  }
  Ok(i)
}

// Formats a number for an ERR_OUT_OF_RANGE "Received" suffix like node (JS
// number formatting: `Infinity`/`-Infinity`/`NaN`, not Rust's `inf`/`NaN`).
fn fmt_num(n: f64) -> String {
  if n.is_nan() {
    return "NaN".to_string();
  }
  if n.is_infinite() {
    return if n < 0.0 { "-Infinity" } else { "Infinity" }.to_string();
  }
  if n.fract() == 0.0 && n.abs() < 1e21 {
    // JS `String()` keeps integers below 1e21 in decimal notation; values at
    // or above 2**63 need the float formatter to render their digits.
    let s = if n.abs() < 9.2e18 {
      format!("{}", n as i64)
    } else {
      format!("{n:.0}")
    };
    // node's ERR_OUT_OF_RANGE adds `_` thousands separators when an integer
    // value's magnitude exceeds 2**32.
    if n.abs() > 4294967296.0 {
      add_numerical_separator(&s)
    } else {
      s
    }
  } else {
    format!("{n}")
  }
}

// Mirrors node's `addNumericalSeparator` (lib/internal/errors.js): inserts `_`
// every three digits from the right, skipping a leading `-`.
fn add_numerical_separator(val: &str) -> String {
  let len = val.len();
  let start = if val.as_bytes().first() == Some(&b'-') {
    1
  } else {
    0
  };
  let mut i = len;
  let mut res = String::new();
  while i >= start + 4 {
    res = format!("_{}{}", &val[i - 3..i], res);
    i -= 3;
  }
  format!("{}{}", &val[0..i], res)
}

// Replicates `parseFileMode`: octal string or 32-bit int -> u32 mode.
fn parse_file_mode(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
  name: &str,
  default: Option<u32>,
) -> Result<u32, FsError> {
  // With a default, null/undefined resolves to it; without one, fall through
  // so `undefined` fails validateInt32 exactly like node.
  if value.is_null_or_undefined()
    && let Some(default) = default
  {
    return Ok(default);
  }
  if value.is_string() {
    let s = value.to_rust_string_lossy(scope);
    // node: `octalReg` is /^[0-7]+$/; a non-octal string is ERR_INVALID_ARG_VALUE
    // (with the original value inspected), while a valid-but-too-large octal
    // string falls through to validateInt32 and is ERR_OUT_OF_RANGE.
    let is_octal =
      !s.is_empty() && s.bytes().all(|b| (b'0'..=b'7').contains(&b));
    if !is_octal {
      return Err(
        err_invalid_arg_value_received(
          name,
          "must be a 32-bit unsigned integer or an octal string",
          &inspect_encoding(scope, value),
        )
        .into(),
      );
    }
    let parsed = u64::from_str_radix(&s, 8).unwrap_or(u64::MAX);
    if parsed > u32::MAX as u64 {
      return Err(
        err_out_of_range(
          name,
          ">= 0 && <= 4294967295",
          &fmt_num(parsed as f64),
        )
        .into(),
      );
    }
    return Ok(parsed as u32);
  }
  // numeric: validateInt32(value, name, 0, 2**32 - 1) in node; mode fits u32.
  if !value.is_number() {
    return Err(err_invalid_arg_type(scope, name, &["number"], value).into());
  }
  let n = value.number_value(scope).unwrap_or(f64::NAN);
  if n.fract() != 0.0 || n.is_nan() {
    // node's validateUint32: a non-integer (incl. NaN) is ERR_OUT_OF_RANGE.
    return Err(err_out_of_range(name, "an integer", &fmt_num(n)).into());
  }
  if n < 0.0 || n > u32::MAX as f64 {
    return Err(
      err_out_of_range(name, ">= 0 && <= 4294967295", &fmt_num(n)).into(),
    );
  }
  Ok(n as u32)
}

// node's `getValidMode(mode, "access")`: an integer in [F_OK, R_OK|W_OK|X_OK]
// (0..=7), defaulting to F_OK (0) when null/undefined.
fn validate_access_mode(
  scope: &mut v8::PinScope<'_, '_>,
  mode: v8::Local<v8::Value>,
) -> Result<u32, FsError> {
  if mode.is_null_or_undefined() {
    return Ok(0); // F_OK
  }
  if mode.is_number() {
    let n = mode.number_value(scope).unwrap_or(f64::NAN);
    if !n.is_nan() && n.fract() == 0.0 && (0.0..=7.0).contains(&n) {
      return Ok(n as u32);
    }
    return Err(
      err_out_of_range("mode", "an integer >= 0 && <= 7", &fmt_num(n)).into(),
    );
  }
  Err(err_invalid_arg_type(scope, "mode", &["integer"], mode).into())
}

// `Deno.errors.InvalidData` (registered as the "InvalidData" error class in
// runtime/js/99_main.js). Used by `getValidTime` for non-finite timestamps.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct InvalidDataError(String);

impl deno_error::JsErrorClass for InvalidDataError {
  fn get_class(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Borrowed("InvalidData")
  }
  fn get_message(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Owned(self.0.clone())
  }
  fn get_additional_properties(&self) -> deno_error::AdditionalProperties {
    Box::new(std::iter::empty())
  }
  fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
    self
  }
}

// Replicates `getValidTime` + `toUnixTimestamp` (internal/fs/utils): accepts a
// number, numeric string, or Date and returns a fractional-seconds unix
// timestamp. Negative finite values resolve to "now" (node quirk). Non-finite
// numbers throw `Deno.errors.InvalidData`; anything else throws
// ERR_INVALID_ARG_TYPE.
fn get_valid_time(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
  name: &str,
) -> Result<f64, FsError> {
  if value.is_date() {
    let date = v8::Local::<v8::Date>::try_from(value).unwrap();
    return Ok(date.value_of() / 1000.0);
  }
  if value.is_string() || value.is_number() {
    let n = value.number_value(scope).unwrap_or(f64::NAN);
    if !n.is_finite() {
      return Err(
        InvalidDataError(format!(
          "invalid {name}, must not be infinity or NaN"
        ))
        .into(),
      );
    }
    if n < 0.0 {
      return Ok(now_unix_secs());
    }
    return Ok(n);
  }
  Err(
    err_invalid_arg_type(scope, name, &["Date", "Time in seconds"], value)
      .into(),
  )
}

// Current time in fractional seconds since the unix epoch (`Date.now() / 1000`).
fn now_unix_secs() -> f64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs_f64())
    .unwrap_or(0.0)
}

// Splits a fractional-seconds timestamp into (seconds, nanoseconds) at
// millisecond resolution (matching the JS `unixTimeToSecNsec`). Uses floored
// (euclidean) division so the nanosecond remainder is always in `[0, 1e9)`
// for negative (pre-epoch) timestamps too -- e.g. -40.691s is (-41, 309_000_000),
// not (-40, -691_000_000) which would saturate to (-40, 0) and lose the sign.
fn unix_time_to_sec_nsec(value: f64) -> (i64, u32) {
  let total_ms = (value * 1e3).trunc() as i64;
  let seconds = total_ms.div_euclid(1_000);
  let nanoseconds = (total_ms.rem_euclid(1_000) as u32) * 1_000_000;
  (seconds, nanoseconds)
}

// Splits a fractional-seconds timestamp into (seconds, nanoseconds) at full
// nanosecond resolution, matching the JS `futimes`/`futimesSync` math. Floored
// division keeps the nanosecond remainder in `[0, 1e9)` for negative timestamps.
fn time_to_sec_nsec_full(value: f64) -> (i64, u32) {
  let total_ns = (value * 1e9).trunc() as i64;
  let seconds = total_ns.div_euclid(1_000_000_000);
  let nanoseconds = total_ns.rem_euclid(1_000_000_000) as u32;
  (seconds, nanoseconds)
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
#[error("{message}")]
#[property("kind" = self.kind())]
#[property("message" = self.message())]
#[property("path" = self.path())]
pub enum CpError {
  EInval { message: String, path: String },
  DirToNonDir { message: String, path: String },
  NonDirToDir { message: String, path: String },
  EExist { message: String, path: String },
  EIsDir { message: String, path: String },
  Socket { message: String, path: String },
  Fifo { message: String, path: String },
  Unknown { message: String, path: String },
  SymlinkToSubdirectory { message: String, path: String },
}

impl CpError {
  fn kind(&self) -> &'static str {
    match self {
      CpError::EInval { .. } => "EINVAL",
      CpError::DirToNonDir { .. } => "DIR_TO_NON_DIR",
      CpError::NonDirToDir { .. } => "NON_DIR_TO_DIR",
      CpError::EExist { .. } => "EEXIST",
      CpError::EIsDir { .. } => "EISDIR",
      CpError::Socket { .. } => "SOCKET",
      CpError::Fifo { .. } => "FIFO",
      CpError::Unknown { .. } => "UNKNOWN",
      CpError::SymlinkToSubdirectory { .. } => "SYMLINK_TO_SUBDIRECTORY",
    }
  }

  fn message(&self) -> String {
    match self {
      CpError::EInval { message, .. } => message.clone(),
      CpError::DirToNonDir { message, .. } => message.clone(),
      CpError::NonDirToDir { message, .. } => message.clone(),
      CpError::EExist { message, .. } => message.clone(),
      CpError::EIsDir { message, .. } => message.clone(),
      CpError::Socket { message, .. } => message.clone(),
      CpError::Fifo { message, .. } => message.clone(),
      CpError::Unknown { message, .. } => message.clone(),
      CpError::SymlinkToSubdirectory { message, .. } => message.clone(),
    }
  }

  fn path(&self) -> String {
    match self {
      CpError::EInval { path, .. } => path.clone(),
      CpError::DirToNonDir { path, .. } => path.clone(),
      CpError::NonDirToDir { path, .. } => path.clone(),
      CpError::EExist { path, .. } => path.clone(),
      CpError::EIsDir { path, .. } => path.clone(),
      CpError::Socket { path, .. } => path.clone(),
      CpError::Fifo { path, .. } => path.clone(),
      CpError::Unknown { path, .. } => path.clone(),
      CpError::SymlinkToSubdirectory { path, .. } => path.clone(),
    }
  }
}

#[op2(fast, stack_trace)]
pub fn op_node_fs_exists_sync(
  state: &mut OpState,
  #[string] path: &str,
) -> Result<bool, deno_permissions::PermissionCheckError> {
  let path_or_err = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(path)),
    OpenAccessKind::ReadNoFollow,
    Some("node:fs.existsSync()"),
  );
  match path_or_err {
    Ok(path) => {
      let fs = state.borrow::<FileSystemRc>();
      Ok(fs.exists_sync(&path))
    }
    Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
    Err(err) => Err(err),
  }
}

#[op2(stack_trace)]
pub async fn op_node_fs_exists(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
) -> Result<bool, FsError> {
  let (fs, path_or_err) = {
    let mut state = state.borrow_mut();
    let path_or_err = state.borrow_mut::<PermissionsContainer>().check_open(
      Cow::Owned(PathBuf::from(path)),
      OpenAccessKind::ReadNoFollow,
      Some("node:fs.exists()"),
    );
    (state.borrow::<FileSystemRc>().clone(), path_or_err)
  };

  match path_or_err {
    Ok(path) => Ok(fs.exists_async(path.into_owned()).await?),
    Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
    Err(err) => Err(FsError::Permission(err)),
  }
}

fn get_open_options(flags: i32, mode: Option<u32>) -> OpenOptions {
  let mut options = OpenOptions::from(flags);
  options.mode = mode;
  options
}

fn open_options_to_access_kind(open_options: &OpenOptions) -> OpenAccessKind {
  let read = open_options.read;
  let write = open_options.write || open_options.append || open_options.create;
  match (read, write) {
    (true, true) => OpenAccessKind::ReadWrite,
    (false, true) => OpenAccessKind::Write,
    (true, false) | (false, false) => OpenAccessKind::Read,
  }
}

#[op2(fast, stack_trace)]
#[smi]
pub fn op_node_open_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  flags: v8::Local<v8::Value>,
  mode: v8::Local<v8::Value>,
) -> Result<i32, FsError> {
  // node validates the path (getValidatedPath), then parses `flags`
  // (stringToFlags) and `mode` (parseFileMode), all synchronously before
  // touching the fs — so the op can be bound directly as the public API.
  let path_str = validate_path_to_string(scope, path, "path")?;
  let flags = string_to_flags(scope, flags, "flags")?;
  let mode = parse_file_mode(scope, mode, "mode", Some(0o666))?;
  let options = get_open_options(flags, Some(mode));

  let fs = state.borrow::<FileSystemRc>().clone();
  let path = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(&path_str)),
    open_options_to_access_kind(&options),
    Some("node:fs.openSync"),
  )?;

  // On Windows, opening with create + truncate uses CREATE_ALWAYS which
  // truncates the file to 0 bytes immediately. If the subsequent CRT fd
  // creation (open_osfhandle) fails, the file is left at 0 bytes — causing
  // permanent data loss. To prevent this, open without truncation first,
  // create the fd, and then truncate.
  #[cfg(windows)]
  let deferred_truncate =
    options.truncate && options.create && !options.create_new;
  #[cfg(windows)]
  let open_options = if deferred_truncate {
    OpenOptions {
      truncate: false,
      ..options
    }
  } else {
    options
  };
  #[cfg(not(windows))]
  let open_options = options;

  let file = fs
    .open_sync(&path, open_options)
    .map_err(|e| node_fs_err(e, "open", &path_str))?;
  // For VFS files (e.g. in deno compile), backing_fd() returns None.
  // Assign a virtual fd so the file can still be used through FdTable.
  let fd = match file.clone().backing_fd() {
    Some(backing_fd) => raw_fd_from_backing_fd(backing_fd)?,
    None => next_virtual_fd(),
  };

  #[cfg(windows)]
  if deferred_truncate
    && !is_virtual_fd(fd)
    && let Err(e) = file.clone().truncate_sync(0)
  {
    // SAFETY: fd is a valid CRT fd just created by raw_fd_from_backing_fd.
    unsafe { libc::close(fd) };
    return Err(e.into());
  }

  state.borrow_mut::<deno_io::FdTable>().register(fd, file);
  Ok(fd)
}

// `async(eager_throw)`: node parses `flags`/`mode` and runs the permission
// check synchronously (throwing ERR_INVALID_ARG_* / permission errors at the
// call site like node), then opens asynchronously and registers the fd.
#[op2(async(eager_throw), stack_trace)]
#[smi]
pub fn op_node_open(
  scope: &mut v8::PinScope<'_, '_>,
  state: Rc<RefCell<OpState>>,
  path: v8::Local<v8::Value>,
  flags: v8::Local<v8::Value>,
  mode: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<i32, FsError>> + use<>, FsError> {
  // node validates the path (getValidatedPathToString) before parsing flags,
  // so the op owns path validation -- letting `open` bind directly without a JS
  // wrapper that pre-converts the path.
  let path_str = validate_path_to_string(scope, path, "path")?;
  let flags = string_to_flags(scope, flags, "flags")?;
  let mode = parse_file_mode(scope, mode, "mode", Some(0o666))?;
  let options = get_open_options(flags, Some(mode));

  let (fs, path) = {
    let mut state = state.borrow_mut();
    (
      state.borrow::<FileSystemRc>().clone(),
      state
        .borrow_mut::<PermissionsContainer>()
        .check_open(
          Cow::Owned(PathBuf::from(&path_str)),
          open_options_to_access_kind(&options),
          Some("node:fs.open"),
        )?
        .into_owned(),
    )
  };

  // See op_node_open_sync for why we defer truncation on Windows.
  #[cfg(windows)]
  let deferred_truncate =
    options.truncate && options.create && !options.create_new;
  #[cfg(windows)]
  let open_options = if deferred_truncate {
    OpenOptions {
      truncate: false,
      ..options
    }
  } else {
    options
  };
  #[cfg(not(windows))]
  let open_options = options;

  Ok(async move {
    let file = fs
      .open_async(path, open_options)
      .await
      .map_err(|e| node_fs_err(e, "open", &path_str))?;
    // For VFS files (e.g. in deno compile), backing_fd() returns None.
    // Assign a virtual fd so the file can still be used through FdTable.
    let fd = match file.clone().backing_fd() {
      Some(backing_fd) => raw_fd_from_backing_fd(backing_fd)?,
      None => next_virtual_fd(),
    };

    #[cfg(windows)]
    if deferred_truncate
      && !is_virtual_fd(fd)
      && let Err(e) = file.clone().truncate_sync(0)
    {
      // SAFETY: fd is a valid CRT fd just created by raw_fd_from_backing_fd.
      unsafe { libc::close(fd) };
      return Err(e.into());
    }

    let mut state = state.borrow_mut();
    state.borrow_mut::<deno_io::FdTable>().register(fd, file);
    Ok(fd)
  })
}
// `fs.statfs` result: a cppgc `StatFs` (so `constructor.name` is "StatFs"
// like node's class) carrying the 8 statfs fields as OWN data properties, all
// Numbers or all BigInts per the `bigint` option (node's StatFs has no
// methods, only own data properties assigned in its constructor).
#[derive(Debug)]
pub struct StatFs {
  pub typ: u64,
  pub bsize: u64,
  pub frsize: u64,
  pub blocks: u64,
  pub bfree: u64,
  pub bavail: u64,
  pub files: u64,
  pub ffree: u64,
  pub bigint: bool,
}

// SAFETY: StatFs holds only plain data, safe to GC.
unsafe impl GarbageCollected for StatFs {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"StatFs"
  }
}

// No methods: node's StatFs prototype only has `constructor`. The empty op2
// impl registers the class template (for the prototype + class name).
#[op2]
impl StatFs {}

impl<'a> ToV8<'a> for StatFs {
  type Error = std::convert::Infallible;
  fn to_v8(
    self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    // node's StatFs constructor assignment order.
    let fields: [(&str, u64); 8] = [
      ("type", self.typ),
      ("bsize", self.bsize),
      ("frsize", self.frsize),
      ("blocks", self.blocks),
      ("bfree", self.bfree),
      ("bavail", self.bavail),
      ("files", self.files),
      ("ffree", self.ffree),
    ];
    let bigint = self.bigint;
    let obj = deno_core::cppgc::make_cppgc_object(scope, self);
    for (name, value) in fields {
      let key = intern_key(scope, name);
      let val: v8::Local<v8::Value> = if bigint {
        v8::BigInt::new_from_u64(scope, value).into()
      } else {
        v8::Number::new(scope, value as f64).into()
      };
      obj.create_data_property(scope, key.into(), val);
    }
    Ok(obj.into())
  }
}

impl StatFs {
  fn from_fs(s: FsStatFs, bigint: bool) -> Self {
    StatFs {
      typ: s.typ,
      bsize: s.bsize,
      frsize: s.frsize,
      blocks: s.blocks,
      bfree: s.bfree,
      bavail: s.bavail,
      files: s.files,
      ffree: s.ffree,
      bigint,
    }
  }
}

// Reads `options.bigint` like the prior JS (`typeof options?.bigint ===
// "boolean" ? options.bigint : false`).
fn parse_bigint_option(
  scope: &mut v8::PinScope<'_, '_>,
  options: v8::Local<v8::Value>,
) -> bool {
  if options.is_object() && !options.is_function() {
    let obj = v8::Local::<v8::Object>::try_from(options).unwrap();
    let v = get_prop(scope, obj, "bigint");
    if v.is_boolean() {
      return v.boolean_value(scope);
    }
  }
  false
}

// node's `options?.throwIfNoEntry ?? true` for stat/lstat.
fn parse_throw_if_no_entry(
  scope: &mut v8::PinScope<'_, '_>,
  options: v8::Local<v8::Value>,
) -> bool {
  if options.is_object() && !options.is_function() {
    let obj = v8::Local::<v8::Object>::try_from(options).unwrap();
    let v = get_prop(scope, obj, "throwIfNoEntry");
    if v.is_boolean() {
      return v.boolean_value(scope);
    }
  }
  true
}

#[op2(stack_trace)]
pub fn op_node_statfs_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<StatFs, FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let bigint = parse_bigint_option(scope, options);
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(&path)),
    OpenAccessKind::ReadNoFollow,
    Some("node:fs.statfsSync"),
  )?;
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("statfs", "node:fs.statfsSync")?;

  let fs = state.borrow::<FileSystemRc>();
  Ok(StatFs::from_fs(
    fs.statfs_sync(&checked, bigint)
      .map_err(|e| node_fs_err(e, "statfs", &path))?,
    bigint,
  ))
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_statfs(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<StatFs, FsError>> + use<>, FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let bigint = parse_bigint_option(scope, options);
  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      OpenAccessKind::ReadNoFollow,
      Some("node:fs.statfs"),
    )?
    .into_owned();
  state
    .borrow_mut::<PermissionsContainer>()
    .check_sys("statfs", "node:fs.statfs")?;
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(async move {
    Ok(StatFs::from_fs(
      fs.statfs_async(checked, bigint)
        .await
        .map_err(|e| node_fs_err(e, "statfs", &path))?,
      bigint,
    ))
  })
}

#[op2(fast, stack_trace)]
#[undefined]
pub fn op_node_lutimes_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  atime: v8::Local<v8::Value>,
  mtime: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let (atime_secs, atime_nanos) =
    unix_time_to_sec_nsec(get_valid_time(scope, atime, "atime")?);
  let (mtime_secs, mtime_nanos) =
    unix_time_to_sec_nsec(get_valid_time(scope, mtime, "mtime")?);
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(&path)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.lutimes"),
  )?;

  let fs = state.borrow::<FileSystemRc>();
  fs.lutime_sync(&checked, atime_secs, atime_nanos, mtime_secs, mtime_nanos)
    .map_err(|e| node_fs_err(e, "utime", &path))?;
  Ok(())
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_lutimes(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  atime: v8::Local<v8::Value>,
  mtime: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let (atime_secs, atime_nanos) =
    unix_time_to_sec_nsec(get_valid_time(scope, atime, "atime")?);
  let (mtime_secs, mtime_nanos) =
    unix_time_to_sec_nsec(get_valid_time(scope, mtime, "mtime")?);
  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.lutimes"),
    )?
    .into_owned();
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(async move {
    fs.lutime_async(checked, atime_secs, atime_nanos, mtime_secs, mtime_nanos)
      .await
      .map_err(|e| node_fs_err(e, "utime", &path))?;
    Ok(())
  })
}

#[op2(fast, stack_trace)]
pub fn op_node_lchown_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  uid: v8::Local<v8::Value>,
  gid: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let uid = validate_integer(scope, uid, "uid", -1, K_MAX_USER_ID)? as u32;
  let gid = validate_integer(scope, gid, "gid", -1, K_MAX_USER_ID)? as u32;
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Owned(PathBuf::from(&path)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.lchownSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  fs.lchown_sync(&checked, Some(uid), Some(gid))
    .map_err(|e| node_fs_err(e, "lchown", &path))?;
  Ok(())
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_lchown(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  uid: v8::Local<v8::Value>,
  gid: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let uid = validate_integer(scope, uid, "uid", -1, K_MAX_USER_ID)? as u32;
  let gid = validate_integer(scope, gid, "gid", -1, K_MAX_USER_ID)? as u32;
  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.lchown"),
    )?
    .into_owned();
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(async move {
    fs.lchown_async(checked, Some(uid), Some(gid))
      .await
      .map_err(|e| node_fs_err(e, "lchown", &path))?;
    Ok(())
  })
}

#[op2(fast, stack_trace)]
pub fn op_node_lchmod_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  mode: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let mode = parse_file_mode(scope, mode, "mode", None)?;
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Owned(PathBuf::from(&path)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.lchmodSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  fs.lchmod_sync(&checked, mode)
    .map_err(|e| node_fs_err(e, "lchmod", &path))?;
  Ok(())
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_lchmod(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  mode: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let mode = parse_file_mode(scope, mode, "mode", None)?;
  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.lchmod"),
    )?
    .into_owned();
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(async move {
    fs.lchmod_async(checked, mode)
      .await
      .map_err(|e| node_fs_err(e, "lchmod", &path))?;
    Ok(())
  })
}

// One-time-per-isolate flag for node's `warnOnNonPortableTemplate`.
struct MkdtempWarned;

// node's `warnOnNonPortableTemplate`: mkdtemp() templates ending in 'X' are
// handled inconsistently across platforms, so it emits a one-time process
// warning. Checking the *validated* prefix's last char covers both node cases
// (a string ending in 'X' and a Buffer/TypedArray whose last byte is 0x58 ->
// decodes to 'X'). The one-time flag lives here so the JS wrapper that used to
// call this can be eliminated; the warning itself is emitted through the same
// `process.emitWarning` the JS used.
fn warn_on_non_portable_template(
  scope: &mut v8::PinScope<'_, '_>,
  state: &Rc<RefCell<OpState>>,
  prefix: &str,
) {
  if !prefix.ends_with('X') {
    return;
  }
  {
    // Scope the borrow so it is released before `emitWarning` re-enters JS
    // (which runs async_hooks ops that need OpState).
    let mut state = state.borrow_mut();
    if state.try_borrow::<MkdtempWarned>().is_some() {
      return;
    }
    state.put(MkdtempWarned);
  }
  // process.emitWarning(msg) -- a lone string defaults the warning name to
  // "Warning". Best-effort: if `process`/`emitWarning` is absent, skip.
  let global = scope.get_current_context().global(scope);
  let process = get_prop(scope, global, "process");
  let Ok(process) = v8::Local::<v8::Object>::try_from(process) else {
    return;
  };
  let emit = get_prop(scope, process, "emitWarning");
  let Ok(emit) = v8::Local::<v8::Function>::try_from(emit) else {
    return;
  };
  let Some(msg) = v8::String::new(
    scope,
    "mkdtemp() templates ending with X are not portable. For details see: https://nodejs.org/api/fs.html",
  ) else {
    return;
  };
  let recv: v8::Local<v8::Value> = process.into();
  emit.call(scope, recv, &[msg.into()]);
}

// `fs.mkdtempSync(prefix, options)` end to end: validates the prefix, parses
// the encoding options (default utf8), creates the dir (retrying on suffix
// collisions like libuv), and returns the path already encoded.
// `reentrant`: emits node's non-portable-template warning via
// `process.emitWarning`, which re-enters JS (async_hooks ops).
#[op2(reentrant, stack_trace)]
pub fn op_node_mkdtemp_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: Rc<RefCell<OpState>>,
  prefix: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<MaybeEncodedBytes, FsError> {
  let prefix = validate_path_to_string(scope, prefix, "prefix")?;
  let enc = parse_encoding_options(scope, options, Some(BufEnc::Utf8))?;
  // Emit the warning before any OpState borrow is held -- it re-enters JS.
  warn_on_non_portable_template(scope, &state, &prefix);
  // https://github.com/nodejs/node/blob/2ea31e53c61463727c002c2d862615081940f355/deps/uv/src/unix/os390-syscalls.c#L409
  for _ in 0..libc::TMP_MAX {
    let candidate = temp_path_append_suffix(&prefix);
    let (fs, checked_path) = {
      let mut state = state.borrow_mut();
      // `checked` borrows `candidate` (its lifetime is the path arg, not
      // OpState), so it stays valid after this borrow is released.
      let checked = state.borrow_mut::<PermissionsContainer>().check_open(
        Cow::Borrowed(Path::new(&candidate)),
        OpenAccessKind::WriteNoFollow,
        Some("node:fs.mkdtempSync()"),
      )?;
      (state.borrow::<FileSystemRc>().clone(), checked)
    };

    match fs.mkdir_sync(&checked_path, false, Some(0o700)) {
      Ok(()) => {
        return MaybeEncodedBytes::new(
          &state.borrow(),
          candidate.into_bytes(),
          enc,
        );
      }
      Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
        continue;
      }
      Err(err) => {
        return Err(node_fs_err(err, "mkdtemp", &format!("{prefix}XXXXXX")));
      }
    }
  }

  Err(FsError::Io(std::io::Error::new(
    std::io::ErrorKind::AlreadyExists,
    "too many temp dirs exist",
  )))
}

#[op2(async(eager_throw), reentrant, stack_trace)]
pub fn op_node_mkdtemp(
  scope: &mut v8::PinScope<'_, '_>,
  state: Rc<RefCell<OpState>>,
  prefix: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<
  impl Future<Output = Result<MaybeEncodedBytes, FsError>> + use<>,
  FsError,
> {
  let prefix = validate_path_to_string(scope, prefix, "prefix")?;
  let enc = parse_encoding_options(scope, options, Some(BufEnc::Utf8))?;
  // Emit the warning before any OpState borrow is held -- it re-enters JS.
  warn_on_non_portable_template(scope, &state, &prefix);
  // The retry loop needs a per-candidate permission check after the await
  // point, so clone the (Arc-based) container into the future.
  let (perms, fs, proto) = {
    let state = state.borrow();
    (
      state.borrow::<PermissionsContainer>().clone(),
      state.borrow::<FileSystemRc>().clone(),
      if enc.is_none() {
        buffer_proto(&state)
      } else {
        None
      },
    )
  };
  Ok(async move {
    // https://github.com/nodejs/node/blob/2ea31e53c61463727c002c2d862615081940f355/deps/uv/src/unix/os390-syscalls.c#L409
    for _ in 0..libc::TMP_MAX {
      let candidate = temp_path_append_suffix(&prefix);
      let checked_path = perms.check_open(
        Cow::Owned(PathBuf::from(candidate.clone())),
        OpenAccessKind::WriteNoFollow,
        Some("node:fs.mkdtemp()"),
      )?;

      match fs
        .mkdir_async(checked_path.into_owned(), false, Some(0o700))
        .await
      {
        Ok(()) => {
          return MaybeEncodedBytes::with_proto(
            candidate.into_bytes(),
            enc,
            proto,
          );
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
          continue;
        }
        Err(err) => {
          return Err(node_fs_err(err, "mkdtemp", &format!("{prefix}XXXXXX")));
        }
      }
    }

    Err(FsError::Io(std::io::Error::new(
      std::io::ErrorKind::AlreadyExists,
      "too many temp dirs exist",
    )))
  })
}

fn temp_path_append_suffix(prefix: &str) -> String {
  use rand::Rng;
  use rand::distributions::Alphanumeric;
  use rand::rngs::OsRng;

  let suffix: String =
    (0..6).map(|_| OsRng.sample(Alphanumeric) as char).collect();
  format!("{}{}", prefix, suffix)
}

// node's `rmdir` option handling: `options?.recursive` is no longer supported
// (ERR_INVALID_ARG_VALUE), and a defined options must be an object
// (validateObject). The recursive/retryDelay/maxRetries field checks were
// removed upstream — only the object-type check remains.
fn validate_rmdir_options(
  scope: &mut v8::PinScope<'_, '_>,
  options: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  // `options?.recursive`: only reads when `options` is an object (a primitive
  // or null/undefined yields `undefined` via optional chaining).
  if let Ok(obj) = v8::Local::<v8::Object>::try_from(options) {
    let key = intern_key(scope, "recursive");
    if let Some(recursive) = obj.get(scope, key.into())
      && !recursive.is_undefined()
    {
      return Err(
        err_invalid_arg_value_received(
          "options.recursive",
          "is no longer supported",
          &inspect_encoding(scope, recursive),
        )
        .into(),
      );
    }
  }
  if !options.is_undefined() {
    validate_object(scope, options, "options")?;
  }
  Ok(())
}

#[op2(fast, stack_trace)]
#[undefined]
pub fn op_node_rmdir_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  validate_rmdir_options(scope, options)?;
  let path = validate_path_to_string(scope, path, "path")?;
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(&path)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.rmdirSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  fs.rmdir_sync(&checked)
    .map_err(|e| node_fs_err(e, "rmdir", &path))?;
  Ok(())
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_rmdir(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  validate_rmdir_options(scope, options)?;
  let path = validate_path_to_string(scope, path, "path")?;
  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.rmdir"),
    )?
    .into_owned();
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(async move {
    fs.rmdir_async(checked)
      .await
      .map_err(|e| node_fs_err(e, "rmdir", &path))?;
    Ok(())
  })
}

/// Create an anonymous pipe pair and return (read_fd, write_fd).
/// The returned fds are NOT registered in FdTable; the caller
/// is responsible for registering or closing them.
#[cfg(unix)]
#[op2]
#[serde]
pub fn op_node_create_pipe() -> Result<(i32, i32), FsError> {
  let mut fds = [0i32; 2];
  // SAFETY: pipe() writes two valid fds into the array on success.
  let ret = unsafe { libc::pipe(fds.as_mut_ptr()) };
  if ret != 0 {
    return Err(FsError::Io(std::io::Error::last_os_error()));
  }
  Ok((fds[0], fds[1]))
}

#[cfg(windows)]
#[op2]
#[serde]
pub fn op_node_create_pipe() -> Result<(i32, i32), FsError> {
  use windows_sys::Win32::Foundation::CloseHandle;
  use windows_sys::Win32::System::Pipes::CreatePipe;

  let mut read_handle = std::ptr::null_mut();
  let mut write_handle = std::ptr::null_mut();

  // SAFETY: CreatePipe writes valid handles on success.
  let ok = unsafe {
    CreatePipe(&mut read_handle, &mut write_handle, std::ptr::null(), 0)
  };
  if ok == 0 {
    return Err(FsError::Io(std::io::Error::last_os_error()));
  }

  // Convert OS handles to CRT file descriptors.
  // SAFETY: read_handle and write_handle are valid pipe handles from
  // CreatePipe. open_osfhandle takes ownership of the handle on success.
  let read_fd = unsafe { libc::open_osfhandle(read_handle as isize, 0) };
  // SAFETY: Same as above for the write handle.
  let write_fd = unsafe { libc::open_osfhandle(write_handle as isize, 0) };

  if read_fd == -1 || write_fd == -1 {
    // Clean up on failure: close whichever succeeded as a CRT fd,
    // and close the raw OS handle for whichever failed.
    if read_fd != -1 {
      // SAFETY: read_fd is a valid CRT fd from open_osfhandle.
      unsafe {
        libc::close(read_fd);
      }
    } else {
      // SAFETY: read_handle is still a valid OS handle (open_osfhandle failed).
      unsafe {
        CloseHandle(read_handle);
      }
    }
    if write_fd != -1 {
      // SAFETY: write_fd is a valid CRT fd from open_osfhandle.
      unsafe {
        libc::close(write_fd);
      }
    } else {
      // SAFETY: write_handle is still a valid OS handle (open_osfhandle failed).
      unsafe {
        CloseHandle(write_handle);
      }
    }
    return Err(ebadf());
  }

  Ok((read_fd, write_fd))
}

// ============================================================
// fd-based ops for node:fs (accept real OS fd, not RID)
// ============================================================

/// Set blocking or non-blocking mode on an OS file descriptor.
/// Matches libuv's `uv_stream_set_blocking`.
#[cfg(unix)]
#[op2(fast)]
#[smi]
pub fn op_node_fd_set_blocking(fd: i32, blocking: bool) -> i32 {
  // SAFETY: fcntl with F_GETFL/F_SETFL is safe on valid fds.
  // Returns -1 on invalid fd, which we map to UV_EBADF.
  unsafe {
    let flags = libc::fcntl(fd, libc::F_GETFL);
    if flags == -1 {
      return -libc::EBADF;
    }
    let flags = if blocking {
      flags & !libc::O_NONBLOCK
    } else {
      flags | libc::O_NONBLOCK
    };
    if libc::fcntl(fd, libc::F_SETFL, flags) == -1 {
      return -libc::EBADF;
    }
    0
  }
}

#[cfg(windows)]
#[op2(fast)]
#[smi]
pub fn op_node_fd_set_blocking(_fd: i32, _blocking: bool) -> i32 {
  // On Windows, named pipes and console handles don't support
  // toggling blocking mode via a simple flag. The Windows-specific
  // behavior is handled at the I/O level instead.
  0
}

// Shared close: remove the fd from the FdTable (dropping the file closes it),
// clean up the stdio resource-table entry, and (Windows) free the CRT fd.
fn do_close(state: &mut OpState, fd: i32) -> Result<(), FsError> {
  // FdTable.remove() drops the Rc<dyn File> and cancels the cancel handle,
  // which will abort any in-flight async reads on this fd.
  let file = state
    .borrow_mut::<deno_io::FdTable>()
    .remove(fd)
    .ok_or_else(|| ebadf_node("close"))?;

  // For stdio fds (0/1/2), also remove the corresponding resource table
  // entry so that Deno.stdin/stdout/stderr see the fd as closed and
  // release their Rc clone of the same File.
  if (0..=2).contains(&fd)
    && let Ok(resource) = state.resource_table.take_any(fd as ResourceId)
  {
    resource.close();
  }

  // Dropping the Rc<dyn File> will close the underlying OS file descriptor
  // when the reference count reaches zero (via std::fs::File Drop).
  drop(file);

  // On Windows, `raw_fd_from_backing_fd` creates a CRT file descriptor via
  // `open_osfhandle` on a duplicated OS handle. The File Drop above closes
  // the original handle; `libc::close` closes the duplicate and frees the
  // CRT fd slot. Skip this for virtual fds (VFS files) which have no CRT fd.
  #[cfg(windows)]
  if !is_virtual_fd(fd) {
    // SAFETY: `fd` is a valid CRT file descriptor created by
    // `open_osfhandle` in `raw_fd_from_backing_fd`.
    unsafe {
      libc::close(fd);
    }
  }

  Ok(())
}

#[op2(fast)]
#[undefined]
pub fn op_node_fs_close(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  fd: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  // node's `getValidatedFd`: validateInt32(fd, "fd", 0) (with -0 -> 0, which the
  // i32 cast in validate_fd_value handles).
  let fd = validate_fd_value(scope, fd)?;
  do_close(state, fd)
}

// `async(eager_throw)`: node's `close` validates the fd synchronously
// (getValidatedFd) but runs the close + callback asynchronously. The fd is
// validated in the eager prologue (synchronous throw); the close itself -- and
// any EBADF -- is deferred to the event loop by `eager_throw`'s (lazy) scheduling,
// so it lands on the callback like node, without a JS `queueMicrotask` wrapper.
// Lazy (the eager_throw default) matters here: `do_close` removes the fd from the
// table, so it must NOT run on the call -- otherwise a same-tick close of that fd
// sees it already gone and rejects with EBADF.
#[op2(async(eager_throw))]
pub fn op_node_fs_close_async(
  scope: &mut v8::PinScope<'_, '_>,
  state: Rc<RefCell<OpState>>,
  fd: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let fd = validate_fd_value(scope, fd)?;
  Ok(async move { do_close(&mut state.borrow_mut(), fd) })
}

/// Positioned read: if position >= 0, uses pread to read without moving the
/// file cursor. If position < 0, reads from the current position. Errors are
/// node-formatted with syscall "read".
fn read_with_position(
  file: Rc<dyn deno_io::fs::File>,
  buf: &mut [u8],
  position: i64,
) -> Result<u32, FsError> {
  let nread = if position >= 0 {
    file.read_at_sync(buf, position as u64)
  } else {
    file.read_sync(buf)
  }
  .map_err(|e| fd_syscall_err(e, "read"))?;
  Ok(nread as u32)
}

// `Array.isArray`: true for arrays and (recursively) for proxies whose
// ultimate target is an array -- node's `validateObject` rejects a proxied
// array as an options bag (test-fs-readSync-optional-params).
fn js_is_array(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
) -> bool {
  let mut v = value;
  loop {
    if v.is_array() {
      return true;
    }
    match v8::Local::<v8::Proxy>::try_from(v) {
      Ok(p) => v = p.get_target(scope),
      Err(_) => return false,
    }
  }
}

// node's `validateObject(value, name, kValidateObjectAllowNullable)`: `null`
// passes; arrays (Proxy-pierced, like `ArrayIsArray`), functions, and
// non-objects are ERR_INVALID_ARG_TYPE "object".
fn validate_object_nullable(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
  name: &str,
) -> Result<(), FsError> {
  if value.is_null() {
    return Ok(());
  }
  if js_is_array(scope, value) || value.is_function() || !value.is_object() {
    return Err(err_invalid_arg_type(scope, name, &["object"], value).into());
  }
  Ok(())
}

// Replicates `validateOffsetLengthRead(offset, length, bufferLength)`.
fn validate_offset_length_read(
  offset: i64,
  length: i64,
  buffer_length: i64,
) -> Result<(), FsError> {
  if offset < 0 {
    return Err(err_oor_int("offset", ">= 0", offset).into());
  }
  if length < 0 {
    return Err(err_oor_int("length", ">= 0", length).into());
  }
  if offset + length > buffer_length {
    return Err(
      err_oor_int("length", &format!("<= {}", buffer_length - offset), length)
        .into(),
    );
  }
  Ok(())
}

// JS ToInt32 for an f64 already known to be in safe-integer range
// (`buffer.byteLength - offset`): modular reduction into i32.
fn js_to_int32_f64(d: f64) -> i32 {
  (d as i64).rem_euclid(4294967296) as u32 as i32
}

// Resolves read's trailing `(offsetOrOptions, length, position)` per node:
// the options form applies when `arguments.length <= 3` or `offsetOrOptions`
// is `typeof "object"` (null included); otherwise the args are positional.
// Validation order matches node readSync: options object-ness, then
// `validateInteger(offset, 0)`, then `length |= 0` (ToInt32, with the options
// default `buffer.byteLength - offset`), then `validatePosition`
// (null/undefined -> -1; numbers are safe integers >= -1; bigints in
// [-1, 2**63 - 1 - length]). Returns `Ok(None)` when a JS conversion (ToInt32
// on a symbol or a throwing valueOf) left an exception pending -- the caller
// must return immediately so it propagates.
fn resolve_read_args(
  scope: &mut v8::PinScope<'_, '_>,
  view: v8::Local<v8::ArrayBufferView>,
  arity: u32,
  offset_or_options: v8::Local<v8::Value>,
  length_v: v8::Local<v8::Value>,
  position_v: v8::Local<v8::Value>,
) -> Result<Option<(i64, i32, i64)>, FsError> {
  let use_options = arity <= 3
    || offset_or_options.is_null()
    || (offset_or_options.is_object() && !offset_or_options.is_function());
  let undefined: v8::Local<v8::Value> = v8::undefined(scope).into();
  let (offset_raw, length_raw, position_raw) = if use_options {
    if !offset_or_options.is_undefined() {
      validate_object_nullable(scope, offset_or_options, "options")?;
    }
    if offset_or_options.is_null_or_undefined() {
      (undefined, undefined, undefined)
    } else {
      let obj = v8::Local::<v8::Object>::try_from(offset_or_options).unwrap();
      (
        get_prop(scope, obj, "offset"),
        get_prop(scope, obj, "length"),
        get_prop(scope, obj, "position"),
      )
    }
  } else {
    (offset_or_options, length_v, position_v)
  };

  let offset: i64 = if offset_raw.is_undefined() {
    0
  } else {
    validate_integer(scope, offset_raw, "offset", 0, MAX_SAFE_INTEGER)?
  };

  let length: i32 = if length_raw.is_undefined() {
    if use_options {
      js_to_int32_f64(view.byte_length() as f64 - offset as f64)
    } else {
      0
    }
  } else {
    match length_raw.int32_value(scope) {
      Some(n) => n,
      // ToInt32 threw (symbol/bigint length or a throwing valueOf): bail so
      // the pending exception propagates.
      None => return Ok(None),
    }
  };

  let position: i64 = if position_raw.is_null_or_undefined() {
    -1
  } else if position_raw.is_number() {
    validate_integer(scope, position_raw, "position", -1, MAX_SAFE_INTEGER)?
  } else if position_raw.is_big_int() {
    // validatePosition's bigint arm: [-1, 2**63 - 1 - length]. `length` may
    // be negative here (position is validated before the length range), so
    // the bound is computed in i128.
    let max = ((1i128 << 63) - 1) - length as i128;
    let big = v8::Local::<v8::BigInt>::try_from(position_raw).unwrap();
    let (v, lossless) = big.i64_value();
    if !lossless || (v as i128) < -1 || (v as i128) > max {
      let digits = position_raw.to_rust_string_lossy(scope);
      // ERR_OUT_OF_RANGE bigint rendering: `_` separators past 2**32, `n`
      // suffix.
      let mut received = if !lossless || v.unsigned_abs() > 4294967296 {
        add_numerical_separator(&digits)
      } else {
        digits
      };
      received.push('n');
      return Err(
        err_out_of_range("position", &format!(">= -1 && <= {max}"), &received)
          .into(),
      );
    }
    v
  } else {
    return Err(
      err_invalid_arg_type(
        scope,
        "position",
        &["bigint", "integer"],
        position_raw,
      )
      .into(),
    );
  };

  Ok(Some((offset, length, position)))
}

// `fs.readSync(fd, buffer, offsetOrOptions?, length?, position?)`: full node
// overload resolution + validation + positioned read. `arity` is the caller's
// `arguments.length` -- node's dispatch is arity-based (an explicit
// `undefined` offsetOrOptions with arity > 3 selects the positional form).
// Validation ORDER matches node: buffer first; the fd is validated LAST (at
// the binding layer in node) and not at all when length resolves to 0.
// Returns -1 as the "buffer is empty" sentinel -- the JS wrapper throws
// ERR_INVALID_ARG_VALUE there, keeping util.inspect's rendering of the
// received buffer.
#[op2(fast, stack_trace)]
#[smi]
pub fn op_node_fs_read_v_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  #[smi] arity: u32,
  fd_v: v8::Local<v8::Value>,
  buffer_v: v8::Local<v8::Value>,
  offset_or_options: v8::Local<v8::Value>,
  length_v: v8::Local<v8::Value>,
  position_v: v8::Local<v8::Value>,
) -> Result<i32, FsError> {
  let view =
    v8::Local::<v8::ArrayBufferView>::try_from(buffer_v).map_err(|_| {
      err_invalid_arg_type(
        scope,
        "buffer",
        &["Buffer", "TypedArray", "DataView"],
        buffer_v,
      )
    })?;
  let Some((offset, length, position)) = resolve_read_args(
    scope,
    view,
    arity,
    offset_or_options,
    length_v,
    position_v,
  )?
  else {
    return Ok(0);
  };
  if length == 0 {
    return Ok(0);
  }
  let byte_length = view.byte_length();
  if byte_length == 0 {
    return Ok(-1);
  }
  validate_offset_length_read(offset, length as i64, byte_length as i64)?;
  let fd = validate_fd_value(scope, fd_v)?;
  let file = file_for_fd(state, fd).map_err(|_| ebadf_node("read"))?;
  // SAFETY: no JS runs during this synchronous op and `buffer_v` keeps the
  // view alive; [offset, offset + length) was just range-checked against the
  // view, whose `data()` already includes its byte_offset.
  let buf: &mut [u8] = unsafe {
    std::slice::from_raw_parts_mut(
      (view.data() as *mut u8).add(offset as usize),
      length as usize,
    )
  };
  read_with_position(file, buf, position).map(|n| n as i32)
}

/// Async read for node:fs. Uses the File trait from FdTable for proper
/// I/O through the file handle. Errors are node-formatted with syscall
/// "read" (EBADF included), matching node's binding-level read errors.
#[op2]
#[smi]
pub async fn op_node_fs_read_deferred(
  state: Rc<RefCell<OpState>>,
  fd: i32,
  #[buffer] buf: JsBuffer,
  #[bigint] position: i64,
) -> Result<u32, FsError> {
  let file =
    file_for_fd(&state.borrow(), fd).map_err(|_| ebadf_node("read"))?;
  let view = deno_core::BufMutView::from(buf);
  let result = if position >= 0 {
    file.read_at_async(view, position as u64).await
  } else {
    file.read_byob(view).await
  };
  let (nread, _) = result.map_err(|e| fd_syscall_err(e, "read"))?;
  Ok(nread as u32)
}

/// Positioned write: if position >= 0, uses pwrite to write without moving
/// the file cursor. If position < 0, writes at the current position.
/// Handles partial writes internally by looping until all bytes are written.
fn write_with_position(
  file: Rc<dyn deno_io::fs::File>,
  buf: &[u8],
  position: i64,
) -> Result<u32, FsError> {
  if position >= 0 {
    let mut total = 0usize;
    while total < buf.len() {
      let nwritten = file
        .clone()
        .write_at_sync(&buf[total..], position as u64 + total as u64)
        .map_err(|e| fd_syscall_err(remap_write_access_denied(e), "write"))?;
      total += nwritten;
    }
    Ok(total as u32)
  } else {
    let mut total = 0usize;
    while total < buf.len() {
      let nwritten = file
        .clone()
        .write_sync(&buf[total..])
        .map_err(|e| fd_syscall_err(remap_write_access_denied(e), "write"))?;
      total += nwritten;
    }
    Ok(total as u32)
  }
}

#[op2(fast)]
#[smi]
pub fn op_node_fs_write_sync(
  state: &mut OpState,
  fd: i32,
  #[buffer] buf: &[u8],
  #[number] position: i64,
) -> Result<u32, FsError> {
  let file = file_for_fd(state, fd)?;
  write_with_position(file, buf, position)
}

// Buffer string encodings (`Buffer.from(str, encoding)` / `normalizeEncoding`).
#[derive(Clone, Copy)]
enum BufEnc {
  Utf8,
  Ascii,
  Latin1,
  Ucs2,
  Base64,
  Base64Url,
  Hex,
}

// Replicates `normalizeEncoding`: maps an encoding-name v8 value to a `BufEnc`.
// `undefined`/`null` -> utf8 (the `Buffer.from` default); an unrecognized name
// yields `None` so the caller can throw ERR_UNKNOWN_ENCODING like `Buffer.from`.
fn parse_buf_encoding(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
) -> Option<BufEnc> {
  if value.is_null_or_undefined() {
    return Some(BufEnc::Utf8);
  }
  if !value.is_string() {
    return None;
  }
  let s = value.to_rust_string_lossy(scope).to_ascii_lowercase();
  Some(match s.as_str() {
    "utf8" | "utf-8" => BufEnc::Utf8,
    "ucs2" | "ucs-2" | "utf16le" | "utf-16le" => BufEnc::Ucs2,
    "latin1" | "binary" => BufEnc::Latin1,
    "ascii" => BufEnc::Ascii,
    "base64" => BufEnc::Base64,
    "base64url" => BufEnc::Base64Url,
    "hex" => BufEnc::Hex,
    _ => return None,
  })
}

fn err_unknown_encoding(name: &str) -> NodeArgError {
  NodeArgError {
    class: deno_error::builtin_classes::TYPE_ERROR,
    code: "ERR_UNKNOWN_ENCODING",
    message: format!("Unknown encoding: {name}"),
  }
}

// Encodes a v8 string to bytes per `encoding`, matching `Buffer.from(str, enc)`.
// latin1/ascii take the low byte of each UTF-16 code unit; ucs2 writes 2 LE
// bytes per unit; hex/base64 decode forgivingly (stop at / skip bad input)
// like Node. The hex odd-length case is rejected earlier by `validateEncoding`.
fn encode_js_string(
  scope: &mut v8::PinScope<'_, '_>,
  s: v8::Local<v8::String>,
  encoding: BufEnc,
) -> Vec<u8> {
  match encoding {
    BufEnc::Utf8 => {
      let len = s.utf8_length(scope);
      let mut out = Vec::with_capacity(len);
      let written = s.write_utf8_uninit_v2(
        scope,
        &mut out.spare_capacity_mut()[..len],
        v8::WriteFlags::kReplaceInvalidUtf8,
        None,
      );
      // SAFETY: write_utf8_uninit_v2 initialized exactly `written` bytes.
      unsafe { out.set_len(written) };
      out
    }
    BufEnc::Latin1 | BufEnc::Ascii => {
      let len = s.length();
      let mut out = Vec::with_capacity(len);
      s.write_one_byte_uninit_v2(
        scope,
        0,
        &mut out.spare_capacity_mut()[..len],
        v8::WriteFlags::empty(),
      );
      // SAFETY: write_one_byte_uninit_v2 initialized exactly `len` bytes.
      unsafe { out.set_len(len) };
      out
    }
    BufEnc::Ucs2 => {
      let len = s.length();
      let mut units = vec![0u16; len];
      s.write_v2(scope, 0, &mut units, v8::WriteFlags::empty());
      let mut out = Vec::with_capacity(len * 2);
      for unit in units {
        out.extend_from_slice(&unit.to_le_bytes());
      }
      out
    }
    BufEnc::Hex => {
      // Node decodes hex pairs left to right, stopping at the first byte that
      // isn't a valid pair of hex digits.
      let text = s.to_rust_string_lossy(scope);
      let bytes = text.as_bytes();
      let mut out = Vec::with_capacity(bytes.len() / 2);
      let mut i = 0;
      while i + 1 < bytes.len() {
        match (hex_val(bytes[i]), hex_val(bytes[i + 1])) {
          (Some(h), Some(l)) => out.push((h << 4) | l),
          _ => break,
        }
        i += 2;
      }
      out
    }
    BufEnc::Base64 | BufEnc::Base64Url => {
      use base64::Engine as _;
      let text = s.to_rust_string_lossy(scope);
      let cleaned: String =
        text.chars().filter(|c| !c.is_ascii_whitespace()).collect();
      let (padded, no_pad) = match encoding {
        BufEnc::Base64Url => (
          base64::engine::general_purpose::URL_SAFE,
          base64::engine::general_purpose::URL_SAFE_NO_PAD,
        ),
        _ => (
          base64::engine::general_purpose::STANDARD,
          base64::engine::general_purpose::STANDARD_NO_PAD,
        ),
      };
      padded
        .decode(&cleaned)
        .or_else(|_| no_pad.decode(cleaned.trim_end_matches('=')))
        .unwrap_or_default()
    }
  }
}

fn hex_val(b: u8) -> Option<u8> {
  match b {
    b'0'..=b'9' => Some(b - b'0'),
    b'a'..=b'f' => Some(b - b'a' + 10),
    b'A'..=b'F' => Some(b - b'A' + 10),
    _ => None,
  }
}

// Replicates `getValidatedFd`: `-0` -> 0, otherwise `validateInt32(fd,"fd",0)`.
// This is node's BINDING-level check (node_file.cc GetValidatedFd), whose C++
// message renders a bigint without inspect's `n` suffix -- `(2)`, where the
// JS validators say `(2n)`.
fn validate_fd_value(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
) -> Result<i32, FsError> {
  if value.is_big_int() {
    return Err(
      NodeArgError {
        class: deno_error::builtin_classes::TYPE_ERROR,
        code: "ERR_INVALID_ARG_TYPE",
        message: format!(
          "The \"fd\" argument must be of type number. Received type bigint ({})",
          value.to_rust_string_lossy(scope)
        ),
      }
      .into(),
    );
  }
  Ok(validate_integer(scope, value, "fd", 0, i32::MAX as i64)? as i32)
}

// `ERR_OUT_OF_RANGE` message uses node's number formatting for the received
// value, matching `validateInteger`/`err_out_of_range` callers.
fn err_oor_int(name: &str, range: &str, got: i64) -> NodeArgError {
  err_out_of_range(name, range, &fmt_num(got as f64))
}

// Replicates `validateOffsetLengthWrite(offset, length, byteLength)`.
fn validate_offset_length_write(
  offset: i64,
  length: i64,
  byte_length: i64,
) -> Result<(), FsError> {
  if offset > byte_length {
    return Err(
      err_oor_int("offset", &format!("<= {byte_length}"), offset).into(),
    );
  }
  if length > byte_length - offset {
    return Err(
      err_oor_int("length", &format!("<= {}", byte_length - offset), length)
        .into(),
    );
  }
  if length < 0 {
    return Err(err_oor_int("length", ">= 0", length).into());
  }
  // validateInt32(length, "length", 0)
  if length > i32::MAX as i64 {
    return Err(err_oor_int("length", ">= 0 && <= 2147483647", length).into());
  }
  Ok(())
}

// Resolves the arguments of `fs.write`/`fs.writeSync`: replicates the JS
// overload resolution (buffer vs string, options-object form, offset/length/
// position defaults), validation, `validateEncoding` (hex odd-length) and
// string encoding (`Buffer.from`). Returns `(fd, bytes-to-write, position)`
// with `position < 0` meaning the current position.
//
// `is_async` selects the few `fs.write` differences: function-typed args (the
// callback in any trailing slot) coerce to their defaults, and the string form
// takes its write position from the `offsetOrOptions` slot, with the encoding
// from the `length` slot only when an explicit callback follows it. The sync
// `fs.writeSync` string form instead uses the `length` slot as the encoding and
// its 5th positional arg as the position (preserving the polyfill's behavior).
//
// Arguments arrive as raw v8 values (a missing trailing arg is `undefined`),
// which lets a single op cover all overloads without `#[varargs]` — async ops
// can't take varargs, but explicit positional `v8::Value` params work for both.
#[allow(
  clippy::too_many_arguments,
  reason = "mirrors node's fs.write overload signature (fd, buffer, offset, length, position)"
)]
fn resolve_write(
  scope: &mut v8::PinScope<'_, '_>,
  fd_v: v8::Local<v8::Value>,
  buffer: v8::Local<v8::Value>,
  offset_or_options: v8::Local<v8::Value>,
  length_v: v8::Local<v8::Value>,
  position_v: v8::Local<v8::Value>,
  is_async: bool,
) -> Result<(i32, Vec<u8>, i64), FsError> {
  let fd = validate_fd_value(scope, fd_v)?;
  if let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(buffer) {
    let byte_length = view.byte_length() as i64;
    // Options-object form `write(fd, buf, { offset, length, position }, ...)`.
    let (offset_v, len_v, pos_v) = if offset_or_options.is_object()
      && !offset_or_options.is_function()
      && v8::Local::<v8::ArrayBufferView>::try_from(offset_or_options).is_err()
    {
      let obj = v8::Local::<v8::Object>::try_from(offset_or_options).unwrap();
      (
        get_prop(scope, obj, "offset"),
        get_prop(scope, obj, "length"),
        get_prop(scope, obj, "position"),
      )
    } else {
      (offset_or_options, length_v, position_v)
    };
    let offset = if offset_v.is_null_or_undefined()
      || (is_async && offset_v.is_function())
    {
      0
    } else {
      validate_integer(scope, offset_v, "offset", 0, MAX_SAFE_INTEGER)?
    };
    let length = if len_v.is_number() {
      len_v.number_value(scope).unwrap_or(0.0) as i64
    } else {
      byte_length - offset
    };
    let position = clamp_position(scope, pos_v);
    validate_offset_length_write(offset, length, byte_length)?;
    let (start, end) = (offset as usize, (offset + length) as usize);
    // copy_contents copies from the view's start, so copy the [0, end)
    // prefix and shift the [start, end) range down -- one allocation and
    // (for the common offset-0 case) one copy of exactly the write span.
    let mut bytes = vec![0u8; end];
    view.copy_contents(&mut bytes);
    if start != 0 {
      bytes.copy_within(start.., 0);
    }
    bytes.truncate(end - start);
    Ok((fd, bytes, position))
  } else {
    let Ok(s) = v8::Local::<v8::String>::try_from(buffer) else {
      return Err(
        err_invalid_arg_type(
          scope,
          "buffer",
          &["string", "Buffer", "TypedArray", "DataView"],
          buffer,
        )
        .into(),
      );
    };
    let encoding_v = if is_async {
      // `write(fd, string[, position[, encoding]], cb)`: the encoding is the
      // `length` slot only when the `position` slot holds the callback.
      if position_v.is_function() {
        length_v
      } else {
        v8::undefined(scope).into()
      }
    } else {
      length_v
    };
    // `Buffer.from(str, enc)` semantics: a non-string encoding (e.g. a number
    // in the length slot) falls back to utf8; an unrecognized string throws.
    let enc = if encoding_v.is_string() {
      match parse_buf_encoding(scope, encoding_v) {
        Some(e) => e,
        None => {
          return Err(
            err_unknown_encoding(&encoding_v.to_rust_string_lossy(scope))
              .into(),
          );
        }
      }
    } else {
      BufEnc::Utf8
    };
    // validateEncoding: hex requires even string length.
    if matches!(enc, BufEnc::Hex) && s.length() % 2 != 0 {
      return Err(
        NodeArgError {
          class: deno_error::builtin_classes::TYPE_ERROR,
          code: "ERR_INVALID_ARG_VALUE",
          message: format!(
            "The argument 'encoding' is invalid for data of length {}. Received {}",
            s.length(),
            inspect_encoding(scope, encoding_v),
          ),
        }
        .into(),
      );
    }
    let position = clamp_position(
      scope,
      if is_async {
        offset_or_options
      } else {
        position_v
      },
    );
    Ok((fd, encode_js_string(scope, s, enc), position))
  }
}

// `fs.writeSync(fd, buffer|string, offsetOrOptions?, length?, position?)`:
// full overload resolution + write, returning bytes written.
#[op2(fast, stack_trace)]
#[smi]
pub fn op_node_fs_write_v_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  fd_v: v8::Local<v8::Value>,
  buffer: v8::Local<v8::Value>,
  offset_or_options: v8::Local<v8::Value>,
  length_v: v8::Local<v8::Value>,
  position_v: v8::Local<v8::Value>,
) -> Result<u32, FsError> {
  let (fd, bytes, position) = resolve_write(
    scope,
    fd_v,
    buffer,
    offset_or_options,
    length_v,
    position_v,
    false,
  )?;
  let file = file_for_fd(state, fd).map_err(|_| ebadf_node("write"))?;
  write_with_position(file, &bytes, position)
}

// Reads `obj[key]` returning `undefined` on a missing/exception result.
fn get_prop<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  obj: v8::Local<v8::Object>,
  key: &str,
) -> v8::Local<'a, v8::Value> {
  let k = intern_key(scope, key);
  obj
    .get(scope, k.into())
    .unwrap_or_else(|| v8::undefined(scope).into())
}

// Inspect-style rendering of the encoding value for the ERR_INVALID_ARG_VALUE
// message (`Received 'hex'`).
fn inspect_encoding(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
) -> String {
  if value.is_string() {
    format!("'{}'", value.to_rust_string_lossy(scope))
  } else if value.is_undefined() {
    "undefined".to_string()
  } else {
    value.to_rust_string_lossy(scope)
  }
}

fn clamp_position(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
) -> i64 {
  if value.is_number() {
    let p = value.number_value(scope).unwrap_or(-1.0) as i64;
    if p >= 0 { p } else { -1 }
  } else {
    -1
  }
}

// `Buffer.prototype`, stored once at fs.ts module init so ops can return real
// node `Buffer`s (a `Buffer` is a `Uint8Array` with a different prototype).
struct NodeBufferPrototype(v8::Global<v8::Object>);

#[op2(fast)]
pub fn op_node_fs_set_buffer_prototype(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  proto: v8::Local<v8::Object>,
) {
  state.put(NodeBufferPrototype(v8::Global::new(scope, proto)));
}

fn buffer_proto(state: &OpState) -> Option<v8::Global<v8::Object>> {
  state
    .try_borrow::<NodeBufferPrototype>()
    .map(|p| p.0.clone())
}

// Decodes bytes to a JS string per `Buffer.prototype.toString(encoding)`:
// utf8 -> WHATWG-lossy; ascii -> high bit stripped; latin1 -> one code unit
// per byte; ucs2 -> LE u16 pairs (a trailing odd byte is dropped); hex/
// base64/base64url -> the encoded ASCII text.
fn decode_bytes_to_string<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  bytes: &[u8],
  enc: BufEnc,
) -> v8::Local<'a, v8::String> {
  let normal = v8::NewStringType::Normal;
  match enc {
    BufEnc::Utf8 => {
      let decoded = String::from_utf8_lossy(bytes);
      v8::String::new_from_utf8(scope, decoded.as_bytes(), normal).unwrap()
    }
    BufEnc::Latin1 => {
      v8::String::new_from_one_byte(scope, bytes, normal).unwrap()
    }
    BufEnc::Ascii => {
      let stripped: Vec<u8> = bytes.iter().map(|b| b & 0x7f).collect();
      v8::String::new_from_one_byte(scope, &stripped, normal).unwrap()
    }
    BufEnc::Ucs2 => {
      let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
      v8::String::new_from_two_byte(scope, &units, normal).unwrap()
    }
    BufEnc::Hex => {
      use std::fmt::Write as _;
      let mut s = String::with_capacity(bytes.len() * 2);
      for b in bytes {
        let _ = write!(s, "{b:02x}");
      }
      v8::String::new_from_one_byte(scope, s.as_bytes(), normal).unwrap()
    }
    BufEnc::Base64 => {
      use base64::Engine as _;
      let s = base64::engine::general_purpose::STANDARD.encode(bytes);
      v8::String::new_from_one_byte(scope, s.as_bytes(), normal).unwrap()
    }
    BufEnc::Base64Url => {
      use base64::Engine as _;
      let s = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
      v8::String::new_from_one_byte(scope, s.as_bytes(), normal).unwrap()
    }
  }
}

// A byte payload plus how to surface it to JS: a decoded string (`Some(enc)`)
// or a node `Buffer` (`None`). The universal return type for ops whose JS
// wrappers did `encoding === "buffer" ? Buffer.from(r) : r.toString(enc)`.
// Carries the Buffer prototype captured in the op prologue, since `ToV8`
// (which runs at promise-resolve time for async ops) has no OpState access.
pub struct MaybeEncodedBytes {
  bytes: Vec<u8>,
  enc: Option<BufEnc>,
  proto: Option<v8::Global<v8::Object>>,
}

// node's ERR_STRING_TOO_LONG (name "Error"): what `buf.toString(enc)` throws
// past `buffer.constants.MAX_STRING_LENGTH` (v8's kMaxLength).
fn err_string_too_long() -> NodeArgError {
  NodeArgError {
    class: deno_error::builtin_classes::GENERIC_ERROR,
    code: "ERR_STRING_TOO_LONG",
    message: format!(
      "Cannot create a string longer than 0x{:x} characters",
      v8::String::MAX_LENGTH
    ),
  }
}

// Projects the v8 string length (UTF-16 code units) `decode_bytes_to_string`
// would produce, so an oversized decode fails in the op with node's
// ERR_STRING_TOO_LONG (`code` intact, rejecting like node's readFile of a
// > 512MiB file with an encoding) instead of panicking in the infallible
// ToV8. hex/base64/latin1/ucs2 lengths are arithmetic; utf8's byte count is
// an upper bound, decoded exactly only when it exceeds the limit.
fn check_decoded_string_length(
  bytes: &[u8],
  enc: BufEnc,
) -> Result<(), FsError> {
  const MAX: usize = v8::String::MAX_LENGTH;
  let units = match enc {
    BufEnc::Utf8 => {
      // The UTF-16 length is always <= the byte length, so anything that fits
      // in MAX bytes is fine. Past that we must determine the unit count, but
      // (like node, which leans on simdutf) avoid decoding char-by-char in the
      // common cases: pure ASCII maps 1 byte -> 1 unit (use the SIMD-optimized
      // std `is_ascii`), and even all-3-byte input yields >= len/3 units. Only
      // genuinely mixed non-ASCII input between MAX and 3*MAX bytes needs the
      // exact (slow) decode.
      if bytes.len() <= MAX {
        return Ok(());
      }
      if bytes.is_ascii() || bytes.len() / 3 > MAX {
        return Err(err_string_too_long().into());
      }
      String::from_utf8_lossy(bytes)
        .chars()
        .map(char::len_utf16)
        .sum()
    }
    BufEnc::Latin1 | BufEnc::Ascii => bytes.len(),
    BufEnc::Ucs2 => bytes.len() / 2,
    BufEnc::Hex => bytes.len() * 2,
    BufEnc::Base64 => bytes.len().div_ceil(3) * 4,
    BufEnc::Base64Url => (bytes.len() * 4).div_ceil(3),
  };
  if units > MAX {
    return Err(err_string_too_long().into());
  }
  Ok(())
}

impl MaybeEncodedBytes {
  fn new(
    state: &OpState,
    bytes: Vec<u8>,
    enc: Option<BufEnc>,
  ) -> Result<MaybeEncodedBytes, FsError> {
    let proto = buffer_proto(state);
    MaybeEncodedBytes::with_proto(bytes, enc, proto)
  }

  // For async futures, which capture the prototype in the op prologue
  // (`ToV8` runs at resolve time, without OpState access).
  fn with_proto(
    bytes: Vec<u8>,
    enc: Option<BufEnc>,
    proto: Option<v8::Global<v8::Object>>,
  ) -> Result<MaybeEncodedBytes, FsError> {
    if let Some(enc) = enc {
      check_decoded_string_length(&bytes, enc)?;
    }
    Ok(MaybeEncodedBytes {
      bytes,
      enc,
      // The prototype is only needed for the Buffer case.
      proto: if enc.is_none() { proto } else { None },
    })
  }
}

impl<'a> ToV8<'a> for MaybeEncodedBytes {
  type Error = std::convert::Infallible;
  fn to_v8(
    self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    match self.enc {
      Some(enc) => Ok(decode_bytes_to_string(scope, &self.bytes, enc).into()),
      None => {
        let len = self.bytes.len();
        let store =
          v8::ArrayBuffer::new_backing_store_from_vec(self.bytes).make_shared();
        let ab = v8::ArrayBuffer::with_backing_store(scope, &store);
        let u8a = v8::Uint8Array::new(scope, ab, 0, len).unwrap();
        if let Some(proto) = &self.proto {
          let p = v8::Local::new(scope, proto);
          u8a.set_prototype(scope, p.into());
        }
        Ok(u8a.into())
      }
    }
  }
}

// Parses the `options` argument of fs functions accepting
// `encoding | { encoding } | null | callback` (node's `getOptions` +
// `assertEncoding`): `Ok(None)` = return a Buffer (encoding "buffer"),
// `Ok(Some(enc))` = decode to a string. `default` applies when no encoding is
// given (falsy values included, matching `if (encoding)` call sites).
fn parse_encoding_options(
  scope: &mut v8::PinScope<'_, '_>,
  options: v8::Local<v8::Value>,
  default: Option<BufEnc>,
) -> Result<Option<BufEnc>, FsError> {
  let enc_v = if options.is_null_or_undefined() || options.is_function() {
    return Ok(default);
  } else if options.is_string() {
    options
  } else if options.is_object() {
    let obj = v8::Local::<v8::Object>::try_from(options).unwrap();
    get_prop(scope, obj, "encoding")
  } else {
    return Err(
      err_invalid_arg_type(scope, "options", &["string", "Object"], options)
        .into(),
    );
  };
  // Falsy encodings (undefined/null/""/0/false) keep the default.
  if !enc_v.boolean_value(scope) {
    return Ok(default);
  }
  if enc_v.is_string() {
    let s = enc_v.to_rust_string_lossy(scope);
    if s == "buffer" {
      return Ok(None);
    }
  }
  match parse_buf_encoding(scope, enc_v) {
    Some(e) => Ok(Some(e)),
    None => Err(
      err_invalid_arg_value_received(
        "encoding",
        "is invalid encoding",
        &inspect_encoding(scope, enc_v),
      )
      .into(),
    ),
  }
}

#[cfg(unix)]
const NODE_O_SYNC: i32 = libc::O_SYNC;
// On Windows node's fs.constants has no O_SYNC; `x | undefined` is `x` in JS.
#[cfg(windows)]
const NODE_O_SYNC: i32 = 0;

// Replicates node's `stringToFlags`: numeric flags pass through (validateInt32),
// null/undefined -> O_RDONLY, known flag strings map to O_* combos, anything
// else throws ERR_INVALID_ARG_VALUE. Bit values are libc's, matching both the
// JS constants (ext:deno_node/internal_binding/constants.ts) and
// `OpenOptions::from(i32)`.
fn string_to_flags(
  scope: &mut v8::PinScope<'_, '_>,
  flags: v8::Local<v8::Value>,
  name: &str,
) -> Result<i32, FsError> {
  if flags.is_number() {
    return Ok(validate_integer(
      scope,
      flags,
      name,
      i32::MIN as i64,
      i32::MAX as i64,
    )? as i32);
  }
  if flags.is_null_or_undefined() {
    return Ok(libc::O_RDONLY);
  }
  let flag_str = if flags.is_string() {
    flags.to_rust_string_lossy(scope)
  } else {
    String::new()
  };
  let mapped = match flag_str.as_str() {
    "r" => libc::O_RDONLY,
    "rs" | "sr" => libc::O_RDONLY | NODE_O_SYNC,
    "r+" => libc::O_RDWR,
    "rs+" | "sr+" => libc::O_RDWR | NODE_O_SYNC,
    "w" => libc::O_TRUNC | libc::O_CREAT | libc::O_WRONLY,
    "wx" | "xw" => {
      libc::O_TRUNC | libc::O_CREAT | libc::O_WRONLY | libc::O_EXCL
    }
    "w+" => libc::O_TRUNC | libc::O_CREAT | libc::O_RDWR,
    "wx+" | "xw+" => {
      libc::O_TRUNC | libc::O_CREAT | libc::O_RDWR | libc::O_EXCL
    }
    "a" => libc::O_APPEND | libc::O_CREAT | libc::O_WRONLY,
    "ax" | "xa" => {
      libc::O_APPEND | libc::O_CREAT | libc::O_WRONLY | libc::O_EXCL
    }
    "as" | "sa" => {
      libc::O_APPEND | libc::O_CREAT | libc::O_WRONLY | NODE_O_SYNC
    }
    "a+" => libc::O_APPEND | libc::O_CREAT | libc::O_RDWR,
    "ax+" | "xa+" => {
      libc::O_APPEND | libc::O_CREAT | libc::O_RDWR | libc::O_EXCL
    }
    "as+" | "sa+" => {
      libc::O_APPEND | libc::O_CREAT | libc::O_RDWR | NODE_O_SYNC
    }
    _ => {
      return Err(
        err_invalid_arg_value_received(
          name,
          "is invalid",
          &inspect_encoding(scope, flags),
        )
        .into(),
      );
    }
  };
  Ok(mapped)
}

// `fs.write(fd, buffer|string, offsetOrOptions?, length?, position?, cb)`: the
// async analogue of `op_node_fs_write_v_sync`. Validates synchronously in the
// eager prologue (so bad args throw at the call site like Node), then writes on
// the event loop. The callback (in whatever trailing slot) is handled in JS,
// which re-attaches the original buffer/string to the completion callback.
// `deferred` (not the eager_throw default of lazy) so the write is eager-polled:
// it must hit the file before a subsequent synchronous `writeSync` to match
// node's ordering. `nofast` keeps it slow-only as it was before `deferred` made
// it fast-eligible.
#[op2(async(deferred), async(eager_throw), nofast, stack_trace)]
#[smi]
pub fn op_node_fs_write_v(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  fd_v: v8::Local<v8::Value>,
  buffer: v8::Local<v8::Value>,
  offset_or_options: v8::Local<v8::Value>,
  length_v: v8::Local<v8::Value>,
  position_v: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<u32, FsError>> + use<>, FsError> {
  let (fd, bytes, position) = resolve_write(
    scope,
    fd_v,
    buffer,
    offset_or_options,
    length_v,
    position_v,
    true,
  )?;
  let file = file_for_fd(state, fd).map_err(|_| ebadf_node("write"));
  Ok(async move { write_with_position(file?, &bytes, position) })
}

// Validates readv/writev's (fd, buffers, position) like node: fd must be a
// number (then `getValidatedFd`), `buffers` an array of `ArrayBufferView`s
// (`validateBufferArray`), and a numeric `position` a non-negative integer
// (non-numbers mean "current position", returned as -1).
fn validate_vectored_args<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  fd_v: v8::Local<v8::Value>,
  buffers_v: v8::Local<'a, v8::Value>,
  position_v: v8::Local<v8::Value>,
) -> Result<(i32, v8::Local<'a, v8::Array>, i64), FsError> {
  if !fd_v.is_number() {
    return Err(err_invalid_arg_type(scope, "fd", &["number"], fd_v).into());
  }
  let fd = validate_fd_value(scope, fd_v)?;
  let buffers = v8::Local::<v8::Array>::try_from(buffers_v).map_err(|_| {
    err_invalid_arg_type(scope, "buffers", &["ArrayBufferView[]"], buffers_v)
  })?;
  for i in 0..buffers.length() {
    let elem = buffers
      .get_index(scope, i)
      .unwrap_or_else(|| v8::undefined(scope).into());
    if v8::Local::<v8::ArrayBufferView>::try_from(elem).is_err() {
      return Err(
        err_invalid_arg_type(
          scope,
          "buffers",
          &["ArrayBufferView[]"],
          buffers_v,
        )
        .into(),
      );
    }
  }
  let position = if position_v.is_number() {
    validate_integer(scope, position_v, "position", 0, MAX_SAFE_INTEGER)?
  } else {
    -1
  };
  Ok((fd, buffers, position))
}

// Gathers `buffers`' bytes into one contiguous Vec (node uses Buffer.concat).
fn concat_buffer_views(
  scope: &mut v8::PinScope<'_, '_>,
  buffers: v8::Local<v8::Array>,
) -> Vec<u8> {
  let mut combined: Vec<u8> = Vec::new();
  for i in 0..buffers.length() {
    let Some(elem) = buffers.get_index(scope, i) else {
      continue;
    };
    let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(elem) else {
      continue;
    };
    let len = view.byte_length();
    let start = combined.len();
    combined.resize(start + len, 0);
    view.copy_contents(&mut combined[start..]);
  }
  combined
}

// `fs.writevSync(fd, buffers, position)` end to end: validates the args,
// gathers the views into one contiguous buffer, and writes it at `position`
// (-1 = current). An empty `buffers` returns 0 without touching the fd
// (matching node's pre-I/O short-circuit). Returns bytes written.
#[op2(fast, stack_trace)]
#[smi]
pub fn op_node_fs_writev_sync<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  state: &mut OpState,
  fd: v8::Local<v8::Value>,
  buffers: v8::Local<'a, v8::Value>,
  position: v8::Local<v8::Value>,
) -> Result<u32, FsError> {
  let (fd, buffers, position) =
    validate_vectored_args(scope, fd, buffers, position)?;
  if buffers.length() == 0 {
    return Ok(0);
  }
  let combined = concat_buffer_views(scope, buffers);
  let file = file_for_fd(state, fd).map_err(|_| ebadf_node("write"))?;
  write_with_position(file, &combined, position)
}

// `fs.writev(fd, buffers, position)`: async analogue of writevSync. Validates
// + gathers in the sync prologue (eager_throw, so bad args throw at the call
// site like node), then writes on the event loop. `deferred` (not the eager_throw
// default of lazy) so the write is eager-polled, matching node's ordering vs a
// later writeSync; `nofast` keeps it slow-only as before.
#[op2(async(deferred), async(eager_throw), nofast, stack_trace)]
#[smi]
pub fn op_node_fs_writev<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  state: &mut OpState,
  fd: v8::Local<v8::Value>,
  buffers: v8::Local<'a, v8::Value>,
  position: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<u32, FsError>> + use<>, FsError> {
  let (fd, buffers, position) =
    validate_vectored_args(scope, fd, buffers, position)?;
  let empty = buffers.length() == 0;
  let combined = concat_buffer_views(scope, buffers);
  // node short-circuits empty writes before any fd use.
  let file = if empty {
    None
  } else {
    Some(file_for_fd(state, fd).map_err(|_| ebadf_node("write")))
  };
  Ok(async move {
    match file {
      None => Ok(0),
      Some(file) => write_with_position(file?, &combined, position),
    }
  })
}

// Writes all of `buf` to `file`, mapping write failures to a node error with
// `syscall: "write"` (no path, matching `denoWriteFileErrorToNodeError`).
fn write_all_node(
  file: Rc<dyn deno_io::fs::File>,
  buf: &[u8],
) -> Result<(), FsError> {
  let mut total = 0usize;
  while total < buf.len() {
    let nwritten = file.clone().write_sync(&buf[total..]).map_err(|e| {
      map_fs_error_to_node_fs_error(
        remap_write_access_denied(e),
        NodeFsErrorContext {
          syscall: Some("write".into()),
          ..Default::default()
        },
      )
    })?;
    total += nwritten;
  }
  Ok(())
}

// Resolves `fs.writeFile{,Sync}`/`fs.appendFile{,Sync}`'s (data, options)
// arguments: returns (bytes-to-write, open flags, mode). Replicates node's
// order: encoding is validated first (`getOptions`/`assertEncoding`), then
// `data` (`validateStringAfterArrayBufferView` + `Buffer.from(str,
// encoding)`), then `options.flag` (default "w", or "a" for the appendFile
// variants) and `options.mode`.
fn resolve_write_file_args(
  scope: &mut v8::PinScope<'_, '_>,
  data: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
  append: bool,
) -> Result<(Vec<u8>, i32, Option<u32>), FsError> {
  let enc = parse_encoding_options(scope, options, Some(BufEnc::Utf8))?;
  let bytes = if let Ok(view) = v8::Local::<v8::ArrayBufferView>::try_from(data)
  {
    let mut all = vec![0u8; view.byte_length()];
    view.copy_contents(&mut all);
    all
  } else if let Ok(s) = v8::Local::<v8::String>::try_from(data) {
    // Encoding "buffer" is not a real data encoding; `Buffer.from(str,
    // "buffer")` throws ERR_UNKNOWN_ENCODING in node.
    let Some(enc) = enc else {
      return Err(err_unknown_encoding("buffer").into());
    };
    encode_js_string(scope, s, enc)
  } else {
    return Err(
      err_invalid_arg_type(
        scope,
        "data",
        &["string", "Buffer", "TypedArray", "DataView"],
        data,
      )
      .into(),
    );
  };
  let (flag_v, mode_v) = if options.is_object() && !options.is_function() {
    let obj = v8::Local::<v8::Object>::try_from(options).unwrap();
    (get_prop(scope, obj, "flag"), get_prop(scope, obj, "mode"))
  } else {
    let undef: v8::Local<v8::Value> = v8::undefined(scope).into();
    (undef, undef)
  };
  let flags = if flag_v.is_null_or_undefined() {
    // writeFile's default flag is "w" (appendFile's is "a"), not
    // stringToFlags' "r".
    if append {
      libc::O_APPEND | libc::O_CREAT | libc::O_WRONLY
    } else {
      libc::O_TRUNC | libc::O_CREAT | libc::O_WRONLY
    }
  } else {
    string_to_flags(scope, flag_v, "options.flag")?
  };
  let mode = if mode_v.is_null_or_undefined() {
    None
  } else {
    Some(parse_file_mode(scope, mode_v, "mode", None)?)
  };
  Ok((bytes, flags, mode))
}

// `fs.writeFileSync(pathOrFd, data, options)` end to end: parses options
// (encoding/flag/mode) and data (string -> encoded bytes) natively; for an
// fd, writes all bytes; for a path, opens (flags + default 0o666), optionally
// chmods to `mode`, writes all bytes, then closes (the file `Rc` drops at
// scope end, including error paths). Emits the final node errors
// (open -> "open" w/ path, write -> "write"). `append` selects the
// appendFile{,Sync} variant (default flag "a"; for an fd the flag is moot --
// like node, the write goes to the fd as-is).
fn write_file_sync_impl(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path_or_rid: v8::Local<v8::Value>,
  data: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
  append: bool,
  api_name: &str,
) -> Result<(), FsError> {
  let (bytes, flags, mode) =
    resolve_write_file_args(scope, data, options, append)?;
  if path_or_rid.is_number() {
    let fd = path_or_rid.int32_value(scope).unwrap_or(0);
    let file = file_for_fd(state, fd).map_err(|_| ebadf_node("write"))?;
    return write_all_node(file, &bytes);
  }
  let path = validate_path_to_string(scope, path_or_rid, "path")?;
  let open_options = get_open_options(flags, Some(0o666));
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(&path)),
    open_options_to_access_kind(&open_options),
    Some(api_name),
  )?;
  let fs = state.borrow::<FileSystemRc>().clone();
  let file = fs
    .open_sync(&checked, open_options)
    .map_err(|e| node_fs_err(e, "open", &path))?;
  if let Some(mode) = mode {
    file
      .clone()
      .chmod_sync(mode)
      .map_err(|e| node_fs_err(e, "chmod", &path))?;
  }
  write_all_node(file, &bytes)
}

#[op2(fast, stack_trace)]
pub fn op_node_fs_write_file_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path_or_rid: v8::Local<v8::Value>,
  data: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  write_file_sync_impl(
    scope,
    state,
    path_or_rid,
    data,
    options,
    false,
    "node:fs.writeFileSync",
  )
}

// `fs.appendFileSync(pathOrFd, data, options)`: writeFileSync with node's
// appendFile option handling (default flag "a") done natively, so the public
// API is a direct op binding.
#[op2(fast, stack_trace)]
pub fn op_node_fs_append_file_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path_or_rid: v8::Local<v8::Value>,
  data: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  write_file_sync_impl(
    scope,
    state,
    path_or_rid,
    data,
    options,
    true,
    "node:fs.appendFileSync",
  )
}

// Async `fs.writeFile`/`fs.appendFile` for the common case (no AbortSignal,
// no custom iterable — those stay in JS), both path and fd. Validates
// synchronously in the eager prologue; open + optional chmod + write-all +
// close run on the event loop. The file `Rc` drops at scope end. Emits node
// errors (open -> "open", write -> "write").
fn write_file_async_impl(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path_or_rid: v8::Local<v8::Value>,
  data: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
  append: bool,
  cancel_rid: Option<ResourceId>,
  api_name: &str,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let cancel = cancel_handle_for(state, cancel_rid);
  let (bytes, flags, mode) =
    resolve_write_file_args(scope, data, options, append)?;
  enum Target {
    // The lookup Result is deferred into the future so a bad fd surfaces as
    // EBADF via the callback (node) rather than a synchronous throw.
    Fd(Result<Rc<dyn deno_io::fs::File>, FsError>),
    Path(String, CheckedPathBuf),
  }
  let target = if path_or_rid.is_number() {
    let fd = path_or_rid.int32_value(scope).unwrap_or(0);
    Target::Fd(file_for_fd(state, fd).map_err(|_| ebadf_node("write")))
  } else {
    let path = validate_path_to_string(scope, path_or_rid, "path")?;
    let open_options = get_open_options(flags, Some(0o666));
    let checked = state
      .borrow_mut::<PermissionsContainer>()
      .check_open(
        Cow::Owned(PathBuf::from(&path)),
        open_options_to_access_kind(&open_options),
        Some(api_name),
      )?
      .into_owned();
    Target::Path(path, checked)
  };
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(with_cancel_handle(
    async move {
      let file = match target {
        Target::Fd(file) => file?,
        Target::Path(path, checked) => {
          let open_options = get_open_options(flags, Some(0o666));
          let file = fs
            .open_async(checked, open_options)
            .await
            .map_err(|e| node_fs_err(e, "open", &path))?;
          if let Some(mode) = mode {
            file
              .clone()
              .chmod_async(mode)
              .await
              .map_err(|e| node_fs_err(e, "chmod", &path))?;
          }
          file
        }
      };
      write_all_node(file, &bytes)
    },
    cancel,
  ))
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_write_file(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path_or_rid: v8::Local<v8::Value>,
  data: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
  #[smi] cancel_rid: Option<ResourceId>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  write_file_async_impl(
    scope,
    state,
    path_or_rid,
    data,
    options,
    false,
    cancel_rid,
    "node:fs.writeFile",
  )
}

// Async `fs.appendFile` for the common case (no AbortSignal, no custom
// iterable): writeFile with node's appendFile option handling (default flag
// "a") done natively.
#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_append_file(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path_or_rid: v8::Local<v8::Value>,
  data: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  write_file_async_impl(
    scope,
    state,
    path_or_rid,
    data,
    options,
    true,
    None,
    "node:fs.appendFile",
  )
}

// `fs.truncateSync(path, len)`: node opens 'r+' (so ENOENT etc. surface with
// syscall="open"), ftruncates to `max(0, len)`, then closes (the file `Rc`
// drops at scope end). `len` is validated like node's `validateInteger`.
#[op2(fast, stack_trace)]
#[undefined]
pub fn op_node_fs_truncate_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  len: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  // node's `truncateSync(path, len = 0)`: a missing/undefined len defaults to 0
  // (so the op can be bound directly as the public API); otherwise it's
  // validated like node's ftruncate.
  let len = if len.is_undefined() {
    0
  } else {
    validate_integer(scope, len, "len", MIN_SAFE_INTEGER, MAX_SAFE_INTEGER)?
      .max(0) as u64
  };
  let options = OpenOptions {
    read: true,
    write: true,
    ..Default::default()
  };
  let fs = state.borrow::<FileSystemRc>().clone();
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(&path)),
    OpenAccessKind::ReadWrite,
    Some("node:fs.truncateSync"),
  )?;
  let file = fs
    .open_sync(&checked, options)
    .map_err(|e| node_fs_err(e, "open", &path))?;
  file
    .truncate_sync(len)
    .map_err(|e| node_fs_err(e, "ftruncate", &path))?;
  Ok(())
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_truncate(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  len: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  // node's `truncate(path, len = 0, cb)`: a missing/undefined len defaults to 0
  // (so the op can be bound with `callbackifyOpt`); otherwise validateInteger.
  let len = if len.is_undefined() {
    0
  } else {
    validate_integer(scope, len, "len", MIN_SAFE_INTEGER, MAX_SAFE_INTEGER)?
      .max(0) as u64
  };
  let options = OpenOptions {
    read: true,
    write: true,
    ..Default::default()
  };
  let fs = state.borrow::<FileSystemRc>().clone();
  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      OpenAccessKind::ReadWrite,
      Some("node:fs.truncate"),
    )?
    .into_owned();
  Ok(async move {
    let file = fs
      .open_async(checked, options)
      .await
      .map_err(|e| node_fs_err(e, "open", &path))?;
    file
      .truncate_async(len)
      .await
      .map_err(|e| node_fs_err(e, "ftruncate", &path))?;
    Ok(())
  })
}

/// Async write for node:fs. Performs the write synchronously but resolves
/// the promise on the next event loop tick.
#[allow(
  clippy::unused_async,
  reason = "async required for deferred op scheduling"
)]
#[op2(async(deferred))]
#[smi]
pub async fn op_node_fs_write_deferred(
  state: Rc<RefCell<OpState>>,
  fd: i32,
  #[buffer] buf: JsBuffer,
  #[number] position: i64,
) -> Result<u32, FsError> {
  let file = file_for_fd(&state.borrow(), fd)?;
  write_with_position(file, &buf, position)
}

#[op2(fast)]
#[number]
pub fn op_node_fs_seek_sync(
  state: &mut OpState,
  fd: i32,
  #[number] offset: i64,
  #[smi] whence: i32,
) -> Result<u64, FsError> {
  let file = file_for_fd(state, fd)?;
  let seek_from = match whence {
    0 => std::io::SeekFrom::Start(offset as u64),
    1 => std::io::SeekFrom::Current(offset),
    2 => std::io::SeekFrom::End(offset),
    _ => {
      return Err(FsError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "invalid whence",
      )));
    }
  };
  let pos = file.seek_sync(seek_from)?;
  Ok(pos)
}

#[op2]
#[number]
pub async fn op_node_fs_seek(
  state: Rc<RefCell<OpState>>,
  fd: i32,
  #[number] offset: i64,
  #[smi] whence: i32,
) -> Result<u64, FsError> {
  let file = file_for_fd(&state.borrow(), fd)?;
  let seek_from = match whence {
    0 => std::io::SeekFrom::Start(offset as u64),
    1 => std::io::SeekFrom::Current(offset),
    2 => std::io::SeekFrom::End(offset),
    _ => {
      return Err(FsError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "invalid whence",
      )));
    }
  };
  let pos = file.seek_async(seek_from).await?;
  Ok(pos)
}

/// Stat result returned to JS. Uses f64 for numeric fields to ensure
/// they always serialize as JS Number (not BigInt). BigInt conversion
/// for the bigint stat API is handled on the JS side by CFISBIS.
#[derive(ToV8)]
pub struct NodeFsStat {
  pub is_file: bool,
  pub is_directory: bool,
  pub is_symlink: bool,
  pub size: f64,
  pub mtime_ms: Option<f64>,
  pub atime_ms: Option<f64>,
  pub birthtime_ms: Option<f64>,
  pub ctime_ms: Option<f64>,
  pub dev: f64,
  pub ino: f64,
  pub mode: u32,
  pub nlink: f64,
  pub uid: u32,
  pub gid: u32,
  pub rdev: f64,
  pub blksize: f64,
  pub blocks: f64,
  pub is_block_device: bool,
  pub is_char_device: bool,
  pub is_fifo: bool,
  pub is_socket: bool,
}

impl From<deno_io::fs::FsStat> for NodeFsStat {
  fn from(stat: deno_io::fs::FsStat) -> Self {
    NodeFsStat {
      is_file: stat.is_file,
      is_directory: stat.is_directory,
      is_symlink: stat.is_symlink,
      size: stat.size as f64,
      mtime_ms: stat.mtime.map(|v| v as f64),
      atime_ms: stat.atime.map(|v| v as f64),
      birthtime_ms: stat.birthtime.map(|v| v as f64),
      ctime_ms: stat.ctime.map(|v| v as f64),
      dev: stat.dev as f64,
      ino: stat.ino.unwrap_or(0) as f64,
      mode: stat.mode,
      nlink: stat.nlink.unwrap_or(0) as f64,
      uid: stat.uid,
      gid: stat.gid,
      rdev: stat.rdev as f64,
      blksize: stat.blksize as f64,
      blocks: stat.blocks.unwrap_or(0) as f64,
      is_block_device: stat.is_block_device,
      is_char_device: stat.is_char_device,
      is_fifo: stat.is_fifo,
      is_socket: stat.is_socket,
    }
  }
}

// node `fs.Stats` as a cppgc object, replacing the JS `Stats`/`StatsBase`
// classes and `convertFileInfoToStats` so the result objects are built in Rust
// and the JS classes leave the snapshot. Predicate methods use the filesystem
// type bits directly, matching the `internal/fs/stat_utils.ts` override (which
// used Deno.FileInfo's `is*` flags rather than `mode & S_IFMT`).
pub struct Stats {
  dev: f64,
  ino: f64,
  mode: u32,
  nlink: f64,
  uid: u32,
  gid: u32,
  rdev: f64,
  size: f64,
  blksize: f64,
  blocks: f64,
  atime_ms: f64,
  mtime_ms: f64,
  ctime_ms: f64,
  birthtime_ms: f64,
  is_file: bool,
  is_directory: bool,
  is_symlink: bool,
  is_block_device: bool,
  is_char_device: bool,
  is_fifo: bool,
  is_socket: bool,
  is_bigint: bool,
  // node's Date properties (atime/mtime/ctime/birthtime) are writable: setting
  // one stores an override that the getter returns instead of the lazy Date.
  // Indices: 0=atime, 1=mtime, 2=ctime, 3=birthtime.
  date_overrides: RefCell<[Option<v8::Global<v8::Value>>; 4]>,
}

// SAFETY: Stats holds only plain data plus untraced v8::Global values, safe to GC.
unsafe impl GarbageCollected for Stats {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Stats"
  }
}

impl Stats {
  fn build(stat: deno_io::fs::FsStat, is_bigint: bool) -> Self {
    let n = NodeFsStat::from(stat);
    Stats {
      dev: n.dev,
      ino: n.ino,
      mode: n.mode,
      nlink: n.nlink,
      uid: n.uid,
      gid: n.gid,
      rdev: n.rdev,
      size: n.size,
      blksize: n.blksize,
      blocks: n.blocks,
      atime_ms: n.atime_ms.unwrap_or(0.0),
      mtime_ms: n.mtime_ms.unwrap_or(0.0),
      ctime_ms: n.ctime_ms.unwrap_or(0.0),
      birthtime_ms: n.birthtime_ms.unwrap_or(0.0),
      is_file: n.is_file,
      is_directory: n.is_directory,
      is_symlink: n.is_symlink,
      is_block_device: n.is_block_device,
      is_char_device: n.is_char_device,
      is_fifo: n.is_fifo,
      is_socket: n.is_socket,
      is_bigint,
      date_overrides: RefCell::new([const { None }, None, None, None]),
    }
  }

  // A Date-valued field (atime/mtime/ctime/birthtime): returns the override
  // set via the property setter, else the lazily-built Date.
  fn date_field<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    idx: usize,
    ms: f64,
  ) -> v8::Local<'a, v8::Value> {
    if let Some(g) = &self.date_overrides.borrow()[idx] {
      return v8::Local::new(scope, g);
    }
    date_val(scope, ms)
  }
}

// A numeric Stats field: a JS Number, or a BigInt for `bigint: true` stats.
fn num_val<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  v: f64,
  is_bigint: bool,
) -> v8::Local<'a, v8::Value> {
  if is_bigint {
    v8::BigInt::new_from_i64(scope, v as i64).into()
  } else {
    v8::Number::new(scope, v).into()
  }
}

// `dateFromMs` in internal/fs/utils.mjs rounds with +0.5 before Date().
fn date_val<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  ms: f64,
) -> v8::Local<'a, v8::Value> {
  match v8::Date::new(scope, ms + 0.5) {
    Some(d) => d.into(),
    None => v8::undefined(scope).into(),
  }
}

// `*Ns` nanosecond fields exist only on `bigint: true` stats (BigInt);
// `undefined` on a regular Number stat, matching node's BigIntStats.
fn ns_val<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  ms: f64,
  is_bigint: bool,
) -> v8::Local<'a, v8::Value> {
  if is_bigint {
    // Integer math: f64 can't hold ns precision at epoch scale (> 2^52).
    // ms is an integer (u64) value, so `ms as i64 * 1e6` is exact, matching
    // node's `BigInt(ms) * 1000000n`.
    v8::BigInt::new_from_i64(scope, (ms as i64) * 1_000_000).into()
  } else {
    v8::undefined(scope).into()
  }
}

#[op2]
impl Stats {
  // node's (deprecated) public `fs.Stats` constructor. Predicates derive from
  // `mode & S_IFMT` here (unlike the op-built path, which uses the FS type
  // bits). Always a Number stat.
  #[constructor]
  #[cppgc]
  #[allow(clippy::too_many_arguments)]
  fn new(
    dev: Option<f64>,
    mode: Option<f64>,
    nlink: Option<f64>,
    uid: Option<f64>,
    gid: Option<f64>,
    rdev: Option<f64>,
    blksize: Option<f64>,
    ino: Option<f64>,
    size: Option<f64>,
    blocks: Option<f64>,
    atime_ms: Option<f64>,
    mtime_ms: Option<f64>,
    ctime_ms: Option<f64>,
    birthtime_ms: Option<f64>,
  ) -> Stats {
    const S_IFMT: u32 = 0o170000;
    let m = mode.unwrap_or(0.0) as u32;
    Stats {
      dev: dev.unwrap_or(0.0),
      ino: ino.unwrap_or(0.0),
      mode: m,
      nlink: nlink.unwrap_or(0.0),
      uid: uid.unwrap_or(0.0) as u32,
      gid: gid.unwrap_or(0.0) as u32,
      rdev: rdev.unwrap_or(0.0),
      size: size.unwrap_or(0.0),
      blksize: blksize.unwrap_or(0.0),
      blocks: blocks.unwrap_or(0.0),
      atime_ms: atime_ms.unwrap_or(0.0),
      mtime_ms: mtime_ms.unwrap_or(0.0),
      ctime_ms: ctime_ms.unwrap_or(0.0),
      birthtime_ms: birthtime_ms.unwrap_or(0.0),
      is_file: (m & S_IFMT) == 0o100000,
      is_directory: (m & S_IFMT) == 0o040000,
      is_symlink: (m & S_IFMT) == 0o120000,
      is_block_device: (m & S_IFMT) == 0o060000,
      is_char_device: (m & S_IFMT) == 0o020000,
      is_fifo: (m & S_IFMT) == 0o010000,
      is_socket: (m & S_IFMT) == 0o140000,
      is_bigint: false,
      date_overrides: RefCell::new([const { None }, None, None, None]),
    }
  }

  #[getter]
  fn dev<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    num_val(scope, self.dev, self.is_bigint)
  }
  #[getter]
  fn ino<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    num_val(scope, self.ino, self.is_bigint)
  }
  #[getter]
  fn mode<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    num_val(scope, self.mode as f64, self.is_bigint)
  }
  #[getter]
  fn nlink<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    num_val(scope, self.nlink, self.is_bigint)
  }
  #[getter]
  fn uid<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    num_val(scope, self.uid as f64, self.is_bigint)
  }
  #[getter]
  fn gid<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    num_val(scope, self.gid as f64, self.is_bigint)
  }
  #[getter]
  fn rdev<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    num_val(scope, self.rdev, self.is_bigint)
  }
  #[getter]
  fn size<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    num_val(scope, self.size, self.is_bigint)
  }
  #[getter]
  fn blksize<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    num_val(scope, self.blksize, self.is_bigint)
  }
  #[getter]
  fn blocks<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    num_val(scope, self.blocks, self.is_bigint)
  }
  #[getter]
  fn atime_ms<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    num_val(scope, self.atime_ms, self.is_bigint)
  }
  #[getter]
  fn mtime_ms<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    num_val(scope, self.mtime_ms, self.is_bigint)
  }
  #[getter]
  fn ctime_ms<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    num_val(scope, self.ctime_ms, self.is_bigint)
  }
  #[getter]
  fn birthtime_ms<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    num_val(scope, self.birthtime_ms, self.is_bigint)
  }

  // `*Ns` fields only exist on `bigint: true` stats; `undefined` otherwise.
  #[getter]
  fn atime_ns<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    ns_val(scope, self.atime_ms, self.is_bigint)
  }
  #[getter]
  fn mtime_ns<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    ns_val(scope, self.mtime_ms, self.is_bigint)
  }
  #[getter]
  fn ctime_ns<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    ns_val(scope, self.ctime_ms, self.is_bigint)
  }
  #[getter]
  fn birthtime_ns<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    ns_val(scope, self.birthtime_ms, self.is_bigint)
  }

  #[getter]
  fn atime<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    self.date_field(scope, 0, self.atime_ms)
  }
  #[setter]
  fn atime(&self, scope: &mut v8::PinScope, value: v8::Local<v8::Value>) {
    self.date_overrides.borrow_mut()[0] = Some(v8::Global::new(scope, value));
  }
  #[getter]
  fn mtime<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    self.date_field(scope, 1, self.mtime_ms)
  }
  #[setter]
  fn mtime(&self, scope: &mut v8::PinScope, value: v8::Local<v8::Value>) {
    self.date_overrides.borrow_mut()[1] = Some(v8::Global::new(scope, value));
  }
  #[getter]
  fn ctime<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    self.date_field(scope, 2, self.ctime_ms)
  }
  #[setter]
  fn ctime(&self, scope: &mut v8::PinScope, value: v8::Local<v8::Value>) {
    self.date_overrides.borrow_mut()[2] = Some(v8::Global::new(scope, value));
  }
  #[getter]
  fn birthtime<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    self.date_field(scope, 3, self.birthtime_ms)
  }
  #[setter]
  fn birthtime(&self, scope: &mut v8::PinScope, value: v8::Local<v8::Value>) {
    self.date_overrides.borrow_mut()[3] = Some(v8::Global::new(scope, value));
  }

  #[fast]
  fn is_file(&self) -> bool {
    self.is_file
  }
  #[fast]
  fn is_directory(&self) -> bool {
    self.is_directory
  }
  #[fast]
  #[rename("isSymbolicLink")]
  fn is_symbolic_link(&self) -> bool {
    self.is_symlink
  }
  #[fast]
  fn is_block_device(&self) -> bool {
    self.is_block_device
  }
  #[fast]
  fn is_character_device(&self) -> bool {
    self.is_char_device
  }
  #[fast]
  #[rename("isFIFO")]
  fn is_fifo(&self) -> bool {
    self.is_fifo
  }
  #[fast]
  fn is_socket(&self) -> bool {
    self.is_socket
  }
}

// Builds the JS `Stats` object: a cppgc object (so `isFile()` etc. and the
// Date accessors live on the prototype) whose numeric value fields are then
// defined as OWN, writable, enumerable data properties — matching node, where
// `StatsBase` assigns `this.dev = ...` etc. (so `Object.hasOwn(stats, 'blksize')`
// is true). The own props shadow the prototype getters for objects built here.
fn stats_to_v8<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  s: Stats,
) -> v8::Local<'a, v8::Value> {
  let bigint = s.is_bigint;
  // node's StatsBase property order.
  let fields: [(&str, f64); 14] = [
    ("dev", s.dev),
    ("mode", s.mode as f64),
    ("nlink", s.nlink),
    ("uid", s.uid as f64),
    ("gid", s.gid as f64),
    ("rdev", s.rdev),
    ("blksize", s.blksize),
    ("ino", s.ino),
    ("size", s.size),
    ("blocks", s.blocks),
    ("atimeMs", s.atime_ms),
    ("mtimeMs", s.mtime_ms),
    ("ctimeMs", s.ctime_ms),
    ("birthtimeMs", s.birthtime_ms),
  ];
  let obj = deno_core::cppgc::make_cppgc_object(scope, s);
  for (name, val) in fields {
    // Internalized one-byte string: the field names are ASCII and reused for
    // every Stats object, so v8 dedups them into a single interned key.
    let key = v8::String::new_from_one_byte(
      scope,
      name.as_bytes(),
      v8::NewStringType::Internalized,
    )
    .unwrap();
    let v = num_val(scope, val, bigint);
    // create_data_property (not set): defines an own writable/enumerable data
    // property, bypassing the getter-only prototype accessor that `set` would
    // hit (and silently no-op on).
    obj.create_data_property(scope, key.into(), v);
  }
  obj.into()
}

// `Stats` or `undefined` (the latter for `throwIfNoEntry: false` misses),
// converted to JS via `stats_to_v8` so the result carries own data properties.
struct MaybeStats(Option<Stats>);

impl<'a> deno_core::ToV8<'a> for MaybeStats {
  type Error = std::convert::Infallible;
  fn to_v8(
    self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    Ok(match self.0 {
      Some(s) => stats_to_v8(scope, s),
      None => v8::undefined(scope).into(),
    })
  }
}

// `Some(s)` -> a JS string, `None` -> `undefined` (NOT null), so an op can be
// bound directly as a public API whose JS contract returns undefined for "no
// value" (e.g. mkdirSync's first-created-dir).
struct MaybeString(Option<String>);

impl<'a> deno_core::ToV8<'a> for MaybeString {
  type Error = std::convert::Infallible;
  fn to_v8(
    self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    Ok(match self.0 {
      Some(s) => v8::String::new(scope, &s)
        .map(|s| s.into())
        .unwrap_or_else(|| v8::undefined(scope).into()),
      None => v8::undefined(scope).into(),
    })
  }
}

// stat/lstat/fstat returning the cppgc `Stats` object directly, so the JS
// `convertFileInfoToStats`/`CFISBIS` + `Stats` classes are unnecessary.
#[op2(stack_trace)]
pub fn op_node_fs_stat_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<MaybeStats, FsError> {
  // Extract bigint/throwIfNoEntry from the options object so this op is the
  // whole public `statSync` (MaybeStats(None) -> undefined, matching node).
  let bigint = parse_bigint_option(scope, options);
  let throw_if_no_entry = parse_throw_if_no_entry(scope, options);
  let path = validate_path_to_string(scope, path, "path")?;
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Owned(PathBuf::from(&path)),
    OpenAccessKind::Read,
    Some("node:fs.statSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  match fs.stat_sync(&checked) {
    Ok(s) => Ok(MaybeStats(Some(Stats::build(s, bigint)))),
    Err(e)
      if !throw_if_no_entry && e.kind() == std::io::ErrorKind::NotFound =>
    {
      Ok(MaybeStats(None))
    }
    Err(e) => Err(node_fs_err(e, "stat", &path)),
  }
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_stat(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<MaybeStats, FsError>> + use<>, FsError>
{
  let bigint = parse_bigint_option(scope, options);
  let throw_if_no_entry = parse_throw_if_no_entry(scope, options);
  let path = validate_path_to_string(scope, path, "path")?;
  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      OpenAccessKind::Read,
      Some("node:fs.stat"),
    )?
    .into_owned();
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(async move {
    match fs.stat_async(checked).await {
      Ok(s) => Ok(MaybeStats(Some(Stats::build(s, bigint)))),
      Err(e)
        if !throw_if_no_entry && e.kind() == std::io::ErrorKind::NotFound =>
      {
        Ok(MaybeStats(None))
      }
      Err(e) => Err(node_fs_err(e, "stat", &path)),
    }
  })
}

#[op2(stack_trace)]
pub fn op_node_fs_lstat_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<MaybeStats, FsError> {
  // Extract bigint/throwIfNoEntry from the options object so this op is the
  // whole public `lstatSync`. Unlike async lstat (which ignores it), the SYNC
  // variant HONORS throwIfNoEntry -- matching node.
  let bigint = parse_bigint_option(scope, options);
  let throw_if_no_entry = parse_throw_if_no_entry(scope, options);
  let path = validate_path_to_string(scope, path, "path")?;
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Owned(PathBuf::from(&path)),
    OpenAccessKind::ReadNoFollow,
    Some("node:fs.lstatSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  match fs.lstat_sync(&checked) {
    Ok(s) => Ok(MaybeStats(Some(Stats::build(s, bigint)))),
    Err(e)
      if !throw_if_no_entry && e.kind() == std::io::ErrorKind::NotFound =>
    {
      Ok(MaybeStats(None))
    }
    Err(e) => Err(node_fs_err(e, "lstat", &path)),
  }
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_lstat(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<MaybeStats, FsError>> + use<>, FsError>
{
  let bigint = parse_bigint_option(scope, options);
  // Unlike `statSync`/`lstatSync` (and async `stat`), node's async `lstat`
  // ignores `throwIfNoEntry` and always errors on a missing path.
  let throw_if_no_entry = true;
  let path = validate_path_to_string(scope, path, "path")?;
  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      OpenAccessKind::ReadNoFollow,
      Some("node:fs.lstat"),
    )?
    .into_owned();
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(async move {
    match fs.lstat_async(checked).await {
      Ok(s) => Ok(MaybeStats(Some(Stats::build(s, bigint)))),
      Err(e)
        if !throw_if_no_entry && e.kind() == std::io::ErrorKind::NotFound =>
      {
        Ok(MaybeStats(None))
      }
      Err(e) => Err(node_fs_err(e, "lstat", &path)),
    }
  })
}

// Validates the fd (getValidatedFd) + extracts bigint from the options object
// and node-formats errors (syscall "fstat"), so this op is the whole public
// `fstatSync`.
#[op2]
pub fn op_node_fs_fstat_stats_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  fd: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<MaybeStats, FsError> {
  let fd = validate_fd_value(scope, fd)?;
  let bigint = parse_bigint_option(scope, options);
  let file = file_for_fd(state, fd).map_err(|_| ebadf_node("fstat"))?;
  match file.stat_sync() {
    Ok(s) => Ok(MaybeStats(Some(Stats::build(s, bigint)))),
    Err(e) => Err(map_fs_error_to_node_fs_error(
      e,
      NodeFsErrorContext {
        syscall: Some("fstat".to_string()),
        ..Default::default()
      },
    )),
  }
}

// `async(eager_throw)`: node's `fstat` validates the fd synchronously
// (getValidatedFd) but delivers EBADF for a valid-but-closed fd via the
// callback; bigint comes from the options object so the wrapper can be bound
// with `callbackifyOpt`.
#[op2(async(eager_throw))]
pub fn op_node_fs_fstat_stats(
  scope: &mut v8::PinScope<'_, '_>,
  state: Rc<RefCell<OpState>>,
  fd: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<MaybeStats, FsError>> + use<>, FsError>
{
  let fd = validate_fd_value(scope, fd)?;
  let bigint = parse_bigint_option(scope, options);
  Ok(async move {
    let file =
      file_for_fd(&state.borrow(), fd).map_err(|_| ebadf_node("fstat"))?;
    match file.stat_async().await {
      Ok(s) => Ok(MaybeStats(Some(Stats::build(s, bigint)))),
      Err(e) => Err(fd_syscall_err(e, "fstat")),
    }
  })
}

// node `fs.Dirent` as a cppgc object (replaces the JS `Dirent` +
// `DirentFromStats` classes). Unified: predicates come from explicit bools,
// set either from a uv dirent type (direntFromDeno) or copied from a `Stats`
// (the former DirentFromStats).
pub struct Dirent {
  // utf8 base values. `name`/`parentPath` are writable in node (glob relabels
  // name; `encoding:"buffer"` sets them to Buffers) — an override holds the
  // set v8 value (string or Buffer) when present.
  name: String,
  parent_path: String,
  name_override: RefCell<Option<v8::Global<v8::Value>>>,
  parent_path_override: RefCell<Option<v8::Global<v8::Value>>>,
  is_file: bool,
  is_directory: bool,
  is_symlink: bool,
  is_block_device: bool,
  is_char_device: bool,
  is_fifo: bool,
  is_socket: bool,
}

// SAFETY: Dirent holds only plain data plus untraced v8::Global values, safe to GC.
unsafe impl GarbageCollected for Dirent {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Dirent"
  }
}

#[op2]
impl Dirent {
  #[getter]
  fn name<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    match &*self.name_override.borrow() {
      Some(g) => v8::Local::new(scope, g),
      None => v8::String::new(scope, &self.name).unwrap().into(),
    }
  }
  #[setter]
  fn name(&self, scope: &mut v8::PinScope, value: v8::Local<v8::Value>) {
    *self.name_override.borrow_mut() = Some(v8::Global::new(scope, value));
  }
  #[getter]
  fn parent_path<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    match &*self.parent_path_override.borrow() {
      Some(g) => v8::Local::new(scope, g),
      None => v8::String::new(scope, &self.parent_path).unwrap().into(),
    }
  }
  #[setter]
  fn parent_path(&self, scope: &mut v8::PinScope, value: v8::Local<v8::Value>) {
    *self.parent_path_override.borrow_mut() =
      Some(v8::Global::new(scope, value));
  }
  #[fast]
  fn is_file(&self) -> bool {
    self.is_file
  }
  #[fast]
  fn is_directory(&self) -> bool {
    self.is_directory
  }
  #[fast]
  #[rename("isSymbolicLink")]
  fn is_symbolic_link(&self) -> bool {
    self.is_symlink
  }
  #[fast]
  fn is_block_device(&self) -> bool {
    self.is_block_device
  }
  #[fast]
  fn is_character_device(&self) -> bool {
    self.is_char_device
  }
  #[fast]
  #[rename("isFIFO")]
  fn is_fifo(&self) -> bool {
    self.is_fifo
  }
  #[fast]
  fn is_socket(&self) -> bool {
    self.is_socket
  }
}

// Build a Dirent from a uv dirent type:
// UNKNOWN=0 FILE=1 DIR=2 LINK=3 FIFO=4 SOCKET=5 CHAR=6 BLOCK=7.
// Splits a dirent name/parentPath argument: strings become the utf8 base
// value; anything else (a Buffer when encoding === "buffer") is kept verbatim
// in the override slot the getters prefer.
fn dirent_string_or_override(
  scope: &mut v8::PinScope<'_, '_>,
  v: v8::Local<v8::Value>,
) -> (String, RefCell<Option<v8::Global<v8::Value>>>) {
  if v.is_string() {
    (v.to_rust_string_lossy(scope), RefCell::new(None))
  } else {
    (String::new(), RefCell::new(Some(v8::Global::new(scope, v))))
  }
}

#[op2]
#[cppgc]
pub fn op_node_fs_dirent(
  scope: &mut v8::PinScope<'_, '_>,
  name: v8::Local<v8::Value>,
  parent_path: v8::Local<v8::Value>,
  kind: i32,
) -> Dirent {
  let (name, name_override) = dirent_string_or_override(scope, name);
  let (parent_path, parent_path_override) =
    dirent_string_or_override(scope, parent_path);
  Dirent {
    name,
    parent_path,
    name_override,
    parent_path_override,
    is_file: kind == 1,
    is_directory: kind == 2,
    is_symlink: kind == 3,
    is_fifo: kind == 4,
    is_socket: kind == 5,
    is_char_device: kind == 6,
    is_block_device: kind == 7,
  }
}

// Build a Dirent whose predicates come from a `Stats` (the former
// `DirentFromStats`, used by glob/opendir/getDirents).
#[op2]
#[cppgc]
pub fn op_node_fs_dirent_from_stats(
  scope: &mut v8::PinScope<'_, '_>,
  name: v8::Local<v8::Value>,
  parent_path: v8::Local<v8::Value>,
  #[cppgc] stats: &Stats,
) -> Dirent {
  let (name, name_override) = dirent_string_or_override(scope, name);
  let (parent_path, parent_path_override) =
    dirent_string_or_override(scope, parent_path);
  Dirent {
    name,
    parent_path,
    name_override,
    parent_path_override,
    is_file: stats.is_file,
    is_directory: stats.is_directory,
    is_symlink: stats.is_symlink,
    is_block_device: stats.is_block_device,
    is_char_device: stats.is_char_device,
    is_fifo: stats.is_fifo,
    is_socket: stats.is_socket,
  }
}

// A directory entry collected during a native readdir walk.
struct ReaddirItem {
  name: String,
  parent: String,
  ent: deno_fs::FsDirEntry,
}

// Collect one directory's entries, queueing subdirs when recursive. For
// recursive walks `name` is the path relative to the root (node semantics).
// Entries are sorted per directory: libuv's uv_fs_scandir runs alphasort, so
// node's readdir output is alphabetical (test-fs-readdir-types relies on it).
fn readdir_collect(
  root: &Path,
  dir: &Path,
  recursive: bool,
  mut entries: Vec<deno_fs::FsDirEntry>,
  queue: &mut std::collections::VecDeque<PathBuf>,
  items: &mut Vec<ReaddirItem>,
) {
  entries.sort_by(|a, b| a.name.cmp(&b.name));
  for ent in entries {
    if recursive && ent.is_directory {
      queue.push_back(dir.join(&ent.name));
    }
    let name = if recursive {
      let full = dir.join(&ent.name);
      full
        .strip_prefix(root)
        .unwrap_or(&full)
        .to_string_lossy()
        .into_owned()
    } else {
      ent.name.clone()
    };
    items.push(ReaddirItem {
      name,
      parent: dir.to_string_lossy().into_owned(),
      ent,
    });
  }
}

// Build a v8 array of cppgc `Dirent`s (withFileTypes) or utf8 name strings.
fn readdir_to_v8<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  with_file_types: bool,
  items: Vec<ReaddirItem>,
) -> v8::Local<'a, v8::Array> {
  let arr = v8::Array::new(scope, items.len() as i32);
  for (i, it) in items.into_iter().enumerate() {
    let val: v8::Local<v8::Value> = if with_file_types {
      // node's Dirent always carries the basename in `name` and the absolute
      // containing directory in `parentPath` (even for recursive walks, where
      // the string form instead returns the root-relative path).
      let d = Dirent {
        name: it.ent.name,
        parent_path: it.parent,
        name_override: RefCell::new(None),
        parent_path_override: RefCell::new(None),
        is_file: it.ent.is_file,
        is_directory: it.ent.is_directory,
        is_symlink: it.ent.is_symlink,
        is_block_device: false,
        is_char_device: false,
        is_fifo: false,
        is_socket: false,
      };
      deno_core::cppgc::make_cppgc_object(scope, d).into()
    } else {
      v8::String::new(scope, &it.name).unwrap().into()
    };
    arr.set_index(scope, i as u32, val);
  }
  arr
}

fn readdir_scandir_err(e: deno_io::fs::FsError, dir: &Path) -> FsError {
  map_fs_error_to_node_fs_error(
    e,
    NodeFsErrorContext {
      syscall: Some("scandir".to_string()),
      path: Some(dir.to_string_lossy().into_owned()),
      ..Default::default()
    },
  )
}

// Native `readdirSync`: walks the tree in Rust (recursive optional). Names are
// utf8; the JS shim handles the rare buffer/other encodings.
#[op2(stack_trace)]
pub fn op_node_fs_readdir_sync<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  state: &mut OpState,
  #[string] path: String,
  recursive: bool,
  with_file_types: bool,
) -> Result<v8::Local<'s, v8::Array>, FsError> {
  let root = PathBuf::from(&path);
  let fs = state.borrow::<FileSystemRc>().clone();
  let mut items: Vec<ReaddirItem> = Vec::new();
  let mut queue: std::collections::VecDeque<PathBuf> =
    std::collections::VecDeque::new();
  queue.push_back(root.clone());
  while let Some(dir) = queue.pop_front() {
    let checked = state.borrow_mut::<PermissionsContainer>().check_open(
      Cow::Owned(dir.clone()),
      OpenAccessKind::Read,
      Some("node:fs.readdirSync"),
    )?;
    let entries = fs
      .read_dir_sync(&checked)
      .map_err(|e| readdir_scandir_err(e, &dir))?;
    readdir_collect(&root, &dir, recursive, entries, &mut queue, &mut items);
  }
  Ok(readdir_to_v8(scope, with_file_types, items))
}

// Validates `opendir`'s `options.bufferSize` and probes the directory (matching
// node's eager `opendir` open) so an invalid/non-dir path produces the final
// node error. The `path` is already validated in JS (its type errors must throw
// synchronously, while bufferSize/probe errors are surfaced via the callback,
// so this op runs inside the JS try). Both `fs.opendir` and `fs.opendirSync`
// validate synchronously, so one op serves both. The node error carries
// `syscall: "opendir"` (no path, matching the prior `denoErrorToNodeError`).
#[op2(fast, stack_trace)]
pub fn op_node_fs_opendir_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  #[string] path: &str,
  buffer_size: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  validate_integer(scope, buffer_size, "options.bufferSize", 1, 4294967295)?;
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(path)),
    OpenAccessKind::Read,
    Some("node:fs.opendir"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  fs.read_dir_sync(&checked).map_err(|e| {
    map_fs_error_to_node_fs_error(
      e,
      NodeFsErrorContext {
        syscall: Some("opendir".into()),
        ..Default::default()
      },
    )
  })?;
  Ok(())
}

// ToV8 wrapper so the async op can build the result array once it resolves
// (async ops have no scope at await time).
pub struct ReaddirOutput {
  with_file_types: bool,
  items: Vec<ReaddirItem>,
}

impl<'a> ToV8<'a> for ReaddirOutput {
  type Error = std::convert::Infallible;
  fn to_v8(
    self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    Ok(readdir_to_v8(scope, self.with_file_types, self.items).into())
  }
}

// Native async `readdir`: the same recursive walk via the async filesystem.
#[op2(stack_trace)]
pub async fn op_node_fs_readdir(
  state: Rc<RefCell<OpState>>,
  #[string] path: String,
  recursive: bool,
  with_file_types: bool,
) -> Result<ReaddirOutput, FsError> {
  let root = PathBuf::from(&path);
  let mut items: Vec<ReaddirItem> = Vec::new();
  let mut queue: std::collections::VecDeque<PathBuf> =
    std::collections::VecDeque::new();
  queue.push_back(root.clone());
  while let Some(dir) = queue.pop_front() {
    let (fs, checked) = {
      let mut state = state.borrow_mut();
      let checked = state.borrow_mut::<PermissionsContainer>().check_open(
        Cow::Owned(dir.clone()),
        OpenAccessKind::Read,
        Some("node:fs.readdir"),
      )?;
      (state.borrow::<FileSystemRc>().clone(), checked.into_owned())
    };
    let reader = fs
      .read_dir_async(checked)
      .await
      .map_err(|e| readdir_scandir_err(e, &dir))?;
    let mut entries = Vec::new();
    while let Some(ent) = reader
      .next()
      .await
      .map_err(|e| readdir_scandir_err(e, &dir))?
    {
      entries.push(ent);
    }
    readdir_collect(&root, &dir, recursive, entries, &mut queue, &mut items);
  }
  Ok(ReaddirOutput {
    with_file_types,
    items,
  })
}

// Map a filesystem error to a fully-formed node error (code/errno/message +
// syscall + path). No JS round-trip is needed to finalize it.
fn node_fs_err(e: deno_io::fs::FsError, syscall: &str, path: &str) -> FsError {
  map_fs_error_to_node_fs_error(
    e,
    NodeFsErrorContext {
      syscall: Some(syscall.to_string()),
      path: Some(path.to_string()),
      ..Default::default()
    },
  )
}

// Variant for two-path syscalls (link/symlink/copyfile/rename); node attaches
// both `path` and `dest`.
fn node_fs_err_dest(
  e: deno_io::fs::FsError,
  syscall: &str,
  path: &str,
  dest: &str,
) -> FsError {
  map_fs_error_to_node_fs_error(
    e,
    NodeFsErrorContext {
      syscall: Some(syscall.to_string()),
      path: Some(path.to_string()),
      dest: Some(dest.to_string()),
      ..Default::default()
    },
  )
}

// --- native metadata ops (no Deno.* wrapper): mkdir / remove / rename ---

// node's `validateBoolean(value, name)`: requires a JS boolean, else
// ERR_INVALID_ARG_TYPE.
fn validate_boolean(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
  name: &str,
) -> Result<bool, FsError> {
  if !value.is_boolean() {
    return Err(err_invalid_arg_type(scope, name, &["boolean"], value).into());
  }
  Ok(value.boolean_value(scope))
}

// Parses node's `mkdir` options argument (number/string mode, boolean
// recursive, or `{ recursive, mode }`) into `(recursive, mode)`, throwing the
// same `ERR_INVALID_ARG_*` errors node would.
fn parse_mkdir_options(
  scope: &mut v8::PinScope<'_, '_>,
  options: v8::Local<v8::Value>,
) -> Result<(bool, u32), FsError> {
  let mut mode = 0o777u32;
  let mut recursive_value: Option<v8::Local<v8::Value>> = None;

  if options.is_number() || options.is_string() {
    // A number or string second arg is a file mode (see lib/fs.js mkdir).
    mode = parse_file_mode(scope, options, "mode", Some(0o777))?;
  } else if options.is_boolean() {
    recursive_value = Some(options);
  } else if !options.is_null_or_undefined()
    && let Ok(obj) = v8::Local::<v8::Object>::try_from(options)
  {
    let rec_key = intern_key(scope, "recursive");
    if let Some(rec) = obj.get(scope, rec_key.into())
      && !rec.is_undefined()
    {
      recursive_value = Some(rec);
    }
    let mode_key = intern_key(scope, "mode");
    if let Some(mode_v) = obj.get(scope, mode_key.into())
      && !mode_v.is_undefined()
    {
      mode = parse_file_mode(scope, mode_v, "options.mode", Some(0o777))?;
    }
  }

  let recursive = match recursive_value {
    Some(v) => validate_boolean(scope, v, "options.recursive")?,
    None => false,
  };
  Ok((recursive, mode))
}

// node's `path.resolve(path)`: make absolute against the cwd and normalize
// `.`/`..` segments.
fn resolve_abs(fs: &FileSystemRc, path: &str) -> PathBuf {
  let p = Path::new(path);
  let abs = if p.is_absolute() {
    Cow::Borrowed(p)
  } else {
    Cow::Owned(fs.cwd().unwrap_or_else(|_| PathBuf::from("/")).join(p))
  };
  deno_path_util::normalize_path(abs).into_owned()
}

// node's `path.toNamespacedPath`: identity on POSIX; `\\?\`-prefixed on Windows.
#[cfg(not(windows))]
fn to_namespaced_path(path: &Path) -> String {
  path.to_string_lossy().into_owned()
}

#[cfg(windows)]
fn to_namespaced_path(path: &Path) -> String {
  let s = path.to_string_lossy();
  if s.starts_with("\\\\?\\") {
    s.into_owned()
  } else if let Some(rest) = s.strip_prefix("\\\\") {
    format!("\\\\?\\UNC\\{rest}")
  } else {
    format!("\\\\?\\{s}")
  }
}

// Replicates `findFirstNonExistent`: returns the topmost path component of a
// recursive `mkdir` that does not yet exist (node's `mkdirp` return value), or
// `None` when the full path already exists. Existence is probed with a
// permission-checked stat, matching the prior `Deno.statSync` walk (failures
// — including permission denials — are treated as "does not exist").
fn find_first_non_existent(
  state: &mut OpState,
  resolved: PathBuf,
) -> Option<String> {
  fn checked_exists(state: &mut OpState, path: &Path) -> bool {
    let checked = match state.borrow_mut::<PermissionsContainer>().check_open(
      Cow::Borrowed(path),
      OpenAccessKind::Read,
      Some("node:fs.mkdir"),
    ) {
      Ok(c) => c,
      Err(_) => return false,
    };
    let fs = state.borrow::<FileSystemRc>();
    fs.stat_sync(&checked).is_ok()
  }

  let mut cursor = resolved;
  loop {
    if checked_exists(state, &cursor) {
      return None;
    }
    let parent = match cursor.parent() {
      Some(parent) => parent.to_path_buf(),
      // Reached the filesystem root with nothing existing.
      None => return Some(to_namespaced_path(&cursor)),
    };
    if checked_exists(state, &parent) {
      return Some(to_namespaced_path(&cursor));
    }
    cursor = parent;
  }
}

// On Windows, recursive `mkdir` through a file yields EEXIST instead of
// ENOTDIR; rewrite the error to ENOTDIR if any ancestor is not a directory.
#[cfg(windows)]
fn fix_mkdir_error(fs: &FileSystemRc, err: FsError, path: &str) -> FsError {
  let FsError::NodeFs(ref nfe) = err else {
    return err;
  };
  if nfe.code != "EEXIST" {
    return err;
  }
  // The target itself already existing as a non-directory is EEXIST (node's
  // recursive mkdir reports the original error), NOT ENOTDIR. ENOTDIR only
  // applies when a non-directory *ancestor* component blocks traversal, so
  // start the walk at the parent and skip the target path.
  let target = resolve_abs(fs, path);
  let mut cursor = match target.parent() {
    Some(parent) if parent != target => parent.to_path_buf(),
    _ => return err,
  };
  loop {
    let checked = CheckedPath::unsafe_new(Cow::Borrowed(cursor.as_path()));
    match fs.stat_sync(&checked) {
      Ok(stat) => {
        if !stat.is_directory {
          return NodeFsError::from_code(
            "ENOTDIR",
            NodeFsErrorContext {
              syscall: Some("mkdir".into()),
              path: Some(path.to_string()),
              ..Default::default()
            },
          )
          .into();
        }
        return err;
      }
      Err(_) => match cursor.parent() {
        Some(parent) if parent != cursor => cursor = parent.to_path_buf(),
        _ => return err,
      },
    }
  }
}

#[op2(stack_trace)]
pub fn op_node_fs_mkdir_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<MaybeString, FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let (recursive, mode) = parse_mkdir_options(scope, options)?;
  let mode = mode & 0o777;

  let first_non_existent = if recursive {
    let resolved = {
      let fs = state.borrow::<FileSystemRc>();
      resolve_abs(fs, &path)
    };
    find_first_non_existent(state, resolved)
  } else {
    None
  };

  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(&path)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.mkdirSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>().clone();
  match fs.mkdir_sync(&checked, recursive, Some(mode)) {
    Ok(()) => Ok(MaybeString(first_non_existent)),
    Err(e) => {
      let err = node_fs_err(e, "mkdir", &path);
      #[cfg(windows)]
      if recursive {
        return Err(fix_mkdir_error(&fs, err, &path));
      }
      Err(err)
    }
  }
}

#[op2(async(eager_throw), stack_trace)]
#[string]
pub fn op_node_fs_mkdir(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<
  impl Future<Output = Result<Option<String>, FsError>> + use<>,
  FsError,
> {
  let path = validate_path_to_string(scope, path, "path")?;
  let (recursive, mode) = parse_mkdir_options(scope, options)?;
  let mode = mode & 0o777;

  let first_non_existent = if recursive {
    let resolved = {
      let fs = state.borrow::<FileSystemRc>();
      resolve_abs(fs, &path)
    };
    find_first_non_existent(state, resolved)
  } else {
    None
  };

  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.mkdir"),
    )?
    .into_owned();
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(async move {
    match fs.mkdir_async(checked, recursive, Some(mode)).await {
      Ok(()) => Ok(first_non_existent),
      Err(e) => {
        let err = node_fs_err(e, "mkdir", &path);
        #[cfg(windows)]
        if recursive {
          return Err(fix_mkdir_error(&fs, err, &path));
        }
        Err(err)
      }
    }
  })
}

// Plain non-recursive/recursive remove used by `unlink`/`unlinkSync` (and as
// the removal step). No option validation or lstat precheck — those are
// `rm`-specific and live in `op_node_fs_rm`. Errors are reported with the
// caller's `syscall` ("unlink"), matching node.
#[op2(fast, stack_trace)]
#[undefined]
pub fn op_node_fs_remove_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Owned(PathBuf::from(&path)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.unlinkSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  fs.remove_sync(&checked, false)
    .map_err(|e| node_fs_err(e, "unlink", &path))?;
  Ok(())
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_remove(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.unlink"),
    )?
    .into_owned();
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(async move {
    fs.remove_async(checked, false)
      .await
      .map_err(|e| node_fs_err(e, "unlink", &path))?;
    Ok(())
  })
}

/// node's `ERR_FS_EISDIR` (thrown by `fs.rm`/`rmSync` on a non-recursive
/// directory). Matches node's class: code `ERR_FS_EISDIR`, the templated
/// message, `syscall: "rm"`, numeric `errno`.
fn eisdir_rm(path: &str) -> FsError {
  NodeFsError {
    code: "ERR_FS_EISDIR",
    errno: libc::EISDIR,
    message: format!(
      "Path is a directory: rm returned EISDIR (is a directory) {path}"
    ),
    syscall: Some("rm".to_string()),
    path: Some(path.to_string()),
    dest: None,
  }
  .into()
}

// node's `validateObject(value, name)` with the default flags
// (kValidateObjectNone): rejects null, arrays, and non-objects (incl.
// functions), all as ERR_INVALID_ARG_TYPE 'Object'.
fn validate_object(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
  name: &str,
) -> Result<(), FsError> {
  if value.is_null()
    || value.is_array()
    || value.is_function()
    || !value.is_object()
  {
    return Err(err_invalid_arg_type(scope, name, &["Object"], value).into());
  }
  Ok(())
}

// Reads an OWN property (matching node's `{...defaults, ...options}` spread,
// which only copies own enumerable props), returning None when absent so the
// caller can apply the default.
fn own_prop<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<v8::Object>,
  key: &str,
) -> Option<v8::Local<'s, v8::Value>> {
  let k = intern_key(scope, key);
  if obj.has_own_property(scope, k.into()).unwrap_or(false) {
    obj.get(scope, k.into())
  } else {
    None
  }
}

// node's `validateRmOptions` field validation over the options merged with
// `defaultRmOptions` (force:false, recursive:false, retryDelay:100,
// maxRetries:0). Validation order matches node (force, recursive, retryDelay,
// maxRetries); retryDelay/maxRetries are validated then discarded (deno does
// not retry). Returns (recursive, force).
fn validate_rm_options(
  scope: &mut v8::PinScope<'_, '_>,
  options: v8::Local<v8::Value>,
) -> Result<(bool, bool), FsError> {
  // node: `if (options === undefined) return defaults;` — every default is
  // valid, so undefined options skip per-field validation.
  if options.is_undefined() {
    return Ok((false, false));
  }
  validate_object(scope, options, "options")?;
  let obj = v8::Local::<v8::Object>::try_from(options).unwrap();
  let force = match own_prop(scope, obj, "force") {
    Some(v) => validate_boolean(scope, v, "options.force")?,
    None => false,
  };
  let recursive = match own_prop(scope, obj, "recursive") {
    Some(v) => validate_boolean(scope, v, "options.recursive")?,
    None => false,
  };
  if let Some(v) = own_prop(scope, obj, "retryDelay") {
    validate_integer(scope, v, "options.retryDelay", 0, 2147483647)?;
  }
  if let Some(v) = own_prop(scope, obj, "maxRetries") {
    validate_integer(scope, v, "options.maxRetries", 0, 4294967295)?;
  }
  Ok((recursive, force))
}

// `fs.rm`/`rmSync`: node's `validateRmOptions` (option validation + lstat
// precheck: a missing path is OK under `force`, else ENOENT with syscall
// "lstat"; a non-recursive directory is `ERR_FS_EISDIR`) followed by the
// recursive/non-recursive removal.
#[op2(fast, stack_trace)]
#[undefined]
pub fn op_node_fs_rm_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let (recursive, force) = validate_rm_options(scope, options)?;
  let path = validate_path_to_string(scope, path, "path")?;
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Owned(PathBuf::from(&path)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.rmSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  match fs.lstat_sync(&checked) {
    Ok(stat) => {
      if stat.is_directory && !recursive {
        return Err(eisdir_rm(&path));
      }
    }
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
      if force {
        return Ok(());
      }
      return Err(node_fs_err(e, "lstat", &path));
    }
    Err(e) => return Err(node_fs_err(e, "lstat", &path)),
  }
  fs.remove_sync(&checked, recursive).or_else(|e| {
    // `force` also swallows an ENOENT that races between the lstat and remove.
    if force
      && matches!(&e, deno_io::fs::FsError::Io(io)
        if io.kind() == std::io::ErrorKind::NotFound)
    {
      Ok(())
    } else {
      Err(node_fs_err(e, "rm", &path))
    }
  })?;
  Ok(())
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_rm(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let (recursive, force) = validate_rm_options(scope, options)?;
  let path = validate_path_to_string(scope, path, "path")?;
  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.rm"),
    )?
    .into_owned();
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(async move {
    match fs.lstat_async(checked.clone()).await {
      Ok(stat) => {
        if stat.is_directory && !recursive {
          return Err(eisdir_rm(&path));
        }
      }
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
        if force {
          return Ok(());
        }
        return Err(node_fs_err(e, "lstat", &path));
      }
      Err(e) => return Err(node_fs_err(e, "lstat", &path)),
    }
    fs.remove_async(checked, recursive).await.or_else(|e| {
      if force
        && matches!(&e, deno_io::fs::FsError::Io(io)
          if io.kind() == std::io::ErrorKind::NotFound)
      {
        Ok(())
      } else {
        Err(node_fs_err(e, "rm", &path))
      }
    })?;
    Ok(())
  })
}

#[op2(fast, stack_trace)]
pub fn op_node_fs_rename_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  oldpath: v8::Local<v8::Value>,
  newpath: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let oldpath = validate_path_to_string(scope, oldpath, "oldPath")?;
  let newpath = validate_path_to_string(scope, newpath, "newPath")?;
  let old = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(&oldpath)),
    OpenAccessKind::ReadWriteNoFollow,
    Some("node:fs.renameSync"),
  )?;
  let new = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(&newpath)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.renameSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  fs.rename_sync(&old, &new)
    .map_err(|e| node_fs_err_dest(e, "rename", &oldpath, &newpath))?;
  Ok(())
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_rename(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  oldpath: v8::Local<v8::Value>,
  newpath: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let oldpath = validate_path_to_string(scope, oldpath, "oldPath")?;
  let newpath = validate_path_to_string(scope, newpath, "newPath")?;
  let (fs, old, new) = {
    let old = state.borrow_mut::<PermissionsContainer>().check_open(
      Cow::Owned(PathBuf::from(&oldpath)),
      OpenAccessKind::ReadWriteNoFollow,
      Some("node:fs.rename"),
    )?;
    let new = state.borrow_mut::<PermissionsContainer>().check_open(
      Cow::Owned(PathBuf::from(&newpath)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.rename"),
    )?;
    (
      state.borrow::<FileSystemRc>().clone(),
      old.into_owned(),
      new.into_owned(),
    )
  };
  Ok(async move {
    fs.rename_async(old, new)
      .await
      .map_err(|e| node_fs_err_dest(e, "rename", &oldpath, &newpath))?;
    Ok(())
  })
}

// --- realpath / readlink (return the resolved path string) ---

// `fs.realpathSync(path, options)` end to end: validates the path, parses the
// encoding options (default utf8), resolves, and returns the result already
// encoded (string or Buffer).
#[op2(stack_trace)]
pub fn op_node_fs_realpath_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
  // `fs.realpath` reports syscall "lstat"; `fs.realpath.native` reports
  // "realpath" (matching node's lib/fs.js).
  #[string] syscall: &str,
) -> Result<MaybeEncodedBytes, FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let enc = parse_encoding_options(scope, options, Some(BufEnc::Utf8))?;
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Owned(PathBuf::from(&path)),
    OpenAccessKind::Read,
    Some("node:fs.realpathSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  let resolved = fs
    .realpath_sync(&checked)
    .map_err(|e| node_fs_err(e, syscall, &path))?;
  MaybeEncodedBytes::new(
    state,
    resolved.to_string_lossy().into_owned().into_bytes(),
    enc,
  )
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_realpath(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
  #[string] syscall: String,
) -> Result<
  impl Future<Output = Result<MaybeEncodedBytes, FsError>> + use<>,
  FsError,
> {
  let path = validate_path_to_string(scope, path, "path")?;
  let enc = parse_encoding_options(scope, options, Some(BufEnc::Utf8))?;
  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      OpenAccessKind::Read,
      Some("node:fs.realpath"),
    )?
    .into_owned();
  let proto = if enc.is_none() {
    buffer_proto(state)
  } else {
    None
  };
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(async move {
    let resolved = fs
      .realpath_async(checked)
      .await
      .map_err(|e| node_fs_err(e, &syscall, &path))?;
    MaybeEncodedBytes::with_proto(
      resolved.to_string_lossy().into_owned().into_bytes(),
      enc,
      proto,
    )
  })
}

// `fs.readlinkSync(path, options)` end to end (default encoding utf8).
#[op2(stack_trace)]
pub fn op_node_fs_read_link_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<MaybeEncodedBytes, FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let enc = parse_encoding_options(scope, options, Some(BufEnc::Utf8))?;
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Owned(PathBuf::from(&path)),
    OpenAccessKind::ReadNoFollow,
    Some("node:fs.readlinkSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  let target = fs
    .read_link_sync(&checked)
    .map_err(|e| node_fs_err(e, "readlink", &path))?;
  MaybeEncodedBytes::new(
    state,
    target.to_string_lossy().into_owned().into_bytes(),
    enc,
  )
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_read_link(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<
  impl Future<Output = Result<MaybeEncodedBytes, FsError>> + use<>,
  FsError,
> {
  let path = validate_path_to_string(scope, path, "path")?;
  let enc = parse_encoding_options(scope, options, Some(BufEnc::Utf8))?;
  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      OpenAccessKind::ReadNoFollow,
      Some("node:fs.readlink"),
    )?
    .into_owned();
  let proto = if enc.is_none() {
    buffer_proto(state)
  } else {
    None
  };
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(async move {
    let target = fs
      .read_link_async(checked)
      .await
      .map_err(|e| node_fs_err(e, "readlink", &path))?;
    MaybeEncodedBytes::with_proto(
      target.to_string_lossy().into_owned().into_bytes(),
      enc,
      proto,
    )
  })
}

// --- access ---

// Replicates node's `fs.access` permission check against an `lstat` result.
// On unix the owner bits are used when the caller owns the file; on windows
// (where the FileSystem reports a synthetic mode) existence alone suffices,
// matching the prior JS behavior.
fn access_ok(stat: &deno_io::fs::FsStat, mode: u32) -> bool {
  #[cfg(windows)]
  {
    let _ = stat;
    let _ = mode;
    true
  }
  #[cfg(unix)]
  {
    // SAFETY: getuid is always safe to call.
    let uid = unsafe { libc::getuid() };
    let mut file_mode = stat.mode;
    if stat.uid == uid {
      file_mode >>= 6;
    }
    (mode & file_mode) == mode
  }
}

fn access_eacces(path: &str) -> FsError {
  NodeFsError::from_code(
    "EACCES",
    NodeFsErrorContext {
      syscall: Some("access".to_string()),
      path: Some(path.to_string()),
      ..Default::default()
    },
  )
  .into()
}

#[op2(fast, stack_trace)]
#[undefined]
pub fn op_node_fs_access_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  mode: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let mode = validate_access_mode(scope, mode)?;
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(&path)),
    OpenAccessKind::ReadNoFollow,
    Some("node:fs.accessSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  let stat = fs
    .lstat_sync(&checked)
    .map_err(|e| node_fs_err(e, "access", &path))?;
  if !access_ok(&stat, mode) {
    return Err(access_eacces(&path));
  }
  Ok(())
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_access(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  mode: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let mode = validate_access_mode(scope, mode)?;
  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      OpenAccessKind::ReadNoFollow,
      Some("node:fs.access"),
    )?
    .into_owned();
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(async move {
    let stat = fs
      .lstat_async(checked)
      .await
      .map_err(|e| node_fs_err(e, "access", &path))?;
    if !access_ok(&stat, mode) {
      return Err(access_eacces(&path));
    }
    Ok(())
  })
}

// --- chmod / chown ---

// Fully validates its arguments in Rust (path + mode) and throws the exact
// node `ERR_INVALID_ARG_*`, so `fs.chmodSync` is a thin alias of this op.
#[op2(fast, stack_trace)]
#[undefined]
pub fn op_node_fs_chmod_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  mode: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let mode = parse_file_mode(scope, mode, "mode", None)?;
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Owned(PathBuf::from(&path)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.chmodSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  fs.chmod_sync(&checked, mode as _)
    .map_err(|e| node_fs_err(e, "chmod", &path))?;
  Ok(())
}

// `async(eager_throw)`: path/mode are validated synchronously (with `scope`)
// and the resulting `ERR_INVALID_ARG_*` throws synchronously like node; only
// the async chmod itself rejects. So `fs.chmod` is a thin passthrough.
#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_chmod(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  mode: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let mode = parse_file_mode(scope, mode, "mode", None)?;
  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.chmod"),
    )?
    .into_owned();
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(async move {
    fs.chmod_async(checked, mode as _)
      .await
      .map_err(|e| node_fs_err(e, "chmod", &path))?;
    Ok(())
  })
}

#[op2(fast, stack_trace)]
#[undefined]
pub fn op_node_fs_chown_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  uid: v8::Local<v8::Value>,
  gid: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  // uid/gid: integers in [-1, 2**32-1]; -1 (0xFFFFFFFF) means "unchanged".
  let uid = validate_integer(scope, uid, "uid", -1, K_MAX_USER_ID)? as u32;
  let gid = validate_integer(scope, gid, "gid", -1, K_MAX_USER_ID)? as u32;
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Owned(PathBuf::from(&path)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.chownSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  fs.chown_sync(&checked, Some(uid), Some(gid))
    .map_err(|e| node_fs_err(e, "chown", &path))?;
  Ok(())
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_chown(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  uid: v8::Local<v8::Value>,
  gid: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let uid = validate_integer(scope, uid, "uid", -1, K_MAX_USER_ID)? as u32;
  let gid = validate_integer(scope, gid, "gid", -1, K_MAX_USER_ID)? as u32;
  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.chown"),
    )?
    .into_owned();
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(async move {
    fs.chown_async(checked, Some(uid), Some(gid))
      .await
      .map_err(|e| node_fs_err(e, "chown", &path))?;
    Ok(())
  })
}

// --- link / symlink ---

#[op2(fast, stack_trace)]
pub fn op_node_fs_link_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  oldpath: v8::Local<v8::Value>,
  newpath: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let oldpath = validate_path_to_string(scope, oldpath, "path")?;
  let newpath = validate_path_to_string(scope, newpath, "path")?;
  let old = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Owned(PathBuf::from(&oldpath)),
    OpenAccessKind::ReadWriteNoFollow,
    Some("node:fs.linkSync"),
  )?;
  let new = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Owned(PathBuf::from(&newpath)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.linkSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  fs.link_sync(&old, &new)
    .map_err(|e| node_fs_err_dest(e, "link", &oldpath, &newpath))?;
  Ok(())
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_link(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  oldpath: v8::Local<v8::Value>,
  newpath: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let oldpath = validate_path_to_string(scope, oldpath, "path")?;
  let newpath = validate_path_to_string(scope, newpath, "path")?;
  let old = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&oldpath)),
      OpenAccessKind::ReadWriteNoFollow,
      Some("node:fs.link"),
    )?
    .into_owned();
  let new = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&newpath)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.link"),
    )?
    .into_owned();
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(async move {
    fs.link_async(old, new)
      .await
      .map_err(|e| node_fs_err_dest(e, "link", &oldpath, &newpath))?;
    Ok(())
  })
}

fn symlink_file_type(kind: &str) -> Option<FsFileType> {
  match kind {
    "dir" => Some(FsFileType::Directory),
    "junction" => Some(FsFileType::Junction),
    _ => Some(FsFileType::File),
  }
}

// Replicates `validateOneOf(type, "type", ["dir","file","junction",null,
// undefined])`: `Ok(None)` = absent (platform default / Windows
// auto-detection), `Ok(Some(kind))` = explicit.
fn validate_symlink_type(
  scope: &mut v8::PinScope<'_, '_>,
  v: v8::Local<v8::Value>,
) -> Result<Option<String>, FsError> {
  if v.is_null_or_undefined() {
    return Ok(None);
  }
  if v.is_string() {
    let s = v.to_rust_string_lossy(scope);
    if matches!(s.as_str(), "dir" | "file" | "junction") {
      return Ok(Some(s));
    }
  }
  Err(
    err_invalid_arg_value_received(
      "type",
      "must be one of: 'dir', 'file', 'junction', null, undefined",
      &inspect_encoding(scope, v),
    )
    .into(),
  )
}

// On Windows with no explicit type, node auto-detects: resolve the target
// relative to the symlink's parent and use "dir" if it stats as a directory
// (stat errors fall back to "file" so error behavior matches other
// platforms). On unix the type is ignored entirely.
#[cfg(windows)]
fn resolve_symlink_kind(
  fs: &FileSystemRc,
  target: &str,
  path: &str,
) -> Option<FsFileType> {
  let base = std::path::absolute(Path::new(path)).ok()?;
  let resolved = base.parent()?.join(target);
  let checked = CheckedPath::unsafe_new(Cow::Borrowed(resolved.as_path()));
  match fs.stat_sync(&checked) {
    Ok(stat) if stat.is_directory => Some(FsFileType::Directory),
    _ => Some(FsFileType::File),
  }
}

#[cfg(not(windows))]
fn resolve_symlink_kind(
  _fs: &FileSystemRc,
  _target: &str,
  _path: &str,
) -> Option<FsFileType> {
  Some(FsFileType::File)
}

#[op2(fast, stack_trace)]
pub fn op_node_fs_symlink_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  target: v8::Local<v8::Value>,
  path: v8::Local<v8::Value>,
  kind: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let kind = validate_symlink_type(scope, kind)?;
  let target = validate_path_to_string(scope, target, "target")?;
  let path = validate_path_to_string(scope, path, "path")?;
  let file_type = match kind.as_deref() {
    Some(k) => symlink_file_type(k),
    None => {
      resolve_symlink_kind(state.borrow::<FileSystemRc>(), &target, &path)
    }
  };
  // PERMISSIONS: a symlink's target is only resolved on traversal, so we
  // verify unscoped read+write (matching Deno.symlink semantics).
  {
    let perms = state.borrow::<PermissionsContainer>();
    perms.check_write_all("node:fs.symlinkSync")?;
    perms.check_read_all("node:fs.symlinkSync")?;
  }
  let target_p = CheckedPath::unsafe_new(Cow::Borrowed(Path::new(&target)));
  let path_p = CheckedPath::unsafe_new(Cow::Borrowed(Path::new(&path)));
  let fs = state.borrow::<FileSystemRc>();
  fs.symlink_sync(&target_p, &path_p, file_type)
    .map_err(|e| node_fs_err_dest(e, "symlink", &target, &path))?;
  Ok(())
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_symlink(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  target: v8::Local<v8::Value>,
  path: v8::Local<v8::Value>,
  kind: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let kind = validate_symlink_type(scope, kind)?;
  let target = validate_path_to_string(scope, target, "target")?;
  let path = validate_path_to_string(scope, path, "path")?;
  let fs = {
    let perms = state.borrow::<PermissionsContainer>();
    perms.check_write_all("node:fs.symlink")?;
    perms.check_read_all("node:fs.symlink")?;
    state.borrow::<FileSystemRc>().clone()
  };
  let file_type = match kind.as_deref() {
    Some(k) => symlink_file_type(k),
    None => resolve_symlink_kind(&fs, &target, &path),
  };
  Ok(async move {
    let target_p = CheckedPathBuf::unsafe_new(PathBuf::from(&target));
    let path_p = CheckedPathBuf::unsafe_new(PathBuf::from(&path));
    fs.symlink_async(target_p, path_p, file_type)
      .await
      .map_err(|e| node_fs_err_dest(e, "symlink", &target, &path))?;
    Ok(())
  })
}

// --- copy_file ---

const COPYFILE_EXCL: i64 = 1;
// COPYFILE_EXCL | COPYFILE_FICLONE | COPYFILE_FICLONE_FORCE
const COPYFILE_MODE_MAX: i64 = 7;

// node's `getValidMode(mode, "copyFile")` (done in its C++ binding):
// null/undefined -> 0, otherwise an integer within [0, 7].
fn validate_copy_mode(
  scope: &mut v8::PinScope<'_, '_>,
  mode: v8::Local<v8::Value>,
) -> Result<i64, FsError> {
  if mode.is_null_or_undefined() {
    return Ok(0);
  }
  validate_integer(scope, mode, "mode", 0, COPYFILE_MODE_MAX)
}

fn copyfile_eexist(src: &str, dest: &str) -> FsError {
  NodeFsError::from_code(
    "EEXIST",
    NodeFsErrorContext {
      syscall: Some("copyfile".to_string()),
      path: Some(src.to_string()),
      dest: Some(dest.to_string()),
      ..Default::default()
    },
  )
  .into()
}

// `COPYFILE_EXCL` fails with node's EEXIST error when the destination exists;
// libuv opens the dest O_EXCL, so an existing (even dangling) symlink counts
// -- hence the lstat probe. libuv opens the SOURCE first though, so a missing
// source must surface ENOENT before the dest EEXIST -- when the src probe
// fails we fall through and let the copy produce the real error. The FICLONE
// flags are validated but treated as hints: the underlying `copy_file`
// already clones where the platform supports it, and node permits
// COPYFILE_FICLONE_FORCE to succeed when cloning works.

#[op2(fast, stack_trace)]
#[undefined]
pub fn op_node_fs_copy_file_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  src: v8::Local<v8::Value>,
  dest: v8::Local<v8::Value>,
  mode: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let srcpath = validate_path_to_string(scope, src, "src")?;
  let destpath = validate_path_to_string(scope, dest, "dest")?;
  let mode = validate_copy_mode(scope, mode)?;
  let old = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(&srcpath)),
    OpenAccessKind::Read,
    Some("node:fs.copyFileSync"),
  )?;
  let new = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(&destpath)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.copyFileSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  if mode & COPYFILE_EXCL != 0
    && fs.lstat_sync(&old).is_ok()
    && fs.lstat_sync(&new).is_ok()
  {
    return Err(copyfile_eexist(&srcpath, &destpath));
  }
  fs.copy_file_sync(&old, &new)
    .map_err(|e| node_fs_err_dest(e, "copyfile", &srcpath, &destpath))?;
  Ok(())
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_copy_file(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  src: v8::Local<v8::Value>,
  dest: v8::Local<v8::Value>,
  mode: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let srcpath = validate_path_to_string(scope, src, "src")?;
  let destpath = validate_path_to_string(scope, dest, "dest")?;
  let mode = validate_copy_mode(scope, mode)?;
  let old = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&srcpath)),
      OpenAccessKind::Read,
      Some("node:fs.copyFile"),
    )?
    .into_owned();
  let new = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&destpath)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.copyFile"),
    )?
    .into_owned();
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(async move {
    if mode & COPYFILE_EXCL != 0
      && fs.lstat_async(old.clone()).await.is_ok()
      && fs.lstat_async(new.clone()).await.is_ok()
    {
      return Err(copyfile_eexist(&srcpath, &destpath));
    }
    fs.copy_file_async(old, new)
      .await
      .map_err(|e| node_fs_err_dest(e, "copyfile", &srcpath, &destpath))?;
    Ok(())
  })
}

// --- utimes ---

#[op2(fast, stack_trace)]
#[undefined]
pub fn op_node_fs_utime_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  atime: v8::Local<v8::Value>,
  mtime: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let (atime_secs, atime_nanos) =
    unix_time_to_sec_nsec(get_valid_time(scope, atime, "atime")?);
  let (mtime_secs, mtime_nanos) =
    unix_time_to_sec_nsec(get_valid_time(scope, mtime, "mtime")?);
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(&path)),
    OpenAccessKind::WriteNoFollow,
    Some("node:fs.utimesSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>();
  fs.utime_sync(&checked, atime_secs, atime_nanos, mtime_secs, mtime_nanos)
    .map_err(|e| node_fs_err(e, "utime", &path))?;
  Ok(())
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_utime(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  atime: v8::Local<v8::Value>,
  mtime: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let path = validate_path_to_string(scope, path, "path")?;
  let (atime_secs, atime_nanos) =
    unix_time_to_sec_nsec(get_valid_time(scope, atime, "atime")?);
  let (mtime_secs, mtime_nanos) =
    unix_time_to_sec_nsec(get_valid_time(scope, mtime, "mtime")?);
  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      OpenAccessKind::WriteNoFollow,
      Some("node:fs.utimes"),
    )?
    .into_owned();
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(async move {
    fs.utime_async(checked, atime_secs, atime_nanos, mtime_secs, mtime_nanos)
      .await
      .map_err(|e| node_fs_err(e, "utime", &path))?;
    Ok(())
  })
}

// An all-zero `Stats`, the watchFile sentinel for a path that can't be
// stat'ed — matching libuv's `uv_fs_poll_t`, which reports zeroed stats when
// the poll target doesn't exist.
fn empty_stats(bigint: bool) -> Stats {
  Stats {
    dev: 0.0,
    ino: 0.0,
    mode: 0,
    nlink: 0.0,
    uid: 0,
    gid: 0,
    rdev: 0.0,
    size: 0.0,
    blksize: 0.0,
    blocks: 0.0,
    atime_ms: 0.0,
    mtime_ms: 0.0,
    ctime_ms: 0.0,
    birthtime_ms: 0.0,
    is_file: false,
    is_directory: false,
    is_symlink: false,
    is_block_device: false,
    is_char_device: false,
    is_fifo: false,
    is_socket: false,
    is_bigint: bigint,
    date_overrides: RefCell::new([const { None }, None, None, None]),
  }
}

// The fields node's watchFile change detector compares
// (lib/internal/fs/watchers.js onchange). `None` (a failed stat) compares as
// all zeros, mirroring libuv's zeroed `uv_fs_poll_t` stats.
fn watch_cmp_fields(
  stat: &Option<deno_io::fs::FsStat>,
) -> (f64, f64, f64, u32, u32, u32, f64, f64) {
  match stat {
    None => (0.0, 0.0, 0.0, 0, 0, 0, 0.0, 0.0),
    Some(s) => {
      let n = NodeFsStat::from(*s);
      (
        n.mtime_ms.unwrap_or(0.0),
        n.ctime_ms.unwrap_or(0.0),
        n.size,
        n.mode,
        n.uid,
        n.gid,
        n.ino,
        n.dev,
      )
    }
  }
}

// `watchFile`'s change detector: any difference in the fields node compares
// counts as a change (chmod/chown, file replacement, and sub-mtime-resolution
// changes all fire "change").
fn watch_stats_changed(
  prev: &Option<deno_io::fs::FsStat>,
  curr: &Option<deno_io::fs::FsStat>,
) -> bool {
  watch_cmp_fields(prev) != watch_cmp_fields(curr)
}

// The Rust half of `fs.watchFile`'s StatWatcher: owns the poll interval and
// the previous stat snapshot. JS keeps only the EventEmitter shell; each
// `op_node_fs_stat_watcher_poll` call resolves with the next [curr, prev]
// change pair (or null once the resource is closed by `unwatchFile`/`stop`).
struct StatWatcherResource {
  path: String,
  bigint: bool,
  interval_ms: u64,
  poll_state: deno_core::AsyncRefCell<StatWatcherPollState>,
  cancel: deno_core::CancelHandle,
}

#[derive(Default)]
struct StatWatcherPollState {
  started: bool,
  prev: Option<deno_io::fs::FsStat>,
}

impl deno_core::Resource for StatWatcherResource {
  fn name(&self) -> Cow<'_, str> {
    "nodeStatWatcher".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

// A watchFile poll result stat: a real stat (converted with own data
// properties like every op-built Stats), or the zeroed sentinel for a failed
// stat. The sentinel is a bare cppgc object with NO own properties so it
// deep-equals a constructor-built `new fs.Stats()`, matching node (where the
// zeroed stats also come from the binding) and the previous `emptyStats()`
// polyfill helper.
struct WatchStat(Option<deno_io::fs::FsStat>, bool);

impl WatchStat {
  fn into_v8<'a>(
    self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    match self.0 {
      Some(stat) => stats_to_v8(scope, Stats::build(stat, self.1)),
      None => {
        deno_core::cppgc::make_cppgc_object(scope, empty_stats(self.1)).into()
      }
    }
  }
}

// `[curr, prev]` Stats pair for a watchFile "change" emission, or `null` once
// the watcher is stopped (ends the JS poll loop).
struct StatsPair(Option<(WatchStat, WatchStat)>);

impl<'a> ToV8<'a> for StatsPair {
  type Error = std::convert::Infallible;
  fn to_v8(
    self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    Ok(match self.0 {
      Some((curr, prev)) => {
        let curr = curr.into_v8(scope);
        let prev = prev.into_v8(scope);
        v8::Array::new_with_elements(scope, &[curr, prev]).into()
      }
      None => v8::null(scope).into(),
    })
  }
}

#[op2(fast)]
#[smi]
pub fn op_node_fs_stat_watcher_open(
  state: &mut OpState,
  #[string] path: String,
  bigint: bool,
  interval: f64,
) -> ResourceId {
  state.resource_table.add(StatWatcherResource {
    path,
    bigint,
    interval_ms: interval.max(0.0) as u64,
    poll_state: deno_core::AsyncRefCell::new(StatWatcherPollState::default()),
    cancel: deno_core::CancelHandle::default(),
  })
}

// watchFile swallows stat errors into the zeroed-stats sentinel (`None`),
// including permission errors — matching the JS `statAsync` it replaces.
async fn stat_watch_path(
  state: &Rc<RefCell<OpState>>,
  path: &str,
) -> Option<deno_io::fs::FsStat> {
  let (fs, checked) = {
    let mut state = state.borrow_mut();
    let checked = state
      .borrow_mut::<PermissionsContainer>()
      .check_open(
        Cow::Owned(PathBuf::from(path)),
        OpenAccessKind::Read,
        Some("node:fs.watchFile"),
      )
      .ok()?
      .into_owned();
    (state.borrow::<FileSystemRc>().clone(), checked)
  };
  fs.stat_async(checked).await.ok()
}

// Resolves with the next change pair. The first call performs the initial
// stat; libuv emits an initial "change" (with zeroed stats for both args)
// only when that first stat fails, which is mirrored here. Resolves null when
// the resource is closed.
#[op2]
pub async fn op_node_fs_stat_watcher_poll(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<StatsPair, deno_core::error::ResourceError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<StatWatcherResource>(rid)?;
  let mut poll_state = deno_core::RcRef::map(&resource, |r| &r.poll_state)
    .borrow_mut()
    .await;
  if !poll_state.started {
    poll_state.started = true;
    poll_state.prev = stat_watch_path(&state, &resource.path).await;
    if poll_state.prev.is_none() {
      return Ok(StatsPair(Some((
        WatchStat(None, resource.bigint),
        WatchStat(None, resource.bigint),
      ))));
    }
  }
  loop {
    let cancel = deno_core::RcRef::map(&resource, |r| &r.cancel);
    let sleep = tokio::time::sleep(std::time::Duration::from_millis(
      resource.interval_ms,
    ));
    if deno_core::CancelFuture::or_cancel(sleep, cancel)
      .await
      .is_err()
    {
      // Closed by `unwatchFile`/`stop()`.
      return Ok(StatsPair(None));
    }
    let curr = stat_watch_path(&state, &resource.path).await;
    if watch_stats_changed(&poll_state.prev, &curr) {
      let prev = poll_state.prev.take();
      poll_state.prev = curr;
      return Ok(StatsPair(Some((
        WatchStat(poll_state.prev, resource.bigint),
        WatchStat(prev, resource.bigint),
      ))));
    }
  }
}

// Replicates `validateIgnoreOption` from node's lib/internal/fs/watchers.js:
// `options.ignore` may be a non-empty string, RegExp, function, or an array
// of those.
#[op2(fast, stack_trace)]
pub fn op_node_fs_validate_watch_ignore(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<v8::Value>,
  #[string] name: &str,
) -> Result<(), FsError> {
  fn element(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<v8::Value>,
    name: &str,
  ) -> Result<(), FsError> {
    if value.is_string() {
      if v8::Local::<v8::String>::try_from(value).unwrap().length() == 0 {
        return Err(
          err_invalid_arg_value_received(
            name,
            "must be a non-empty string",
            "''",
          )
          .into(),
        );
      }
      return Ok(());
    }
    if value.is_reg_exp() || value.is_function() {
      return Ok(());
    }
    Err(
      err_invalid_arg_type(
        scope,
        name,
        &["string", "RegExp", "Function"],
        value,
      )
      .into(),
    )
  }
  if value.is_null_or_undefined() {
    return Ok(());
  }
  if let Ok(arr) = v8::Local::<v8::Array>::try_from(value) {
    for i in 0..arr.length() {
      let Some(elem) = arr.get_index(scope, i) else {
        continue;
      };
      element(scope, elem, &format!("{name}[{i}]"))?;
    }
    return Ok(());
  }
  element(scope, value, name)
}

// `fs.watch`'s per-event filename shaping (lib/internal/fs/watchers.js):
// `encoding: "buffer"` returns a Buffer, any other named encoding re-encodes
// the utf8 filename, default/utf8 returns the string unchanged. An unknown
// encoding throws ERR_UNKNOWN_ENCODING (matching `Buffer.toString`).
#[op2(stack_trace)]
pub fn op_node_fs_encode_watch_filename(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  #[string] filename: String,
  encoding: v8::Local<v8::Value>,
) -> Result<MaybeEncodedBytes, FsError> {
  let enc = if !encoding.boolean_value(scope) {
    Some(BufEnc::Utf8)
  } else if encoding.is_string()
    && encoding.to_rust_string_lossy(scope) == "buffer"
  {
    None
  } else {
    match parse_buf_encoding(scope, encoding) {
      Some(e) => Some(e),
      None => {
        return Err(
          err_unknown_encoding(&encoding.to_rust_string_lossy(scope)).into(),
        );
      }
    }
  };
  MaybeEncodedBytes::new(state, filename.into_bytes(), enc)
}

#[op2(fast)]
#[undefined]
pub fn op_node_fs_ftruncate_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  fd: v8::Local<v8::Value>,
  len: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  // node's `ftruncateSync(fd, len = 0)`: fd must be a number (the binding
  // validates the value); len defaults to 0, is validateInteger'd, and clamped
  // to >= 0. Replicated here so the op can be bound directly as the public API.
  if !fd.is_number() {
    return Err(err_invalid_arg_type(scope, "fd", &["number"], fd).into());
  }
  let fd = fd.int32_value(scope).unwrap_or(0);
  let len = if len.is_undefined() {
    0
  } else {
    validate_integer(scope, len, "len", MIN_SAFE_INTEGER, MAX_SAFE_INTEGER)?
      .max(0) as u64
  };
  let file = file_for_fd(state, fd).map_err(|_| ebadf_node("ftruncate"))?;
  file.truncate_sync(len)?;
  Ok(())
}

// `async(eager_throw)`: node's `ftruncate(fd, len = 0, cb)` validates fd
// (typeof number) + len synchronously, then truncates asynchronously. Mirrors
// `op_node_fs_ftruncate_sync` so the wrapper can be bound with `callbackifyOpt`.
#[op2(async(eager_throw))]
pub fn op_node_fs_ftruncate(
  scope: &mut v8::PinScope<'_, '_>,
  state: Rc<RefCell<OpState>>,
  fd: v8::Local<v8::Value>,
  len: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  if !fd.is_number() {
    return Err(err_invalid_arg_type(scope, "fd", &["number"], fd).into());
  }
  let fd = fd.int32_value(scope).unwrap_or(0);
  let len = if len.is_undefined() {
    0
  } else {
    validate_integer(scope, len, "len", MIN_SAFE_INTEGER, MAX_SAFE_INTEGER)?
      .max(0) as u64
  };
  Ok(async move {
    let file =
      file_for_fd(&state.borrow(), fd).map_err(|_| ebadf_node("ftruncate"))?;
    file.truncate_async(len).await?;
    Ok(())
  })
}

#[op2(fast)]
#[undefined]
pub fn op_node_fs_fsync_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  fd: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let fd = validate_fd_value(scope, fd)?;
  let file = file_for_fd(state, fd).map_err(|_| ebadf_node("fsync"))?;
  file.sync_sync()?;
  Ok(())
}

// `async(eager_throw)`: node validates `fd` synchronously (validateInt32 ->
// ERR_OUT_OF_RANGE) but delivers EBADF for a valid-but-closed fd via the
// callback. So validate the fd in the eager prologue (synchronous throw) and
// look the fd up inside the future, whose result -- including EBADF -- is
// deferred to the event loop by `eager_throw`'s scheduling and so rejects
// (callback) instead of being rethrown synchronously.
#[op2(async(eager_throw))]
pub fn op_node_fs_fsync(
  scope: &mut v8::PinScope<'_, '_>,
  state: Rc<RefCell<OpState>>,
  fd: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let fd = validate_fd_value(scope, fd)?;
  Ok(async move {
    let file =
      file_for_fd(&state.borrow(), fd).map_err(|_| ebadf_node("fsync"))?;
    file.sync_async().await?;
    Ok(())
  })
}

#[op2(fast)]
#[undefined]
pub fn op_node_fs_fdatasync_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  fd: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let fd = validate_fd_value(scope, fd)?;
  let file = file_for_fd(state, fd).map_err(|_| ebadf_node("fdatasync"))?;
  file.datasync_sync()?;
  Ok(())
}

// See op_node_fs_fsync for the eager-validate / async-EBADF split.
#[op2(async(eager_throw))]
pub fn op_node_fs_fdatasync(
  scope: &mut v8::PinScope<'_, '_>,
  state: Rc<RefCell<OpState>>,
  fd: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let fd = validate_fd_value(scope, fd)?;
  Ok(async move {
    let file =
      file_for_fd(&state.borrow(), fd).map_err(|_| ebadf_node("fdatasync"))?;
    file.datasync_async().await?;
    Ok(())
  })
}

#[op2(fast)]
#[undefined]
pub fn op_node_fs_futimes_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  fd: v8::Local<v8::Value>,
  atime: v8::Local<v8::Value>,
  mtime: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let fd = validate_integer(scope, fd, "fd", 0, 2147483647)? as i32;
  let (atime_secs, atime_nanos) =
    time_to_sec_nsec_full(get_valid_time(scope, atime, "atime")?);
  let (mtime_secs, mtime_nanos) =
    time_to_sec_nsec_full(get_valid_time(scope, mtime, "mtime")?);
  let file = file_for_fd(state, fd).map_err(|_| ebadf_node("futime"))?;
  file.utime_sync(atime_secs, atime_nanos, mtime_secs, mtime_nanos)?;
  Ok(())
}

#[op2(async(eager_throw))]
pub fn op_node_fs_futimes(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  fd: v8::Local<v8::Value>,
  atime: v8::Local<v8::Value>,
  mtime: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let fd = validate_integer(scope, fd, "fd", 0, 2147483647)? as i32;
  let (atime_secs, atime_nanos) =
    time_to_sec_nsec_full(get_valid_time(scope, atime, "atime")?);
  let (mtime_secs, mtime_nanos) =
    time_to_sec_nsec_full(get_valid_time(scope, mtime, "mtime")?);
  // Deferred so a bad fd surfaces as EBADF via the callback, like node.
  let file = file_for_fd(state, fd).map_err(|_| ebadf_node("futime"));
  Ok(async move {
    file?
      .utime_async(atime_secs, atime_nanos, mtime_secs, mtime_nanos)
      .await
      .map_err(|e| fd_syscall_err(e, "futime"))?;
    Ok(())
  })
}

#[op2(fast, stack_trace)]
#[undefined]
pub fn op_node_fs_fchmod_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  fd: v8::Local<v8::Value>,
  mode: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let fd = validate_integer(scope, fd, "fd", 0, 2147483647)? as i32;
  let mode = parse_file_mode(scope, mode, "mode", None)?;
  let file = file_for_fd(state, fd).map_err(|_| ebadf_node("fchmod"))?;
  file.chmod_sync(mode)?;
  Ok(())
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_fchmod(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  fd: v8::Local<v8::Value>,
  mode: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let fd = validate_integer(scope, fd, "fd", 0, 2147483647)? as i32;
  let mode = parse_file_mode(scope, mode, "mode", None)?;
  // Deferred so a bad fd surfaces as EBADF via the callback, like node.
  let file = file_for_fd(state, fd).map_err(|_| ebadf_node("fchmod"));
  Ok(async move {
    file?
      .chmod_async(mode)
      .await
      .map_err(|e| fd_syscall_err(e, "fchmod"))?;
    Ok(())
  })
}

#[op2(fast, stack_trace)]
#[undefined]
pub fn op_node_fs_fchown_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  fd: v8::Local<v8::Value>,
  uid: v8::Local<v8::Value>,
  gid: v8::Local<v8::Value>,
) -> Result<(), FsError> {
  let fd = validate_integer(scope, fd, "fd", 0, 2147483647)? as i32;
  let uid = validate_integer(scope, uid, "uid", -1, K_MAX_USER_ID)? as u32;
  let gid = validate_integer(scope, gid, "gid", -1, K_MAX_USER_ID)? as u32;
  let file = file_for_fd(state, fd).map_err(|_| ebadf_node("fchown"))?;
  file.chown_sync(Some(uid), Some(gid))?;
  Ok(())
}

#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_fchown(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  fd: v8::Local<v8::Value>,
  uid: v8::Local<v8::Value>,
  gid: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<(), FsError>> + use<>, FsError> {
  let fd = validate_integer(scope, fd, "fd", 0, 2147483647)? as i32;
  let uid = validate_integer(scope, uid, "uid", -1, K_MAX_USER_ID)? as u32;
  let gid = validate_integer(scope, gid, "gid", -1, K_MAX_USER_ID)? as u32;
  // Deferred so a bad fd surfaces as EBADF via the callback, like node.
  let file = file_for_fd(state, fd).map_err(|_| ebadf_node("fchown"));
  Ok(async move {
    file?
      .chown_async(Some(uid), Some(gid))
      .await
      .map_err(|e| fd_syscall_err(e, "fchown"))?;
    Ok(())
  })
}

// node's kIoMaxLength: reads beyond 2^31 - 1 bytes throw
// ERR_FS_FILE_TOO_LARGE (checked via fstat before reading, like node's
// readFileHandle, so huge sparse files aren't materialized).
const K_IO_MAX_LENGTH: u64 = 2_147_483_647;

fn err_file_too_large(size: u64) -> NodeArgError {
  NodeArgError {
    class: deno_error::builtin_classes::RANGE_ERROR,
    code: "ERR_FS_FILE_TOO_LARGE",
    message: format!("File size ({size}) is greater than 2 GB"),
  }
}

// fstat the open file and reject regular files larger than kIoMaxLength.
fn check_read_file_size_sync(
  file: &Rc<dyn deno_io::fs::File>,
) -> Result<(), FsError> {
  if let Ok(stat) = file.clone().stat_sync()
    && stat.is_file
    && stat.size > K_IO_MAX_LENGTH
  {
    return Err(err_file_too_large(stat.size).into());
  }
  Ok(())
}

// Cancellation plumbing for the AbortSignal-capable ops (readFile/writeFile):
// JS passes the rid of a `CancelHandle` resource that the signal's abort
// handler closes, interrupting the in-flight I/O. The op-side cancellation
// error is a placeholder -- the JS wrapper always replaces it with node's
// AbortError once `signal.aborted` is set.
fn cancel_handle_for(
  state: &OpState,
  cancel_rid: Option<ResourceId>,
) -> Option<Rc<deno_core::CancelHandle>> {
  cancel_rid.and_then(|rid| {
    state
      .resource_table
      .get::<deno_core::CancelHandle>(rid)
      .ok()
  })
}

async fn with_cancel_handle<T>(
  fut: impl Future<Output = Result<T, FsError>>,
  cancel: Option<Rc<deno_core::CancelHandle>>,
) -> Result<T, FsError> {
  match cancel {
    Some(cancel) => {
      match deno_core::CancelFuture::or_cancel(fut, cancel).await {
        Ok(res) => res,
        Err(_canceled) => Err(FsError::Io(std::io::Error::new(
          std::io::ErrorKind::Interrupted,
          "operation canceled",
        ))),
      }
    }
    None => fut.await,
  }
}

// Extracts `options.flag` (when `options` is an object) for `string_to_flags`;
// the string-options form (`readFileSync(p, "utf8")`) has no flag slot.
fn read_file_flags(
  scope: &mut v8::PinScope<'_, '_>,
  options: v8::Local<v8::Value>,
) -> Result<i32, FsError> {
  let flag_v = if options.is_object() && !options.is_function() {
    let obj = v8::Local::<v8::Object>::try_from(options).unwrap();
    get_prop(scope, obj, "flag")
  } else {
    v8::undefined(scope).into()
  };
  string_to_flags(scope, flag_v, "options.flag")
}

// `fs.readFileSync(path, options)` end to end: validate path, parse options
// (flag via `string_to_flags`, encoding), open, read all bytes, and return
// them already encoded. node reports both open and read failures with
// `syscall: "open"` and the path. A numeric first arg is an already-open fd
// (node's `readFileSync(fd)`) -- read it directly with no open/flags, so this
// op is the whole public `readFileSync`.
#[op2(stack_trace)]
pub fn op_node_fs_read_file_path_sync(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
) -> Result<MaybeEncodedBytes, FsError> {
  let enc = parse_encoding_options(scope, options, None)?;
  if path.is_number() {
    let fd = path.int32_value(scope).unwrap_or(0);
    // node fstats the fd first to size the buffer, so a bad fd reports
    // syscall "fstat"; read failures report "read".
    let file = file_for_fd(state, fd).map_err(|_| ebadf_node("fstat"))?;
    check_read_file_size_sync(&file)?;
    let buf = file
      .read_all_sync()
      .map_err(|e| fd_syscall_err(e, "read"))?;
    return MaybeEncodedBytes::new(state, buf.to_vec(), enc);
  }
  let path = validate_path_to_string(scope, path, "path")?;
  let flags = read_file_flags(scope, options)?;
  let open_options = get_open_options(flags, Some(0o666));
  let checked = state.borrow_mut::<PermissionsContainer>().check_open(
    Cow::Borrowed(Path::new(&path)),
    open_options_to_access_kind(&open_options),
    Some("node:fs.readFileSync"),
  )?;
  let fs = state.borrow::<FileSystemRc>().clone();
  let file = fs
    .open_sync(&checked, open_options)
    .map_err(|e| node_fs_err(e, "open", &path))?;
  check_read_file_size_sync(&file)?;
  let buf = file
    .read_all_sync()
    .map_err(|e| node_fs_err(e, "open", &path))?;
  MaybeEncodedBytes::new(state, buf.to_vec(), enc)
}

// `fs.readFile(path, options)`: async counterpart of the above. `cancel_rid`
// (an AbortSignal-backed CancelHandle) interrupts the open/read when given.
#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_read_file_path(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  path: v8::Local<v8::Value>,
  options: v8::Local<v8::Value>,
  #[smi] cancel_rid: Option<ResourceId>,
) -> Result<
  impl Future<Output = Result<MaybeEncodedBytes, FsError>> + use<>,
  FsError,
> {
  let cancel = cancel_handle_for(state, cancel_rid);
  let path = validate_path_to_string(scope, path, "path")?;
  let enc = parse_encoding_options(scope, options, None)?;
  let flags = read_file_flags(scope, options)?;
  let open_options = get_open_options(flags, Some(0o666));
  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      open_options_to_access_kind(&open_options),
      Some("node:fs.readFile"),
    )?
    .into_owned();
  let proto = if enc.is_none() {
    buffer_proto(state)
  } else {
    None
  };
  let fs = state.borrow::<FileSystemRc>().clone();
  Ok(with_cancel_handle(
    async move {
      // Open + fstat run synchronously on the runtime thread (as the
      // pre-port op_fs_read_file_async did), leaving the read as the
      // single blocking-pool round-trip; the extra hops dominated
      // small-file readFile cost.
      let file = fs
        .open_sync(&checked.as_checked_path(), open_options)
        .map_err(|e| node_fs_err(e, "open", &path))?;
      check_read_file_size_sync(&file)?;
      let buf = file
        .read_all_async()
        .await
        .map_err(|e| node_fs_err(e, "open", &path))?;
      // `into_owned()` avoids re-copying an already-owned read buffer (a large
      // file would otherwise be duplicated just to hand it to ToV8).
      MaybeEncodedBytes::with_proto(buf.into_owned(), enc, proto)
    },
    cancel,
  ))
}

// `fs.readFile(fd, options)`: read all bytes from the fd's current position
// (read_to_end handles unknown-size sources like pipes), returning them
// already encoded per `options.encoding` (default: a Buffer). The
// AbortSignal-capable fd read stays in JS: node guarantees that an abort
// scheduled via process.nextTick before a chunk read completes is observed,
// which needs JS-visible async hops between chunk reads (a one-shot native
// read with or_cancel races the nextTick queue).
#[op2(async(eager_throw), stack_trace)]
pub fn op_node_fs_read_file(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  fd: i32,
  options: v8::Local<v8::Value>,
) -> Result<
  impl Future<Output = Result<MaybeEncodedBytes, FsError>> + use<>,
  FsError,
> {
  let enc = parse_encoding_options(scope, options, None)?;
  let proto = if enc.is_none() {
    buffer_proto(state)
  } else {
    None
  };
  // Deferred so a bad fd rejects (EBADF via the callback) like node. node
  // fstats the fd first to size the buffer, hence syscall "fstat".
  let file = file_for_fd(state, fd).map_err(|_| ebadf_node("fstat"));
  Ok(async move {
    let file = file?;
    // Sync fstat on the runtime thread: a pooled stat round-trip costs
    // more than the syscall.
    check_read_file_size_sync(&file)?;
    let buf = file
      .read_all_async()
      .await
      .map_err(|e| fd_syscall_err(e, "read"))?;
    MaybeEncodedBytes::with_proto(buf.to_vec(), enc, proto)
  })
}

// fstat field-object form for the JS abort-signal fd read (it needs
// isFile/size to mirror node's readFileHandle chunking).
#[op2]
pub fn op_node_fs_fstat_sync(
  state: &mut OpState,
  fd: i32,
) -> Result<NodeFsStat, FsError> {
  let file = file_for_fd(state, fd).map_err(|_| ebadf_node("fstat"))?;
  let stat = file.stat_sync().map_err(|e| fd_syscall_err(e, "fstat"))?;
  Ok(NodeFsStat::from(stat))
}

// Encodes already-read bytes per `options.encoding` (default: a Buffer); used
// by the JS abort-signal fd read path whose data arrives as a plain
// Uint8Array.
#[op2(stack_trace)]
pub fn op_node_fs_encode_bytes(
  scope: &mut v8::PinScope<'_, '_>,
  state: &mut OpState,
  #[buffer] data: &[u8],
  options: v8::Local<v8::Value>,
) -> Result<MaybeEncodedBytes, FsError> {
  let enc = parse_encoding_options(scope, options, None)?;
  MaybeEncodedBytes::new(state, data.to_vec(), enc)
}

// `fs.readvSync(fd, buffers, position)`: read into each `ArrayBufferView` in
// turn, filling each fully (looping over short reads) and stopping at EOF.
// `position >= 0` seeks to that absolute offset first (-1 = current cursor).
// Arg validation stays in JS (this op only does I/O).
#[op2(fast, stack_trace)]
#[smi]
pub fn op_node_fs_readv_sync<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  state: &mut OpState,
  fd: v8::Local<v8::Value>,
  buffers: v8::Local<'a, v8::Value>,
  position: v8::Local<v8::Value>,
) -> Result<u32, FsError> {
  let (fd, buffers, position) =
    validate_vectored_args(scope, fd, buffers, position)?;
  if buffers.length() == 0 {
    return Ok(0);
  }
  let file = file_for_fd(state, fd).map_err(|_| ebadf_node("read"))?;
  if position >= 0 {
    file
      .clone()
      .seek_sync(std::io::SeekFrom::Start(position as u64))
      .map_err(|e| fd_syscall_err(e, "read"))?;
  }
  let mut read_total: u32 = 0;
  'outer: for i in 0..buffers.length() {
    let Some(elem) = buffers.get_index(scope, i) else {
      continue;
    };
    let view =
      v8::Local::<v8::ArrayBufferView>::try_from(elem).map_err(|_| {
        FsError::Io(std::io::Error::from(std::io::ErrorKind::InvalidInput))
      })?;
    let len = view.byte_length();
    if len == 0 {
      continue;
    }
    // SAFETY: no JS runs during this synchronous op and `buffers` keeps the
    // view alive, so `view.data()` (already offset by the view's byte_offset)
    // is valid for `len` bytes. node `Buffer`s are off-heap Uint8Arrays.
    let buf: &mut [u8] =
      unsafe { std::slice::from_raw_parts_mut(view.data() as *mut u8, len) };
    let mut filled = 0usize;
    while filled < len {
      let nread = file
        .clone()
        .read_sync(&mut buf[filled..])
        .map_err(|e| fd_syscall_err(e, "read"))?;
      if nread == 0 {
        break 'outer; // EOF
      }
      filled += nread;
      read_total += nread as u32;
    }
  }
  Ok(read_total)
}

// Holds an `ArrayBufferView`'s backing store alive so the raw slice survives
// across `.await` in `op_node_fs_readv`.
struct RawView {
  _store: v8::SharedRef<v8::BackingStore>,
  data: *mut u8,
  len: usize,
}

// `fs.readv(fd, buffers, position?)`: async analogue of readvSync. Captures each
// view's backing store + pointer in the sync prologue (needs scope), then on
// the event loop optionally seeks and fills each view in order, stopping at EOF.
#[op2(async(eager_throw), stack_trace)]
#[number]
pub fn op_node_fs_readv<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  state: &mut OpState,
  fd: v8::Local<v8::Value>,
  buffers: v8::Local<'a, v8::Value>,
  position: v8::Local<v8::Value>,
) -> Result<impl Future<Output = Result<u64, FsError>> + use<>, FsError> {
  let (fd, buffers, position) =
    validate_vectored_args(scope, fd, buffers, position)?;
  // node short-circuits empty reads before any fd use.
  let file = if buffers.length() == 0 {
    None
  } else {
    Some(file_for_fd(state, fd).map_err(|_| ebadf_node("read")))
  };
  let mut views: Vec<RawView> = Vec::with_capacity(buffers.length() as usize);
  for i in 0..buffers.length() {
    let Some(elem) = buffers.get_index(scope, i) else {
      continue;
    };
    let view =
      v8::Local::<v8::ArrayBufferView>::try_from(elem).map_err(|_| {
        FsError::Io(std::io::Error::from(std::io::ErrorKind::InvalidInput))
      })?;
    let len = view.byte_length();
    let store = view.get_backing_store().ok_or_else(|| {
      FsError::Io(std::io::Error::from(std::io::ErrorKind::InvalidInput))
    })?;
    let data = view.data() as *mut u8; // includes the view's byte_offset
    views.push(RawView {
      _store: store,
      data,
      len,
    });
  }
  let read_err = |e| {
    map_fs_error_to_node_fs_error(
      e,
      NodeFsErrorContext {
        syscall: Some("read".into()),
        ..Default::default()
      },
    )
  };
  Ok(async move {
    let Some(file) = file else {
      return Ok(0);
    };
    let file = file?;
    if position >= 0 {
      file
        .clone()
        .seek_async(std::io::SeekFrom::Start(position as u64))
        .await
        .map_err(read_err)?;
    }
    let mut read_total: u64 = 0;
    'outer: for view in &views {
      if view.len == 0 {
        continue;
      }
      // SAFETY: `view._store` keeps the region alive for the whole future;
      // `data`/`len` describe the view's bytes (offset included).
      let buf = unsafe { std::slice::from_raw_parts_mut(view.data, view.len) };
      let mut filled = 0usize;
      while filled < buf.len() {
        let nread = file
          .clone()
          .read_sync(&mut buf[filled..])
          .map_err(read_err)?;
        if nread == 0 {
          break 'outer; // EOF
        }
        filled += nread;
        read_total += nread as u64;
      }
    }
    Ok(read_total)
  })
}

const CP_IS_DEST_EXISTS_FLAG: u64 = 1u64 << 32;
const CP_IS_SRC_DIRECTORY_FLAG: u64 = 1u64 << 33;
const CP_IS_SRC_FILE_FLAG: u64 = 1u64 << 34;
const CP_IS_SRC_CHAR_DEVICE_FLAG: u64 = 1u64 << 35;
const CP_IS_SRC_BLOCK_DEVICE_FLAG: u64 = 1u64 << 36;
const CP_IS_SRC_SYMLINK_FLAG: u64 = 1u64 << 37;
const CP_IS_SRC_SOCKET_FLAG: u64 = 1u64 << 38;
const CP_IS_SRC_FIFO_FLAG: u64 = 1u64 << 39;

/// Bit-packed stat metadata passed to JS for `cp` operations.
///
/// Layout (u64):
/// - bits 0..31  : src mode (`st_mode`)
/// - bit 32      : destination exists
/// - bits 33..39 : source type flags (dir, file, char/block device, symlink, socket, fifo)
fn compact_stat_info(
  src_stat: &deno_io::fs::FsStat,
  is_dest_exists: bool,
) -> u64 {
  let mut packed = src_stat.mode as u64;
  if is_dest_exists {
    packed |= CP_IS_DEST_EXISTS_FLAG;
  }
  if src_stat.is_file {
    packed |= CP_IS_SRC_FILE_FLAG;
  }
  if src_stat.is_directory {
    packed |= CP_IS_SRC_DIRECTORY_FLAG;
  }
  if src_stat.is_char_device {
    packed |= CP_IS_SRC_CHAR_DEVICE_FLAG;
  }
  if src_stat.is_block_device {
    packed |= CP_IS_SRC_BLOCK_DEVICE_FLAG;
  }
  if src_stat.is_symlink {
    packed |= CP_IS_SRC_SYMLINK_FLAG;
  }
  if src_stat.is_socket {
    packed |= CP_IS_SRC_SOCKET_FLAG;
  }
  if src_stat.is_fifo {
    packed |= CP_IS_SRC_FIFO_FLAG;
  }
  packed
}

struct CpCheckPathsSyncResult {
  is_dest_exists: bool,
  is_src_directory: bool,
  is_src_file: bool,
  is_src_char_device: bool,
  is_src_block_device: bool,
  is_src_symlink: bool,
  is_src_socket: bool,
  is_src_fifo: bool,
  src_mode: u32,
  src_dev: u64,
  src_ino: u64,
}

// To save the cost of multiple round trips between Rust and JS,
// we pack the necessary metadata for `cp` operations into a single u64 value and pass it to JS.
#[derive(Debug)]
struct CpCheckPathsResult {
  src_dev: u64,
  src_ino: u64,
  stat_info: u64,
}

#[derive(Clone, Copy)]
struct CpSyncOptions<'a> {
  dereference: bool,
  recursive: bool,
  force: bool,
  error_on_exist: bool,
  preserve_timestamps: bool,
  verbatim_symlinks: bool,
  mode: u32,
  filter: Option<v8::Local<'a, v8::Function>>,
}

enum CpSyncRunStatus {
  Completed,
  JsException,
}

fn call_cp_sync_filter<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  filter: v8::Local<'a, v8::Function>,
  src: &str,
  dest: &str,
) -> Option<bool> {
  v8::tc_scope!(tc_scope, scope);

  let recv = v8::undefined(tc_scope);
  let src = v8::String::new(tc_scope, src).unwrap();
  let dest = v8::String::new(tc_scope, dest).unwrap();

  let result = filter.call(tc_scope, recv.into(), &[src.into(), dest.into()]);
  if tc_scope.has_caught() || tc_scope.has_terminated() {
    tc_scope.rethrow();
    return None;
  }

  Some(result.unwrap().boolean_value(tc_scope))
}

fn cp_sync_copy_dir<'a>(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  scope: &mut v8::PinScope<'a, '_>,
  src: &str,
  dest: &str,
  opts: CpSyncOptions<'a>,
) -> Result<CpSyncRunStatus, FsError> {
  let src_path = check_cp_path(state, src, OpenAccessKind::ReadNoFollow)?;
  let entries =
    fs.read_dir_sync(&src_path.as_checked_path())
      .map_err(|err| {
        map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(src.to_string()),
            syscall: Some("opendir".into()),
            ..Default::default()
          },
        )
      })?;

  for entry in entries {
    let src_item = Path::new(src)
      .join(&entry.name)
      .to_string_lossy()
      .to_string();
    let dest_item = Path::new(dest)
      .join(&entry.name)
      .to_string_lossy()
      .to_string();

    if let Some(filter) = opts.filter {
      let Some(should_copy) =
        call_cp_sync_filter(scope, filter, &src_item, &dest_item)
      else {
        return Ok(CpSyncRunStatus::JsException);
      };

      if !should_copy {
        continue;
      }
    }

    let stat_info = check_paths_impl_sync(
      state,
      fs,
      &src_item,
      &dest_item,
      opts.dereference,
    )?;

    match cp_sync_dispatch(
      state, fs, scope, &stat_info, &src_item, &dest_item, opts,
    )? {
      CpSyncRunStatus::Completed => {}
      CpSyncRunStatus::JsException => return Ok(CpSyncRunStatus::JsException),
    }
  }

  Ok(CpSyncRunStatus::Completed)
}

fn cp_sync_mkdir_and_copy<'a>(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  scope: &mut v8::PinScope<'a, '_>,
  src_mode: u32,
  src: &str,
  dest: &str,
  opts: CpSyncOptions<'a>,
) -> Result<CpSyncRunStatus, FsError> {
  let dest_path = check_cp_path(state, dest, OpenAccessKind::Write)?;
  fs.mkdir_sync(&dest_path.as_checked_path(), false, None)
    .map_err(|err| {
      map_fs_error_to_node_fs_error(
        err,
        NodeFsErrorContext {
          path: Some(dest.to_string()),
          syscall: Some("mkdir".into()),
          ..Default::default()
        },
      )
    })?;

  let result = cp_sync_copy_dir(state, fs, scope, src, dest, opts)?;
  if let CpSyncRunStatus::JsException = result {
    return Ok(result);
  }

  set_dest_mode(fs, &dest_path.as_checked_path(), src_mode)?;
  Ok(CpSyncRunStatus::Completed)
}

fn cp_sync_on_dir<'a>(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  scope: &mut v8::PinScope<'a, '_>,
  stat_info: &CpCheckPathsSyncResult,
  src: &str,
  dest: &str,
  opts: CpSyncOptions<'a>,
) -> Result<CpSyncRunStatus, FsError> {
  if !stat_info.is_dest_exists {
    return cp_sync_mkdir_and_copy(
      state,
      fs,
      scope,
      stat_info.src_mode,
      src,
      dest,
      opts,
    );
  }

  cp_sync_copy_dir(state, fs, scope, src, dest, opts)
}

fn cp_sync_dispatch<'a>(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  scope: &mut v8::PinScope<'a, '_>,
  stat_info: &CpCheckPathsSyncResult,
  src: &str,
  dest: &str,
  opts: CpSyncOptions<'a>,
) -> Result<CpSyncRunStatus, FsError> {
  if stat_info.is_src_directory && opts.recursive {
    return cp_sync_on_dir(state, fs, scope, stat_info, src, dest, opts);
  } else if stat_info.is_src_directory {
    return Err(
      CpError::EIsDir {
        message: format!("{} is a directory (not copied)", src),
        path: src.to_string(),
      }
      .into(),
    );
  } else if stat_info.is_src_file
    || stat_info.is_src_char_device
    || stat_info.is_src_block_device
  {
    op_node_cp_on_file_sync(state, fs, src, dest, stat_info, &opts)?;
    return Ok(CpSyncRunStatus::Completed);
  } else if stat_info.is_src_symlink {
    op_node_cp_on_link_sync(
      state,
      fs,
      src,
      dest,
      stat_info.is_dest_exists,
      opts.verbatim_symlinks,
    )?;
    return Ok(CpSyncRunStatus::Completed);
  } else if stat_info.is_src_socket {
    return Err(
      CpError::Socket {
        message: format!("cannot copy a socket file: {}", dest),
        path: dest.to_string(),
      }
      .into(),
    );
  } else if stat_info.is_src_fifo {
    return Err(
      CpError::Fifo {
        message: format!("cannot copy a FIFO pipe: {}", dest),
        path: dest.to_string(),
      }
      .into(),
    );
  }

  Err(
    CpError::Unknown {
      message: format!("cannot copy an unknown file type: {}", dest),
      path: dest.to_string(),
    }
    .into(),
  )
}

/// Check if two stat results refer to the same file (same dev + ino).
fn are_identical_stat(
  src_stat: &deno_io::fs::FsStat,
  dest_stat: &deno_io::fs::FsStat,
) -> bool {
  match (dest_stat.ino, src_stat.ino) {
    (Some(dest_ino), Some(src_ino)) if dest_ino > 0 && dest_stat.dev > 0 => {
      dest_ino == src_ino && dest_stat.dev == src_stat.dev
    }
    _ => false,
  }
}

fn is_src_subdir(src: &str, dest: &str) -> bool {
  let src_resolved =
    std::path::absolute(src).unwrap_or_else(|_| PathBuf::from(src));
  let dest_resolved =
    std::path::absolute(dest).unwrap_or_else(|_| PathBuf::from(dest));

  let src_components = src_resolved.components().collect::<Vec<_>>();
  let dest_components = dest_resolved.components().collect::<Vec<_>>();

  src_components
    .iter()
    .enumerate()
    .all(|(i, c)| dest_components.get(i) == Some(c))
}

/// As this function attempts to get the absolute path of the target,
/// callers of this function should ensure that the permissions are granted
/// with at minimum using `.check_read_all()`.
fn cp_symlink_type(
  fs: &FileSystemRc,
  target: &CheckedPathBuf,
  link_path: &CheckedPathBuf,
) -> Option<FsFileType> {
  #[cfg(windows)]
  {
    // Mirror node polyfill behavior: resolve target relative to the link
    // path, infer dir/file from stat, and default to file on any error.
    // https://github.com/nodejs/node/blob/70f6b58ac655234435a99d72b857dd7b316d34bf/lib/fs.js#L1806-L1837
    if let Ok(absolute_target) = std::path::absolute(
      Path::new(link_path.as_os_str())
        .join("..")
        .join(target.as_os_str()),
    ) {
      let checked_target = CheckedPathBuf::unsafe_new(absolute_target);
      let checked_target = checked_target.as_checked_path();
      if let Ok(stat) = fs.stat_sync(&checked_target)
        && stat.is_directory
      {
        return Some(FsFileType::Directory);
      }
    }

    Some(FsFileType::File)
  }

  #[cfg(not(windows))]
  {
    let _ = (fs, target, link_path);
    None
  }
}

#[allow(clippy::unused_async, reason = "sometimes it's async")]
async fn cp_create_symlink(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  target: String,
  link_path: String,
) -> Result<(), FsError> {
  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions.check_write_all("node:fs.symlink")?;
    permissions.check_read_all("node:fs.symlink")?;
  }
  let fs = fs.clone();
  maybe_spawn_blocking!(move || -> Result<(), FsError> {
    // PERMISSIONS: ok because we verified --allow-write and --allow-read above
    let oldpath = CheckedPathBuf::unsafe_new(PathBuf::from(&target));
    let newpath = CheckedPathBuf::unsafe_new(PathBuf::from(&link_path));
    let file_type = cp_symlink_type(&fs, &oldpath, &newpath);
    let oldpath = oldpath.as_checked_path();
    let newpath = newpath.as_checked_path();

    fs.symlink_sync(&oldpath, &newpath, file_type)
      .map_err(|err| {
        map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(target),
            dest: Some(link_path),
            syscall: Some("symlink".into()),
            ..Default::default()
          },
        )
      })?;
    Ok(())
  })
}

fn check_cp_path(
  state: &Rc<RefCell<OpState>>,
  path: &str,
  access_kind: OpenAccessKind,
) -> Result<CheckedPathBuf, FsError> {
  let mut state = state.borrow_mut();
  Ok(
    state
      .borrow_mut::<PermissionsContainer>()
      .check_open(
        Cow::Owned(PathBuf::from(path)),
        access_kind,
        Some("node:fs.cp"),
      )?
      .into_owned(),
  )
}

fn check_paths_impl_sync(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  src: &str,
  dest: &str,
  dereference: bool,
) -> Result<CpCheckPathsSyncResult, FsError> {
  let (src_stat_result, dest_result, syscall) = if dereference {
    let src_path = check_cp_path(state, src, OpenAccessKind::Read)?;
    let dest_path = check_cp_path(state, dest, OpenAccessKind::Read)?;
    (
      fs.stat_sync(&src_path.as_checked_path()),
      fs.stat_sync(&dest_path.as_checked_path()),
      "stat".to_string(),
    )
  } else {
    let src_path = check_cp_path(state, src, OpenAccessKind::ReadNoFollow)?;
    let dest_path = check_cp_path(state, dest, OpenAccessKind::ReadNoFollow)?;
    (
      fs.lstat_sync(&src_path.as_checked_path()),
      fs.lstat_sync(&dest_path.as_checked_path()),
      "lstat".to_string(),
    )
  };

  let src_stat = src_stat_result.map_err(|err| {
    map_fs_error_to_node_fs_error(
      err,
      NodeFsErrorContext {
        path: Some(src.to_string()),
        syscall: Some(syscall.to_string()),
        ..Default::default()
      },
    )
  })?;

  let dest_stat = match dest_result {
    Ok(stat) => Some(stat),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
    Err(e) => {
      return Err(map_fs_error_to_node_fs_error(
        e,
        NodeFsErrorContext {
          path: Some(dest.to_string()),
          syscall: Some(syscall),
          ..Default::default()
        },
      ));
    }
  };

  let src_dev = src_stat.dev;
  let src_ino = src_stat.ino.unwrap_or(0);

  if let Some(ref dest_stat) = dest_stat {
    if are_identical_stat(&src_stat, dest_stat) {
      return Err(
        CpError::EInval {
          message: "src and dest cannot be the same".to_string(),
          path: dest.to_string(),
        }
        .into(),
      );
    }
    if src_stat.is_directory && !dest_stat.is_directory {
      return Err(
        CpError::DirToNonDir {
          message: format!(
            "cannot overwrite non-directory {} with directory {}",
            dest, src
          ),
          path: dest.to_string(),
        }
        .into(),
      );
    }
    if !src_stat.is_directory && dest_stat.is_directory {
      return Err(
        CpError::NonDirToDir {
          message: format!(
            "cannot overwrite directory {} with non-directory {}",
            dest, src
          ),
          path: dest.to_string(),
        }
        .into(),
      );
    }
  }

  if src_stat.is_directory && is_src_subdir(src, dest) {
    return Err(
      CpError::EInval {
        message: format!(
          "cannot copy {} to a subdirectory of self {}",
          src, dest
        ),
        path: dest.to_string(),
      }
      .into(),
    );
  }

  Ok(CpCheckPathsSyncResult {
    src_dev,
    src_ino,
    src_mode: src_stat.mode,
    is_dest_exists: dest_stat.is_some(),
    is_src_directory: src_stat.is_directory,
    is_src_file: src_stat.is_file,
    is_src_char_device: src_stat.is_char_device,
    is_src_block_device: src_stat.is_block_device,
    is_src_symlink: src_stat.is_symlink,
    is_src_socket: src_stat.is_socket,
    is_src_fifo: src_stat.is_fifo,
  })
}

/// Validates src and dest paths for a cp operation.
/// Checks identity, directory type conflicts, and subdirectory relationships.
#[allow(clippy::unused_async, reason = "sometimes async depending on cfg")]
async fn check_paths_impl(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  src: &str,
  dest: &str,
  dereference: bool,
) -> Result<CpCheckPathsResult, FsError> {
  let open_access_kind = if dereference {
    OpenAccessKind::Read
  } else {
    OpenAccessKind::ReadNoFollow
  };

  let src_path = check_cp_path(state, src, open_access_kind)?;
  let dest_path = check_cp_path(state, dest, open_access_kind)?;

  let fs = fs.clone();
  let (src_stat_result, dest_result, syscall) =
    maybe_spawn_blocking!(move || -> (FsResult<_>, FsResult<_>, String) {
      let src_path = src_path.as_checked_path();
      let dest_path = dest_path.as_checked_path();
      if dereference {
        (
          fs.stat_sync(&src_path),
          fs.stat_sync(&dest_path),
          "stat".to_string(),
        )
      } else {
        (
          fs.lstat_sync(&src_path),
          fs.lstat_sync(&dest_path),
          "lstat".to_string(),
        )
      }
    });

  let src_stat = src_stat_result.map_err(|err| {
    map_fs_error_to_node_fs_error(
      err,
      NodeFsErrorContext {
        path: Some(src.to_string()),
        syscall: Some(syscall.clone()),
        ..Default::default()
      },
    )
  })?;

  let dest_stat = match dest_result {
    Ok(stat) => Some(stat),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
    Err(e) => {
      return Err(map_fs_error_to_node_fs_error(
        e,
        NodeFsErrorContext {
          path: Some(dest.to_string()),
          syscall: Some(syscall),
          ..Default::default()
        },
      ));
    }
  };

  let src_dev = src_stat.dev;
  let src_ino = src_stat.ino.unwrap_or(0);

  if let Some(ref dest_stat) = dest_stat {
    if are_identical_stat(&src_stat, dest_stat) {
      return Err(
        CpError::EInval {
          message: "src and dest cannot be the same".to_string(),
          path: dest.to_string(),
        }
        .into(),
      );
    }
    if src_stat.is_directory && !dest_stat.is_directory {
      return Err(
        CpError::DirToNonDir {
          message: format!(
            "cannot overwrite non-directory {} with directory {}",
            dest, src
          ),
          path: dest.to_string(),
        }
        .into(),
      );
    }
    if !src_stat.is_directory && dest_stat.is_directory {
      return Err(
        CpError::NonDirToDir {
          message: format!(
            "cannot overwrite directory {} with non-directory {}",
            dest, src
          ),
          path: dest.to_string(),
        }
        .into(),
      );
    }
  }

  if src_stat.is_directory && is_src_subdir(src, dest) {
    return Err(
      CpError::EInval {
        message: format!(
          "cannot copy {} to a subdirectory of self {}",
          src, dest
        ),
        path: dest.to_string(),
      }
      .into(),
    );
  }

  Ok(CpCheckPathsResult {
    src_dev,
    src_ino,
    stat_info: compact_stat_info(&src_stat, dest_stat.is_some()),
  })
}

/// Validates src and dest paths for recursive cp operations.
/// Returns stat info for the source file
#[op2(stack_trace)]
#[bigint]
pub async fn op_node_cp_check_paths_recursive(
  state: Rc<RefCell<OpState>>,
  #[string] src: String,
  #[string] dest: String,
  dereference: bool,
) -> Result<u64, FsError> {
  let fs = {
    let state = state.borrow();
    state.borrow::<FileSystemRc>().clone()
  };

  let result = check_paths_impl(&state, &fs, &src, &dest, dereference).await?;

  Ok(result.stat_info)
}

/// Validates src and dest paths, checks parent paths, and ensures
/// parent directory exists
/// Returns stat info for the source file
#[op2(stack_trace)]
#[bigint]
pub async fn op_node_cp_validate_and_prepare(
  state: Rc<RefCell<OpState>>,
  #[string] src: String,
  #[string] dest: String,
  dereference: bool,
) -> Result<u64, FsError> {
  let fs = {
    let state = state.borrow();
    state.borrow::<FileSystemRc>().clone()
  };

  let check_result =
    check_paths_impl(&state, &fs, &src, &dest, dereference).await?;

  check_parent_paths_impl(
    &state,
    &fs,
    &src,
    check_result.src_dev,
    check_result.src_ino,
    &dest,
  )
  .await?;

  ensure_parent_dir_impl(&state, &fs, &dest).await?;

  Ok(check_result.stat_info)
}

/// Validates src and dest paths, checks parent paths, and ensures
/// parent directory exists for cpSync.
/// Returns stat info for the source file.
fn op_node_cp_validate_and_prepare_sync(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  src: &str,
  dest: &str,
  dereference: bool,
) -> Result<CpCheckPathsSyncResult, FsError> {
  let check_result = check_paths_impl_sync(state, fs, src, dest, dereference)?;

  check_parent_paths_impl_sync(
    state,
    fs,
    src,
    check_result.src_dev,
    check_result.src_ino,
    dest,
  )?;

  ensure_parent_dir_impl_sync(state, fs, dest)?;

  Ok(check_result)
}

#[op2(fast, reentrant, stack_trace)]
pub fn op_node_cp_sync<'a>(
  state: Rc<RefCell<OpState>>,
  scope: &mut v8::PinScope<'a, '_>,
  #[string] src: &str,
  #[string] dest: &str,
  dereference: bool,
  recursive: bool,
  force: bool,
  error_on_exist: bool,
  preserve_timestamps: bool,
  verbatim_symlinks: bool,
  #[smi] mode: u32,
  filter: v8::Local<'a, v8::Value>,
) -> Result<(), FsError> {
  let fs = {
    let state = state.borrow();
    state.borrow::<FileSystemRc>().clone()
  };

  let stat_info =
    op_node_cp_validate_and_prepare_sync(&state, &fs, src, dest, dereference)?;

  let filter = v8::Local::<v8::Function>::try_from(filter).ok();

  let opts = CpSyncOptions {
    dereference,
    recursive,
    force,
    error_on_exist,
    preserve_timestamps,
    verbatim_symlinks,
    mode,
    filter,
  };

  match cp_sync_dispatch(&state, &fs, scope, &stat_info, src, dest, opts)? {
    // Treat JsException as success here so the pending V8 exception can propagate
    // naturally, preserving the original JS stack instead of rethrowing from Rust.
    CpSyncRunStatus::Completed | CpSyncRunStatus::JsException => Ok(()),
  }
}

/// Recursively check if dest parent is a subdirectory of src.
/// It works for all file types including symlinks since it
/// checks the src and dest inodes. It starts from the deepest
/// parent and stops once it reaches the src parent or the root path.
async fn check_parent_paths_impl(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  src: &str,
  src_dev: u64,
  src_ino: u64,
  dest: &str,
) -> Result<(), FsError> {
  let src_parent = Path::new(src)
    .parent()
    .map(Cow::Borrowed)
    .unwrap_or_default();
  let src_parent = deno_path_util::strip_unc_prefix(
    fs.realpath_sync(&CheckedPath::unsafe_new(Cow::Borrowed(&src_parent)))
      .unwrap_or_else(|_| {
        fs.cwd()
          .map(|cwd| cwd.join(&src_parent))
          .unwrap_or_else(|_| src_parent.into_owned())
      }),
  );

  let mut current = Path::new(dest)
    .parent()
    .map(|p| p.to_path_buf())
    .unwrap_or_default();
  current = deno_path_util::strip_unc_prefix(
    fs.realpath_sync(&CheckedPath::unsafe_new(Cow::Borrowed(&current)))
      .unwrap_or_else(|_| std::path::absolute(&current).unwrap_or(current)),
  );

  loop {
    if current == src_parent {
      return Ok(());
    }

    // Check if current is the root
    if current.parent().is_none() || current.parent() == Some(&current) {
      return Ok(());
    }

    let current_str = current.to_string_lossy();
    let checked_path =
      match check_cp_path(state, &current_str, OpenAccessKind::Read) {
        Ok(p) => p,
        // When read permission is ignored, the check returns NotFound.
        // Treat it like a non-existent directory: stop walking.
        Err(FsError::Permission(e))
          if e.kind() == std::io::ErrorKind::NotFound =>
        {
          return Ok(());
        }
        Err(e) => return Err(e),
      };
    let stat_result = fs.stat_async(checked_path).await;
    match stat_result {
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
      Err(e) => {
        return Err(map_fs_error_to_node_fs_error(
          e,
          NodeFsErrorContext {
            path: Some(current_str.to_string()),
            syscall: Some("stat".to_string()),
            ..Default::default()
          },
        ));
      }
      Ok(dest_stat) => {
        // Check if src and current parent are identical (same dev + ino)
        if let Some(dest_ino) = dest_stat.ino
          && dest_ino == src_ino
          && dest_stat.dev == src_dev
        {
          return Err(
            CpError::EInval {
              message: format!(
                "cannot copy {} to a subdirectory of self {}",
                src, dest
              ),
              path: dest.to_string(),
            }
            .into(),
          );
        }
      }
    }

    current = match current.parent() {
      Some(p) => p.to_path_buf(),
      None => return Ok(()),
    };
  }
}

fn check_parent_paths_impl_sync(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  src: &str,
  src_dev: u64,
  src_ino: u64,
  dest: &str,
) -> Result<(), FsError> {
  let src_parent = Path::new(src)
    .parent()
    .map(Cow::Borrowed)
    .unwrap_or_default();
  let src_parent = deno_path_util::strip_unc_prefix(
    fs.realpath_sync(&CheckedPath::unsafe_new(Cow::Borrowed(&src_parent)))
      .unwrap_or_else(|_| {
        fs.cwd()
          .map(|cwd| cwd.join(&src_parent))
          .unwrap_or_else(|_| src_parent.into_owned())
      }),
  );

  let mut current = Path::new(dest)
    .parent()
    .map(|p| p.to_path_buf())
    .unwrap_or_default();
  current = deno_path_util::strip_unc_prefix(
    fs.realpath_sync(&CheckedPath::unsafe_new(Cow::Borrowed(&current)))
      .unwrap_or_else(|_| std::path::absolute(&current).unwrap_or(current)),
  );

  loop {
    if current == src_parent {
      return Ok(());
    }

    // Check if current is the root
    if current.parent().is_none() || current.parent() == Some(&current) {
      return Ok(());
    }

    let current_str = current.to_string_lossy();
    let checked_path =
      match check_cp_path(state, &current_str, OpenAccessKind::Read) {
        Ok(p) => p,
        // When read permission is ignored, the check returns NotFound.
        // Treat it like a non-existent directory: stop walking.
        Err(FsError::Permission(e))
          if e.kind() == std::io::ErrorKind::NotFound =>
        {
          return Ok(());
        }
        Err(e) => return Err(e),
      };
    match fs.stat_sync(&checked_path.as_checked_path()) {
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
      Err(e) => {
        return Err(map_fs_error_to_node_fs_error(
          e,
          NodeFsErrorContext {
            path: Some(current_str.to_string()),
            syscall: Some("stat".to_string()),
            ..Default::default()
          },
        ));
      }
      Ok(dest_stat) => {
        // Check if src and current parent are identical (same dev + ino)
        if let Some(dest_ino) = dest_stat.ino
          && dest_ino == src_ino
          && dest_stat.dev == src_dev
        {
          return Err(
            CpError::EInval {
              message: format!(
                "cannot copy {} to a subdirectory of self {}",
                src, dest
              ),
              path: dest.to_string(),
            }
            .into(),
          );
        }
      }
    }

    current = match current.parent() {
      Some(p) => p.to_path_buf(),
      None => return Ok(()),
    };
  }
}

/// Ensures the parent directory of `dest` exists, creating it recursively
/// if needed.
async fn ensure_parent_dir_impl(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  dest: &str,
) -> Result<(), FsError> {
  let dest_parent = Path::new(dest)
    .parent()
    .map(|p| p.to_path_buf())
    .unwrap_or_default();

  let parent_str = dest_parent.to_string_lossy();
  let checked_parent = check_cp_path(state, &parent_str, OpenAccessKind::Read)?;
  let exists = fs.exists_async(checked_parent).await.map_err(|err| {
    map_fs_error_to_node_fs_error(
      err,
      NodeFsErrorContext {
        path: Some(parent_str.to_string()),
        syscall: Some("exists".into()),
        ..Default::default()
      },
    )
  })?;
  if !exists {
    let checked_parent =
      check_cp_path(state, &parent_str, OpenAccessKind::Write)?;
    fs.mkdir_async(checked_parent, true, None)
      .await
      .map_err(|err| {
        map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(parent_str.to_string()),
            syscall: Some("mkdir".into()),
            ..Default::default()
          },
        )
      })?;
  }
  Ok(())
}

fn ensure_parent_dir_impl_sync(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  dest: &str,
) -> Result<(), FsError> {
  let dest_parent = Path::new(dest)
    .parent()
    .map(|p| p.to_path_buf())
    .unwrap_or_default();

  let parent_str = dest_parent.to_string_lossy();
  let checked_parent = check_cp_path(state, &parent_str, OpenAccessKind::Read)?;
  let exists = fs.exists_sync(&checked_parent.as_checked_path());

  if !exists {
    let checked_parent =
      check_cp_path(state, &parent_str, OpenAccessKind::Write)?;
    fs.mkdir_sync(&checked_parent.as_checked_path(), true, None)
      .map_err(|err| {
        map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(parent_str.to_string()),
            syscall: Some("mkdir".into()),
            ..Default::default()
          },
        )
      })?;
  }

  Ok(())
}

fn handle_timestamps_and_mode(
  fs: &FileSystemRc,
  src_path: &CheckedPath,
  dest_path: &CheckedPath,
  src_mode: u32,
) -> Result<(), FsError> {
  // Make sure the file is writable before setting the timestamp
  // otherwise open fails with EPERM when invoked with 'r+' (through utimes call)
  if file_is_not_writable(src_mode) {
    let mode = src_mode | 0o200;
    set_dest_mode(fs, dest_path, mode)?;
  }

  // Set timestamps from a fresh stat of src (atime is modified by read).
  set_dest_timestamps_and_mode(fs, src_path, dest_path, src_mode)?;
  Ok(())
}

fn file_is_not_writable(mode: u32) -> bool {
  (mode & 0o200) == 0
}

fn cp_mode_has_copyfile_excl(mode: u32) -> bool {
  let copyfile_excl = UV_FS_COPYFILE_EXCL as u32;
  (mode & copyfile_excl) == copyfile_excl
}

fn set_dest_mode(
  fs: &FileSystemRc,
  dest_path: &CheckedPath,
  mode: u32,
) -> Result<(), FsError> {
  if mode == 0 {
    return Ok(());
  }

  fs.chmod_sync(dest_path, mode as _).map_err(|err| {
    map_fs_error_to_node_fs_error(
      err,
      NodeFsErrorContext {
        path: Some(dest_path.to_string_lossy().to_string()),
        syscall: Some("chmod".into()),
        ..Default::default()
      },
    )
  })?;
  Ok(())
}

fn set_dest_timestamps_and_mode(
  fs: &FileSystemRc,
  src_path: &CheckedPath,
  dest_path: &CheckedPath,
  src_mode: u32,
) -> Result<(), FsError> {
  // Re-stat src to get fresh atime/mtime
  let src_stat = fs.stat_sync(src_path).map_err(|err| {
    map_fs_error_to_node_fs_error(
      err,
      NodeFsErrorContext {
        path: Some(src_path.to_string_lossy().to_string()),
        syscall: Some("stat".into()),
        ..Default::default()
      },
    )
  })?;

  if let (Some(atime), Some(mtime)) = (src_stat.atime, src_stat.mtime) {
    // FsStat stores times as signed milliseconds since the Unix epoch.
    // utime_sync expects split values: whole seconds + nanoseconds remainder.
    // Floored (euclidean) division keeps the ns remainder in [0, 1e9) for
    // pre-epoch (negative) source times.
    let atime_secs = atime.div_euclid(1000);
    let atime_nanos = (atime.rem_euclid(1000) as u32) * 1_000_000;
    let mtime_secs = mtime.div_euclid(1000);
    let mtime_nanos = (mtime.rem_euclid(1000) as u32) * 1_000_000;
    fs.utime_sync(dest_path, atime_secs, atime_nanos, mtime_secs, mtime_nanos)
      .map_err(|err| {
        map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(dest_path.to_string_lossy().to_string()),
            syscall: Some("utime".into()),
            ..Default::default()
          },
        )
      })?;
  }

  set_dest_mode(fs, dest_path, src_mode)?;
  Ok(())
}

/// Handle copying a single file.
///
/// If dest does not exist, copies src to dest directly.
/// If dest exists and force is true, removes dest and copies.
/// If dest exists and error_on_exist is true, returns an EExist error.
/// Otherwise does nothing.
///
/// When preserve_timestamps is true, copies the timestamps from src to dest
/// and ensures the file is writable before doing so.
#[op2(stack_trace)]
pub async fn op_node_cp_on_file(
  state: Rc<RefCell<OpState>>,
  #[string] src: String,
  #[string] dest: String,
  #[smi] src_mode: u32,
  dest_exists: bool,
  force: bool,
  error_on_exist: bool,
  preserve_timestamps: bool,
  #[smi] mode: u32,
) -> Result<(), FsError> {
  let fs = {
    let state = state.borrow();
    state.borrow::<FileSystemRc>().clone()
  };

  if dest_exists {
    if force {
      // Remove dest, then copy
      let dest_path = check_cp_path(&state, &dest, OpenAccessKind::Write)?;
      fs.remove_async(dest_path, false).await.map_err(|err| {
        map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(dest.clone()),
            syscall: Some("unlink".into()),
            ..Default::default()
          },
        )
      })?;
    } else if error_on_exist {
      return Err(
        CpError::EExist {
          message: format!("{} already exists", dest),
          path: dest.to_string(),
        }
        .into(),
      );
    } else {
      // Neither force nor errorOnExist: do nothing
      return Ok(());
    }
  }

  // Copy file: read from src, write to dest
  if cp_mode_has_copyfile_excl(mode) {
    let dest_path = check_cp_path(&state, &dest, OpenAccessKind::ReadNoFollow)?;
    match fs.lstat_async(dest_path.clone()).await {
      Ok(_) => {
        return Err(
          CpError::EExist {
            message: format!("{} already exists", dest),
            path: dest.to_string(),
          }
          .into(),
        );
      }
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
      Err(err) => {
        return Err(map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(dest.to_string()),
            syscall: Some("lstat".into()),
            ..Default::default()
          },
        ));
      }
    }
  }

  let src_path = check_cp_path(&state, &src, OpenAccessKind::Read)?;
  let dest_path = check_cp_path(&state, &dest, OpenAccessKind::Write)?;

  maybe_spawn_blocking!(move || -> Result<(), FsError> {
    fs.copy_file_sync(
      &src_path.as_checked_path(),
      &dest_path.as_checked_path(),
    )
    .map_err(|err| {
      map_fs_error_to_node_fs_error(
        err,
        NodeFsErrorContext {
          path: Some(src_path.to_string_lossy().to_string()),
          dest: Some(dest_path.to_string_lossy().to_string()),
          syscall: Some("copyfile".into()),
          ..Default::default()
        },
      )
    })?;
    if preserve_timestamps {
      handle_timestamps_and_mode(
        &fs,
        &src_path.as_checked_path(),
        &dest_path.as_checked_path(),
        src_mode,
      )?;
    } else {
      set_dest_mode(&fs, &dest_path.as_checked_path(), src_mode)?;
    }
    Ok(())
  })
}

fn op_node_cp_on_file_sync(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  src: &str,
  dest: &str,
  stat_info: &CpCheckPathsSyncResult,
  opts: &CpSyncOptions,
) -> Result<(), FsError> {
  if stat_info.is_dest_exists {
    if opts.force {
      // Remove dest, then copy.
      let dest_path = check_cp_path(state, dest, OpenAccessKind::Write)?;
      fs.remove_sync(&dest_path.as_checked_path(), false)
        .map_err(|err| {
          map_fs_error_to_node_fs_error(
            err,
            NodeFsErrorContext {
              path: Some(dest.to_string()),
              syscall: Some("unlink".into()),
              ..Default::default()
            },
          )
        })?;
    } else if opts.error_on_exist {
      return Err(
        CpError::EExist {
          message: format!("{} already exists", dest),
          path: dest.to_string(),
        }
        .into(),
      );
    } else {
      // Neither force nor errorOnExist: do nothing.
      return Ok(());
    }
  }

  if cp_mode_has_copyfile_excl(opts.mode) {
    let dest_path = check_cp_path(state, dest, OpenAccessKind::ReadNoFollow)?;
    match fs.lstat_sync(&dest_path.as_checked_path()) {
      Ok(_) => {
        return Err(
          CpError::EExist {
            message: format!("{} already exists", dest),
            path: dest.to_string(),
          }
          .into(),
        );
      }
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
      Err(err) => {
        return Err(map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(dest.to_string()),
            syscall: Some("lstat".into()),
            ..Default::default()
          },
        ));
      }
    }
  }

  let src_path = check_cp_path(state, src, OpenAccessKind::Read)?;
  let dest_path = check_cp_path(state, dest, OpenAccessKind::Write)?;

  fs.copy_file_sync(&src_path.as_checked_path(), &dest_path.as_checked_path())
    .map_err(|err| {
      map_fs_error_to_node_fs_error(
        err,
        NodeFsErrorContext {
          path: Some(src.to_string()),
          dest: Some(dest.to_string()),
          syscall: Some("copyfile".into()),
          ..Default::default()
        },
      )
    })?;

  if opts.preserve_timestamps {
    handle_timestamps_and_mode(
      fs,
      &src_path.as_checked_path(),
      &dest_path.as_checked_path(),
      stat_info.src_mode,
    )?;
  } else {
    set_dest_mode(fs, &dest_path.as_checked_path(), stat_info.src_mode)?;
  }

  Ok(())
}

#[op2(stack_trace)]
pub async fn op_node_cp_on_link(
  state: Rc<RefCell<OpState>>,
  #[string] src: String,
  #[string] dest: String,
  dest_exists: bool,
  verbatim_symlinks: bool,
) -> Result<(), FsError> {
  let fs = {
    let state = state.borrow();
    state.borrow::<FileSystemRc>().clone()
  };

  let src_path = check_cp_path(&state, &src, OpenAccessKind::ReadNoFollow)?;
  let resolved_src_buf =
    fs.read_link_async(src_path.clone()).await.map_err(|err| {
      map_fs_error_to_node_fs_error(
        err,
        NodeFsErrorContext {
          path: Some(src.clone()),
          syscall: Some("readlink".into()),
          ..Default::default()
        },
      )
    })?;
  let mut resolved_src = resolved_src_buf.to_string_lossy().to_string();

  // Resolve relative symlink targets
  if !verbatim_symlinks
    && !Path::new(&resolved_src).is_absolute()
    && let Some(parent) = Path::new(&src).parent()
  {
    resolved_src = parent.join(&resolved_src).to_string_lossy().to_string();
  }

  if !dest_exists {
    cp_create_symlink(&state, &fs, resolved_src.to_string(), dest).await?;
    return Ok(());
  }

  // Dest exists — try to read it as a symlink
  let dest_path = check_cp_path(&state, &dest, OpenAccessKind::ReadNoFollow)?;
  let resolved_dest_result = fs.read_link_async(dest_path).await;
  let resolved_dest = match resolved_dest_result {
    Ok(p) => {
      let s = p.to_string_lossy().to_string();
      // If relative, resolve against dirname(dest)
      if !Path::new(&s).is_absolute() {
        if let Some(parent) = Path::new(&dest).parent() {
          parent.join(&s).to_string_lossy().to_string()
        } else {
          s
        }
      } else {
        s
      }
    }
    Err(e) => {
      let kind = e.kind();
      // EINVAL or UNKNOWN means dest is a regular file/directory, not a symlink
      if kind == std::io::ErrorKind::InvalidInput
        || kind == std::io::ErrorKind::Other
      {
        cp_create_symlink(
          &state,
          &fs,
          resolved_src.to_string(),
          dest.to_string(),
        )
        .await?;
        return Ok(());
      }

      #[cfg(windows)]
      {
        use windows_sys::Win32::Foundation::ERROR_NOT_A_REPARSE_POINT;

        let os_error = e.into_io_error();
        let Some(errno) = os_error.raw_os_error() else {
          return Err(FsError::Io(os_error));
        };

        let errno = errno as u32;
        if errno != ERROR_NOT_A_REPARSE_POINT {
          return Err(
            NodeFsError::new(
              errno as i32,
              NodeFsErrorContext {
                path: Some(resolved_src),
                dest: Some(dest),
                syscall: Some("symlink".into()),
                ..Default::default()
              },
            )
            .into(),
          );
        }

        return cp_create_symlink(
          &state,
          &fs,
          resolved_src.to_string(),
          dest.to_string(),
        )
        .await;
      }
      #[cfg(not(windows))]
      {
        return Err(map_fs_error_to_node_fs_error(
          e,
          NodeFsErrorContext {
            path: Some(resolved_src),
            dest: Some(dest),
            syscall: Some("symlink".into()),
            ..Default::default()
          },
        ));
      }
    }
  };

  // Check subdirectory relationships
  let src_path = check_cp_path(&state, &src, OpenAccessKind::Read)?;
  let src_stat = fs.stat_async(src_path).await.map_err(|err| {
    map_fs_error_to_node_fs_error(
      err,
      NodeFsErrorContext {
        path: Some(src),
        syscall: Some("stat".into()),
        ..Default::default()
      },
    )
  })?;

  let src_is_dir = src_stat.is_directory;
  if src_is_dir && is_src_subdir(&resolved_src, &resolved_dest) {
    return Err(
      CpError::EInval {
        message: format!(
          "cannot copy {} to a subdirectory of self {}",
          resolved_src, resolved_dest
        ),
        path: dest.to_string(),
      }
      .into(),
    );
  }

  // Do not copy if src is a subdir of dest since unlinking
  // dest would remove src contents and create a broken symlink.
  if src_is_dir && is_src_subdir(&resolved_dest, &resolved_src) {
    return Err(
      CpError::SymlinkToSubdirectory {
        message: format!(
          "cannot overwrite {} with {}",
          resolved_dest, resolved_src
        ),
        path: dest.to_string(),
      }
      .into(),
    );
  }

  {
    let mut state = state.borrow_mut();
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions.check_write_all("node:fs.cp")?;
    permissions.check_read_all("node:fs.cp")?;
  }

  // Unlink dest and create new symlink
  maybe_spawn_blocking!(move || -> Result<(), FsError> {
    let src_path_buf = CheckedPathBuf::unsafe_new(PathBuf::from(&resolved_src));
    let dest_path_buf = CheckedPathBuf::unsafe_new(PathBuf::from(&dest));
    let src_path = src_path_buf.as_checked_path();
    let dest_path = dest_path_buf.as_checked_path();

    fs.remove_sync(&dest_path, false).map_err(|err| {
      map_fs_error_to_node_fs_error(
        err,
        NodeFsErrorContext {
          path: Some(dest_path.to_string_lossy().to_string()),
          syscall: Some("unlink".into()),
          ..Default::default()
        },
      )
    })?;

    let file_type = cp_symlink_type(&fs, &src_path_buf, &dest_path_buf);
    fs.symlink_sync(&src_path, &dest_path, file_type)
      .map_err(|err| {
        map_fs_error_to_node_fs_error(
          err,
          NodeFsErrorContext {
            path: Some(src_path.to_string_lossy().to_string()),
            dest: Some(dest_path.to_string_lossy().to_string()),
            syscall: Some("symlink".into()),
            ..Default::default()
          },
        )
      })
  })
}

fn op_node_cp_on_link_sync(
  state: &Rc<RefCell<OpState>>,
  fs: &FileSystemRc,
  src: &str,
  dest: &str,
  dest_exists: bool,
  verbatim_symlinks: bool,
) -> Result<(), FsError> {
  let src_path = check_cp_path(state, src, OpenAccessKind::ReadNoFollow)?;
  let resolved_src_buf = fs
    .read_link_sync(&src_path.as_checked_path())
    .map_err(|err| {
      map_fs_error_to_node_fs_error(
        err,
        NodeFsErrorContext {
          path: Some(src.to_string()),
          syscall: Some("readlink".into()),
          ..Default::default()
        },
      )
    })?;
  let mut resolved_src = resolved_src_buf.to_string_lossy().to_string();

  // Resolve relative symlink targets.
  if !verbatim_symlinks
    && !Path::new(&resolved_src).is_absolute()
    && let Some(parent) = Path::new(src).parent()
  {
    resolved_src = parent.join(&resolved_src).to_string_lossy().to_string();
  }

  if !dest_exists {
    {
      let mut state = state.borrow_mut();
      state
        .borrow_mut::<PermissionsContainer>()
        .check_write_all("node:fs.symlink")?;
      state
        .borrow_mut::<PermissionsContainer>()
        .check_read_all("node:fs.symlink")?;
    }

    let oldpath = CheckedPathBuf::unsafe_new(PathBuf::from(&resolved_src));
    let newpath = CheckedPathBuf::unsafe_new(PathBuf::from(dest));
    let file_type = cp_symlink_type(fs, &oldpath, &newpath);
    fs.symlink_sync(
      &oldpath.as_checked_path(),
      &newpath.as_checked_path(),
      file_type,
    )
    .map_err(|err| {
      map_fs_error_to_node_fs_error(
        err,
        NodeFsErrorContext {
          path: Some(resolved_src),
          dest: Some(dest.to_string()),
          syscall: Some("symlink".into()),
          ..Default::default()
        },
      )
    })?;
    return Ok(());
  }

  // Dest exists — try to read it as a symlink.
  let dest_path = check_cp_path(state, dest, OpenAccessKind::ReadNoFollow)?;
  let resolved_dest_result = fs.read_link_sync(&dest_path.as_checked_path());
  let resolved_dest = match resolved_dest_result {
    Ok(p) => {
      let s = p.to_string_lossy().to_string();
      // If relative, resolve against dirname(dest).
      if !Path::new(&s).is_absolute() {
        if let Some(parent) = Path::new(dest).parent() {
          parent.join(&s).to_string_lossy().to_string()
        } else {
          s
        }
      } else {
        s
      }
    }
    Err(e) => {
      let kind = e.kind();
      // EINVAL or UNKNOWN means dest is a regular file/directory, not a symlink.
      if kind == std::io::ErrorKind::InvalidInput
        || kind == std::io::ErrorKind::Other
      {
        {
          let mut state = state.borrow_mut();
          state
            .borrow_mut::<PermissionsContainer>()
            .check_write_all("node:fs.symlink")?;
          state
            .borrow_mut::<PermissionsContainer>()
            .check_read_all("node:fs.symlink")?;
        }

        let oldpath = CheckedPathBuf::unsafe_new(PathBuf::from(&resolved_src));
        let newpath = CheckedPathBuf::unsafe_new(PathBuf::from(dest));
        let file_type = cp_symlink_type(fs, &oldpath, &newpath);
        fs.symlink_sync(
          &oldpath.as_checked_path(),
          &newpath.as_checked_path(),
          file_type,
        )
        .map_err(|err| {
          map_fs_error_to_node_fs_error(
            err,
            NodeFsErrorContext {
              path: Some(resolved_src.clone()),
              dest: Some(dest.to_string()),
              syscall: Some("symlink".into()),
              ..Default::default()
            },
          )
        })?;
        return Ok(());
      }

      #[cfg(windows)]
      {
        use windows_sys::Win32::Foundation::ERROR_NOT_A_REPARSE_POINT;

        let os_error = e.into_io_error();
        let Some(errno) = os_error.raw_os_error() else {
          return Err(FsError::Io(os_error));
        };

        let errno = errno as u32;
        if errno != ERROR_NOT_A_REPARSE_POINT {
          return Err(
            NodeFsError::new(
              errno as i32,
              NodeFsErrorContext {
                path: Some(resolved_src),
                dest: Some(dest.to_string()),
                syscall: Some("symlink".into()),
                ..Default::default()
              },
            )
            .into(),
          );
        }

        {
          let mut state = state.borrow_mut();
          state
            .borrow_mut::<PermissionsContainer>()
            .check_write_all("node:fs.symlink")?;
          state
            .borrow_mut::<PermissionsContainer>()
            .check_read_all("node:fs.symlink")?;
        }

        let oldpath = CheckedPathBuf::unsafe_new(PathBuf::from(&resolved_src));
        let newpath = CheckedPathBuf::unsafe_new(PathBuf::from(dest));
        let file_type = cp_symlink_type(fs, &oldpath, &newpath);
        fs.symlink_sync(
          &oldpath.as_checked_path(),
          &newpath.as_checked_path(),
          file_type,
        )
        .map_err(|err| {
          map_fs_error_to_node_fs_error(
            err,
            NodeFsErrorContext {
              path: Some(resolved_src.clone()),
              dest: Some(dest.to_string()),
              syscall: Some("symlink".into()),
              ..Default::default()
            },
          )
        })?;
        return Ok(());
      }
      #[cfg(not(windows))]
      {
        return Err(map_fs_error_to_node_fs_error(
          e,
          NodeFsErrorContext {
            path: Some(resolved_src),
            dest: Some(dest.to_string()),
            syscall: Some("symlink".into()),
            ..Default::default()
          },
        ));
      }
    }
  };

  // Check subdirectory relationships.
  let src_path = check_cp_path(state, src, OpenAccessKind::Read)?;
  let src_stat = fs.stat_sync(&src_path.as_checked_path()).map_err(|err| {
    map_fs_error_to_node_fs_error(
      err,
      NodeFsErrorContext {
        path: Some(src.to_string()),
        syscall: Some("stat".into()),
        ..Default::default()
      },
    )
  })?;

  let src_is_dir = src_stat.is_directory;
  if src_is_dir && is_src_subdir(&resolved_src, &resolved_dest) {
    return Err(
      CpError::EInval {
        message: format!(
          "cannot copy {} to a subdirectory of self {}",
          resolved_src, resolved_dest
        ),
        path: dest.to_string(),
      }
      .into(),
    );
  }

  // Do not copy if src is a subdir of dest since unlinking
  // dest would remove src contents and create a broken symlink.
  if src_is_dir && is_src_subdir(&resolved_dest, &resolved_src) {
    return Err(
      CpError::SymlinkToSubdirectory {
        message: format!(
          "cannot overwrite {} with {}",
          resolved_dest, resolved_src
        ),
        path: dest.to_string(),
      }
      .into(),
    );
  }

  let dest_path = {
    let mut state = state.borrow_mut();
    state.borrow_mut::<PermissionsContainer>().check_open(
      Cow::Owned(PathBuf::from(dest)),
      OpenAccessKind::Write,
      Some("node:fs.rm"),
    )?
  };

  fs.remove_sync(&dest_path, false).map_err(|err| {
    map_fs_error_to_node_fs_error(
      err,
      NodeFsErrorContext {
        path: Some(dest_path.to_string_lossy().to_string()),
        syscall: Some("unlink".into()),
        ..Default::default()
      },
    )
  })?;

  {
    let mut state = state.borrow_mut();
    state
      .borrow_mut::<PermissionsContainer>()
      .check_write_all("node:fs.symlink")?;
    state
      .borrow_mut::<PermissionsContainer>()
      .check_read_all("node:fs.symlink")?;
  }

  let src_path_buf = CheckedPathBuf::unsafe_new(PathBuf::from(&resolved_src));
  let dest_path_buf = CheckedPathBuf::unsafe_new(dest_path.to_path_buf());
  let src_path = src_path_buf.as_checked_path();

  let file_type = cp_symlink_type(fs, &src_path_buf, &dest_path_buf);
  fs.symlink_sync(&src_path, &dest_path, file_type)
    .map_err(|err| {
      map_fs_error_to_node_fs_error(
        err,
        NodeFsErrorContext {
          path: Some(src_path.to_string_lossy().to_string()),
          dest: Some(dest_path.to_string_lossy().to_string()),
          syscall: Some("symlink".into()),
          ..Default::default()
        },
      )
    })
}

#[cfg(test)]
mod tests {
  use super::is_src_subdir;

  #[test]
  fn test_is_src_subdir() {
    let base = std::env::temp_dir().join("deno_is_src_subdir_test");
    let src = base.join("src");
    let child = src.join("child");
    let sibling = base.join("sibling");

    let src = src.to_string_lossy().into_owned();
    let child = child.to_string_lossy().into_owned();
    let sibling = sibling.to_string_lossy().into_owned();

    assert!(is_src_subdir(&src, &child));
    assert!(is_src_subdir(&src, &src));
    assert!(!is_src_subdir(&src, &sibling));
    assert!(!is_src_subdir(&child, &src));
  }
}
