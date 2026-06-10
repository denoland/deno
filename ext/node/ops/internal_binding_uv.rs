// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::op2;
use deno_core::v8;

use crate::ops::winerror::node_sys_to_uv_error;

struct UvError {
  code: i32,
  name: &'static str,
  message: &'static str,
}

const CODE_TO_ERROR_WINDOWS: &[UvError] = &[
  UvError {
    code: -4093,
    name: "E2BIG",
    message: "argument list too long",
  },
  UvError {
    code: -4092,
    name: "EACCES",
    message: "permission denied",
  },
  UvError {
    code: -4091,
    name: "EADDRINUSE",
    message: "address already in use",
  },
  UvError {
    code: -4090,
    name: "EADDRNOTAVAIL",
    message: "address not available",
  },
  UvError {
    code: -4089,
    name: "EAFNOSUPPORT",
    message: "address family not supported",
  },
  UvError {
    code: -4088,
    name: "EAGAIN",
    message: "resource temporarily unavailable",
  },
  UvError {
    code: -3000,
    name: "EAI_ADDRFAMILY",
    message: "address family not supported",
  },
  UvError {
    code: -3001,
    name: "EAI_AGAIN",
    message: "temporary failure",
  },
  UvError {
    code: -3002,
    name: "EAI_BADFLAGS",
    message: "bad ai_flags value",
  },
  UvError {
    code: -3013,
    name: "EAI_BADHINTS",
    message: "invalid value for hints",
  },
  UvError {
    code: -3003,
    name: "EAI_CANCELED",
    message: "request canceled",
  },
  UvError {
    code: -3004,
    name: "EAI_FAIL",
    message: "permanent failure",
  },
  UvError {
    code: -3005,
    name: "EAI_FAMILY",
    message: "ai_family not supported",
  },
  UvError {
    code: -3006,
    name: "EAI_MEMORY",
    message: "out of memory",
  },
  UvError {
    code: -3007,
    name: "EAI_NODATA",
    message: "no address",
  },
  UvError {
    code: -3008,
    name: "EAI_NONAME",
    message: "unknown node or service",
  },
  UvError {
    code: -3009,
    name: "EAI_OVERFLOW",
    message: "argument buffer overflow",
  },
  UvError {
    code: -3014,
    name: "EAI_PROTOCOL",
    message: "resolved protocol is unknown",
  },
  UvError {
    code: -3010,
    name: "EAI_SERVICE",
    message: "service not available for socket type",
  },
  UvError {
    code: -3011,
    name: "EAI_SOCKTYPE",
    message: "socket type not supported",
  },
  UvError {
    code: -4084,
    name: "EALREADY",
    message: "connection already in progress",
  },
  UvError {
    code: -4083,
    name: "EBADF",
    message: "bad file descriptor",
  },
  UvError {
    code: -4082,
    name: "EBUSY",
    message: "resource busy or locked",
  },
  UvError {
    code: -4081,
    name: "ECANCELED",
    message: "operation canceled",
  },
  UvError {
    code: -4080,
    name: "ECHARSET",
    message: "invalid Unicode character",
  },
  UvError {
    code: -4079,
    name: "ECONNABORTED",
    message: "software caused connection abort",
  },
  UvError {
    code: -4078,
    name: "ECONNREFUSED",
    message: "connection refused",
  },
  UvError {
    code: -4077,
    name: "ECONNRESET",
    message: "connection reset by peer",
  },
  UvError {
    code: -4076,
    name: "EDESTADDRREQ",
    message: "destination address required",
  },
  UvError {
    code: -4075,
    name: "EEXIST",
    message: "file already exists",
  },
  UvError {
    code: -4074,
    name: "EFAULT",
    message: "bad address in system call argument",
  },
  UvError {
    code: -4036,
    name: "EFBIG",
    message: "file too large",
  },
  UvError {
    code: -4073,
    name: "EHOSTUNREACH",
    message: "host is unreachable",
  },
  UvError {
    code: -4072,
    name: "EINTR",
    message: "interrupted system call",
  },
  UvError {
    code: -4071,
    name: "EINVAL",
    message: "invalid argument",
  },
  UvError {
    code: -4070,
    name: "EIO",
    message: "i/o error",
  },
  UvError {
    code: -4069,
    name: "EISCONN",
    message: "socket is already connected",
  },
  UvError {
    code: -4068,
    name: "EISDIR",
    message: "illegal operation on a directory",
  },
  UvError {
    code: -4067,
    name: "ELOOP",
    message: "too many symbolic links encountered",
  },
  UvError {
    code: -4066,
    name: "EMFILE",
    message: "too many open files",
  },
  UvError {
    code: -4065,
    name: "EMSGSIZE",
    message: "message too long",
  },
  UvError {
    code: -4064,
    name: "ENAMETOOLONG",
    message: "name too long",
  },
  UvError {
    code: -4063,
    name: "ENETDOWN",
    message: "network is down",
  },
  UvError {
    code: -4062,
    name: "ENETUNREACH",
    message: "network is unreachable",
  },
  UvError {
    code: -4061,
    name: "ENFILE",
    message: "file table overflow",
  },
  UvError {
    code: -4060,
    name: "ENOBUFS",
    message: "no buffer space available",
  },
  UvError {
    code: -4059,
    name: "ENODEV",
    message: "no such device",
  },
  UvError {
    code: -4058,
    name: "ENOENT",
    message: "no such file or directory",
  },
  UvError {
    code: -4057,
    name: "ENOMEM",
    message: "not enough memory",
  },
  UvError {
    code: -4056,
    name: "ENONET",
    message: "machine is not on the network",
  },
  UvError {
    code: -4035,
    name: "ENOPROTOOPT",
    message: "protocol not available",
  },
  UvError {
    code: -4055,
    name: "ENOSPC",
    message: "no space left on device",
  },
  UvError {
    code: -4054,
    name: "ENOSYS",
    message: "function not implemented",
  },
  UvError {
    code: -4053,
    name: "ENOTCONN",
    message: "socket is not connected",
  },
  UvError {
    code: -4052,
    name: "ENOTDIR",
    message: "not a directory",
  },
  UvError {
    code: -4051,
    name: "ENOTEMPTY",
    message: "directory not empty",
  },
  UvError {
    code: -4050,
    name: "ENOTSOCK",
    message: "socket operation on non-socket",
  },
  UvError {
    code: -4049,
    name: "ENOTSUP",
    message: "operation not supported on socket",
  },
  UvError {
    code: -4048,
    name: "EPERM",
    message: "operation not permitted",
  },
  UvError {
    code: -4047,
    name: "EPIPE",
    message: "broken pipe",
  },
  UvError {
    code: -4046,
    name: "EPROTO",
    message: "protocol error",
  },
  UvError {
    code: -4045,
    name: "EPROTONOSUPPORT",
    message: "protocol not supported",
  },
  UvError {
    code: -4044,
    name: "EPROTOTYPE",
    message: "protocol wrong type for socket",
  },
  UvError {
    code: -4034,
    name: "ERANGE",
    message: "result too large",
  },
  UvError {
    code: -4043,
    name: "EROFS",
    message: "read-only file system",
  },
  UvError {
    code: -4042,
    name: "ESHUTDOWN",
    message: "cannot send after transport endpoint shutdown",
  },
  UvError {
    code: -4041,
    name: "ESPIPE",
    message: "invalid seek",
  },
  UvError {
    code: -4040,
    name: "ESRCH",
    message: "no such process",
  },
  UvError {
    code: -4039,
    name: "ETIMEDOUT",
    message: "connection timed out",
  },
  UvError {
    code: -4038,
    name: "ETXTBSY",
    message: "text file is busy",
  },
  UvError {
    code: -4037,
    name: "EXDEV",
    message: "cross-device link not permitted",
  },
  UvError {
    code: -4094,
    name: "UNKNOWN",
    message: "unknown error",
  },
  UvError {
    code: -4095,
    name: "EOF",
    message: "end of file",
  },
  UvError {
    code: -4033,
    name: "ENXIO",
    message: "no such device or address",
  },
  UvError {
    code: -4032,
    name: "EMLINK",
    message: "too many links",
  },
  UvError {
    code: -4031,
    name: "EHOSTDOWN",
    message: "host is down",
  },
  UvError {
    code: -4030,
    name: "EREMOTEIO",
    message: "remote I/O error",
  },
  UvError {
    code: -4029,
    name: "ENOTTY",
    message: "inappropriate ioctl for device",
  },
  UvError {
    code: -4028,
    name: "EFTYPE",
    message: "inappropriate file type or format",
  },
  UvError {
    code: -4027,
    name: "EILSEQ",
    message: "illegal byte sequence",
  },
];
const CODE_TO_ERROR_DARWIN: &[UvError] = &[
  UvError {
    code: -7,
    name: "E2BIG",
    message: "argument list too long",
  },
  UvError {
    code: -13,
    name: "EACCES",
    message: "permission denied",
  },
  UvError {
    code: -48,
    name: "EADDRINUSE",
    message: "address already in use",
  },
  UvError {
    code: -49,
    name: "EADDRNOTAVAIL",
    message: "address not available",
  },
  UvError {
    code: -47,
    name: "EAFNOSUPPORT",
    message: "address family not supported",
  },
  UvError {
    code: -35,
    name: "EAGAIN",
    message: "resource temporarily unavailable",
  },
  UvError {
    code: -3000,
    name: "EAI_ADDRFAMILY",
    message: "address family not supported",
  },
  UvError {
    code: -3001,
    name: "EAI_AGAIN",
    message: "temporary failure",
  },
  UvError {
    code: -3002,
    name: "EAI_BADFLAGS",
    message: "bad ai_flags value",
  },
  UvError {
    code: -3013,
    name: "EAI_BADHINTS",
    message: "invalid value for hints",
  },
  UvError {
    code: -3003,
    name: "EAI_CANCELED",
    message: "request canceled",
  },
  UvError {
    code: -3004,
    name: "EAI_FAIL",
    message: "permanent failure",
  },
  UvError {
    code: -3005,
    name: "EAI_FAMILY",
    message: "ai_family not supported",
  },
  UvError {
    code: -3006,
    name: "EAI_MEMORY",
    message: "out of memory",
  },
  UvError {
    code: -3007,
    name: "EAI_NODATA",
    message: "no address",
  },
  UvError {
    code: -3008,
    name: "EAI_NONAME",
    message: "unknown node or service",
  },
  UvError {
    code: -3009,
    name: "EAI_OVERFLOW",
    message: "argument buffer overflow",
  },
  UvError {
    code: -3014,
    name: "EAI_PROTOCOL",
    message: "resolved protocol is unknown",
  },
  UvError {
    code: -3010,
    name: "EAI_SERVICE",
    message: "service not available for socket type",
  },
  UvError {
    code: -3011,
    name: "EAI_SOCKTYPE",
    message: "socket type not supported",
  },
  UvError {
    code: -37,
    name: "EALREADY",
    message: "connection already in progress",
  },
  UvError {
    code: -9,
    name: "EBADF",
    message: "bad file descriptor",
  },
  UvError {
    code: -16,
    name: "EBUSY",
    message: "resource busy or locked",
  },
  UvError {
    code: -89,
    name: "ECANCELED",
    message: "operation canceled",
  },
  UvError {
    code: -4080,
    name: "ECHARSET",
    message: "invalid Unicode character",
  },
  UvError {
    code: -53,
    name: "ECONNABORTED",
    message: "software caused connection abort",
  },
  UvError {
    code: -61,
    name: "ECONNREFUSED",
    message: "connection refused",
  },
  UvError {
    code: -54,
    name: "ECONNRESET",
    message: "connection reset by peer",
  },
  UvError {
    code: -39,
    name: "EDESTADDRREQ",
    message: "destination address required",
  },
  UvError {
    code: -17,
    name: "EEXIST",
    message: "file already exists",
  },
  UvError {
    code: -14,
    name: "EFAULT",
    message: "bad address in system call argument",
  },
  UvError {
    code: -27,
    name: "EFBIG",
    message: "file too large",
  },
  UvError {
    code: -65,
    name: "EHOSTUNREACH",
    message: "host is unreachable",
  },
  UvError {
    code: -4,
    name: "EINTR",
    message: "interrupted system call",
  },
  UvError {
    code: -22,
    name: "EINVAL",
    message: "invalid argument",
  },
  UvError {
    code: -5,
    name: "EIO",
    message: "i/o error",
  },
  UvError {
    code: -56,
    name: "EISCONN",
    message: "socket is already connected",
  },
  UvError {
    code: -21,
    name: "EISDIR",
    message: "illegal operation on a directory",
  },
  UvError {
    code: -62,
    name: "ELOOP",
    message: "too many symbolic links encountered",
  },
  UvError {
    code: -24,
    name: "EMFILE",
    message: "too many open files",
  },
  UvError {
    code: -40,
    name: "EMSGSIZE",
    message: "message too long",
  },
  UvError {
    code: -63,
    name: "ENAMETOOLONG",
    message: "name too long",
  },
  UvError {
    code: -50,
    name: "ENETDOWN",
    message: "network is down",
  },
  UvError {
    code: -51,
    name: "ENETUNREACH",
    message: "network is unreachable",
  },
  UvError {
    code: -23,
    name: "ENFILE",
    message: "file table overflow",
  },
  UvError {
    code: -55,
    name: "ENOBUFS",
    message: "no buffer space available",
  },
  UvError {
    code: -19,
    name: "ENODEV",
    message: "no such device",
  },
  UvError {
    code: -2,
    name: "ENOENT",
    message: "no such file or directory",
  },
  UvError {
    code: -12,
    name: "ENOMEM",
    message: "not enough memory",
  },
  UvError {
    code: -4056,
    name: "ENONET",
    message: "machine is not on the network",
  },
  UvError {
    code: -42,
    name: "ENOPROTOOPT",
    message: "protocol not available",
  },
  UvError {
    code: -28,
    name: "ENOSPC",
    message: "no space left on device",
  },
  UvError {
    code: -78,
    name: "ENOSYS",
    message: "function not implemented",
  },
  UvError {
    code: -57,
    name: "ENOTCONN",
    message: "socket is not connected",
  },
  UvError {
    code: -20,
    name: "ENOTDIR",
    message: "not a directory",
  },
  UvError {
    code: -66,
    name: "ENOTEMPTY",
    message: "directory not empty",
  },
  UvError {
    code: -38,
    name: "ENOTSOCK",
    message: "socket operation on non-socket",
  },
  UvError {
    code: -45,
    name: "ENOTSUP",
    message: "operation not supported on socket",
  },
  UvError {
    code: -1,
    name: "EPERM",
    message: "operation not permitted",
  },
  UvError {
    code: -32,
    name: "EPIPE",
    message: "broken pipe",
  },
  UvError {
    code: -100,
    name: "EPROTO",
    message: "protocol error",
  },
  UvError {
    code: -43,
    name: "EPROTONOSUPPORT",
    message: "protocol not supported",
  },
  UvError {
    code: -41,
    name: "EPROTOTYPE",
    message: "protocol wrong type for socket",
  },
  UvError {
    code: -34,
    name: "ERANGE",
    message: "result too large",
  },
  UvError {
    code: -30,
    name: "EROFS",
    message: "read-only file system",
  },
  UvError {
    code: -58,
    name: "ESHUTDOWN",
    message: "cannot send after transport endpoint shutdown",
  },
  UvError {
    code: -29,
    name: "ESPIPE",
    message: "invalid seek",
  },
  UvError {
    code: -3,
    name: "ESRCH",
    message: "no such process",
  },
  UvError {
    code: -60,
    name: "ETIMEDOUT",
    message: "connection timed out",
  },
  UvError {
    code: -26,
    name: "ETXTBSY",
    message: "text file is busy",
  },
  UvError {
    code: -18,
    name: "EXDEV",
    message: "cross-device link not permitted",
  },
  UvError {
    code: -4094,
    name: "UNKNOWN",
    message: "unknown error",
  },
  UvError {
    code: -4095,
    name: "EOF",
    message: "end of file",
  },
  UvError {
    code: -6,
    name: "ENXIO",
    message: "no such device or address",
  },
  UvError {
    code: -31,
    name: "EMLINK",
    message: "too many links",
  },
  UvError {
    code: -64,
    name: "EHOSTDOWN",
    message: "host is down",
  },
  UvError {
    code: -4030,
    name: "EREMOTEIO",
    message: "remote I/O error",
  },
  UvError {
    code: -25,
    name: "ENOTTY",
    message: "inappropriate ioctl for device",
  },
  UvError {
    code: -79,
    name: "EFTYPE",
    message: "inappropriate file type or format",
  },
  UvError {
    code: -92,
    name: "EILSEQ",
    message: "illegal byte sequence",
  },
];
const CODE_TO_ERROR_LINUX: &[UvError] = &[
  UvError {
    code: -7,
    name: "E2BIG",
    message: "argument list too long",
  },
  UvError {
    code: -13,
    name: "EACCES",
    message: "permission denied",
  },
  UvError {
    code: -98,
    name: "EADDRINUSE",
    message: "address already in use",
  },
  UvError {
    code: -99,
    name: "EADDRNOTAVAIL",
    message: "address not available",
  },
  UvError {
    code: -97,
    name: "EAFNOSUPPORT",
    message: "address family not supported",
  },
  UvError {
    code: -11,
    name: "EAGAIN",
    message: "resource temporarily unavailable",
  },
  UvError {
    code: -3000,
    name: "EAI_ADDRFAMILY",
    message: "address family not supported",
  },
  UvError {
    code: -3001,
    name: "EAI_AGAIN",
    message: "temporary failure",
  },
  UvError {
    code: -3002,
    name: "EAI_BADFLAGS",
    message: "bad ai_flags value",
  },
  UvError {
    code: -3013,
    name: "EAI_BADHINTS",
    message: "invalid value for hints",
  },
  UvError {
    code: -3003,
    name: "EAI_CANCELED",
    message: "request canceled",
  },
  UvError {
    code: -3004,
    name: "EAI_FAIL",
    message: "permanent failure",
  },
  UvError {
    code: -3005,
    name: "EAI_FAMILY",
    message: "ai_family not supported",
  },
  UvError {
    code: -3006,
    name: "EAI_MEMORY",
    message: "out of memory",
  },
  UvError {
    code: -3007,
    name: "EAI_NODATA",
    message: "no address",
  },
  UvError {
    code: -3008,
    name: "EAI_NONAME",
    message: "unknown node or service",
  },
  UvError {
    code: -3009,
    name: "EAI_OVERFLOW",
    message: "argument buffer overflow",
  },
  UvError {
    code: -3014,
    name: "EAI_PROTOCOL",
    message: "resolved protocol is unknown",
  },
  UvError {
    code: -3010,
    name: "EAI_SERVICE",
    message: "service not available for socket type",
  },
  UvError {
    code: -3011,
    name: "EAI_SOCKTYPE",
    message: "socket type not supported",
  },
  UvError {
    code: -114,
    name: "EALREADY",
    message: "connection already in progress",
  },
  UvError {
    code: -9,
    name: "EBADF",
    message: "bad file descriptor",
  },
  UvError {
    code: -16,
    name: "EBUSY",
    message: "resource busy or locked",
  },
  UvError {
    code: -125,
    name: "ECANCELED",
    message: "operation canceled",
  },
  UvError {
    code: -4080,
    name: "ECHARSET",
    message: "invalid Unicode character",
  },
  UvError {
    code: -103,
    name: "ECONNABORTED",
    message: "software caused connection abort",
  },
  UvError {
    code: -111,
    name: "ECONNREFUSED",
    message: "connection refused",
  },
  UvError {
    code: -104,
    name: "ECONNRESET",
    message: "connection reset by peer",
  },
  UvError {
    code: -89,
    name: "EDESTADDRREQ",
    message: "destination address required",
  },
  UvError {
    code: -17,
    name: "EEXIST",
    message: "file already exists",
  },
  UvError {
    code: -14,
    name: "EFAULT",
    message: "bad address in system call argument",
  },
  UvError {
    code: -27,
    name: "EFBIG",
    message: "file too large",
  },
  UvError {
    code: -113,
    name: "EHOSTUNREACH",
    message: "host is unreachable",
  },
  UvError {
    code: -4,
    name: "EINTR",
    message: "interrupted system call",
  },
  UvError {
    code: -22,
    name: "EINVAL",
    message: "invalid argument",
  },
  UvError {
    code: -5,
    name: "EIO",
    message: "i/o error",
  },
  UvError {
    code: -106,
    name: "EISCONN",
    message: "socket is already connected",
  },
  UvError {
    code: -21,
    name: "EISDIR",
    message: "illegal operation on a directory",
  },
  UvError {
    code: -40,
    name: "ELOOP",
    message: "too many symbolic links encountered",
  },
  UvError {
    code: -24,
    name: "EMFILE",
    message: "too many open files",
  },
  UvError {
    code: -90,
    name: "EMSGSIZE",
    message: "message too long",
  },
  UvError {
    code: -36,
    name: "ENAMETOOLONG",
    message: "name too long",
  },
  UvError {
    code: -100,
    name: "ENETDOWN",
    message: "network is down",
  },
  UvError {
    code: -101,
    name: "ENETUNREACH",
    message: "network is unreachable",
  },
  UvError {
    code: -23,
    name: "ENFILE",
    message: "file table overflow",
  },
  UvError {
    code: -105,
    name: "ENOBUFS",
    message: "no buffer space available",
  },
  UvError {
    code: -19,
    name: "ENODEV",
    message: "no such device",
  },
  UvError {
    code: -2,
    name: "ENOENT",
    message: "no such file or directory",
  },
  UvError {
    code: -12,
    name: "ENOMEM",
    message: "not enough memory",
  },
  UvError {
    code: -64,
    name: "ENONET",
    message: "machine is not on the network",
  },
  UvError {
    code: -92,
    name: "ENOPROTOOPT",
    message: "protocol not available",
  },
  UvError {
    code: -28,
    name: "ENOSPC",
    message: "no space left on device",
  },
  UvError {
    code: -38,
    name: "ENOSYS",
    message: "function not implemented",
  },
  UvError {
    code: -107,
    name: "ENOTCONN",
    message: "socket is not connected",
  },
  UvError {
    code: -20,
    name: "ENOTDIR",
    message: "not a directory",
  },
  UvError {
    code: -39,
    name: "ENOTEMPTY",
    message: "directory not empty",
  },
  UvError {
    code: -88,
    name: "ENOTSOCK",
    message: "socket operation on non-socket",
  },
  UvError {
    code: -95,
    name: "ENOTSUP",
    message: "operation not supported on socket",
  },
  UvError {
    code: -1,
    name: "EPERM",
    message: "operation not permitted",
  },
  UvError {
    code: -32,
    name: "EPIPE",
    message: "broken pipe",
  },
  UvError {
    code: -71,
    name: "EPROTO",
    message: "protocol error",
  },
  UvError {
    code: -93,
    name: "EPROTONOSUPPORT",
    message: "protocol not supported",
  },
  UvError {
    code: -91,
    name: "EPROTOTYPE",
    message: "protocol wrong type for socket",
  },
  UvError {
    code: -34,
    name: "ERANGE",
    message: "result too large",
  },
  UvError {
    code: -30,
    name: "EROFS",
    message: "read-only file system",
  },
  UvError {
    code: -108,
    name: "ESHUTDOWN",
    message: "cannot send after transport endpoint shutdown",
  },
  UvError {
    code: -29,
    name: "ESPIPE",
    message: "invalid seek",
  },
  UvError {
    code: -3,
    name: "ESRCH",
    message: "no such process",
  },
  UvError {
    code: -110,
    name: "ETIMEDOUT",
    message: "connection timed out",
  },
  UvError {
    code: -26,
    name: "ETXTBSY",
    message: "text file is busy",
  },
  UvError {
    code: -18,
    name: "EXDEV",
    message: "cross-device link not permitted",
  },
  UvError {
    code: -4094,
    name: "UNKNOWN",
    message: "unknown error",
  },
  UvError {
    code: -4095,
    name: "EOF",
    message: "end of file",
  },
  UvError {
    code: -6,
    name: "ENXIO",
    message: "no such device or address",
  },
  UvError {
    code: -31,
    name: "EMLINK",
    message: "too many links",
  },
  UvError {
    code: -112,
    name: "EHOSTDOWN",
    message: "host is down",
  },
  UvError {
    code: -121,
    name: "EREMOTEIO",
    message: "remote I/O error",
  },
  UvError {
    code: -25,
    name: "ENOTTY",
    message: "inappropriate ioctl for device",
  },
  UvError {
    code: -4028,
    name: "EFTYPE",
    message: "inappropriate file type or format",
  },
  UvError {
    code: -84,
    name: "EILSEQ",
    message: "illegal byte sequence",
  },
];
const CODE_TO_ERROR_FREEBSD: &[UvError] = &[
  UvError {
    code: -7,
    name: "E2BIG",
    message: "argument list too long",
  },
  UvError {
    code: -13,
    name: "EACCES",
    message: "permission denied",
  },
  UvError {
    code: -48,
    name: "EADDRINUSE",
    message: "address already in use",
  },
  UvError {
    code: -49,
    name: "EADDRNOTAVAIL",
    message: "address not available",
  },
  UvError {
    code: -47,
    name: "EAFNOSUPPORT",
    message: "address family not supported",
  },
  UvError {
    code: -35,
    name: "EAGAIN",
    message: "resource temporarily unavailable",
  },
  UvError {
    code: -3000,
    name: "EAI_ADDRFAMILY",
    message: "address family not supported",
  },
  UvError {
    code: -3001,
    name: "EAI_AGAIN",
    message: "temporary failure",
  },
  UvError {
    code: -3002,
    name: "EAI_BADFLAGS",
    message: "bad ai_flags value",
  },
  UvError {
    code: -3013,
    name: "EAI_BADHINTS",
    message: "invalid value for hints",
  },
  UvError {
    code: -3003,
    name: "EAI_CANCELED",
    message: "request canceled",
  },
  UvError {
    code: -3004,
    name: "EAI_FAIL",
    message: "permanent failure",
  },
  UvError {
    code: -3005,
    name: "EAI_FAMILY",
    message: "ai_family not supported",
  },
  UvError {
    code: -3006,
    name: "EAI_MEMORY",
    message: "out of memory",
  },
  UvError {
    code: -3007,
    name: "EAI_NODATA",
    message: "no address",
  },
  UvError {
    code: -3008,
    name: "EAI_NONAME",
    message: "unknown node or service",
  },
  UvError {
    code: -3009,
    name: "EAI_OVERFLOW",
    message: "argument buffer overflow",
  },
  UvError {
    code: -3014,
    name: "EAI_PROTOCOL",
    message: "resolved protocol is unknown",
  },
  UvError {
    code: -3010,
    name: "EAI_SERVICE",
    message: "service not available for socket type",
  },
  UvError {
    code: -3011,
    name: "EAI_SOCKTYPE",
    message: "socket type not supported",
  },
  UvError {
    code: -37,
    name: "EALREADY",
    message: "connection already in progress",
  },
  UvError {
    code: -9,
    name: "EBADF",
    message: "bad file descriptor",
  },
  UvError {
    code: -16,
    name: "EBUSY",
    message: "resource busy or locked",
  },
  UvError {
    code: -85,
    name: "ECANCELED",
    message: "operation canceled",
  },
  UvError {
    code: -4080,
    name: "ECHARSET",
    message: "invalid Unicode character",
  },
  UvError {
    code: -53,
    name: "ECONNABORTED",
    message: "software caused connection abort",
  },
  UvError {
    code: -61,
    name: "ECONNREFUSED",
    message: "connection refused",
  },
  UvError {
    code: -54,
    name: "ECONNRESET",
    message: "connection reset by peer",
  },
  UvError {
    code: -39,
    name: "EDESTADDRREQ",
    message: "destination address required",
  },
  UvError {
    code: -17,
    name: "EEXIST",
    message: "file already exists",
  },
  UvError {
    code: -14,
    name: "EFAULT",
    message: "bad address in system call argument",
  },
  UvError {
    code: -27,
    name: "EFBIG",
    message: "file too large",
  },
  UvError {
    code: -65,
    name: "EHOSTUNREACH",
    message: "host is unreachable",
  },
  UvError {
    code: -4,
    name: "EINTR",
    message: "interrupted system call",
  },
  UvError {
    code: -22,
    name: "EINVAL",
    message: "invalid argument",
  },
  UvError {
    code: -5,
    name: "EIO",
    message: "i/o error",
  },
  UvError {
    code: -56,
    name: "EISCONN",
    message: "socket is already connected",
  },
  UvError {
    code: -21,
    name: "EISDIR",
    message: "illegal operation on a directory",
  },
  UvError {
    code: -62,
    name: "ELOOP",
    message: "too many symbolic links encountered",
  },
  UvError {
    code: -24,
    name: "EMFILE",
    message: "too many open files",
  },
  UvError {
    code: -40,
    name: "EMSGSIZE",
    message: "message too long",
  },
  UvError {
    code: -63,
    name: "ENAMETOOLONG",
    message: "name too long",
  },
  UvError {
    code: -50,
    name: "ENETDOWN",
    message: "network is down",
  },
  UvError {
    code: -51,
    name: "ENETUNREACH",
    message: "network is unreachable",
  },
  UvError {
    code: -23,
    name: "ENFILE",
    message: "file table overflow",
  },
  UvError {
    code: -55,
    name: "ENOBUFS",
    message: "no buffer space available",
  },
  UvError {
    code: -19,
    name: "ENODEV",
    message: "no such device",
  },
  UvError {
    code: -2,
    name: "ENOENT",
    message: "no such file or directory",
  },
  UvError {
    code: -12,
    name: "ENOMEM",
    message: "not enough memory",
  },
  UvError {
    code: -4056,
    name: "ENONET",
    message: "machine is not on the network",
  },
  UvError {
    code: -42,
    name: "ENOPROTOOPT",
    message: "protocol not available",
  },
  UvError {
    code: -28,
    name: "ENOSPC",
    message: "no space left on device",
  },
  UvError {
    code: -78,
    name: "ENOSYS",
    message: "function not implemented",
  },
  UvError {
    code: -57,
    name: "ENOTCONN",
    message: "socket is not connected",
  },
  UvError {
    code: -20,
    name: "ENOTDIR",
    message: "not a directory",
  },
  UvError {
    code: -66,
    name: "ENOTEMPTY",
    message: "directory not empty",
  },
  UvError {
    code: -38,
    name: "ENOTSOCK",
    message: "socket operation on non-socket",
  },
  UvError {
    code: -45,
    name: "ENOTSUP",
    message: "operation not supported on socket",
  },
  UvError {
    code: -84,
    name: "EOVERFLOW",
    message: "value too large for defined data type",
  },
  UvError {
    code: -1,
    name: "EPERM",
    message: "operation not permitted",
  },
  UvError {
    code: -32,
    name: "EPIPE",
    message: "broken pipe",
  },
  UvError {
    code: -92,
    name: "EPROTO",
    message: "protocol error",
  },
  UvError {
    code: -43,
    name: "EPROTONOSUPPORT",
    message: "protocol not supported",
  },
  UvError {
    code: -41,
    name: "EPROTOTYPE",
    message: "protocol wrong type for socket",
  },
  UvError {
    code: -34,
    name: "ERANGE",
    message: "result too large",
  },
  UvError {
    code: -30,
    name: "EROFS",
    message: "read-only file system",
  },
  UvError {
    code: -58,
    name: "ESHUTDOWN",
    message: "cannot send after transport endpoint shutdown",
  },
  UvError {
    code: -29,
    name: "ESPIPE",
    message: "invalid seek",
  },
  UvError {
    code: -3,
    name: "ESRCH",
    message: "no such process",
  },
  UvError {
    code: -60,
    name: "ETIMEDOUT",
    message: "connection timed out",
  },
  UvError {
    code: -26,
    name: "ETXTBSY",
    message: "text file is busy",
  },
  UvError {
    code: -18,
    name: "EXDEV",
    message: "cross-device link not permitted",
  },
  UvError {
    code: -4094,
    name: "UNKNOWN",
    message: "unknown error",
  },
  UvError {
    code: -4095,
    name: "EOF",
    message: "end of file",
  },
  UvError {
    code: -6,
    name: "ENXIO",
    message: "no such device or address",
  },
  UvError {
    code: -31,
    name: "EMLINK",
    message: "too many links",
  },
  UvError {
    code: -64,
    name: "EHOSTDOWN",
    message: "host is down",
  },
  UvError {
    code: -4030,
    name: "EREMOTEIO",
    message: "remote I/O error",
  },
  UvError {
    code: -25,
    name: "ENOTTY",
    message: "inappropriate ioctl for device",
  },
  UvError {
    code: -79,
    name: "EFTYPE",
    message: "inappropriate file type or format",
  },
  UvError {
    code: -86,
    name: "EILSEQ",
    message: "illegal byte sequence",
  },
  UvError {
    code: -44,
    name: "ESOCKTNOSUPPORT",
    message: "socket type not supported",
  },
];
const CODE_TO_ERROR_OPENBSD: &[UvError] = &[
  UvError {
    code: -7,
    name: "E2BIG",
    message: "argument list too long",
  },
  UvError {
    code: -13,
    name: "EACCES",
    message: "permission denied",
  },
  UvError {
    code: -48,
    name: "EADDRINUSE",
    message: "address already in use",
  },
  UvError {
    code: -49,
    name: "EADDRNOTAVAIL",
    message: "address not available",
  },
  UvError {
    code: -47,
    name: "EAFNOSUPPORT",
    message: "address family not supported",
  },
  UvError {
    code: -35,
    name: "EAGAIN",
    message: "resource temporarily unavailable",
  },
  UvError {
    code: -3000,
    name: "EAI_ADDRFAMILY",
    message: "address family not supported",
  },
  UvError {
    code: -3001,
    name: "EAI_AGAIN",
    message: "temporary failure",
  },
  UvError {
    code: -3002,
    name: "EAI_BADFLAGS",
    message: "bad ai_flags value",
  },
  UvError {
    code: -3013,
    name: "EAI_BADHINTS",
    message: "invalid value for hints",
  },
  UvError {
    code: -3003,
    name: "EAI_CANCELED",
    message: "request canceled",
  },
  UvError {
    code: -3004,
    name: "EAI_FAIL",
    message: "permanent failure",
  },
  UvError {
    code: -3005,
    name: "EAI_FAMILY",
    message: "ai_family not supported",
  },
  UvError {
    code: -3006,
    name: "EAI_MEMORY",
    message: "out of memory",
  },
  UvError {
    code: -3007,
    name: "EAI_NODATA",
    message: "no address",
  },
  UvError {
    code: -3008,
    name: "EAI_NONAME",
    message: "unknown node or service",
  },
  UvError {
    code: -3009,
    name: "EAI_OVERFLOW",
    message: "argument buffer overflow",
  },
  UvError {
    code: -3014,
    name: "EAI_PROTOCOL",
    message: "resolved protocol is unknown",
  },
  UvError {
    code: -3010,
    name: "EAI_SERVICE",
    message: "service not available for socket type",
  },
  UvError {
    code: -3011,
    name: "EAI_SOCKTYPE",
    message: "socket type not supported",
  },
  UvError {
    code: -37,
    name: "EALREADY",
    message: "connection already in progress",
  },
  UvError {
    code: -9,
    name: "EBADF",
    message: "bad file descriptor",
  },
  UvError {
    code: -16,
    name: "EBUSY",
    message: "resource busy or locked",
  },
  UvError {
    code: -88,
    name: "ECANCELED",
    message: "operation canceled",
  },
  UvError {
    code: -4080,
    name: "ECHARSET",
    message: "invalid Unicode character",
  },
  UvError {
    code: -53,
    name: "ECONNABORTED",
    message: "software caused connection abort",
  },
  UvError {
    code: -61,
    name: "ECONNREFUSED",
    message: "connection refused",
  },
  UvError {
    code: -54,
    name: "ECONNRESET",
    message: "connection reset by peer",
  },
  UvError {
    code: -39,
    name: "EDESTADDRREQ",
    message: "destination address required",
  },
  UvError {
    code: -17,
    name: "EEXIST",
    message: "file already exists",
  },
  UvError {
    code: -14,
    name: "EFAULT",
    message: "bad address in system call argument",
  },
  UvError {
    code: -27,
    name: "EFBIG",
    message: "file too large",
  },
  UvError {
    code: -65,
    name: "EHOSTUNREACH",
    message: "host is unreachable",
  },
  UvError {
    code: -4,
    name: "EINTR",
    message: "interrupted system call",
  },
  UvError {
    code: -22,
    name: "EINVAL",
    message: "invalid argument",
  },
  UvError {
    code: -5,
    name: "EIO",
    message: "i/o error",
  },
  UvError {
    code: -56,
    name: "EISCONN",
    message: "socket is already connected",
  },
  UvError {
    code: -21,
    name: "EISDIR",
    message: "illegal operation on a directory",
  },
  UvError {
    code: -62,
    name: "ELOOP",
    message: "too many symbolic links encountered",
  },
  UvError {
    code: -24,
    name: "EMFILE",
    message: "too many open files",
  },
  UvError {
    code: -40,
    name: "EMSGSIZE",
    message: "message too long",
  },
  UvError {
    code: -63,
    name: "ENAMETOOLONG",
    message: "name too long",
  },
  UvError {
    code: -50,
    name: "ENETDOWN",
    message: "network is down",
  },
  UvError {
    code: -51,
    name: "ENETUNREACH",
    message: "network is unreachable",
  },
  UvError {
    code: -23,
    name: "ENFILE",
    message: "file table overflow",
  },
  UvError {
    code: -55,
    name: "ENOBUFS",
    message: "no buffer space available",
  },
  UvError {
    code: -19,
    name: "ENODEV",
    message: "no such device",
  },
  UvError {
    code: -2,
    name: "ENOENT",
    message: "no such file or directory",
  },
  UvError {
    code: -12,
    name: "ENOMEM",
    message: "not enough memory",
  },
  UvError {
    code: -4056,
    name: "ENONET",
    message: "machine is not on the network",
  },
  UvError {
    code: -42,
    name: "ENOPROTOOPT",
    message: "protocol not available",
  },
  UvError {
    code: -28,
    name: "ENOSPC",
    message: "no space left on device",
  },
  UvError {
    code: -78,
    name: "ENOSYS",
    message: "function not implemented",
  },
  UvError {
    code: -57,
    name: "ENOTCONN",
    message: "socket is not connected",
  },
  UvError {
    code: -20,
    name: "ENOTDIR",
    message: "not a directory",
  },
  UvError {
    code: -66,
    name: "ENOTEMPTY",
    message: "directory not empty",
  },
  UvError {
    code: -38,
    name: "ENOTSOCK",
    message: "socket operation on non-socket",
  },
  UvError {
    code: -45,
    name: "ENOTSUP",
    message: "operation not supported on socket",
  },
  UvError {
    code: -87,
    name: "EOVERFLOW",
    message: "value too large for defined data type",
  },
  UvError {
    code: -1,
    name: "EPERM",
    message: "operation not permitted",
  },
  UvError {
    code: -32,
    name: "EPIPE",
    message: "broken pipe",
  },
  UvError {
    code: -95,
    name: "EPROTO",
    message: "protocol error",
  },
  UvError {
    code: -43,
    name: "EPROTONOSUPPORT",
    message: "protocol not supported",
  },
  UvError {
    code: -41,
    name: "EPROTOTYPE",
    message: "protocol wrong type for socket",
  },
  UvError {
    code: -34,
    name: "ERANGE",
    message: "result too large",
  },
  UvError {
    code: -30,
    name: "EROFS",
    message: "read-only file system",
  },
  UvError {
    code: -58,
    name: "ESHUTDOWN",
    message: "cannot send after transport endpoint shutdown",
  },
  UvError {
    code: -29,
    name: "ESPIPE",
    message: "invalid seek",
  },
  UvError {
    code: -3,
    name: "ESRCH",
    message: "no such process",
  },
  UvError {
    code: -60,
    name: "ETIMEDOUT",
    message: "connection timed out",
  },
  UvError {
    code: -26,
    name: "ETXTBSY",
    message: "text file is busy",
  },
  UvError {
    code: -18,
    name: "EXDEV",
    message: "cross-device link not permitted",
  },
  UvError {
    code: -4094,
    name: "UNKNOWN",
    message: "unknown error",
  },
  UvError {
    code: -4095,
    name: "EOF",
    message: "end of file",
  },
  UvError {
    code: -6,
    name: "ENXIO",
    message: "no such device or address",
  },
  UvError {
    code: -31,
    name: "EMLINK",
    message: "too many links",
  },
  UvError {
    code: -64,
    name: "EHOSTDOWN",
    message: "host is down",
  },
  UvError {
    code: -4030,
    name: "EREMOTEIO",
    message: "remote I/O error",
  },
  UvError {
    code: -25,
    name: "ENOTTY",
    message: "inappropriate ioctl for device",
  },
  UvError {
    code: -79,
    name: "EFTYPE",
    message: "inappropriate file type or format",
  },
  UvError {
    code: -84,
    name: "EILSEQ",
    message: "illegal byte sequence",
  },
  UvError {
    code: -44,
    name: "ESOCKTNOSUPPORT",
    message: "socket type not supported",
  },
];

fn active_errors() -> &'static [UvError] {
  if cfg!(windows) {
    CODE_TO_ERROR_WINDOWS
  } else if cfg!(target_os = "macos") {
    CODE_TO_ERROR_DARWIN
  } else if cfg!(any(target_os = "linux", target_os = "android")) {
    CODE_TO_ERROR_LINUX
  } else if cfg!(target_os = "freebsd") {
    CODE_TO_ERROR_FREEBSD
  } else if cfg!(target_os = "openbsd") {
    CODE_TO_ERROR_OPENBSD
  } else {
    CODE_TO_ERROR_LINUX
  }
}

fn error_for_code(errno: i32) -> Option<&'static UvError> {
  active_errors().iter().find(|error| error.code == errno)
}

fn code_for_name(name: &str) -> Option<i32> {
  active_errors()
    .iter()
    .find(|error| error.name == name)
    .map(|error| error.code)
}

fn set_value(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  name: &str,
  value: v8::Local<v8::Value>,
) {
  let key = v8::String::new(scope, name).unwrap();
  obj.set(scope, key.into(), value);
}

fn set_i32(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  name: &str,
  value: i32,
) {
  let value = v8::Integer::new(scope, value);
  set_value(scope, obj, name, value.into());
}

fn set_code_const(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  export_name: &str,
  code_name: &str,
) {
  if let Some(code) = code_for_name(code_name) {
    set_i32(scope, obj, export_name, code);
  } else {
    let undefined: v8::Local<v8::Value> = v8::undefined(scope).into();
    set_value(scope, obj, export_name, undefined);
  }
}

fn core_ops<'s>(scope: &mut v8::PinScope<'s, '_>) -> v8::Local<'s, v8::Object> {
  let context = scope.get_current_context();
  let global = context.global(scope);
  let deno_key = v8::String::new(scope, "Deno").unwrap();
  let core_key = v8::String::new(scope, "core").unwrap();
  let ops_key = v8::String::new(scope, "ops").unwrap();
  let deno = global.get(scope, deno_key.into()).unwrap();
  let deno = v8::Local::<v8::Object>::try_from(deno).unwrap();
  let core = deno.get(scope, core_key.into()).unwrap();
  let core = v8::Local::<v8::Object>::try_from(core).unwrap();
  let ops = core.get(scope, ops_key.into()).unwrap();
  v8::Local::<v8::Object>::try_from(ops).unwrap()
}

fn set_op_alias(
  scope: &mut v8::PinScope,
  obj: v8::Local<v8::Object>,
  export_name: &str,
  op_name: &str,
) {
  let ops = core_ops(scope);
  let op_key = v8::String::new(scope, op_name).unwrap();
  let op = ops.get(scope, op_key.into()).unwrap();
  set_value(scope, obj, export_name, op);
}

fn build_error_map<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Map> {
  let map = v8::Map::new(scope);
  for error in active_errors() {
    let key = v8::Integer::new(scope, error.code);
    let value = v8::Array::new(scope, 2);
    let name = v8::String::new(scope, error.name).unwrap();
    let message = v8::String::new(scope, error.message).unwrap();
    value.set_index(scope, 0, name.into());
    value.set_index(scope, 1, message.into());
    map.set(scope, key.into(), value.into()).unwrap();
  }
  map
}

fn build_code_map<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Map> {
  let map = v8::Map::new(scope);
  for error in active_errors() {
    let key = v8::String::new(scope, error.name).unwrap();
    let value = v8::Integer::new(scope, error.code);
    map.set(scope, key.into(), value.into()).unwrap();
  }
  map
}

#[op2]
pub fn op_node_internal_binding_uv<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Object> {
  let obj = v8::Object::new(scope);
  let error_map = build_error_map(scope);
  set_value(scope, obj, "errorMap", error_map.into());
  let code_map = build_code_map(scope);
  set_value(scope, obj, "codeMap", code_map.into());

  set_code_const(scope, obj, "UV_EAI_MEMORY", "EAI_MEMORY");
  set_code_const(scope, obj, "UV_EBADF", "EBADF");
  set_code_const(scope, obj, "UV_ECANCELED", "ECANCELED");
  set_code_const(scope, obj, "UV_EEXIST", "EEXIST");
  set_code_const(scope, obj, "UV_EINVAL", "EINVAL");
  set_code_const(scope, obj, "UV_ENETUNREACH", "ENETUNREACH");
  set_code_const(scope, obj, "UV_ENOENT", "ENOENT");
  set_code_const(scope, obj, "UV_ENOMEM", "ENOMEM");
  set_code_const(scope, obj, "UV_ENOTSOCK", "ENOTSOCK");
  set_code_const(scope, obj, "UV_ETIMEDOUT", "ETIMEDOUT");
  set_code_const(scope, obj, "UV_UNKNOWN", "UNKNOWN");
  set_code_const(scope, obj, "UV_EOF", "EOF");

  set_op_alias(
    scope,
    obj,
    "mapSysErrnoToUvErrno",
    "op_node_uv_map_sys_errno_to_uv_errno",
  );
  set_op_alias(scope, obj, "errname", "op_node_uv_errname");
  set_op_alias(
    scope,
    obj,
    "getErrorMessage",
    "op_node_uv_get_error_message",
  );
  set_op_alias(scope, obj, "getErrorMap", "op_node_uv_get_error_map");
  set_op_alias(scope, obj, "getCodeMap", "op_node_uv_get_code_map");
  obj
}

#[op2(fast)]
pub fn op_node_uv_map_sys_errno_to_uv_errno(#[smi] sys_errno: i32) -> i32 {
  if cfg!(windows) {
    let code = node_sys_to_uv_error(sys_errno);
    code_for_name(code).unwrap_or(-sys_errno)
  } else {
    -sys_errno
  }
}

#[op2]
#[string]
pub fn op_node_uv_errname(#[smi] errno: i32) -> String {
  if let Some(error) = error_for_code(errno) {
    error.name.to_string()
  } else {
    format!("UNKNOWN ({errno})")
  }
}

#[op2]
#[string]
pub fn op_node_uv_get_error_message(#[smi] errno: i32) -> String {
  if let Some(error) = error_for_code(errno) {
    error.message.to_string()
  } else {
    format!("UNKNOWN ({errno})")
  }
}

#[op2]
pub fn op_node_uv_get_error_map<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Map> {
  build_error_map(scope)
}

#[op2]
pub fn op_node_uv_get_code_map<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Map> {
  build_code_map(scope)
}
