// Copyright 2018-2026 the Deno authors. MIT license.
import { fs as fsConstants } from "ext:deno_node/internal_binding/constants.ts";
import { codeMap } from "ext:deno_node/internal_binding/uv.ts";
import {
  type CallbackWithError,
  isFd,
  makeCallback,
  maybeCallback,
  type WriteFileOptions,
} from "ext:deno_node/_fs/_fs_common.ts";
import type { Encodings } from "ext:deno_node/_utils.ts";
import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";
import * as constants from "ext:deno_node/_fs/_fs_constants.ts";

import { copyFile, copyFileSync } from "ext:deno_node/_fs/_fs_copy.ts";
import { cp, cpSync } from "ext:deno_node/_fs/_fs_cp.ts";
import Dir from "ext:deno_node/_fs/_fs_dir.ts";
import { exists, existsSync } from "ext:deno_node/_fs/_fs_exists.ts";
import { fstat, fstatSync } from "ext:deno_node/_fs/_fs_fstat.ts";
import { lstat, lstatSync } from "ext:deno_node/_fs/_fs_lstat.ts";
import { lutimes, lutimesSync } from "ext:deno_node/_fs/_fs_lutimes.ts";
import { mkdir, mkdirSync } from "ext:deno_node/_fs/_fs_mkdir.ts";
import { mkdtemp, mkdtempSync } from "ext:deno_node/_fs/_fs_mkdtemp.ts";
import { open, openSync } from "ext:deno_node/_fs/_fs_open.ts";
import { opendir, opendirSync } from "ext:deno_node/_fs/_fs_opendir.ts";
import { read, readSync } from "ext:deno_node/_fs/_fs_read.ts";
import { readdir, readdirSync } from "ext:deno_node/_fs/_fs_readdir.ts";
import { readFile, readFileSync } from "ext:deno_node/_fs/_fs_readFile.ts";
import { readlink, readlinkSync } from "ext:deno_node/_fs/_fs_readlink.ts";
import { realpath, realpathSync } from "ext:deno_node/_fs/_fs_realpath.ts";
import { stat, Stats, statSync } from "ext:deno_node/_fs/_fs_stat.ts";
import { statfs, statfsSync } from "ext:deno_node/_fs/_fs_statfs.ts";
import { symlink, symlinkSync } from "ext:deno_node/_fs/_fs_symlink.ts";
import { truncate, truncateSync } from "ext:deno_node/_fs/_fs_truncate.ts";
import { utimes, utimesSync } from "ext:deno_node/_fs/_fs_utimes.ts";
import { unwatchFile, watch, watchFile } from "ext:deno_node/_fs/_fs_watch.ts";
// @deno-types="./_fs/_fs_write.d.ts"
import { write, writeSync } from "ext:deno_node/_fs/_fs_write.ts";
// @deno-types="./_fs/_fs_writev.d.ts"
import { writev, writevSync } from "ext:deno_node/_fs/_fs_writev.ts";
import { readv, readvSync } from "ext:deno_node/_fs/_fs_readv.ts";
import { writeFile, writeFileSync } from "ext:deno_node/_fs/_fs_writeFile.ts";
import promises from "ext:deno_node/internal/fs/promises.ts";
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
  validateRmdirOptions,
  validateRmOptions,
  validateRmOptionsSync,
} from "ext:deno_node/internal/fs/utils.mjs";
import { glob, globSync } from "ext:deno_node/_fs/_fs_glob.ts";
import {
  parseFileMode,
  validateFunction,
  validateInt32,
  validateInteger,
  validateObject,
  validateString,
} from "ext:deno_node/internal/validators.mjs";
import type { Buffer } from "node:buffer";
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
  op_node_rmdir,
  op_node_rmdir_sync,
} from "ext:core/ops";
import { FsFile } from "ext:deno_fs/30_fs.js";
import {
  ERR_FS_RMDIR_ENOTDIR,
  ERR_INVALID_ARG_TYPE,
} from "ext:deno_node/internal/errors.ts";
import { toUnixTimestamp } from "ext:deno_node/internal/fs/utils.mjs";
import { isMacOS } from "ext:deno_node/_util/os.ts";
import { core, primordials } from "ext:core/mod.js";

const {
  Error,
  ErrorPrototype,
  Number,
  NumberIsFinite,
  NumberIsNaN,
  ObjectPrototypeIsPrototypeOf,
  PromisePrototypeThen,
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
        // deno-lint-ignore no-explicit-any
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
        // deno-lint-ignore no-explicit-any
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
      // deno-lint-ignore no-explicit-any
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
      // deno-lint-ignore no-explicit-any
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

/**
 * TODO: Also accept 'data' parameter as a Node polyfill Buffer type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
function appendFile(
  path: string | number | URL,
  data: string | Uint8Array,
  options: Encodings | WriteFileOptions | CallbackWithError,
  callback?: CallbackWithError,
) {
  callback = maybeCallback(callback || options);
  options = getOptions(options, { encoding: "utf8", mode: 0o666, flag: "a" });

  // Don't make changes directly on options object
  options = copyObject(options);

  // Force append behavior when using a supplied file descriptor
  if (!options.flag || isFd(path)) {
    options.flag = "a";
  }

  writeFile(path, data, options, callback);
}

/**
 * TODO: Also accept 'data' parameter as a Node polyfill Buffer type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
function appendFileSync(
  path: string | number | URL,
  data: string | Uint8Array,
  options?: Encodings | WriteFileOptions,
) {
  options = getOptions(options, { encoding: "utf8", mode: 0o666, flag: "a" });

  // Don't make changes directly on options object
  options = copyObject(options);

  // Force append behavior when using a supplied file descriptor
  if (!options.flag || isFd(path)) {
    options.flag = "a";
  }

  writeFileSync(path, data, options);
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

function chmodSync(path: string | Buffer | URL, mode: string | number) {
  path = getValidatedPathToString(path);
  mode = parseFileMode(mode, "mode");

  try {
    Deno.chmodSync(path, mode);
  } catch (error) {
    throw denoErrorToNodeError(error as Error, { syscall: "chmod", path });
  }
}

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
      // TODO(@littledivy): Treat `fd` as real file descriptor. `rid` is an
      // implementation detail and may change.
      core.close(fd);
    } catch (err) {
      error = ObjectPrototypeIsPrototypeOf(ErrorPrototype, err)
        ? err as Error
        : new Error("[non-error thrown]");
    }
    callback(error);
  }, 0);
}

function closeSync(fd: number) {
  fd = getValidatedFd(fd);
  // TODO(@littledivy): Treat `fd` as real file descriptor. `rid` is an
  // implementation detail and may change.
  core.close(fd);
}

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

  if (!callback) throw new Error("No callback function supplied");

  PromisePrototypeThen(
    new FsFile(fd, SymbolFor("Deno.internal.FsFile")).truncate(len),
    () => callback(null),
    callback,
  );
}

function ftruncateSync(fd: number, len?: number) {
  new FsFile(fd, SymbolFor("Deno.internal.FsFile")).truncateSync(len);
}

function _getValidTime(
  time: number | string | Date,
  name: string,
): number | Date {
  if (typeof time === "string") {
    time = Number(time);
  }

  if (
    typeof time === "number" &&
    (NumberIsNaN(time) || !NumberIsFinite(time))
  ) {
    throw new Deno.errors.InvalidData(
      `invalid ${name}, must not be infinity or NaN`,
    );
  }

  return toUnixTimestamp(time);
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

  atime = _getValidTime(atime, "atime");
  mtime = _getValidTime(mtime, "mtime");

  PromisePrototypeThen(
    new FsFile(fd, SymbolFor("Deno.internal.FsFile")).utime(atime, mtime),
    () => callback(null),
    callback,
  );
}

function fchmodSync(fd: number, mode: string | number) {
  validateInteger(fd, "fd", 0, 2147483647);

  op_fs_fchmod_sync(fd, parseFileMode(mode, "mode"));
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

function futimesSync(
  fd: number,
  atime: number | string | Date,
  mtime: number | string | Date,
) {
  if (typeof fd !== "number") {
    throw new ERR_INVALID_ARG_TYPE("fd", "number", fd);
  }

  validateInteger(fd, "fd", 0, 2147483647);

  atime = _getValidTime(atime, "atime");
  mtime = _getValidTime(mtime, "mtime");

  new FsFile(fd, SymbolFor("Deno.internal.FsFile")).utimeSync(atime, mtime);
}

const lchmod:
  | ((
    path: string | Buffer | URL,
    mode: number,
    callback: CallbackWithError,
  ) => void)
  | undefined = !isMacOS ? undefined : (
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
      (err: Error) => callback(err),
    );
  };

const lchmodSync:
  | ((
    path: string | Buffer | URL,
    mode: number,
  ) => void)
  | undefined = !isMacOS
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

function fdatasyncSync(fd: number) {
  validateInt32(fd, "fd", 0);
  new FsFile(fd, SymbolFor("Deno.internal.FsFile")).syncDataSync();
}

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

function fsyncSync(fd: number) {
  validateInt32(fd, "fd", 0);
  new FsFile(fd, SymbolFor("Deno.internal.FsFile")).syncSync();
}

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

function linkSync(
  existingPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) {
  existingPath = getValidatedPathToString(existingPath);
  newPath = getValidatedPathToString(newPath);

  Deno.linkSync(existingPath, newPath);
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

function unlinkSync(path: string | Buffer | URL): void {
  path = getValidatedPathToString(path);
  try {
    Deno.removeSync(path);
  } catch (err) {
    throw denoErrorToNodeError(err as Error, { syscall: "unlink", path });
  }
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

  if (!callback) throw new Error("No callback function supplied");

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

export default {
  access,
  accessSync,
  appendFile,
  appendFileSync,
  chmod,
  chmodSync,
  chown,
  chownSync,
  close,
  closeSync,
  constants,
  copyFile,
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
  fchmodSync,
  fchown,
  fchownSync,
  fdatasync,
  fdatasyncSync,
  fstat,
  fstatSync,
  fsync,
  fsyncSync,
  ftruncate,
  ftruncateSync,
  futimes,
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
  symlinkSync,
  truncate,
  truncateSync,
  unlink,
  unlinkSync,
  unwatchFile,
  utimes,
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
  chmodSync,
  chown,
  chownSync,
  close,
  closeSync,
  constants,
  copyFile,
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
  fchmodSync,
  fchown,
  fchownSync,
  fdatasync,
  fdatasyncSync,
  fstat,
  fstatSync,
  fsync,
  fsyncSync,
  ftruncate,
  ftruncateSync,
  futimes,
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
  symlinkSync,
  truncate,
  truncateSync,
  unlink,
  unlinkSync,
  unwatchFile,
  utimes,
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
