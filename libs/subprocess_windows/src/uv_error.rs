// Copyright 2018-2025 the Deno authors. MIT license.

// ported straight from libuv

use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
use windows_sys::Win32::Foundation::ERROR_ADDRESS_ALREADY_ASSOCIATED;
use windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS;
use windows_sys::Win32::Foundation::ERROR_BAD_EXE_FORMAT;
use windows_sys::Win32::Foundation::ERROR_BAD_PATHNAME;
use windows_sys::Win32::Foundation::ERROR_BAD_PIPE;
use windows_sys::Win32::Foundation::ERROR_BEGINNING_OF_MEDIA;
use windows_sys::Win32::Foundation::ERROR_BROKEN_PIPE;
use windows_sys::Win32::Foundation::ERROR_BUFFER_OVERFLOW;
use windows_sys::Win32::Foundation::ERROR_BUS_RESET;
use windows_sys::Win32::Foundation::ERROR_CANNOT_MAKE;
use windows_sys::Win32::Foundation::ERROR_CANT_ACCESS_FILE;
use windows_sys::Win32::Foundation::ERROR_CANT_RESOLVE_FILENAME;
use windows_sys::Win32::Foundation::ERROR_CONNECTION_ABORTED;
use windows_sys::Win32::Foundation::ERROR_CONNECTION_REFUSED;
use windows_sys::Win32::Foundation::ERROR_CRC;
use windows_sys::Win32::Foundation::ERROR_DEVICE_DOOR_OPEN;
use windows_sys::Win32::Foundation::ERROR_DEVICE_REQUIRES_CLEANING;
use windows_sys::Win32::Foundation::ERROR_DIR_NOT_EMPTY;
use windows_sys::Win32::Foundation::ERROR_DIRECTORY;
use windows_sys::Win32::Foundation::ERROR_DISK_CORRUPT;
use windows_sys::Win32::Foundation::ERROR_DISK_FULL;
use windows_sys::Win32::Foundation::ERROR_EA_TABLE_FULL;
use windows_sys::Win32::Foundation::ERROR_ELEVATION_REQUIRED;
use windows_sys::Win32::Foundation::ERROR_END_OF_MEDIA;
use windows_sys::Win32::Foundation::ERROR_ENVVAR_NOT_FOUND;
use windows_sys::Win32::Foundation::ERROR_EOM_OVERFLOW;
use windows_sys::Win32::Foundation::ERROR_FILE_EXISTS;
use windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows_sys::Win32::Foundation::ERROR_FILEMARK_DETECTED;
use windows_sys::Win32::Foundation::ERROR_FILENAME_EXCED_RANGE;
use windows_sys::Win32::Foundation::ERROR_GEN_FAILURE;
use windows_sys::Win32::Foundation::ERROR_HANDLE_DISK_FULL;
use windows_sys::Win32::Foundation::ERROR_HOST_UNREACHABLE;
use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows_sys::Win32::Foundation::ERROR_INVALID_BLOCK_LENGTH;
use windows_sys::Win32::Foundation::ERROR_INVALID_DATA;
use windows_sys::Win32::Foundation::ERROR_INVALID_DRIVE;
use windows_sys::Win32::Foundation::ERROR_INVALID_FLAGS;
use windows_sys::Win32::Foundation::ERROR_INVALID_FUNCTION;
use windows_sys::Win32::Foundation::ERROR_INVALID_HANDLE;
use windows_sys::Win32::Foundation::ERROR_INVALID_NAME;
use windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER;
use windows_sys::Win32::Foundation::ERROR_INVALID_REPARSE_DATA;
use windows_sys::Win32::Foundation::ERROR_IO_DEVICE;
use windows_sys::Win32::Foundation::ERROR_LOCK_VIOLATION;
use windows_sys::Win32::Foundation::ERROR_META_EXPANSION_TOO_LONG;
use windows_sys::Win32::Foundation::ERROR_MOD_NOT_FOUND;
use windows_sys::Win32::Foundation::ERROR_NETNAME_DELETED;
use windows_sys::Win32::Foundation::ERROR_NETWORK_UNREACHABLE;
use windows_sys::Win32::Foundation::ERROR_NO_DATA;
use windows_sys::Win32::Foundation::ERROR_NO_DATA_DETECTED;
use windows_sys::Win32::Foundation::ERROR_NO_SIGNAL_SENT;
use windows_sys::Win32::Foundation::ERROR_NO_UNICODE_TRANSLATION;
use windows_sys::Win32::Foundation::ERROR_NOACCESS;
use windows_sys::Win32::Foundation::ERROR_NOT_CONNECTED;
use windows_sys::Win32::Foundation::ERROR_NOT_ENOUGH_MEMORY;
use windows_sys::Win32::Foundation::ERROR_NOT_SAME_DEVICE;
use windows_sys::Win32::Foundation::ERROR_NOT_SUPPORTED;
use windows_sys::Win32::Foundation::ERROR_OPEN_FAILED;
use windows_sys::Win32::Foundation::ERROR_OPERATION_ABORTED;
use windows_sys::Win32::Foundation::ERROR_OUTOFMEMORY;
use windows_sys::Win32::Foundation::ERROR_PATH_NOT_FOUND;
use windows_sys::Win32::Foundation::ERROR_PIPE_BUSY;
use windows_sys::Win32::Foundation::ERROR_PIPE_NOT_CONNECTED;
use windows_sys::Win32::Foundation::ERROR_PRIVILEGE_NOT_HELD;
use windows_sys::Win32::Foundation::ERROR_SEM_TIMEOUT;
use windows_sys::Win32::Foundation::ERROR_SETMARK_DETECTED;
use windows_sys::Win32::Foundation::ERROR_SHARING_VIOLATION;
use windows_sys::Win32::Foundation::ERROR_SIGNAL_REFUSED;
use windows_sys::Win32::Foundation::ERROR_SYMLINK_NOT_SUPPORTED;
use windows_sys::Win32::Foundation::ERROR_TOO_MANY_OPEN_FILES;
use windows_sys::Win32::Foundation::ERROR_WRITE_PROTECT;

pub const UV_E2BIG: i32 = -7;
pub const UV_EACCES: i32 = -13;
pub const UV_EADDRINUSE: i32 = -98;
pub const UV_EAGAIN: i32 = -11;
pub const UV_EBADF: i32 = -9;
pub const UV_EBUSY: i32 = -16;
pub const UV_ECANCELED: i32 = -125;
pub const UV_ECHARSET: i32 = -4080;
pub const UV_ECONNABORTED: i32 = -103;
pub const UV_ECONNREFUSED: i32 = -111;
pub const UV_ECONNRESET: i32 = -104;
pub const UV_EEXIST: i32 = -17;
pub const UV_EFAULT: i32 = -14;
pub const UV_EHOSTUNREACH: i32 = -113;
pub const UV_EINVAL: i32 = -22;
pub const UV_EIO: i32 = -5;
pub const UV_EISDIR: i32 = -21;
pub const UV_ELOOP: i32 = -40;
pub const UV_EMFILE: i32 = -24;
pub const UV_ENAMETOOLONG: i32 = -36;
pub const UV_ENETUNREACH: i32 = -101;
pub const UV_ENOENT: i32 = -2;
pub const UV_ENOMEM: i32 = -12;
pub const UV_ENOSPC: i32 = -28;
pub const UV_ENOTCONN: i32 = -107;
pub const UV_ENOTEMPTY: i32 = -39;
pub const UV_ENOTSUP: i32 = -95;
pub const UV_EOF: i32 = -4095;
pub const UV_EPERM: i32 = -1;
pub const UV_EPIPE: i32 = -32;
pub const UV_EROFS: i32 = -30;
pub const UV_ETIMEDOUT: i32 = -110;
pub const UV_EXDEV: i32 = -18;
pub const UV_EFTYPE: i32 = -4028;
pub const UV_UNKNOWN: i32 = -4094;
pub const UV_ESRCH: i32 = -4040;

pub fn uv_translate_sys_error(sys_errno: u32) -> i32 {
  match sys_errno {
    ERROR_ELEVATION_REQUIRED => UV_EACCES,
    ERROR_CANT_ACCESS_FILE => UV_EACCES,
    ERROR_ADDRESS_ALREADY_ASSOCIATED => UV_EADDRINUSE,
    ERROR_NO_DATA => UV_EAGAIN,
    ERROR_INVALID_FLAGS => UV_EBADF,
    ERROR_INVALID_HANDLE => UV_EBADF,
    ERROR_LOCK_VIOLATION => UV_EBUSY,
    ERROR_PIPE_BUSY => UV_EBUSY,
    ERROR_SHARING_VIOLATION => UV_EBUSY,
    ERROR_OPERATION_ABORTED => UV_ECANCELED,
    ERROR_NO_UNICODE_TRANSLATION => UV_ECHARSET,
    ERROR_CONNECTION_ABORTED => UV_ECONNABORTED,
    ERROR_CONNECTION_REFUSED => UV_ECONNREFUSED,
    ERROR_NETNAME_DELETED => UV_ECONNRESET,
    ERROR_ALREADY_EXISTS => UV_EEXIST,
    ERROR_FILE_EXISTS => UV_EEXIST,
    ERROR_NOACCESS => UV_EFAULT,
    ERROR_HOST_UNREACHABLE => UV_EHOSTUNREACH,
    ERROR_INSUFFICIENT_BUFFER => UV_EINVAL,
    ERROR_INVALID_DATA => UV_EINVAL,
    ERROR_INVALID_PARAMETER => UV_EINVAL,
    ERROR_SYMLINK_NOT_SUPPORTED => UV_EINVAL,
    ERROR_BEGINNING_OF_MEDIA => UV_EIO,
    ERROR_BUS_RESET => UV_EIO,
    ERROR_CRC => UV_EIO,
    ERROR_DEVICE_DOOR_OPEN => UV_EIO,
    ERROR_DEVICE_REQUIRES_CLEANING => UV_EIO,
    ERROR_DISK_CORRUPT => UV_EIO,
    ERROR_EOM_OVERFLOW => UV_EIO,
    ERROR_FILEMARK_DETECTED => UV_EIO,
    ERROR_GEN_FAILURE => UV_EIO,
    ERROR_INVALID_BLOCK_LENGTH => UV_EIO,
    ERROR_IO_DEVICE => UV_EIO,
    ERROR_NO_DATA_DETECTED => UV_EIO,
    ERROR_NO_SIGNAL_SENT => UV_EIO,
    ERROR_OPEN_FAILED => UV_EIO,
    ERROR_SETMARK_DETECTED => UV_EIO,
    ERROR_SIGNAL_REFUSED => UV_EIO,
    ERROR_CANT_RESOLVE_FILENAME => UV_ELOOP,
    ERROR_TOO_MANY_OPEN_FILES => UV_EMFILE,
    ERROR_BUFFER_OVERFLOW => UV_ENAMETOOLONG,
    ERROR_FILENAME_EXCED_RANGE => UV_ENAMETOOLONG,
    ERROR_NETWORK_UNREACHABLE => UV_ENETUNREACH,
    ERROR_BAD_PATHNAME => UV_ENOENT,
    ERROR_DIRECTORY => UV_ENOENT,
    ERROR_ENVVAR_NOT_FOUND => UV_ENOENT,
    ERROR_FILE_NOT_FOUND => UV_ENOENT,
    ERROR_INVALID_NAME => UV_ENOENT,
    ERROR_INVALID_DRIVE => UV_ENOENT,
    ERROR_INVALID_REPARSE_DATA => UV_ENOENT,
    ERROR_MOD_NOT_FOUND => UV_ENOENT,
    ERROR_PATH_NOT_FOUND => UV_ENOENT,
    ERROR_NOT_ENOUGH_MEMORY => UV_ENOMEM,
    ERROR_OUTOFMEMORY => UV_ENOMEM,
    ERROR_CANNOT_MAKE => UV_ENOSPC,
    ERROR_DISK_FULL => UV_ENOSPC,
    ERROR_EA_TABLE_FULL => UV_ENOSPC,
    ERROR_END_OF_MEDIA => UV_ENOSPC,
    ERROR_HANDLE_DISK_FULL => UV_ENOSPC,
    ERROR_NOT_CONNECTED => UV_ENOTCONN,
    ERROR_DIR_NOT_EMPTY => UV_ENOTEMPTY,
    ERROR_NOT_SUPPORTED => UV_ENOTSUP,
    ERROR_BROKEN_PIPE => UV_EOF,
    ERROR_ACCESS_DENIED => UV_EPERM,
    ERROR_PRIVILEGE_NOT_HELD => UV_EPERM,
    ERROR_BAD_PIPE => UV_EPIPE,
    ERROR_PIPE_NOT_CONNECTED => UV_EPIPE,
    ERROR_WRITE_PROTECT => UV_EROFS,
    ERROR_SEM_TIMEOUT => UV_ETIMEDOUT,
    ERROR_NOT_SAME_DEVICE => UV_EXDEV,
    ERROR_INVALID_FUNCTION => UV_EISDIR,
    ERROR_META_EXPANSION_TOO_LONG => UV_E2BIG,
    ERROR_BAD_EXE_FORMAT => UV_EFTYPE,
    _ => UV_UNKNOWN,
  }
}
