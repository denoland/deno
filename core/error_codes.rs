use crate::error::AnyError;

pub(crate) fn get_error_code(err: &AnyError) -> Option<&'static str> {
  err.downcast_ref::<std::io::Error>().and_then(|e| {
    let code = get_io_error_code(e);
    match code.is_empty() {
      true => None,
      false => Some(code),
    }
  })
}

fn get_io_error_code(err: &std::io::Error) -> &'static str {
  // NOTE: not exhaustive but simple and possibly sufficient
  // inversion of https://github.com/rust-lang/rust/blob/dca3f1b786efd27be3b325ed1e01e247aa589c3b/library/std/src/sys/unix/mod.rs#L138-L185
  // TODO(@AaronO): maybe revisit this and reuse libuv's mappings
  // TODO(@AaronO): update when `io_error_more` is stabilized (https://github.com/rust-lang/rust/issues/86442)
  use std::io::ErrorKind;
  match err.kind() {
    // ErrorKind::ArgumentListTooLong => "E2BIG",
    ErrorKind::AddrInUse => "EADDRINUSE",
    ErrorKind::AddrNotAvailable => "EADDRNOTAVAIL",
    // ErrorKind::ResourceBusy => "EBUSY",
    ErrorKind::ConnectionAborted => "ECONNABORTED",
    ErrorKind::ConnectionRefused => "ECONNREFUSED",
    ErrorKind::ConnectionReset => "ECONNRESET",
    // ErrorKind::Deadlock => "EDEADLK",
    // ErrorKind::FilesystemQuotaExceeded => "EDQUOT",
    ErrorKind::AlreadyExists => "EEXIST",
    // ErrorKind::FileTooLarge => "EFBIG",
    // ErrorKind::HostUnreachable => "EHOSTUNREACH",
    ErrorKind::Interrupted => "EINTR",
    ErrorKind::InvalidInput => "EINVAL",
    // ErrorKind::IsADirectory => "EISDIR",
    // ErrorKind::FilesystemLoop => "ELOOP",
    ErrorKind::NotFound => "ENOENT",
    ErrorKind::OutOfMemory => "ENOMEM",
    // ErrorKind::StorageFull => "ENOSPC",
    ErrorKind::Unsupported => "ENOSYS",
    // ErrorKind::TooManyLinks => "EMLINK",
    // ErrorKind::FilenameTooLong => "ENAMETOOLONG",
    // ErrorKind::NetworkDown => "ENETDOWN",
    // ErrorKind::NetworkUnreachable => "ENETUNREACH",
    ErrorKind::NotConnected => "ENOTCONN",
    // ErrorKind::NotADirectory => "ENOTDIR",
    // ErrorKind::DirectoryNotEmpty => "ENOTEMPTY",
    ErrorKind::BrokenPipe => "EPIPE",
    // ErrorKind::ReadOnlyFilesystem => "EROFS",
    // ErrorKind::NotSeekable => "ESPIPE",
    // ErrorKind::StaleNetworkFileHandle => "ESTALE",
    ErrorKind::TimedOut => "ETIMEDOUT",
    // ErrorKind::ExecutableFileBusy => "ETXTBSY",
    // ErrorKind::CrossesDevices => "EXDEV",
    ErrorKind::PermissionDenied => "EACCES", // NOTE: Collides with EPERM ...
    _ => "",
  }
}
