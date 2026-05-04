// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Node.js contributors. All rights reserved. MIT License.
// deno-fmt-ignore-file

/** NOT IMPLEMENTED
 * ERR_MANIFEST_ASSERT_INTEGRITY
 * ERR_QUICSESSION_VERSION_NEGOTIATION
 * ERR_REQUIRE_ESM
 * ERR_QUIC_ERROR
 * ERR_SYSTEM_ERROR //System error, shouldn't ever happen inside Deno
 * ERR_TTY_INIT_FAILED //System error, shouldn't ever happen inside Deno
 * ERR_INVALID_PACKAGE_CONFIG // package.json stuff, probably useless
 */

(function () {
const { core, primordials } = globalThis.__bootstrap;
const {
  AggregateError,
  ArrayIsArray,
  ArrayPrototypeIncludes,
  ArrayPrototypeIndexOf,
  ArrayPrototypeMap,
  ArrayPrototypeJoin,
  ArrayPrototypePush,
  ArrayPrototypePop,
  ArrayPrototypeSlice,
  ArrayPrototypeSplice,
  Error,
  ErrorPrototype,
  ErrorCaptureStackTrace,
  JSONStringify,
  MapPrototypeGet,
  MathAbs,
  NumberIsInteger,
  ObjectAssign,
  ObjectDefineProperty,
  ObjectDefineProperties,
  ObjectGetOwnPropertyDescriptor,
  ObjectIsExtensible,
  ObjectKeys,
  ObjectPrototypeHasOwnProperty,
  ObjectPrototypeIsPrototypeOf,
  ObjectSetPrototypeOf,
  RangeErrorPrototype,
  RegExpPrototypeTest,
  SafeArrayIterator,
  SafeRegExp,
  String,
  StringPrototypeEndsWith,
  StringPrototypeIncludes,
  StringPrototypeMatch,
  StringPrototypeSlice,
  StringPrototypeStartsWith,
  StringPrototypeToLocaleLowerCase,
  StringPrototypeToLowerCase,
  StringPrototypeToString,
  Symbol,
  SymbolFor,
  SymbolPrototypeToString,
  SyntaxError,
  SyntaxErrorPrototype,
  TypeError,
  TypeErrorPrototype,
  URIError,
  URIErrorPrototype,
} = primordials;
const { format, inspect } = core.loadExtScript("ext:deno_node/internal/util/inspect.mjs");
const { codes } = core.loadExtScript("ext:deno_node/internal/error_codes.ts");
const {
  codeMap,
  errorMap,
  mapSysErrnoToUvErrno,
  UV_EBADF,
} = core.loadExtScript("ext:deno_node/internal_binding/uv.ts");
const { isWindows } = core.loadExtScript("ext:deno_node/_util/os.ts");
const { os: osConstants } = core.loadExtScript("ext:deno_node/internal_binding/constants.ts");
const { hideStackFrames } = core.loadExtScript(
  "ext:deno_node/internal/hide_stack_frames.ts",
);

// Lazy loader for getSystemErrorName to break circular dep with _utils.ts
let _getSystemErrorName;
function getSystemErrorName(code) {
  if (!_getSystemErrorName) {
    _getSystemErrorName = core.loadExtScript("ext:deno_node/_utils.ts").getSystemErrorName;
  }
  return _getSystemErrorName(code);
}

let assert;
const lazyLoadAssert = () => {
  return core.createLazyLoader(
    "node:assert",
  )().default;
};

const kIsNodeError = Symbol("kIsNodeError");

/**
 * @see https://github.com/nodejs/node/blob/f3eb224/lib/internal/errors.js
 */
const classRegExp = new SafeRegExp(/^([A-Z][a-z0-9]*)+$/);

/**
 * @see https://github.com/nodejs/node/blob/f3eb224/lib/internal/errors.js
 * @description Sorted by a rough estimate on most frequently used entries.
 */
const kTypes = [
  "string",
  "function",
  "number",
  "object",
  // Accept 'Function' and 'Object' as alternative to the lower cased version.
  "Function",
  "Object",
  "boolean",
  "bigint",
  "symbol",
];

// Node uses an AbortError that isn't exactly the same as the DOMException
// to make usage of the error in userland and readable-stream easier.
// It is a regular error with `.code` and `.name`.
class AbortError extends Error {
  code: string;

  constructor(message = "The operation was aborted", options?: ErrorOptions) {
    if (options !== undefined && typeof options !== "object") {
      throw new codes.ERR_INVALID_ARG_TYPE("options", "Object", options);
    }
    super(message, options);
    this.code = "ABORT_ERR";
    this.name = "AbortError";
  }
}

let maxStackErrorName: string | undefined;
let maxStackErrorMessage: string | undefined;
/**
 * Returns true if `err.name` and `err.message` are equal to engine-specific
 * values indicating max call stack size has been exceeded.
 * "Maximum call stack size exceeded" in V8.
 */
function isStackOverflowError(err: Error): boolean {
  if (maxStackErrorMessage === undefined) {
    try {
      // deno-lint-ignore no-inner-declarations
      function overflowStack() {
        overflowStack();
      }
      overflowStack();
      // deno-lint-ignore no-explicit-any
    } catch (err: any) {
      maxStackErrorMessage = err.message;
      maxStackErrorName = err.name;
    }
  }

  return err && err.name === maxStackErrorName &&
    err.message === maxStackErrorMessage;
}

function isErrorStackTraceLimitWritable(): boolean {
  const desc = ObjectGetOwnPropertyDescriptor(Error, "stackTraceLimit");
  if (desc === undefined) {
    return ObjectIsExtensible(Error);
  }

  return ObjectPrototypeHasOwnProperty(desc, "writable")
    ? desc.writable
    : desc.set !== undefined;
}

function addNumericalSeparator(val: string) {
  let res = "";
  let i = val.length;
  const start = val[0] === "-" ? 1 : 0;
  for (; i >= start + 4; i -= 3) {
    res = `_${StringPrototypeSlice(val, i - 3, i)}${res}`;
  }
  return `${StringPrototypeSlice(val, 0, i)}${res}`;
}

interface ErrnoException extends Error {
  errno?: number;
  code?: string;
  path?: string;
  syscall?: string;
  spawnargs?: string[];
}

/**
 * This creates an error compatible with errors produced in the C++
 * This function should replace the deprecated
 * `exceptionWithHostPort()` function.
 *
 * @param err A libuv error number
 * @param syscall
 * @param address
 * @param port
 * @return The error.
 */
const uvExceptionWithHostPort = hideStackFrames(
  function uvExceptionWithHostPort(
    err: number,
    syscall: string,
    address?: string | null,
    port?: number | null,
  ) {
    const { 0: code, 1: uvmsg } = uvErrmapGet(err) || uvUnmappedError;
    const message = `${syscall} ${code}: ${uvmsg}`;
    let details = "";

    if (port && port > 0) {
      details = ` ${address}:${port}`;
    } else if (address) {
      details = ` ${address}`;
    }

    // deno-lint-ignore no-explicit-any
    const ex: any = new Error(`${message}${details}`);
    ex.code = code;
    ex.errno = err;
    ex.syscall = syscall;
    ex.address = address;

    if (port) {
      ex.port = port;
    }

    return ex;
  },
);

/**
 * This used to be `util._errnoException()`.
 *
 * @param err A libuv error number
 * @param syscall
 * @param original
 * @return A `ErrnoException`
 */
const errnoException = hideStackFrames(function errnoException(
  err,
  syscall,
  original?,
): ErrnoException {
  const code = getSystemErrorName(err);
  const message = original
    ? `${syscall} ${code} ${original}`
    : `${syscall} ${code}`;

  // deno-lint-ignore no-explicit-any
  const ex: any = new Error(message);
  ex.errno = err;
  ex.code = code;
  ex.syscall = syscall;

  return ex;
});

function uvErrmapGet(name: number) {
  return MapPrototypeGet(errorMap, name);
}

const uvUnmappedError = ["UNKNOWN", "unknown error"];

/**
 * This creates an error compatible with errors produced in the C++
 * function UVException using a context object with data assembled in C++.
 * The goal is to migrate them to ERR_* errors later when compatibility is
 * not a concern.
 *
 * @param ctx
 * @return The error.
 */
const uvException = hideStackFrames(function uvException(ctx) {
  const { 0: code, 1: uvmsg } = uvErrmapGet(ctx.errno) || uvUnmappedError;

  let message = `${code}: ${ctx.message || uvmsg}, ${ctx.syscall}`;

  let path;
  let dest;

  if (ctx.path) {
    path = StringPrototypeToString(ctx.path);
    message += ` '${path}'`;
  }
  if (ctx.dest) {
    dest = StringPrototypeToString(ctx.dest);
    message += ` -> '${dest}'`;
  }

  // deno-lint-ignore no-explicit-any
  const err: any = new Error(message);

  for (const prop of new SafeArrayIterator(ObjectKeys(ctx))) {
    if (prop === "message" || prop === "path" || prop === "dest") {
      continue;
    }

    err[prop] = ctx[prop];
  }

  err.code = code;

  if (path) {
    err.path = path;
  }

  if (dest) {
    err.dest = dest;
  }

  return err;
});

/**
 * Deprecated, new function is `uvExceptionWithHostPort()`
 * New function added the error description directly
 * from C++. this method for backwards compatibility
 * @param err A libuv error number
 * @param syscall
 * @param address
 * @param port
 * @param additional
 */
const exceptionWithHostPort = hideStackFrames(
  function exceptionWithHostPort(
    err: number,
    syscall: string,
    address: string,
    port: number,
    additional?: string,
  ) {
    const code = getSystemErrorName(err);
    let details = "";

    if (port && port > 0) {
      details = ` ${address}:${port}`;
    } else if (address) {
      details = ` ${address}`;
    }

    if (additional) {
      details += ` - Local (${additional})`;
    }

    // deno-lint-ignore no-explicit-any
    const ex: any = new Error(`${syscall} ${code}${details}`);
    ex.errno = err;
    ex.code = code;
    ex.syscall = syscall;
    ex.address = address;

    if (port) {
      ex.port = port;
    }

    return ex;
  },
);

const handleDnsError = hideStackFrames(
  (err: Error, syscall: string, address: string) => {
    //@ts-expect-error code is safe to access with optional chaining
    if (typeof err?.uv_errcode === "number") {
      //@ts-expect-error code is safe to access with optional chaining
      return dnsException(err?.uv_errcode, syscall, address);
    }

    if (ObjectPrototypeIsPrototypeOf(Deno.errors.NotCapable.prototype, err)) {
      return dnsException(codeMap.get("EPERM")!, syscall, address);
    }

    return denoErrorToNodeError(err, { syscall });
  },
);

/**
 * @param code A libuv error number or a c-ares error code
 * @param syscall
 * @param hostname
 */
const dnsException = hideStackFrames(function (code, syscall, hostname) {
  let errno;

  // If `code` is of type number, it is a libuv error number, else it is a
  // c-ares error code.
  if (typeof code === "number") {
    errno = code;
    // ENOTFOUND is not a proper POSIX error, but this error has been in place
    // long enough that it's not practical to remove it.
    if (
      code === MapPrototypeGet(codeMap, "EAI_NODATA") ||
      code === MapPrototypeGet(codeMap, "EAI_NONAME")
    ) {
      code = "ENOTFOUND"; // Fabricated error name.
    } else {
      code = getSystemErrorName(code);
    }
  }

  const message = `${syscall} ${code}${hostname ? ` ${hostname}` : ""}`;

  // deno-lint-ignore no-explicit-any
  const ex: any = new Error(message);
  ex.errno = errno;
  ex.code = code;
  ex.syscall = syscall;

  if (hostname) {
    ex.hostname = hostname;
  }

  ErrorCaptureStackTrace(ex, dnsException);

  return ex;
});

/**
 * All error instances in Node have additional methods and properties
 * This class is meant to be extended by these instances abstracting native JS error instances
 */
class NodeErrorAbstraction extends Error {
  code: string;

  constructor(name: string, code: string, message: string) {
    super(message);
    this.code = code;
    this.name = name;
    this.stack = this.stack &&
      `${name} [${this.code}]${
        StringPrototypeSlice(this.stack, this.name.length)
      }`;
  }

  override toString() {
    return `${this.name} [${this.code}]: ${this.message}`;
  }
}

class NodeError extends NodeErrorAbstraction {
  constructor(code: string, message: string) {
    super(Error.prototype.name, code, message);
  }
}

class NodeSyntaxError extends NodeErrorAbstraction
  implements SyntaxError {
  constructor(code: string, message: string) {
    super(SyntaxError.prototype.name, code, message);
    ObjectSetPrototypeOf(this, SyntaxErrorPrototype);
    this.toString = function () {
      return `${this.name} [${this.code}]: ${this.message}`;
    };
  }
}

class NodeRangeError extends NodeErrorAbstraction {
  constructor(code: string, message: string) {
    super(RangeError.prototype.name, code, message);
    ObjectSetPrototypeOf(this, RangeErrorPrototype);
    this.toString = function () {
      return `${this.name} [${this.code}]: ${this.message}`;
    };
  }
}

class NodeTypeError extends NodeErrorAbstraction implements TypeError {
  constructor(code: string, message: string) {
    super(TypeError.prototype.name, code, message);
    ObjectSetPrototypeOf(this, TypeErrorPrototype);
    this.toString = function () {
      return `${this.name} [${this.code}]: ${this.message}`;
    };
  }
}

class NodeURIError extends NodeErrorAbstraction implements URIError {
  constructor(code: string, message: string) {
    super(URIError.prototype.name, code, message);
    ObjectSetPrototypeOf(this, URIErrorPrototype);
    this.toString = function () {
      return `${this.name} [${this.code}]: ${this.message}`;
    };
  }
}

interface NodeSystemErrorCtx {
  code: string;
  syscall: string;
  message: string;
  errno: number;
  path?: string;
  dest?: string;
}
// A specialized Error that includes an additional info property with
// additional information about the error condition.
// It has the properties present in a UVException but with a custom error
// message followed by the uv error code and uv error message.
// It also has its own error code with the original uv error context put into
// `err.info`.
// The context passed into this error must have .code, .syscall and .message,
// and may have .path and .dest.
class NodeSystemError extends Error {
  code: string;
  constructor(key: string, context: NodeSystemErrorCtx, msgPrefix: string) {
    super();
    this.code = key;
    let message = `${msgPrefix}: ${context.syscall} returned ` +
      `${context.code} (${context.message})`;

    if (context.path !== undefined) {
      message += ` ${context.path}`;
    }
    if (context.dest !== undefined) {
      message += ` => ${context.dest}`;
    }

    ErrorCaptureStackTrace(this);

    ObjectDefineProperties(this, {
      [kIsNodeError]: {
        __proto__: null,
        value: true,
        enumerable: false,
        writable: false,
        configurable: true,
      },
      name: {
        __proto__: null,
        value: "SystemError",
        enumerable: false,
        writable: true,
        configurable: true,
      },
      message: {
        __proto__: null,
        value: message,
        enumerable: false,
        writable: true,
        configurable: true,
      },
      info: {
        __proto__: null,
        value: context,
        enumerable: true,
        configurable: true,
        writable: false,
      },
      errno: {
        __proto__: null,
        get() {
          return context.errno;
        },
        set: (value) => {
          context.errno = value;
        },
        enumerable: true,
        configurable: true,
      },
      syscall: {
        __proto__: null,
        get() {
          return context.syscall;
        },
        set: (value) => {
          context.syscall = value;
        },
        enumerable: true,
        configurable: true,
      },
    });

    if (context.path !== undefined) {
      ObjectDefineProperty(this, "path", {
        __proto__: null,
        get() {
          return context.path;
        },
        set: (value) => {
          context.path = value;
        },
        enumerable: true,
        configurable: true,
      });
    }

    if (context.dest !== undefined) {
      ObjectDefineProperty(this, "dest", {
        __proto__: null,
        get() {
          return context.dest;
        },
        set: (value) => {
          context.dest = value;
        },
        enumerable: true,
        configurable: true,
      });
    }
  }

  override toString() {
    return `${this.name} [${this.code}]: ${this.message}`;
  }

  // deno-lint-ignore no-explicit-any
  [SymbolFor("nodejs.util.inspect.custom")](_recurseTimes: number, ctx: any) {
    return inspect(this, {
      ...ctx,
      getters: true,
      customInspect: false,
    });
  }
}

function makeSystemErrorWithCode(key: string, msgPrfix: string) {
  return class NodeError extends NodeSystemError {
    constructor(ctx: NodeSystemErrorCtx) {
      super(key, ctx, msgPrfix);
    }
  };
}

const ERR_FS_CP_DIR_TO_NON_DIR = makeSystemErrorWithCode(
  "ERR_FS_CP_DIR_TO_NON_DIR",
  "Cannot overwrite non-directory with directory",
);
const ERR_FS_CP_EEXIST = makeSystemErrorWithCode(
  "ERR_FS_CP_EEXIST",
  "Target already exists",
);
const ERR_FS_CP_EINVAL = makeSystemErrorWithCode(
  "ERR_FS_CP_EINVAL",
  "Invalid src or dest",
);
const ERR_FS_CP_FIFO_PIPE = makeSystemErrorWithCode(
  "ERR_FS_CP_FIFO_PIPE",
  "Cannot copy a FIFO pipe",
);
const ERR_FS_CP_NON_DIR_TO_DIR = makeSystemErrorWithCode(
  "ERR_FS_CP_NON_DIR_TO_DIR",
  "Cannot overwrite directory with non-directory",
);
const ERR_FS_CP_SOCKET = makeSystemErrorWithCode(
  "ERR_FS_CP_SOCKET",
  "Cannot copy a socket file",
);
const ERR_FS_CP_SYMLINK_TO_SUBDIRECTORY = makeSystemErrorWithCode(
  "ERR_FS_CP_SYMLINK_TO_SUBDIRECTORY",
  "Cannot overwrite symlink in subdirectory of self",
);
const ERR_FS_CP_UNKNOWN = makeSystemErrorWithCode(
  "ERR_FS_CP_UNKNOWN",
  "Cannot copy an unknown file type",
);
const ERR_FS_EISDIR = makeSystemErrorWithCode(
  "ERR_FS_EISDIR",
  "Path is a directory",
);
const ERR_TTY_INIT_FAILED = makeSystemErrorWithCode(
  "ERR_TTY_INIT_FAILED",
  "TTY initialization failed",
);

function createInvalidArgType(
  name: string,
  expected: string | string[],
): string {
  // https://github.com/nodejs/node/blob/f3eb224/lib/internal/errors.js#L1037-L1087
  expected = ArrayIsArray(expected) ? expected : [expected];
  let msg = "The ";
  if (StringPrototypeEndsWith(name, " argument")) {
    // For cases like 'first argument'
    msg += `${name} `;
  } else {
    const type = StringPrototypeIncludes(name, ".") ? "property" : "argument";
    msg += `"${name}" ${type} `;
  }
  msg += "must be ";

  const types = [];
  const instances = [];
  const other = [];
  for (const value of new SafeArrayIterator(expected)) {
    if (ArrayPrototypeIncludes(kTypes, value)) {
      ArrayPrototypePush(types, StringPrototypeToLocaleLowerCase(value));
    } else if (RegExpPrototypeTest(classRegExp, value)) {
      ArrayPrototypePush(instances, value);
    } else {
      ArrayPrototypePush(other, value);
    }
  }

  // Special handle `object` in case other instances are allowed to outline
  // the differences between each other.
  if (instances.length > 0) {
    const pos = ArrayPrototypeIndexOf(types, "object");
    if (pos !== -1) {
      ArrayPrototypeSplice(types, pos, 1);
      ArrayPrototypePush(instances, "Object");
    }
  }

  if (types.length > 0) {
    if (types.length > 2) {
      const last = ArrayPrototypePop(types);
      msg += `one of type ${ArrayPrototypeJoin(types, ", ")}, or ${last}`;
    } else if (types.length === 2) {
      msg += `one of type ${types[0]} or ${types[1]}`;
    } else {
      msg += `of type ${types[0]}`;
    }
    if (instances.length > 0 || other.length > 0) {
      msg += " or ";
    }
  }

  if (instances.length > 0) {
    if (instances.length > 2) {
      const last = ArrayPrototypePop(instances);
      msg += `an instance of ${
        ArrayPrototypeJoin(instances, ", ")
      }, or ${last}`;
    } else {
      msg += `an instance of ${instances[0]}`;
      if (instances.length === 2) {
        msg += ` or ${instances[1]}`;
      }
    }
    if (other.length > 0) {
      msg += " or ";
    }
  }

  if (other.length > 0) {
    if (other.length > 2) {
      const last = ArrayPrototypePop(other);
      msg += `one of ${ArrayPrototypeJoin(other, ", ")}, or ${last}`;
    } else if (other.length === 2) {
      msg += `one of ${other[0]} or ${other[1]}`;
    } else {
      if (StringPrototypeToLowerCase(other[0]) !== other[0]) {
        msg += "an ";
      }
      msg += `${other[0]}`;
    }
  }

  return msg;
}

class ERR_CRYPTO_TIMING_SAFE_EQUAL_LENGTH extends NodeRangeError {
  constructor() {
    super(
      "ERR_CRYPTO_TIMING_SAFE_EQUAL_LENGTH",
      "Input buffers must have the same byte length",
    );
  }
}

class ERR_INVALID_ARG_TYPE_RANGE extends NodeRangeError {
  constructor(name: string, expected: string | string[], actual: unknown) {
    const msg = createInvalidArgType(name, expected);

    super("ERR_INVALID_ARG_TYPE", `${msg}.${invalidArgTypeHelper(actual)}`);
  }
}

class ERR_INVALID_ARG_TYPE extends NodeTypeError {
  constructor(name: string, expected: string | string[], actual: unknown) {
    const msg = createInvalidArgType(name, expected);
    super("ERR_INVALID_ARG_TYPE", `${msg}.${invalidArgTypeHelper(actual)}`);
  }

  static RangeError = ERR_INVALID_ARG_TYPE_RANGE;
}

class ERR_INVALID_ARG_VALUE_RANGE extends NodeRangeError {
  constructor(name: string, value: unknown, reason: string = "is invalid") {
    const type = StringPrototypeIncludes(name, ".") ? "property" : "argument";
    const inspected = inspect(value);

    super(
      "ERR_INVALID_ARG_VALUE",
      `The ${type} '${name}' ${reason}. Received ${inspected}`,
    );
  }
}

class ERR_INVALID_ARG_VALUE extends NodeTypeError {
  constructor(name: string, value: unknown, reason: string = "is invalid") {
    const type = StringPrototypeIncludes(name, ".") ? "property" : "argument";
    const inspected = inspect(value);

    super(
      "ERR_INVALID_ARG_VALUE",
      `The ${type} '${name}' ${reason}. Received ${inspected}`,
    );
  }

  static RangeError = ERR_INVALID_ARG_VALUE_RANGE;
}

// A helper function to simplify checking for ERR_INVALID_ARG_TYPE output.
// deno-lint-ignore no-explicit-any
function invalidArgTypeHelper(input: any) {
  if (input == null) {
    return ` Received ${input}`;
  }
  if (typeof input === "function") {
    return ` Received function ${input.name}`;
  }
  if (typeof input === "object") {
    if (input.constructor && input.constructor.name) {
      return ` Received an instance of ${input.constructor.name}`;
    }
    return ` Received ${inspect(input, { depth: -1 })}`;
  }
  let inspected = inspect(input, { colors: false });
  if (inspected.length > 25) {
    inspected = `${StringPrototypeSlice(inspected, 0, 25)}...`;
  }
  return ` Received type ${typeof input} (${inspected})`;
}

class ERR_OUT_OF_RANGE extends NodeRangeError {
  constructor(
    str: string,
    range: string,
    input: unknown,
    replaceDefaultBoolean = false,
  ) {
    assert ??= lazyLoadAssert();
    assert(range, 'Missing "range" argument');
    let msg = replaceDefaultBoolean
      ? str
      : `The value of "${str}" is out of range.`;
    let received;
    if (NumberIsInteger(input) && MathAbs(input as number) > 2 ** 32) {
      received = addNumericalSeparator(String(input));
    } else if (typeof input === "bigint") {
      received = String(input);
      if (input > 2n ** 32n || input < -(2n ** 32n)) {
        received = addNumericalSeparator(received);
      }
      received += "n";
    } else {
      received = inspect(input);
    }
    msg += ` It must be ${range}. Received ${received}`;

    super("ERR_OUT_OF_RANGE", msg);
  }
}

class ERR_AMBIGUOUS_ARGUMENT extends NodeTypeError {
  constructor(x: string, y: string) {
    super("ERR_AMBIGUOUS_ARGUMENT", `The "${x}" argument is ambiguous. ${y}`);
  }
}

class ERR_ARG_NOT_ITERABLE extends NodeTypeError {
  constructor(x: string) {
    super("ERR_ARG_NOT_ITERABLE", `${x} must be iterable`);
  }
}

class ERR_ASSERTION extends NodeError {
  constructor(x: string) {
    super("ERR_ASSERTION", `${x}`);
  }
}

class ERR_ASYNC_CALLBACK extends NodeTypeError {
  constructor(x: string) {
    super("ERR_ASYNC_CALLBACK", `${x} must be a function`);
  }
}

class ERR_ASYNC_TYPE extends NodeTypeError {
  constructor(x: string) {
    super("ERR_ASYNC_TYPE", `Invalid name for async "type": ${x}`);
  }
}

class ERR_BROTLI_INVALID_PARAM extends NodeRangeError {
  constructor(x: string) {
    super("ERR_BROTLI_INVALID_PARAM", `${x} is not a valid Brotli parameter`);
  }
}

class ERR_ZSTD_INVALID_PARAM extends NodeRangeError {
  constructor(x: string) {
    super("ERR_ZSTD_INVALID_PARAM", `${x} is not a valid zstd parameter`);
  }
}

class ERR_BUFFER_OUT_OF_BOUNDS extends NodeRangeError {
  constructor(name?: string) {
    super(
      "ERR_BUFFER_OUT_OF_BOUNDS",
      name
        ? `"${name}" is outside of buffer bounds`
        : "Attempt to access memory outside buffer bounds",
    );
  }
}

class ERR_BUFFER_TOO_LARGE extends NodeRangeError {
  constructor(x: string) {
    super(
      "ERR_BUFFER_TOO_LARGE",
      `Cannot create a Buffer larger than ${x} bytes`,
    );
  }
}

class ERR_CANNOT_WATCH_SIGINT extends NodeError {
  constructor() {
    super("ERR_CANNOT_WATCH_SIGINT", "Cannot watch for SIGINT signals");
  }
}

class ERR_CHILD_CLOSED_BEFORE_REPLY extends NodeError {
  constructor() {
    super(
      "ERR_CHILD_CLOSED_BEFORE_REPLY",
      "Child closed before reply received",
    );
  }
}

class ERR_CHILD_PROCESS_IPC_REQUIRED extends NodeError {
  constructor(x: string) {
    super(
      "ERR_CHILD_PROCESS_IPC_REQUIRED",
      `Forked processes must have an IPC channel, missing value 'ipc' in ${x}`,
    );
  }
}

class ERR_CHILD_PROCESS_STDIO_MAXBUFFER extends NodeRangeError {
  constructor(x: string) {
    super(
      "ERR_CHILD_PROCESS_STDIO_MAXBUFFER",
      `${x} maxBuffer length exceeded`,
    );
  }
}

class ERR_CONSOLE_WRITABLE_STREAM extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_CONSOLE_WRITABLE_STREAM",
      `Console expects a writable stream instance for ${x}`,
    );
  }
}

class ERR_CONSTRUCT_CALL_REQUIRED extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_CONSTRUCT_CALL_REQUIRED",
      `Class constructor ${x} cannot be invoked without \`new\``,
    );
  }
}

class ERR_CONTEXT_NOT_INITIALIZED extends NodeError {
  constructor() {
    super("ERR_CONTEXT_NOT_INITIALIZED", "context used is not initialized");
  }
}

class ERR_CPU_USAGE extends NodeError {
  constructor(x: string) {
    super("ERR_CPU_USAGE", `Unable to obtain cpu usage ${x}`);
  }
}

class ERR_CRYPTO_CUSTOM_ENGINE_NOT_SUPPORTED extends NodeError {
  constructor() {
    super(
      "ERR_CRYPTO_CUSTOM_ENGINE_NOT_SUPPORTED",
      "Custom engines not supported by this OpenSSL",
    );
  }
}

class ERR_CRYPTO_ECDH_INVALID_FORMAT extends NodeTypeError {
  constructor(x: string) {
    super("ERR_CRYPTO_ECDH_INVALID_FORMAT", `Invalid ECDH format: ${x}`);
  }
}

class ERR_CRYPTO_ECDH_INVALID_PUBLIC_KEY extends NodeError {
  constructor() {
    super(
      "ERR_CRYPTO_ECDH_INVALID_PUBLIC_KEY",
      "Public key is not valid for specified curve",
    );
  }
}

class ERR_CRYPTO_UNKNOWN_DH_GROUP extends NodeError {
  constructor() {
    super(
      "ERR_CRYPTO_UNKNOWN_DH_GROUP",
      "Unknown DH group",
    );
  }
}

class ERR_CRYPTO_UNKNOWN_CIPHER extends NodeError {
  constructor() {
    super(
      "ERR_CRYPTO_UNKNOWN_CIPHER",
      "Unknown cipher",
    );
  }
}

class ERR_CRYPTO_ENGINE_UNKNOWN extends NodeError {
  constructor(x: string) {
    super("ERR_CRYPTO_ENGINE_UNKNOWN", `Engine "${x}" was not found`);
  }
}

class ERR_CRYPTO_FIPS_FORCED extends NodeError {
  constructor() {
    super(
      "ERR_CRYPTO_FIPS_FORCED",
      "Cannot set FIPS mode, it was forced with --force-fips at startup.",
    );
  }
}

class ERR_CRYPTO_FIPS_UNAVAILABLE extends NodeError {
  constructor() {
    super(
      "ERR_CRYPTO_FIPS_UNAVAILABLE",
      "Cannot set FIPS mode in a non-FIPS build.",
    );
  }
}

class ERR_CRYPTO_HASH_FINALIZED extends NodeError {
  constructor() {
    super("ERR_CRYPTO_HASH_FINALIZED", "Digest already called");
  }
}

class ERR_CRYPTO_HASH_UPDATE_FAILED extends NodeError {
  constructor() {
    super("ERR_CRYPTO_HASH_UPDATE_FAILED", "Hash update failed");
  }
}

class ERR_CRYPTO_INCOMPATIBLE_KEY extends NodeError {
  constructor(x: string, y: string) {
    super("ERR_CRYPTO_INCOMPATIBLE_KEY", `Incompatible ${x}: ${y}`);
  }
}

class ERR_CRYPTO_INCOMPATIBLE_KEY_OPTIONS extends NodeError {
  constructor(x: string, y: string) {
    super(
      "ERR_CRYPTO_INCOMPATIBLE_KEY_OPTIONS",
      `The selected key encoding ${x} ${y}.`,
    );
  }
}

class ERR_CRYPTO_INVALID_DIGEST extends NodeTypeError {
  constructor(x: string, prefix?: string) {
    super(
      "ERR_CRYPTO_INVALID_DIGEST",
      prefix ? `Invalid ${prefix} digest: ${x}` : `Invalid digest: ${x}`,
    );
  }
}

class ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE extends NodeTypeError {
  constructor(x: string, y: string) {
    super(
      "ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE",
      `Invalid key object type ${x}, expected ${y}.`,
    );
  }
}

class ERR_CRYPTO_INVALID_KEYLEN extends NodeRangeError {
  constructor() {
    super("ERR_CRYPTO_INVALID_KEYLEN", "Invalid key length");
  }
}

class ERR_CRYPTO_INVALID_JWK extends NodeError {
  constructor() {
    super("ERR_CRYPTO_INVALID_JWK", "Invalid JWK");
  }
}

class ERR_CRYPTO_INVALID_STATE extends NodeError {
  constructor(x: string) {
    super("ERR_CRYPTO_INVALID_STATE", `Invalid state for operation ${x}`);
  }
}

class ERR_CRYPTO_INVALID_SCRYPT_PARAMS extends NodeRangeError {
  constructor(details?: string) {
    super(
      "ERR_CRYPTO_INVALID_SCRYPT_PARAMS",
      details ? `Invalid scrypt params: ${details}` : "Invalid scrypt params",
    );
  }
}

class ERR_CRYPTO_PBKDF2_ERROR extends NodeError {
  constructor() {
    super("ERR_CRYPTO_PBKDF2_ERROR", "PBKDF2 error");
  }
}

class ERR_CRYPTO_SCRYPT_INVALID_PARAMETER extends NodeError {
  constructor() {
    super("ERR_CRYPTO_SCRYPT_INVALID_PARAMETER", "Invalid scrypt parameter");
  }
}

class ERR_CRYPTO_SCRYPT_NOT_SUPPORTED extends NodeError {
  constructor() {
    super("ERR_CRYPTO_SCRYPT_NOT_SUPPORTED", "Scrypt algorithm not supported");
  }
}

class ERR_CRYPTO_SIGN_KEY_REQUIRED extends NodeError {
  constructor() {
    super("ERR_CRYPTO_SIGN_KEY_REQUIRED", "No key provided to sign");
  }
}

class ERR_DIR_CLOSED extends NodeError {
  constructor() {
    super("ERR_DIR_CLOSED", "Directory handle was closed");
  }
}

class ERR_DIR_CONCURRENT_OPERATION extends NodeError {
  constructor() {
    super(
      "ERR_DIR_CONCURRENT_OPERATION",
      "Cannot do synchronous work on directory handle with concurrent asynchronous operations",
    );
  }
}

class ERR_DNS_SET_SERVERS_FAILED extends NodeError {
  constructor(x: string, y: string) {
    super(
      "ERR_DNS_SET_SERVERS_FAILED",
      `c-ares failed to set servers: "${x}" [${y}]`,
    );
  }
}

class ERR_DOMAIN_CALLBACK_NOT_AVAILABLE extends NodeError {
  constructor() {
    super(
      "ERR_DOMAIN_CALLBACK_NOT_AVAILABLE",
      "A callback was registered through " +
        "process.setUncaughtExceptionCaptureCallback(), which is mutually " +
        "exclusive with using the `domain` module",
    );
  }
}

class ERR_DOMAIN_CANNOT_SET_UNCAUGHT_EXCEPTION_CAPTURE
  extends NodeError {
  constructor() {
    super(
      "ERR_DOMAIN_CANNOT_SET_UNCAUGHT_EXCEPTION_CAPTURE",
      "The `domain` module is in use, which is mutually exclusive with calling " +
        "process.setUncaughtExceptionCaptureCallback()",
    );
  }
}

class ERR_ENCODING_INVALID_ENCODED_DATA extends NodeErrorAbstraction
  implements TypeError {
  errno: number;
  constructor(encoding: string, ret: number) {
    super(
      TypeError.prototype.name,
      "ERR_ENCODING_INVALID_ENCODED_DATA",
      `The encoded data was not valid for encoding ${encoding}`,
    );
    ObjectSetPrototypeOf(this, TypeErrorPrototype);

    this.errno = ret;
  }
}

class ERR_ENCODING_NOT_SUPPORTED extends NodeRangeError {
  constructor(x: string) {
    super("ERR_ENCODING_NOT_SUPPORTED", `The "${x}" encoding is not supported`);
  }
}
class ERR_EVAL_ESM_CANNOT_PRINT extends NodeError {
  constructor() {
    super("ERR_EVAL_ESM_CANNOT_PRINT", `--print cannot be used with ESM input`);
  }
}
class ERR_EVENT_RECURSION extends NodeError {
  constructor(x: string) {
    super(
      "ERR_EVENT_RECURSION",
      `The event "${x}" is already being dispatched`,
    );
  }
}
class ERR_FEATURE_UNAVAILABLE_ON_PLATFORM extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_FEATURE_UNAVAILABLE_ON_PLATFORM",
      `The feature ${x} is unavailable on the current platform, which is being used to run Node.js`,
    );
  }
}
class ERR_FS_FILE_TOO_LARGE extends NodeRangeError {
  constructor(x: string | number) {
    super("ERR_FS_FILE_TOO_LARGE", `File size (${x}) is greater than 2 GB`);
  }
}
class ERR_FS_INVALID_SYMLINK_TYPE extends NodeError {
  constructor(x: string) {
    super(
      "ERR_FS_INVALID_SYMLINK_TYPE",
      `Symlink type must be one of "dir", "file", or "junction". Received "${x}"`,
    );
  }
}
class ERR_HTTP2_ALTSVC_INVALID_ORIGIN extends NodeTypeError {
  constructor() {
    super(
      "ERR_HTTP2_ALTSVC_INVALID_ORIGIN",
      `HTTP/2 ALTSVC frames require a valid origin`,
    );
  }
}
class ERR_HTTP2_ALTSVC_LENGTH extends NodeTypeError {
  constructor() {
    super(
      "ERR_HTTP2_ALTSVC_LENGTH",
      `HTTP/2 ALTSVC frames are limited to 16382 bytes`,
    );
  }
}
class ERR_HTTP2_CONNECT_AUTHORITY extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_CONNECT_AUTHORITY",
      `:authority header is required for CONNECT requests`,
    );
  }
}
class ERR_HTTP2_CONNECT_PATH extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_CONNECT_PATH",
      `The :path header is forbidden for CONNECT requests`,
    );
  }
}
class ERR_HTTP2_CONNECT_SCHEME extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_CONNECT_SCHEME",
      `The :scheme header is forbidden for CONNECT requests`,
    );
  }
}
class ERR_HTTP2_GOAWAY_SESSION extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_GOAWAY_SESSION",
      `New streams cannot be created after receiving a GOAWAY`,
    );
  }
}
class ERR_HTTP2_HEADERS_AFTER_RESPOND extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_HEADERS_AFTER_RESPOND",
      `Cannot specify additional headers after response initiated`,
    );
  }
}
class ERR_HTTP2_HEADERS_SENT extends NodeError {
  constructor() {
    super("ERR_HTTP2_HEADERS_SENT", `Response has already been initiated.`);
  }
}
class ERR_HTTP2_HEADER_SINGLE_VALUE extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_HTTP2_HEADER_SINGLE_VALUE",
      `Header field "${x}" must only have a single value`,
    );
  }
}
class ERR_HTTP2_INFO_STATUS_NOT_ALLOWED extends NodeRangeError {
  constructor() {
    super(
      "ERR_HTTP2_INFO_STATUS_NOT_ALLOWED",
      `Informational status codes cannot be used`,
    );
  }
}
class ERR_HTTP2_INVALID_CONNECTION_HEADERS extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_HTTP2_INVALID_CONNECTION_HEADERS",
      `HTTP/1 Connection specific headers are forbidden: "${x}"`,
    );
  }
}
class ERR_HTTP2_INVALID_HEADER_VALUE extends NodeTypeError {
  static HideStackFramesError = this;
  constructor(x: string, y: string) {
    super(
      "ERR_HTTP2_INVALID_HEADER_VALUE",
      `Invalid value "${x}" for header "${y}"`,
    );
  }
}
class ERR_HTTP2_INVALID_INFO_STATUS extends NodeRangeError {
  constructor(x: string) {
    super(
      "ERR_HTTP2_INVALID_INFO_STATUS",
      `Invalid informational status code: ${x}`,
    );
  }
}
class ERR_HTTP2_INVALID_ORIGIN extends NodeTypeError {
  constructor() {
    super(
      "ERR_HTTP2_INVALID_ORIGIN",
      `HTTP/2 ORIGIN frames require a valid origin`,
    );
  }
}
class ERR_HTTP2_INVALID_PACKED_SETTINGS_LENGTH extends NodeRangeError {
  constructor() {
    super(
      "ERR_HTTP2_INVALID_PACKED_SETTINGS_LENGTH",
      `Packed settings length must be a multiple of six`,
    );
  }
}
class ERR_HTTP2_INVALID_PSEUDOHEADER extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_HTTP2_INVALID_PSEUDOHEADER",
      `"${x}" is an invalid pseudoheader or is used incorrectly`,
    );
  }
}
class ERR_HTTP2_INVALID_SESSION extends NodeError {
  constructor() {
    super("ERR_HTTP2_INVALID_SESSION", `The session has been destroyed`);
  }
}
class ERR_HTTP2_INVALID_STREAM extends NodeError {
  constructor() {
    super("ERR_HTTP2_INVALID_STREAM", `The stream has been destroyed`);
  }
}
class ERR_HTTP2_MAX_PENDING_SETTINGS_ACK extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_MAX_PENDING_SETTINGS_ACK",
      `Maximum number of pending settings acknowledgements`,
    );
  }
}
class ERR_HTTP2_NESTED_PUSH extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_NESTED_PUSH",
      `A push stream cannot initiate another push stream.`,
    );
  }
}
class ERR_HTTP2_NO_SOCKET_MANIPULATION extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_NO_SOCKET_MANIPULATION",
      `HTTP/2 sockets should not be directly manipulated (e.g. read and written)`,
    );
  }
}
class ERR_HTTP2_ORIGIN_LENGTH extends NodeTypeError {
  constructor() {
    super(
      "ERR_HTTP2_ORIGIN_LENGTH",
      `HTTP/2 ORIGIN frames are limited to 16382 bytes`,
    );
  }
}
class ERR_HTTP2_OUT_OF_STREAMS extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_OUT_OF_STREAMS",
      `No stream ID is available because maximum stream ID has been reached`,
    );
  }
}
class ERR_HTTP2_PAYLOAD_FORBIDDEN extends NodeError {
  constructor(x: string) {
    super(
      "ERR_HTTP2_PAYLOAD_FORBIDDEN",
      `Responses with ${x} status must not have a payload`,
    );
  }
}
class ERR_HTTP2_PING_CANCEL extends NodeError {
  constructor() {
    super("ERR_HTTP2_PING_CANCEL", `HTTP2 ping cancelled`);
  }
}
class ERR_HTTP2_PING_LENGTH extends NodeRangeError {
  constructor() {
    super("ERR_HTTP2_PING_LENGTH", `HTTP2 ping payload must be 8 bytes`);
  }
}
class ERR_HTTP2_PSEUDOHEADER_NOT_ALLOWED extends NodeTypeError {
  static HideStackFramesError = this;
  constructor() {
    super(
      "ERR_HTTP2_PSEUDOHEADER_NOT_ALLOWED",
      `Cannot set HTTP/2 pseudo-headers`,
    );
  }
}
class ERR_HTTP2_PUSH_DISABLED extends NodeError {
  constructor() {
    super("ERR_HTTP2_PUSH_DISABLED", `HTTP/2 client has disabled push streams`);
  }
}
class ERR_HTTP2_SEND_FILE extends NodeError {
  constructor() {
    super("ERR_HTTP2_SEND_FILE", `Directories cannot be sent`);
  }
}
class ERR_HTTP2_SEND_FILE_NOSEEK extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_SEND_FILE_NOSEEK",
      `Offset or length can only be specified for regular files`,
    );
  }
}
class ERR_HTTP2_SESSION_ERROR extends NodeError {
  constructor(x: string) {
    super("ERR_HTTP2_SESSION_ERROR", `Session closed with error code ${x}`);
  }
}
class ERR_HTTP2_SETTINGS_CANCEL extends NodeError {
  constructor() {
    super("ERR_HTTP2_SETTINGS_CANCEL", `HTTP2 session settings canceled`);
  }
}
class ERR_HTTP2_SOCKET_BOUND extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_SOCKET_BOUND",
      `The socket is already bound to an Http2Session`,
    );
  }
}
class ERR_HTTP2_SOCKET_UNBOUND extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_SOCKET_UNBOUND",
      `The socket has been disconnected from the Http2Session`,
    );
  }
}
class ERR_HTTP2_STATUS_101 extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_STATUS_101",
      `HTTP status code 101 (Switching Protocols) is forbidden in HTTP/2`,
    );
  }
}
class ERR_HTTP2_STATUS_INVALID extends NodeRangeError {
  constructor(x: string) {
    super("ERR_HTTP2_STATUS_INVALID", `Invalid status code: ${x}`);
  }
}
class ERR_HTTP2_STREAM_ERROR extends NodeError {
  constructor(x: string) {
    super("ERR_HTTP2_STREAM_ERROR", `Stream closed with error code ${x}`);
  }
}
class ERR_HTTP2_STREAM_SELF_DEPENDENCY extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_STREAM_SELF_DEPENDENCY",
      `A stream cannot depend on itself`,
    );
  }
}
class ERR_HTTP2_TRAILERS_ALREADY_SENT extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_TRAILERS_ALREADY_SENT",
      `Trailing headers have already been sent`,
    );
  }
}
class ERR_HTTP2_TRAILERS_NOT_READY extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_TRAILERS_NOT_READY",
      `Trailing headers cannot be sent until after the wantTrailers event is emitted`,
    );
  }
}
class ERR_HTTP2_UNSUPPORTED_PROTOCOL extends NodeError {
  constructor(x: string) {
    super("ERR_HTTP2_UNSUPPORTED_PROTOCOL", `protocol "${x}" is unsupported.`);
  }
}
class ERR_HTTP_BODY_NOT_ALLOWED extends NodeError {
  constructor() {
    super(
      "ERR_HTTP_BODY_NOT_ALLOWED",
      "Adding content for this request method or response status is not allowed.",
    );
  }
}
class ERR_HTTP_CONTENT_LENGTH_MISMATCH extends NodeError {
  constructor(bodyLength: number, contentLength: number) {
    super(
      "ERR_HTTP_CONTENT_LENGTH_MISMATCH",
      `Response body's content-length of ${bodyLength} byte(s) does not match the content-length of ${contentLength} byte(s) set in header`,
    );
  }
}
class ERR_HTTP_HEADERS_SENT extends NodeError {
  constructor(x: string) {
    super(
      "ERR_HTTP_HEADERS_SENT",
      `Cannot ${x} headers after they are sent to the client`,
    );
  }
}
class ERR_HTTP_INVALID_HEADER_VALUE extends NodeTypeError {
  constructor(x: string, y: string) {
    super(
      "ERR_HTTP_INVALID_HEADER_VALUE",
      `Invalid value "${x}" for header "${y}"`,
    );
  }
}
class ERR_HTTP_INVALID_STATUS_CODE extends NodeRangeError {
  constructor(x: string) {
    super("ERR_HTTP_INVALID_STATUS_CODE", `Invalid status code: ${x}`);
  }
}
class ERR_HTTP_SOCKET_ENCODING extends NodeError {
  constructor() {
    super(
      "ERR_HTTP_SOCKET_ENCODING",
      `Changing the socket encoding is not allowed per RFC7230 Section 3.`,
    );
  }
}
class ERR_HTTP_TRAILER_INVALID extends NodeError {
  constructor() {
    super(
      "ERR_HTTP_TRAILER_INVALID",
      `Trailers are invalid with this transfer encoding`,
    );
  }
}
class ERR_ILLEGAL_CONSTRUCTOR extends NodeTypeError {
  constructor() {
    super("ERR_ILLEGAL_CONSTRUCTOR", "Illegal constructor");
  }
}
class ERR_INCOMPATIBLE_OPTION_PAIR extends NodeTypeError {
  constructor(x: string, y: string) {
    super(
      "ERR_INCOMPATIBLE_OPTION_PAIR",
      `Option "${x}" cannot be used in combination with option "${y}"`,
    );
  }
}
class ERR_INPUT_TYPE_NOT_ALLOWED extends NodeError {
  constructor() {
    super(
      "ERR_INPUT_TYPE_NOT_ALLOWED",
      `--input-type can only be used with string input via --eval, --print, or STDIN`,
    );
  }
}
class ERR_INSPECTOR_ALREADY_ACTIVATED extends NodeError {
  constructor() {
    super(
      "ERR_INSPECTOR_ALREADY_ACTIVATED",
      `Inspector is already activated. Close it with inspector.close() before activating it again.`,
    );
  }
}
class ERR_INSPECTOR_ALREADY_CONNECTED extends NodeError {
  constructor(x: string) {
    super("ERR_INSPECTOR_ALREADY_CONNECTED", `${x} is already connected`);
  }
}
class ERR_INSPECTOR_CLOSED extends NodeError {
  constructor() {
    super("ERR_INSPECTOR_CLOSED", `Session was closed`);
  }
}
class ERR_INSPECTOR_COMMAND extends NodeError {
  constructor(x: number, y: string) {
    super("ERR_INSPECTOR_COMMAND", `Inspector error ${x}: ${y}`);
  }
}
class ERR_INSPECTOR_NOT_ACTIVE extends NodeError {
  constructor() {
    super("ERR_INSPECTOR_NOT_ACTIVE", `Inspector is not active`);
  }
}
class ERR_INSPECTOR_NOT_AVAILABLE extends NodeError {
  constructor() {
    super("ERR_INSPECTOR_NOT_AVAILABLE", `Inspector is not available`);
  }
}
class ERR_INSPECTOR_NOT_CONNECTED extends NodeError {
  constructor() {
    super("ERR_INSPECTOR_NOT_CONNECTED", `Session is not connected`);
  }
}
class ERR_INSPECTOR_NOT_WORKER extends NodeError {
  constructor() {
    super("ERR_INSPECTOR_NOT_WORKER", `Current thread is not a worker`);
  }
}
class ERR_INVALID_ASYNC_ID extends NodeRangeError {
  constructor(x: string, y: string | number) {
    super("ERR_INVALID_ASYNC_ID", `Invalid ${x} value: ${y}`);
  }
}
class ERR_INVALID_BUFFER_SIZE extends NodeRangeError {
  constructor(x: string) {
    super("ERR_INVALID_BUFFER_SIZE", `Buffer size must be a multiple of ${x}`);
  }
}
class ERR_INVALID_CURSOR_POS extends NodeTypeError {
  constructor() {
    super(
      "ERR_INVALID_CURSOR_POS",
      `Cannot set cursor row without setting its column`,
    );
  }
}
class ERR_INVALID_FD extends NodeRangeError {
  constructor(x: string) {
    super("ERR_INVALID_FD", `"fd" must be a positive integer: ${x}`);
  }
}
class ERR_INVALID_FD_TYPE extends NodeTypeError {
  constructor(x: string) {
    super("ERR_INVALID_FD_TYPE", `Unsupported fd type: ${x}`);
  }
}
class ERR_INVALID_FILE_URL_HOST extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_INVALID_FILE_URL_HOST",
      `File URL host must be "localhost" or empty on ${x}`,
    );
  }
}
class ERR_INVALID_FILE_URL_PATH extends NodeTypeError {
  input?: URL;
  constructor(x: string, input?: URL) {
    super("ERR_INVALID_FILE_URL_PATH", `File URL path ${x}`);
    this.input = input;
  }
}
class ERR_INVALID_HANDLE_TYPE extends NodeTypeError {
  constructor() {
    super("ERR_INVALID_HANDLE_TYPE", `This handle type cannot be sent`);
  }
}
class ERR_INVALID_HTTP_TOKEN extends NodeTypeError {
  static HideStackFramesError = this;
  constructor(x: string, y: string) {
    super("ERR_INVALID_HTTP_TOKEN", `${x} must be a valid HTTP token ["${y}"]`);
  }
}
class ERR_INVALID_IP_ADDRESS extends NodeTypeError {
  constructor(x: string) {
    super("ERR_INVALID_IP_ADDRESS", `Invalid IP address: ${x}`);
  }
}
class ERR_IP_BLOCKED extends NodeError {
  constructor(x: string) {
    super("ERR_IP_BLOCKED", `Address blocked: ${x}`);
  }
}
class ERR_INVALID_MIME_SYNTAX extends NodeTypeError {
  constructor(production: string, str: string, invalidIndex: number) {
    const msg = invalidIndex !== -1 ? ` at ${invalidIndex}` : "";
    super(
      "ERR_INVALID_MIME_SYNTAX",
      `The MIME syntax for a ${production} in "${str}" is invalid` + msg,
    );
  }
}
class ERR_INVALID_OBJECT_DEFINE_PROPERTY extends NodeTypeError {
  constructor(message: string) {
    super(
      "ERR_INVALID_OBJECT_DEFINE_PROPERTY",
      message,
    );
  }
}
class ERR_INVALID_OPT_VALUE_ENCODING extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_INVALID_OPT_VALUE_ENCODING",
      `The value "${x}" is invalid for option "encoding"`,
    );
  }
}
class ERR_INVALID_PERFORMANCE_MARK extends NodeError {
  constructor(x: string) {
    super(
      "ERR_INVALID_PERFORMANCE_MARK",
      `The "${x}" performance mark has not been set`,
    );
  }
}
class ERR_INVALID_PROTOCOL extends NodeTypeError {
  constructor(x: string, y: string) {
    super(
      "ERR_INVALID_PROTOCOL",
      `Protocol "${x}" not supported. Expected "${y}"`,
    );
  }
}
class ERR_INVALID_REPL_EVAL_CONFIG extends NodeTypeError {
  constructor() {
    super(
      "ERR_INVALID_REPL_EVAL_CONFIG",
      `Cannot specify both "breakEvalOnSigint" and "eval" for REPL`,
    );
  }
}
class ERR_INVALID_REPL_INPUT extends NodeTypeError {
  constructor(x: string) {
    super("ERR_INVALID_REPL_INPUT", `${x}`);
  }
}
class ERR_INVALID_SYNC_FORK_INPUT extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_INVALID_SYNC_FORK_INPUT",
      `Asynchronous forks do not support Buffer, TypedArray, DataView or string input: ${x}`,
    );
  }
}
class ERR_INVALID_THIS extends NodeTypeError {
  constructor(x: string) {
    super("ERR_INVALID_THIS", `Value of "this" must be of type ${x}`);
  }
}
class ERR_INVALID_TUPLE extends NodeTypeError {
  constructor(x: string, y: string) {
    super("ERR_INVALID_TUPLE", `${x} must be an iterable ${y} tuple`);
  }
}
class ERR_INVALID_URI extends NodeURIError {
  constructor() {
    super("ERR_INVALID_URI", `URI malformed`);
  }
}
class ERR_IPC_CHANNEL_CLOSED extends NodeError {
  constructor() {
    super("ERR_IPC_CHANNEL_CLOSED", `Channel closed`);
  }
}
class ERR_IPC_DISCONNECTED extends NodeError {
  constructor() {
    super("ERR_IPC_DISCONNECTED", `IPC channel is already disconnected`);
  }
}
class ERR_IPC_ONE_PIPE extends NodeError {
  constructor() {
    super("ERR_IPC_ONE_PIPE", `Child process can have only one IPC pipe`);
  }
}
class ERR_IPC_SYNC_FORK extends NodeError {
  constructor() {
    super("ERR_IPC_SYNC_FORK", `IPC cannot be used with synchronous forks`);
  }
}
class ERR_MANIFEST_DEPENDENCY_MISSING extends NodeError {
  constructor(x: string, y: string) {
    super(
      "ERR_MANIFEST_DEPENDENCY_MISSING",
      `Manifest resource ${x} does not list ${y} as a dependency specifier`,
    );
  }
}
class ERR_MANIFEST_INTEGRITY_MISMATCH extends NodeSyntaxError {
  constructor(x: string) {
    super(
      "ERR_MANIFEST_INTEGRITY_MISMATCH",
      `Manifest resource ${x} has multiple entries but integrity lists do not match`,
    );
  }
}
class ERR_MANIFEST_INVALID_RESOURCE_FIELD extends NodeTypeError {
  constructor(x: string, y: string) {
    super(
      "ERR_MANIFEST_INVALID_RESOURCE_FIELD",
      `Manifest resource ${x} has invalid property value for ${y}`,
    );
  }
}
class ERR_MANIFEST_TDZ extends NodeError {
  constructor() {
    super("ERR_MANIFEST_TDZ", `Manifest initialization has not yet run`);
  }
}
class ERR_MANIFEST_UNKNOWN_ONERROR extends NodeSyntaxError {
  constructor(x: string) {
    super(
      "ERR_MANIFEST_UNKNOWN_ONERROR",
      `Manifest specified unknown error behavior "${x}".`,
    );
  }
}
class ERR_METHOD_NOT_IMPLEMENTED extends NodeError {
  constructor(x: string) {
    super("ERR_METHOD_NOT_IMPLEMENTED", `The ${x} method is not implemented`);
  }
}
class ERR_MISSING_ARGS extends NodeTypeError {
  constructor(...args: (string | string[])[]) {
    let msg = "The ";

    const len = args.length;

    const wrap = (a: unknown) => `"${a}"`;

    args = ArrayPrototypeMap(
      args,
      (a) =>
        ArrayIsArray(a)
          ? ArrayPrototypeJoin(ArrayPrototypeMap(a, wrap), " or ")
          : wrap(a),
    );

    switch (len) {
      case 1:
        msg += `${args[0]} argument`;
        break;
      case 2:
        msg += `${args[0]} and ${args[1]} arguments`;
        break;
      default:
        msg += ArrayPrototypeJoin(ArrayPrototypeSlice(args, 0, len - 1), ", ");
        msg += `, and ${args[len - 1]} arguments`;
        break;
    }

    super("ERR_MISSING_ARGS", `${msg} must be specified`);
  }
}
class ERR_MISSING_OPTION extends NodeTypeError {
  constructor(x: string) {
    super("ERR_MISSING_OPTION", `${x} is required`);
  }
}
class ERR_MULTIPLE_CALLBACK extends NodeError {
  constructor() {
    super("ERR_MULTIPLE_CALLBACK", `Callback called multiple times`);
  }
}
class ERR_NAPI_CONS_FUNCTION extends NodeTypeError {
  constructor() {
    super("ERR_NAPI_CONS_FUNCTION", `Constructor must be a function`);
  }
}
class ERR_NAPI_INVALID_DATAVIEW_ARGS extends NodeRangeError {
  constructor() {
    super(
      "ERR_NAPI_INVALID_DATAVIEW_ARGS",
      `byte_offset + byte_length should be less than or equal to the size in bytes of the array passed in`,
    );
  }
}
class ERR_NAPI_INVALID_TYPEDARRAY_ALIGNMENT extends NodeRangeError {
  constructor(x: string, y: string) {
    super(
      "ERR_NAPI_INVALID_TYPEDARRAY_ALIGNMENT",
      `start offset of ${x} should be a multiple of ${y}`,
    );
  }
}
class ERR_NAPI_INVALID_TYPEDARRAY_LENGTH extends NodeRangeError {
  constructor() {
    super("ERR_NAPI_INVALID_TYPEDARRAY_LENGTH", `Invalid typed array length`);
  }
}
class ERR_NO_CRYPTO extends NodeError {
  constructor() {
    super(
      "ERR_NO_CRYPTO",
      `Node.js is not compiled with OpenSSL crypto support`,
    );
  }
}
class ERR_NO_ICU extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_NO_ICU",
      `${x} is not supported on Node.js compiled without ICU`,
    );
  }
}
class ERR_QUICCLIENTSESSION_FAILED extends NodeError {
  constructor(x: string) {
    super(
      "ERR_QUICCLIENTSESSION_FAILED",
      `Failed to create a new QuicClientSession: ${x}`,
    );
  }
}
class ERR_QUICCLIENTSESSION_FAILED_SETSOCKET extends NodeError {
  constructor() {
    super(
      "ERR_QUICCLIENTSESSION_FAILED_SETSOCKET",
      `Failed to set the QuicSocket`,
    );
  }
}
class ERR_QUICSESSION_DESTROYED extends NodeError {
  constructor(x: string) {
    super(
      "ERR_QUICSESSION_DESTROYED",
      `Cannot call ${x} after a QuicSession has been destroyed`,
    );
  }
}
class ERR_QUICSESSION_INVALID_DCID extends NodeError {
  constructor(x: string) {
    super("ERR_QUICSESSION_INVALID_DCID", `Invalid DCID value: ${x}`);
  }
}
class ERR_QUICSESSION_UPDATEKEY extends NodeError {
  constructor() {
    super("ERR_QUICSESSION_UPDATEKEY", `Unable to update QuicSession keys`);
  }
}
class ERR_QUICSOCKET_DESTROYED extends NodeError {
  constructor(x: string) {
    super(
      "ERR_QUICSOCKET_DESTROYED",
      `Cannot call ${x} after a QuicSocket has been destroyed`,
    );
  }
}
class ERR_QUICSOCKET_INVALID_STATELESS_RESET_SECRET_LENGTH
  extends NodeError {
  constructor() {
    super(
      "ERR_QUICSOCKET_INVALID_STATELESS_RESET_SECRET_LENGTH",
      `The stateResetToken must be exactly 16-bytes in length`,
    );
  }
}
class ERR_QUICSOCKET_LISTENING extends NodeError {
  constructor() {
    super("ERR_QUICSOCKET_LISTENING", `This QuicSocket is already listening`);
  }
}
class ERR_QUICSOCKET_UNBOUND extends NodeError {
  constructor(x: string) {
    super(
      "ERR_QUICSOCKET_UNBOUND",
      `Cannot call ${x} before a QuicSocket has been bound`,
    );
  }
}
class ERR_QUICSTREAM_DESTROYED extends NodeError {
  constructor(x: string) {
    super(
      "ERR_QUICSTREAM_DESTROYED",
      `Cannot call ${x} after a QuicStream has been destroyed`,
    );
  }
}
class ERR_QUICSTREAM_INVALID_PUSH extends NodeError {
  constructor() {
    super(
      "ERR_QUICSTREAM_INVALID_PUSH",
      `Push streams are only supported on client-initiated, bidirectional streams`,
    );
  }
}
class ERR_QUICSTREAM_OPEN_FAILED extends NodeError {
  constructor() {
    super("ERR_QUICSTREAM_OPEN_FAILED", `Opening a new QuicStream failed`);
  }
}
class ERR_QUICSTREAM_UNSUPPORTED_PUSH extends NodeError {
  constructor() {
    super(
      "ERR_QUICSTREAM_UNSUPPORTED_PUSH",
      `Push streams are not supported on this QuicSession`,
    );
  }
}
class ERR_QUIC_TLS13_REQUIRED extends NodeError {
  constructor() {
    super("ERR_QUIC_TLS13_REQUIRED", `QUIC requires TLS version 1.3`);
  }
}
class ERR_SCRIPT_EXECUTION_INTERRUPTED extends NodeError {
  constructor() {
    super(
      "ERR_SCRIPT_EXECUTION_INTERRUPTED",
      "Script execution was interrupted by `SIGINT`",
    );
  }
}
class ERR_SERVER_ALREADY_LISTEN extends NodeError {
  constructor() {
    super(
      "ERR_SERVER_ALREADY_LISTEN",
      `Listen method has been called more than once without closing.`,
    );
  }
}
class ERR_SERVER_NOT_RUNNING extends NodeError {
  constructor() {
    super("ERR_SERVER_NOT_RUNNING", `Server is not running.`);
  }
}
class ERR_SOCKET_ALREADY_BOUND extends NodeError {
  constructor() {
    super("ERR_SOCKET_ALREADY_BOUND", `Socket is already bound`);
  }
}
class ERR_SOCKET_BAD_BUFFER_SIZE extends NodeTypeError {
  constructor() {
    super(
      "ERR_SOCKET_BAD_BUFFER_SIZE",
      `Buffer size must be a positive integer`,
    );
  }
}
class ERR_SOCKET_BAD_PORT extends NodeRangeError {
  constructor(name: string, port: unknown, allowZero = true) {
    assert ??= lazyLoadAssert();
    assert(
      typeof allowZero === "boolean",
      "The 'allowZero' argument must be of type boolean.",
    );

    const operator = allowZero ? ">=" : ">";
    const portStr = typeof port === "symbol"
      ? SymbolPrototypeToString(port)
      : typeof port === "bigint"
      ? `${port}n`
      : String(port);

    super(
      "ERR_SOCKET_BAD_PORT",
      `${name} should be ${operator} 0 and < 65536. Received ${portStr}.`,
    );
  }
}
class ERR_SOCKET_BAD_TYPE extends NodeTypeError {
  constructor() {
    super(
      "ERR_SOCKET_BAD_TYPE",
      `Bad socket type specified. Valid types are: udp4, udp6`,
    );
  }
}
class ERR_SOCKET_BUFFER_SIZE extends NodeSystemError {
  constructor(ctx: NodeSystemErrorCtx) {
    super("ERR_SOCKET_BUFFER_SIZE", ctx, "Could not get or set buffer size");
  }
}
class ERR_SOCKET_CLOSED extends NodeError {
  constructor() {
    super("ERR_SOCKET_CLOSED", `Socket is closed`);
  }
}
class ERR_SOCKET_CLOSED_BEFORE_CONNECTION extends NodeError {
  constructor() {
    super(
      "ERR_SOCKET_CLOSED_BEFORE_CONNECTION",
      `Socket closed before the connection was established`,
    );
  }
}
class ERR_SOCKET_CONNECTION_TIMEOUT extends NodeError {
  constructor() {
    super("ERR_SOCKET_CONNECTION_TIMEOUT", `Socket connection timeout`);
  }
}
class ERR_SOCKET_DGRAM_IS_CONNECTED extends NodeError {
  constructor() {
    super("ERR_SOCKET_DGRAM_IS_CONNECTED", `Already connected`);
  }
}
class ERR_SOCKET_DGRAM_NOT_CONNECTED extends NodeError {
  constructor() {
    super("ERR_SOCKET_DGRAM_NOT_CONNECTED", `Not connected`);
  }
}
class ERR_SOCKET_DGRAM_NOT_RUNNING extends NodeError {
  constructor() {
    super("ERR_SOCKET_DGRAM_NOT_RUNNING", `Not running`);
  }
}
class ERR_SRI_PARSE extends NodeSyntaxError {
  constructor(name: string, char: string, position: number) {
    super(
      "ERR_SRI_PARSE",
      `Subresource Integrity string ${name} had an unexpected ${char} at position ${position}`,
    );
  }
}
class ERR_STREAM_ALREADY_FINISHED extends NodeError {
  constructor(x: string) {
    super(
      "ERR_STREAM_ALREADY_FINISHED",
      `Cannot call ${x} after a stream was finished`,
    );
  }
}
class ERR_STREAM_CANNOT_PIPE extends NodeError {
  constructor() {
    super("ERR_STREAM_CANNOT_PIPE", `Cannot pipe, not readable`);
  }
}
class ERR_STREAM_DESTROYED extends NodeError {
  constructor(x: string) {
    super(
      "ERR_STREAM_DESTROYED",
      `Cannot call ${x} after a stream was destroyed`,
    );
  }
}
class ERR_STREAM_NULL_VALUES extends NodeTypeError {
  constructor() {
    super("ERR_STREAM_NULL_VALUES", `May not write null values to stream`);
  }
}
class ERR_STREAM_PREMATURE_CLOSE extends NodeError {
  constructor() {
    super("ERR_STREAM_PREMATURE_CLOSE", `Premature close`);
  }
}
class ERR_STREAM_PUSH_AFTER_EOF extends NodeError {
  constructor() {
    super("ERR_STREAM_PUSH_AFTER_EOF", `stream.push() after EOF`);
  }
}
class ERR_STREAM_UNSHIFT_AFTER_END_EVENT extends NodeError {
  constructor() {
    super(
      "ERR_STREAM_UNSHIFT_AFTER_END_EVENT",
      `stream.unshift() after end event`,
    );
  }
}
class ERR_STREAM_WRAP extends NodeError {
  constructor() {
    super(
      "ERR_STREAM_WRAP",
      `Stream has StringDecoder set or is in objectMode`,
    );
  }
}
class ERR_STREAM_WRITE_AFTER_END extends NodeError {
  constructor() {
    super("ERR_STREAM_WRITE_AFTER_END", `write after end`);
  }
}
class ERR_SYNTHETIC extends NodeError {
  constructor() {
    super("ERR_SYNTHETIC", `JavaScript Callstack`);
  }
}
class ERR_TLS_CERT_ALTNAME_INVALID extends NodeError {
  reason: string;
  host: string;
  cert: string;

  constructor(reason: string, host: string, cert: string) {
    super(
      "ERR_TLS_CERT_ALTNAME_INVALID",
      `Hostname/IP does not match certificate's altnames: ${reason}`,
    );
    this.reason = reason;
    this.host = host;
    this.cert = cert;
  }
}
class ERR_TLS_ALPN_CALLBACK_WITH_PROTOCOLS extends NodeTypeError {
  constructor() {
    super(
      "ERR_TLS_ALPN_CALLBACK_WITH_PROTOCOLS",
      "The ALPNCallback and ALPNProtocols TLS options are mutually exclusive",
    );
  }
}
class ERR_TLS_DH_PARAM_SIZE extends NodeError {
  constructor(x: string) {
    super("ERR_TLS_DH_PARAM_SIZE", `DH parameter size ${x} is less than 2048`);
  }
}
class ERR_TLS_HANDSHAKE_TIMEOUT extends NodeError {
  constructor() {
    super("ERR_TLS_HANDSHAKE_TIMEOUT", `TLS handshake timeout`);
  }
}
class ERR_TLS_INVALID_CONTEXT extends NodeTypeError {
  constructor(x: string) {
    super("ERR_TLS_INVALID_CONTEXT", `${x} must be a SecureContext`);
  }
}
class ERR_TLS_INVALID_STATE extends NodeError {
  constructor() {
    super(
      "ERR_TLS_INVALID_STATE",
      `TLS socket connection must be securely established`,
    );
  }
}
class ERR_TLS_INVALID_PROTOCOL_VERSION extends NodeTypeError {
  constructor(protocol: string, x: string) {
    super(
      "ERR_TLS_INVALID_PROTOCOL_VERSION",
      `${protocol} is not a valid ${x} TLS protocol version`,
    );
  }
}
class ERR_TLS_PROTOCOL_VERSION_CONFLICT extends NodeTypeError {
  constructor(prevProtocol: string, protocol: string) {
    super(
      "ERR_TLS_PROTOCOL_VERSION_CONFLICT",
      `TLS protocol version ${prevProtocol} conflicts with secureProtocol ${protocol}`,
    );
  }
}
class ERR_TLS_RENEGOTIATION_DISABLED extends NodeError {
  constructor() {
    super(
      "ERR_TLS_RENEGOTIATION_DISABLED",
      `TLS session renegotiation disabled for this socket`,
    );
  }
}
class ERR_TLS_REQUIRED_SERVER_NAME extends NodeError {
  constructor() {
    super(
      "ERR_TLS_REQUIRED_SERVER_NAME",
      `"servername" is required parameter for Server.addContext`,
    );
  }
}
class ERR_TLS_SESSION_ATTACK extends NodeError {
  constructor() {
    super(
      "ERR_TLS_SESSION_ATTACK",
      `TLS session renegotiation attack detected`,
    );
  }
}
class ERR_TLS_SNI_FROM_SERVER extends NodeError {
  constructor() {
    super(
      "ERR_TLS_SNI_FROM_SERVER",
      `Cannot issue SNI from a TLS server-side socket`,
    );
  }
}
class ERR_TRACE_EVENTS_CATEGORY_REQUIRED extends NodeTypeError {
  constructor() {
    super(
      "ERR_TRACE_EVENTS_CATEGORY_REQUIRED",
      `At least one category is required`,
    );
  }
}
class ERR_TRACE_EVENTS_UNAVAILABLE extends NodeError {
  constructor() {
    super("ERR_TRACE_EVENTS_UNAVAILABLE", `Trace events are unavailable`);
  }
}
class ERR_UNAVAILABLE_DURING_EXIT extends NodeError {
  constructor() {
    super(
      "ERR_UNAVAILABLE_DURING_EXIT",
      `Cannot call function in process exit handler`,
    );
  }
}
class ERR_UNCAUGHT_EXCEPTION_CAPTURE_ALREADY_SET extends NodeError {
  constructor() {
    super(
      "ERR_UNCAUGHT_EXCEPTION_CAPTURE_ALREADY_SET",
      "`process.setupUncaughtExceptionCapture()` was called while a capture callback was already active",
    );
  }
}
class ERR_UNESCAPED_CHARACTERS extends NodeTypeError {
  constructor(x: string) {
    super("ERR_UNESCAPED_CHARACTERS", `${x} contains unescaped characters`);
  }
}
class ERR_UNHANDLED_ERROR extends NodeError {
  constructor(x: string) {
    super("ERR_UNHANDLED_ERROR", `Unhandled error. (${x})`);
  }
}
class ERR_UNKNOWN_BUILTIN_MODULE extends NodeError {
  constructor(x: string) {
    super("ERR_UNKNOWN_BUILTIN_MODULE", `No such built-in module: ${x}`);
  }
}
class ERR_UNKNOWN_CREDENTIAL extends NodeError {
  constructor(x: string, y: string) {
    super("ERR_UNKNOWN_CREDENTIAL", `${x} identifier does not exist: ${y}`);
  }
}
class ERR_UNKNOWN_ENCODING extends NodeTypeError {
  constructor(x: string) {
    super("ERR_UNKNOWN_ENCODING", format("Unknown encoding: %s", x));
  }
}
class ERR_UNKNOWN_FILE_EXTENSION extends NodeTypeError {
  constructor(x: string, y: string) {
    super(
      "ERR_UNKNOWN_FILE_EXTENSION",
      `Unknown file extension "${x}" for ${y}`,
    );
  }
}
class ERR_UNKNOWN_MODULE_FORMAT extends NodeRangeError {
  constructor(x: string) {
    super("ERR_UNKNOWN_MODULE_FORMAT", `Unknown module format: ${x}`);
  }
}
class ERR_UNKNOWN_SIGNAL extends NodeTypeError {
  constructor(x: string) {
    super("ERR_UNKNOWN_SIGNAL", `Unknown signal: ${x}`);
  }
}
class ERR_UNSUPPORTED_DIR_IMPORT extends NodeError {
  constructor(x: string, y: string) {
    super(
      "ERR_UNSUPPORTED_DIR_IMPORT",
      `Directory import '${x}' is not supported resolving ES modules, imported from ${y}`,
    );
  }
}
class ERR_UNSUPPORTED_ESM_URL_SCHEME extends NodeError {
  constructor() {
    super(
      "ERR_UNSUPPORTED_ESM_URL_SCHEME",
      `Only file and data URLs are supported by the default ESM loader`,
    );
  }
}
class ERR_USE_AFTER_CLOSE extends NodeError {
  constructor(x: string) {
    super(
      "ERR_USE_AFTER_CLOSE",
      `${x} was closed`,
    );
  }
}
class ERR_V8BREAKITERATOR extends NodeError {
  constructor() {
    super(
      "ERR_V8BREAKITERATOR",
      `Full ICU data not installed. See https://github.com/nodejs/node/wiki/Intl`,
    );
  }
}
class ERR_VALID_PERFORMANCE_ENTRY_TYPE extends NodeError {
  constructor() {
    super(
      "ERR_VALID_PERFORMANCE_ENTRY_TYPE",
      `At least one valid performance entry type is required`,
    );
  }
}
class ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING extends NodeTypeError {
  constructor() {
    super(
      "ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING",
      `A dynamic import callback was not specified.`,
    );
  }
}
class ERR_VM_MODULE_ALREADY_LINKED extends NodeError {
  constructor() {
    super("ERR_VM_MODULE_ALREADY_LINKED", `Module has already been linked`);
  }
}
class ERR_VM_MODULE_CANNOT_CREATE_CACHED_DATA extends NodeError {
  constructor() {
    super(
      "ERR_VM_MODULE_CANNOT_CREATE_CACHED_DATA",
      `Cached data cannot be created for a module which has been evaluated`,
    );
  }
}
class ERR_VM_MODULE_DIFFERENT_CONTEXT extends NodeError {
  constructor() {
    super(
      "ERR_VM_MODULE_DIFFERENT_CONTEXT",
      `Linked modules must use the same context`,
    );
  }
}
class ERR_VM_MODULE_LINKING_ERRORED extends NodeError {
  constructor() {
    super(
      "ERR_VM_MODULE_LINKING_ERRORED",
      `Linking has already failed for the provided module`,
    );
  }
}
class ERR_VM_MODULE_NOT_MODULE extends NodeError {
  constructor() {
    super(
      "ERR_VM_MODULE_NOT_MODULE",
      `Provided module is not an instance of Module`,
    );
  }
}
class ERR_VM_MODULE_STATUS extends NodeError {
  constructor(x: string) {
    super("ERR_VM_MODULE_STATUS", `Module status ${x}`);
  }
}
class ERR_WASI_ALREADY_STARTED extends NodeError {
  constructor() {
    super("ERR_WASI_ALREADY_STARTED", `WASI instance has already started`);
  }
}
class ERR_WORKER_INVALID_EXEC_ARGV extends NodeError {
  constructor(errors: string[], msg = "invalid execArgv flags") {
    super(
      "ERR_WORKER_INVALID_EXEC_ARGV",
      `Initiated Worker with ${msg}: ${ArrayPrototypeJoin(errors, ", ")}`,
    );
  }
}
class ERR_WORKER_INIT_FAILED extends NodeError {
  constructor(x: string) {
    super("ERR_WORKER_INIT_FAILED", `Worker initialization failure: ${x}`);
  }
}
class ERR_WORKER_PATH extends NodeTypeError {
  constructor(filename: string) {
    const base =
      "The worker script or module filename must be an absolute path or a relative path starting with './' or '../'.";
    let detail = "";
    if (
      typeof filename === "string" &&
      (StringPrototypeStartsWith(filename, "file://") ||
        StringPrototypeStartsWith(filename, "File://"))
    ) {
      detail = " Wrap file:// URLs with `new URL`.";
    } else if (
      typeof filename === "string" &&
      StringPrototypeStartsWith(filename, "data:")
    ) {
      detail = " Wrap data: URLs with `new URL`.";
    }
    super("ERR_WORKER_PATH", base + detail);
  }
}
class ERR_WORKER_NOT_RUNNING extends NodeError {
  constructor() {
    super("ERR_WORKER_NOT_RUNNING", `Worker instance not running`);
  }
}
class ERR_WORKER_OUT_OF_MEMORY extends NodeError {
  constructor(x: string) {
    super(
      "ERR_WORKER_OUT_OF_MEMORY",
      `Worker terminated due to reaching memory limit: ${x}`,
    );
  }
}
class ERR_WORKER_UNSERIALIZABLE_ERROR extends NodeError {
  constructor() {
    super(
      "ERR_WORKER_UNSERIALIZABLE_ERROR",
      `Serializing an uncaught exception failed`,
    );
  }
}
class ERR_WORKER_UNSUPPORTED_EXTENSION extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_WORKER_UNSUPPORTED_EXTENSION",
      `The worker script extension must be ".js", ".mjs", or ".cjs". Received "${x}"`,
    );
  }
}
class ERR_WORKER_UNSUPPORTED_OPERATION extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_WORKER_UNSUPPORTED_OPERATION",
      `${x} is not supported in workers`,
    );
  }
}
class ERR_ZLIB_INITIALIZATION_FAILED extends NodeError {
  constructor(message = "Initialization failed") {
    super("ERR_ZLIB_INITIALIZATION_FAILED", message);
  }
}
class ERR_FALSY_VALUE_REJECTION extends NodeError {
  reason: string;
  constructor(reason: string) {
    super("ERR_FALSY_VALUE_REJECTION", "Promise was rejected with falsy value");
    this.reason = reason;
  }
}

class ERR_HTTP2_TOO_MANY_CUSTOM_SETTINGS extends NodeError {
  constructor() {
    super(
      "ERR_HTTP2_TOO_MANY_CUSTOM_SETTINGS",
      "Number of custom settings exceeds MAX_ADDITIONAL_SETTINGS",
    );
  }
}

function _http2InvalidSettingMsg(name: string, actual: unknown) {
  return `Invalid value for setting "${name}": ${actual}`;
}

// deno-lint-ignore camelcase
class _ERR_HTTP2_INVALID_SETTING_VALUE_TypeError extends NodeTypeError {
  actual: unknown;
  constructor(name: string, actual: unknown) {
    super(
      "ERR_HTTP2_INVALID_SETTING_VALUE",
      _http2InvalidSettingMsg(name, actual),
    );
    this.actual = actual;
  }
}

// deno-lint-ignore camelcase
class _ERR_HTTP2_INVALID_SETTING_VALUE_RangeError extends NodeRangeError {
  actual: unknown;
  min?: number;
  max?: number;

  constructor(name: string, actual: unknown, min?: number, max?: number) {
    super(
      "ERR_HTTP2_INVALID_SETTING_VALUE",
      _http2InvalidSettingMsg(name, actual),
    );
    this.actual = actual;
    if (min !== undefined) {
      this.min = min;
      this.max = max;
    }
  }
}

// In Node.js, ERR_HTTP2_INVALID_SETTING_VALUE has both TypeError and RangeError
// variants. The access patterns used are:
//   new ERR_HTTP2_INVALID_SETTING_VALUE.HideStackFramesError(...)            -> TypeError
//   new ERR_HTTP2_INVALID_SETTING_VALUE.RangeError(...)                      -> RangeError
//   new ERR_HTTP2_INVALID_SETTING_VALUE.RangeError.HideStackFramesError(...) -> RangeError

// deno-lint-ignore no-explicit-any
const _RangeErrorWithHSFE: any = _ERR_HTTP2_INVALID_SETTING_VALUE_RangeError;
_RangeErrorWithHSFE.HideStackFramesError =
  _ERR_HTTP2_INVALID_SETTING_VALUE_RangeError;

const ERR_HTTP2_INVALID_SETTING_VALUE = {
  HideStackFramesError: _ERR_HTTP2_INVALID_SETTING_VALUE_TypeError,
  RangeError: _RangeErrorWithHSFE,
};
class ERR_HTTP2_STREAM_CANCEL extends NodeError {
  override cause?: Error;
  constructor(error?: Error) {
    super(
      "ERR_HTTP2_STREAM_CANCEL",
      error && typeof error.message === "string"
        ? `The pending stream has been canceled (caused by: ${error.message})`
        : "The pending stream has been canceled",
    );
    if (error) {
      this.cause = error;
    }
  }
}

class ERR_INVALID_ADDRESS_FAMILY extends NodeRangeError {
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

class ERR_INVALID_CHAR extends NodeTypeError {
  constructor(name: string, field?: string) {
    super(
      "ERR_INVALID_CHAR",
      field === undefined
        ? `Invalid character in ${name}`
        : `Invalid character in ${name} ["${field}"]`,
    );
  }
}

class ERR_INVALID_OPT_VALUE extends NodeTypeError {
  constructor(name: string, value: unknown) {
    super(
      "ERR_INVALID_OPT_VALUE",
      `The value "${value}" is invalid for option "${name}"`,
    );
  }
}

class ERR_INVALID_RETURN_PROPERTY extends NodeTypeError {
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

class ERR_INVALID_RETURN_PROPERTY_VALUE extends NodeTypeError {
  constructor(input: string, name: string, prop: string, value: unknown) {
    super(
      "ERR_INVALID_RETURN_PROPERTY_VALUE",
      `Expected ${input} to be returned for the "${prop}" from the "${name}" function but got ${
        buildReturnPropertyType(
          value,
        )
      }.`,
    );
  }
}

class ERR_INVALID_RETURN_VALUE extends NodeTypeError {
  constructor(input: string, name: string, value: unknown) {
    super(
      "ERR_INVALID_RETURN_VALUE",
      `Expected ${input} to be returned from the "${name}" function but got ${
        determineSpecificType(
          value,
        )
      }.`,
    );
  }
}

class ERR_NOT_IMPLEMENTED extends NodeError {
  constructor(message?: string) {
    super(
      "ERR_NOT_IMPLEMENTED",
      message ? `Not implemented: ${message}` : "Not implemented",
    );
  }
}

class ERR_INVALID_URL extends NodeTypeError {
  input: string;
  constructor(input: string) {
    super("ERR_INVALID_URL", `Invalid URL: ${input}`);
    this.input = input;
  }
}

class ERR_INVALID_URL_SCHEME extends NodeTypeError {
  constructor(expected: string | [string] | [string, string]) {
    expected = ArrayIsArray(expected) ? expected : [expected];
    const res = expected.length === 2
      ? `one of scheme ${expected[0]} or ${expected[1]}`
      : `of scheme ${expected[0]}`;
    super("ERR_INVALID_URL_SCHEME", `The URL must be ${res}`);
  }
}

class ERR_MODULE_NOT_FOUND extends NodeError {
  constructor(path: string, base: string, type: string = "package") {
    super(
      "ERR_MODULE_NOT_FOUND",
      `Cannot find ${type} '${path}' imported from ${base}`,
    );
  }
}

class ERR_INVALID_PACKAGE_CONFIG extends NodeError {
  constructor(path: string, base?: string, message?: string) {
    const msg = `Invalid package config ${path}${
      base ? ` while importing ${base}` : ""
    }${message ? `. ${message}` : ""}`;
    super("ERR_INVALID_PACKAGE_CONFIG", msg);
  }
}

class ERR_INVALID_MODULE_SPECIFIER extends NodeTypeError {
  constructor(request: string, reason: string, base?: string) {
    super(
      "ERR_INVALID_MODULE_SPECIFIER",
      `Invalid module "${request}" ${reason}${
        base ? ` imported from ${base}` : ""
      }`,
    );
  }
}

class ERR_INVALID_PACKAGE_TARGET extends NodeError {
  constructor(
    pkgPath: string,
    key: string,
    // deno-lint-ignore no-explicit-any
    target: any,
    isImport?: boolean,
    base?: string,
  ) {
    let msg: string;
    const relError = typeof target === "string" &&
      !isImport &&
      target.length &&
      !StringPrototypeStartsWith(target, "./");
    if (key === ".") {
      assert ??= lazyLoadAssert();
      assert(isImport === false);
      msg = `Invalid "exports" main target ${JSONStringify(target)} defined ` +
        `in the package config ${displayJoin(pkgPath, "package.json")}${
          base ? ` imported from ${base}` : ""
        }${relError ? '; targets must start with "./"' : ""}`;
    } else {
      msg = `Invalid "${isImport ? "imports" : "exports"}" target ${
        JSONStringify(
          target,
        )
      } defined for '${key}' in the package config ${
        displayJoin(pkgPath, "package.json")
      }${base ? ` imported from ${base}` : ""}${
        relError ? '; targets must start with "./"' : ""
      }`;
    }
    super("ERR_INVALID_PACKAGE_TARGET", msg);
  }
}

class ERR_PACKAGE_IMPORT_NOT_DEFINED extends NodeTypeError {
  constructor(
    specifier: string,
    packagePath: string | undefined,
    base: string,
  ) {
    const msg = `Package import specifier "${specifier}" is not defined${
      packagePath
        ? ` in package ${displayJoin(packagePath, "package.json")}`
        : ""
    } imported from ${base}`;

    super("ERR_PACKAGE_IMPORT_NOT_DEFINED", msg);
  }
}

class ERR_PACKAGE_PATH_NOT_EXPORTED extends NodeError {
  constructor(subpath: string, pkgPath: string, basePath?: string) {
    let msg: string;
    if (subpath === ".") {
      msg = `No "exports" main defined in ${
        displayJoin(pkgPath, "package.json")
      }${basePath ? ` imported from ${basePath}` : ""}`;
    } else {
      msg = `Package subpath '${subpath}' is not defined by "exports" in ${
        displayJoin(pkgPath, "package.json")
      }${basePath ? ` imported from ${basePath}` : ""}`;
    }

    super("ERR_PACKAGE_PATH_NOT_EXPORTED", msg);
  }
}

class ERR_PARSE_ARGS_INVALID_OPTION_VALUE extends NodeTypeError {
  constructor(x: string) {
    super("ERR_PARSE_ARGS_INVALID_OPTION_VALUE", x);
  }
}

class ERR_PARSE_ARGS_UNEXPECTED_POSITIONAL extends NodeTypeError {
  constructor(x: string) {
    super(
      "ERR_PARSE_ARGS_UNEXPECTED_POSITIONAL",
      `Unexpected argument '${x}'. This ` +
        `command does not take positional arguments`,
    );
  }
}

class ERR_PARSE_ARGS_UNKNOWN_OPTION extends NodeTypeError {
  constructor(option, allowPositionals) {
    const suggestDashDash = allowPositionals
      ? ". To specify a positional " +
        "argument starting with a '-', place it at the end of the command after " +
        `'--', as in '-- ${JSONStringify(option)}`
      : "";
    super(
      "ERR_PARSE_ARGS_UNKNOWN_OPTION",
      `Unknown option '${option}'${suggestDashDash}`,
    );
  }
}

class ERR_INTERNAL_ASSERTION extends NodeError {
  constructor(message?: string) {
    const suffix = "This is caused by either a bug in Node.js " +
      "or incorrect usage of Node.js internals.\n" +
      "Please open an issue with this stack trace at " +
      "https://github.com/nodejs/node/issues\n";
    super(
      "ERR_INTERNAL_ASSERTION",
      message === undefined ? suffix : `${message}\n${suffix}`,
    );
  }
}

// Using `fs.rmdir` on a path that is a file results in an ENOENT error on Windows and an ENOTDIR error on POSIX.
class ERR_FS_RMDIR_ENOTDIR extends NodeSystemError {
  constructor(path: string) {
    const code = isWindows ? "ENOENT" : "ENOTDIR";
    const ctx: NodeSystemErrorCtx = {
      message: "not a directory",
      path,
      syscall: "rmdir",
      code,
      errno: isWindows ? osConstants.errno.ENOENT : osConstants.errno.ENOTDIR,
    };
    super(code, ctx, "Path is not a directory");
  }
}

class ERR_HTTP_SOCKET_ASSIGNED extends NodeError {
  constructor() {
    super(
      "ERR_HTTP_SOCKET_ASSIGNED",
      `ServerResponse has an already assigned socket`,
    );
  }
}

class ERR_INVALID_STATE extends NodeError {
  constructor(message: string) {
    super("ERR_INVALID_STATE", `Invalid state: ${message}`);
  }
}

interface UvExceptionContext {
  syscall: string;
  path?: string;
  dest?: string;
}
function denoErrorToNodeError(e: Error, ctx: UvExceptionContext) {
  if (ObjectPrototypeIsPrototypeOf(Deno.errors.BadResource.prototype, e)) {
    return uvException({
      errno: UV_EBADF,
      ...ctx,
    });
  }

  const errno = extractOsErrorNumberFromErrorMessage(e);
  if (typeof errno === "undefined") {
    return e;
  }

  const ex = uvException({
    errno: mapSysErrnoToUvErrno(errno),
    ...ctx,
  });
  return ex;
}

function denoWriteFileErrorToNodeError(
  e: Error,
  ctx: UvExceptionContext,
) {
  if (ObjectPrototypeIsPrototypeOf(Deno.errors.BadResource.prototype, e)) {
    return uvException({
      errno: UV_EBADF,
      ...ctx,
    });
  }

  let errno = extractOsErrorNumberFromErrorMessage(e);
  if (typeof errno === "undefined") {
    return e;
  }

  if (isWindows) {
    // https://learn.microsoft.com/en-us/windows/win32/debug/system-error-codes--0-499-#ERROR_ACCESS_DENIED
    const ERROR_ACCESS_DENIED = 5;
    // https://learn.microsoft.com/en-us/windows/win32/debug/system-error-codes--1000-1299-#ERROR_INVALID_FLAGS
    const ERROR_INVALID_FLAGS = 1004;

    // https://github.com/nodejs/node/blob/70f6b58ac655234435a99d72b857dd7b316d34bf/deps/uv/src/win/fs.c#L1090-L1092
    if (errno === ERROR_ACCESS_DENIED) {
      errno = ERROR_INVALID_FLAGS;
    }
  }

  return uvException({
    errno: mapSysErrnoToUvErrno(errno),
    ...ctx,
  });
}

const denoErrorToNodeSystemError = hideStackFrames((
  e: Error,
  syscall: string,
): Error => {
  const osErrno = extractOsErrorNumberFromErrorMessage(e);
  if (typeof osErrno === "undefined") {
    return e;
  }

  const uvErrno = mapSysErrnoToUvErrno(osErrno);
  const { 0: code, 1: message } = uvErrmapGet(uvErrno) || uvUnmappedError;
  const ctx: NodeSystemErrorCtx = {
    errno: uvErrno,
    code,
    message,
    syscall,
  };

  return new NodeSystemError(
    "ERR_SYSTEM_ERROR",
    ctx,
    "A system error occurred",
  );
});

function extractOsErrorNumberFromErrorMessage(e: unknown): number | undefined {
  if (typeof e === "object" && typeof e.os_errno === "number") {
    return e.os_errno;
  }

  const match = ObjectPrototypeIsPrototypeOf(ErrorPrototype, e)
    ? StringPrototypeMatch(e.message, new SafeRegExp(/\(os error (\d+)\)/))
    : false;

  if (match) {
    return +match[1];
  }

  return undefined;
}

function connResetException(msg: string) {
  const ex = new Error(msg);
  // deno-lint-ignore no-explicit-any
  (ex as any).code = "ECONNRESET";
  return ex;
}

function aggregateTwoErrors(
  innerError: AggregateError,
  outerError: AggregateError & { code: string },
) {
  if (innerError && outerError && innerError !== outerError) {
    if (ArrayIsArray(outerError.errors)) {
      // If `outerError` is already an `AggregateError`.
      ArrayPrototypePush(outerError.errors, innerError);
      return outerError;
    }
    // eslint-disable-next-line no-restricted-syntax
    const err = new AggregateError(
      [
        outerError,
        innerError,
      ],
      outerError.message,
    );
    // deno-lint-ignore no-explicit-any
    (err as any).code = outerError.code;
    ErrorCaptureStackTrace(err, aggregateTwoErrors);
    return err;
  }
  return innerError || outerError;
}

class NodeAggregateError extends AggregateError {
  code: string;
  constructor(errors, message) {
    super(new SafeArrayIterator(errors), message);
    this.code = errors[0]?.code;
  }

  get [kIsNodeError]() {
    return true;
  }

  // deno-lint-ignore adjacent-overload-signatures
  get ["constructor"]() {
    return AggregateError;
  }
}

codes.ERR_BUFFER_TOO_LARGE = ERR_BUFFER_TOO_LARGE;
codes.ERR_IPC_CHANNEL_CLOSED = ERR_IPC_CHANNEL_CLOSED;
codes.ERR_METHOD_NOT_IMPLEMENTED = ERR_METHOD_NOT_IMPLEMENTED;
codes.ERR_INVALID_RETURN_VALUE = ERR_INVALID_RETURN_VALUE;
codes.ERR_MISSING_ARGS = ERR_MISSING_ARGS;
codes.ERR_MULTIPLE_CALLBACK = ERR_MULTIPLE_CALLBACK;
codes.ERR_STREAM_WRITE_AFTER_END = ERR_STREAM_WRITE_AFTER_END;
codes.ERR_INVALID_ARG_TYPE = ERR_INVALID_ARG_TYPE;
codes.ERR_INVALID_ARG_VALUE = ERR_INVALID_ARG_VALUE;
codes.ERR_INVALID_HTTP_TOKEN = ERR_INVALID_HTTP_TOKEN;
codes.ERR_UNAVAILABLE_DURING_EXIT = ERR_UNAVAILABLE_DURING_EXIT;
codes.ERR_OUT_OF_RANGE = ERR_OUT_OF_RANGE;
codes.ERR_SOCKET_BAD_PORT = ERR_SOCKET_BAD_PORT;
codes.ERR_SOCKET_CONNECTION_TIMEOUT = ERR_SOCKET_CONNECTION_TIMEOUT;
codes.ERR_BUFFER_OUT_OF_BOUNDS = ERR_BUFFER_OUT_OF_BOUNDS;
codes.ERR_UNKNOWN_ENCODING = ERR_UNKNOWN_ENCODING;
codes.ERR_PARSE_ARGS_INVALID_OPTION_VALUE = ERR_PARSE_ARGS_INVALID_OPTION_VALUE;
codes.ERR_PARSE_ARGS_UNEXPECTED_POSITIONAL =
  ERR_PARSE_ARGS_UNEXPECTED_POSITIONAL;
codes.ERR_PARSE_ARGS_UNKNOWN_OPTION = ERR_PARSE_ARGS_UNKNOWN_OPTION;
codes.ERR_STREAM_ALREADY_FINISHED = ERR_STREAM_ALREADY_FINISHED;
codes.ERR_STREAM_CANNOT_PIPE = ERR_STREAM_CANNOT_PIPE;
codes.ERR_STREAM_DESTROYED = ERR_STREAM_DESTROYED;
codes.ERR_STREAM_NULL_VALUES = ERR_STREAM_NULL_VALUES;
codes.ERR_STREAM_PREMATURE_CLOSE = ERR_STREAM_PREMATURE_CLOSE;
codes.ERR_STREAM_PUSH_AFTER_EOF = ERR_STREAM_PUSH_AFTER_EOF;
codes.ERR_STREAM_UNSHIFT_AFTER_END_EVENT = ERR_STREAM_UNSHIFT_AFTER_END_EVENT;
codes.ERR_STREAM_WRAP = ERR_STREAM_WRAP;
codes.ERR_STREAM_WRITE_AFTER_END = ERR_STREAM_WRITE_AFTER_END;
codes.ERR_BROTLI_INVALID_PARAM = ERR_BROTLI_INVALID_PARAM;
codes.ERR_ZSTD_INVALID_PARAM = ERR_ZSTD_INVALID_PARAM;
codes.ERR_ZLIB_INITIALIZATION_FAILED = ERR_ZLIB_INITIALIZATION_FAILED;
codes.ERR_HTTP2_CONNECT_AUTHORITY = ERR_HTTP2_CONNECT_AUTHORITY;
codes.ERR_HTTP2_CONNECT_PATH = ERR_HTTP2_CONNECT_PATH;
codes.ERR_HTTP2_CONNECT_SCHEME = ERR_HTTP2_CONNECT_SCHEME;
codes.ERR_HTTP2_HEADER_SINGLE_VALUE = ERR_HTTP2_HEADER_SINGLE_VALUE;
codes.ERR_HTTP2_HEADERS_SENT = ERR_HTTP2_HEADERS_SENT;
codes.ERR_HTTP2_INFO_STATUS_NOT_ALLOWED = ERR_HTTP2_INFO_STATUS_NOT_ALLOWED;
codes.ERR_HTTP2_INVALID_CONNECTION_HEADERS =
  ERR_HTTP2_INVALID_CONNECTION_HEADERS;
codes.ERR_HTTP2_INVALID_HEADER_VALUE = ERR_HTTP2_INVALID_HEADER_VALUE;
codes.ERR_HTTP2_INVALID_PSEUDOHEADER = ERR_HTTP2_INVALID_PSEUDOHEADER;
codes.ERR_HTTP2_INVALID_SETTING_VALUE = ERR_HTTP2_INVALID_SETTING_VALUE;
codes.ERR_HTTP2_INVALID_STREAM = ERR_HTTP2_INVALID_STREAM;
codes.ERR_HTTP2_NO_SOCKET_MANIPULATION = ERR_HTTP2_NO_SOCKET_MANIPULATION;
codes.ERR_HTTP2_PAYLOAD_FORBIDDEN = ERR_HTTP2_PAYLOAD_FORBIDDEN;
codes.ERR_HTTP2_PSEUDOHEADER_NOT_ALLOWED = ERR_HTTP2_PSEUDOHEADER_NOT_ALLOWED;
codes.ERR_HTTP2_STATUS_INVALID = ERR_HTTP2_STATUS_INVALID;
codes.ERR_HTTP2_TOO_MANY_CUSTOM_SETTINGS = ERR_HTTP2_TOO_MANY_CUSTOM_SETTINGS;

// TODO(kt3k): assign all error classes here.

/**
 * This creates a generic Node.js error.
 *
 * @param message The error message.
 * @param errorProperties Object with additional properties to be added to the error.
 * @returns
 */
const genericNodeError = hideStackFrames(
  function genericNodeError(message, errorProperties) {
    // eslint-disable-next-line no-restricted-syntax
    const err = new Error(message);
    ObjectAssign(err, errorProperties);

    return err;
  },
);

/**
 * Determine the specific type of a value for type-mismatch errors.
 * @param {*} value
 * @returns {string}
 */
// deno-lint-ignore no-explicit-any
function determineSpecificType(value: any) {
  if (value == null) {
    return "" + value;
  }
  if (typeof value === "function" && value.name) {
    return `function ${value.name}`;
  }
  if (typeof value === "object") {
    if (value.constructor?.name) {
      return `an instance of ${value.constructor.name}`;
    }
    return `${inspect(value, { depth: -1 })}`;
  }
  let inspected = inspect(value, { colors: false });
  if (inspected.length > 28) {
    inspected = `${StringPrototypeSlice(inspected, 0, 25)}...`;
  }

  return `type ${typeof value} (${inspected})`;
}

// Non-robust path join
function displayJoin(dir: string, fileName: string) {
  const sep = StringPrototypeIncludes(dir, "\\") ? "\\" : "/";
  return StringPrototypeEndsWith(dir, sep)
    ? dir + fileName
    : dir + sep + fileName;
}

return {
  AbortError,
  ERR_AMBIGUOUS_ARGUMENT,
  ERR_ARG_NOT_ITERABLE,
  ERR_ASSERTION,
  ERR_ASYNC_CALLBACK,
  ERR_ASYNC_TYPE,
  ERR_BROTLI_INVALID_PARAM,
  ERR_ZSTD_INVALID_PARAM,
  ERR_BUFFER_OUT_OF_BOUNDS,
  ERR_BUFFER_TOO_LARGE,
  ERR_CANNOT_WATCH_SIGINT,
  ERR_CHILD_CLOSED_BEFORE_REPLY,
  ERR_CHILD_PROCESS_IPC_REQUIRED,
  ERR_CHILD_PROCESS_STDIO_MAXBUFFER,
  ERR_CONSOLE_WRITABLE_STREAM,
  ERR_CONSTRUCT_CALL_REQUIRED,
  ERR_CONTEXT_NOT_INITIALIZED,
  ERR_CPU_USAGE,
  ERR_CRYPTO_CUSTOM_ENGINE_NOT_SUPPORTED,
  ERR_CRYPTO_ECDH_INVALID_FORMAT,
  ERR_CRYPTO_ECDH_INVALID_PUBLIC_KEY,
  ERR_CRYPTO_ENGINE_UNKNOWN,
  ERR_CRYPTO_FIPS_FORCED,
  ERR_CRYPTO_FIPS_UNAVAILABLE,
  ERR_CRYPTO_HASH_FINALIZED,
  ERR_CRYPTO_HASH_UPDATE_FAILED,
  ERR_CRYPTO_INCOMPATIBLE_KEY,
  ERR_CRYPTO_INCOMPATIBLE_KEY_OPTIONS,
  ERR_CRYPTO_INVALID_DIGEST,
  ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE,
  ERR_CRYPTO_INVALID_JWK,
  ERR_CRYPTO_INVALID_SCRYPT_PARAMS,
  ERR_CRYPTO_INVALID_STATE,
  ERR_CRYPTO_PBKDF2_ERROR,
  ERR_CRYPTO_SCRYPT_INVALID_PARAMETER,
  ERR_CRYPTO_SCRYPT_NOT_SUPPORTED,
  ERR_CRYPTO_SIGN_KEY_REQUIRED,
  ERR_DIR_CLOSED,
  ERR_DIR_CONCURRENT_OPERATION,
  ERR_DNS_SET_SERVERS_FAILED,
  ERR_DOMAIN_CALLBACK_NOT_AVAILABLE,
  ERR_DOMAIN_CANNOT_SET_UNCAUGHT_EXCEPTION_CAPTURE,
  ERR_ENCODING_INVALID_ENCODED_DATA,
  ERR_ENCODING_NOT_SUPPORTED,
  ERR_EVAL_ESM_CANNOT_PRINT,
  ERR_EVENT_RECURSION,
  ERR_FALSY_VALUE_REJECTION,
  ERR_FEATURE_UNAVAILABLE_ON_PLATFORM,
  ERR_FS_CP_DIR_TO_NON_DIR,
  ERR_FS_CP_EEXIST,
  ERR_FS_CP_EINVAL,
  ERR_FS_EISDIR,
  ERR_FS_CP_FIFO_PIPE,
  ERR_FS_CP_NON_DIR_TO_DIR,
  ERR_FS_CP_SOCKET,
  ERR_FS_CP_SYMLINK_TO_SUBDIRECTORY,
  ERR_FS_CP_UNKNOWN,
  ERR_FS_FILE_TOO_LARGE,
  ERR_FS_INVALID_SYMLINK_TYPE,
  ERR_FS_RMDIR_ENOTDIR,
  ERR_HTTP2_ALTSVC_INVALID_ORIGIN,
  ERR_HTTP2_ALTSVC_LENGTH,
  ERR_HTTP2_CONNECT_AUTHORITY,
  ERR_HTTP2_CONNECT_PATH,
  ERR_HTTP2_CONNECT_SCHEME,
  ERR_HTTP2_GOAWAY_SESSION,
  ERR_HTTP2_HEADERS_AFTER_RESPOND,
  ERR_HTTP2_HEADERS_SENT,
  ERR_HTTP2_HEADER_SINGLE_VALUE,
  ERR_HTTP2_INFO_STATUS_NOT_ALLOWED,
  ERR_HTTP2_INVALID_CONNECTION_HEADERS,
  ERR_HTTP2_INVALID_HEADER_VALUE,
  ERR_HTTP2_INVALID_INFO_STATUS,
  ERR_HTTP2_INVALID_ORIGIN,
  ERR_HTTP2_INVALID_PACKED_SETTINGS_LENGTH,
  ERR_HTTP2_INVALID_PSEUDOHEADER,
  ERR_HTTP2_INVALID_SESSION,
  ERR_HTTP2_INVALID_SETTING_VALUE,
  ERR_HTTP2_INVALID_STREAM,
  ERR_HTTP2_MAX_PENDING_SETTINGS_ACK,
  ERR_HTTP2_NESTED_PUSH,
  ERR_HTTP2_NO_SOCKET_MANIPULATION,
  ERR_HTTP2_ORIGIN_LENGTH,
  ERR_HTTP2_OUT_OF_STREAMS,
  ERR_HTTP2_PAYLOAD_FORBIDDEN,
  ERR_HTTP2_PING_CANCEL,
  ERR_HTTP2_PING_LENGTH,
  ERR_HTTP2_PSEUDOHEADER_NOT_ALLOWED,
  ERR_HTTP2_PUSH_DISABLED,
  ERR_HTTP2_SEND_FILE,
  ERR_HTTP2_SEND_FILE_NOSEEK,
  ERR_HTTP2_SESSION_ERROR,
  ERR_HTTP2_SETTINGS_CANCEL,
  ERR_HTTP2_SOCKET_BOUND,
  ERR_HTTP2_SOCKET_UNBOUND,
  ERR_HTTP2_STATUS_101,
  ERR_HTTP2_STATUS_INVALID,
  ERR_HTTP2_STREAM_CANCEL,
  ERR_HTTP2_STREAM_ERROR,
  ERR_HTTP2_STREAM_SELF_DEPENDENCY,
  ERR_HTTP2_TRAILERS_ALREADY_SENT,
  ERR_HTTP2_TRAILERS_NOT_READY,
  ERR_HTTP2_UNSUPPORTED_PROTOCOL,
  ERR_HTTP_BODY_NOT_ALLOWED,
  ERR_HTTP_CONTENT_LENGTH_MISMATCH,
  ERR_HTTP_HEADERS_SENT,
  ERR_HTTP_INVALID_HEADER_VALUE,
  ERR_HTTP_INVALID_STATUS_CODE,
  ERR_HTTP_SOCKET_ENCODING,
  ERR_HTTP_TRAILER_INVALID,
  ERR_ILLEGAL_CONSTRUCTOR,
  ERR_INCOMPATIBLE_OPTION_PAIR,
  ERR_INPUT_TYPE_NOT_ALLOWED,
  ERR_INSPECTOR_ALREADY_ACTIVATED,
  ERR_INSPECTOR_ALREADY_CONNECTED,
  ERR_INSPECTOR_CLOSED,
  ERR_INSPECTOR_COMMAND,
  ERR_INSPECTOR_NOT_ACTIVE,
  ERR_INSPECTOR_NOT_AVAILABLE,
  ERR_INSPECTOR_NOT_CONNECTED,
  ERR_INSPECTOR_NOT_WORKER,
  ERR_INTERNAL_ASSERTION,
  ERR_INVALID_ADDRESS_FAMILY,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_TYPE_RANGE,
  ERR_CRYPTO_TIMING_SAFE_EQUAL_LENGTH,
  ERR_INVALID_ARG_VALUE,
  ERR_INVALID_ARG_VALUE_RANGE,
  ERR_INVALID_ASYNC_ID,
  ERR_INVALID_BUFFER_SIZE,
  ERR_INVALID_CHAR,
  ERR_INVALID_CURSOR_POS,
  ERR_INVALID_FD,
  ERR_INVALID_FD_TYPE,
  ERR_INVALID_FILE_URL_HOST,
  ERR_INVALID_FILE_URL_PATH,
  ERR_INVALID_HANDLE_TYPE,
  ERR_INVALID_HTTP_TOKEN,
  ERR_INVALID_IP_ADDRESS,
  ERR_IP_BLOCKED,
  ERR_INVALID_MIME_SYNTAX,
  ERR_INVALID_MODULE_SPECIFIER,
  ERR_INVALID_OBJECT_DEFINE_PROPERTY,
  ERR_INVALID_OPT_VALUE,
  ERR_INVALID_OPT_VALUE_ENCODING,
  ERR_INVALID_PACKAGE_CONFIG,
  ERR_INVALID_PACKAGE_TARGET,
  ERR_INVALID_PERFORMANCE_MARK,
  ERR_INVALID_PROTOCOL,
  ERR_INVALID_REPL_EVAL_CONFIG,
  ERR_INVALID_REPL_INPUT,
  ERR_INVALID_RETURN_PROPERTY,
  ERR_INVALID_RETURN_PROPERTY_VALUE,
  ERR_INVALID_RETURN_VALUE,
  ERR_INVALID_STATE,
  ERR_INVALID_SYNC_FORK_INPUT,
  ERR_INVALID_THIS,
  ERR_INVALID_TUPLE,
  ERR_NOT_IMPLEMENTED,
  ERR_INVALID_URI,
  ERR_INVALID_URL,
  ERR_INVALID_URL_SCHEME,
  ERR_IPC_CHANNEL_CLOSED,
  ERR_IPC_DISCONNECTED,
  ERR_IPC_ONE_PIPE,
  ERR_IPC_SYNC_FORK,
  ERR_MANIFEST_DEPENDENCY_MISSING,
  ERR_MANIFEST_INTEGRITY_MISMATCH,
  ERR_MANIFEST_INVALID_RESOURCE_FIELD,
  ERR_MANIFEST_TDZ,
  ERR_MANIFEST_UNKNOWN_ONERROR,
  ERR_METHOD_NOT_IMPLEMENTED,
  ERR_MISSING_ARGS,
  ERR_MISSING_OPTION,
  ERR_MODULE_NOT_FOUND,
  ERR_MULTIPLE_CALLBACK,
  ERR_NAPI_CONS_FUNCTION,
  ERR_NAPI_INVALID_DATAVIEW_ARGS,
  ERR_NAPI_INVALID_TYPEDARRAY_ALIGNMENT,
  ERR_NAPI_INVALID_TYPEDARRAY_LENGTH,
  ERR_NO_CRYPTO,
  ERR_NO_ICU,
  ERR_OUT_OF_RANGE,
  ERR_PACKAGE_IMPORT_NOT_DEFINED,
  ERR_PACKAGE_PATH_NOT_EXPORTED,
  ERR_PARSE_ARGS_INVALID_OPTION_VALUE,
  ERR_PARSE_ARGS_UNEXPECTED_POSITIONAL,
  ERR_PARSE_ARGS_UNKNOWN_OPTION,
  ERR_QUICCLIENTSESSION_FAILED,
  ERR_QUICCLIENTSESSION_FAILED_SETSOCKET,
  ERR_QUICSESSION_DESTROYED,
  ERR_QUICSESSION_INVALID_DCID,
  ERR_QUICSESSION_UPDATEKEY,
  ERR_QUICSOCKET_DESTROYED,
  ERR_QUICSOCKET_INVALID_STATELESS_RESET_SECRET_LENGTH,
  ERR_QUICSOCKET_LISTENING,
  ERR_QUICSOCKET_UNBOUND,
  ERR_QUICSTREAM_DESTROYED,
  ERR_QUICSTREAM_INVALID_PUSH,
  ERR_QUICSTREAM_OPEN_FAILED,
  ERR_QUICSTREAM_UNSUPPORTED_PUSH,
  ERR_QUIC_TLS13_REQUIRED,
  ERR_SCRIPT_EXECUTION_INTERRUPTED,
  ERR_SERVER_ALREADY_LISTEN,
  ERR_SERVER_NOT_RUNNING,
  ERR_SOCKET_ALREADY_BOUND,
  ERR_SOCKET_BAD_BUFFER_SIZE,
  ERR_SOCKET_BAD_PORT,
  ERR_SOCKET_BAD_TYPE,
  ERR_SOCKET_BUFFER_SIZE,
  ERR_SOCKET_CLOSED,
  ERR_SOCKET_CLOSED_BEFORE_CONNECTION,
  ERR_SOCKET_CONNECTION_TIMEOUT,
  ERR_SOCKET_DGRAM_IS_CONNECTED,
  ERR_SOCKET_DGRAM_NOT_CONNECTED,
  ERR_SOCKET_DGRAM_NOT_RUNNING,
  ERR_SRI_PARSE,
  ERR_STREAM_ALREADY_FINISHED,
  ERR_STREAM_CANNOT_PIPE,
  ERR_STREAM_DESTROYED,
  ERR_STREAM_NULL_VALUES,
  ERR_STREAM_PREMATURE_CLOSE,
  ERR_STREAM_PUSH_AFTER_EOF,
  ERR_STREAM_UNSHIFT_AFTER_END_EVENT,
  ERR_STREAM_WRAP,
  ERR_STREAM_WRITE_AFTER_END,
  ERR_SYNTHETIC,
  ERR_TLS_ALPN_CALLBACK_WITH_PROTOCOLS,
  ERR_TLS_CERT_ALTNAME_INVALID,
  ERR_TLS_DH_PARAM_SIZE,
  ERR_TLS_HANDSHAKE_TIMEOUT,
  ERR_TLS_INVALID_CONTEXT,
  ERR_TLS_INVALID_PROTOCOL_VERSION,
  ERR_TLS_INVALID_STATE,
  ERR_TLS_PROTOCOL_VERSION_CONFLICT,
  ERR_TLS_RENEGOTIATION_DISABLED,
  ERR_TLS_REQUIRED_SERVER_NAME,
  ERR_TLS_SESSION_ATTACK,
  ERR_TLS_SNI_FROM_SERVER,
  ERR_TRACE_EVENTS_CATEGORY_REQUIRED,
  ERR_TRACE_EVENTS_UNAVAILABLE,
  ERR_TTY_INIT_FAILED,
  ERR_UNAVAILABLE_DURING_EXIT,
  ERR_UNCAUGHT_EXCEPTION_CAPTURE_ALREADY_SET,
  ERR_UNESCAPED_CHARACTERS,
  ERR_UNHANDLED_ERROR,
  ERR_UNKNOWN_BUILTIN_MODULE,
  ERR_UNKNOWN_CREDENTIAL,
  ERR_UNKNOWN_ENCODING,
  ERR_UNKNOWN_FILE_EXTENSION,
  ERR_UNKNOWN_MODULE_FORMAT,
  ERR_UNKNOWN_SIGNAL,
  ERR_UNSUPPORTED_DIR_IMPORT,
  ERR_UNSUPPORTED_ESM_URL_SCHEME,
  ERR_USE_AFTER_CLOSE,
  ERR_V8BREAKITERATOR,
  ERR_VALID_PERFORMANCE_ENTRY_TYPE,
  ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING,
  ERR_VM_MODULE_ALREADY_LINKED,
  ERR_VM_MODULE_CANNOT_CREATE_CACHED_DATA,
  ERR_VM_MODULE_DIFFERENT_CONTEXT,
  ERR_VM_MODULE_LINKING_ERRORED,
  ERR_VM_MODULE_NOT_MODULE,
  ERR_VM_MODULE_STATUS,
  ERR_WASI_ALREADY_STARTED,
  ERR_WORKER_INIT_FAILED,
  ERR_WORKER_INVALID_EXEC_ARGV,
  ERR_WORKER_NOT_RUNNING,
  ERR_WORKER_OUT_OF_MEMORY,
  ERR_WORKER_PATH,
  ERR_WORKER_UNSERIALIZABLE_ERROR,
  ERR_WORKER_UNSUPPORTED_EXTENSION,
  ERR_WORKER_UNSUPPORTED_OPERATION,
  ERR_ZLIB_INITIALIZATION_FAILED,
  ERR_CRYPTO_UNKNOWN_DH_GROUP,
  ERR_CRYPTO_UNKNOWN_CIPHER,
  ERR_CRYPTO_INVALID_KEYLEN,
  ERR_HTTP2_TOO_MANY_CUSTOM_SETTINGS,
  NodeError,
  NodeErrorAbstraction,
  NodeRangeError,
  NodeSyntaxError,
  NodeTypeError,
  NodeURIError,
  NodeAggregateError,
  aggregateTwoErrors,
  codes,
  connResetException,
  denoErrorToNodeError,
  denoErrorToNodeSystemError,
  denoWriteFileErrorToNodeError,
  dnsException,
  DNSException: dnsException,
  errnoException,
  errorMap,
  exceptionWithHostPort,
  genericNodeError,
  handleDnsError,
  hideStackFrames,
  isErrorStackTraceLimitWritable,
  isStackOverflowError,
  uvException,
  uvExceptionWithHostPort,
  default: {
    AbortError,
    ERR_AMBIGUOUS_ARGUMENT,
    ERR_ARG_NOT_ITERABLE,
    ERR_ASSERTION,
    ERR_ASYNC_CALLBACK,
    ERR_ASYNC_TYPE,
    ERR_BROTLI_INVALID_PARAM,
    ERR_ZSTD_INVALID_PARAM,
    ERR_BUFFER_OUT_OF_BOUNDS,
    ERR_BUFFER_TOO_LARGE,
    ERR_CANNOT_WATCH_SIGINT,
    ERR_CHILD_CLOSED_BEFORE_REPLY,
    ERR_CHILD_PROCESS_IPC_REQUIRED,
    ERR_CHILD_PROCESS_STDIO_MAXBUFFER,
    ERR_CONSOLE_WRITABLE_STREAM,
    ERR_CONSTRUCT_CALL_REQUIRED,
    ERR_CONTEXT_NOT_INITIALIZED,
    ERR_CPU_USAGE,
    ERR_CRYPTO_CUSTOM_ENGINE_NOT_SUPPORTED,
    ERR_CRYPTO_ECDH_INVALID_FORMAT,
    ERR_CRYPTO_ECDH_INVALID_PUBLIC_KEY,
    ERR_CRYPTO_ENGINE_UNKNOWN,
    ERR_CRYPTO_FIPS_FORCED,
    ERR_CRYPTO_FIPS_UNAVAILABLE,
    ERR_CRYPTO_HASH_FINALIZED,
    ERR_CRYPTO_HASH_UPDATE_FAILED,
    ERR_CRYPTO_INCOMPATIBLE_KEY,
    ERR_CRYPTO_INCOMPATIBLE_KEY_OPTIONS,
    ERR_CRYPTO_INVALID_DIGEST,
    ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE,
    ERR_CRYPTO_INVALID_JWK,
    ERR_CRYPTO_INVALID_SCRYPT_PARAMS,
    ERR_CRYPTO_INVALID_STATE,
    ERR_CRYPTO_PBKDF2_ERROR,
    ERR_CRYPTO_SCRYPT_INVALID_PARAMETER,
    ERR_CRYPTO_SCRYPT_NOT_SUPPORTED,
    ERR_CRYPTO_SIGN_KEY_REQUIRED,
    ERR_DIR_CLOSED,
    ERR_DIR_CONCURRENT_OPERATION,
    ERR_DNS_SET_SERVERS_FAILED,
    ERR_DOMAIN_CALLBACK_NOT_AVAILABLE,
    ERR_DOMAIN_CANNOT_SET_UNCAUGHT_EXCEPTION_CAPTURE,
    ERR_ENCODING_INVALID_ENCODED_DATA,
    ERR_ENCODING_NOT_SUPPORTED,
    ERR_EVAL_ESM_CANNOT_PRINT,
    ERR_EVENT_RECURSION,
    ERR_FALSY_VALUE_REJECTION,
    ERR_FEATURE_UNAVAILABLE_ON_PLATFORM,
    ERR_FS_CP_DIR_TO_NON_DIR,
    ERR_FS_CP_EEXIST,
    ERR_FS_CP_EINVAL,
    ERR_FS_EISDIR,
    ERR_FS_CP_FIFO_PIPE,
    ERR_FS_CP_NON_DIR_TO_DIR,
    ERR_FS_CP_SOCKET,
    ERR_FS_CP_SYMLINK_TO_SUBDIRECTORY,
    ERR_FS_CP_UNKNOWN,
    ERR_FS_FILE_TOO_LARGE,
    ERR_FS_INVALID_SYMLINK_TYPE,
    ERR_FS_RMDIR_ENOTDIR,
    ERR_HTTP2_ALTSVC_INVALID_ORIGIN,
    ERR_HTTP2_ALTSVC_LENGTH,
    ERR_HTTP2_CONNECT_AUTHORITY,
    ERR_HTTP2_CONNECT_PATH,
    ERR_HTTP2_CONNECT_SCHEME,
    ERR_HTTP2_GOAWAY_SESSION,
    ERR_HTTP2_HEADERS_AFTER_RESPOND,
    ERR_HTTP2_HEADERS_SENT,
    ERR_HTTP2_HEADER_SINGLE_VALUE,
    ERR_HTTP2_INFO_STATUS_NOT_ALLOWED,
    ERR_HTTP2_INVALID_CONNECTION_HEADERS,
    ERR_HTTP2_INVALID_HEADER_VALUE,
    ERR_HTTP2_INVALID_INFO_STATUS,
    ERR_HTTP2_INVALID_ORIGIN,
    ERR_HTTP2_INVALID_PACKED_SETTINGS_LENGTH,
    ERR_HTTP2_INVALID_PSEUDOHEADER,
    ERR_HTTP2_INVALID_SESSION,
    ERR_HTTP2_INVALID_SETTING_VALUE,
    ERR_HTTP2_INVALID_STREAM,
    ERR_HTTP2_MAX_PENDING_SETTINGS_ACK,
    ERR_HTTP2_NESTED_PUSH,
    ERR_HTTP2_NO_SOCKET_MANIPULATION,
    ERR_HTTP2_ORIGIN_LENGTH,
    ERR_HTTP2_OUT_OF_STREAMS,
    ERR_HTTP2_PAYLOAD_FORBIDDEN,
    ERR_HTTP2_PING_CANCEL,
    ERR_HTTP2_PING_LENGTH,
    ERR_HTTP2_PSEUDOHEADER_NOT_ALLOWED,
    ERR_HTTP2_PUSH_DISABLED,
    ERR_HTTP2_SEND_FILE,
    ERR_HTTP2_SEND_FILE_NOSEEK,
    ERR_HTTP2_SESSION_ERROR,
    ERR_HTTP2_SETTINGS_CANCEL,
    ERR_HTTP2_SOCKET_BOUND,
    ERR_HTTP2_SOCKET_UNBOUND,
    ERR_HTTP2_STATUS_101,
    ERR_HTTP2_STATUS_INVALID,
    ERR_HTTP2_STREAM_CANCEL,
    ERR_HTTP2_STREAM_ERROR,
    ERR_HTTP2_STREAM_SELF_DEPENDENCY,
    ERR_HTTP2_TRAILERS_ALREADY_SENT,
    ERR_HTTP2_TRAILERS_NOT_READY,
    ERR_HTTP2_UNSUPPORTED_PROTOCOL,
    ERR_HTTP_BODY_NOT_ALLOWED,
    ERR_HTTP_HEADERS_SENT,
    ERR_HTTP_INVALID_HEADER_VALUE,
    ERR_HTTP_INVALID_STATUS_CODE,
    ERR_HTTP_SOCKET_ASSIGNED,
    ERR_HTTP_SOCKET_ENCODING,
    ERR_HTTP_TRAILER_INVALID,
    ERR_INCOMPATIBLE_OPTION_PAIR,
    ERR_INPUT_TYPE_NOT_ALLOWED,
    ERR_INSPECTOR_ALREADY_ACTIVATED,
    ERR_INSPECTOR_ALREADY_CONNECTED,
    ERR_INSPECTOR_CLOSED,
    ERR_INSPECTOR_COMMAND,
    ERR_INSPECTOR_NOT_ACTIVE,
    ERR_INSPECTOR_NOT_AVAILABLE,
    ERR_INSPECTOR_NOT_CONNECTED,
    ERR_INSPECTOR_NOT_WORKER,
    ERR_INTERNAL_ASSERTION,
    ERR_INVALID_ADDRESS_FAMILY,
    ERR_INVALID_ARG_TYPE,
    ERR_INVALID_ARG_TYPE_RANGE,
    ERR_CRYPTO_TIMING_SAFE_EQUAL_LENGTH,
    ERR_INVALID_ARG_VALUE,
    ERR_INVALID_ARG_VALUE_RANGE,
    ERR_INVALID_ASYNC_ID,
    ERR_INVALID_BUFFER_SIZE,
    ERR_INVALID_CHAR,
    ERR_INVALID_CURSOR_POS,
    ERR_INVALID_FD,
    ERR_INVALID_FD_TYPE,
    ERR_INVALID_FILE_URL_HOST,
    ERR_INVALID_FILE_URL_PATH,
    ERR_INVALID_HANDLE_TYPE,
    ERR_INVALID_HTTP_TOKEN,
    ERR_INVALID_IP_ADDRESS,
    ERR_INVALID_MIME_SYNTAX,
    ERR_INVALID_MODULE_SPECIFIER,
    ERR_INVALID_OBJECT_DEFINE_PROPERTY,
    ERR_INVALID_OPT_VALUE,
    ERR_INVALID_OPT_VALUE_ENCODING,
    ERR_INVALID_PACKAGE_CONFIG,
    ERR_INVALID_PACKAGE_TARGET,
    ERR_INVALID_PERFORMANCE_MARK,
    ERR_INVALID_PROTOCOL,
    ERR_INVALID_REPL_EVAL_CONFIG,
    ERR_INVALID_REPL_INPUT,
    ERR_INVALID_RETURN_PROPERTY,
    ERR_INVALID_RETURN_PROPERTY_VALUE,
    ERR_INVALID_RETURN_VALUE,
    ERR_INVALID_STATE,
    ERR_INVALID_SYNC_FORK_INPUT,
    ERR_INVALID_THIS,
    ERR_INVALID_TUPLE,
    ERR_NOT_IMPLEMENTED,
    ERR_INVALID_URI,
    ERR_INVALID_URL,
    ERR_INVALID_URL_SCHEME,
    ERR_IPC_CHANNEL_CLOSED,
    ERR_IPC_DISCONNECTED,
    ERR_IPC_ONE_PIPE,
    ERR_IPC_SYNC_FORK,
    ERR_MANIFEST_DEPENDENCY_MISSING,
    ERR_MANIFEST_INTEGRITY_MISMATCH,
    ERR_MANIFEST_INVALID_RESOURCE_FIELD,
    ERR_MANIFEST_TDZ,
    ERR_MANIFEST_UNKNOWN_ONERROR,
    ERR_METHOD_NOT_IMPLEMENTED,
    ERR_MISSING_ARGS,
    ERR_MISSING_OPTION,
    ERR_MODULE_NOT_FOUND,
    ERR_MULTIPLE_CALLBACK,
    ERR_NAPI_CONS_FUNCTION,
    ERR_NAPI_INVALID_DATAVIEW_ARGS,
    ERR_NAPI_INVALID_TYPEDARRAY_ALIGNMENT,
    ERR_NAPI_INVALID_TYPEDARRAY_LENGTH,
    ERR_NO_CRYPTO,
    ERR_NO_ICU,
    ERR_OUT_OF_RANGE,
    ERR_PACKAGE_IMPORT_NOT_DEFINED,
    ERR_PACKAGE_PATH_NOT_EXPORTED,
    ERR_PARSE_ARGS_INVALID_OPTION_VALUE,
    ERR_QUICCLIENTSESSION_FAILED,
    ERR_QUICCLIENTSESSION_FAILED_SETSOCKET,
    ERR_QUICSESSION_DESTROYED,
    ERR_QUICSESSION_INVALID_DCID,
    ERR_QUICSESSION_UPDATEKEY,
    ERR_QUICSOCKET_DESTROYED,
    ERR_QUICSOCKET_INVALID_STATELESS_RESET_SECRET_LENGTH,
    ERR_QUICSOCKET_LISTENING,
    ERR_QUICSOCKET_UNBOUND,
    ERR_QUICSTREAM_DESTROYED,
    ERR_QUICSTREAM_INVALID_PUSH,
    ERR_QUICSTREAM_OPEN_FAILED,
    ERR_QUICSTREAM_UNSUPPORTED_PUSH,
    ERR_QUIC_TLS13_REQUIRED,
    ERR_SCRIPT_EXECUTION_INTERRUPTED,
    ERR_SERVER_ALREADY_LISTEN,
    ERR_SERVER_NOT_RUNNING,
    ERR_SOCKET_ALREADY_BOUND,
    ERR_SOCKET_BAD_BUFFER_SIZE,
    ERR_SOCKET_BAD_PORT,
    ERR_SOCKET_BAD_TYPE,
    ERR_SOCKET_BUFFER_SIZE,
    ERR_SOCKET_CLOSED,
    ERR_SOCKET_DGRAM_IS_CONNECTED,
    ERR_SOCKET_DGRAM_NOT_CONNECTED,
    ERR_SOCKET_DGRAM_NOT_RUNNING,
    ERR_SRI_PARSE,
    ERR_STREAM_ALREADY_FINISHED,
    ERR_STREAM_CANNOT_PIPE,
    ERR_STREAM_DESTROYED,
    ERR_STREAM_NULL_VALUES,
    ERR_STREAM_PREMATURE_CLOSE,
    ERR_STREAM_PUSH_AFTER_EOF,
    ERR_STREAM_UNSHIFT_AFTER_END_EVENT,
    ERR_STREAM_WRAP,
    ERR_STREAM_WRITE_AFTER_END,
    ERR_SYNTHETIC,
    ERR_TLS_ALPN_CALLBACK_WITH_PROTOCOLS,
    ERR_TLS_CERT_ALTNAME_INVALID,
    ERR_TLS_DH_PARAM_SIZE,
    ERR_TLS_HANDSHAKE_TIMEOUT,
    ERR_TLS_INVALID_CONTEXT,
    ERR_TLS_INVALID_PROTOCOL_VERSION,
    ERR_TLS_INVALID_STATE,
    ERR_TLS_PROTOCOL_VERSION_CONFLICT,
    ERR_TLS_RENEGOTIATION_DISABLED,
    ERR_TLS_REQUIRED_SERVER_NAME,
    ERR_TLS_SESSION_ATTACK,
    ERR_TLS_SNI_FROM_SERVER,
    ERR_TRACE_EVENTS_CATEGORY_REQUIRED,
    ERR_TRACE_EVENTS_UNAVAILABLE,
    ERR_UNAVAILABLE_DURING_EXIT,
    ERR_UNCAUGHT_EXCEPTION_CAPTURE_ALREADY_SET,
    ERR_UNESCAPED_CHARACTERS,
    ERR_UNHANDLED_ERROR,
    ERR_UNKNOWN_BUILTIN_MODULE,
    ERR_UNKNOWN_CREDENTIAL,
    ERR_UNKNOWN_ENCODING,
    ERR_UNKNOWN_FILE_EXTENSION,
    ERR_UNKNOWN_MODULE_FORMAT,
    ERR_UNKNOWN_SIGNAL,
    ERR_UNSUPPORTED_DIR_IMPORT,
    ERR_UNSUPPORTED_ESM_URL_SCHEME,
    ERR_USE_AFTER_CLOSE,
    ERR_V8BREAKITERATOR,
    ERR_VALID_PERFORMANCE_ENTRY_TYPE,
    ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING,
    ERR_VM_MODULE_ALREADY_LINKED,
    ERR_VM_MODULE_CANNOT_CREATE_CACHED_DATA,
    ERR_VM_MODULE_DIFFERENT_CONTEXT,
    ERR_VM_MODULE_LINKING_ERRORED,
    ERR_VM_MODULE_NOT_MODULE,
    ERR_VM_MODULE_STATUS,
    ERR_WASI_ALREADY_STARTED,
    ERR_WORKER_INIT_FAILED,
    ERR_WORKER_INVALID_EXEC_ARGV,
    ERR_WORKER_NOT_RUNNING,
    ERR_WORKER_OUT_OF_MEMORY,
    ERR_WORKER_PATH,
    ERR_WORKER_UNSERIALIZABLE_ERROR,
    ERR_WORKER_UNSUPPORTED_EXTENSION,
    ERR_WORKER_UNSUPPORTED_OPERATION,
    ERR_ZLIB_INITIALIZATION_FAILED,
    NodeError,
    NodeErrorAbstraction,
    NodeRangeError,
    NodeSyntaxError,
    NodeTypeError,
    NodeURIError,
    aggregateTwoErrors,
    codes,
    connResetException,
    denoErrorToNodeError,
    denoErrorToNodeSystemError,
    dnsException,
    DNSException: dnsException,
    errnoException,
    errorMap,
    exceptionWithHostPort,
    genericNodeError,
    hideStackFrames,
    isErrorStackTraceLimitWritable,
    isStackOverflowError,
    uvException,
    uvExceptionWithHostPort,
  },
};
})()
