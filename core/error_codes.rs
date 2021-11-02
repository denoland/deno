use crate::error::AnyError;

pub(crate) fn get_error_code(err: &AnyError) -> Option<&'static str> {
  err
    .downcast_ref::<std::io::Error>()
    .map(|e| match e.raw_os_error() {
      Some(code) => get_os_error_code(code),
      None => get_io_error_code(e),
    })
    .and_then(|code| match code.is_empty() {
      true => None,
      false => Some(code),
    })
}

fn get_io_error_code(err: &std::io::Error) -> &'static str {
  // not exhaustive but simple and possibly sufficient once `io_error_more` is stabilized (https://github.com/rust-lang/rust/issues/86442)
  // inversion of https://github.com/rust-lang/rust/blob/dca3f1b786efd27be3b325ed1e01e247aa589c3b/library/std/src/sys/unix/mod.rs#L138-L185
  // TODO(@AaronO): revisit as `io_error_more` lands in rust stable
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

/// Maps OS errno codes to string names
/// derived from libuv: https://github.com/libuv/libuv/blob/26b2e5dbb6301756644d6e4cf6ca9c49c00513d3/include/uv/errno.h
#[cfg(unix)]
fn get_os_error_code(errno: i32) -> &'static str {
  match errno {
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
    // libc::ECHARSET => "ECHARSET",
    libc::ECONNABORTED => "ECONNABORTED",
    libc::ECONNREFUSED => "ECONNREFUSED",
    libc::ECONNRESET => "ECONNRESET",
    libc::EDESTADDRREQ => "EDESTADDRREQ",
    libc::EEXIST => "EEXIST",
    libc::EFAULT => "EFAULT",
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
    // libc::ENONET => "ENONET",
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
    libc::EROFS => "EROFS",
    libc::ESHUTDOWN => "ESHUTDOWN",
    libc::ESPIPE => "ESPIPE",
    libc::ESRCH => "ESRCH",
    libc::ETIMEDOUT => "ETIMEDOUT",
    libc::ETXTBSY => "ETXTBSY",
    libc::EXDEV => "EXDEV",
    libc::EFBIG => "EFBIG",
    libc::ENOPROTOOPT => "ENOPROTOOPT",
    libc::ERANGE => "ERANGE",
    libc::ENXIO => "ENXIO",
    libc::EMLINK => "EMLINK",
    libc::EHOSTDOWN => "EHOSTDOWN",
    libc::EREMOTE => "EREMOTE", // Changed from EREMOTEIO
    libc::ENOTTY => "ENOTTY",
    libc::EFTYPE => "EFTYPE",
    libc::EILSEQ => "EILSEQ",
    libc::EOVERFLOW => "EOVERFLOW",
    libc::ESOCKTNOSUPPORT => "ESOCKTNOSUPPORT",
    _ => "",
  }
}
