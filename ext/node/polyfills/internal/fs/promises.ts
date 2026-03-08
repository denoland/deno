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
import {
  denoErrorToNodeError,
  ERR_INVALID_ARG_TYPE,
  uvException,
} from "ext:deno_node/internal/errors.ts";
import { normalizeEncoding, promisify } from "ext:deno_node/internal/util.mjs";
import * as constants from "ext:deno_node/_fs/_fs_constants.ts";
import { copyFilePromise } from "ext:deno_node/_fs/_fs_copy.ts";
import { cpPromise } from "ext:deno_node/_fs/_fs_cp.ts";
import { lstatPromise } from "ext:deno_node/_fs/_fs_lstat.ts";
import { lutimesPromise } from "ext:deno_node/_fs/_fs_lutimes.ts";
import { readdirPromise } from "ext:deno_node/_fs/_fs_readdir.ts";
import { readFilePromise } from "ext:deno_node/_fs/_fs_readFile.ts";
import { readlinkPromise } from "ext:deno_node/_fs/_fs_readlink.ts";
import { realpathPromise } from "ext:deno_node/_fs/_fs_realpath.ts";
import { statPromise } from "ext:deno_node/_fs/_fs_stat.ts";
import { statfsPromise } from "ext:deno_node/_fs/_fs_statfs.ts";
import { symlinkPromise } from "ext:deno_node/_fs/_fs_symlink.ts";
import { truncatePromise } from "ext:deno_node/_fs/_fs_truncate.ts";
import { utimesPromise } from "ext:deno_node/_fs/_fs_utimes.ts";
import { watchPromise } from "ext:deno_node/_fs/_fs_watch.ts";
import {
  writeFile,
  writeFilePromise,
} from "ext:deno_node/_fs/_fs_writeFile.ts";
import { globPromise } from "ext:deno_node/_fs/_fs_glob.ts";
import {
  copyObject,
  emitRecursiveRmdirWarning,
  getOptions,
  getValidatedPath,
  getValidatedPathToString,
  getValidMode,
  kMaxUserId,
  type RmOptions,
  stringToFlags,
  validateRmdirOptions,
  validateRmOptions,
  warnOnNonPortableTemplate,
} from "ext:deno_node/internal/fs/utils.mjs";
import {
  parseFileMode,
  validateBoolean,
  validateFunction,
  validateInteger,
} from "ext:deno_node/internal/validators.mjs";
import { Buffer } from "node:buffer";
import Dir from "ext:deno_node/_fs/_fs_dir.ts";
import { FileHandle } from "ext:deno_node/internal/fs/handle.ts";
import { resolve, toNamespacedPath } from "node:path";
import { isWindows } from "ext:deno_node/_util/os.ts";
import type { Encoding } from "node:crypto";
import { primordials } from "ext:core/mod.js";
import {
  op_node_lchmod,
  op_node_lchown,
  op_node_mkdtemp,
  op_node_open,
  op_node_rmdir,
} from "ext:core/ops";
import { isMacOS } from "ext:deno_node/_util/os.ts";
import {
  ERR_FS_RMDIR_ENOTDIR,
  ERR_METHOD_NOT_IMPLEMENTED,
} from "ext:deno_node/internal/errors.ts";

const {
  Error,
  ObjectPrototypeIsPrototypeOf,
  Promise,
  PromisePrototypeThen,
  PromiseReject,
  StringPrototypeToString,
} = primordials;

// -- access --

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

const accessPromise = promisify(access) as (
  path: string | Buffer | URL,
  mode?: number,
) => Promise<void>;

// -- appendFile --

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

const appendFilePromise = promisify(appendFile) as (
  path: string | number | URL,
  data: string | Uint8Array,
  options?: Encodings | WriteFileOptions,
) => Promise<void>;

// -- chmod --

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

// -- chown --

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

const lchmodPromise: (
  path: string | Buffer | URL,
  mode: number,
) => Promise<void> = !isMacOS
  ? () => PromiseReject(new ERR_METHOD_NOT_IMPLEMENTED("lchmod()"))
  : (path: string | Buffer | URL, mode: number) => {
    path = getValidatedPathToString(path);
    mode = parseFileMode(mode, "mode");
    return op_node_lchmod(path, mode);
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

const lchownPromise = promisify(lchown) as (
  path: string | Buffer | URL,
  uid: number,
  gid: number,
) => Promise<void>;

const linkPromise = promisify(link) as (
  existingPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) => Promise<void>;

function unlink(
  path: string | Buffer | URL,
  callback: (err?: Error) => void,
): void {
  path = getValidatedPathToString(path);

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

// -- rename --

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

// -- rm --

type rmOptions = {
  force?: boolean;
  maxRetries?: number;
  recursive?: boolean;
  retryDelay?: number;
};

type rmCallback = (err: Error | null) => void;

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

const rmPromise = promisify(rm) as (
  path: string | URL,
  options?: rmOptions,
) => Promise<void>;

// -- rmdir --

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

// -- mkdir --

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
  let cursor = resolve(path, "..");
  while (true) {
    try {
      const stat = Deno.statSync(cursor);
      if (!stat.isDirectory) {
        return uvException({
          errno: codeMap.get("ENOTDIR")!,
          syscall: "mkdir",
          path,
        });
      }
      break;
    } catch {
      const parent = resolve(cursor, "..");
      if (parent === cursor) break;
      cursor = parent;
    }
  }
  return nodeErr;
}

function findFirstNonExistent(path: string): string | undefined {
  let cursor = resolve(path);
  while (true) {
    try {
      Deno.statSync(cursor);
      return undefined;
    } catch {
      const parent = resolve(cursor, "..");
      if (parent === cursor) {
        return toNamespacedPath(cursor);
      }
      try {
        Deno.statSync(parent);
        return toNamespacedPath(cursor);
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

  PromisePrototypeThen(
    Deno.mkdir(path, { recursive, mode }),
    () => {
      if (typeof callback === "function") {
        callback(null, firstNonExistent);
      }
    },
    (err: Error) => {
      if (typeof callback === "function") {
        callback(
          recursive
            ? fixMkdirError(err as Error, path as string)
            : denoErrorToNodeError(err as Error, { syscall: "mkdir", path }),
        );
      }
    },
  );
}

const mkdirPromise = promisify(mkdir) as (
  path: string | URL,
  options?: MkdirOptions,
) => Promise<string | undefined>;

// -- mkdtemp --

type MkdtempCallback = (
  err: Error | null,
  directory?: string,
) => void;
type MkdtempBufferCallback = (
  err: Error | null,
  directory?: Buffer<ArrayBufferLike>,
) => void;

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

const mkdtempPromise = promisify(mkdtemp) as (
  prefix: string | Buffer | Uint8Array | URL,
  options?: { encoding: string } | string,
) => Promise<string>;

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

// -- open --

type OpenFlags =
  | "a"
  | "ax"
  | "a+"
  | "ax+"
  | "as"
  | "as+"
  | "r"
  | "r+"
  | "rs"
  | "rs+"
  | "w"
  | "wx"
  | "w+"
  | "wx+"
  | number
  | string;

type OpenCallback = (err: Error | null, fd?: number) => void;

function open(
  path: string | Buffer | URL,
  flags: OpenCallback | OpenFlags,
  mode?: OpenCallback | number,
  callback?: OpenCallback,
) {
  path = getValidatedPathToString(path);
  if (arguments.length < 3) {
    // deno-lint-ignore no-explicit-any
    callback = flags as any;
    flags = "r";
    mode = 0o666;
  } else if (typeof mode === "function") {
    callback = mode;
    mode = 0o666;
  } else {
    mode = parseFileMode(mode, "mode", 0o666);
  }
  flags = stringToFlags(flags);
  callback = makeCallback(callback);

  PromisePrototypeThen(
    op_node_open(path, flags, mode),
    (rid: number) => callback(null, rid),
    (err: Error) =>
      callback(denoErrorToNodeError(err, { syscall: "open", path })),
  );
}

function openPromise(
  path: string | Buffer | URL,
  flags: OpenFlags = "r",
  mode = 0o666,
): Promise<FileHandle> {
  return new Promise((resolve, reject) => {
    open(path, flags, mode, (err, fd) => {
      if (err) reject(err);
      else resolve(new FileHandle(fd as number));
    });
  });
}

// -- opendir --

type OpendirOptions = {
  encoding?: string;
  bufferSize?: number;
};
type OpendirCallback = (err?: Error | null, dir?: Dir) => void;

function _opendirValidateFunction(
  callback: unknown,
): asserts callback is OpendirCallback {
  validateFunction(callback, "callback");
}

function _opendirGetPathString(
  path: string | Buffer | URL,
): string {
  if (Buffer.isBuffer(path)) {
    // deno-lint-ignore prefer-primordials
    return path.toString();
  }

  return StringPrototypeToString(path);
}

function opendir(
  path: string | Buffer | URL,
  options: OpendirOptions | OpendirCallback,
  callback?: OpendirCallback,
) {
  callback = typeof options === "function" ? options : callback;
  _opendirValidateFunction(callback);

  path = _opendirGetPathString(getValidatedPath(path));

  let err, dir;
  try {
    const { bufferSize } = getOptions(options, {
      encoding: "utf8",
      bufferSize: 32,
    });
    validateInteger(bufferSize, "options.bufferSize", 1, 4294967295);

    Deno.readDirSync(path);

    dir = new Dir(path);
  } catch (error) {
    err = denoErrorToNodeError(error as Error, { syscall: "opendir" });
  }
  if (err) {
    callback(err);
  } else {
    callback(null, dir);
  }
}

const opendirPromise = promisify(opendir) as (
  path: string | Buffer | URL,
  options?: OpendirOptions,
) => Promise<Dir>;

// -- promises object --

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

export default promises;

export { mkdirPromise, opendirPromise };
