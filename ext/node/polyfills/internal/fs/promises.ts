// Copyright 2018-2026 the Deno authors. MIT license.

import { core } from "ext:core/mod.js";
import type { WriteFileOptions } from "ext:deno_node/_fs/_fs_common.ts";
import type { Encodings } from "ext:deno_node/_utils.ts";
const { promisify } = core.loadExtScript("ext:deno_node/internal/util.mjs");
const constants = core.loadExtScript("ext:deno_node/_fs/_fs_constants.ts");
import { copyFilePromise } from "ext:deno_node/_fs/_fs_copy.ts";
const { cpPromise } = core.loadExtScript("ext:deno_node/_fs/_fs_cp.ts");
import { lutimesPromise } from "ext:deno_node/_fs/_fs_lutimes.ts";
import { readdirPromise } from "ext:deno_node/_fs/_fs_readdir.ts";
const { lstatPromise } = core.loadExtScript("ext:deno_node/_fs/_fs_lstat.ts");
const lazyFs = core.createLazyLoader("node:fs");
import { globPromise } from "ext:deno_node/_fs/_fs_glob.ts";
import { getValidatedPathToString } from "ext:deno_node/internal/fs/utils.mjs";
import type { Buffer } from "node:buffer";
import Dir from "ext:deno_node/_fs/_fs_dir.ts";
import { FileHandle } from "ext:deno_node/internal/fs/handle.ts";
import { primordials } from "ext:core/mod.js";
const { parseFileMode } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
import { op_node_lchmod } from "ext:core/ops";
const { isMacOS } = core.loadExtScript("ext:deno_node/_util/os.ts");
const { ERR_METHOD_NOT_IMPLEMENTED, aggregateTwoErrors } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);
const lazyPath = core.createLazyLoader("node:path");
const lazyProcess = core.createLazyLoader("node:process");

const {
  ObjectPrototypeIsPrototypeOf,
  Promise,
  PromiseReject,
  SymbolAsyncDispose,
} = primordials;

// Mirrors Node's lib/internal/fs/promises.js handleFdClose(): run the file op,
// then close the FileHandle. Looks up `fh.close` lazily so tests that
// monkey-patch the prototype/instance close still take effect.
//   op ok, close ok       -> resolve(result)
//   op ok, close throws   -> throw closeError
//   op throws, close ok   -> throw opError
//   op throws, close throws -> throw AggregateError([opError, closeError])
async function handleFdClose<T>(
  fileOpPromise: Promise<T>,
  closeFunc: () => Promise<void>,
): Promise<T> {
  let result: T;
  let opError: unknown;
  let opFailed = false;
  try {
    result = await fileOpPromise;
  } catch (err) {
    opError = err;
    opFailed = true;
  }
  try {
    await closeFunc();
  } catch (closeError) {
    if (opFailed) {
      // Mirrors Node's aggregateTwoErrors(): preserves opError.code on the
      // AggregateError so callers asserting err.code see the op's code.
      throw aggregateTwoErrors(closeError, opError);
    }
    throw closeError;
  }
  if (opFailed) {
    throw opError;
  }
  return result!;
}

// -- access --

const accessPromise = promisify(lazyFs().access) as (
  path: string | Buffer | URL,
  mode?: number,
) => Promise<void>;

// -- appendFile --

// Delegates to writeFilePromise with an "a" flag, mirroring Node's
// lib/internal/fs/promises.js appendFile(). Per Node semantics, when given a
// FileHandle the existing flag stays in effect.
function appendFilePromise(
  path: string | number | URL | FileHandle,
  data: string | Uint8Array,
  options?: Encodings | WriteFileOptions,
): Promise<void> {
  let opts: WriteFileOptions;
  if (typeof options === "string") {
    opts = { encoding: options };
  } else if (options == null || typeof options !== "object") {
    opts = {};
  } else {
    opts = { ...options };
  }
  opts.flag = opts.flag || "a";
  return writeFilePromise(path, data, opts);
}

// -- chmod --

const chmodPromise = promisify(lazyFs().chmod) as (
  path: string | Buffer | URL,
  mode: string | number,
) => Promise<void>;

// -- chown --

const chownPromise = promisify(lazyFs().chown) as (
  path: string | Buffer | URL,
  uid: number,
  gid: number,
) => Promise<void>;

const lchmodPromise: (
  path: string | Buffer | URL,
  mode: number,
) => Promise<void> = !isMacOS
  ? () => PromiseReject(new ERR_METHOD_NOT_IMPLEMENTED("lchmod()"))
  : async (path: string | Buffer | URL, mode: number) => {
    path = getValidatedPathToString(path);
    mode = parseFileMode(mode, "mode");
    return await op_node_lchmod(path, mode);
  };

const lchownPromise = promisify(lazyFs().lchown) as (
  path: string | Buffer | URL,
  uid: number,
  gid: number,
) => Promise<void>;

const linkPromise = promisify(lazyFs().link) as (
  existingPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) => Promise<void>;

const unlinkPromise = promisify(lazyFs().unlink) as (
  path: string | Buffer | URL,
) => Promise<void>;

const renamePromise = promisify(lazyFs().rename) as (
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

const rmPromise = promisify(lazyFs().rm) as (
  path: string | URL,
  options?: rmOptions,
) => Promise<void>;

// -- rmdir --

type rmdirOptions = {
  maxRetries?: number;
  recursive?: boolean;
  retryDelay?: number;
};

const rmdirPromise = promisify(lazyFs().rmdir) as (
  path: string | Buffer | URL,
  options?: rmdirOptions,
) => Promise<void>;

type MkdirOptions =
  | { recursive?: boolean; mode?: number | undefined }
  | number
  | boolean;

const mkdirPromise = promisify(lazyFs().mkdir) as (
  path: string | URL,
  options?: MkdirOptions,
) => Promise<string | undefined>;

const mkdtempPromise = promisify(lazyFs().mkdtemp) as (
  prefix: string | Buffer | Uint8Array | URL,
  options?: { encoding: string } | string,
) => Promise<string>;

// Mirrors Node's lib/internal/fs/promises.js mkdtempDisposable(): create the
// temp dir, then return an object with .path, .remove(), and Symbol.asyncDispose
// that recursively removes the directory. Capture cwd at creation time so a
// later process.chdir() doesn't break removal.
async function mkdtempDisposablePromise(
  prefix: string | Buffer | Uint8Array | URL,
  options?: { encoding: string } | string,
) {
  const cwd = lazyProcess().default.cwd();
  const path = await mkdtempPromise(prefix, options);
  const fullPath = lazyPath().resolve(cwd, path);
  // `force: true` makes the second remove() a no-op when the dir is already
  // gone (Node's rimraf-based implementation treats ENOENT as success); other
  // errors (EACCES, EPERM, ...) still propagate.
  const remove = async () => {
    await rmPromise(fullPath, {
      force: true,
      maxRetries: 0,
      recursive: true,
      retryDelay: 0,
    });
  };
  return {
    __proto__: null,
    path,
    remove,
    async [SymbolAsyncDispose]() {
      await remove();
    },
  };
}

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

function openPromise(
  path: string | Buffer | URL,
  flags: OpenFlags = "r",
  mode = 0o666,
): Promise<FileHandle> {
  return new Promise((resolve, reject) => {
    lazyFs().open(path, flags, mode, (err, fd) => {
      if (err) reject(err);
      else resolve(new FileHandle(fd as number));
    });
  });
}

type OpendirOptions = {
  encoding?: string;
  bufferSize?: number;
};

const opendirPromise = promisify(lazyFs().opendir) as (
  path: string | Buffer | URL,
  options?: OpendirOptions,
) => Promise<Dir>;

// -- symlink --

const symlinkPromise = promisify(lazyFs().symlink) as (
  target: string | Buffer | URL,
  path: string | Buffer | URL,
  type?: string,
) => Promise<void>;

// -- truncate --

// Mirrors Node's lib/internal/fs/promises.js truncate(): open the path as a
// FileHandle, delegate to its truncate method, then close via handleFdClose
// so callers that monkey-patch FileHandle still observe the fd access and
// AggregateError-on-double-failure semantics that Node tests rely on.
async function truncatePromise(
  path: string | URL,
  len?: number,
): Promise<void> {
  const fh = await openPromise(path, "r+");
  return handleFdClose(fh.truncate(len), () => fh.close());
}

// -- utimes --

const utimesPromise = promisify(lazyFs().utimes) as (
  path: string | URL,
  atime: number | string | Date,
  mtime: number | string | Date,
) => Promise<void>;

// -- writeFile --

// Low-level callback writeFile, used when we already have an fd/FileHandle
// (i.e. avoid recursing back through writeFilePromise via FileHandle.writeFile).
const rawWriteFilePromise = promisify(lazyFs().writeFile) as (
  pathOrRid: string | number | URL | FileHandle,
  data:
    | string
    | DataView
    | NodeJS.TypedArray
    | AsyncIterable<NodeJS.TypedArray | string>,
  options?: Encodings | WriteFileOptions,
) => Promise<void>;

// Mirrors Node's lib/internal/fs/promises.js writeFile(): when given a path,
// open a FileHandle and delegate via handleFdClose so the close error
// semantics are observable; when given an fd/FileHandle, write directly.
function writeFilePromise(
  pathOrRid: string | number | URL | FileHandle,
  data:
    | string
    | DataView
    | NodeJS.TypedArray
    | AsyncIterable<NodeJS.TypedArray | string>,
  options?: Encodings | WriteFileOptions,
): Promise<void> {
  if (
    typeof pathOrRid === "number" ||
    ObjectPrototypeIsPrototypeOf(FileHandle.prototype, pathOrRid)
  ) {
    return rawWriteFilePromise(pathOrRid, data, options);
  }
  const opts: WriteFileOptions = typeof options === "string"
    ? { encoding: options }
    : (options ?? {});
  const flag = opts.flag ?? "w";
  const mode = opts.mode ?? 0o666;
  return (async () => {
    // Match the existing path-based behavior: surface the same `DOMException`
    // that `signal.throwIfAborted()` produces (the fd-based fallback would
    // throw Deno's `AbortError` instead). Inside the async IIFE so the throw
    // becomes a promise rejection, not a sync throw.
    if (opts.signal?.aborted) opts.signal.throwIfAborted();
    const fh = await openPromise(
      pathOrRid as string | Buffer | URL,
      flag,
      mode,
    );
    return handleFdClose(fh.writeFile(data, opts), () => fh.close());
  })();
}

// -- realpath --

const realpathPromise = promisify(lazyFs().realpath) as (
  path: string | Buffer,
  options?: string | { encoding?: string },
) => Promise<string | Buffer>;

// -- stat --

const statPromise = promisify(lazyFs().stat) as (
  path: string | Buffer | URL,
  options?: { bigint?: boolean },
) => Promise<unknown>;

// -- statfs --

const statfsPromise = promisify(lazyFs().statfs) as (
  path: string | Buffer | URL,
  options?: { bigint?: boolean },
) => Promise<unknown>;

// -- readFile / readlink --

// Low-level callback readFile, used when we already have an fd/FileHandle
// (i.e. avoid recursing back through readFilePromise via FileHandle.readFile).
const rawReadFilePromise = promisify(lazyFs().readFile);

// Mirrors Node's lib/internal/fs/promises.js readFile(): when given a path,
// open a FileHandle and delegate via handleFdClose so the close error
// semantics are observable; when given an fd/FileHandle, read directly.
function readFilePromise(
  path: string | number | URL | FileHandle,
  options?: Encodings | {
    encoding?: Encodings;
    flag?: string;
    signal?: AbortSignal;
  },
): Promise<string | Buffer> {
  if (
    typeof path === "number" ||
    ObjectPrototypeIsPrototypeOf(FileHandle.prototype, path)
  ) {
    return rawReadFilePromise(path, options);
  }
  const opts: { encoding?: Encodings; flag?: string; signal?: AbortSignal } =
    typeof options === "string" ? { encoding: options } : (options ?? {});
  const flag = opts.flag ?? "r";
  return (async () => {
    // Match the existing path-based behavior: surface the same `DOMException`
    // that `signal.throwIfAborted()` produces (the fd-based fallback would
    // throw Deno's `AbortError` instead). Inside the async IIFE so the throw
    // becomes a promise rejection, not a sync throw.
    if (opts.signal?.aborted) opts.signal.throwIfAborted();
    const fh = await openPromise(path as string | Buffer | URL, flag);
    return handleFdClose(fh.readFile(opts), () => fh.close()) as Promise<
      string | Buffer
    >;
  })();
}

const readlinkPromise = promisify(lazyFs().readlink) as (
  path: string | Buffer | URL,
  opt?: { encoding?: string | null },
) => Promise<string | Uint8Array>;

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
  mkdtempDisposable: mkdtempDisposablePromise,
  writeFile: writeFilePromise,
  appendFile: appendFilePromise,
  readFile: readFilePromise,
  watch: lazyFs().watchPromise,
};

export default promises;

export { constants, FileHandle, mkdirPromise, opendirPromise };
