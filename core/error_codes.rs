// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use anyhow::Error;

pub fn get_error_code(err: &Error) -> Option<&'static str> {
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
/// generated with tools/codegen_error_codes.js
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
    libc::ECONNABORTED => "ECONNABORTED",
    libc::ECONNREFUSED => "ECONNREFUSED",
    libc::ECONNRESET => "ECONNRESET",
    libc::EEXIST => "EEXIST",
    libc::EFAULT => "EFAULT",
    libc::EHOSTUNREACH => "EHOSTUNREACH",
    libc::EINVAL => "EINVAL",
    libc::EIO => "EIO",
    libc::EISCONN => "EISCONN",
    libc::EISDIR => "EISDIR",
    libc::ELOOP => "ELOOP",
    libc::EMFILE => "EMFILE",
    libc::EMSGSIZE => "EMSGSIZE",
    libc::ENAMETOOLONG => "ENAMETOOLONG",
    libc::ENETUNREACH => "ENETUNREACH",
    libc::ENOBUFS => "ENOBUFS",
    libc::ENOENT => "ENOENT",
    libc::ENOMEM => "ENOMEM",
    libc::ENOSPC => "ENOSPC",
    libc::ENOTCONN => "ENOTCONN",
    libc::ENOTDIR => "ENOTDIR",
    libc::ENOTEMPTY => "ENOTEMPTY",
    libc::ENOTSOCK => "ENOTSOCK",
    libc::ENOTSUP => "ENOTSUP",
    libc::EPERM => "EPERM",
    libc::EPIPE => "EPIPE",
    libc::EPROTONOSUPPORT => "EPROTONOSUPPORT",
    libc::EROFS => "EROFS",
    libc::ETIMEDOUT => "ETIMEDOUT",
    libc::EXDEV => "EXDEV",
    libc::ESOCKTNOSUPPORT => "ESOCKTNOSUPPORT",
    _ => "",
  }
}

#[cfg(windows)]
fn get_os_error_code(errno: i32) -> &'static str {
  match errno {
    998 => "EACCES",            // ERROR_NOACCESS
    10013 => "EACCES",          // WSAEACCES
    1920 => "EACCES",           // ERROR_CANT_ACCESS_FILE
    1227 => "EADDRINUSE",       // ERROR_ADDRESS_ALREADY_ASSOCIATED
    10048 => "EADDRINUSE",      // WSAEADDRINUSE
    10049 => "EADDRNOTAVAIL",   // WSAEADDRNOTAVAIL
    10047 => "EAFNOSUPPORT",    // WSAEAFNOSUPPORT
    10035 => "EAGAIN",          // WSAEWOULDBLOCK
    10037 => "EALREADY",        // WSAEALREADY
    1004 => "EBADF",            // ERROR_INVALID_FLAGS
    6 => "EBADF",               // ERROR_INVALID_HANDLE
    33 => "EBUSY",              // ERROR_LOCK_VIOLATION
    231 => "EBUSY",             // ERROR_PIPE_BUSY
    32 => "EBUSY",              // ERROR_SHARING_VIOLATION
    995 => "ECANCELED",         // ERROR_OPERATION_ABORTED
    10004 => "ECANCELED",       // WSAEINTR
    1236 => "ECONNABORTED",     // ERROR_CONNECTION_ABORTED
    10053 => "ECONNABORTED",    // WSAECONNABORTED
    1225 => "ECONNREFUSED",     // ERROR_CONNECTION_REFUSED
    10061 => "ECONNREFUSED",    // WSAECONNREFUSED
    64 => "ECONNRESET",         // ERROR_NETNAME_DELETED
    10054 => "ECONNRESET",      // WSAECONNRESET
    183 => "EEXIST",            // ERROR_ALREADY_EXISTS
    80 => "EEXIST",             // ERROR_FILE_EXISTS
    111 => "EFAULT",            // ERROR_BUFFER_OVERFLOW
    10014 => "EFAULT",          // WSAEFAULT
    1232 => "EHOSTUNREACH",     // ERROR_HOST_UNREACHABLE
    10065 => "EHOSTUNREACH",    // WSAEHOSTUNREACH
    122 => "EINVAL",            // ERROR_INSUFFICIENT_BUFFER
    13 => "EINVAL",             // ERROR_INVALID_DATA
    87 => "EINVAL",             // ERROR_INVALID_PARAMETER
    1464 => "EINVAL",           // ERROR_SYMLINK_NOT_SUPPORTED
    10022 => "EINVAL",          // WSAEINVAL
    10046 => "EINVAL",          // WSAEPFNOSUPPORT
    1102 => "EIO",              // ERROR_BEGINNING_OF_MEDIA
    1111 => "EIO",              // ERROR_BUS_RESET
    23 => "EIO",                // ERROR_CRC
    1166 => "EIO",              // ERROR_DEVICE_DOOR_OPEN
    1165 => "EIO",              // ERROR_DEVICE_REQUIRES_CLEANING
    1393 => "EIO",              // ERROR_DISK_CORRUPT
    1129 => "EIO",              // ERROR_EOM_OVERFLOW
    1101 => "EIO",              // ERROR_FILEMARK_DETECTED
    31 => "EIO",                // ERROR_GEN_FAILURE
    1106 => "EIO",              // ERROR_INVALID_BLOCK_LENGTH
    1117 => "EIO",              // ERROR_IO_DEVICE
    1104 => "EIO",              // ERROR_NO_DATA_DETECTED
    205 => "EIO",               // ERROR_NO_SIGNAL_SENT
    110 => "EIO",               // ERROR_OPEN_FAILED
    1103 => "EIO",              // ERROR_SETMARK_DETECTED
    156 => "EIO",               // ERROR_SIGNAL_REFUSED
    10056 => "EISCONN",         // WSAEISCONN
    1921 => "ELOOP",            // ERROR_CANT_RESOLVE_FILENAME
    4 => "EMFILE",              // ERROR_TOO_MANY_OPEN_FILES
    10024 => "EMFILE",          // WSAEMFILE
    10040 => "EMSGSIZE",        // WSAEMSGSIZE
    206 => "ENAMETOOLONG",      // ERROR_FILENAME_EXCED_RANGE
    1231 => "ENETUNREACH",      // ERROR_NETWORK_UNREACHABLE
    10051 => "ENETUNREACH",     // WSAENETUNREACH
    10055 => "ENOBUFS",         // WSAENOBUFS
    161 => "ENOENT",            // ERROR_BAD_PATHNAME
    267 => "ENOENT",            // ERROR_DIRECTORY
    203 => "ENOENT",            // ERROR_ENVVAR_NOT_FOUND
    2 => "ENOENT",              // ERROR_FILE_NOT_FOUND
    123 => "ENOENT",            // ERROR_INVALID_NAME
    15 => "ENOENT",             // ERROR_INVALID_DRIVE
    4392 => "ENOENT",           // ERROR_INVALID_REPARSE_DATA
    126 => "ENOENT",            // ERROR_MOD_NOT_FOUND
    3 => "ENOENT",              // ERROR_PATH_NOT_FOUND
    11001 => "ENOENT",          // WSAHOST_NOT_FOUND
    11004 => "ENOENT",          // WSANO_DATA
    8 => "ENOMEM",              // ERROR_NOT_ENOUGH_MEMORY
    14 => "ENOMEM",             // ERROR_OUTOFMEMORY
    82 => "ENOSPC",             // ERROR_CANNOT_MAKE
    112 => "ENOSPC",            // ERROR_DISK_FULL
    277 => "ENOSPC",            // ERROR_EA_TABLE_FULL
    1100 => "ENOSPC",           // ERROR_END_OF_MEDIA
    39 => "ENOSPC",             // ERROR_HANDLE_DISK_FULL
    2250 => "ENOTCONN",         // ERROR_NOT_CONNECTED
    10057 => "ENOTCONN",        // WSAENOTCONN
    145 => "ENOTEMPTY",         // ERROR_DIR_NOT_EMPTY
    10038 => "ENOTSOCK",        // WSAENOTSOCK
    50 => "ENOTSUP",            // ERROR_NOT_SUPPORTED
    5 => "EPERM",               // ERROR_ACCESS_DENIED
    1314 => "EPERM",            // ERROR_PRIVILEGE_NOT_HELD
    230 => "EPIPE",             // ERROR_BAD_PIPE
    232 => "EPIPE",             // ERROR_NO_DATA
    233 => "EPIPE",             // ERROR_PIPE_NOT_CONNECTED
    10058 => "EPIPE",           // WSAESHUTDOWN
    10043 => "EPROTONOSUPPORT", // WSAEPROTONOSUPPORT
    19 => "EROFS",              // ERROR_WRITE_PROTECT
    121 => "ETIMEDOUT",         // ERROR_SEM_TIMEOUT
    10060 => "ETIMEDOUT",       // WSAETIMEDOUT
    17 => "EXDEV",              // ERROR_NOT_SAME_DEVICE
    1 => "EISDIR",              // ERROR_INVALID_FUNCTION
    208 => "E2BIG",             // ERROR_META_EXPANSION_TOO_LONG
    10044 => "ESOCKTNOSUPPORT", // WSAESOCKTNOSUPPORT
    _ => "",
  }
}
