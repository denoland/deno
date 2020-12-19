// Copyright Node.js contributors. All rights reserved. MIT License.
/************ NOT IMPLEMENTED
* ERR_INVALID_ARG_VALUE
* ERR_INVALID_MODULE_SPECIFIER
* ERR_INVALID_PACKAGE_TARGET
* ERR_INVALID_URL_SCHEME
* ERR_MANIFEST_ASSERT_INTEGRITY
* ERR_MODULE_NOT_FOUND
* ERR_PACKAGE_PATH_NOT_EXPORTED
* ERR_QUICSESSION_VERSION_NEGOTIATION
* ERR_REQUIRE_ESM
* ERR_SOCKET_BAD_PORT
* ERR_TLS_CERT_ALTNAME_INVALID
* ERR_UNHANDLED_ERROR
* ERR_WORKER_INVALID_EXEC_ARGV
* ERR_WORKER_PATH
* ERR_QUIC_ERROR
* ERR_SOCKET_BUFFER_SIZE //System error, shouldn't ever happen inside Deno
* ERR_SYSTEM_ERROR //System error, shouldn't ever happen inside Deno
* ERR_TTY_INIT_FAILED //System error, shouldn't ever happen inside Deno
* ERR_INVALID_PACKAGE_CONFIG // package.json stuff, probably useless
*************/

import { unreachable } from "../testing/asserts.ts";

/**
 * All error instances in Node have additional methods and properties
 * This export class is meant to be extended by these instances abstracting native JS error instances
 */
export class NodeErrorAbstraction extends Error {
  code: string;

  constructor(name: string, code: string, message: string) {
    super(message);
    this.code = code;
    this.name = name;
    //This number changes dependending on the name of this class
    //20 characters as of now
    this.stack = this.stack && `${name} [${this.code}]${this.stack.slice(20)}`;
  }

  toString() {
    return `${this.name} [${this.code}]: ${this.message}`;
  }
}

export class NodeError extends NodeErrorAbstraction {
  constructor(code: string, message: string) {
    super(Error.prototype.name, code, message);
  }
}

export class NodeSyntaxError extends NodeErrorAbstraction
  implements SyntaxError {
  constructor(code: string, message: string) {
    super(SyntaxError.prototype.name, code, message);
    Object.setPrototypeOf(this, SyntaxError.prototype);
  }
}

export class NodeRangeError extends NodeErrorAbstraction {
  constructor(code: string, message: string) {
    super(RangeError.prototype.name, code, message);
    Object.setPrototypeOf(this, RangeError.prototype);
  }
}

export class NodeTypeError extends NodeErrorAbstraction implements TypeError {
  constructor(code: string, message: string) {
    super(TypeError.prototype.name, code, message);
    Object.setPrototypeOf(this, TypeError.prototype);
  }
}

export class NodeURIError extends NodeErrorAbstraction implements URIError {
  constructor(code: string, message: string) {
    super(URIError.prototype.name, code, message);
    Object.setPrototypeOf(this, URIError.prototype);
  }
}

export class ERR_INVALID_ARG_TYPE extends NodeTypeError {
  constructor(a1: string, a2: string | string[], a3: unknown) {
    super(
      "ERR_INVALID_ARG_TYPE",
      `The "${a1}" argument must be of type ${
        typeof a2 === "string"
          ? a2.toLocaleLowerCase()
          : a2.map((x) => x.toLocaleLowerCase()).join(", ")
      }. Received ${typeof a3} (${a3})`,
    );
  }
}

export class ERR_OUT_OF_RANGE extends RangeError {
  code = "ERR_OUT_OF_RANGE";

  constructor(str: string, range: string, received: unknown) {
    super(
      `The value of "${str}" is out of range. It must be ${range}. Received ${received}`,
    );

    const { name } = this;
    // Add the error code to the name to include it in the stack trace.
    this.name = `${name} [${this.code}]`;
    // Access the stack to generate the error message including the error code from the name.
    this.stack;
    // Reset the name to the actual name.
    this.name = name;
  }
}

export class ERR_AMBIGUOUS_ARGUMENT extends NodeTypeError {
  constructor(x: string, y: string) {
    super("ERR_AMBIGUOUS_ARGUMENT", `The "${x}" argument is ambiguous. ${y}`);
  }
}

export class ERR_ARG_NOT_ITERABLE extends NodeTypeError {
  constructor(x: string) {
    super("ERR_ARG_NOT_ITERABLE", `${x} must be iterable`);
  }
}

export class ERR_ASSERTION extends NodeError {
  constructor(x: string) {
    super("ERR_ASSERTION", `${x}`);
  }
}

export class ERR_ASYNC_CALLBACK extends NodeTypeError {
  constructor(x: string) {
    super("ERR_ASYNC_CALLBACK", `${x} must be a function`);
  }
}

export class ERR_ASYNC_TYPE extends NodeTypeError {
  constructor(x: string) {
    super("ERR_ASYNC_TYPE", `Invalid name for async "type": ${x}`);
  }
}

export class ERR_BROTLI_INVALID_PARAM extends NodeRangeError {
  constructor(x: string) {
    super("ERR_BROTLI_INVALID_PARAM", `${x} is not a valid Brotli parameter`);
  }
}

export class ERR_BUFFER_OUT_OF_BOUNDS extends NodeRangeError {
  constructor(name?: string) {
    super(
      "ERR_BUFFER_OUT_OF_BOUNDS",
      name
        ? `"${name}" is outside of buffer bounds`
        : "Attempt to access memory outside buffer bounds",
    );
  }
}

export class ERR_BUFFER_TOO_LARGE extends NodeRangeError {
  constructor(x: string) {
    super(
      "ERR_BUFFER_TOO_LARGE",
      `Cannot create a Buffer larger than ${x} bytes`,
    );
  }
}

export class ERR_CANNOT_WATCH_SIGINT extends NodeError {
  constructor() {
    super(
      "ERR_CANNOT_WATCH_SIGINT",
      "Cannot watch for SIGINT signals",
    );
  }
}

export class ERR_CHILD_CLOSED_BEFORE_REPLY extends NodeError {
  constructor() {
    super(
      "ERR_CHILD_CLOSED_BEFORE_REPLY",
      "Child closed before reply received",
    );
  }
}

export class ERR_CHILD_PROCESS_IPC_REQUIRED extends NodeError {
  constructor(x: string) {
    super(
      "ERR_CHILD_PROCESS_IPC_REQUIRED",
      `Forked processes must have an IPC channel, missing value 'ipc' in ${x}`,
    );
  }
}

export class ERR_CHILD_PROCESS_STDIO_MAXBUFFER extends NodeRangeError {
  constructor(x: string) {
    super(
      "ERR_CHILD_PROCESS_STDIO_MAXBUFFER",
      `${x} maxBuffer length exceeded`,
    );
  }
}

export class ERR_CONSOLE_WRITABLE_STREAM extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_CONSOLE_WRITABLE_STREAM",
      `Console expects a writable stream instance for ${x}`,
    );
  }
}

export class ERR_CONTEXT_NOT_INITIALIZED extends NodeError {
  constructor() {
    super(
      "ERR_CONTEXT_NOT_INITIALIZED",
      "context used is not initialized",
    );
  }
}

export class ERR_CPU_USAGE extends NodeError {
  constructor(x: string) {
    super(
      "ERR_CPU_USAGE",
      `Unable to obtain cpu usage ${x}`,
    );
  }
}

export class ERR_CRYPTO_CUSTOM_ENGINE_NOT_SUPPORTED extends NodeError {
  constructor() {
    super(
      "ERR_CRYPTO_CUSTOM_ENGINE_NOT_SUPPORTED",
      "Custom engines not supported by this OpenSSL",
    );
  }
}

export class ERR_CRYPTO_ECDH_INVALID_FORMAT extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_CRYPTO_ECDH_INVALID_FORMAT",
      `Invalid ECDH format: ${x}`,
    );
  }
}

export class ERR_CRYPTO_ECDH_INVALID_PUBLIC_KEY extends NodeError {
  constructor() {
    super(
      "ERR_CRYPTO_ECDH_INVALID_PUBLIC_KEY",
      "Public key is not valid for specified curve",
    );
  }
}

export class ERR_CRYPTO_ENGINE_UNKNOWN extends NodeError {
  constructor(x: string) {
    super(
      "ERR_CRYPTO_ENGINE_UNKNOWN",
      `Engine "${x}" was not found`,
    );
  }
}

export class ERR_CRYPTO_FIPS_FORCED extends NodeError {
  constructor() {
    super(
      "ERR_CRYPTO_FIPS_FORCED",
      "Cannot set FIPS mode, it was forced with --force-fips at startup.",
    );
  }
}

export class ERR_CRYPTO_FIPS_UNAVAILABLE extends NodeError {
  constructor() {
    super(
      "ERR_CRYPTO_FIPS_UNAVAILABLE",
      "Cannot set FIPS mode in a non-FIPS build.",
    );
  }
}

export class ERR_CRYPTO_HASH_FINALIZED extends NodeError {
  constructor() {
    super(
      "ERR_CRYPTO_HASH_FINALIZED",
      "Digest already called",
    );
  }
}

export class ERR_CRYPTO_HASH_UPDATE_FAILED extends NodeError {
  constructor() {
    super(
      "ERR_CRYPTO_HASH_UPDATE_FAILED",
      "Hash update failed",
    );
  }
}

export class ERR_CRYPTO_INCOMPATIBLE_KEY extends NodeError {
  constructor(x: string, y: string) {
    super(
      "ERR_CRYPTO_INCOMPATIBLE_KEY",
      `Incompatible ${x}: ${y}`,
    );
  }
}

export class ERR_CRYPTO_INCOMPATIBLE_KEY_OPTIONS extends NodeError {
  constructor(x: string, y: string) {
    super(
      "ERR_CRYPTO_INCOMPATIBLE_KEY_OPTIONS",
      `The selected key encoding ${x} ${y}.`,
    );
  }
}

export class ERR_CRYPTO_INVALID_DIGEST extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_CRYPTO_INVALID_DIGEST",
      `Invalid digest: ${x}`,
    );
  }
}

export class ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE extends NodeTypeError {
  constructor(x: string, y: string) {
    super(
      "ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE",
      `Invalid key object type ${x}, expected ${y}.`,
    );
  }
}

export class ERR_CRYPTO_INVALID_STATE extends NodeError {
  constructor(x: string) {
    super(
      "ERR_CRYPTO_INVALID_STATE",
      `Invalid state for operation ${x}`,
    );
  }
}

export class ERR_CRYPTO_PBKDF2_ERROR extends NodeError {
  constructor() {
    super(
      "ERR_CRYPTO_PBKDF2_ERROR",
      "PBKDF2 error",
    );
  }
}

export class ERR_CRYPTO_SCRYPT_INVALID_PARAMETER extends NodeError {
  constructor() {
    super(
      "ERR_CRYPTO_SCRYPT_INVALID_PARAMETER",
      "Invalid scrypt parameter",
    );
  }
}

export class ERR_CRYPTO_SCRYPT_NOT_SUPPORTED extends NodeError {
  constructor() {
    super(
      "ERR_CRYPTO_SCRYPT_NOT_SUPPORTED",
      "Scrypt algorithm not supported",
    );
  }
}

export class ERR_CRYPTO_SIGN_KEY_REQUIRED extends NodeError {
  constructor() {
    super(
      "ERR_CRYPTO_SIGN_KEY_REQUIRED",
      "No key provided to sign",
    );
  }
}

export class ERR_DIR_CLOSED extends NodeError {
  constructor() {
    super(
      "ERR_DIR_CLOSED",
      "Directory handle was closed",
    );
  }
}

export class ERR_DIR_CONCURRENT_OPERATION extends NodeError {
  constructor() {
    super(
      "ERR_DIR_CONCURRENT_OPERATION",
      "Cannot do synchronous work on directory handle with concurrent asynchronous operations",
    );
  }
}

export class ERR_DNS_SET_SERVERS_FAILED extends NodeError {
  constructor(x: string, y: string) {
    super(
      "ERR_DNS_SET_SERVERS_FAILED",
      `c-ares failed to set servers: "${x}" [${y}]`,
    );
  }
}

export class ERR_DOMAIN_CALLBACK_NOT_AVAILABLE extends NodeError {
  constructor() {
    super(
      "ERR_DOMAIN_CALLBACK_NOT_AVAILABLE",
      "A callback was registered through " +
        "process.setUncaughtExceptionCaptureCallback(), which is mutually " +
        "exclusive with using the `domain` module",
    );
  }
}

export class ERR_DOMAIN_CANNOT_SET_UNCAUGHT_EXCEPTION_CAPTURE
  extends NodeError {
  constructor() {
    super(
      "ERR_DOMAIN_CANNOT_SET_UNCAUGHT_EXCEPTION_CAPTURE",
      "The `domain` module is in use, which is mutually exclusive with calling " +
        "process.setUncaughtExceptionCaptureCallback()",
    );
  }
}

export class ERR_ENCODING_INVALID_ENCODED_DATA extends NodeErrorAbstraction
  implements TypeError {
  errno: number;
  constructor(encoding: string, ret: number) {
    super(
      TypeError.prototype.name,
      "ERR_ENCODING_INVALID_ENCODED_DATA",
      `The encoded data was not valid for encoding ${encoding}`,
    );
    Object.setPrototypeOf(this, TypeError.prototype);

    this.errno = ret;
  }
}

// In Node these values are coming from libuv:
// Ref: https://github.com/libuv/libuv/blob/v1.x/include/uv/errno.h
// Ref: https://github.com/nodejs/node/blob/524123fbf064ff64bb6fcd83485cfc27db932f68/lib/internal/errors.js#L383
// Since there is no easy way to port code from libuv and these maps are
// changing very rarely, we simply extract them from Node and store here.

// Note
// Run the following to get the map:
// $ node -e "console.log(process.binding('uv').getErrorMap())"
// This setup automatically exports maps from both "win", "linux" & darwin:
// https://github.com/schwarzkopfb/node_errno_map

type ErrMapData = Array<[number, [string, string]]>;

const windows: ErrMapData = [
  [-4093, ["E2BIG", "argument list too long"]],
  [-4092, ["EACCES", "permission denied"]],
  [-4091, ["EADDRINUSE", "address already in use"]],
  [-4090, ["EADDRNOTAVAIL", "address not available"]],
  [-4089, ["EAFNOSUPPORT", "address family not supported"]],
  [-4088, ["EAGAIN", "resource temporarily unavailable"]],
  [-3000, ["EAI_ADDRFAMILY", "address family not supported"]],
  [-3001, ["EAI_AGAIN", "temporary failure"]],
  [-3002, ["EAI_BADFLAGS", "bad ai_flags value"]],
  [-3013, ["EAI_BADHINTS", "invalid value for hints"]],
  [-3003, ["EAI_CANCELED", "request canceled"]],
  [-3004, ["EAI_FAIL", "permanent failure"]],
  [-3005, ["EAI_FAMILY", "ai_family not supported"]],
  [-3006, ["EAI_MEMORY", "out of memory"]],
  [-3007, ["EAI_NODATA", "no address"]],
  [-3008, ["EAI_NONAME", "unknown node or service"]],
  [-3009, ["EAI_OVERFLOW", "argument buffer overflow"]],
  [-3014, ["EAI_PROTOCOL", "resolved protocol is unknown"]],
  [-3010, ["EAI_SERVICE", "service not available for socket type"]],
  [-3011, ["EAI_SOCKTYPE", "socket type not supported"]],
  [-4084, ["EALREADY", "connection already in progress"]],
  [-4083, ["EBADF", "bad file descriptor"]],
  [-4082, ["EBUSY", "resource busy or locked"]],
  [-4081, ["ECANCELED", "operation canceled"]],
  [-4080, ["ECHARSET", "invalid Unicode character"]],
  [-4079, ["ECONNABORTED", "software caused connection abort"]],
  [-4078, ["ECONNREFUSED", "connection refused"]],
  [-4077, ["ECONNRESET", "connection reset by peer"]],
  [-4076, ["EDESTADDRREQ", "destination address required"]],
  [-4075, ["EEXIST", "file already exists"]],
  [-4074, ["EFAULT", "bad address in system call argument"]],
  [-4036, ["EFBIG", "file too large"]],
  [-4073, ["EHOSTUNREACH", "host is unreachable"]],
  [-4072, ["EINTR", "interrupted system call"]],
  [-4071, ["EINVAL", "invalid argument"]],
  [-4070, ["EIO", "i/o error"]],
  [-4069, ["EISCONN", "socket is already connected"]],
  [-4068, ["EISDIR", "illegal operation on a directory"]],
  [-4067, ["ELOOP", "too many symbolic links encountered"]],
  [-4066, ["EMFILE", "too many open files"]],
  [-4065, ["EMSGSIZE", "message too long"]],
  [-4064, ["ENAMETOOLONG", "name too long"]],
  [-4063, ["ENETDOWN", "network is down"]],
  [-4062, ["ENETUNREACH", "network is unreachable"]],
  [-4061, ["ENFILE", "file table overflow"]],
  [-4060, ["ENOBUFS", "no buffer space available"]],
  [-4059, ["ENODEV", "no such device"]],
  [-4058, ["ENOENT", "no such file or directory"]],
  [-4057, ["ENOMEM", "not enough memory"]],
  [-4056, ["ENONET", "machine is not on the network"]],
  [-4035, ["ENOPROTOOPT", "protocol not available"]],
  [-4055, ["ENOSPC", "no space left on device"]],
  [-4054, ["ENOSYS", "function not implemented"]],
  [-4053, ["ENOTCONN", "socket is not connected"]],
  [-4052, ["ENOTDIR", "not a directory"]],
  [-4051, ["ENOTEMPTY", "directory not empty"]],
  [-4050, ["ENOTSOCK", "socket operation on non-socket"]],
  [-4049, ["ENOTSUP", "operation not supported on socket"]],
  [-4048, ["EPERM", "operation not permitted"]],
  [-4047, ["EPIPE", "broken pipe"]],
  [-4046, ["EPROTO", "protocol error"]],
  [-4045, ["EPROTONOSUPPORT", "protocol not supported"]],
  [-4044, ["EPROTOTYPE", "protocol wrong type for socket"]],
  [-4034, ["ERANGE", "result too large"]],
  [-4043, ["EROFS", "read-only file system"]],
  [-4042, ["ESHUTDOWN", "cannot send after transport endpoint shutdown"]],
  [-4041, ["ESPIPE", "invalid seek"]],
  [-4040, ["ESRCH", "no such process"]],
  [-4039, ["ETIMEDOUT", "connection timed out"]],
  [-4038, ["ETXTBSY", "text file is busy"]],
  [-4037, ["EXDEV", "cross-device link not permitted"]],
  [-4094, ["UNKNOWN", "unknown error"]],
  [-4095, ["EOF", "end of file"]],
  [-4033, ["ENXIO", "no such device or address"]],
  [-4032, ["EMLINK", "too many links"]],
  [-4031, ["EHOSTDOWN", "host is down"]],
  [-4030, ["EREMOTEIO", "remote I/O error"]],
  [-4029, ["ENOTTY", "inappropriate ioctl for device"]],
  [-4028, ["EFTYPE", "inappropriate file type or format"]],
  [-4027, ["EILSEQ", "illegal byte sequence"]],
];

const darwin: ErrMapData = [
  [-7, ["E2BIG", "argument list too long"]],
  [-13, ["EACCES", "permission denied"]],
  [-48, ["EADDRINUSE", "address already in use"]],
  [-49, ["EADDRNOTAVAIL", "address not available"]],
  [-47, ["EAFNOSUPPORT", "address family not supported"]],
  [-35, ["EAGAIN", "resource temporarily unavailable"]],
  [-3000, ["EAI_ADDRFAMILY", "address family not supported"]],
  [-3001, ["EAI_AGAIN", "temporary failure"]],
  [-3002, ["EAI_BADFLAGS", "bad ai_flags value"]],
  [-3013, ["EAI_BADHINTS", "invalid value for hints"]],
  [-3003, ["EAI_CANCELED", "request canceled"]],
  [-3004, ["EAI_FAIL", "permanent failure"]],
  [-3005, ["EAI_FAMILY", "ai_family not supported"]],
  [-3006, ["EAI_MEMORY", "out of memory"]],
  [-3007, ["EAI_NODATA", "no address"]],
  [-3008, ["EAI_NONAME", "unknown node or service"]],
  [-3009, ["EAI_OVERFLOW", "argument buffer overflow"]],
  [-3014, ["EAI_PROTOCOL", "resolved protocol is unknown"]],
  [-3010, ["EAI_SERVICE", "service not available for socket type"]],
  [-3011, ["EAI_SOCKTYPE", "socket type not supported"]],
  [-37, ["EALREADY", "connection already in progress"]],
  [-9, ["EBADF", "bad file descriptor"]],
  [-16, ["EBUSY", "resource busy or locked"]],
  [-89, ["ECANCELED", "operation canceled"]],
  [-4080, ["ECHARSET", "invalid Unicode character"]],
  [-53, ["ECONNABORTED", "software caused connection abort"]],
  [-61, ["ECONNREFUSED", "connection refused"]],
  [-54, ["ECONNRESET", "connection reset by peer"]],
  [-39, ["EDESTADDRREQ", "destination address required"]],
  [-17, ["EEXIST", "file already exists"]],
  [-14, ["EFAULT", "bad address in system call argument"]],
  [-27, ["EFBIG", "file too large"]],
  [-65, ["EHOSTUNREACH", "host is unreachable"]],
  [-4, ["EINTR", "interrupted system call"]],
  [-22, ["EINVAL", "invalid argument"]],
  [-5, ["EIO", "i/o error"]],
  [-56, ["EISCONN", "socket is already connected"]],
  [-21, ["EISDIR", "illegal operation on a directory"]],
  [-62, ["ELOOP", "too many symbolic links encountered"]],
  [-24, ["EMFILE", "too many open files"]],
  [-40, ["EMSGSIZE", "message too long"]],
  [-63, ["ENAMETOOLONG", "name too long"]],
  [-50, ["ENETDOWN", "network is down"]],
  [-51, ["ENETUNREACH", "network is unreachable"]],
  [-23, ["ENFILE", "file table overflow"]],
  [-55, ["ENOBUFS", "no buffer space available"]],
  [-19, ["ENODEV", "no such device"]],
  [-2, ["ENOENT", "no such file or directory"]],
  [-12, ["ENOMEM", "not enough memory"]],
  [-4056, ["ENONET", "machine is not on the network"]],
  [-42, ["ENOPROTOOPT", "protocol not available"]],
  [-28, ["ENOSPC", "no space left on device"]],
  [-78, ["ENOSYS", "function not implemented"]],
  [-57, ["ENOTCONN", "socket is not connected"]],
  [-20, ["ENOTDIR", "not a directory"]],
  [-66, ["ENOTEMPTY", "directory not empty"]],
  [-38, ["ENOTSOCK", "socket operation on non-socket"]],
  [-45, ["ENOTSUP", "operation not supported on socket"]],
  [-1, ["EPERM", "operation not permitted"]],
  [-32, ["EPIPE", "broken pipe"]],
  [-100, ["EPROTO", "protocol error"]],
  [-43, ["EPROTONOSUPPORT", "protocol not supported"]],
  [-41, ["EPROTOTYPE", "protocol wrong type for socket"]],
  [-34, ["ERANGE", "result too large"]],
  [-30, ["EROFS", "read-only file system"]],
  [-58, ["ESHUTDOWN", "cannot send after transport endpoint shutdown"]],
  [-29, ["ESPIPE", "invalid seek"]],
  [-3, ["ESRCH", "no such process"]],
  [-60, ["ETIMEDOUT", "connection timed out"]],
  [-26, ["ETXTBSY", "text file is busy"]],
  [-18, ["EXDEV", "cross-device link not permitted"]],
  [-4094, ["UNKNOWN", "unknown error"]],
  [-4095, ["EOF", "end of file"]],
  [-6, ["ENXIO", "no such device or address"]],
  [-31, ["EMLINK", "too many links"]],
  [-64, ["EHOSTDOWN", "host is down"]],
  [-4030, ["EREMOTEIO", "remote I/O error"]],
  [-25, ["ENOTTY", "inappropriate ioctl for device"]],
  [-79, ["EFTYPE", "inappropriate file type or format"]],
  [-92, ["EILSEQ", "illegal byte sequence"]],
];

const linux: ErrMapData = [
  [-7, ["E2BIG", "argument list too long"]],
  [-13, ["EACCES", "permission denied"]],
  [-98, ["EADDRINUSE", "address already in use"]],
  [-99, ["EADDRNOTAVAIL", "address not available"]],
  [-97, ["EAFNOSUPPORT", "address family not supported"]],
  [-11, ["EAGAIN", "resource temporarily unavailable"]],
  [-3000, ["EAI_ADDRFAMILY", "address family not supported"]],
  [-3001, ["EAI_AGAIN", "temporary failure"]],
  [-3002, ["EAI_BADFLAGS", "bad ai_flags value"]],
  [-3013, ["EAI_BADHINTS", "invalid value for hints"]],
  [-3003, ["EAI_CANCELED", "request canceled"]],
  [-3004, ["EAI_FAIL", "permanent failure"]],
  [-3005, ["EAI_FAMILY", "ai_family not supported"]],
  [-3006, ["EAI_MEMORY", "out of memory"]],
  [-3007, ["EAI_NODATA", "no address"]],
  [-3008, ["EAI_NONAME", "unknown node or service"]],
  [-3009, ["EAI_OVERFLOW", "argument buffer overflow"]],
  [-3014, ["EAI_PROTOCOL", "resolved protocol is unknown"]],
  [-3010, ["EAI_SERVICE", "service not available for socket type"]],
  [-3011, ["EAI_SOCKTYPE", "socket type not supported"]],
  [-114, ["EALREADY", "connection already in progress"]],
  [-9, ["EBADF", "bad file descriptor"]],
  [-16, ["EBUSY", "resource busy or locked"]],
  [-125, ["ECANCELED", "operation canceled"]],
  [-4080, ["ECHARSET", "invalid Unicode character"]],
  [-103, ["ECONNABORTED", "software caused connection abort"]],
  [-111, ["ECONNREFUSED", "connection refused"]],
  [-104, ["ECONNRESET", "connection reset by peer"]],
  [-89, ["EDESTADDRREQ", "destination address required"]],
  [-17, ["EEXIST", "file already exists"]],
  [-14, ["EFAULT", "bad address in system call argument"]],
  [-27, ["EFBIG", "file too large"]],
  [-113, ["EHOSTUNREACH", "host is unreachable"]],
  [-4, ["EINTR", "interrupted system call"]],
  [-22, ["EINVAL", "invalid argument"]],
  [-5, ["EIO", "i/o error"]],
  [-106, ["EISCONN", "socket is already connected"]],
  [-21, ["EISDIR", "illegal operation on a directory"]],
  [-40, ["ELOOP", "too many symbolic links encountered"]],
  [-24, ["EMFILE", "too many open files"]],
  [-90, ["EMSGSIZE", "message too long"]],
  [-36, ["ENAMETOOLONG", "name too long"]],
  [-100, ["ENETDOWN", "network is down"]],
  [-101, ["ENETUNREACH", "network is unreachable"]],
  [-23, ["ENFILE", "file table overflow"]],
  [-105, ["ENOBUFS", "no buffer space available"]],
  [-19, ["ENODEV", "no such device"]],
  [-2, ["ENOENT", "no such file or directory"]],
  [-12, ["ENOMEM", "not enough memory"]],
  [-64, ["ENONET", "machine is not on the network"]],
  [-92, ["ENOPROTOOPT", "protocol not available"]],
  [-28, ["ENOSPC", "no space left on device"]],
  [-38, ["ENOSYS", "function not implemented"]],
  [-107, ["ENOTCONN", "socket is not connected"]],
  [-20, ["ENOTDIR", "not a directory"]],
  [-39, ["ENOTEMPTY", "directory not empty"]],
  [-88, ["ENOTSOCK", "socket operation on non-socket"]],
  [-95, ["ENOTSUP", "operation not supported on socket"]],
  [-1, ["EPERM", "operation not permitted"]],
  [-32, ["EPIPE", "broken pipe"]],
  [-71, ["EPROTO", "protocol error"]],
  [-93, ["EPROTONOSUPPORT", "protocol not supported"]],
  [-91, ["EPROTOTYPE", "protocol wrong type for socket"]],
  [-34, ["ERANGE", "result too large"]],
  [-30, ["EROFS", "read-only file system"]],
  [-108, ["ESHUTDOWN", "cannot send after transport endpoint shutdown"]],
  [-29, ["ESPIPE", "invalid seek"]],
  [-3, ["ESRCH", "no such process"]],
  [-110, ["ETIMEDOUT", "connection timed out"]],
  [-26, ["ETXTBSY", "text file is busy"]],
  [-18, ["EXDEV", "cross-device link not permitted"]],
  [-4094, ["UNKNOWN", "unknown error"]],
  [-4095, ["EOF", "end of file"]],
  [-6, ["ENXIO", "no such device or address"]],
  [-31, ["EMLINK", "too many links"]],
  [-112, ["EHOSTDOWN", "host is down"]],
  [-121, ["EREMOTEIO", "remote I/O error"]],
  [-25, ["ENOTTY", "inappropriate ioctl for device"]],
  [-4028, ["EFTYPE", "inappropriate file type or format"]],
  [-84, ["EILSEQ", "illegal byte sequence"]],
];

const { os } = Deno.build;
export const errorMap = new Map<number, [string, string]>(
  os === "windows"
    ? windows
    : os === "darwin"
    ? darwin
    : os === "linux"
    ? linux
    : unreachable(),
);
export class ERR_ENCODING_NOT_SUPPORTED extends NodeRangeError {
  constructor(x: string) {
    super(
      "ERR_ENCODING_NOT_SUPPORTED",
      `The "${x}" encoding is not supported`,
    );
  }
}
export class ERR_EVAL_ESM_CANNOT_PRINT extends NodeError {
  constructor() {
    super(
      "ERR_EVAL_ESM_CANNOT_PRINT",
      `--print cannot be used with ESM input`,
    );
  }
}
export class ERR_EVENT_RECURSION extends NodeError {
  constructor(x: string) {
    super(
      "ERR_EVENT_RECURSION",
      `The event "${x}" is already being dispatched`,
    );
  }
}
export class ERR_FEATURE_UNAVAILABLE_ON_PLATFORM extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_FEATURE_UNAVAILABLE_ON_PLATFORM",
      `The feature ${x} is unavailable on the current platform, which is being used to run Node.js`,
    );
  }
}
export class ERR_FS_FILE_TOO_LARGE extends NodeRangeError {
  constructor(x: string) {
    super(
      "ERR_FS_FILE_TOO_LARGE",
      `File size (${x}) is greater than 2 GB`,
    );
  }
}
export class ERR_FS_INVALID_SYMLINK_TYPE extends NodeError {
  constructor(x: string) {
    super(
      "ERR_FS_INVALID_SYMLINK_TYPE",
      `Symlink type must be one of "dir", "file", or "junction". Received "${x}"`,
    );
  }
}
export class ERR_HTTP2_ALTSVC_INVALID_ORIGIN extends NodeTypeError {
  constructor() {
    super(
      "ERR_HTTP2_ALTSVC_INVALID_ORIGIN",
      `HTTP/2 ALTSVC frames require a valid origin`,
    );
  }
}
export class ERR_HTTP2_ALTSVC_LENGTH extends NodeTypeError {
  constructor() {
    super(
      "ERR_HTTP2_ALTSVC_LENGTH",
      `HTTP/2 ALTSVC frames are limited to 16382 bytes`,
    );
  }
}
export class ERR_HTTP2_CONNECT_AUTHORITY extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_CONNECT_AUTHORITY",
      `:authority header is required for CONNECT requests`,
    );
  }
}
export class ERR_HTTP2_CONNECT_PATH extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_CONNECT_PATH",
      `The :path header is forbidden for CONNECT requests`,
    );
  }
}
export class ERR_HTTP2_CONNECT_SCHEME extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_CONNECT_SCHEME",
      `The :scheme header is forbidden for CONNECT requests`,
    );
  }
}
export class ERR_HTTP2_GOAWAY_SESSION extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_GOAWAY_SESSION",
      `New streams cannot be created after receiving a GOAWAY`,
    );
  }
}
export class ERR_HTTP2_HEADERS_AFTER_RESPOND extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_HEADERS_AFTER_RESPOND",
      `Cannot specify additional headers after response initiated`,
    );
  }
}
export class ERR_HTTP2_HEADERS_SENT extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_HEADERS_SENT",
      `Response has already been initiated.`,
    );
  }
}
export class ERR_HTTP2_HEADER_SINGLE_VALUE extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_HTTP2_HEADER_SINGLE_VALUE",
      `Header field "${x}" must only have a single value`,
    );
  }
}
export class ERR_HTTP2_INFO_STATUS_NOT_ALLOWED extends NodeRangeError {
  constructor() {
    super(
      "ERR_HTTP2_INFO_STATUS_NOT_ALLOWED",
      `Informational status codes cannot be used`,
    );
  }
}
export class ERR_HTTP2_INVALID_CONNECTION_HEADERS extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_HTTP2_INVALID_CONNECTION_HEADERS",
      `HTTP/1 Connection specific headers are forbidden: "${x}"`,
    );
  }
}
export class ERR_HTTP2_INVALID_HEADER_VALUE extends NodeTypeError {
  constructor(x: string, y: string) {
    super(
      "ERR_HTTP2_INVALID_HEADER_VALUE",
      `Invalid value "${x}" for header "${y}"`,
    );
  }
}
export class ERR_HTTP2_INVALID_INFO_STATUS extends NodeRangeError {
  constructor(x: string) {
    super(
      "ERR_HTTP2_INVALID_INFO_STATUS",
      `Invalid informational status code: ${x}`,
    );
  }
}
export class ERR_HTTP2_INVALID_ORIGIN extends NodeTypeError {
  constructor() {
    super(
      "ERR_HTTP2_INVALID_ORIGIN",
      `HTTP/2 ORIGIN frames require a valid origin`,
    );
  }
}
export class ERR_HTTP2_INVALID_PACKED_SETTINGS_LENGTH extends NodeRangeError {
  constructor() {
    super(
      "ERR_HTTP2_INVALID_PACKED_SETTINGS_LENGTH",
      `Packed settings length must be a multiple of six`,
    );
  }
}
export class ERR_HTTP2_INVALID_PSEUDOHEADER extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_HTTP2_INVALID_PSEUDOHEADER",
      `"${x}" is an invalid pseudoheader or is used incorrectly`,
    );
  }
}
export class ERR_HTTP2_INVALID_SESSION extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_INVALID_SESSION",
      `The session has been destroyed`,
    );
  }
}
export class ERR_HTTP2_INVALID_STREAM extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_INVALID_STREAM",
      `The stream has been destroyed`,
    );
  }
}
export class ERR_HTTP2_MAX_PENDING_SETTINGS_ACK extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_MAX_PENDING_SETTINGS_ACK",
      `Maximum number of pending settings acknowledgements`,
    );
  }
}
export class ERR_HTTP2_NESTED_PUSH extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_NESTED_PUSH",
      `A push stream cannot initiate another push stream.`,
    );
  }
}
export class ERR_HTTP2_NO_SOCKET_MANIPULATION extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_NO_SOCKET_MANIPULATION",
      `HTTP/2 sockets should not be directly manipulated (e.g. read and written)`,
    );
  }
}
export class ERR_HTTP2_ORIGIN_LENGTH extends NodeTypeError {
  constructor() {
    super(
      "ERR_HTTP2_ORIGIN_LENGTH",
      `HTTP/2 ORIGIN frames are limited to 16382 bytes`,
    );
  }
}
export class ERR_HTTP2_OUT_OF_STREAMS extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_OUT_OF_STREAMS",
      `No stream ID is available because maximum stream ID has been reached`,
    );
  }
}
export class ERR_HTTP2_PAYLOAD_FORBIDDEN extends NodeError {
  constructor(x: string) {
    super(
      "ERR_HTTP2_PAYLOAD_FORBIDDEN",
      `Responses with ${x} status must not have a payload`,
    );
  }
}
export class ERR_HTTP2_PING_CANCEL extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_PING_CANCEL",
      `HTTP2 ping cancelled`,
    );
  }
}
export class ERR_HTTP2_PING_LENGTH extends NodeRangeError {
  constructor() {
    super(
      "ERR_HTTP2_PING_LENGTH",
      `HTTP2 ping payload must be 8 bytes`,
    );
  }
}
export class ERR_HTTP2_PSEUDOHEADER_NOT_ALLOWED extends NodeTypeError {
  constructor() {
    super(
      "ERR_HTTP2_PSEUDOHEADER_NOT_ALLOWED",
      `Cannot set HTTP/2 pseudo-headers`,
    );
  }
}
export class ERR_HTTP2_PUSH_DISABLED extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_PUSH_DISABLED",
      `HTTP/2 client has disabled push streams`,
    );
  }
}
export class ERR_HTTP2_SEND_FILE extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_SEND_FILE",
      `Directories cannot be sent`,
    );
  }
}
export class ERR_HTTP2_SEND_FILE_NOSEEK extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_SEND_FILE_NOSEEK",
      `Offset or length can only be specified for regular files`,
    );
  }
}
export class ERR_HTTP2_SESSION_ERROR extends NodeError {
  constructor(x: string) {
    super(
      "ERR_HTTP2_SESSION_ERROR",
      `Session closed with error code ${x}`,
    );
  }
}
export class ERR_HTTP2_SETTINGS_CANCEL extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_SETTINGS_CANCEL",
      `HTTP2 session settings canceled`,
    );
  }
}
export class ERR_HTTP2_SOCKET_BOUND extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_SOCKET_BOUND",
      `The socket is already bound to an Http2Session`,
    );
  }
}
export class ERR_HTTP2_SOCKET_UNBOUND extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_SOCKET_UNBOUND",
      `The socket has been disconnected from the Http2Session`,
    );
  }
}
export class ERR_HTTP2_STATUS_101 extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_STATUS_101",
      `HTTP status code 101 (Switching Protocols) is forbidden in HTTP/2`,
    );
  }
}
export class ERR_HTTP2_STATUS_INVALID extends NodeRangeError {
  constructor(x: string) {
    super(
      "ERR_HTTP2_STATUS_INVALID",
      `Invalid status code: ${x}`,
    );
  }
}
export class ERR_HTTP2_STREAM_ERROR extends NodeError {
  constructor(x: string) {
    super(
      "ERR_HTTP2_STREAM_ERROR",
      `Stream closed with error code ${x}`,
    );
  }
}
export class ERR_HTTP2_STREAM_SELF_DEPENDENCY extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_STREAM_SELF_DEPENDENCY",
      `A stream cannot depend on itself`,
    );
  }
}
export class ERR_HTTP2_TRAILERS_ALREADY_SENT extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_TRAILERS_ALREADY_SENT",
      `Trailing headers have already been sent`,
    );
  }
}
export class ERR_HTTP2_TRAILERS_NOT_READY extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_TRAILERS_NOT_READY",
      `Trailing headers cannot be sent until after the wantTrailers event is emitted`,
    );
  }
}
export class ERR_HTTP2_UNSUPPORTED_PROTOCOL extends NodeError {
  constructor(x: string) {
    super(
      "ERR_HTTP2_UNSUPPORTED_PROTOCOL",
      `protocol "${x}" is unsupported.`,
    );
  }
}
export class ERR_HTTP_HEADERS_SENT extends NodeError {
  constructor(x: string) {
    super(
      "ERR_HTTP_HEADERS_SENT",
      `Cannot ${x} headers after they are sent to the client`,
    );
  }
}
export class ERR_HTTP_INVALID_HEADER_VALUE extends NodeTypeError {
  constructor(x: string, y: string) {
    super(
      "ERR_HTTP_INVALID_HEADER_VALUE",
      `Invalid value "${x}" for header "${y}"`,
    );
  }
}
export class ERR_HTTP_INVALID_STATUS_CODE extends NodeRangeError {
  constructor(x: string) {
    super(
      "ERR_HTTP_INVALID_STATUS_CODE",
      `Invalid status code: ${x}`,
    );
  }
}
export class ERR_HTTP_SOCKET_ENCODING extends NodeError {
  constructor() {
    super(
      "ERR_HTTP_SOCKET_ENCODING",
      `Changing the socket encoding is not allowed per RFC7230 Section 3.`,
    );
  }
}
export class ERR_HTTP_TRAILER_INVALID extends NodeError {
  constructor() {
    super(
      "ERR_HTTP_TRAILER_INVALID",
      `Trailers are invalid with this transfer encoding`,
    );
  }
}
export class ERR_INCOMPATIBLE_OPTION_PAIR extends NodeTypeError {
  constructor(x: string, y: string) {
    super(
      "ERR_INCOMPATIBLE_OPTION_PAIR",
      `Option "${x}" cannot be used in combination with option "${y}"`,
    );
  }
}
export class ERR_INPUT_TYPE_NOT_ALLOWED extends NodeError {
  constructor() {
    super(
      "ERR_INPUT_TYPE_NOT_ALLOWED",
      `--input-type can only be used with string input via --eval, --print, or STDIN`,
    );
  }
}
export class ERR_INSPECTOR_ALREADY_ACTIVATED extends NodeError {
  constructor() {
    super(
      "ERR_INSPECTOR_ALREADY_ACTIVATED",
      `Inspector is already activated. Close it with inspector.close() before activating it again.`,
    );
  }
}
export class ERR_INSPECTOR_ALREADY_CONNECTED extends NodeError {
  constructor(x: string) {
    super(
      "ERR_INSPECTOR_ALREADY_CONNECTED",
      `${x} is already connected`,
    );
  }
}
export class ERR_INSPECTOR_CLOSED extends NodeError {
  constructor() {
    super(
      "ERR_INSPECTOR_CLOSED",
      `Session was closed`,
    );
  }
}
export class ERR_INSPECTOR_COMMAND extends NodeError {
  constructor(x: number, y: string) {
    super(
      "ERR_INSPECTOR_COMMAND",
      `Inspector error ${x}: ${y}`,
    );
  }
}
export class ERR_INSPECTOR_NOT_ACTIVE extends NodeError {
  constructor() {
    super(
      "ERR_INSPECTOR_NOT_ACTIVE",
      `Inspector is not active`,
    );
  }
}
export class ERR_INSPECTOR_NOT_AVAILABLE extends NodeError {
  constructor() {
    super(
      "ERR_INSPECTOR_NOT_AVAILABLE",
      `Inspector is not available`,
    );
  }
}
export class ERR_INSPECTOR_NOT_CONNECTED extends NodeError {
  constructor() {
    super(
      "ERR_INSPECTOR_NOT_CONNECTED",
      `Session is not connected`,
    );
  }
}
export class ERR_INSPECTOR_NOT_WORKER extends NodeError {
  constructor() {
    super(
      "ERR_INSPECTOR_NOT_WORKER",
      `Current thread is not a worker`,
    );
  }
}
export class ERR_INVALID_ASYNC_ID extends NodeRangeError {
  constructor(x: string, y: string) {
    super(
      "ERR_INVALID_ASYNC_ID",
      `Invalid ${x} value: ${y}`,
    );
  }
}
export class ERR_INVALID_BUFFER_SIZE extends NodeRangeError {
  constructor(x: string) {
    super(
      "ERR_INVALID_BUFFER_SIZE",
      `Buffer size must be a multiple of ${x}`,
    );
  }
}
export class ERR_INVALID_CALLBACK extends NodeTypeError {
  constructor(object: unknown) {
    super(
      "ERR_INVALID_CALLBACK",
      `Callback must be a function. Received ${JSON.stringify(object)}`,
    );
  }
}
export class ERR_INVALID_CURSOR_POS extends NodeTypeError {
  constructor() {
    super(
      "ERR_INVALID_CURSOR_POS",
      `Cannot set cursor row without setting its column`,
    );
  }
}
export class ERR_INVALID_FD extends NodeRangeError {
  constructor(x: string) {
    super(
      "ERR_INVALID_FD",
      `"fd" must be a positive integer: ${x}`,
    );
  }
}
export class ERR_INVALID_FD_TYPE extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_INVALID_FD_TYPE",
      `Unsupported fd type: ${x}`,
    );
  }
}
export class ERR_INVALID_FILE_URL_HOST extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_INVALID_FILE_URL_HOST",
      `File URL host must be "localhost" or empty on ${x}`,
    );
  }
}
export class ERR_INVALID_FILE_URL_PATH extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_INVALID_FILE_URL_PATH",
      `File URL path ${x}`,
    );
  }
}
export class ERR_INVALID_HANDLE_TYPE extends NodeTypeError {
  constructor() {
    super(
      "ERR_INVALID_HANDLE_TYPE",
      `This handle type cannot be sent`,
    );
  }
}
export class ERR_INVALID_HTTP_TOKEN extends NodeTypeError {
  constructor(x: string, y: string) {
    super(
      "ERR_INVALID_HTTP_TOKEN",
      `${x} must be a valid HTTP token ["${y}"]`,
    );
  }
}
export class ERR_INVALID_IP_ADDRESS extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_INVALID_IP_ADDRESS",
      `Invalid IP address: ${x}`,
    );
  }
}
export class ERR_INVALID_OPT_VALUE_ENCODING extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_INVALID_OPT_VALUE_ENCODING",
      `The value "${x}" is invalid for option "encoding"`,
    );
  }
}
export class ERR_INVALID_PERFORMANCE_MARK extends NodeError {
  constructor(x: string) {
    super(
      "ERR_INVALID_PERFORMANCE_MARK",
      `The "${x}" performance mark has not been set`,
    );
  }
}
export class ERR_INVALID_PROTOCOL extends NodeTypeError {
  constructor(x: string, y: string) {
    super(
      "ERR_INVALID_PROTOCOL",
      `Protocol "${x}" not supported. Expected "${y}"`,
    );
  }
}
export class ERR_INVALID_REPL_EVAL_CONFIG extends NodeTypeError {
  constructor() {
    super(
      "ERR_INVALID_REPL_EVAL_CONFIG",
      `Cannot specify both "breakEvalOnSigint" and "eval" for REPL`,
    );
  }
}
export class ERR_INVALID_REPL_INPUT extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_INVALID_REPL_INPUT",
      `${x}`,
    );
  }
}
export class ERR_INVALID_SYNC_FORK_INPUT extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_INVALID_SYNC_FORK_INPUT",
      `Asynchronous forks do not support Buffer, TypedArray, DataView or string input: ${x}`,
    );
  }
}
export class ERR_INVALID_THIS extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_INVALID_THIS",
      `Value of "this" must be of type ${x}`,
    );
  }
}
export class ERR_INVALID_TUPLE extends NodeTypeError {
  constructor(x: string, y: string) {
    super(
      "ERR_INVALID_TUPLE",
      `${x} must be an iterable ${y} tuple`,
    );
  }
}
export class ERR_INVALID_URI extends NodeURIError {
  constructor() {
    super(
      "ERR_INVALID_URI",
      `URI malformed`,
    );
  }
}
export class ERR_IPC_CHANNEL_CLOSED extends NodeError {
  constructor() {
    super(
      "ERR_IPC_CHANNEL_CLOSED",
      `Channel closed`,
    );
  }
}
export class ERR_IPC_DISCONNECTED extends NodeError {
  constructor() {
    super(
      "ERR_IPC_DISCONNECTED",
      `IPC channel is already disconnected`,
    );
  }
}
export class ERR_IPC_ONE_PIPE extends NodeError {
  constructor() {
    super(
      "ERR_IPC_ONE_PIPE",
      `Child process can have only one IPC pipe`,
    );
  }
}
export class ERR_IPC_SYNC_FORK extends NodeError {
  constructor() {
    super(
      "ERR_IPC_SYNC_FORK",
      `IPC cannot be used with synchronous forks`,
    );
  }
}
export class ERR_MANIFEST_DEPENDENCY_MISSING extends NodeError {
  constructor(x: string, y: string) {
    super(
      "ERR_MANIFEST_DEPENDENCY_MISSING",
      `Manifest resource ${x} does not list ${y} as a dependency specifier`,
    );
  }
}
export class ERR_MANIFEST_INTEGRITY_MISMATCH extends NodeSyntaxError {
  constructor(x: string) {
    super(
      "ERR_MANIFEST_INTEGRITY_MISMATCH",
      `Manifest resource ${x} has multiple entries but integrity lists do not match`,
    );
  }
}
export class ERR_MANIFEST_INVALID_RESOURCE_FIELD extends NodeTypeError {
  constructor(x: string, y: string) {
    super(
      "ERR_MANIFEST_INVALID_RESOURCE_FIELD",
      `Manifest resource ${x} has invalid property value for ${y}`,
    );
  }
}
export class ERR_MANIFEST_TDZ extends NodeError {
  constructor() {
    super(
      "ERR_MANIFEST_TDZ",
      `Manifest initialization has not yet run`,
    );
  }
}
export class ERR_MANIFEST_UNKNOWN_ONERROR extends NodeSyntaxError {
  constructor(x: string) {
    super(
      "ERR_MANIFEST_UNKNOWN_ONERROR",
      `Manifest specified unknown error behavior "${x}".`,
    );
  }
}
export class ERR_METHOD_NOT_IMPLEMENTED extends NodeError {
  constructor(x: string) {
    super(
      "ERR_METHOD_NOT_IMPLEMENTED",
      `The ${x} method is not implemented`,
    );
  }
}
export class ERR_MISSING_ARGS extends NodeTypeError {
  constructor(...args: string[]) {
    args = args.map((a) => `"${a}"`);

    let msg = "The ";
    switch (args.length) {
      case 1:
        msg += `${args[0]} argument`;
        break;
      case 2:
        msg += `${args[0]} and ${args[1]} arguments`;
        break;
      default:
        msg += args.slice(0, args.length - 1).join(", ");
        msg += `, and ${args[args.length - 1]} arguments`;
        break;
    }
    super(
      "ERR_MISSING_ARGS",
      `${msg} must be specified`,
    );
  }
}
export class ERR_MISSING_OPTION extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_MISSING_OPTION",
      `${x} is required`,
    );
  }
}
export class ERR_MULTIPLE_CALLBACK extends NodeError {
  constructor() {
    super(
      "ERR_MULTIPLE_CALLBACK",
      `Callback called multiple times`,
    );
  }
}
export class ERR_NAPI_CONS_FUNCTION extends NodeTypeError {
  constructor() {
    super(
      "ERR_NAPI_CONS_FUNCTION",
      `Constructor must be a function`,
    );
  }
}
export class ERR_NAPI_INVALID_DATAVIEW_ARGS extends NodeRangeError {
  constructor() {
    super(
      "ERR_NAPI_INVALID_DATAVIEW_ARGS",
      `byte_offset + byte_length should be less than or equal to the size in bytes of the array passed in`,
    );
  }
}
export class ERR_NAPI_INVALID_TYPEDARRAY_ALIGNMENT extends NodeRangeError {
  constructor(x: string, y: string) {
    super(
      "ERR_NAPI_INVALID_TYPEDARRAY_ALIGNMENT",
      `start offset of ${x} should be a multiple of ${y}`,
    );
  }
}
export class ERR_NAPI_INVALID_TYPEDARRAY_LENGTH extends NodeRangeError {
  constructor() {
    super(
      "ERR_NAPI_INVALID_TYPEDARRAY_LENGTH",
      `Invalid typed array length`,
    );
  }
}
export class ERR_NO_CRYPTO extends NodeError {
  constructor() {
    super(
      "ERR_NO_CRYPTO",
      `Node.js is not compiled with OpenSSL crypto support`,
    );
  }
}
export class ERR_NO_ICU extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_NO_ICU",
      `${x} is not supported on Node.js compiled without ICU`,
    );
  }
}
export class ERR_QUICCLIENTSESSION_FAILED extends NodeError {
  constructor(x: string) {
    super(
      "ERR_QUICCLIENTSESSION_FAILED",
      `Failed to create a new QuicClientSession: ${x}`,
    );
  }
}
export class ERR_QUICCLIENTSESSION_FAILED_SETSOCKET extends NodeError {
  constructor() {
    super(
      "ERR_QUICCLIENTSESSION_FAILED_SETSOCKET",
      `Failed to set the QuicSocket`,
    );
  }
}
export class ERR_QUICSESSION_DESTROYED extends NodeError {
  constructor(x: string) {
    super(
      "ERR_QUICSESSION_DESTROYED",
      `Cannot call ${x} after a QuicSession has been destroyed`,
    );
  }
}
export class ERR_QUICSESSION_INVALID_DCID extends NodeError {
  constructor(x: string) {
    super(
      "ERR_QUICSESSION_INVALID_DCID",
      `Invalid DCID value: ${x}`,
    );
  }
}
export class ERR_QUICSESSION_UPDATEKEY extends NodeError {
  constructor() {
    super(
      "ERR_QUICSESSION_UPDATEKEY",
      `Unable to update QuicSession keys`,
    );
  }
}
export class ERR_QUICSOCKET_DESTROYED extends NodeError {
  constructor(x: string) {
    super(
      "ERR_QUICSOCKET_DESTROYED",
      `Cannot call ${x} after a QuicSocket has been destroyed`,
    );
  }
}
export class ERR_QUICSOCKET_INVALID_STATELESS_RESET_SECRET_LENGTH
  extends NodeError {
  constructor() {
    super(
      "ERR_QUICSOCKET_INVALID_STATELESS_RESET_SECRET_LENGTH",
      `The stateResetToken must be exactly 16-bytes in length`,
    );
  }
}
export class ERR_QUICSOCKET_LISTENING extends NodeError {
  constructor() {
    super(
      "ERR_QUICSOCKET_LISTENING",
      `This QuicSocket is already listening`,
    );
  }
}
export class ERR_QUICSOCKET_UNBOUND extends NodeError {
  constructor(x: string) {
    super(
      "ERR_QUICSOCKET_UNBOUND",
      `Cannot call ${x} before a QuicSocket has been bound`,
    );
  }
}
export class ERR_QUICSTREAM_DESTROYED extends NodeError {
  constructor(x: string) {
    super(
      "ERR_QUICSTREAM_DESTROYED",
      `Cannot call ${x} after a QuicStream has been destroyed`,
    );
  }
}
export class ERR_QUICSTREAM_INVALID_PUSH extends NodeError {
  constructor() {
    super(
      "ERR_QUICSTREAM_INVALID_PUSH",
      `Push streams are only supported on client-initiated, bidirectional streams`,
    );
  }
}
export class ERR_QUICSTREAM_OPEN_FAILED extends NodeError {
  constructor() {
    super(
      "ERR_QUICSTREAM_OPEN_FAILED",
      `Opening a new QuicStream failed`,
    );
  }
}
export class ERR_QUICSTREAM_UNSUPPORTED_PUSH extends NodeError {
  constructor() {
    super(
      "ERR_QUICSTREAM_UNSUPPORTED_PUSH",
      `Push streams are not supported on this QuicSession`,
    );
  }
}
export class ERR_QUIC_TLS13_REQUIRED extends NodeError {
  constructor() {
    super(
      "ERR_QUIC_TLS13_REQUIRED",
      `QUIC requires TLS version 1.3`,
    );
  }
}
export class ERR_SCRIPT_EXECUTION_INTERRUPTED extends NodeError {
  constructor() {
    super(
      "ERR_SCRIPT_EXECUTION_INTERRUPTED",
      "Script execution was interrupted by `SIGINT`",
    );
  }
}
export class ERR_SERVER_ALREADY_LISTEN extends NodeError {
  constructor() {
    super(
      "ERR_SERVER_ALREADY_LISTEN",
      `Listen method has been called more than once without closing.`,
    );
  }
}
export class ERR_SERVER_NOT_RUNNING extends NodeError {
  constructor() {
    super(
      "ERR_SERVER_NOT_RUNNING",
      `Server is not running.`,
    );
  }
}
export class ERR_SOCKET_ALREADY_BOUND extends NodeError {
  constructor() {
    super(
      "ERR_SOCKET_ALREADY_BOUND",
      `Socket is already bound`,
    );
  }
}
export class ERR_SOCKET_BAD_BUFFER_SIZE extends NodeTypeError {
  constructor() {
    super(
      "ERR_SOCKET_BAD_BUFFER_SIZE",
      `Buffer size must be a positive integer`,
    );
  }
}
export class ERR_SOCKET_BAD_TYPE extends NodeTypeError {
  constructor() {
    super(
      "ERR_SOCKET_BAD_TYPE",
      `Bad socket type specified. Valid types are: udp4, udp6`,
    );
  }
}
export class ERR_SOCKET_CLOSED extends NodeError {
  constructor() {
    super(
      "ERR_SOCKET_CLOSED",
      `Socket is closed`,
    );
  }
}
export class ERR_SOCKET_DGRAM_IS_CONNECTED extends NodeError {
  constructor() {
    super(
      "ERR_SOCKET_DGRAM_IS_CONNECTED",
      `Already connected`,
    );
  }
}
export class ERR_SOCKET_DGRAM_NOT_CONNECTED extends NodeError {
  constructor() {
    super(
      "ERR_SOCKET_DGRAM_NOT_CONNECTED",
      `Not connected`,
    );
  }
}
export class ERR_SOCKET_DGRAM_NOT_RUNNING extends NodeError {
  constructor() {
    super(
      "ERR_SOCKET_DGRAM_NOT_RUNNING",
      `Not running`,
    );
  }
}
export class ERR_SRI_PARSE extends NodeSyntaxError {
  constructor(name: string, char: string, position: number) {
    super(
      "ERR_SRI_PARSE",
      `Subresource Integrity string ${name} had an unexpected ${char} at position ${position}`,
    );
  }
}
export class ERR_STREAM_ALREADY_FINISHED extends NodeError {
  constructor(x: string) {
    super(
      "ERR_STREAM_ALREADY_FINISHED",
      `Cannot call ${x} after a stream was finished`,
    );
  }
}
export class ERR_STREAM_CANNOT_PIPE extends NodeError {
  constructor() {
    super(
      "ERR_STREAM_CANNOT_PIPE",
      `Cannot pipe, not readable`,
    );
  }
}
export class ERR_STREAM_DESTROYED extends NodeError {
  constructor(x: string) {
    super(
      "ERR_STREAM_DESTROYED",
      `Cannot call ${x} after a stream was destroyed`,
    );
  }
}
export class ERR_STREAM_NULL_VALUES extends NodeTypeError {
  constructor() {
    super(
      "ERR_STREAM_NULL_VALUES",
      `May not write null values to stream`,
    );
  }
}
export class ERR_STREAM_PREMATURE_CLOSE extends NodeError {
  constructor() {
    super(
      "ERR_STREAM_PREMATURE_CLOSE",
      `Premature close`,
    );
  }
}
export class ERR_STREAM_PUSH_AFTER_EOF extends NodeError {
  constructor() {
    super(
      "ERR_STREAM_PUSH_AFTER_EOF",
      `stream.push() after EOF`,
    );
  }
}
export class ERR_STREAM_UNSHIFT_AFTER_END_EVENT extends NodeError {
  constructor() {
    super(
      "ERR_STREAM_UNSHIFT_AFTER_END_EVENT",
      `stream.unshift() after end event`,
    );
  }
}
export class ERR_STREAM_WRAP extends NodeError {
  constructor() {
    super(
      "ERR_STREAM_WRAP",
      `Stream has StringDecoder set or is in objectMode`,
    );
  }
}
export class ERR_STREAM_WRITE_AFTER_END extends NodeError {
  constructor() {
    super(
      "ERR_STREAM_WRITE_AFTER_END",
      `write after end`,
    );
  }
}
export class ERR_SYNTHETIC extends NodeError {
  constructor() {
    super(
      "ERR_SYNTHETIC",
      `JavaScript Callstack`,
    );
  }
}
export class ERR_TLS_DH_PARAM_SIZE extends NodeError {
  constructor(x: string) {
    super(
      "ERR_TLS_DH_PARAM_SIZE",
      `DH parameter size ${x} is less than 2048`,
    );
  }
}
export class ERR_TLS_HANDSHAKE_TIMEOUT extends NodeError {
  constructor() {
    super(
      "ERR_TLS_HANDSHAKE_TIMEOUT",
      `TLS handshake timeout`,
    );
  }
}
export class ERR_TLS_INVALID_CONTEXT extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_TLS_INVALID_CONTEXT",
      `${x} must be a SecureContext`,
    );
  }
}
export class ERR_TLS_INVALID_STATE extends NodeError {
  constructor() {
    super(
      "ERR_TLS_INVALID_STATE",
      `TLS socket connection must be securely established`,
    );
  }
}
export class ERR_TLS_INVALID_PROTOCOL_VERSION extends NodeTypeError {
  constructor(protocol: string, x: string) {
    super(
      "ERR_TLS_INVALID_PROTOCOL_VERSION",
      `${protocol} is not a valid ${x} TLS protocol version`,
    );
  }
}
export class ERR_TLS_PROTOCOL_VERSION_CONFLICT extends NodeTypeError {
  constructor(prevProtocol: string, protocol: string) {
    super(
      "ERR_TLS_PROTOCOL_VERSION_CONFLICT",
      `TLS protocol version ${prevProtocol} conflicts with secureProtocol ${protocol}`,
    );
  }
}
export class ERR_TLS_RENEGOTIATION_DISABLED extends NodeError {
  constructor() {
    super(
      "ERR_TLS_RENEGOTIATION_DISABLED",
      `TLS session renegotiation disabled for this socket`,
    );
  }
}
export class ERR_TLS_REQUIRED_SERVER_NAME extends NodeError {
  constructor() {
    super(
      "ERR_TLS_REQUIRED_SERVER_NAME",
      `"servername" is required parameter for Server.addContext`,
    );
  }
}
export class ERR_TLS_SESSION_ATTACK extends NodeError {
  constructor() {
    super(
      "ERR_TLS_SESSION_ATTACK",
      `TLS session renegotiation attack detected`,
    );
  }
}
export class ERR_TLS_SNI_FROM_SERVER extends NodeError {
  constructor() {
    super(
      "ERR_TLS_SNI_FROM_SERVER",
      `Cannot issue SNI from a TLS server-side socket`,
    );
  }
}
export class ERR_TRACE_EVENTS_CATEGORY_REQUIRED extends NodeTypeError {
  constructor() {
    super(
      "ERR_TRACE_EVENTS_CATEGORY_REQUIRED",
      `At least one category is required`,
    );
  }
}
export class ERR_TRACE_EVENTS_UNAVAILABLE extends NodeError {
  constructor() {
    super(
      "ERR_TRACE_EVENTS_UNAVAILABLE",
      `Trace events are unavailable`,
    );
  }
}
export class ERR_UNAVAILABLE_DURING_EXIT extends NodeError {
  constructor() {
    super(
      "ERR_UNAVAILABLE_DURING_EXIT",
      `Cannot call function in process exit handler`,
    );
  }
}
export class ERR_UNCAUGHT_EXCEPTION_CAPTURE_ALREADY_SET extends NodeError {
  constructor() {
    super(
      "ERR_UNCAUGHT_EXCEPTION_CAPTURE_ALREADY_SET",
      "`process.setupUncaughtExceptionCapture()` was called while a capture callback was already active",
    );
  }
}
export class ERR_UNESCAPED_CHARACTERS extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_UNESCAPED_CHARACTERS",
      `${x} contains unescaped characters`,
    );
  }
}
export class ERR_UNKNOWN_BUILTIN_MODULE extends NodeError {
  constructor(x: string) {
    super(
      "ERR_UNKNOWN_BUILTIN_MODULE",
      `No such built-in module: ${x}`,
    );
  }
}
export class ERR_UNKNOWN_CREDENTIAL extends NodeError {
  constructor(x: string, y: string) {
    super(
      "ERR_UNKNOWN_CREDENTIAL",
      `${x} identifier does not exist: ${y}`,
    );
  }
}
export class ERR_UNKNOWN_ENCODING extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_UNKNOWN_ENCODING",
      `Unknown encoding: ${x}`,
    );
  }
}
export class ERR_UNKNOWN_FILE_EXTENSION extends NodeTypeError {
  constructor(x: string, y: string) {
    super(
      "ERR_UNKNOWN_FILE_EXTENSION",
      `Unknown file extension "${x}" for ${y}`,
    );
  }
}
export class ERR_UNKNOWN_MODULE_FORMAT extends NodeRangeError {
  constructor(x: string) {
    super(
      "ERR_UNKNOWN_MODULE_FORMAT",
      `Unknown module format: ${x}`,
    );
  }
}
export class ERR_UNKNOWN_SIGNAL extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_UNKNOWN_SIGNAL",
      `Unknown signal: ${x}`,
    );
  }
}
export class ERR_UNSUPPORTED_DIR_IMPORT extends NodeError {
  constructor(x: string, y: string) {
    super(
      "ERR_UNSUPPORTED_DIR_IMPORT",
      `Directory import '${x}' is not supported resolving ES modules, imported from ${y}`,
    );
  }
}
export class ERR_UNSUPPORTED_ESM_URL_SCHEME extends NodeError {
  constructor() {
    super(
      "ERR_UNSUPPORTED_ESM_URL_SCHEME",
      `Only file and data URLs are supported by the default ESM loader`,
    );
  }
}
export class ERR_V8BREAKITERATOR extends NodeError {
  constructor() {
    super(
      "ERR_V8BREAKITERATOR",
      `Full ICU data not installed. See https://github.com/nodejs/node/wiki/Intl`,
    );
  }
}
export class ERR_VALID_PERFORMANCE_ENTRY_TYPE extends NodeError {
  constructor() {
    super(
      "ERR_VALID_PERFORMANCE_ENTRY_TYPE",
      `At least one valid performance entry type is required`,
    );
  }
}
export class ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING extends NodeTypeError {
  constructor() {
    super(
      "ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING",
      `A dynamic import callback was not specified.`,
    );
  }
}
export class ERR_VM_MODULE_ALREADY_LINKED extends NodeError {
  constructor() {
    super(
      "ERR_VM_MODULE_ALREADY_LINKED",
      `Module has already been linked`,
    );
  }
}
export class ERR_VM_MODULE_CANNOT_CREATE_CACHED_DATA extends NodeError {
  constructor() {
    super(
      "ERR_VM_MODULE_CANNOT_CREATE_CACHED_DATA",
      `Cached data cannot be created for a module which has been evaluated`,
    );
  }
}
export class ERR_VM_MODULE_DIFFERENT_CONTEXT extends NodeError {
  constructor() {
    super(
      "ERR_VM_MODULE_DIFFERENT_CONTEXT",
      `Linked modules must use the same context`,
    );
  }
}
export class ERR_VM_MODULE_LINKING_ERRORED extends NodeError {
  constructor() {
    super(
      "ERR_VM_MODULE_LINKING_ERRORED",
      `Linking has already failed for the provided module`,
    );
  }
}
export class ERR_VM_MODULE_NOT_MODULE extends NodeError {
  constructor() {
    super(
      "ERR_VM_MODULE_NOT_MODULE",
      `Provided module is not an instance of Module`,
    );
  }
}
export class ERR_VM_MODULE_STATUS extends NodeError {
  constructor(x: string) {
    super(
      "ERR_VM_MODULE_STATUS",
      `Module status ${x}`,
    );
  }
}
export class ERR_WASI_ALREADY_STARTED extends NodeError {
  constructor() {
    super(
      "ERR_WASI_ALREADY_STARTED",
      `WASI instance has already started`,
    );
  }
}
export class ERR_WORKER_INIT_FAILED extends NodeError {
  constructor(x: string) {
    super(
      "ERR_WORKER_INIT_FAILED",
      `Worker initialization failure: ${x}`,
    );
  }
}
export class ERR_WORKER_NOT_RUNNING extends NodeError {
  constructor() {
    super(
      "ERR_WORKER_NOT_RUNNING",
      `Worker instance not running`,
    );
  }
}
export class ERR_WORKER_OUT_OF_MEMORY extends NodeError {
  constructor(x: string) {
    super(
      "ERR_WORKER_OUT_OF_MEMORY",
      `Worker terminated due to reaching memory limit: ${x}`,
    );
  }
}
export class ERR_WORKER_UNSERIALIZABLE_ERROR extends NodeError {
  constructor() {
    super(
      "ERR_WORKER_UNSERIALIZABLE_ERROR",
      `Serializing an uncaught exception failed`,
    );
  }
}
export class ERR_WORKER_UNSUPPORTED_EXTENSION extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_WORKER_UNSUPPORTED_EXTENSION",
      `The worker script extension must be ".js", ".mjs", or ".cjs". Received "${x}"`,
    );
  }
}
export class ERR_WORKER_UNSUPPORTED_OPERATION extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_WORKER_UNSUPPORTED_OPERATION",
      `${x} is not supported in workers`,
    );
  }
}
export class ERR_ZLIB_INITIALIZATION_FAILED extends NodeError {
  constructor() {
    super(
      "ERR_ZLIB_INITIALIZATION_FAILED",
      `Initialization failed`,
    );
  }
}
export class ERR_FALSY_VALUE_REJECTION extends NodeError {
  reason: string;
  constructor(reason: string) {
    super(
      "ERR_FALSY_VALUE_REJECTION",
      "Promise was rejected with falsy value",
    );
    this.reason = reason;
  }
}
export class ERR_HTTP2_INVALID_SETTING_VALUE extends NodeRangeError {
  actual: unknown;
  min?: number;
  max?: number;

  constructor(name: string, actual: unknown, min?: number, max?: number) {
    super(
      "ERR_HTTP2_INVALID_SETTING_VALUE",
      `Invalid value for setting "${name}": ${actual}`,
    );
    this.actual = actual;
    if (min !== undefined) {
      this.min = min;
      this.max = max;
    }
  }
}
export class ERR_HTTP2_STREAM_CANCEL extends NodeError {
  cause?: Error;
  constructor(error: Error) {
    super(
      "ERR_HTTP2_STREAM_CANCEL",
      typeof error.message === "string"
        ? `The pending stream has been canceled (caused by: ${error.message})`
        : "The pending stream has been canceled",
    );
    if (error) {
      this.cause = error;
    }
  }
}

export class ERR_INVALID_ADDRESS_FAMILY extends NodeRangeError {
  host: string;
  port: number;
  constructor(addressType: string, host: string, port: number) {
    super(
      "ERR_INVALID_ADDRESS_FAMILY",
      `Invalid address family: ${addressType} ${host}:${port}`,
    );
    this.host = host;
    this.port = port;
  }
}

export class ERR_INVALID_CHAR extends NodeTypeError {
  constructor(name: string, field?: string) {
    super(
      "ERR_INVALID_CHAR",
      field
        ? `Invalid character in ${name}`
        : `Invalid character in ${name} ["${field}"]`,
    );
  }
}

export class ERR_INVALID_OPT_VALUE extends NodeTypeError {
  constructor(name: string, value: unknown) {
    super(
      "ERR_INVALID_OPT_VALUE",
      `The value "${value}" is invalid for option "${name}"`,
    );
  }
}

export class ERR_INVALID_RETURN_PROPERTY extends NodeTypeError {
  constructor(input: string, name: string, prop: string, value: string) {
    super(
      "ERR_INVALID_RETURN_PROPERTY",
      `Expected a valid ${input} to be returned for the "${prop}" from the "${name}" function but got ${value}.`,
    );
  }
}

// deno-lint-ignore no-explicit-any
function buildReturnPropertyType(value: any) {
  if (value && value.constructor && value.constructor.name) {
    return `instance of ${value.constructor.name}`;
  } else {
    return `type ${typeof value}`;
  }
}

export class ERR_INVALID_RETURN_PROPERTY_VALUE extends NodeTypeError {
  constructor(input: string, name: string, prop: string, value: unknown) {
    super(
      "ERR_INVALID_RETURN_PROPERTY_VALUE",
      `Expected ${input} to be returned for the "${prop}" from the "${name}" function but got ${
        buildReturnPropertyType(value)
      }.`,
    );
  }
}

export class ERR_INVALID_RETURN_VALUE extends NodeTypeError {
  constructor(input: string, name: string, value: unknown) {
    super(
      "ERR_INVALID_RETURN_VALUE",
      `Expected ${input} to be returned from the "${name}" function but got ${
        buildReturnPropertyType(value)
      }.`,
    );
  }
}

export class ERR_INVALID_URL extends NodeTypeError {
  input: string;
  constructor(input: string) {
    super(
      "ERR_INVALID_URL",
      `Invalid URL: ${input}`,
    );
    this.input = input;
  }
}
