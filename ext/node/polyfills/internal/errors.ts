// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Node.js contributors. All rights reserved. MIT License.

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
  const { core, primordials } = __bootstrap;
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
    ArrayPrototypeForEach,
    ArrayPrototypeSplice,
    Error,
    ErrorPrototype,
    ErrorCaptureStackTrace,
    JSONStringify,
    MapPrototypeGet,
    MapPrototypeSet,
    SafeMap,
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
  const { format, inspect } = core.loadExtScript(
    "ext:deno_node/internal/util/inspect.mjs",
  );
  const { codes } = core.loadExtScript("ext:deno_node/internal/error_codes.ts");
  const {
    codeMap,
    errorMap,
    mapSysErrnoToUvErrno,
    UV_EBADF,
  } = core.loadExtScript("ext:deno_node/internal_binding/uv.ts");
  const { isWindows } = core.loadExtScript("ext:deno_node/_util/os.ts");
  const { os: osConstants } = core.loadExtScript(
    "ext:deno_node/internal_binding/constants.ts",
  );
  const { hideStackFrames } = core.loadExtScript(
    "ext:deno_node/internal/hide_stack_frames.ts",
  );

  // Lazy loader for getSystemErrorName to break circular dep with _utils.ts
  let _getSystemErrorName;
  function getSystemErrorName(code) {
    if (!_getSystemErrorName) {
      _getSystemErrorName =
        core.loadExtScript("ext:deno_node/_utils.ts").getSystemErrorName;
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

  class NodeSyntaxError extends NodeErrorAbstraction implements SyntaxError {
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

  type NodeErrorBaseClass = new (
    code: string,
    message: string,
  ) => NodeErrorAbstraction;

  // deno-lint-ignore no-explicit-any
  type NodeErrorCodeConstructor = new (...args: any[]) => NodeErrorAbstraction;

  function defineNodeError(
    code: string,
    Base: NodeErrorBaseClass,
    // deno-lint-ignore no-explicit-any
    format: (...args: any[]) => string,
  ): NodeErrorCodeConstructor {
    return {
      [code]: class extends Base {
        // deno-lint-ignore no-explicit-any
        constructor(...args: any[]) {
          super(code, format(...new SafeArrayIterator(args)));
        }
      },
    }[code];
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

  // `SystemError` alias used by `--expose-internals` consumers (e.g. Node's
  // own test suite) that import `internal/errors`.
  const SystemError = NodeSystemError;

  // Mirror of Node's `messages` map: associates an error code with its message
  // template (string or function). Populated by `E()` and read by
  // `makeNodeErrorWithCode()`.
  // deno-lint-ignore no-explicit-any
  const messages = new SafeMap<string, string | ((...args: any[]) => string)>();

  // Builds a class that extends a non-system Error base (Error, TypeError, etc.)
  // so that `E()` can register codes whose parent isn't `SystemError`.
  // Mirrors Node's `makeNodeErrorWithCode(Base, key)`.
  function makeNodeErrorWithCode(Base: typeof Error, key: string) {
    return class NodeErrorWithCode extends Base {
      // deno-lint-ignore no-explicit-any
      constructor(...args: any[]) {
        const template = MapPrototypeGet(messages, key);
        super(
          typeof template === "function"
            // deno-lint-ignore no-explicit-any
            ? (template as (...a: any[]) => string)(
              ...new SafeArrayIterator(args),
            )
            : template as string,
        );
        this.code = key;
        this[kIsNodeError] = true;
        this.toString = function () {
          return `${this.name} [${this.code}]: ${this.message}`;
        };
      }
    };
  }

  // Mirrors Node's `lib/internal/errors.js`'s `E(sym, val, def, ...otherClasses)`
  // helper: registers a new error class on `codes[sym]`. When `def` is
  // `SystemError`, the class extends `NodeSystemError` and treats `val` as the
  // message prefix (matching Node's `makeSystemErrorWithCode`). Otherwise the
  // class extends `def` (Error/TypeError/RangeError/...) and uses `val` as the
  // message template.
  function E(
    sym: string,
    // deno-lint-ignore no-explicit-any
    val: string | ((...args: any[]) => string),
    // deno-lint-ignore no-explicit-any
    def: any,
    // deno-lint-ignore no-explicit-any
    ...otherClasses: any[]
  ) {
    MapPrototypeSet(messages, sym, val);
    let cls;
    if (def === SystemError) {
      cls = makeSystemErrorWithCode(sym, val as string);
    } else {
      cls = makeNodeErrorWithCode(def, sym);
    }
    if (otherClasses.length !== 0) {
      ArrayPrototypeForEach(otherClasses, (clazz) => {
        cls[clazz.name] = makeNodeErrorWithCode(clazz, sym);
      });
    }
    codes[sym] = cls;
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

  const ERR_CRYPTO_TIMING_SAFE_EQUAL_LENGTH = defineNodeError(
    "ERR_CRYPTO_TIMING_SAFE_EQUAL_LENGTH",
    NodeRangeError,
    () => "Input buffers must have the same byte length",
  );

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

  const ERR_AMBIGUOUS_ARGUMENT = defineNodeError(
    "ERR_AMBIGUOUS_ARGUMENT",
    NodeTypeError,
    (x: string, y: string) => `The "${x}" argument is ambiguous. ${y}`,
  );

  const ERR_ARG_NOT_ITERABLE = defineNodeError(
    "ERR_ARG_NOT_ITERABLE",
    NodeTypeError,
    (x: string) => `${x} must be iterable`,
  );

  const ERR_ASSERTION = defineNodeError(
    "ERR_ASSERTION",
    NodeError,
    (x: string) => `${x}`,
  );

  const ERR_ASYNC_CALLBACK = defineNodeError(
    "ERR_ASYNC_CALLBACK",
    NodeTypeError,
    (x: string) => `${x} must be a function`,
  );

  const ERR_ASYNC_TYPE = defineNodeError(
    "ERR_ASYNC_TYPE",
    NodeTypeError,
    (x: string) => `Invalid name for async "type": ${x}`,
  );

  const ERR_BROTLI_INVALID_PARAM = defineNodeError(
    "ERR_BROTLI_INVALID_PARAM",
    NodeRangeError,
    (x: string) => `${x} is not a valid Brotli parameter`,
  );

  const ERR_ZSTD_INVALID_PARAM = defineNodeError(
    "ERR_ZSTD_INVALID_PARAM",
    NodeRangeError,
    (x: string) => `${x} is not a valid zstd parameter`,
  );

  const ERR_BUFFER_OUT_OF_BOUNDS = defineNodeError(
    "ERR_BUFFER_OUT_OF_BOUNDS",
    NodeRangeError,
    (name?: string) =>
      name
        ? `"${name}" is outside of buffer bounds`
        : "Attempt to access memory outside buffer bounds",
  );

  const ERR_BUFFER_TOO_LARGE = defineNodeError(
    "ERR_BUFFER_TOO_LARGE",
    NodeRangeError,
    (x: string) => `Cannot create a Buffer larger than ${x} bytes`,
  );

  const ERR_CANNOT_WATCH_SIGINT = defineNodeError(
    "ERR_CANNOT_WATCH_SIGINT",
    NodeError,
    () => "Cannot watch for SIGINT signals",
  );

  const ERR_CHILD_CLOSED_BEFORE_REPLY = defineNodeError(
    "ERR_CHILD_CLOSED_BEFORE_REPLY",
    NodeError,
    () => "Child closed before reply received",
  );

  const ERR_CHILD_PROCESS_IPC_REQUIRED = defineNodeError(
    "ERR_CHILD_PROCESS_IPC_REQUIRED",
    NodeError,
    (x: string) =>
      `Forked processes must have an IPC channel, missing value 'ipc' in ${x}`,
  );

  const ERR_CHILD_PROCESS_STDIO_MAXBUFFER = defineNodeError(
    "ERR_CHILD_PROCESS_STDIO_MAXBUFFER",
    NodeRangeError,
    (x: string) => `${x} maxBuffer length exceeded`,
  );

  const ERR_CONSOLE_WRITABLE_STREAM = defineNodeError(
    "ERR_CONSOLE_WRITABLE_STREAM",
    NodeTypeError,
    (x: string) => `Console expects a writable stream instance for ${x}`,
  );

  const ERR_CONSTRUCT_CALL_REQUIRED = defineNodeError(
    "ERR_CONSTRUCT_CALL_REQUIRED",
    NodeTypeError,
    (x: string) => `Class constructor ${x} cannot be invoked without \`new\``,
  );

  const ERR_CONSTRUCT_CALL_INVALID = defineNodeError(
    "ERR_CONSTRUCT_CALL_INVALID",
    NodeTypeError,
    (x: string) => `Constructor for class ${x} cannot be invoked`,
  );

  const ERR_CLOSED_MESSAGE_PORT = defineNodeError(
    "ERR_CLOSED_MESSAGE_PORT",
    NodeError,
    () => "Cannot send data on closed MessagePort",
  );

  const ERR_CONTEXT_NOT_INITIALIZED = defineNodeError(
    "ERR_CONTEXT_NOT_INITIALIZED",
    NodeError,
    () => "context used is not initialized",
  );

  const ERR_CPU_USAGE = defineNodeError(
    "ERR_CPU_USAGE",
    NodeError,
    (x: string) => `Unable to obtain cpu usage ${x}`,
  );

  const ERR_CRYPTO_CUSTOM_ENGINE_NOT_SUPPORTED = defineNodeError(
    "ERR_CRYPTO_CUSTOM_ENGINE_NOT_SUPPORTED",
    NodeError,
    () => "Custom engines not supported by this OpenSSL",
  );

  const ERR_CRYPTO_ECDH_INVALID_FORMAT = defineNodeError(
    "ERR_CRYPTO_ECDH_INVALID_FORMAT",
    NodeTypeError,
    (x: string) => `Invalid ECDH format: ${x}`,
  );

  const ERR_CRYPTO_ECDH_INVALID_PUBLIC_KEY = defineNodeError(
    "ERR_CRYPTO_ECDH_INVALID_PUBLIC_KEY",
    NodeError,
    () => "Public key is not valid for specified curve",
  );

  const ERR_CRYPTO_UNKNOWN_DH_GROUP = defineNodeError(
    "ERR_CRYPTO_UNKNOWN_DH_GROUP",
    NodeError,
    () => "Unknown DH group",
  );

  const ERR_CRYPTO_UNKNOWN_CIPHER = defineNodeError(
    "ERR_CRYPTO_UNKNOWN_CIPHER",
    NodeError,
    () => "Unknown cipher",
  );

  const ERR_CRYPTO_ENGINE_UNKNOWN = defineNodeError(
    "ERR_CRYPTO_ENGINE_UNKNOWN",
    NodeError,
    (x: string) => `Engine "${x}" was not found`,
  );

  const ERR_CRYPTO_FIPS_FORCED = defineNodeError(
    "ERR_CRYPTO_FIPS_FORCED",
    NodeError,
    () => "Cannot set FIPS mode, it was forced with --force-fips at startup.",
  );

  const ERR_CRYPTO_FIPS_UNAVAILABLE = defineNodeError(
    "ERR_CRYPTO_FIPS_UNAVAILABLE",
    NodeError,
    () => "Cannot set FIPS mode in a non-FIPS build.",
  );

  const ERR_CRYPTO_HASH_FINALIZED = defineNodeError(
    "ERR_CRYPTO_HASH_FINALIZED",
    NodeError,
    () => "Digest already called",
  );

  const ERR_CRYPTO_HASH_UPDATE_FAILED = defineNodeError(
    "ERR_CRYPTO_HASH_UPDATE_FAILED",
    NodeError,
    () => "Hash update failed",
  );

  const ERR_CRYPTO_INCOMPATIBLE_KEY = defineNodeError(
    "ERR_CRYPTO_INCOMPATIBLE_KEY",
    NodeError,
    (x: string, y: string) => `Incompatible ${x}: ${y}`,
  );

  const ERR_CRYPTO_INCOMPATIBLE_KEY_OPTIONS = defineNodeError(
    "ERR_CRYPTO_INCOMPATIBLE_KEY_OPTIONS",
    NodeError,
    (x: string, y: string) => `The selected key encoding ${x} ${y}.`,
  );

  const ERR_CRYPTO_INVALID_DIGEST = defineNodeError(
    "ERR_CRYPTO_INVALID_DIGEST",
    NodeTypeError,
    (x: string, prefix?: string) =>
      prefix ? `Invalid ${prefix} digest: ${x}` : `Invalid digest: ${x}`,
  );

  const ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE = defineNodeError(
    "ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE",
    NodeTypeError,
    (x: string, y: string) => `Invalid key object type ${x}, expected ${y}.`,
  );

  const ERR_CRYPTO_INVALID_KEYLEN = defineNodeError(
    "ERR_CRYPTO_INVALID_KEYLEN",
    NodeRangeError,
    () => "Invalid key length",
  );

  const ERR_CRYPTO_INVALID_JWK = defineNodeError(
    "ERR_CRYPTO_INVALID_JWK",
    NodeError,
    () => "Invalid JWK",
  );

  const ERR_CRYPTO_INVALID_STATE = defineNodeError(
    "ERR_CRYPTO_INVALID_STATE",
    NodeError,
    (x: string) => `Invalid state for operation ${x}`,
  );

  const ERR_CRYPTO_INVALID_SCRYPT_PARAMS = defineNodeError(
    "ERR_CRYPTO_INVALID_SCRYPT_PARAMS",
    NodeRangeError,
    (details?: string) =>
      details ? `Invalid scrypt params: ${details}` : "Invalid scrypt params",
  );

  const ERR_CRYPTO_PBKDF2_ERROR = defineNodeError(
    "ERR_CRYPTO_PBKDF2_ERROR",
    NodeError,
    () => "PBKDF2 error",
  );

  const ERR_CRYPTO_SCRYPT_INVALID_PARAMETER = defineNodeError(
    "ERR_CRYPTO_SCRYPT_INVALID_PARAMETER",
    NodeError,
    () => "Invalid scrypt parameter",
  );

  const ERR_CRYPTO_SCRYPT_NOT_SUPPORTED = defineNodeError(
    "ERR_CRYPTO_SCRYPT_NOT_SUPPORTED",
    NodeError,
    () => "Scrypt algorithm not supported",
  );

  const ERR_CRYPTO_SIGN_KEY_REQUIRED = defineNodeError(
    "ERR_CRYPTO_SIGN_KEY_REQUIRED",
    NodeError,
    () => "No key provided to sign",
  );

  const ERR_DIR_CLOSED = defineNodeError(
    "ERR_DIR_CLOSED",
    NodeError,
    () => "Directory handle was closed",
  );

  const ERR_DIR_CONCURRENT_OPERATION = defineNodeError(
    "ERR_DIR_CONCURRENT_OPERATION",
    NodeError,
    () =>
      "Cannot do synchronous work on directory handle with concurrent asynchronous operations",
  );

  const ERR_DNS_SET_SERVERS_FAILED = defineNodeError(
    "ERR_DNS_SET_SERVERS_FAILED",
    NodeError,
    (x: string, y: string) => `c-ares failed to set servers: "${x}" [${y}]`,
  );

  const ERR_DOMAIN_CALLBACK_NOT_AVAILABLE = defineNodeError(
    "ERR_DOMAIN_CALLBACK_NOT_AVAILABLE",
    NodeError,
    () =>
      "A callback was registered through " +
      "process.setUncaughtExceptionCaptureCallback(), which is mutually " +
      "exclusive with using the `domain` module",
  );

  const ERR_DOMAIN_CANNOT_SET_UNCAUGHT_EXCEPTION_CAPTURE = defineNodeError(
    "ERR_DOMAIN_CANNOT_SET_UNCAUGHT_EXCEPTION_CAPTURE",
    NodeError,
    () =>
      "The `domain` module is in use, which is mutually exclusive with calling " +
      "process.setUncaughtExceptionCaptureCallback()",
  );

  class ERR_ENCODING_INVALID_ENCODED_DATA extends NodeErrorAbstraction
    implements TypeError {
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

  const ERR_ENCODING_NOT_SUPPORTED = defineNodeError(
    "ERR_ENCODING_NOT_SUPPORTED",
    NodeRangeError,
    (x: string) => `The "${x}" encoding is not supported`,
  );
  const ERR_EVAL_ESM_CANNOT_PRINT = defineNodeError(
    "ERR_EVAL_ESM_CANNOT_PRINT",
    NodeError,
    () => `--print cannot be used with ESM input`,
  );
  const ERR_EVENT_RECURSION = defineNodeError(
    "ERR_EVENT_RECURSION",
    NodeError,
    (x: string) => `The event "${x}" is already being dispatched`,
  );
  const ERR_FEATURE_UNAVAILABLE_ON_PLATFORM = defineNodeError(
    "ERR_FEATURE_UNAVAILABLE_ON_PLATFORM",
    NodeTypeError,
    (x: string) =>
      `The feature ${x} is unavailable on the current platform, which is being used to run Node.js`,
  );
  const ERR_FS_FILE_TOO_LARGE = defineNodeError(
    "ERR_FS_FILE_TOO_LARGE",
    NodeRangeError,
    (x: string | number) => `File size (${x}) is greater than 2 GB`,
  );
  const ERR_FS_INVALID_SYMLINK_TYPE = defineNodeError(
    "ERR_FS_INVALID_SYMLINK_TYPE",
    NodeError,
    (x: string) =>
      `Symlink type must be one of "dir", "file", or "junction". Received "${x}"`,
  );
  const ERR_HTTP2_ALTSVC_INVALID_ORIGIN = defineNodeError(
    "ERR_HTTP2_ALTSVC_INVALID_ORIGIN",
    NodeTypeError,
    () => `HTTP/2 ALTSVC frames require a valid origin`,
  );
  const ERR_HTTP2_ALTSVC_LENGTH = defineNodeError(
    "ERR_HTTP2_ALTSVC_LENGTH",
    NodeTypeError,
    () => `HTTP/2 ALTSVC frames are limited to 16382 bytes`,
  );
  const ERR_HTTP2_CONNECT_AUTHORITY = defineNodeError(
    "ERR_HTTP2_CONNECT_AUTHORITY",
    NodeError,
    () => `:authority header is required for CONNECT requests`,
  );
  const ERR_HTTP2_CONNECT_PATH = defineNodeError(
    "ERR_HTTP2_CONNECT_PATH",
    NodeError,
    () => `The :path header is forbidden for CONNECT requests`,
  );
  const ERR_HTTP2_CONNECT_SCHEME = defineNodeError(
    "ERR_HTTP2_CONNECT_SCHEME",
    NodeError,
    () => `The :scheme header is forbidden for CONNECT requests`,
  );
  const ERR_HTTP2_GOAWAY_SESSION = defineNodeError(
    "ERR_HTTP2_GOAWAY_SESSION",
    NodeError,
    () => `New streams cannot be created after receiving a GOAWAY`,
  );
  const ERR_HTTP2_HEADERS_AFTER_RESPOND = defineNodeError(
    "ERR_HTTP2_HEADERS_AFTER_RESPOND",
    NodeError,
    () => `Cannot specify additional headers after response initiated`,
  );
  const ERR_HTTP2_HEADERS_SENT = defineNodeError(
    "ERR_HTTP2_HEADERS_SENT",
    NodeError,
    () => `Response has already been initiated.`,
  );
  const ERR_HTTP2_HEADER_SINGLE_VALUE = defineNodeError(
    "ERR_HTTP2_HEADER_SINGLE_VALUE",
    NodeTypeError,
    (x: string) => `Header field "${x}" must only have a single value`,
  );
  const ERR_HTTP2_INFO_STATUS_NOT_ALLOWED = defineNodeError(
    "ERR_HTTP2_INFO_STATUS_NOT_ALLOWED",
    NodeRangeError,
    () => `Informational status codes cannot be used`,
  );
  const ERR_HTTP2_INVALID_CONNECTION_HEADERS = defineNodeError(
    "ERR_HTTP2_INVALID_CONNECTION_HEADERS",
    NodeTypeError,
    (x: string) => `HTTP/1 Connection specific headers are forbidden: "${x}"`,
  );
  class ERR_HTTP2_INVALID_HEADER_VALUE extends NodeTypeError {
    static HideStackFramesError = this;
    constructor(x: string, y: string) {
      super(
        "ERR_HTTP2_INVALID_HEADER_VALUE",
        `Invalid value "${x}" for header "${y}"`,
      );
    }
  }
  const ERR_HTTP2_INVALID_INFO_STATUS = defineNodeError(
    "ERR_HTTP2_INVALID_INFO_STATUS",
    NodeRangeError,
    (x: string) => `Invalid informational status code: ${x}`,
  );
  const ERR_HTTP2_INVALID_ORIGIN = defineNodeError(
    "ERR_HTTP2_INVALID_ORIGIN",
    NodeTypeError,
    () => `HTTP/2 ORIGIN frames require a valid origin`,
  );
  const ERR_HTTP2_INVALID_PACKED_SETTINGS_LENGTH = defineNodeError(
    "ERR_HTTP2_INVALID_PACKED_SETTINGS_LENGTH",
    NodeRangeError,
    () => `Packed settings length must be a multiple of six`,
  );
  const ERR_HTTP2_INVALID_PSEUDOHEADER = defineNodeError(
    "ERR_HTTP2_INVALID_PSEUDOHEADER",
    NodeTypeError,
    (x: string) => `"${x}" is an invalid pseudoheader or is used incorrectly`,
  );
  const ERR_HTTP2_INVALID_SESSION = defineNodeError(
    "ERR_HTTP2_INVALID_SESSION",
    NodeError,
    () => `The session has been destroyed`,
  );
  const ERR_HTTP2_INVALID_STREAM = defineNodeError(
    "ERR_HTTP2_INVALID_STREAM",
    NodeError,
    () => `The stream has been destroyed`,
  );
  const ERR_HTTP2_MAX_PENDING_SETTINGS_ACK = defineNodeError(
    "ERR_HTTP2_MAX_PENDING_SETTINGS_ACK",
    NodeError,
    () => `Maximum number of pending settings acknowledgements`,
  );
  const ERR_HTTP2_NESTED_PUSH = defineNodeError(
    "ERR_HTTP2_NESTED_PUSH",
    NodeError,
    () => `A push stream cannot initiate another push stream.`,
  );
  const ERR_HTTP2_NO_SOCKET_MANIPULATION = defineNodeError(
    "ERR_HTTP2_NO_SOCKET_MANIPULATION",
    NodeError,
    () =>
      `HTTP/2 sockets should not be directly manipulated (e.g. read and written)`,
  );
  const ERR_HTTP2_ORIGIN_LENGTH = defineNodeError(
    "ERR_HTTP2_ORIGIN_LENGTH",
    NodeTypeError,
    () => `HTTP/2 ORIGIN frames are limited to 16382 bytes`,
  );
  const ERR_HTTP2_OUT_OF_STREAMS = defineNodeError(
    "ERR_HTTP2_OUT_OF_STREAMS",
    NodeError,
    () =>
      `No stream ID is available because maximum stream ID has been reached`,
  );
  const ERR_HTTP2_PAYLOAD_FORBIDDEN = defineNodeError(
    "ERR_HTTP2_PAYLOAD_FORBIDDEN",
    NodeError,
    (x: string) => `Responses with ${x} status must not have a payload`,
  );
  const ERR_HTTP2_PING_CANCEL = defineNodeError(
    "ERR_HTTP2_PING_CANCEL",
    NodeError,
    () => `HTTP2 ping cancelled`,
  );
  const ERR_HTTP2_PING_LENGTH = defineNodeError(
    "ERR_HTTP2_PING_LENGTH",
    NodeRangeError,
    () => `HTTP2 ping payload must be 8 bytes`,
  );
  class ERR_HTTP2_PSEUDOHEADER_NOT_ALLOWED extends NodeTypeError {
    static HideStackFramesError = this;
    constructor() {
      super(
        "ERR_HTTP2_PSEUDOHEADER_NOT_ALLOWED",
        `Cannot set HTTP/2 pseudo-headers`,
      );
    }
  }
  const ERR_HTTP2_PUSH_DISABLED = defineNodeError(
    "ERR_HTTP2_PUSH_DISABLED",
    NodeError,
    () => `HTTP/2 client has disabled push streams`,
  );
  const ERR_HTTP2_SEND_FILE = defineNodeError(
    "ERR_HTTP2_SEND_FILE",
    NodeError,
    () => `Directories cannot be sent`,
  );
  const ERR_HTTP2_SEND_FILE_NOSEEK = defineNodeError(
    "ERR_HTTP2_SEND_FILE_NOSEEK",
    NodeError,
    () => `Offset or length can only be specified for regular files`,
  );
  const ERR_HTTP2_SESSION_ERROR = defineNodeError(
    "ERR_HTTP2_SESSION_ERROR",
    NodeError,
    (x: string) => `Session closed with error code ${x}`,
  );
  const ERR_HTTP2_SETTINGS_CANCEL = defineNodeError(
    "ERR_HTTP2_SETTINGS_CANCEL",
    NodeError,
    () => `HTTP2 session settings canceled`,
  );
  const ERR_HTTP2_SOCKET_BOUND = defineNodeError(
    "ERR_HTTP2_SOCKET_BOUND",
    NodeError,
    () => `The socket is already bound to an Http2Session`,
  );
  const ERR_HTTP2_SOCKET_UNBOUND = defineNodeError(
    "ERR_HTTP2_SOCKET_UNBOUND",
    NodeError,
    () => `The socket has been disconnected from the Http2Session`,
  );
  const ERR_HTTP2_STATUS_101 = defineNodeError(
    "ERR_HTTP2_STATUS_101",
    NodeError,
    () => `HTTP status code 101 (Switching Protocols) is forbidden in HTTP/2`,
  );
  const ERR_HTTP2_STATUS_INVALID = defineNodeError(
    "ERR_HTTP2_STATUS_INVALID",
    NodeRangeError,
    (x: string) => `Invalid status code: ${x}`,
  );
  const ERR_HTTP2_STREAM_ERROR = defineNodeError(
    "ERR_HTTP2_STREAM_ERROR",
    NodeError,
    (x: string) => `Stream closed with error code ${x}`,
  );
  const ERR_HTTP2_STREAM_SELF_DEPENDENCY = defineNodeError(
    "ERR_HTTP2_STREAM_SELF_DEPENDENCY",
    NodeError,
    () => `A stream cannot depend on itself`,
  );
  const ERR_HTTP2_TRAILERS_ALREADY_SENT = defineNodeError(
    "ERR_HTTP2_TRAILERS_ALREADY_SENT",
    NodeError,
    () => `Trailing headers have already been sent`,
  );
  const ERR_HTTP2_TRAILERS_NOT_READY = defineNodeError(
    "ERR_HTTP2_TRAILERS_NOT_READY",
    NodeError,
    () =>
      `Trailing headers cannot be sent until after the wantTrailers event is emitted`,
  );
  const ERR_HTTP2_UNSUPPORTED_PROTOCOL = defineNodeError(
    "ERR_HTTP2_UNSUPPORTED_PROTOCOL",
    NodeError,
    (x: string) => `protocol "${x}" is unsupported.`,
  );
  const ERR_HTTP_BODY_NOT_ALLOWED = defineNodeError(
    "ERR_HTTP_BODY_NOT_ALLOWED",
    NodeError,
    () =>
      "Adding content for this request method or response status is not allowed.",
  );
  const ERR_HTTP_CONTENT_LENGTH_MISMATCH = defineNodeError(
    "ERR_HTTP_CONTENT_LENGTH_MISMATCH",
    NodeError,
    (bodyLength: number, contentLength: number) =>
      `Response body's content-length of ${bodyLength} byte(s) does not match the content-length of ${contentLength} byte(s) set in header`,
  );
  const ERR_HTTP_HEADERS_SENT = defineNodeError(
    "ERR_HTTP_HEADERS_SENT",
    NodeError,
    (x: string) => `Cannot ${x} headers after they are sent to the client`,
  );
  const ERR_HTTP_INVALID_HEADER_VALUE = defineNodeError(
    "ERR_HTTP_INVALID_HEADER_VALUE",
    NodeTypeError,
    (x: string, y: string) => `Invalid value "${x}" for header "${y}"`,
  );
  const ERR_HTTP_INVALID_STATUS_CODE = defineNodeError(
    "ERR_HTTP_INVALID_STATUS_CODE",
    NodeRangeError,
    (x: string) => `Invalid status code: ${x}`,
  );
  const ERR_HTTP_SOCKET_ENCODING = defineNodeError(
    "ERR_HTTP_SOCKET_ENCODING",
    NodeError,
    () => `Changing the socket encoding is not allowed per RFC7230 Section 3.`,
  );
  const ERR_HTTP_TRAILER_INVALID = defineNodeError(
    "ERR_HTTP_TRAILER_INVALID",
    NodeError,
    () => `Trailers are invalid with this transfer encoding`,
  );
  const ERR_ILLEGAL_CONSTRUCTOR = defineNodeError(
    "ERR_ILLEGAL_CONSTRUCTOR",
    NodeTypeError,
    () => "Illegal constructor",
  );
  const ERR_INCOMPATIBLE_OPTION_PAIR = defineNodeError(
    "ERR_INCOMPATIBLE_OPTION_PAIR",
    NodeTypeError,
    (x: string, y: string) =>
      `Option "${x}" cannot be used in combination with option "${y}"`,
  );
  const ERR_INPUT_TYPE_NOT_ALLOWED = defineNodeError(
    "ERR_INPUT_TYPE_NOT_ALLOWED",
    NodeError,
    () =>
      `--input-type can only be used with string input via --eval, --print, or STDIN`,
  );
  const ERR_INSPECTOR_ALREADY_ACTIVATED = defineNodeError(
    "ERR_INSPECTOR_ALREADY_ACTIVATED",
    NodeError,
    () =>
      `Inspector is already activated. Close it with inspector.close() before activating it again.`,
  );
  const ERR_INSPECTOR_ALREADY_CONNECTED = defineNodeError(
    "ERR_INSPECTOR_ALREADY_CONNECTED",
    NodeError,
    (x: string) => `${x} is already connected`,
  );
  const ERR_INSPECTOR_CLOSED = defineNodeError(
    "ERR_INSPECTOR_CLOSED",
    NodeError,
    () => `Session was closed`,
  );
  const ERR_INSPECTOR_COMMAND = defineNodeError(
    "ERR_INSPECTOR_COMMAND",
    NodeError,
    (x: number, y: string) => `Inspector error ${x}: ${y}`,
  );
  const ERR_INSPECTOR_NOT_ACTIVE = defineNodeError(
    "ERR_INSPECTOR_NOT_ACTIVE",
    NodeError,
    () => `Inspector is not active`,
  );
  const ERR_INSPECTOR_NOT_AVAILABLE = defineNodeError(
    "ERR_INSPECTOR_NOT_AVAILABLE",
    NodeError,
    () => `Inspector is not available`,
  );
  const ERR_INSPECTOR_NOT_CONNECTED = defineNodeError(
    "ERR_INSPECTOR_NOT_CONNECTED",
    NodeError,
    () => `Session is not connected`,
  );
  const ERR_INSPECTOR_NOT_WORKER = defineNodeError(
    "ERR_INSPECTOR_NOT_WORKER",
    NodeError,
    () => `Current thread is not a worker`,
  );
  const ERR_INVALID_ASYNC_ID = defineNodeError(
    "ERR_INVALID_ASYNC_ID",
    NodeRangeError,
    (x: string, y: string | number) => `Invalid ${x} value: ${y}`,
  );
  const ERR_INVALID_BUFFER_SIZE = defineNodeError(
    "ERR_INVALID_BUFFER_SIZE",
    NodeRangeError,
    (x: string) => `Buffer size must be a multiple of ${x}`,
  );
  const ERR_INVALID_CURSOR_POS = defineNodeError(
    "ERR_INVALID_CURSOR_POS",
    NodeTypeError,
    () => `Cannot set cursor row without setting its column`,
  );
  const ERR_INVALID_FD = defineNodeError(
    "ERR_INVALID_FD",
    NodeRangeError,
    (x: string) => `"fd" must be a positive integer: ${x}`,
  );
  const ERR_INVALID_FD_TYPE = defineNodeError(
    "ERR_INVALID_FD_TYPE",
    NodeTypeError,
    (x: string) => `Unsupported fd type: ${x}`,
  );
  const ERR_INVALID_FILE_URL_HOST = defineNodeError(
    "ERR_INVALID_FILE_URL_HOST",
    NodeTypeError,
    (x: string) => `File URL host must be "localhost" or empty on ${x}`,
  );
  class ERR_INVALID_FILE_URL_PATH extends NodeTypeError {
    input?: URL;
    constructor(x: string, input?: URL) {
      super("ERR_INVALID_FILE_URL_PATH", `File URL path ${x}`);
      this.input = input;
    }
  }
  const ERR_INVALID_HANDLE_TYPE = defineNodeError(
    "ERR_INVALID_HANDLE_TYPE",
    NodeTypeError,
    () => `This handle type cannot be sent`,
  );
  class ERR_INVALID_HTTP_TOKEN extends NodeTypeError {
    static HideStackFramesError = this;
    constructor(x: string, y: string) {
      super(
        "ERR_INVALID_HTTP_TOKEN",
        `${x} must be a valid HTTP token ["${y}"]`,
      );
    }
  }
  const ERR_INVALID_IP_ADDRESS = defineNodeError(
    "ERR_INVALID_IP_ADDRESS",
    NodeTypeError,
    (x: string) => `Invalid IP address: ${x}`,
  );
  const ERR_IP_BLOCKED = defineNodeError(
    "ERR_IP_BLOCKED",
    NodeError,
    (x: string) => `Address blocked: ${x}`,
  );
  class ERR_INVALID_MIME_SYNTAX extends NodeTypeError {
    constructor(production: string, str: string, invalidIndex: number) {
      const msg = invalidIndex !== -1 ? ` at ${invalidIndex}` : "";
      super(
        "ERR_INVALID_MIME_SYNTAX",
        `The MIME syntax for a ${production} in "${str}" is invalid` + msg,
      );
    }
  }
  const ERR_INVALID_OBJECT_DEFINE_PROPERTY = defineNodeError(
    "ERR_INVALID_OBJECT_DEFINE_PROPERTY",
    NodeTypeError,
    (message: string) => message,
  );
  const ERR_INVALID_OPT_VALUE_ENCODING = defineNodeError(
    "ERR_INVALID_OPT_VALUE_ENCODING",
    NodeTypeError,
    (x: string) => `The value "${x}" is invalid for option "encoding"`,
  );
  const ERR_INVALID_PERFORMANCE_MARK = defineNodeError(
    "ERR_INVALID_PERFORMANCE_MARK",
    NodeError,
    (x: string) => `The "${x}" performance mark has not been set`,
  );
  const ERR_INVALID_PROTOCOL = defineNodeError(
    "ERR_INVALID_PROTOCOL",
    NodeTypeError,
    (x: string, y: string) => `Protocol "${x}" not supported. Expected "${y}"`,
  );
  const ERR_PROXY_INVALID_CONFIG = defineNodeError(
    "ERR_PROXY_INVALID_CONFIG",
    NodeError,
    (reason: string) => reason,
  );
  const ERR_PROXY_TUNNEL = defineNodeError(
    "ERR_PROXY_TUNNEL",
    NodeError,
    (reason: string) => reason,
  );
  const ERR_INVALID_REPL_EVAL_CONFIG = defineNodeError(
    "ERR_INVALID_REPL_EVAL_CONFIG",
    NodeTypeError,
    () => `Cannot specify both "breakEvalOnSigint" and "eval" for REPL`,
  );
  const ERR_INVALID_REPL_INPUT = defineNodeError(
    "ERR_INVALID_REPL_INPUT",
    NodeTypeError,
    (x: string) => `${x}`,
  );
  const ERR_INVALID_SYNC_FORK_INPUT = defineNodeError(
    "ERR_INVALID_SYNC_FORK_INPUT",
    NodeTypeError,
    (x: string) =>
      `Asynchronous forks do not support Buffer, TypedArray, DataView or string input: ${x}`,
  );
  const ERR_INVALID_THIS = defineNodeError(
    "ERR_INVALID_THIS",
    NodeTypeError,
    (x: string) => `Value of "this" must be of type ${x}`,
  );
  const ERR_INVALID_TUPLE = defineNodeError(
    "ERR_INVALID_TUPLE",
    NodeTypeError,
    (x: string, y: string) => `${x} must be an iterable ${y} tuple`,
  );
  const ERR_INVALID_URI = defineNodeError(
    "ERR_INVALID_URI",
    NodeURIError,
    () => `URI malformed`,
  );
  const ERR_IPC_CHANNEL_CLOSED = defineNodeError(
    "ERR_IPC_CHANNEL_CLOSED",
    NodeError,
    () => `Channel closed`,
  );
  const ERR_IPC_DISCONNECTED = defineNodeError(
    "ERR_IPC_DISCONNECTED",
    NodeError,
    () => `IPC channel is already disconnected`,
  );
  const ERR_IPC_ONE_PIPE = defineNodeError(
    "ERR_IPC_ONE_PIPE",
    NodeError,
    () => `Child process can have only one IPC pipe`,
  );
  const ERR_IPC_SYNC_FORK = defineNodeError(
    "ERR_IPC_SYNC_FORK",
    NodeError,
    () => `IPC cannot be used with synchronous forks`,
  );
  const ERR_MANIFEST_DEPENDENCY_MISSING = defineNodeError(
    "ERR_MANIFEST_DEPENDENCY_MISSING",
    NodeError,
    (x: string, y: string) =>
      `Manifest resource ${x} does not list ${y} as a dependency specifier`,
  );
  const ERR_MANIFEST_INTEGRITY_MISMATCH = defineNodeError(
    "ERR_MANIFEST_INTEGRITY_MISMATCH",
    NodeSyntaxError,
    (x: string) =>
      `Manifest resource ${x} has multiple entries but integrity lists do not match`,
  );
  const ERR_MANIFEST_INVALID_RESOURCE_FIELD = defineNodeError(
    "ERR_MANIFEST_INVALID_RESOURCE_FIELD",
    NodeTypeError,
    (x: string, y: string) =>
      `Manifest resource ${x} has invalid property value for ${y}`,
  );
  const ERR_MANIFEST_TDZ = defineNodeError(
    "ERR_MANIFEST_TDZ",
    NodeError,
    () => `Manifest initialization has not yet run`,
  );
  const ERR_MANIFEST_UNKNOWN_ONERROR = defineNodeError(
    "ERR_MANIFEST_UNKNOWN_ONERROR",
    NodeSyntaxError,
    (x: string) => `Manifest specified unknown error behavior "${x}".`,
  );
  const ERR_METHOD_NOT_IMPLEMENTED = defineNodeError(
    "ERR_METHOD_NOT_IMPLEMENTED",
    NodeError,
    (x: string) => `The ${x} method is not implemented`,
  );
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
          msg += ArrayPrototypeJoin(
            ArrayPrototypeSlice(args, 0, len - 1),
            ", ",
          );
          msg += `, and ${args[len - 1]} arguments`;
          break;
      }

      super("ERR_MISSING_ARGS", `${msg} must be specified`);
    }
  }
  const ERR_MISSING_OPTION = defineNodeError(
    "ERR_MISSING_OPTION",
    NodeTypeError,
    (x: string) => `${x} is required`,
  );
  const ERR_MULTIPLE_CALLBACK = defineNodeError(
    "ERR_MULTIPLE_CALLBACK",
    NodeError,
    () => `Callback called multiple times`,
  );
  const ERR_NAPI_CONS_FUNCTION = defineNodeError(
    "ERR_NAPI_CONS_FUNCTION",
    NodeTypeError,
    () => `Constructor must be a function`,
  );
  const ERR_NAPI_INVALID_DATAVIEW_ARGS = defineNodeError(
    "ERR_NAPI_INVALID_DATAVIEW_ARGS",
    NodeRangeError,
    () =>
      `byte_offset + byte_length should be less than or equal to the size in bytes of the array passed in`,
  );
  const ERR_NAPI_INVALID_TYPEDARRAY_ALIGNMENT = defineNodeError(
    "ERR_NAPI_INVALID_TYPEDARRAY_ALIGNMENT",
    NodeRangeError,
    (x: string, y: string) =>
      `start offset of ${x} should be a multiple of ${y}`,
  );
  const ERR_NAPI_INVALID_TYPEDARRAY_LENGTH = defineNodeError(
    "ERR_NAPI_INVALID_TYPEDARRAY_LENGTH",
    NodeRangeError,
    () => `Invalid typed array length`,
  );
  const ERR_NO_CRYPTO = defineNodeError(
    "ERR_NO_CRYPTO",
    NodeError,
    () => `Node.js is not compiled with OpenSSL crypto support`,
  );
  const ERR_NO_ICU = defineNodeError(
    "ERR_NO_ICU",
    NodeTypeError,
    (x: string) => `${x} is not supported on Node.js compiled without ICU`,
  );
  const ERR_QUICCLIENTSESSION_FAILED = defineNodeError(
    "ERR_QUICCLIENTSESSION_FAILED",
    NodeError,
    (x: string) => `Failed to create a new QuicClientSession: ${x}`,
  );
  const ERR_QUICCLIENTSESSION_FAILED_SETSOCKET = defineNodeError(
    "ERR_QUICCLIENTSESSION_FAILED_SETSOCKET",
    NodeError,
    () => `Failed to set the QuicSocket`,
  );
  const ERR_QUICSESSION_DESTROYED = defineNodeError(
    "ERR_QUICSESSION_DESTROYED",
    NodeError,
    (x: string) => `Cannot call ${x} after a QuicSession has been destroyed`,
  );
  const ERR_QUICSESSION_INVALID_DCID = defineNodeError(
    "ERR_QUICSESSION_INVALID_DCID",
    NodeError,
    (x: string) => `Invalid DCID value: ${x}`,
  );
  const ERR_QUICSESSION_UPDATEKEY = defineNodeError(
    "ERR_QUICSESSION_UPDATEKEY",
    NodeError,
    () => `Unable to update QuicSession keys`,
  );
  const ERR_QUICSOCKET_DESTROYED = defineNodeError(
    "ERR_QUICSOCKET_DESTROYED",
    NodeError,
    (x: string) => `Cannot call ${x} after a QuicSocket has been destroyed`,
  );
  const ERR_QUICSOCKET_INVALID_STATELESS_RESET_SECRET_LENGTH = defineNodeError(
    "ERR_QUICSOCKET_INVALID_STATELESS_RESET_SECRET_LENGTH",
    NodeError,
    () => `The stateResetToken must be exactly 16-bytes in length`,
  );
  const ERR_QUICSOCKET_LISTENING = defineNodeError(
    "ERR_QUICSOCKET_LISTENING",
    NodeError,
    () => `This QuicSocket is already listening`,
  );
  const ERR_QUICSOCKET_UNBOUND = defineNodeError(
    "ERR_QUICSOCKET_UNBOUND",
    NodeError,
    (x: string) => `Cannot call ${x} before a QuicSocket has been bound`,
  );
  const ERR_QUICSTREAM_DESTROYED = defineNodeError(
    "ERR_QUICSTREAM_DESTROYED",
    NodeError,
    (x: string) => `Cannot call ${x} after a QuicStream has been destroyed`,
  );
  const ERR_QUICSTREAM_INVALID_PUSH = defineNodeError(
    "ERR_QUICSTREAM_INVALID_PUSH",
    NodeError,
    () =>
      `Push streams are only supported on client-initiated, bidirectional streams`,
  );
  const ERR_QUICSTREAM_OPEN_FAILED = defineNodeError(
    "ERR_QUICSTREAM_OPEN_FAILED",
    NodeError,
    () => `Opening a new QuicStream failed`,
  );
  const ERR_QUICSTREAM_UNSUPPORTED_PUSH = defineNodeError(
    "ERR_QUICSTREAM_UNSUPPORTED_PUSH",
    NodeError,
    () => `Push streams are not supported on this QuicSession`,
  );
  const ERR_QUIC_TLS13_REQUIRED = defineNodeError(
    "ERR_QUIC_TLS13_REQUIRED",
    NodeError,
    () => `QUIC requires TLS version 1.3`,
  );
  class ERR_REQUIRE_ASYNC_MODULE extends NodeError {
    constructor(filename: string, parentFilename: string) {
      super(
        "ERR_REQUIRE_ASYNC_MODULE",
        `require() cannot be used on an ESM graph with top-level await. Use import() instead. To see where the top-level await comes from, use --stack-trace-limit=100 and inspect the dependency graph. Requiring ${filename}. From ${parentFilename}`,
      );
      this.name = `Error [${this.code}]`;
      this.toString = nodeErrorToStringWithEmbeddedCode;
    }
  }
  class ERR_REQUIRE_CYCLE_MODULE extends NodeError {
    constructor(filename: string, parentFilename: string) {
      super(
        "ERR_REQUIRE_CYCLE_MODULE",
        `Cannot require() ES Module ${filename} in a cycle. (from ${parentFilename})`,
      );
      this.name = `Error [${this.code}]`;
      this.toString = nodeErrorToStringWithEmbeddedCode;
    }
  }
  function nodeErrorToStringWithEmbeddedCode(this: NodeErrorAbstraction) {
    return `${this.name}: ${this.message}`;
  }
  const ERR_SCRIPT_EXECUTION_INTERRUPTED = defineNodeError(
    "ERR_SCRIPT_EXECUTION_INTERRUPTED",
    NodeError,
    () => "Script execution was interrupted by `SIGINT`",
  );
  const ERR_SERVER_ALREADY_LISTEN = defineNodeError(
    "ERR_SERVER_ALREADY_LISTEN",
    NodeError,
    () => `Listen method has been called more than once without closing.`,
  );
  const ERR_SERVER_NOT_RUNNING = defineNodeError(
    "ERR_SERVER_NOT_RUNNING",
    NodeError,
    () => `Server is not running.`,
  );
  const ERR_SOCKET_ALREADY_BOUND = defineNodeError(
    "ERR_SOCKET_ALREADY_BOUND",
    NodeError,
    () => `Socket is already bound`,
  );
  const ERR_SOCKET_BAD_BUFFER_SIZE = defineNodeError(
    "ERR_SOCKET_BAD_BUFFER_SIZE",
    NodeTypeError,
    () => `Buffer size must be a positive integer`,
  );
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
  const ERR_SOCKET_BAD_TYPE = defineNodeError(
    "ERR_SOCKET_BAD_TYPE",
    NodeTypeError,
    () => `Bad socket type specified. Valid types are: udp4, udp6`,
  );
  class ERR_SOCKET_BUFFER_SIZE extends NodeSystemError {
    constructor(ctx: NodeSystemErrorCtx) {
      super("ERR_SOCKET_BUFFER_SIZE", ctx, "Could not get or set buffer size");
    }
  }
  const ERR_SOCKET_CLOSED = defineNodeError(
    "ERR_SOCKET_CLOSED",
    NodeError,
    () => `Socket is closed`,
  );
  const ERR_SOCKET_CLOSED_BEFORE_CONNECTION = defineNodeError(
    "ERR_SOCKET_CLOSED_BEFORE_CONNECTION",
    NodeError,
    () => `Socket closed before the connection was established`,
  );
  const ERR_SOCKET_CONNECTION_TIMEOUT = defineNodeError(
    "ERR_SOCKET_CONNECTION_TIMEOUT",
    NodeError,
    () => `Socket connection timeout`,
  );
  const ERR_SOCKET_DGRAM_IS_CONNECTED = defineNodeError(
    "ERR_SOCKET_DGRAM_IS_CONNECTED",
    NodeError,
    () => `Already connected`,
  );
  const ERR_SOCKET_DGRAM_NOT_CONNECTED = defineNodeError(
    "ERR_SOCKET_DGRAM_NOT_CONNECTED",
    NodeError,
    () => `Not connected`,
  );
  const ERR_SOCKET_DGRAM_NOT_RUNNING = defineNodeError(
    "ERR_SOCKET_DGRAM_NOT_RUNNING",
    NodeError,
    () => `Not running`,
  );
  const ERR_SRI_PARSE = defineNodeError(
    "ERR_SRI_PARSE",
    NodeSyntaxError,
    (name: string, char: string, position: number) =>
      `Subresource Integrity string ${name} had an unexpected ${char} at position ${position}`,
  );
  const ERR_STREAM_ALREADY_FINISHED = defineNodeError(
    "ERR_STREAM_ALREADY_FINISHED",
    NodeError,
    (x: string) => `Cannot call ${x} after a stream was finished`,
  );
  const ERR_STREAM_CANNOT_PIPE = defineNodeError(
    "ERR_STREAM_CANNOT_PIPE",
    NodeError,
    () => `Cannot pipe, not readable`,
  );
  const ERR_STREAM_DESTROYED = defineNodeError(
    "ERR_STREAM_DESTROYED",
    NodeError,
    (x: string) => `Cannot call ${x} after a stream was destroyed`,
  );
  const ERR_STREAM_NULL_VALUES = defineNodeError(
    "ERR_STREAM_NULL_VALUES",
    NodeTypeError,
    () => `May not write null values to stream`,
  );
  const ERR_STREAM_PREMATURE_CLOSE = defineNodeError(
    "ERR_STREAM_PREMATURE_CLOSE",
    NodeError,
    () => `Premature close`,
  );
  const ERR_STREAM_PUSH_AFTER_EOF = defineNodeError(
    "ERR_STREAM_PUSH_AFTER_EOF",
    NodeError,
    () => `stream.push() after EOF`,
  );
  const ERR_STREAM_UNSHIFT_AFTER_END_EVENT = defineNodeError(
    "ERR_STREAM_UNSHIFT_AFTER_END_EVENT",
    NodeError,
    () => `stream.unshift() after end event`,
  );
  const ERR_STREAM_WRAP = defineNodeError(
    "ERR_STREAM_WRAP",
    NodeError,
    () => `Stream has StringDecoder set or is in objectMode`,
  );
  const ERR_STREAM_WRITE_AFTER_END = defineNodeError(
    "ERR_STREAM_WRITE_AFTER_END",
    NodeError,
    () => `write after end`,
  );
  const ERR_SYNTHETIC = defineNodeError(
    "ERR_SYNTHETIC",
    NodeError,
    () => `JavaScript Callstack`,
  );
  class ERR_TLS_CERT_ALTNAME_INVALID extends NodeError {
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
  const ERR_TLS_ALPN_CALLBACK_WITH_PROTOCOLS = defineNodeError(
    "ERR_TLS_ALPN_CALLBACK_WITH_PROTOCOLS",
    NodeTypeError,
    () =>
      "The ALPNCallback and ALPNProtocols TLS options are mutually exclusive",
  );
  const ERR_TLS_DH_PARAM_SIZE = defineNodeError(
    "ERR_TLS_DH_PARAM_SIZE",
    NodeError,
    (x: string) => `DH parameter size ${x} is less than 2048`,
  );
  const ERR_TLS_HANDSHAKE_TIMEOUT = defineNodeError(
    "ERR_TLS_HANDSHAKE_TIMEOUT",
    NodeError,
    () => `TLS handshake timeout`,
  );
  const ERR_TLS_INVALID_CONTEXT = defineNodeError(
    "ERR_TLS_INVALID_CONTEXT",
    NodeTypeError,
    (x: string) => `${x} must be a SecureContext`,
  );
  const ERR_TLS_INVALID_STATE = defineNodeError(
    "ERR_TLS_INVALID_STATE",
    NodeError,
    () => `TLS socket connection must be securely established`,
  );
  const ERR_TLS_INVALID_PROTOCOL_VERSION = defineNodeError(
    "ERR_TLS_INVALID_PROTOCOL_VERSION",
    NodeTypeError,
    (protocol: string, x: string) =>
      `${protocol} is not a valid ${x} TLS protocol version`,
  );
  const ERR_TLS_INVALID_PROTOCOL_METHOD = defineNodeError(
    "ERR_TLS_INVALID_PROTOCOL_METHOD",
    NodeTypeError,
    (message: string) => message,
  );
  const ERR_TLS_PROTOCOL_VERSION_CONFLICT = defineNodeError(
    "ERR_TLS_PROTOCOL_VERSION_CONFLICT",
    NodeTypeError,
    (prevProtocol: string, protocol: string) =>
      `TLS protocol version ${prevProtocol} conflicts with secureProtocol ${protocol}`,
  );
  const ERR_TLS_RENEGOTIATION_DISABLED = defineNodeError(
    "ERR_TLS_RENEGOTIATION_DISABLED",
    NodeError,
    () => `TLS session renegotiation disabled for this socket`,
  );
  const ERR_TLS_ALPN_CALLBACK_INVALID_RESULT = defineNodeError(
    "ERR_TLS_ALPN_CALLBACK_INVALID_RESULT",
    NodeError,
    () =>
      `ALPN callback returned a value not present in the client's ALPN protocols`,
  );
  const ERR_TLS_REQUIRED_SERVER_NAME = defineNodeError(
    "ERR_TLS_REQUIRED_SERVER_NAME",
    NodeError,
    () => `"servername" is required parameter for Server.addContext`,
  );
  const ERR_TLS_SESSION_ATTACK = defineNodeError(
    "ERR_TLS_SESSION_ATTACK",
    NodeError,
    () => `TLS session renegotiation attack detected`,
  );
  const ERR_TLS_SNI_FROM_SERVER = defineNodeError(
    "ERR_TLS_SNI_FROM_SERVER",
    NodeError,
    () => `Cannot issue SNI from a TLS server-side socket`,
  );
  const ERR_TRACE_EVENTS_CATEGORY_REQUIRED = defineNodeError(
    "ERR_TRACE_EVENTS_CATEGORY_REQUIRED",
    NodeTypeError,
    () => `At least one category is required`,
  );
  const ERR_TRACE_EVENTS_UNAVAILABLE = defineNodeError(
    "ERR_TRACE_EVENTS_UNAVAILABLE",
    NodeError,
    () => `Trace events are unavailable`,
  );
  const ERR_UNAVAILABLE_DURING_EXIT = defineNodeError(
    "ERR_UNAVAILABLE_DURING_EXIT",
    NodeError,
    () => `Cannot call function in process exit handler`,
  );
  const ERR_UNCAUGHT_EXCEPTION_CAPTURE_ALREADY_SET = defineNodeError(
    "ERR_UNCAUGHT_EXCEPTION_CAPTURE_ALREADY_SET",
    NodeError,
    () =>
      "`process.setupUncaughtExceptionCapture()` was called while a capture callback was already active",
  );
  const ERR_UNESCAPED_CHARACTERS = defineNodeError(
    "ERR_UNESCAPED_CHARACTERS",
    NodeTypeError,
    (x: string) => `${x} contains unescaped characters`,
  );
  const ERR_UNHANDLED_ERROR = defineNodeError(
    "ERR_UNHANDLED_ERROR",
    NodeError,
    (x: string) => `Unhandled error. (${x})`,
  );
  const ERR_UNKNOWN_BUILTIN_MODULE = defineNodeError(
    "ERR_UNKNOWN_BUILTIN_MODULE",
    NodeError,
    (x: string) => `No such built-in module: ${x}`,
  );
  const ERR_UNKNOWN_CREDENTIAL = defineNodeError(
    "ERR_UNKNOWN_CREDENTIAL",
    NodeError,
    (x: string, y: string) => `${x} identifier does not exist: ${y}`,
  );
  const ERR_UNKNOWN_ENCODING = defineNodeError(
    "ERR_UNKNOWN_ENCODING",
    NodeTypeError,
    (x: string) => format("Unknown encoding: %s", x),
  );
  const ERR_UNKNOWN_FILE_EXTENSION = defineNodeError(
    "ERR_UNKNOWN_FILE_EXTENSION",
    NodeTypeError,
    (x: string, y: string) => `Unknown file extension "${x}" for ${y}`,
  );
  const ERR_UNKNOWN_MODULE_FORMAT = defineNodeError(
    "ERR_UNKNOWN_MODULE_FORMAT",
    NodeRangeError,
    (x: string) => `Unknown module format: ${x}`,
  );
  const ERR_UNKNOWN_SIGNAL = defineNodeError(
    "ERR_UNKNOWN_SIGNAL",
    NodeTypeError,
    (x: string) => `Unknown signal: ${x}`,
  );
  const ERR_UNSUPPORTED_DIR_IMPORT = defineNodeError(
    "ERR_UNSUPPORTED_DIR_IMPORT",
    NodeError,
    (x: string, y: string) =>
      `Directory import '${x}' is not supported resolving ES modules, imported from ${y}`,
  );
  const ERR_UNSUPPORTED_ESM_URL_SCHEME = defineNodeError(
    "ERR_UNSUPPORTED_ESM_URL_SCHEME",
    NodeError,
    () => `Only file and data URLs are supported by the default ESM loader`,
  );
  const ERR_USE_AFTER_CLOSE = defineNodeError(
    "ERR_USE_AFTER_CLOSE",
    NodeError,
    (x: string) => `${x} was closed`,
  );
  const ERR_V8BREAKITERATOR = defineNodeError(
    "ERR_V8BREAKITERATOR",
    NodeError,
    () =>
      `Full ICU data not installed. See https://github.com/nodejs/node/wiki/Intl`,
  );
  const ERR_VALID_PERFORMANCE_ENTRY_TYPE = defineNodeError(
    "ERR_VALID_PERFORMANCE_ENTRY_TYPE",
    NodeError,
    () => `At least one valid performance entry type is required`,
  );
  const ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING = defineNodeError(
    "ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING",
    NodeTypeError,
    () => `A dynamic import callback was not specified.`,
  );
  const ERR_VM_MODULE_ALREADY_LINKED = defineNodeError(
    "ERR_VM_MODULE_ALREADY_LINKED",
    NodeError,
    () => `Module has already been linked`,
  );
  const ERR_VM_MODULE_CANNOT_CREATE_CACHED_DATA = defineNodeError(
    "ERR_VM_MODULE_CANNOT_CREATE_CACHED_DATA",
    NodeError,
    () => `Cached data cannot be created for a module which has been evaluated`,
  );
  const ERR_VM_MODULE_DIFFERENT_CONTEXT = defineNodeError(
    "ERR_VM_MODULE_DIFFERENT_CONTEXT",
    NodeError,
    () => `Linked modules must use the same context`,
  );
  const ERR_VM_MODULE_LINKING_ERRORED = defineNodeError(
    "ERR_VM_MODULE_LINKING_ERRORED",
    NodeError,
    () => `Linking has already failed for the provided module`,
  );
  const ERR_VM_MODULE_NOT_MODULE = defineNodeError(
    "ERR_VM_MODULE_NOT_MODULE",
    NodeError,
    () => `Provided module is not an instance of Module`,
  );
  class ERR_VM_MODULE_LINK_FAILURE extends NodeError {
    // deno-lint-ignore no-explicit-any
    constructor(message: string, cause?: any) {
      super("ERR_VM_MODULE_LINK_FAILURE", message);
      if (cause !== undefined) {
        // deno-lint-ignore no-explicit-any
        (this as any).cause = cause;
      }
    }
  }
  const ERR_MODULE_LINK_MISMATCH = defineNodeError(
    "ERR_MODULE_LINK_MISMATCH",
    NodeTypeError,
    (x: string) => x,
  );
  const ERR_VM_MODULE_STATUS = defineNodeError(
    "ERR_VM_MODULE_STATUS",
    NodeError,
    (x: string) => `Module status ${x}`,
  );
  const ERR_WASI_ALREADY_STARTED = defineNodeError(
    "ERR_WASI_ALREADY_STARTED",
    NodeError,
    () => `WASI instance has already started`,
  );
  const ERR_WASI_NOT_STARTED = defineNodeError(
    "ERR_WASI_NOT_STARTED",
    NodeError,
    () => `wasi.start() has not been called`,
  );
  const ERR_WORKER_INVALID_EXEC_ARGV = defineNodeError(
    "ERR_WORKER_INVALID_EXEC_ARGV",
    NodeError,
    (errors: string[], msg = "invalid execArgv flags") =>
      `Initiated Worker with ${msg}: ${ArrayPrototypeJoin(errors, ", ")}`,
  );
  const ERR_WORKER_INIT_FAILED = defineNodeError(
    "ERR_WORKER_INIT_FAILED",
    NodeError,
    (x: string) => `Worker initialization failure: ${x}`,
  );
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
  const ERR_WORKER_NOT_RUNNING = defineNodeError(
    "ERR_WORKER_NOT_RUNNING",
    NodeError,
    () => `Worker instance not running`,
  );
  const ERR_WORKER_MESSAGING_ERRORED = defineNodeError(
    "ERR_WORKER_MESSAGING_ERRORED",
    NodeError,
    () => "The destination thread threw an error while receiving the message.",
  );
  const ERR_WORKER_MESSAGING_FAILED = defineNodeError(
    "ERR_WORKER_MESSAGING_FAILED",
    NodeError,
    () => "The destination thread refused or failed to receive the message.",
  );
  const ERR_WORKER_MESSAGING_SAME_THREAD = defineNodeError(
    "ERR_WORKER_MESSAGING_SAME_THREAD",
    NodeError,
    () => "Cannot send a message to the same thread.",
  );
  const ERR_WORKER_MESSAGING_TIMEOUT = defineNodeError(
    "ERR_WORKER_MESSAGING_TIMEOUT",
    NodeError,
    () => "Sending a message to another thread timed out.",
  );
  const ERR_WORKER_OUT_OF_MEMORY = defineNodeError(
    "ERR_WORKER_OUT_OF_MEMORY",
    NodeError,
    (x: string) => `Worker terminated due to reaching memory limit: ${x}`,
  );
  const ERR_WORKER_UNSERIALIZABLE_ERROR = defineNodeError(
    "ERR_WORKER_UNSERIALIZABLE_ERROR",
    NodeError,
    () => `Serializing an uncaught exception failed`,
  );
  const ERR_WORKER_UNSUPPORTED_EXTENSION = defineNodeError(
    "ERR_WORKER_UNSUPPORTED_EXTENSION",
    NodeTypeError,
    (x: string) =>
      `The worker script extension must be ".js", ".mjs", or ".cjs". Received "${x}"`,
  );
  const ERR_WORKER_UNSUPPORTED_OPERATION = defineNodeError(
    "ERR_WORKER_UNSUPPORTED_OPERATION",
    NodeTypeError,
    (x: string) => `${x} is not supported in workers`,
  );
  const ERR_ZLIB_INITIALIZATION_FAILED = defineNodeError(
    "ERR_ZLIB_INITIALIZATION_FAILED",
    NodeError,
    (message = "Initialization failed") => message,
  );
  class ERR_FALSY_VALUE_REJECTION extends NodeError {
    reason: string;
    constructor(reason: string) {
      super(
        "ERR_FALSY_VALUE_REJECTION",
        "Promise was rejected with falsy value",
      );
      this.reason = reason;
    }
  }

  const ERR_HTTP2_TOO_MANY_CUSTOM_SETTINGS = defineNodeError(
    "ERR_HTTP2_TOO_MANY_CUSTOM_SETTINGS",
    NodeError,
    () => "Number of custom settings exceeds MAX_ADDITIONAL_SETTINGS",
  );

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

  const ERR_INVALID_CHAR = defineNodeError(
    "ERR_INVALID_CHAR",
    NodeTypeError,
    (name: string, field?: string) =>
      field === undefined
        ? `Invalid character in ${name}`
        : `Invalid character in ${name} ["${field}"]`,
  );

  const ERR_INVALID_OPT_VALUE = defineNodeError(
    "ERR_INVALID_OPT_VALUE",
    NodeTypeError,
    (name: string, value: unknown) =>
      `The value "${value}" is invalid for option "${name}"`,
  );

  const ERR_INVALID_RETURN_PROPERTY = defineNodeError(
    "ERR_INVALID_RETURN_PROPERTY",
    NodeTypeError,
    (input: string, name: string, prop: string, value: string) =>
      `Expected a valid ${input} to be returned for the "${prop}" from the "${name}" function but got ${value}.`,
  );

  // deno-lint-ignore no-explicit-any
  function buildReturnPropertyType(value: any) {
    if (value && value.constructor && value.constructor.name) {
      return `instance of ${value.constructor.name}`;
    } else {
      return `type ${typeof value}`;
    }
  }

  const ERR_INVALID_RETURN_PROPERTY_VALUE = defineNodeError(
    "ERR_INVALID_RETURN_PROPERTY_VALUE",
    NodeTypeError,
    (input: string, name: string, prop: string, value: unknown) =>
      `Expected ${input} to be returned for the "${prop}" from the "${name}" function but got ${
        buildReturnPropertyType(
          value,
        )
      }.`,
  );

  const ERR_INVALID_RETURN_VALUE = defineNodeError(
    "ERR_INVALID_RETURN_VALUE",
    NodeTypeError,
    (input: string, name: string, value: unknown) =>
      `Expected ${input} to be returned from the "${name}" function but got ${
        determineSpecificType(
          value,
        )
      }.`,
  );

  const ERR_NOT_IMPLEMENTED = defineNodeError(
    "ERR_NOT_IMPLEMENTED",
    NodeError,
    (message?: string) =>
      message ? `Not implemented: ${message}` : "Not implemented",
  );

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

  const ERR_MODULE_NOT_FOUND = defineNodeError(
    "ERR_MODULE_NOT_FOUND",
    NodeError,
    (path: string, base: string, type: string = "package") =>
      `Cannot find ${type} '${path}' imported from ${base}`,
  );

  class ERR_INVALID_PACKAGE_CONFIG extends NodeError {
    constructor(path: string, base?: string, message?: string) {
      const msg = `Invalid package config ${path}${
        base ? ` while importing ${base}` : ""
      }${message ? `. ${message}` : ""}`;
      super("ERR_INVALID_PACKAGE_CONFIG", msg);
    }
  }

  const ERR_INVALID_MODULE_SPECIFIER = defineNodeError(
    "ERR_INVALID_MODULE_SPECIFIER",
    NodeTypeError,
    (request: string, reason: string, base?: string) =>
      `Invalid module "${request}" ${reason}${
        base ? ` imported from ${base}` : ""
      }`,
  );

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
        msg =
          `Invalid "exports" main target ${JSONStringify(target)} defined ` +
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

  const ERR_PARSE_ARGS_INVALID_OPTION_VALUE = defineNodeError(
    "ERR_PARSE_ARGS_INVALID_OPTION_VALUE",
    NodeTypeError,
    (x: string) => x,
  );

  const ERR_PARSE_ARGS_UNEXPECTED_POSITIONAL = defineNodeError(
    "ERR_PARSE_ARGS_UNEXPECTED_POSITIONAL",
    NodeTypeError,
    (x: string) =>
      `Unexpected argument '${x}'. This ` +
      `command does not take positional arguments`,
  );

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

  const ERR_HTTP_SOCKET_ASSIGNED = defineNodeError(
    "ERR_HTTP_SOCKET_ASSIGNED",
    NodeError,
    () => `ServerResponse has an already assigned socket`,
  );

  const ERR_INVALID_STATE = defineNodeError(
    "ERR_INVALID_STATE",
    NodeError,
    (message: string) => `Invalid state: ${message}`,
  );

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

  function extractOsErrorNumberFromErrorMessage(
    e: unknown,
  ): number | undefined {
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
  codes.ERR_PARSE_ARGS_INVALID_OPTION_VALUE =
    ERR_PARSE_ARGS_INVALID_OPTION_VALUE;
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
    ERR_CLOSED_MESSAGE_PORT,
    ERR_CONSOLE_WRITABLE_STREAM,
    ERR_CONSTRUCT_CALL_INVALID,
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
    ERR_HTTP_SOCKET_ASSIGNED,
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
    ERR_PROXY_INVALID_CONFIG,
    ERR_PROXY_TUNNEL,
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
    ERR_REQUIRE_ASYNC_MODULE,
    ERR_REQUIRE_CYCLE_MODULE,
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
    ERR_TLS_INVALID_PROTOCOL_METHOD,
    ERR_TLS_INVALID_PROTOCOL_VERSION,
    ERR_TLS_ALPN_CALLBACK_INVALID_RESULT,
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
    ERR_VM_MODULE_LINK_FAILURE,
    ERR_VM_MODULE_LINKING_ERRORED,
    ERR_VM_MODULE_NOT_MODULE,
    ERR_VM_MODULE_STATUS,
    ERR_MODULE_LINK_MISMATCH,
    ERR_WASI_ALREADY_STARTED,
    ERR_WASI_NOT_STARTED,
    ERR_WORKER_INIT_FAILED,
    ERR_WORKER_INVALID_EXEC_ARGV,
    ERR_WORKER_MESSAGING_ERRORED,
    ERR_WORKER_MESSAGING_FAILED,
    ERR_WORKER_MESSAGING_SAME_THREAD,
    ERR_WORKER_MESSAGING_TIMEOUT,
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
    E,
    NodeError,
    NodeErrorAbstraction,
    NodeRangeError,
    NodeSyntaxError,
    NodeTypeError,
    NodeURIError,
    NodeAggregateError,
    SystemError,
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
      ERR_CLOSED_MESSAGE_PORT,
      ERR_CONSOLE_WRITABLE_STREAM,
      ERR_CONSTRUCT_CALL_INVALID,
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
      ERR_PROXY_INVALID_CONFIG,
      ERR_PROXY_TUNNEL,
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
      ERR_REQUIRE_ASYNC_MODULE,
      ERR_REQUIRE_CYCLE_MODULE,
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
      ERR_TLS_INVALID_PROTOCOL_METHOD,
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
      ERR_VM_MODULE_LINK_FAILURE,
      ERR_VM_MODULE_LINKING_ERRORED,
      ERR_VM_MODULE_NOT_MODULE,
      ERR_VM_MODULE_STATUS,
      ERR_MODULE_LINK_MISMATCH,
      ERR_WASI_ALREADY_STARTED,
      ERR_WASI_NOT_STARTED,
      ERR_WORKER_INIT_FAILED,
      ERR_WORKER_INVALID_EXEC_ARGV,
      ERR_WORKER_MESSAGING_ERRORED,
      ERR_WORKER_MESSAGING_FAILED,
      ERR_WORKER_MESSAGING_SAME_THREAD,
      ERR_WORKER_MESSAGING_TIMEOUT,
      ERR_WORKER_NOT_RUNNING,
      ERR_WORKER_OUT_OF_MEMORY,
      ERR_WORKER_PATH,
      ERR_WORKER_UNSERIALIZABLE_ERROR,
      ERR_WORKER_UNSUPPORTED_EXTENSION,
      ERR_WORKER_UNSUPPORTED_OPERATION,
      ERR_ZLIB_INITIALIZATION_FAILED,
      E,
      NodeError,
      NodeErrorAbstraction,
      NodeRangeError,
      NodeSyntaxError,
      NodeTypeError,
      NodeURIError,
      SystemError,
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
})();
