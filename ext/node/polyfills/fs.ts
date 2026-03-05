// Copyright 2018-2026 the Deno authors. MIT license.
import { fs as fsConstants } from "ext:deno_node/internal_binding/constants.ts";
import { codeMap } from "ext:deno_node/internal_binding/uv.ts";
import * as constants from "ext:deno_node/_fs/_fs_constants.ts";
import { cpFn } from "ext:deno_node/_fs/cp/cp.ts";
import { cpSyncFn } from "ext:deno_node/_fs/cp/cp_sync.ts";
import type {
  CopyOptions,
  CopySyncOptions,
} from "ext:deno_node/_fs/cp/cp.d.ts";
import Dir from "ext:deno_node/_fs/_fs_dir.ts";
import { op_node_fs_exists, op_node_fs_exists_sync } from "ext:core/ops";
import { kCustomPromisifiedSymbol } from "ext:deno_node/internal/util.mjs";
import * as process from "node:process";
import { fstat, fstatSync } from "ext:deno_node/_fs/_fs_fstat.ts";
import { FsFile } from "ext:deno_fs/30_fs.js";
import { lstat, lstatPromise, lstatSync } from "ext:deno_node/_fs/_fs_lstat.ts";
import { open, openPromise, openSync } from "ext:deno_node/_fs/_fs_open.ts";
import {
  opendir,
  opendirPromise,
  opendirSync,
} from "ext:deno_node/_fs/_fs_opendir.ts";
import { read, readSync } from "ext:deno_node/_fs/_fs_read.ts";
import {
  readdir,
  readdirPromise,
  readdirSync,
} from "ext:deno_node/_fs/_fs_readdir.ts";
import {
  readFile,
  readFilePromise,
  readFileSync,
} from "ext:deno_node/_fs/_fs_readFile.ts";
import { TextEncoder } from "ext:deno_web/08_text_encoding.js";
import { type MaybeEmpty, notImplemented } from "ext:deno_node/_utils.ts";
import {
  stat,
  statPromise,
  Stats,
  statSync,
} from "ext:deno_node/_fs/_fs_stat.ts";
import * as pathModule from "node:path";
import { pathFromURL } from "ext:deno_web/00_infra.js";
import {
  unwatchFile,
  watch,
  watchFile,
  watchPromise,
} from "ext:deno_node/_fs/_fs_watch.ts";
// @deno-types="./_fs/_fs_write.d.ts"
import { write, writeSync } from "ext:deno_node/_fs/_fs_write.ts";
// @deno-types="./_fs/_fs_writev.d.ts"
import { writev, writevSync } from "ext:deno_node/_fs/_fs_writev.ts";
import { readv, readvSync } from "ext:deno_node/_fs/_fs_readv.ts";
import {
  writeFile,
  writeFilePromise,
  writeFileSync,
} from "ext:deno_node/_fs/_fs_writeFile.ts";
// @deno-types="./internal/fs/streams.d.ts"
import {
  createReadStream,
  createWriteStream,
  ReadStream,
  WriteStream,
} from "ext:deno_node/internal/fs/streams.mjs";
import {
  copyObject,
  Dirent,
  emitRecursiveRmdirWarning,
  getOptions,
  getValidatedFd,
  getValidatedPath,
  getValidatedPathToString,
  getValidMode,
  kMaxUserId,
  type RmOptions,
  toUnixTimestamp as _toUnixTimestamp,
  validateCpOptions,
  validateRmdirOptions,
  validateRmOptions,
  validateRmOptionsSync,
  warnOnNonPortableTemplate,
} from "ext:deno_node/internal/fs/utils.mjs";
import { glob, globPromise, globSync } from "ext:deno_node/_fs/_fs_glob.ts";
import {
  parseFileMode,
  validateBoolean,
  validateFunction,
  validateInt32,
  validateInteger,
  validateObject,
  validateOneOf,
  validateString,
} from "ext:deno_node/internal/validators.mjs";
import { BigInt } from "ext:deno_node/internal/primordials.mjs";
import {
  denoErrorToNodeError,
  ERR_FS_RMDIR_ENOTDIR,
  ERR_INVALID_ARG_TYPE,
  ERR_METHOD_NOT_IMPLEMENTED,
  uvException,
} from "ext:deno_node/internal/errors.ts";
import { isMacOS, isWindows } from "ext:deno_node/_util/os.ts";
import {
  type CallbackWithError,
  isFd,
  makeCallback,
  maybeCallback,
  type WriteFileOptions,
} from "ext:deno_node/_fs/_fs_common.ts";
import { Encodings } from "ext:deno_node/_utils.ts";
import { normalizeEncoding, promisify } from "ext:deno_node/internal/util.mjs";
import { Buffer } from "node:buffer";
import type { BufferEncoding } from "ext:deno_node/_global.d.ts";
import type { Encoding } from "node:crypto";
import {
  op_fs_fchmod_async,
  op_fs_fchmod_sync,
  op_fs_fchown_async,
  op_fs_fchown_sync,
  op_fs_read_file_async,
  op_node_lchmod,
  op_node_lchmod_sync,
  op_node_lchown,
  op_node_lchown_sync,
  op_node_lutimes,
  op_node_lutimes_sync,
  op_node_mkdtemp,
  op_node_mkdtemp_sync,
  op_node_rmdir,
  op_node_rmdir_sync,
  op_node_statfs,
  op_node_statfs_sync,
} from "ext:core/ops";
import { core, primordials } from "ext:core/mod.js";

const {
  Error: PrimError,
  ErrorPrototype,
  MathTrunc,
  Number: PrimNumber,
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  Promise: PrimPromise,
  PromisePrototypeThen,
  PromiseReject,
  SymbolFor,
} = primordials;

const {
  F_OK,
  R_OK,
  W_OK,
  X_OK,
  O_RDONLY,
  O_WRONLY,
  O_RDWR,
  O_NOCTTY,
  O_TRUNC,
  O_APPEND,
  O_DIRECTORY,
  O_NOFOLLOW,
  O_SYNC,
  O_DSYNC,
  O_SYMLINK,
  O_NONBLOCK,
  O_CREAT,
  O_EXCL,
} = constants;

function link(
  existingPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
  callback: CallbackWithError,
) {
  existingPath = getValidatedPathToString(existingPath);
  newPath = getValidatedPathToString(newPath);

  PromisePrototypeThen(
    Deno.link(existingPath, newPath),
    () => callback(null),
    callback,
  );
}

const linkPromise = promisify(link) as (
  existingPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) => Promise<void>;

function linkSync(
  existingPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) {
  existingPath = getValidatedPathToString(existingPath);
  newPath = getValidatedPathToString(newPath);

  Deno.linkSync(existingPath, newPath);
}

function rename(
  oldPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
  callback: (err?: Error) => void,
) {
  oldPath = getValidatedPathToString(oldPath, "oldPath");
  newPath = getValidatedPathToString(newPath, "newPath");
  validateFunction(callback, "callback");

  PromisePrototypeThen(
    Deno.rename(oldPath, newPath),
    () => callback(),
    (err: Error) =>
      callback(denoErrorToNodeError(err, {
        syscall: "rename",
        path: oldPath,
        dest: newPath,
      })),
  );
}

const renamePromise = promisify(rename) as (
  oldPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) => Promise<void>;

function renameSync(
  oldPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) {
  oldPath = getValidatedPathToString(oldPath, "oldPath");
  newPath = getValidatedPathToString(newPath, "newPath");

  try {
    Deno.renameSync(oldPath, newPath);
  } catch (err) {
    throw denoErrorToNodeError(err as Error, {
      syscall: "rename",
      path: oldPath,
      dest: newPath,
    });
  }
}

type RealpathEncoding = BufferEncoding | "buffer";
type RealpathEncodingObj = { encoding?: RealpathEncoding };
type RealpathOptions = RealpathEncoding | RealpathEncodingObj;
type RealpathCallback = (err: Error | null, path?: string | Buffer) => void;

function encodeRealpathResult(
  result: string,
  options?: RealpathEncodingObj,
): string | Buffer {
  if (!options || !options.encoding || options.encoding === "utf8") {
    return result;
  }

  const asBuffer = Buffer.from(result);
  if (options.encoding === "buffer") {
    return asBuffer;
  }
  // deno-lint-ignore prefer-primordials
  return asBuffer.toString(options.encoding);
}

function realpath(
  path: string | Buffer,
  options?: RealpathOptions | RealpathCallback | RealpathEncoding,
  callback?: RealpathCallback,
) {
  if (typeof options === "function") {
    callback = options;
  }
  validateFunction(callback, "cb");
  options = getOptions(options) as RealpathEncodingObj;
  path = getValidatedPathToString(path);

  PromisePrototypeThen(
    Deno.realPath(path),
    (path) => callback!(null, encodeRealpathResult(path, options)),
    (err) => callback!(err),
  );
}

realpath.native = realpath;

const realpathPromise = promisify(realpath) as (
  path: string | Buffer,
  options?: RealpathOptions,
) => Promise<string | Buffer>;

function realpathSync(
  path: string,
  options?: RealpathOptions | RealpathEncoding,
): string | Buffer {
  options = getOptions(options) as RealpathEncodingObj;
  path = getValidatedPathToString(path);
  const result = Deno.realPathSync(path);
  return encodeRealpathResult(result, options);
}

realpathSync.native = realpathSync;

type ExistsCallback = (exists: boolean) => void;

function exists(path: string | Buffer | URL, callback: ExistsCallback) {
  callback = makeCallback(callback);

  try {
    path = getValidatedPathToString(path);
  } catch {
    callback(false);
    return;
  }

  PromisePrototypeThen(
    op_node_fs_exists(path),
    callback,
  );
}

ObjectDefineProperty(exists, kCustomPromisifiedSymbol, {
  __proto__: null,
  value: (path: string | URL) => {
    return new PrimPromise((resolve) => {
      exists(path, (exists) => resolve(exists));
    });
  },
  enumerable: false,
  writable: false,
  configurable: true,
});

let showExistsDeprecation = true;
function existsSync(path: string | Buffer | URL): boolean {
  try {
    path = getValidatedPathToString(path);
  } catch (err) {
    // @ts-expect-error `code` is safe to check with optional chaining
    if (showExistsDeprecation && err?.code === "ERR_INVALID_ARG_TYPE") {
      process.emitWarning(
        "Passing invalid argument types to fs.existsSync is deprecated",
        "DeprecationWarning",
        "DEP0187",
      );
      showExistsDeprecation = false;
    }
    return false;
  }
  return op_node_fs_exists_sync(path);
}

function copyFile(
  src: string | Buffer | URL,
  dest: string | Buffer | URL,
  mode: number | CallbackWithError,
  callback?: CallbackWithError,
) {
  if (typeof mode === "function") {
    callback = mode;
    mode = 0;
  }
  // deno-lint-ignore prefer-primordials
  const srcStr = getValidatedPath(src, "src").toString();
  // deno-lint-ignore prefer-primordials
  const destStr = getValidatedPath(dest, "dest").toString();
  const modeNum = getValidMode(mode, "copyFile");
  const cb = makeCallback(callback);

  if ((modeNum & fsConstants.COPYFILE_EXCL) === fsConstants.COPYFILE_EXCL) {
    // deno-lint-ignore prefer-primordials
    Deno.lstat(destStr).then(() => {
      // deno-lint-ignore no-explicit-any prefer-primordials
      const e: any = new Error(
        `EEXIST: file already exists, copyfile '${srcStr}' -> '${destStr}'`,
      );
      e.syscall = "copyfile";
      e.errno = codeMap.get("EEXIST");
      e.code = "EEXIST";
      cb(e);
    }, (e) => {
      // deno-lint-ignore prefer-primordials
      if (e instanceof Deno.errors.NotFound) {
        // deno-lint-ignore prefer-primordials
        Deno.copyFile(srcStr, destStr).then(() => cb(null), cb);
      } else {
        cb(e);
      }
    });
  } else {
    // deno-lint-ignore prefer-primordials
    Deno.copyFile(srcStr, destStr).then(() => cb(null), cb);
  }
}

const copyFilePromise = promisify(copyFile) as (
  src: string | Buffer | URL,
  dest: string | Buffer | URL,
  mode?: number,
) => Promise<void>;

function copyFileSync(
  src: string | Buffer | URL,
  dest: string | Buffer | URL,
  mode?: number,
) {
  // deno-lint-ignore prefer-primordials
  const srcStr = getValidatedPath(src, "src").toString();
  // deno-lint-ignore prefer-primordials
  const destStr = getValidatedPath(dest, "dest").toString();
  const modeNum = getValidMode(mode, "copyFile");

  if ((modeNum & fsConstants.COPYFILE_EXCL) === fsConstants.COPYFILE_EXCL) {
    try {
      Deno.lstatSync(destStr);
      // deno-lint-ignore prefer-primordials
      throw new Error(`A file exists at the destination: ${destStr}`);
    } catch (e) {
      // deno-lint-ignore prefer-primordials
      if (e instanceof Deno.errors.NotFound) {
        Deno.copyFileSync(srcStr, destStr);
      } else {
        throw e;
      }
    }
  } else {
    Deno.copyFileSync(srcStr, destStr);
  }
}

function chmod(
  path: string | Buffer | URL,
  mode: string | number,
  callback: CallbackWithError,
) {
  path = getValidatedPathToString(path);
  mode = parseFileMode(mode, "mode");

  PromisePrototypeThen(
    Deno.chmod(path, mode),
    () => callback(null),
    (err: Error) =>
      callback(denoErrorToNodeError(err, { syscall: "chmod", path })),
  );
}

const chmodPromise = promisify(chmod) as (
  path: string | Buffer | URL,
  mode: string | number,
) => Promise<void>;

function chmodSync(path: string | Buffer | URL, mode: string | number) {
  path = getValidatedPathToString(path);
  mode = parseFileMode(mode, "mode");

  try {
    Deno.chmodSync(path, mode);
  } catch (error) {
    throw denoErrorToNodeError(error as Error, { syscall: "chmod", path });
  }
}

function defaultCloseCallback(err: Error | null) {
  if (err !== null) throw err;
}

function close(
  fd: number,
  callback: CallbackWithError = defaultCloseCallback,
) {
  fd = getValidatedFd(fd);
  if (callback !== defaultCloseCallback) {
    callback = makeCallback(callback);
  }

  setTimeout(() => {
    let error = null;
    try {
      core.close(fd);
    } catch (err) {
      error = ObjectPrototypeIsPrototypeOf(ErrorPrototype, err)
        ? err as Error
        : new PrimError("[non-error thrown]");
    }
    callback(error);
  }, 0);
}

function closeSync(fd: number) {
  fd = getValidatedFd(fd);
  core.close(fd);
}

function fdatasync(
  fd: number,
  callback: CallbackWithError,
) {
  validateInt32(fd, "fd", 0);
  PromisePrototypeThen(
    new FsFile(fd, SymbolFor("Deno.internal.FsFile")).syncData(),
    () => callback(null),
    callback,
  );
}

function fdatasyncSync(fd: number) {
  validateInt32(fd, "fd", 0);
  new FsFile(fd, SymbolFor("Deno.internal.FsFile")).syncDataSync();
}

const fdatasyncPromise = promisify(fdatasync) as (
  fd: number,
) => Promise<void>;

function fsync(
  fd: number,
  callback: CallbackWithError,
) {
  validateInt32(fd, "fd", 0);
  PromisePrototypeThen(
    new FsFile(fd, SymbolFor("Deno.internal.FsFile")).sync(),
    () => callback(null),
    callback,
  );
}

function fsyncSync(fd: number) {
  validateInt32(fd, "fd", 0);
  new FsFile(fd, SymbolFor("Deno.internal.FsFile")).syncSync();
}

const fsyncPromise = promisify(fsync) as (fd: number) => Promise<void>;

function fchmod(
  fd: number,
  mode: string | number,
  callback: CallbackWithError,
) {
  validateInteger(fd, "fd", 0, 2147483647);
  mode = parseFileMode(mode, "mode");
  callback = makeCallback(callback);

  PromisePrototypeThen(
    op_fs_fchmod_async(fd, mode),
    () => callback(null),
    callback,
  );
}

function fchmodSync(fd: number, mode: string | number) {
  validateInteger(fd, "fd", 0, 2147483647);

  op_fs_fchmod_sync(fd, parseFileMode(mode, "mode"));
}

const fchmodPromise = promisify(fchmod) as (
  fd: number,
  mode: string | number,
) => Promise<void>;

function fchown(
  fd: number,
  uid: number,
  gid: number,
  callback: CallbackWithError,
) {
  validateInteger(fd, "fd", 0, 2147483647);
  validateInteger(uid, "uid", -1, kMaxUserId);
  validateInteger(gid, "gid", -1, kMaxUserId);
  callback = makeCallback(callback);

  PromisePrototypeThen(
    op_fs_fchown_async(fd, uid, gid),
    () => callback(null),
    callback,
  );
}

function fchownSync(
  fd: number,
  uid: number,
  gid: number,
) {
  validateInteger(fd, "fd", 0, 2147483647);
  validateInteger(uid, "uid", -1, kMaxUserId);
  validateInteger(gid, "gid", -1, kMaxUserId);

  op_fs_fchown_sync(fd, uid, gid);
}

const fchownPromise = promisify(fchown) as (
  fd: number,
  uid: number,
  gid: number,
) => Promise<void>;

function ftruncate(
  fd: number,
  lenOrCallback: number | CallbackWithError,
  maybeCallback?: CallbackWithError,
) {
  const len: number | undefined = typeof lenOrCallback === "number"
    ? lenOrCallback
    : undefined;
  const callback: CallbackWithError = typeof lenOrCallback === "function"
    ? lenOrCallback
    : (maybeCallback as CallbackWithError);

  if (!callback) throw new PrimError("No callback function supplied");

  PromisePrototypeThen(
    new FsFile(fd, SymbolFor("Deno.internal.FsFile")).truncate(len),
    () => callback(null),
    callback,
  );
}

function ftruncateSync(fd: number, len?: number) {
  new FsFile(fd, SymbolFor("Deno.internal.FsFile")).truncateSync(len);
}

const ftruncatePromise = promisify(ftruncate) as (
  fd: number,
  len?: number,
) => Promise<void>;

function getValidTime(
  time: number | string | Date,
  name: string,
): number | Date {
  if (typeof time === "string") {
    // deno-lint-ignore prefer-primordials
    time = Number(time);
  }

  if (
    typeof time === "number" &&
    // deno-lint-ignore prefer-primordials
    (Number.isNaN(time) || !Number.isFinite(time))
  ) {
    throw new Deno.errors.InvalidData(
      `invalid ${name}, must not be infinity or NaN`,
    );
  }

  return _toUnixTimestamp(time);
}

function futimes(
  fd: number,
  atime: number | string | Date,
  mtime: number | string | Date,
  callback: CallbackWithError,
) {
  if (!callback) {
    throw new Deno.errors.InvalidData("No callback function supplied");
  }
  if (typeof fd !== "number") {
    throw new ERR_INVALID_ARG_TYPE("fd", "number", fd);
  }

  validateInteger(fd, "fd", 0, 2147483647);

  atime = getValidTime(atime, "atime");
  mtime = getValidTime(mtime, "mtime");

  // deno-lint-ignore prefer-primordials
  new FsFile(fd, SymbolFor("Deno.internal.FsFile")).utime(atime, mtime).then(
    () => callback(null),
    callback,
  );
}

function futimesSync(
  fd: number,
  atime: number | string | Date,
  mtime: number | string | Date,
) {
  if (typeof fd !== "number") {
    throw new ERR_INVALID_ARG_TYPE("fd", "number", fd);
  }

  validateInteger(fd, "fd", 0, 2147483647);

  atime = getValidTime(atime, "atime");
  mtime = getValidTime(mtime, "mtime");

  new FsFile(fd, SymbolFor("Deno.internal.FsFile")).utimeSync(atime, mtime);
}

const futimesPromise = promisify(futimes) as (
  fd: number,
  atime: number | string | Date,
  mtime: number | string | Date,
) => Promise<void>;

function chown(
  path: string | Buffer | URL,
  uid: number,
  gid: number,
  callback: CallbackWithError,
) {
  callback = makeCallback(callback);
  // deno-lint-ignore prefer-primordials
  path = getValidatedPath(path).toString();
  validateInteger(uid, "uid", -1, kMaxUserId);
  validateInteger(gid, "gid", -1, kMaxUserId);

  // deno-lint-ignore prefer-primordials
  Deno.chown(path, uid, gid).then(
    () => callback(null),
    callback,
  );
}

const chownPromise = promisify(chown) as (
  path: string | Buffer | URL,
  uid: number,
  gid: number,
) => Promise<void>;

function chownSync(
  path: string | Buffer | URL,
  uid: number,
  gid: number,
) {
  // deno-lint-ignore prefer-primordials
  path = getValidatedPath(path).toString();
  validateInteger(uid, "uid", -1, kMaxUserId);
  validateInteger(gid, "gid", -1, kMaxUserId);

  Deno.chownSync(path, uid, gid);
}

const lchmod = !isMacOS ? undefined : (
  path: string | Buffer | URL,
  mode: number,
  callback: CallbackWithError,
) => {
  path = getValidatedPathToString(path);
  mode = parseFileMode(mode, "mode");
  callback = makeCallback(callback);

  PromisePrototypeThen(
    op_node_lchmod(path, mode),
    () => callback(null),
    (err) => callback(err),
  );
};

const lchmodPromise = !isMacOS
  ? () => PromiseReject(new ERR_METHOD_NOT_IMPLEMENTED("lchmod()"))
  : promisify(lchmod) as (
    path: string | Buffer | URL,
    mode: number,
  ) => Promise<void>;

const lchmodSync = !isMacOS
  ? undefined
  : (path: string | Buffer | URL, mode: number) => {
    path = getValidatedPathToString(path);
    mode = parseFileMode(mode, "mode");
    return op_node_lchmod_sync(path, mode);
  };

function lchown(
  path: string | Buffer | URL,
  uid: number,
  gid: number,
  callback: CallbackWithError,
) {
  callback = makeCallback(callback);
  path = getValidatedPathToString(path);
  validateInteger(uid, "uid", -1, kMaxUserId);
  validateInteger(gid, "gid", -1, kMaxUserId);

  PromisePrototypeThen(
    op_node_lchown(path, uid, gid),
    () => callback(null),
    callback,
  );
}

const lchownPromise = promisify(lchown) as (
  path: string | Buffer | URL,
  uid: number,
  gid: number,
) => Promise<void>;

function lchownSync(
  path: string | Buffer | URL,
  uid: number,
  gid: number,
) {
  path = getValidatedPathToString(path);
  validateInteger(uid, "uid", -1, kMaxUserId);
  validateInteger(gid, "gid", -1, kMaxUserId);

  op_node_lchown_sync(path, uid, gid);
}

type TimeLike = number | string | Date;
type PathLike = string | Buffer | URL;

function getValidUnixTime(
  value: TimeLike,
  name: string,
): [number, number] {
  if (typeof value === "string") {
    value = PrimNumber(value);
  }

  if (
    typeof value === "number" &&
    // deno-lint-ignore prefer-primordials
    (Number.isNaN(value) || !Number.isFinite(value))
  ) {
    throw new Deno.errors.InvalidData(
      `invalid ${name}, must not be infinity or NaN`,
    );
  }

  const unixSeconds = _toUnixTimestamp(value);

  const seconds = MathTrunc(unixSeconds);
  const nanoseconds = MathTrunc((unixSeconds * 1e3) - (seconds * 1e3)) * 1e6;

  return [
    seconds,
    nanoseconds,
  ];
}

function lutimes(
  path: PathLike,
  atime: TimeLike,
  mtime: TimeLike,
  callback: CallbackWithError,
): void {
  if (!callback) {
    throw new PrimError("No callback function supplied");
  }
  const { 0: atimeSecs, 1: atimeNanos } = getValidUnixTime(atime, "atime");
  const { 0: mtimeSecs, 1: mtimeNanos } = getValidUnixTime(mtime, "mtime");

  // deno-lint-ignore prefer-primordials
  path = getValidatedPath(path).toString();

  // deno-lint-ignore prefer-primordials
  op_node_lutimes(path, atimeSecs, atimeNanos, mtimeSecs, mtimeNanos).then(
    () => callback(null),
    callback,
  );
}

function lutimesSync(
  path: PathLike,
  atime: TimeLike,
  mtime: TimeLike,
): void {
  const { 0: atimeSecs, 1: atimeNanos } = getValidUnixTime(atime, "atime");
  const { 0: mtimeSecs, 1: mtimeNanos } = getValidUnixTime(mtime, "mtime");

  // deno-lint-ignore prefer-primordials
  path = getValidatedPath(path).toString();

  op_node_lutimes_sync(path, atimeSecs, atimeNanos, mtimeSecs, mtimeNanos);
}

const lutimesPromise = promisify(lutimes) as (
  path: PathLike,
  atime: TimeLike,
  mtime: TimeLike,
) => Promise<void>;

function access(
  path: string | Buffer | URL,
  mode: number | CallbackWithError,
  callback?: CallbackWithError,
) {
  if (typeof mode === "function") {
    callback = mode;
    mode = fsConstants.F_OK;
  }

  // deno-lint-ignore prefer-primordials
  path = getValidatedPath(path).toString();
  mode = getValidMode(mode, "access");
  const cb = makeCallback(callback);

  // deno-lint-ignore prefer-primordials
  Deno.lstat(path).then(
    (info) => {
      if (info.mode === null) {
        cb(null);
        return;
      }
      let m = +mode || 0;
      let fileMode = +info.mode || 0;

      if (Deno.build.os === "windows") {
        m &= ~fsConstants.X_OK;
      } else if (info.uid === Deno.uid()) {
        fileMode >>= 6;
      }

      if ((m & fileMode) === m) {
        cb(null);
      } else {
        // deno-lint-ignore no-explicit-any prefer-primordials
        const e: any = new Error(`EACCES: permission denied, access '${path}'`);
        e.path = path;
        e.syscall = "access";
        e.errno = codeMap.get("EACCES");
        e.code = "EACCES";
        cb(e);
      }
    },
    (err) => {
      // deno-lint-ignore prefer-primordials
      if (err instanceof Deno.errors.NotFound) {
        // deno-lint-ignore no-explicit-any prefer-primordials
        const e: any = new Error(
          `ENOENT: no such file or directory, access '${path}'`,
        );
        e.path = path;
        e.syscall = "access";
        e.errno = codeMap.get("ENOENT");
        e.code = "ENOENT";
        cb(e);
      } else {
        cb(err);
      }
    },
  );
}

const accessPromise = promisify(access) as (
  path: string | Buffer | URL,
  mode?: number,
) => Promise<void>;

function accessSync(path: string | Buffer | URL, mode?: number) {
  // deno-lint-ignore prefer-primordials
  path = getValidatedPath(path).toString();
  mode = getValidMode(mode, "access");
  try {
    // deno-lint-ignore prefer-primordials
    const info = Deno.lstatSync(path.toString());
    if (info.mode === null) {
      return;
    }
    let m = +mode! || 0;
    let fileMode = +info.mode! || 0;
    if (Deno.build.os === "windows") {
      m &= ~fsConstants.X_OK;
    } else if (info.uid === Deno.uid()) {
      fileMode >>= 6;
    }
    if ((m & fileMode) === m) {
      // all required flags exist
    } else {
      // deno-lint-ignore no-explicit-any prefer-primordials
      const e: any = new Error(`EACCES: permission denied, access '${path}'`);
      e.path = path;
      e.syscall = "access";
      e.errno = codeMap.get("EACCES");
      e.code = "EACCES";
      throw e;
    }
  } catch (err) {
    // deno-lint-ignore prefer-primordials
    if (err instanceof Deno.errors.NotFound) {
      // deno-lint-ignore no-explicit-any prefer-primordials
      const e: any = new Error(
        `ENOENT: no such file or directory, access '${path}'`,
      );
      e.path = path;
      e.syscall = "access";
      e.errno = codeMap.get("ENOENT");
      e.code = "ENOENT";
      throw e;
    } else {
      throw err;
    }
  }
}

function cpSync(
  src: string | URL,
  dest: string | URL,
  options: CopySyncOptions,
) {
  options = validateCpOptions(options);
  const srcPath = getValidatedPathToString(src, "src");
  const destPath = getValidatedPathToString(dest, "dest");

  cpSyncFn(srcPath, destPath, options);
}

function cp(
  src: string | URL,
  dest: string | URL,
  options: CopyOptions | undefined,
  callback: CallbackWithError,
) {
  if (typeof options === "function") {
    callback = options;
    options = undefined;
  }
  callback = makeCallback(callback);
  options = validateCpOptions(options);
  const srcPath = getValidatedPathToString(src, "src");
  const destPath = getValidatedPathToString(dest, "dest");

  PromisePrototypeThen(
    cpFn(srcPath, destPath, options),
    () => callback(null),
    callback,
  );
}

async function cpPromise(
  src: string | URL,
  dest: string | URL,
  options?: CopyOptions,
): Promise<void> {
  options = validateCpOptions(options);
  const srcPath = getValidatedPathToString(src, "src");
  const destPath = getValidatedPathToString(dest, "dest");
  return await cpFn(srcPath, destPath, options);
}

function appendFile(
  path: string | number | URL,
  data: string | Uint8Array,
  options: Encodings | WriteFileOptions | CallbackWithError,
  callback?: CallbackWithError,
) {
  callback = maybeCallback(callback || options);
  options = getOptions(options, { encoding: "utf8", mode: 0o666, flag: "a" });

  options = copyObject(options);

  if (!options.flag || isFd(path)) {
    options.flag = "a";
  }

  writeFile(path, data, options, callback);
}

const appendFilePromise = promisify(appendFile) as (
  path: string | number | URL,
  data: string | Uint8Array,
  options?: Encodings | WriteFileOptions,
) => Promise<void>;

function appendFileSync(
  path: string | number | URL,
  data: string | Uint8Array,
  options?: Encodings | WriteFileOptions,
) {
  options = getOptions(options, { encoding: "utf8", mode: 0o666, flag: "a" });

  options = copyObject(options);

  if (!options.flag || isFd(path)) {
    options.flag = "a";
  }

  writeFileSync(path, data, options);
}

type MkdtempCallback = (
  err: Error | null,
  directory?: string,
) => void;
type MkdtempBufferCallback = (
  err: Error | null,
  directory?: Buffer<ArrayBufferLike>,
) => void;
type MkdTempPromise = (
  prefix: string | Buffer | Uint8Array | URL,
  options?: { encoding: string } | string,
) => Promise<string>;
type MkdTempPromiseBuffer = (
  prefix: string | Buffer | Uint8Array | URL,
  options: { encoding: "buffer" } | "buffer",
) => Promise<Buffer<ArrayBufferLike>>;

function mkdtemp(
  prefix: string | Buffer | Uint8Array | URL,
  callback: MkdtempCallback,
): void;
function mkdtemp(
  prefix: string | Buffer | Uint8Array | URL,
  options: { encoding: "buffer" } | "buffer",
  callback: MkdtempBufferCallback,
): void;
function mkdtemp(
  prefix: string | Buffer | Uint8Array | URL,
  options: { encoding: string } | string,
  callback: MkdtempCallback,
): void;
function mkdtemp(
  prefix: string | Buffer | Uint8Array | URL,
  options: { encoding: string } | string | MkdtempCallback | undefined,
  callback?: MkdtempCallback | MkdtempBufferCallback,
) {
  if (typeof options === "function") {
    callback = options;
    options = undefined;
  }
  callback = makeCallback(callback);
  const encoding = parseMkdtempEncoding(options);
  prefix = getValidatedPathToString(prefix, "prefix");

  warnOnNonPortableTemplate(prefix);

  PromisePrototypeThen(
    op_node_mkdtemp(prefix),
    (path: string) => callback(null, decodeMkdtemp(path, encoding)),
    (err: Error) =>
      callback(denoErrorToNodeError(err, {
        syscall: "mkdtemp",
        path: `${prefix}XXXXXX`,
      })),
  );
}

const mkdtempPromise = promisify(mkdtemp) as
  | MkdTempPromise
  | MkdTempPromiseBuffer;

function mkdtempSync(
  prefix: string | Buffer | Uint8Array | URL,
  options?: { encoding: "buffer" } | "buffer",
): Buffer<ArrayBufferLike>;
function mkdtempSync(
  prefix: string | Buffer | Uint8Array | URL,
  options?: { encoding: string } | string,
): string;
function mkdtempSync(
  prefix: string | Buffer | Uint8Array | URL,
  options?: { encoding: string } | string,
): string | Buffer<ArrayBufferLike> {
  const encoding = parseMkdtempEncoding(options);
  prefix = getValidatedPathToString(prefix, "prefix");

  warnOnNonPortableTemplate(prefix);

  try {
    const path = op_node_mkdtemp_sync(prefix) as string;
    return decodeMkdtemp(path, encoding);
  } catch (err) {
    throw denoErrorToNodeError(err as Error, {
      syscall: "mkdtemp",
      path: `${prefix}XXXXXX`,
    });
  }
}

function decodeMkdtemp(str: string, encoding: Encoding): string;
function decodeMkdtemp(
  str: string,
  encoding: "buffer",
): Buffer<ArrayBufferLike>;
function decodeMkdtemp(
  str: string,
  encoding: Encoding | "buffer",
): string | Buffer<ArrayBufferLike> {
  if (encoding === "utf8") return str;
  const buffer = Buffer.from(str);
  if (encoding === "buffer") return buffer;
  // deno-lint-ignore prefer-primordials
  return buffer.toString(encoding);
}

function parseMkdtempEncoding(
  options: string | { encoding?: string } | undefined,
): Encoding | "buffer" {
  let encoding: string | undefined;

  if (typeof options === "undefined" || options === null) {
    encoding = "utf8";
  } else if (typeof options === "string") {
    encoding = options;
  } else if (typeof options === "object") {
    encoding = options.encoding ?? "utf8";
  } else {
    throw new ERR_INVALID_ARG_TYPE("options", ["string", "Object"], options);
  }

  if (encoding === "buffer") {
    return encoding;
  }

  const parsedEncoding = normalizeEncoding(encoding);
  if (!parsedEncoding) {
    throw new ERR_INVALID_ARG_TYPE("encoding", encoding, "is invalid encoding");
  }

  return parsedEncoding;
}

type StatFsPromise = (
  path: string | Buffer | URL,
  options?: { bigint?: false },
) => Promise<StatFs<number>>;
type StatFsBigIntPromise = (
  path: string | Buffer | URL,
  options: { bigint: true },
) => Promise<StatFs<bigint>>;

type StatFsCallback<T> = (err: Error | null, stats?: StatFs<T>) => void;

type StatFsOptions = {
  bigint?: boolean;
};

class StatFs<T> {
  type: T;
  bsize: T;
  blocks: T;
  bfree: T;
  bavail: T;
  files: T;
  ffree: T;
  constructor(
    type: T,
    bsize: T,
    blocks: T,
    bfree: T,
    bavail: T,
    files: T,
    ffree: T,
  ) {
    this.type = type;
    this.bsize = bsize;
    this.blocks = blocks;
    this.bfree = bfree;
    this.bavail = bavail;
    this.files = files;
    this.ffree = ffree;
  }
}

type StatFsOpResult = {
  type: number;
  bsize: number;
  blocks: number;
  bfree: number;
  bavail: number;
  files: number;
  ffree: number;
};

function opResultToStatFs(result: StatFsOpResult, bigint: true): StatFs<bigint>;
function opResultToStatFs(
  result: StatFsOpResult,
  bigint: false,
): StatFs<number>;
function opResultToStatFs(
  result: StatFsOpResult,
  bigint: boolean,
): StatFs<bigint> | StatFs<number> {
  if (!bigint) {
    return new StatFs(
      result.type,
      result.bsize,
      result.blocks,
      result.bfree,
      result.bavail,
      result.files,
      result.ffree,
    );
  }
  return new StatFs(
    BigInt(result.type),
    BigInt(result.bsize),
    BigInt(result.blocks),
    BigInt(result.bfree),
    BigInt(result.bavail),
    BigInt(result.files),
    BigInt(result.ffree),
  );
}

function statfs(
  path: string | Buffer | URL,
  callback: StatFsCallback<number>,
): void;
function statfs(
  path: string | Buffer | URL,
  options: { bigint?: false },
  callback: StatFsCallback<number>,
): void;
function statfs(
  path: string | Buffer | URL,
  options: { bigint: true },
  callback: StatFsCallback<bigint>,
): void;
function statfs(
  path: string | Buffer | URL,
  options: StatFsOptions | StatFsCallback<number> | undefined,
  callback?: StatFsCallback<number> | StatFsCallback<bigint>,
): void {
  if (typeof options === "function") {
    callback = options;
    options = undefined;
  }
  // @ts-expect-error callback type is known to be valid
  callback = makeCallback(callback);
  path = getValidatedPathToString(path);
  const bigint = typeof options?.bigint === "boolean" ? options.bigint : false;

  PromisePrototypeThen(
    op_node_statfs(path, bigint),
    (statFs) => {
      callback(
        null,
        opResultToStatFs(statFs, bigint),
      );
    },
    (err: Error) =>
      callback(denoErrorToNodeError(err, {
        syscall: "statfs",
        path,
      })),
  );
}

function statfsSync(
  path: string | Buffer | URL,
  options?: { bigint?: false },
): StatFs<number>;
function statfsSync(
  path: string | Buffer | URL,
  options: { bigint: true },
): StatFs<bigint>;
function statfsSync(
  path: string | Buffer | URL,
  options?: StatFsOptions,
): StatFs<number> | StatFs<bigint> {
  path = getValidatedPathToString(path);
  const bigint = typeof options?.bigint === "boolean" ? options.bigint : false;

  try {
    const result = op_node_statfs_sync(
      path,
      bigint,
    );
    return opResultToStatFs(result, bigint);
  } catch (err) {
    throw denoErrorToNodeError(err as Error, {
      syscall: "statfs",
      path,
    });
  }
}

const statfsPromise = promisify(statfs) as (
  StatFsPromise & StatFsBigIntPromise
);

type rmdirOptions = {
  maxRetries?: number;
  recursive?: boolean;
  retryDelay?: number;
};

type rmdirCallback = (err?: Error) => void;

const rmdirRecursive =
  (path: string, callback: rmdirCallback) =>
  (err: Error | false | null, options?: RmOptions) => {
    if (err === false) {
      return callback(new ERR_FS_RMDIR_ENOTDIR(path));
    }
    if (err) {
      return callback(err);
    }

    PromisePrototypeThen(
      Deno.remove(path, { recursive: options?.recursive }),
      (_) => callback(),
      (err: Error) =>
        callback(
          denoErrorToNodeError(err, { syscall: "rmdir", path }),
        ),
    );
  };

function rmdir(
  path: string | Buffer | URL,
  callback: rmdirCallback,
): void;
function rmdir(
  path: string | Buffer | URL,
  options: rmdirOptions,
  callback: rmdirCallback,
): void;
function rmdir(
  path: string | Buffer | URL,
  options: rmdirOptions | rmdirCallback | undefined,
  callback?: rmdirCallback,
) {
  if (typeof options === "function") {
    callback = options;
    options = undefined;
  }
  validateFunction(callback, "cb");
  path = getValidatedPathToString(path);

  if (options?.recursive) {
    emitRecursiveRmdirWarning();
    validateRmOptions(
      path,
      { ...options, force: false },
      true,
      rmdirRecursive(path, callback),
    );
  } else {
    validateRmdirOptions(options);
    PromisePrototypeThen(
      op_node_rmdir(path),
      (_) => callback(),
      (err: Error) =>
        callback(
          denoErrorToNodeError(err, { syscall: "rmdir", path }),
        ),
    );
  }
}

const rmdirPromise = promisify(rmdir) as (
  path: string | Buffer | URL,
  options?: rmdirOptions,
) => Promise<void>;

function rmdirSync(path: string | Buffer | URL, options?: rmdirOptions) {
  path = getValidatedPathToString(path);
  if (options?.recursive) {
    emitRecursiveRmdirWarning();
    const optionsOrFalse = validateRmOptionsSync(path, {
      ...options,
      force: false,
    }, true);
    if (optionsOrFalse === false) {
      throw new ERR_FS_RMDIR_ENOTDIR(path);
    }
    return Deno.removeSync(path, {
      recursive: true,
    });
  }

  validateRmdirOptions(options);
  try {
    op_node_rmdir_sync(path);
  } catch (err) {
    throw (denoErrorToNodeError(err as Error, { syscall: "rmdir", path }));
  }
}

type rmOptions = {
  force?: boolean;
  maxRetries?: number;
  recursive?: boolean;
  retryDelay?: number;
};

type rmCallback = (err: Error | null) => void;

function rm(path: string | URL, callback: rmCallback): void;
function rm(
  path: string | URL,
  options: rmOptions,
  callback: rmCallback,
): void;
function rm(
  path: string | URL,
  optionsOrCallback: rmOptions | rmCallback,
  maybeCallback?: rmCallback,
) {
  const callback = typeof optionsOrCallback === "function"
    ? optionsOrCallback
    : maybeCallback;
  const options = typeof optionsOrCallback === "object"
    ? optionsOrCallback
    : undefined;

  if (!callback) throw new PrimError("No callback function supplied");

  validateRmOptions(
    path,
    options,
    false,
    (err: Error | null, options: rmOptions) => {
      if (err) {
        return callback(err);
      }

      PromisePrototypeThen(
        Deno.remove(path, { recursive: options?.recursive }),
        () => callback(null),
        (err) => {
          if (
            options?.force &&
            ObjectPrototypeIsPrototypeOf(Deno.errors.NotFound.prototype, err)
          ) {
            return callback(null);
          }

          callback(denoErrorToNodeError(err, { syscall: "rm" }));
        },
      );
    },
  );
}

const rmPromise = promisify(rm) as (
  path: string | URL,
  options?: rmOptions,
) => Promise<void>;

function rmSync(path: string | URL, options?: rmOptions) {
  options = validateRmOptionsSync(path, options, false);
  try {
    Deno.removeSync(path, { recursive: options?.recursive });
  } catch (err: unknown) {
    if (
      options?.force &&
      ObjectPrototypeIsPrototypeOf(Deno.errors.NotFound.prototype, err)
    ) {
      return;
    }
    throw denoErrorToNodeError(err, { syscall: "rm" });
  }
}

type ReadlinkCallback = (
  err: MaybeEmpty<Error>,
  linkString: MaybeEmpty<string | Uint8Array>,
) => void;

interface ReadlinkOptions {
  encoding?: string | null;
}

function readlinkMaybeEncode(
  data: string,
  encoding: string | null,
): string | Uint8Array {
  if (encoding === "buffer") {
    return new TextEncoder().encode(data);
  }
  return data;
}

function readlinkGetEncoding(
  optOrCallback?: ReadlinkOptions | ReadlinkCallback,
): string | null {
  if (!optOrCallback || typeof optOrCallback === "function") {
    return null;
  } else {
    if (optOrCallback.encoding) {
      if (
        optOrCallback.encoding === "utf8" ||
        optOrCallback.encoding === "utf-8"
      ) {
        return "utf8";
      } else if (optOrCallback.encoding === "buffer") {
        return "buffer";
      } else {
        notImplemented(`fs.readlink encoding=${optOrCallback.encoding}`);
      }
    }
    return null;
  }
}

function readlink(
  path: string | Buffer | URL,
  optOrCallback: ReadlinkCallback | ReadlinkOptions,
  callback?: ReadlinkCallback,
) {
  path = getValidatedPathToString(path);

  let cb: ReadlinkCallback | undefined;
  if (typeof optOrCallback === "function") {
    cb = optOrCallback;
  } else {
    cb = callback;
  }
  cb = makeCallback(cb);

  const encoding = readlinkGetEncoding(optOrCallback);

  // deno-lint-ignore prefer-primordials
  Deno.readLink(path).then((data: string) => {
    const res = readlinkMaybeEncode(data, encoding);
    if (cb) cb(null, res);
  }, (err: Error) => {
    if (cb) {
      (cb as (e: Error) => void)(denoErrorToNodeError(err, {
        syscall: "readlink",
        path,
      }));
    }
  });
}

const readlinkPromise = promisify(readlink) as (
  path: string | Buffer | URL,
  opt?: ReadlinkOptions,
) => Promise<string | Uint8Array>;

function readlinkSync(
  path: string | Buffer | URL,
  opt?: ReadlinkOptions,
): string | Uint8Array {
  path = getValidatedPathToString(path);

  try {
    return readlinkMaybeEncode(
      Deno.readLinkSync(path),
      readlinkGetEncoding(opt),
    );
  } catch (error) {
    throw denoErrorToNodeError(error, {
      syscall: "readlink",
      path,
    });
  }
}

type MkdirCallback =
  | ((err: Error | null, path?: string) => void)
  | CallbackWithError;

function fixMkdirError(
  err: Error,
  path: string,
): Error {
  const nodeErr = denoErrorToNodeError(err, { syscall: "mkdir", path });
  if (!isWindows) return nodeErr;
  if ((nodeErr as NodeJS.ErrnoException).code !== "EEXIST") return nodeErr;
  let cursor = pathModule.resolve(path, "..");
  while (true) {
    try {
      const st = Deno.statSync(cursor);
      if (!st.isDirectory) {
        return uvException({
          errno: codeMap.get("ENOTDIR")!,
          syscall: "mkdir",
          path,
        });
      }
      break;
    } catch {
      const parent = pathModule.resolve(cursor, "..");
      if (parent === cursor) break;
      cursor = parent;
    }
  }
  return nodeErr;
}

function findFirstNonExistent(path: string): string | undefined {
  let cursor = pathModule.resolve(path);
  while (true) {
    try {
      Deno.statSync(cursor);
      return undefined;
    } catch {
      const parent = pathModule.resolve(cursor, "..");
      if (parent === cursor) {
        return pathModule.toNamespacedPath(cursor);
      }
      try {
        Deno.statSync(parent);
        return pathModule.toNamespacedPath(cursor);
      } catch {
        cursor = parent;
      }
    }
  }
}

type MkdirOptions =
  | { recursive?: boolean; mode?: number | undefined }
  | number
  | boolean;

function mkdir(
  path: string | URL,
  options?: MkdirOptions | MkdirCallback,
  callback?: MkdirCallback,
) {
  path = getValidatedPath(path) as string;

  let mode = 0o777;
  let recursive = false;

  if (typeof options == "function") {
    callback = options;
  } else if (typeof options === "number") {
    mode = parseFileMode(options, "mode");
  } else if (typeof options === "boolean") {
    recursive = options;
  } else if (options) {
    if (options.recursive !== undefined) recursive = options.recursive;
    if (options.mode !== undefined) {
      mode = parseFileMode(options.mode, "options.mode");
    }
  }
  validateBoolean(recursive, "options.recursive");

  let firstNonExistent: string | undefined;
  try {
    firstNonExistent = recursive ? findFirstNonExistent(path) : undefined;
  } catch (err) {
    if (typeof callback === "function") {
      callback(
        denoErrorToNodeError(err as Error, { syscall: "mkdir", path }),
      );
    }
    return;
  }

  // deno-lint-ignore prefer-primordials
  Deno.mkdir(path, { recursive, mode })
    .then(() => {
      if (typeof callback === "function") {
        callback(null, firstNonExistent);
      }
    }, (err) => {
      if (typeof callback === "function") {
        callback(
          recursive
            ? fixMkdirError(err as Error, path as string)
            : denoErrorToNodeError(err as Error, { syscall: "mkdir", path }),
        );
      }
    });
}

const mkdirPromise = promisify(mkdir) as (
  path: string | URL,
  options?: MkdirOptions,
) => Promise<string | undefined>;

function mkdirSync(
  path: string | URL,
  options?: MkdirOptions,
): string | undefined {
  path = getValidatedPath(path) as string;

  let mode = 0o777;
  let recursive = false;

  if (typeof options === "number") {
    mode = parseFileMode(options, "mode");
  } else if (typeof options === "boolean") {
    recursive = options;
  } else if (options) {
    if (options.recursive !== undefined) recursive = options.recursive;
    if (options.mode !== undefined) {
      mode = parseFileMode(options.mode, "options.mode");
    }
  }
  validateBoolean(recursive, "options.recursive");

  let firstNonExistent: string | undefined;
  try {
    firstNonExistent = recursive ? findFirstNonExistent(path) : undefined;
    Deno.mkdirSync(path, { recursive, mode });
  } catch (err) {
    throw recursive
      ? fixMkdirError(err as Error, path)
      : denoErrorToNodeError(err as Error, { syscall: "mkdir", path });
  }

  return firstNonExistent;
}

type SymlinkType = "file" | "dir" | "junction";

function symlink(
  target: string | Buffer | URL,
  path: string | Buffer | URL,
  linkType?: SymlinkType | CallbackWithError,
  callback?: CallbackWithError,
) {
  if (callback === undefined) {
    callback = linkType as CallbackWithError;
    linkType = undefined;
  } else {
    validateOneOf(linkType, "type", [
      "dir",
      "file",
      "junction",
      null,
      undefined,
    ]);
  }

  callback = makeCallback(callback);
  target = getValidatedPathToString(target, "target");
  path = getValidatedPathToString(path);

  if (isWindows && !linkType) {
    let absoluteTarget;
    try {
      absoluteTarget = pathModule.resolve(path, "..", target);
    } catch {
      // Continue regardless of error.
    }
    if (absoluteTarget !== undefined) {
      stat(absoluteTarget, (err, stat) => {
        const resolvedType = !err && stat.isDirectory() ? "dir" : "file";

        PromisePrototypeThen(
          Deno.symlink(
            target,
            path,
            { type: resolvedType },
          ),
          () => callback(null),
          callback,
        );
      });
      return;
    }
  }

  PromisePrototypeThen(
    Deno.symlink(
      target,
      path,
      { type: linkType ?? "file" },
    ),
    () => callback(null),
    callback,
  );
}

const symlinkPromise = promisify(symlink) as (
  target: string | Buffer | URL,
  path: string | Buffer | URL,
  type?: SymlinkType,
) => Promise<void>;

function symlinkSync(
  target: string | Buffer | URL,
  path: string | Buffer | URL,
  type?: SymlinkType,
) {
  validateOneOf(type, "type", ["dir", "file", "junction", null, undefined]);
  target = getValidatedPathToString(target, "target");
  path = getValidatedPathToString(path);

  if (isWindows && !type) {
    const absoluteTarget = pathModule.resolve(path, "..", target);
    if (
      statSync(absoluteTarget, { bigint: false, throwIfNoEntry: false })
        ?.isDirectory()
    ) {
      type = "dir";
    }
  }

  Deno.symlinkSync(target, path, { type: type ?? "file" });
}

function utimes(
  path: string | URL,
  atime: number | string | Date,
  mtime: number | string | Date,
  callback: CallbackWithError,
) {
  // deno-lint-ignore prefer-primordials
  path = getValidatedPath(path).toString();

  if (!callback) {
    throw new Deno.errors.InvalidData("No callback function supplied");
  }

  atime = getValidTime(atime, "atime");
  mtime = getValidTime(mtime, "mtime");

  // deno-lint-ignore prefer-primordials
  Deno.utime(path, atime, mtime).then(() => callback(null), callback);
}

const utimesPromise = promisify(utimes) as (
  path: string | URL,
  atime: number | string | Date,
  mtime: number | string | Date,
) => Promise<void>;

function utimesSync(
  path: string | URL,
  atime: number | string | Date,
  mtime: number | string | Date,
) {
  // deno-lint-ignore prefer-primordials
  path = getValidatedPath(path).toString();
  atime = getValidTime(atime, "atime");
  mtime = getValidTime(mtime, "mtime");

  Deno.utimeSync(path, atime, mtime);
}

function unlink(
  path: string | Buffer | URL,
  callback: (err?: Error) => void,
): void {
  path = getValidatedPathToString(path);
  validateFunction(callback, "callback");

  PromisePrototypeThen(
    Deno.remove(path),
    () => callback(),
    (err: Error) =>
      callback(denoErrorToNodeError(err, { syscall: "unlink", path })),
  );
}

const unlinkPromise = promisify(unlink) as (
  path: string | Buffer | URL,
) => Promise<void>;

function unlinkSync(path: string | Buffer | URL): void {
  path = getValidatedPathToString(path);
  try {
    Deno.removeSync(path);
  } catch (err) {
    throw denoErrorToNodeError(err as Error, { syscall: "unlink", path });
  }
}

function truncate(
  path: string | URL,
  lenOrCallback: number | CallbackWithError,
  maybeCallback?: CallbackWithError,
) {
  path = ObjectPrototypeIsPrototypeOf(URL, path) ? pathFromURL(path) : path;
  const len: number | undefined = typeof lenOrCallback === "number"
    ? lenOrCallback
    : undefined;
  const callback: CallbackWithError = typeof lenOrCallback === "function"
    ? lenOrCallback
    : maybeCallback as CallbackWithError;

  if (!callback) throw new PrimError("No callback function supplied");

  PromisePrototypeThen(
    Deno.truncate(path, len),
    () => callback(null),
    callback,
  );
}

const truncatePromise = promisify(truncate) as (
  path: string | URL,
  len?: number,
) => Promise<void>;

function truncateSync(path: string | URL, len?: number) {
  path = ObjectPrototypeIsPrototypeOf(URL, path) ? pathFromURL(path) : path;

  Deno.truncateSync(path, len);
}

/**
 * Returns a `Blob` whose data is read from the given file.
 */
function openAsBlob(
  path: string | Buffer | URL,
  options: { type?: string } = { __proto__: null },
): Promise<Blob> {
  validateObject(options, "options");
  const type = options.type || "";
  validateString(type, "options.type");
  path = getValidatedPath(path);
  return PromisePrototypeThen(
    op_fs_read_file_async(path as string, undefined, 0),
    (data: Uint8Array) => new Blob([data], { type }),
  );
}

const promises = {
  access: accessPromise,
  constants,
  copyFile: copyFilePromise,
  cp: cpPromise,
  glob: globPromise,
  open: openPromise,
  opendir: opendirPromise,
  rename: renamePromise,
  truncate: truncatePromise,
  rm: rmPromise,
  rmdir: rmdirPromise,
  mkdir: mkdirPromise,
  readdir: readdirPromise,
  readlink: readlinkPromise,
  symlink: symlinkPromise,
  lstat: lstatPromise,
  stat: statPromise,
  statfs: statfsPromise,
  link: linkPromise,
  unlink: unlinkPromise,
  chmod: chmodPromise,
  lchmod: lchmodPromise,
  lchown: lchownPromise,
  chown: chownPromise,
  utimes: utimesPromise,
  lutimes: lutimesPromise,
  realpath: realpathPromise,
  mkdtemp: mkdtempPromise,
  writeFile: writeFilePromise,
  appendFile: appendFilePromise,
  readFile: readFilePromise,
  watch: watchPromise,
};

export default {
  access,
  accessSync,
  appendFile,
  appendFileSync,
  chmod,
  chmodPromise,
  chmodSync,
  chown,
  chownSync,
  close,
  closeSync,
  constants,
  copyFile,
  copyFilePromise,
  copyFileSync,
  cp,
  cpSync,
  createReadStream,
  createWriteStream,
  Dir,
  Dirent,
  exists,
  existsSync,
  F_OK,
  fchmod,
  fchmodPromise,
  fchmodSync,
  fchown,
  fchownPromise,
  fchownSync,
  fdatasync,
  fdatasyncPromise,
  fdatasyncSync,
  fstat,
  fstatSync,
  fsync,
  fsyncPromise,
  fsyncSync,
  ftruncate,
  ftruncatePromise,
  ftruncateSync,
  futimes,
  futimesPromise,
  futimesSync,
  glob,
  globSync,
  lchmod,
  lchmodSync,
  lchown,
  lchownSync,
  link,
  linkSync,
  lstat,
  lstatSync,
  lutimes,
  lutimesSync,
  mkdir,
  mkdirPromise,
  mkdirSync,
  mkdtemp,
  mkdtempSync,
  O_APPEND,
  O_CREAT,
  O_DIRECTORY,
  O_DSYNC,
  O_EXCL,
  O_NOCTTY,
  O_NOFOLLOW,
  O_NONBLOCK,
  O_RDONLY,
  O_RDWR,
  O_SYMLINK,
  O_SYNC,
  O_TRUNC,
  O_WRONLY,
  open,
  openAsBlob,
  openSync,
  opendir,
  opendirSync,
  read,
  readSync,
  promises,
  R_OK,
  readdir,
  readdirSync,
  readFile,
  readFileSync,
  readlink,
  readlinkPromise,
  readlinkSync,
  ReadStream,
  realpath,
  realpathSync,
  readv,
  readvSync,
  rename,
  renameSync,
  rmdir,
  rmdirSync,
  rm,
  rmSync,
  stat,
  Stats,
  statSync,
  statfs,
  statfsSync,
  symlink,
  symlinkPromise,
  symlinkSync,
  truncate,
  truncateSync,
  unlink,
  unlinkPromise,
  unlinkSync,
  unwatchFile,
  utimes,
  utimesPromise,
  utimesSync,
  W_OK,
  watch,
  watchFile,
  write,
  writeFile,
  writev,
  writevSync,
  writeFileSync,
  WriteStream,
  writeSync,
  X_OK,
  // For tests
  _toUnixTimestamp,
};

export {
  // For tests
  _toUnixTimestamp,
  access,
  accessSync,
  appendFile,
  appendFileSync,
  chmod,
  chmodPromise,
  chmodSync,
  chown,
  chownSync,
  close,
  closeSync,
  constants,
  copyFile,
  copyFilePromise,
  copyFileSync,
  cp,
  cpSync,
  createReadStream,
  createWriteStream,
  Dir,
  Dirent,
  exists,
  existsSync,
  F_OK,
  fchmod,
  fchmodPromise,
  fchmodSync,
  fchown,
  fchownPromise,
  fchownSync,
  fdatasync,
  fdatasyncPromise,
  fdatasyncSync,
  fstat,
  fstatSync,
  fsync,
  fsyncPromise,
  fsyncSync,
  ftruncate,
  ftruncatePromise,
  ftruncateSync,
  futimes,
  futimesPromise,
  futimesSync,
  glob,
  globSync,
  lchmod,
  lchmodSync,
  link,
  linkSync,
  lstat,
  lstatSync,
  lutimes,
  lutimesSync,
  mkdir,
  mkdirPromise,
  mkdirSync,
  mkdtemp,
  mkdtempSync,
  O_APPEND,
  O_CREAT,
  O_DIRECTORY,
  O_DSYNC,
  O_EXCL,
  O_NOCTTY,
  O_NOFOLLOW,
  O_NONBLOCK,
  O_RDONLY,
  O_RDWR,
  O_SYMLINK,
  O_SYNC,
  O_TRUNC,
  O_WRONLY,
  open,
  openAsBlob,
  opendir,
  opendirSync,
  openSync,
  promises,
  R_OK,
  read,
  readdir,
  readdirSync,
  readFile,
  readFileSync,
  readlink,
  readlinkPromise,
  readlinkSync,
  ReadStream,
  readSync,
  readv,
  readvSync,
  realpath,
  realpathSync,
  rename,
  renameSync,
  rm,
  rmdir,
  rmdirSync,
  rmSync,
  stat,
  statfs,
  statfsSync,
  Stats,
  statSync,
  symlink,
  symlinkPromise,
  symlinkSync,
  truncate,
  truncateSync,
  unlink,
  unlinkPromise,
  unlinkSync,
  unwatchFile,
  utimes,
  utimesPromise,
  utimesSync,
  W_OK,
  watch,
  watchFile,
  write,
  writeFile,
  writeFileSync,
  WriteStream,
  writeSync,
  writev,
  writevSync,
  X_OK,
};
