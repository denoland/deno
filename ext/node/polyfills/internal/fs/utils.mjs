// Copyright 2018-2026 the Deno authors. MIT license.

"use strict";

import { core, primordials } from "ext:core/mod.js";
// node `fs.Stats` / `fs.Dirent` cppgc classes, implemented in Rust
// (ext/node/ops/fs.rs).
import { Dirent, Stats } from "ext:core/ops";
const { op_node_fs_dirent, op_node_fs_dirent_from_stats } = core.ops;
const {
  ArrayIsArray,
  BigInt,
  DataViewPrototype,
  DataViewPrototypeGetBuffer,
  DataViewPrototypeGetByteOffset,
  DataViewPrototypeGetByteLength,
  Date,
  DateNow,
  DatePrototypeGetTime,
  ErrorCaptureStackTrace,
  FunctionPrototypeCall,
  MathMin,
  Number,
  NumberIsFinite,
  NumberIsInteger,
  ObjectDefineProperties,
  ObjectDefineProperty,
  ObjectIs,
  ObjectPrototypeIsPrototypeOf,
  ObjectSetPrototypeOf,
  ReflectApply,
  ReflectOwnKeys,
  SafeArrayIterator,
  SafeRegExp,
  StringPrototypeEndsWith,
  StringPrototypeIncludes,
  StringPrototypeReplace,
  Symbol,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetByteOffset,
  TypedArrayPrototypeIncludes,
  Uint8Array,
  Uint8ArrayPrototype,
} = primordials;
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const {
  ERR_FS_EISDIR,
  ERR_FS_INVALID_SYMLINK_TYPE,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
  ERR_OUT_OF_RANGE,
  hideStackFrames,
  uvException,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");

const {
  isArrayBufferView,
  isBigUint64Array,
  isDate,
  isUint8Array,
} = core.loadExtScript("ext:deno_node/internal/util/types.ts");
const { kEmptyObject, once } = core.loadExtScript(
  "ext:deno_node/internal/util.mjs",
);
const { toPathIfFileURL } = core.loadExtScript(
  "ext:deno_node/internal/url.ts",
);
const {
  validateAbortSignal,
  validateBoolean,
  validateFunction,
  validateInt32,
  validateInteger,
  validateObject,
  validateUint32,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const lazyPath = core.createLazyLoader("node:path");
const kType = Symbol("type");
const kStats = Symbol("stats");
const assert = core.loadExtScript(
  "ext:deno_node/internal/assert.mjs",
);
const { lstat, lstatSync } = core.loadExtScript(
  "ext:deno_node/_fs/_fs_lstat.ts",
);
const { isWindows } = core.loadExtScript("ext:deno_node/_util/os.ts");
const lazyProcess = core.createLazyLoader("node:process");
const { ERR_INCOMPATIBLE_OPTION_PAIR } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);
const {
  fs: fsConstants,
  os: osConstants,
} = core.loadExtScript("ext:deno_node/internal_binding/constants.ts");
const {
  F_OK = 0,
  W_OK = 0,
  R_OK = 0,
  X_OK = 0,
  COPYFILE_EXCL,
  COPYFILE_FICLONE,
  COPYFILE_FICLONE_FORCE,
  O_APPEND,
  O_CREAT,
  O_EXCL,
  O_RDONLY,
  O_RDWR,
  O_SYNC,
  O_TRUNC,
  O_WRONLY,
  S_IFBLK,
  S_IFCHR,
  S_IFDIR,
  S_IFIFO,
  S_IFLNK,
  S_IFMT,
  S_IFREG,
  S_IFSOCK,
  UV_FS_SYMLINK_DIR,
  UV_FS_SYMLINK_JUNCTION,
  UV_DIRENT_UNKNOWN,
  UV_DIRENT_FILE,
  UV_DIRENT_DIR,
  UV_DIRENT_LINK,
  UV_DIRENT_FIFO,
  UV_DIRENT_SOCKET,
  UV_DIRENT_CHAR,
  UV_DIRENT_BLOCK,
} = fsConstants;

// The access modes can be any of F_OK, R_OK, W_OK or X_OK. Some might not be
// available on specific systems. They can be used in combination as well
// (F_OK | R_OK | W_OK | X_OK).
const kMinimumAccessMode = MathMin(F_OK, W_OK, R_OK, X_OK);
const kMaximumAccessMode = F_OK | W_OK | R_OK | X_OK;

const kDefaultCopyMode = 0;
// The copy modes can be any of COPYFILE_EXCL, COPYFILE_FICLONE or
// COPYFILE_FICLONE_FORCE. They can be used in combination as well
// (COPYFILE_EXCL | COPYFILE_FICLONE | COPYFILE_FICLONE_FORCE).
const kMinimumCopyMode = MathMin(
  kDefaultCopyMode,
  COPYFILE_EXCL,
  COPYFILE_FICLONE,
  COPYFILE_FICLONE_FORCE,
);
const kMaximumCopyMode = COPYFILE_EXCL |
  COPYFILE_FICLONE |
  COPYFILE_FICLONE_FORCE;

// Most platforms don't allow reads or writes >= 2 GB.
// See https://github.com/libuv/libuv/pull/1501.
const kIoMaxLength = 2 ** 31 - 1;

// Use 64kb in case the file type is not a regular file and thus do not know the
// actual file size. Increasing the value further results in more frequent over
// allocation for small files and consumes CPU time and memory that should be
// used else wise.
// Use up to 512kb per read otherwise to partition reading big files to prevent
// blocking other threads in case the available threads are all in use.
const kReadFileUnknownBufferLength = 64 * 1024;
const kReadFileBufferLength = 512 * 1024;

const kWriteFileMaxChunkSize = 512 * 1024;

export const kMaxUserId = 2 ** 32 - 1;

export function assertEncoding(encoding) {
  if (encoding && !Buffer.isEncoding(encoding)) {
    const reason = "is invalid encoding";
    throw new ERR_INVALID_ARG_VALUE("encoding", encoding, reason);
  }
}

// `fs.Dirent` is the cppgc class above (built in Rust). The JS `Dirent` +
// `DirentFromStats` classes were removed; entries are built via ops.
export { Dirent };

// Build a Dirent from a Deno DirEntry, mapping to the uv dirent type
// (UNKNOWN=0 FILE=1 DIR=2 LINK=3) the cppgc Dirent expects.
export function direntFromDeno(entry, path) {
  let kind = UV_DIRENT_UNKNOWN;
  if (entry.isDirectory) {
    kind = UV_DIRENT_DIR;
  } else if (entry.isFile) {
    kind = UV_DIRENT_FILE;
  } else if (entry.isSymlink) {
    kind = UV_DIRENT_LINK;
  }
  return op_node_fs_dirent(entry.name, path ?? entry.parentPath, kind);
}

// Build a Dirent whose predicates come from a `Stats` (was `DirentFromStats`).
export function direntFromStats(name, stats, path) {
  return op_node_fs_dirent_from_stats(name, path, stats);
}

export function copyObject(source) {
  const target = {};
  for (const key in source) {
    target[key] = source[key];
  }
  return target;
}

const bufferSep = Buffer.from(lazyPath().default.sep);

function join(path, name) {
  if (
    (typeof path === "string" || isUint8Array(path)) &&
    name === undefined
  ) {
    return path;
  }

  if (typeof path === "string" && isUint8Array(name)) {
    const pathBuffer = Buffer.from(
      // deno-lint-ignore prefer-primordials `join` is a `node:path` function
      lazyPath().default.join(path, lazyPath().default.sep),
    );
    // Ignore lint. `concat` is a 'node:buffer' static method on `Buffer`
    // deno-lint-ignore prefer-primordials
    return Buffer.concat([pathBuffer, name]);
  }

  if (typeof path === "string" && typeof name === "string") {
    // deno-lint-ignore prefer-primordials `join` is a `node:path` function
    return lazyPath().default.join(path, name);
  }

  if (isUint8Array(path) && isUint8Array(name)) {
    // Ignore lint. `concat` is a 'node:buffer' static method on `Buffer`
    // deno-lint-ignore prefer-primordials
    return Buffer.concat([path, bufferSep, name]);
  }

  throw new ERR_INVALID_ARG_TYPE(
    "path",
    ["string", "Buffer"],
    path,
  );
}

export function getDirents(path, { 0: names, 1: types }, callback) {
  let i;
  if (typeof callback === "function") {
    const len = names.length;
    let toFinish = 0;
    callback = once(callback);
    for (i = 0; i < len; i++) {
      const type = types[i];
      if (type === UV_DIRENT_UNKNOWN) {
        const name = names[i];
        const idx = i;
        toFinish++;
        let filepath;
        try {
          filepath = join(path, name);
        } catch (err) {
          callback(err);
          return;
        }
        lstat(filepath, (err, stats) => {
          if (err) {
            callback(err);
            return;
          }
          names[idx] = direntFromStats(name, stats, path);
          if (--toFinish === 0) {
            callback(null, names);
          }
        });
      } else {
        names[i] = op_node_fs_dirent(names[i], path, types[i]);
      }
    }
    if (toFinish === 0) {
      callback(null, names);
    }
  } else {
    const len = names.length;
    for (i = 0; i < len; i++) {
      names[i] = getDirent(path, names[i], types[i]);
    }
    return names;
  }
}

export function getDirent(path, name, type, callback) {
  if (typeof callback === "function") {
    if (type === UV_DIRENT_UNKNOWN) {
      let filepath;
      try {
        filepath = join(path, name);
      } catch (err) {
        callback(err);
        return;
      }
      lstat(filepath, (err, stats) => {
        if (err) {
          callback(err);
          return;
        }
        callback(null, direntFromStats(name, stats, path));
      });
    } else {
      callback(null, op_node_fs_dirent(name, path, type));
    }
  } else if (type === UV_DIRENT_UNKNOWN) {
    const stats = lstatSync(join(path, name));
    return direntFromStats(name, stats, path);
  } else {
    return op_node_fs_dirent(name, path, type);
  }
}

/**
 * @template T
 * @param {unknown} options
 * @param {T} [defaultOptions]
 * @returns {T}
 */
export function getOptions(options, defaultOptions = kEmptyObject) {
  if (
    options === null || options === undefined ||
    typeof options === "function"
  ) {
    return defaultOptions;
  }

  if (typeof options === "string") {
    defaultOptions = { ...defaultOptions };
    defaultOptions.encoding = options;
    options = defaultOptions;
  } else if (typeof options !== "object") {
    throw new ERR_INVALID_ARG_TYPE("options", ["string", "Object"], options);
  }

  if (options.encoding !== "buffer") {
    assertEncoding(options.encoding);
  }

  if (options.signal !== undefined) {
    validateAbortSignal(options.signal, "options.signal");
  }
  return options;
}

/**
 * @param {InternalFSBinding.FSSyncContext} ctx
 */
export function handleErrorFromBinding(ctx) {
  if (ctx.errno !== undefined) { // libuv error numbers
    const err = uvException(ctx);
    ErrorCaptureStackTrace(err, handleErrorFromBinding);
    throw err;
  }
  if (ctx.error !== undefined) { // Errors created in C++ land.
    // TODO(joyeecheung): currently, ctx.error are encoding errors
    // usually caused by memory problems. We need to figure out proper error
    // code(s) for this.
    ErrorCaptureStackTrace(ctx.error, handleErrorFromBinding);
    throw ctx.error;
  }
}

// Check if the path contains null types if it is a string nor Uint8Array,
// otherwise return silently.
export const nullCheck = hideStackFrames(
  (path, propName, throwError = true) => {
    const pathIsString = typeof path === "string";
    const pathIsUint8Array = isUint8Array(path);

    // We can only perform meaningful checks on strings and Uint8Arrays.
    if (
      (!pathIsString && !pathIsUint8Array) ||
      (pathIsString && !StringPrototypeIncludes(path, "\u0000")) ||
      (pathIsUint8Array && !TypedArrayPrototypeIncludes(path, 0))
    ) {
      return;
    }

    const err = new ERR_INVALID_ARG_VALUE(
      propName,
      path,
      "must be a string or Uint8Array without null bytes",
    );
    if (throwError) {
      throw err;
    }
    return err;
  },
);

export function preprocessSymlinkDestination(path, type, linkPath) {
  if (!isWindows) {
    // No preprocessing is needed on Unix.
    return path;
  }
  path = "" + path;
  if (type === "junction") {
    // Junctions paths need to be absolute and \\?\-prefixed.
    // A relative target is relative to the link's parent directory.
    path = lazyPath().default.resolve(linkPath, "..", path);
    return lazyPath().default.toNamespacedPath(path);
  }
  if (lazyPath().default.isAbsolute(path)) {
    // If the path is absolute, use the \\?\-prefix to enable long filenames
    return lazyPath().default.toNamespacedPath(path);
  }
  // Windows symlinks don't tolerate forward slashes.
  return StringPrototypeReplace(path, new SafeRegExp(/\//g), "\\");
}

// Constructor for file stats.
// node `fs.Stats` is the cppgc object built in Rust (ext/node/ops/fs.rs);
// the stat ops return it directly. `BigIntStats` is the same class
// (one-class design: `bigint: true` stats carry BigInt fields). The JS
// `StatsBase`/`Stats`/`BigIntStats` classes and `getStatsFromBinding` were
// removed so they no longer load into the startup snapshot.
export { Stats };
export const BigIntStats = Stats;

export function stringToFlags(flags, name = "flags") {
  if (typeof flags === "number") {
    validateInt32(flags, name);
    return flags;
  }

  if (flags == null) {
    return O_RDONLY;
  }

  switch (flags) {
    case "r":
      return O_RDONLY;
    case "rs": // Fall through.
    case "sr":
      return O_RDONLY | O_SYNC;
    case "r+":
      return O_RDWR;
    case "rs+": // Fall through.
    case "sr+":
      return O_RDWR | O_SYNC;

    case "w":
      return O_TRUNC | O_CREAT | O_WRONLY;
    case "wx": // Fall through.
    case "xw":
      return O_TRUNC | O_CREAT | O_WRONLY | O_EXCL;

    case "w+":
      return O_TRUNC | O_CREAT | O_RDWR;
    case "wx+": // Fall through.
    case "xw+":
      return O_TRUNC | O_CREAT | O_RDWR | O_EXCL;

    case "a":
      return O_APPEND | O_CREAT | O_WRONLY;
    case "ax": // Fall through.
    case "xa":
      return O_APPEND | O_CREAT | O_WRONLY | O_EXCL;
    case "as": // Fall through.
    case "sa":
      return O_APPEND | O_CREAT | O_WRONLY | O_SYNC;

    case "a+":
      return O_APPEND | O_CREAT | O_RDWR;
    case "ax+": // Fall through.
    case "xa+":
      return O_APPEND | O_CREAT | O_RDWR | O_EXCL;
    case "as+": // Fall through.
    case "sa+":
      return O_APPEND | O_CREAT | O_RDWR | O_SYNC;
  }

  throw new ERR_INVALID_ARG_VALUE("flags", flags);
}

export const stringToSymlinkType = hideStackFrames((type) => {
  let flags = 0;
  if (typeof type === "string") {
    switch (type) {
      case "dir":
        flags |= UV_FS_SYMLINK_DIR;
        break;
      case "junction":
        flags |= UV_FS_SYMLINK_JUNCTION;
        break;
      case "file":
        break;
      default:
        throw new ERR_FS_INVALID_SYMLINK_TYPE(type);
    }
  }
  return flags;
});

// converts Date or number to a fractional UNIX timestamp
export function toUnixTimestamp(time, name = "time") {
  // eslint-disable-next-line eqeqeq
  if (typeof time === "string" && +time == time) {
    return +time;
  }
  if (NumberIsFinite(time)) {
    if (time < 0) {
      return DateNow() / 1000;
    }
    return time;
  }
  if (isDate(time)) {
    // Convert to 123.456 UNIX timestamp
    return DatePrototypeGetTime(time) / 1000;
  }
  throw new ERR_INVALID_ARG_TYPE(name, ["Date", "Time in seconds"], time);
}

export const validateOffsetLengthRead = hideStackFrames(
  (offset, length, bufferLength) => {
    if (offset < 0) {
      throw new ERR_OUT_OF_RANGE("offset", ">= 0", offset);
    }
    if (length < 0) {
      throw new ERR_OUT_OF_RANGE("length", ">= 0", length);
    }
    if (offset + length > bufferLength) {
      throw new ERR_OUT_OF_RANGE(
        "length",
        `<= ${bufferLength - offset}`,
        length,
      );
    }
  },
);

export const validateOffsetLengthWrite = hideStackFrames(
  (offset, length, byteLength) => {
    if (offset > byteLength) {
      throw new ERR_OUT_OF_RANGE("offset", `<= ${byteLength}`, offset);
    }

    if (length > byteLength - offset) {
      throw new ERR_OUT_OF_RANGE("length", `<= ${byteLength - offset}`, length);
    }

    if (length < 0) {
      throw new ERR_OUT_OF_RANGE("length", ">= 0", length);
    }

    validateInt32(length, "length", 0);
  },
);

export const validatePath = hideStackFrames((path, propName = "path") => {
  if (typeof path !== "string" && !isUint8Array(path)) {
    throw new ERR_INVALID_ARG_TYPE(propName, ["string", "Buffer", "URL"], path);
  }

  const err = nullCheck(path, propName, false);

  if (err !== undefined) {
    throw err;
  }
});

export const getValidatedPath = hideStackFrames(
  (fileURLOrPath, propName = "path") => {
    const path = toPathIfFileURL(fileURLOrPath);
    validatePath(path, propName);
    return path;
  },
);

/**
 * @param {string | Buffer | Uint8Array | URL} fileURLOrPath
 * @param {string} [propName]
 * @returns string
 */
export const getValidatedPathToString = (fileURLOrPath, propName) => {
  const path = getValidatedPath(fileURLOrPath, propName);
  if (isUint8Array(path)) {
    return new TextDecoder().decode(path);
  }
  if (Buffer.isBuffer(path)) {
    // deno-lint-ignore prefer-primordials
    return path.toString();
  }
  return path;
};

export const getValidatedFd = hideStackFrames((fd, propName = "fd") => {
  if (ObjectIs(fd, -0)) {
    return 0;
  }

  validateInt32(fd, propName, 0);

  return fd;
});

export const validateBufferArray = hideStackFrames(
  (buffers, propName = "buffers") => {
    if (!ArrayIsArray(buffers)) {
      throw new ERR_INVALID_ARG_TYPE(propName, "ArrayBufferView[]", buffers);
    }

    for (let i = 0; i < buffers.length; i++) {
      if (!isArrayBufferView(buffers[i])) {
        throw new ERR_INVALID_ARG_TYPE(propName, "ArrayBufferView[]", buffers);
      }
    }

    return buffers;
  },
);

let nonPortableTemplateWarn = true;

export function warnOnNonPortableTemplate(template) {
  // Template strings passed to the mkdtemp() family of functions should not
  // end with 'X' because they are handled inconsistently across platforms.
  if (nonPortableTemplateWarn && StringPrototypeEndsWith(template, "X")) {
    lazyProcess().default.emitWarning(
      "mkdtemp() templates ending with X are not portable. " +
        "For details see: https://nodejs.org/api/fs.html",
    );
    nonPortableTemplateWarn = false;
  }
}

/** @import { CopyOptionsBase } from "ext:deno_node/_fs/cp/cp.d.ts" */
/** @type {CopyOptionsBase} */
const defaultCpOptions = {
  dereference: false,
  errorOnExist: false,
  filter: undefined,
  force: true,
  preserveTimestamps: false,
  recursive: false,
  verbatimSymlinks: false,
};

export const defaultRmOptions = {
  recursive: false,
  force: false,
  retryDelay: 100,
  maxRetries: 0,
};

const defaultRmdirOptions = {
  retryDelay: 100,
  maxRetries: 0,
  recursive: false,
};

/** @type {(options: CopyOptionsBase | undefined) => CopyOptionsBase} */
export const validateCpOptions = hideStackFrames((options) => {
  if (options === undefined) {
    return { ...defaultCpOptions };
  }
  validateObject(options, "options");
  options = { ...defaultCpOptions, ...options };
  validateBoolean(options.dereference, "options.dereference");
  validateBoolean(options.errorOnExist, "options.errorOnExist");
  validateBoolean(options.force, "options.force");
  validateBoolean(options.preserveTimestamps, "options.preserveTimestamps");
  validateBoolean(options.recursive, "options.recursive");
  validateBoolean(options.verbatimSymlinks, "options.verbatimSymlinks");
  options.mode = getValidMode(options.mode, "copyFile");
  if (options.dereference === true && options.verbatimSymlinks === true) {
    throw new ERR_INCOMPATIBLE_OPTION_PAIR("dereference", "verbatimSymlinks");
  }
  if (options.filter !== undefined) {
    validateFunction(options.filter, "options.filter");
  }
  return options;
});

/**
 * @typedef {{
 *   force: boolean;
 *   recursive?: boolean;
 *   retryDelay?: number;
 *   maxRetries?: number;
 * }} RmOptions
 */

/**
 * @typedef {(err: Error | false | null, options?: RmOptions) => void} RmOptionsCallback
 */

/** @type {(path: string, options: RmOptions, expectDir: boolean, cb: RmOptionsCallback) => void} */
export const validateRmOptions = hideStackFrames(
  (path, options, expectDir, cb) => {
    options = validateRmdirOptions(options, defaultRmOptions);
    validateBoolean(options.force, "options.force");

    lstat(path, (err, stats) => {
      if (err) {
        if (options.force && err.code === "ENOENT") {
          return cb(null, options);
        }
        return cb(err, options);
      }

      if (expectDir && !stats.isDirectory()) {
        return cb(false);
      }

      if (stats.isDirectory() && !options.recursive) {
        return cb(
          new ERR_FS_EISDIR({
            code: "EISDIR",
            message: "is a directory",
            path,
            syscall: "rm",
            errno: osConstants.errno.EISDIR,
          }),
        );
      }
      return cb(null, options);
    });
  },
);

/** @type {(path: string, options: RmOptions, expectDir: boolean) => RmOptions | false} */
export const validateRmOptionsSync = hideStackFrames(
  (path, options, expectDir) => {
    options = validateRmdirOptions(options, defaultRmOptions);
    validateBoolean(options.force, "options.force");

    if (!options.force || expectDir || !options.recursive) {
      const isDirectory = lstatSync(path, { throwIfNoEntry: !options.force })
        ?.isDirectory();

      if (expectDir && !isDirectory) {
        return false;
      }

      if (isDirectory && !options.recursive) {
        throw new ERR_FS_EISDIR({
          code: "EISDIR",
          message: "is a directory",
          path,
          syscall: "rm",
          errno: osConstants.errno.EISDIR,
        });
      }
    }

    return options;
  },
);

let recursiveRmdirWarned = lazyProcess().default.noDeprecation;
export function emitRecursiveRmdirWarning() {
  if (!recursiveRmdirWarned) {
    lazyProcess().default.emitWarning(
      "In future versions of Node.js, fs.rmdir(path, { recursive: true }) " +
        "will be removed. Use fs.rm(path, { recursive: true }) instead",
      "DeprecationWarning",
      "DEP0147",
    );
    recursiveRmdirWarned = true;
  }
}

export const validateRmdirOptions = hideStackFrames(
  (options, defaults = defaultRmdirOptions) => {
    if (options === undefined) {
      return defaults;
    }
    validateObject(options, "options");

    options = { ...defaults, ...options };

    validateBoolean(options.recursive, "options.recursive");
    validateInt32(options.retryDelay, "options.retryDelay", 0);
    validateUint32(options.maxRetries, "options.maxRetries");

    return options;
  },
);

export const getValidMode = hideStackFrames((mode, type) => {
  let min = kMinimumAccessMode;
  let max = kMaximumAccessMode;
  let def = F_OK;
  if (type === "copyFile") {
    min = kMinimumCopyMode;
    max = kMaximumCopyMode;
    def = mode || kDefaultCopyMode;
  } else {
    assert(type === "access");
  }
  if (mode == null) {
    return def;
  }
  if (NumberIsInteger(mode) && mode >= min && mode <= max) {
    return mode;
  }
  if (typeof mode !== "number") {
    throw new ERR_INVALID_ARG_TYPE("mode", "integer", mode);
  }
  throw new ERR_OUT_OF_RANGE(
    "mode",
    `an integer >= ${min} && <= ${max}`,
    mode,
  );
});

/** @type {(buffer: unknown, name: string) => asserts value is string} */
export const validateStringAfterArrayBufferView = hideStackFrames(
  (buffer, name) => {
    if (typeof buffer !== "string") {
      throw new ERR_INVALID_ARG_TYPE(
        name,
        ["string", "Buffer", "TypedArray", "DataView"],
        buffer,
      );
    }
  },
);

/** @type {(position: unknown, name: string, length?: number) => asserts position is number | bigint} */
export const validatePosition = hideStackFrames((position, name, length) => {
  if (typeof position === "number") {
    validateInteger(position, name, -1);
  } else if (typeof position === "bigint") {
    const maxPosition = 2n ** 63n - 1n - BigInt(length);
    if (!(position >= -1n && position <= maxPosition)) {
      throw new ERR_OUT_OF_RANGE(
        name,
        `>= -1 && <= ${maxPosition}`,
        position,
      );
    }
  } else {
    throw new ERR_INVALID_ARG_TYPE(name, ["integer", "bigint"], position);
  }
});

/** @type {(buffer: ArrayBufferView) => Uint8Array} */
export const arrayBufferViewToUint8Array = hideStackFrames(
  (buffer) => {
    if (!ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, buffer)) {
      if (ObjectPrototypeIsPrototypeOf(DataViewPrototype, buffer)) {
        return new Uint8Array(
          DataViewPrototypeGetBuffer(buffer),
          DataViewPrototypeGetByteOffset(buffer),
          DataViewPrototypeGetByteLength(buffer),
        );
      } else {
        return new Uint8Array(
          TypedArrayPrototypeGetBuffer(buffer),
          TypedArrayPrototypeGetByteOffset(buffer),
          TypedArrayPrototypeGetByteLength(buffer),
        );
      }
    }
    return buffer;
  },
);

export const realpathCacheKey = Symbol("realpathCacheKey");
export const constants = {
  kIoMaxLength,
  kMaxUserId,
  kReadFileBufferLength,
  kReadFileUnknownBufferLength,
  kWriteFileMaxChunkSize,
};

export default {
  constants,
  assertEncoding,
  BigIntStats, // for testing
  copyObject,
  Dirent,
  emitRecursiveRmdirWarning,
  getDirent,
  getDirents,
  getOptions,
  getValidatedFd,
  getValidatedPath,
  getValidatedPathToString,
  getValidMode,
  handleErrorFromBinding,
  kMaxUserId,
  nullCheck,
  preprocessSymlinkDestination,
  realpathCacheKey,
  stringToFlags,
  stringToSymlinkType,
  Stats,
  toUnixTimestamp,
  validateBufferArray,
  validateCpOptions,
  validateOffsetLengthRead,
  validateOffsetLengthWrite,
  validatePath,
  validatePosition,
  validateRmOptions,
  validateRmOptionsSync,
  validateRmdirOptions,
  validateStringAfterArrayBufferView,
  warnOnNonPortableTemplate,
};
