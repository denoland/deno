// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-explicit-any

(function () {
const { core, primordials } = __bootstrap;
const { codeMap } = core.loadExtScript(
  "ext:deno_node/internal_binding/uv.ts",
);
type BinaryOptionsArgument = any;
type CallbackWithError = any;
type FileOptions = any;
type FileOptionsArgument = any;
type TextOptionsArgument = any;
type WriteFileOptions = any;
const {
  callbackify,
  callbackifyOpt,
  callbackifyWrite,
  getValidatedEncoding,
  isFd,
  isFileOptions,
  makeCallback,
  maybeCallback,
} = core.loadExtScript("ext:deno_node/_fs/_fs_common.ts");
type Encodings = any;
const {
  AbortError,
  denoErrorToNodeError,
  denoWriteFileErrorToNodeError,
  ERR_FS_FILE_TOO_LARGE,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const constants = core.loadExtScript("ext:deno_node/_fs/_fs_constants.ts");
type statCallback = any;
type statCallbackBigInt = any;
type statOptions = any;
const { copyFile, copyFileSync } = core.createLazyLoader(
  "ext:deno_node/_fs/_fs_copy.ts",
)();
const { cp, cpSync } = core.loadExtScript("ext:deno_node/_fs/_fs_cp.ts");
const { default: Dir } = core.createLazyLoader(
  "ext:deno_node/_fs/_fs_dir.ts",
)();
const { exists, existsSync } = core.createLazyLoader(
  "ext:deno_node/_fs/_fs_exists.ts",
)();
const { fstat, fstatSync } = core.loadExtScript(
  "ext:deno_node/_fs/_fs_fstat.ts",
);
const { lstat, lstatSync } = core.loadExtScript(
  "ext:deno_node/_fs/_fs_lstat.ts",
);
const { lutimes, lutimesSync } = core.createLazyLoader(
  "ext:deno_node/_fs/_fs_lutimes.ts",
)();
const { read, readSync } = core.createLazyLoader(
  "ext:deno_node/_fs/_fs_read.ts",
)();
const { readdir, readdirSync } = core.createLazyLoader(
  "ext:deno_node/_fs/_fs_readdir.ts",
)();
const { EventEmitter } = core.loadExtScript("ext:deno_node/_events.mjs");
const lazyTimers = core.createLazyLoader("node:timers");
const { clearTimeout, setTimeout } = lazyTimers();
type MaybeEmpty<T> = T | null | undefined;
const { deprecate, promisify } = core.loadExtScript("ext:deno_node/util.ts");
// internal/fs/{promises,streams,handle}.ts call `lazyFs()` at top-level to
// build promisified wrappers around members of `node:fs`. Loading them
// eagerly from inside fs.ts would re-enter the partially-loaded `node:fs`
// namespace and hit a TDZ error. Defer to first access of `fs.promises` etc.
const lazyInternalPromises = core.createLazyLoader(
  "ext:deno_node/internal/fs/promises.ts",
);
const lazyInternalStreams = core.createLazyLoader(
  "ext:deno_node/internal/fs/streams.mjs",
);
const lazyInternalHandle = core.createLazyLoader(
  "ext:deno_node/internal/fs/handle.ts",
);
// Backing storage so the lazy getters below can be paired with setters;
// some packages monkey-patch these on the `node:fs` namespace.
let _createReadStream: any;
let _createWriteStream: any;
let _ReadStream: any;
let _WriteStream: any;
let _promises: any;
const { default: SyncWriteStream } = core.loadExtScript(
  "ext:deno_node/internal/fs/sync_write_stream.js",
);
// Utf8Stream is only re-exported, never used at module body. Keep this as
// a thunk so loading fs.ts doesn't immediately pull fast-utf8-stream.js
// (which statically imports node:fs and triggers the whole stream subtree).
const lazyUtf8Stream = core.createLazyLoader(
  "ext:deno_node/internal/streams/fast-utf8-stream.js",
);
const {
  BigIntStats,
  constants: fsUtilConstants,
  copyObject,
  Dirent,
  getOptions,
  getValidatedPath,
  getValidatedPathToString,
  Stats,
  stringToFlags,
  toUnixTimestamp,
  validateStringAfterArrayBufferView,
} = core.createLazyLoader("ext:deno_node/internal/fs/utils.mjs")();
const { glob, globSync } = core.createLazyLoader(
  "ext:deno_node/_fs/_fs_glob.ts",
)();
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const lazyProcess = core.createLazyLoader("node:process");
const process = lazyProcess().default;
const { isIterable } = core.loadExtScript(
  "ext:deno_node/internal/streams/utils.js",
);
type FileHandle = any;
type ErrnoException = any;
type BufferEncoding = any;
const {
  op_fs_read_file_async,
  op_node_fs_close,
  op_node_fs_close_async,
  op_node_fs_fchmod,
  op_node_fs_fchmod_sync,
  op_node_fs_fchown,
  op_node_fs_fchown_sync,
  op_node_fs_fdatasync,
  op_node_fs_fdatasync_sync,
  op_node_fs_fstat_sync,
  op_node_fs_fsync,
  op_node_fs_fsync_sync,
  op_node_fs_ftruncate,
  op_node_fs_ftruncate_sync,
  op_node_fs_futimes,
  op_node_fs_futimes_sync,
  op_node_fs_read_deferred,
  op_node_fs_encode_bytes,
  op_node_fs_read_file,
  op_node_fs_readv,
  op_node_fs_readv_sync,
  op_node_fs_read_file_path_sync,
  op_node_fs_read_file_path,
  op_node_fs_write_deferred,
  op_node_fs_write_sync,
  op_node_fs_write_v_sync,
  op_node_fs_write_v,
  op_node_fs_writev_sync,
  op_node_fs_writev,
  op_node_fs_write_file_sync,
  op_node_fs_write_file,
  op_node_fs_truncate_sync,
  op_node_fs_truncate,
  op_node_lchmod,
  op_node_lchmod_sync,
  op_node_lchown,
  op_node_lchown_sync,
  op_node_mkdtemp,
  op_node_mkdtemp_sync,
  op_node_open,
  op_node_open_sync,
  op_node_rmdir,
  op_node_rmdir_sync,
  op_node_statfs,
  op_node_statfs_sync,
  op_node_fs_stat,
  op_node_fs_stat_sync,
  op_node_fs_empty_stats,
  op_node_fs_mkdir,
  op_node_fs_mkdir_sync,
  op_node_fs_remove,
  op_node_fs_remove_sync,
  op_node_fs_rm,
  op_node_fs_rm_sync,
  op_node_fs_rename,
  op_node_fs_rename_sync,
  op_node_fs_realpath,
  op_node_fs_realpath_sync,
  op_node_fs_read_link,
  op_node_fs_read_link_sync,
  op_node_fs_chmod,
  op_node_fs_chmod_sync,
  op_node_fs_chown,
  op_node_fs_chown_sync,
  op_node_fs_link,
  op_node_fs_link_sync,
  op_node_fs_symlink,
  op_node_fs_symlink_sync,
  op_node_fs_utime,
  op_node_fs_utime_sync,
  op_node_fs_opendir_sync,
  op_node_fs_access,
  op_node_fs_access_sync,
  op_node_fs_stats_changed,
  op_node_fs_validate_watch_ignore,
  op_node_fs_encode_watch_filename,
} = core.ops;
const {
  ERR_INVALID_ARG_VALUE,
  uvException,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const { isMacOS, isWindows } = core.loadExtScript(
  "ext:deno_node/_util/os.ts",
);
const {
  customPromisifyArgs,
} = core.loadExtScript("ext:deno_node/internal/util.mjs");
const lazyPath = core.createLazyLoader("node:path");
const pathModule = lazyPath();
const { basename, relative, resolve } = pathModule;
type Encoding = any;
const {
  validateAbortSignal,
  validateBoolean,
  validateFunction,
  validateObject,
  validateString,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const { Blob, markFileBackedBlob } = core.loadExtScript(
  "ext:deno_web/09_file.js",
);
// Re-exported under both names for tests.
const _toUnixTimestamp = toUnixTimestamp;

const {
  ArrayBufferIsView,
  ArrayIsArray,
  Error,
  FunctionPrototypeBind,
  MapPrototypeDelete,
  MapPrototypeGet,
  MapPrototypeSet,
  MathMin,
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  Promise,
  PromisePrototypeThen,
  PromiseResolve,
  RegExpPrototype,
  RegExpPrototypeTest,
  SafeMap,
  SymbolAsyncIterator,
  SymbolDispose,
  SymbolFor,
  ArrayPrototypePush,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeSet,
  TypedArrayPrototypeSubarray,
  Uint8Array,
} = primordials;

const abortSignal = core.loadExtScript("ext:deno_web/03_abort_signal.js");
const { pathFromURL } = core.loadExtScript("ext:deno_web/00_infra.js");
const { URLPrototype } = core.loadExtScript("ext:deno_web/00_url.js");

const {
  kIoMaxLength,
  kReadFileUnknownBufferLength,
} = fsUtilConstants;

function stat(
  path: string | Buffer | URL,
  callback: statCallback,
): void;
function stat(
  path: string | Buffer | URL,
  options: { bigint: false },
  callback: statCallback,
): void;
function stat(
  path: string | Buffer | URL,
  options: { bigint: true },
  callback: statCallbackBigInt,
): void;
// The op extracts bigint/throwIfNoEntry from options, validates the path
// (async(eager_throw)), and resolves the Stats (or undefined when
// throwIfNoEntry is false and the path is missing).
const stat = callbackifyOpt(op_node_fs_stat);

// Direct op binding: the op extracts bigint/throwIfNoEntry from options,
// validates the path, and returns the cppgc Stats (or undefined when
// throwIfNoEntry is false and the path is missing).
const statSync = op_node_fs_stat_sync as {
  (path: string | Buffer | URL): Stats;
  (
    path: string | Buffer | URL,
    options: { bigint: false; throwIfNoEntry: true },
  ): Stats;
  (
    path: string | Buffer | URL,
    options: { bigint: false; throwIfNoEntry: false },
  ): Stats | undefined;
  (
    path: string | Buffer | URL,
    options: { bigint: true; throwIfNoEntry: true },
  ): BigIntStats;
  (
    path: string | Buffer | URL,
    options: { bigint: true; throwIfNoEntry: false },
  ): BigIntStats | undefined;
  (
    path: string | Buffer | URL,
    options?: statOptions,
  ): Stats | BigIntStats | undefined;
};

// -- realpath --

type RealpathEncoding = BufferEncoding | "buffer";
type RealpathEncodingObj = { encoding?: RealpathEncoding };
type RealpathOptions = RealpathEncoding | RealpathEncodingObj;
type RealpathCallback = (
  err: Error | null,
  path?: string | Buffer,
) => void;

// The op validates the path + encoding options synchronously
// (async(eager_throw)) and returns the result already encoded (string or
// Buffer). `fs.realpath` reports syscall "lstat"; `fs.realpath.native`
// reports "realpath" (matching node's lib/fs.js).
// realpath / realpath.native share the op but pass a different `syscall`
// ("lstat" vs "realpath"); bind it once each (no per-function wrapper SFI).
function callbackifyRealpath(syscall: "lstat" | "realpath") {
  return function (
    path: string | Buffer,
    optionsOrCb: any,
    maybeCb?: any,
  ) {
    const isCb = typeof optionsOrCb === "function";
    const promise = op_node_fs_realpath(
      path,
      isCb ? undefined : optionsOrCb,
      syscall,
    );
    const callback = makeCallback(isCb ? optionsOrCb : maybeCb);
    return PromisePrototypeThen(
      promise,
      (resolved: unknown) => callback(null, resolved),
      callback,
    );
  };
}

const realpath = callbackifyRealpath("lstat");
realpath.native = callbackifyRealpath("realpath");

function realpathSync(
  path: string,
  options?: RealpathOptions | RealpathEncoding,
): string | Buffer {
  return op_node_fs_realpath_sync(path, options, "lstat");
}

realpathSync.native = function realpathSync_native(
  path: string,
  options?: RealpathOptions | RealpathEncoding,
): string | Buffer {
  return op_node_fs_realpath_sync(path, options, "realpath");
};

// -- readv --

type ReadvCallback = (
  err: ErrnoException | null,
  bytesRead: number,
  buffers: readonly ArrayBufferView[],
) => void;

function readv(
  fd: number,
  buffers: readonly ArrayBufferView[],
  callback: ReadvCallback,
): void;
// The op validates fd/buffers/position synchronously (async(eager_throw)),
// short-circuits empty buffer lists, then seeks to `position` (-1 = current)
// and fills each view in order.
function readv(
  fd: number,
  buffers: readonly ArrayBufferView[],
  position: number | ReadvCallback,
  callback?: ReadvCallback,
): void {
  const promise = op_node_fs_readv(fd, buffers, position);
  const cb = maybeCallback(callback || position) as ReadvCallback;
  PromisePrototypeThen(
    promise,
    (numRead) => cb(null, numRead, buffers),
    (err) => cb(err, -1, buffers),
  );
}

ObjectDefineProperty(readv, customPromisifyArgs, {
  __proto__: null,
  value: ["bytesRead", "buffers"],
  enumerable: false,
});

interface ReadVResult {
  bytesRead: number;
  buffers: readonly ArrayBufferView[];
}

function readvSync(
  fd: number,
  buffers: readonly ArrayBufferView[],
  position: number | null = null,
): number {
  return op_node_fs_readv_sync(fd, buffers, position);
}

function readvPromise(
  fd: number,
  buffers: readonly ArrayBufferView[],
  position?: number,
): Promise<ReadVResult> {
  return new Promise((resolve, reject) => {
    readv(fd, buffers, position ?? null, (err, bytesRead, buffers) => {
      if (err) reject(err);
      else resolve({ bytesRead, buffers });
    });
  });
}

// -- readFile --

const readFileDefaultOptions = {
  __proto__: null,
  flag: "r",
};

type ReadFileTextCallback = (err: Error | null, data?: string) => void;
type ReadFileBinaryCallback = (err: Error | null, data?: Buffer) => void;
type ReadFileGenericCallback = (
  err: Error | null,
  data?: string | Buffer,
) => void;
type ReadFileCallback =
  | ReadFileTextCallback
  | ReadFileBinaryCallback
  | ReadFileGenericCallback;
type ReadFilePath = string | URL | FileHandle | number;

// Abort-capable path read (the no-signal case goes straight to
// `op_node_fs_read_file_path`): a cancel handle lets the read be interrupted.
// Returns raw bytes; the caller encodes via `op_node_fs_encode_bytes`.
async function readFileAsyncWithSignal(
  path: string,
  options: FileOptions,
): Promise<Uint8Array> {
  const flagsNumber = stringToFlags(options.flag, "options.flag");
  options.signal!.throwIfAborted();
  const cancelRid = core.createCancelHandle();
  const abortHandler = () => core.tryClose(cancelRid as number);
  options.signal![abortSignal.add](abortHandler);

  try {
    const data = await op_fs_read_file_async(
      path,
      cancelRid,
      flagsNumber,
    );
    return data;
  } finally {
    options.signal![abortSignal.remove](abortHandler);
    // always throw the abort error when aborted
    options.signal!.throwIfAborted();
  }
}

function readFileCheckAborted(signal: AbortSignal | undefined) {
  if (signal?.aborted) {
    throw new AbortError(undefined, { cause: signal.reason });
  }
}

function readFileConcatBuffers(buffers: Uint8Array[]): Uint8Array {
  let totalLen = 0;
  for (let i = 0; i < buffers.length; ++i) {
    totalLen += TypedArrayPrototypeGetByteLength(buffers[i]);
  }

  const contents = new Uint8Array(totalLen);
  let n = 0;
  for (let i = 0; i < buffers.length; ++i) {
    const buf = buffers[i];
    TypedArrayPrototypeSet(contents, buf, n);
    n += TypedArrayPrototypeGetByteLength(buf);
  }

  return contents;
}

// Abort-capable fd read (the no-signal case goes straight to
// `op_node_fs_read_file`, whose read_to_end handles unknown-size sources and
// "zero-byte liar" files). Returns raw bytes; the caller encodes via
// `op_node_fs_encode_bytes`.
async function readFileFromFdWithSignal(fd: number, options: FileOptions) {
  const signal = options.signal;
  readFileCheckAborted(signal);

  const statFields = op_node_fs_fstat_sync(fd);
  readFileCheckAborted(signal);

  const isFile = statFields.isFile;
  const size = isFile ? statFields.size : 0;

  if (size > kIoMaxLength) {
    throw new ERR_FS_FILE_TOO_LARGE(size);
  }

  if (isFile && size > 0) {
    // Known size: read into a single buffer with an advancing offset.
    // Mirrors Node's readFileHandle which avoids the subarray-aliasing trap
    // by writing successive reads into different regions of one buffer.
    const buffer = new Uint8Array(size);
    let totalRead = 0;
    while (totalRead < size) {
      readFileCheckAborted(signal);
      const slice = TypedArrayPrototypeSubarray(buffer, totalRead);
      // Use the deferred op so we yield to the event loop between reads,
      // allowing abort signals scheduled via process.nextTick to fire.
      const nread = await op_node_fs_read_deferred(fd, slice, -1n);
      if (nread === 0) break;
      totalRead += nread;
    }
    readFileCheckAborted(signal);
    return totalRead === size
      ? buffer
      : TypedArrayPrototypeSubarray(buffer, 0, totalRead);
  }

  // Unknown size (pipes, sockets, /dev/stdin): allocate a fresh buffer per
  // iteration so pushed subarrays don't alias a reused read buffer.
  const buffers: Uint8Array[] = [];
  let totalRead = 0;
  while (true) {
    readFileCheckAborted(signal);
    const chunk = new Uint8Array(kReadFileUnknownBufferLength);
    const nread = await op_node_fs_read_deferred(fd, chunk, -1n);
    if (nread === 0) break;
    totalRead += nread;
    if (totalRead > kIoMaxLength) {
      throw new ERR_FS_FILE_TOO_LARGE(totalRead);
    }
    ArrayPrototypePush(buffers, TypedArrayPrototypeSubarray(chunk, 0, nread));
  }

  return readFileConcatBuffers(buffers);
}

function readFile(
  path: ReadFilePath,
  options: TextOptionsArgument,
  callback: ReadFileTextCallback,
): void;
function readFile(
  path: ReadFilePath,
  options: BinaryOptionsArgument,
  callback: ReadFileBinaryCallback,
): void;
function readFile(
  path: ReadFilePath,
  options: null | undefined | FileOptionsArgument,
  callback: ReadFileBinaryCallback,
): void;
function readFile(
  path: string | URL,
  callback: ReadFileBinaryCallback,
): void;
function readFile(
  pathOrRid: ReadFilePath,
  optOrCallback?:
    | FileOptionsArgument
    | ReadFileCallback
    | null
    | undefined,
  callback?: ReadFileCallback,
) {
  if (
    ObjectPrototypeIsPrototypeOf(
      lazyInternalHandle().FileHandle.prototype,
      pathOrRid,
    )
  ) {
    pathOrRid = (pathOrRid as FileHandle).fd;
  }

  let cb: ReadFileCallback | undefined;
  if (typeof optOrCallback === "function") {
    cb = optOrCallback;
  } else {
    cb = callback;
  }

  const options = getOptions<FileOptions>(
    optOrCallback,
    readFileDefaultOptions,
  );

  // The common no-signal cases resolve to an already-encoded string/Buffer
  // (the ops parse options + decode); the rarer signal-capable JS paths
  // return raw bytes encoded afterwards.
  let p: Promise<string | Buffer>;
  if (!options?.signal) {
    p = typeof pathOrRid === "number"
      ? op_node_fs_read_file(pathOrRid, options)
      : op_node_fs_read_file_path(pathOrRid, options);
  } else {
    const raw = typeof pathOrRid === "number"
      ? readFileFromFdWithSignal(pathOrRid, options)
      : readFileAsyncWithSignal(
        getValidatedPathToString(pathOrRid as string),
        options,
      );
    p = PromisePrototypeThen(
      raw,
      (data: Uint8Array) => op_node_fs_encode_bytes(data, options),
    );
  }

  if (cb) {
    PromisePrototypeThen(
      p,
      (textOrBuffer) => {
        (cb as ReadFileBinaryCallback)(null, textOrBuffer);
      },
      (err) =>
        cb &&
        cb(
          denoErrorToNodeError(err, {
            path: typeof pathOrRid === "string" ? pathOrRid : undefined,
            syscall: "open",
          }),
        ),
    );
  }
}

function readFilePromise(
  path: ReadFilePath,
  options?: FileOptionsArgument | null | undefined,
): Promise<any> {
  return new Promise((resolve, reject) => {
    readFile(path, options, (err, data) => {
      if (err) reject(err);
      else resolve(data);
    });
  });
}

// Direct op binding: the op validates path + options (flag, encoding), reads
// all bytes, and returns them already encoded, throwing final node errors. A
// numeric first arg is an already-open fd (read directly, no open).
const readFileSync = op_node_fs_read_file_path_sync as {
  (path: string | URL | number, opt: TextOptionsArgument): string;
  (path: string | URL | number, opt?: BinaryOptionsArgument): Buffer;
  (path: string | URL | number, opt?: FileOptionsArgument): string | Buffer;
};

// -- readlink --

type ReadlinkCallback = (
  err: MaybeEmpty<Error>,
  linkString: MaybeEmpty<string | Uint8Array>,
) => void;

interface ReadlinkOptions {
  encoding?: string | null;
}

// The op validates the path + encoding options synchronously
// (async(eager_throw)) and returns the link target already encoded
// (string or Buffer).
const readlink = callbackifyOpt(op_node_fs_read_link);

const readlinkPromise = promisify(readlink) as (
  path: string | Buffer | URL,
  opt?: ReadlinkOptions,
) => Promise<string | Uint8Array>;

// Direct op binding: the op validates path + encoding and returns the link
// target already encoded.
const readlinkSync = op_node_fs_read_link_sync as (
  path: string | Buffer | URL,
  opt?: ReadlinkOptions,
) => string | Uint8Array;

// -- statfs --

type StatFsCallback<T> = (err: Error | null, stats?: StatFs<T>) => void;

type StatFsOptions = {
  bigint?: boolean;
};

type StatFs<T> = {
  type: T;
  bsize: T;
  blocks: T;
  bfree: T;
  bavail: T;
  files: T;
  ffree: T;
};

// The op validates the path + options synchronously (async(eager_throw)),
// throws the final node error (syscall "statfs", path), and returns the
// statfs object with Number or BigInt fields per `options.bigint`.
const statfs = callbackifyOpt(op_node_statfs);

// Direct op bindings (no JS wrapper to bake into the snapshot): the op does
// all validation + the syscall. See also chmodSync/rmSync/etc. below.
const statfsSync = op_node_statfs_sync;

function access(
  path: string | Buffer | URL,
  mode: any,
  callback?: CallbackWithError,
) {
  if (typeof mode === "function") {
    callback = mode;
    mode = undefined;
  }
  // The op validates path + mode synchronously (async(eager_throw)).
  const promise = op_node_fs_access(path, mode);
  const cb = makeCallback(callback);
  PromisePrototypeThen(promise, () => cb(null), cb);
}

const accessSync = op_node_fs_access_sync;

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

const chmod = callbackify(op_node_fs_chmod, 2);

const chmodSync = op_node_fs_chmod_sync;

const chown = callbackify(op_node_fs_chown, 3);

const chownSync = op_node_fs_chown_sync;

function defaultCloseCallback(err: Error | null) {
  if (err !== null) throw err;
}

// The async op validates the fd eagerly (getValidatedFd) and runs the close on
// a microtask (resolving the promise), so the callback fires asynchronously
// like node. No-callback close uses node's `defaultCloseCallback` (rethrows on
// error).
const close = callbackify(op_node_fs_close_async, 1, defaultCloseCallback);

const closeSync = op_node_fs_close;

const fchown = callbackify(op_node_fs_fchown, 3);

const fchmod = callbackify(op_node_fs_fchmod, 2);

const fchownSync = op_node_fs_fchown_sync;

// The op validates fd (typeof number) + len (validateInteger, default 0) and
// truncates; callbackifyOpt(.., 1) treats the optional 2nd arg as len-or-cb.
const ftruncate = callbackifyOpt(op_node_fs_ftruncate, 1);

const ftruncateSync = op_node_fs_ftruncate_sync;

const futimes = callbackify(op_node_fs_futimes, 3);

const fchmodSync = op_node_fs_fchmod_sync;

const fdatasync = callbackify(op_node_fs_fdatasync, 1);

const futimesSync = op_node_fs_futimes_sync;

const lchmod = !isMacOS ? undefined : callbackify(op_node_lchmod, 2);

const lchmodSync:
  | ((
    path: string | Buffer | URL,
    mode: number,
  ) => void)
  | undefined = !isMacOS
    ? undefined
    : (path: string | Buffer | URL, mode: number) =>
      op_node_lchmod_sync(path, mode);

const lchown = callbackify(op_node_lchown, 3);

const fdatasyncSync = op_node_fs_fdatasync_sync;

const fsync = callbackify(op_node_fs_fsync, 1);

function lchownSync(
  path: string | Buffer | URL,
  uid: number,
  gid: number,
) {
  op_node_lchown_sync(path, uid, gid);
}

const fsyncSync = op_node_fs_fsync_sync;

const link = callbackify(op_node_fs_link, 2);

function linkSync(
  existingPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) {
  op_node_fs_link_sync(existingPath, newPath);
}

const unlink = callbackify(op_node_fs_remove, 1);
const unlinkSync = op_node_fs_remove_sync;

const rename = callbackify(op_node_fs_rename, 2);

function renameSync(
  oldPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) {
  op_node_fs_rename_sync(oldPath, newPath);
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

  // The op validates `options` (node's validateRmOptions: object + force/
  // recursive/retryDelay/maxRetries types) and runs the lstat precheck
  // (ERR_FS_EISDIR / force-ENOENT) natively, emitting the final node error.
  PromisePrototypeThen(
    op_node_fs_rm(path, options),
    () => callback(null),
    callback,
  );
}

const rmSync = op_node_fs_rm_sync;

type rmdirOptions = {
  maxRetries?: number;
  recursive?: boolean;
  retryDelay?: number;
};

type rmdirCallback = (err?: Error) => void;

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

  if (options?.recursive !== undefined) {
    // The `recursive` option was deprecated and removed in Node. Throw with a
    // clear message rather than silently doing the wrong thing.
    throw new ERR_INVALID_ARG_VALUE(
      "options.recursive",
      options.recursive,
      "is no longer supported",
    );
  }

  validateFunction(callback, "cb");
  // Current node's validateRmdirOptions only validates the options object type
  // (the recursive/retryDelay/maxRetries field checks were removed upstream).
  if (options !== undefined) validateObject(options, "options");
  // The op validates the path synchronously (async(eager_throw)) and throws
  // the final node error (syscall "rmdir", path).
  PromisePrototypeThen(
    op_node_rmdir(path),
    (_) => callback(),
    callback,
  );
}

const rmdirSync = op_node_rmdir_sync;

// -- mkdir --

type MkdirCallback =
  | ((err: Error | null, path?: string) => void)
  | CallbackWithError;

type MkdirOptions =
  | { recursive?: boolean; mode?: number | undefined }
  | number
  | boolean;

// The op validates path + options, computes the recursive `mkdirp` return path
// (passed as the 2nd callback arg), and throws the final node error.
const mkdir = callbackifyOpt(op_node_fs_mkdir);

// Direct op binding: the op validates path + options, creates the dir(s), and
// returns the first created path (recursive) or undefined.
const mkdirSync = op_node_fs_mkdir_sync as (
  path: string | URL,
  options?: MkdirOptions,
) => string | undefined;

// -- mkdtemp --

type MkdtempCallback = (
  err: Error | null,
  directory?: string,
) => void;
type MkdtempBufferCallback = (
  err: Error | null,
  directory?: Buffer<ArrayBufferLike>,
) => void;

// The op validates the prefix + encoding options synchronously
// (async(eager_throw)), emits node's one-time non-portable-template warning
// (templates ending in 'X'), creates the dir, and returns the path already
// encoded (string or Buffer). It throws the final node error (syscall
// "mkdtemp"). callbackifyOpt handles the (prefix, options?, cb) shuffle.
const mkdtemp = callbackifyOpt(op_node_mkdtemp, 1) as {
  (prefix: string | Buffer | Uint8Array | URL, callback: MkdtempCallback): void;
  (
    prefix: string | Buffer | Uint8Array | URL,
    options: { encoding: "buffer" } | "buffer",
    callback: MkdtempBufferCallback,
  ): void;
  (
    prefix: string | Buffer | Uint8Array | URL,
    options: { encoding: string } | string,
    callback: MkdtempCallback,
  ): void;
};

// Direct op binding: the op validates prefix + options, emits the one-time
// warning, creates the dir, and returns the path already encoded.
const mkdtempSync = op_node_mkdtemp_sync as {
  (
    prefix: string | Buffer | Uint8Array | URL,
    options?: { encoding: "buffer" } | "buffer",
  ): Buffer<ArrayBufferLike>;
  (
    prefix: string | Buffer | Uint8Array | URL,
    options?: { encoding: string } | string,
  ): string;
};

// Mirrors Node's lib/fs.js mkdtempDisposableSync(): create the temp dir and
// return an object with .path, .remove(), and Symbol.dispose. cwd is captured
// at creation time so a later process.chdir() doesn't break removal.
function mkdtempDisposableSync(
  prefix: string | Buffer | Uint8Array | URL,
  options?: { encoding: string } | string,
) {
  const cwd = process.cwd();
  const path = mkdtempSync(prefix, options as { encoding: string }) as string;
  const fullPath = resolve(cwd, path);
  const remove = () => {
    rmSync(fullPath, {
      force: true,
      maxRetries: 0,
      recursive: true,
      retryDelay: 0,
    });
  };
  return {
    path,
    remove,
    [SymbolDispose]() {
      remove();
    },
  };
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

// `open(path, flags?, mode?, callback)`: the op validates the path
// (getValidatedPathToString), parses flags (default "r") + mode (default 0o666)
// synchronously -- throwing ERR_INVALID_ARG_* at the call site like node --
// then opens asynchronously and emits the final node error (syscall "open",
// path). callbackifyOpt forwards the supplied leading args (path + any of
// flags/mode), letting the op default the rest, and passes the resolved fd to
// the callback.
const open = callbackifyOpt(op_node_open, 1) as {
  (path: string | Buffer | URL, callback: OpenCallback): void;
  (path: string | Buffer | URL, flags: OpenFlags, callback: OpenCallback): void;
  (
    path: string | Buffer | URL,
    flags: OpenFlags,
    mode: number,
    callback: OpenCallback,
  ): void;
};

// Direct op binding: the op validates path, parses flags (default "r") + mode,
// opens, and returns the fd. No JS wrapper to bake into the snapshot.
const openSync = op_node_open_sync;

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

function opendir(
  path: string | Buffer | URL,
  options: OpendirOptions | OpendirCallback,
  callback?: OpendirCallback,
) {
  callback = typeof options === "function" ? options : callback;
  _opendirValidateFunction(callback);

  // Path type errors must throw synchronously (node validates the path before
  // the async open), so validate here; bufferSize + the dir probe go through
  // the op and are surfaced via the callback.
  path = getValidatedPathToString(path);
  const opts = getOptions(options, { encoding: "utf8" });
  const bufferSize = opts.bufferSize === undefined ? 32 : opts.bufferSize;
  const recursive = opts.recursive ?? false;

  let err, dir;
  try {
    // Validates bufferSize, probes the dir, throws the node error ("opendir").
    op_node_fs_opendir_sync(path, bufferSize);
    dir = new Dir(path, recursive);
  } catch (error) {
    err = error as Error;
  }
  if (err) {
    callback(err);
  } else {
    callback(null, dir);
  }
}

function opendirSync(
  path: string | Buffer | URL,
  options?: OpendirOptions,
): Dir {
  path = getValidatedPathToString(path);
  const opts = getOptions(options, { encoding: "utf8" });
  const bufferSize = opts.bufferSize === undefined ? 32 : opts.bufferSize;
  const recursive = opts.recursive ?? false;
  // Validates bufferSize, probes the dir, throws the node error ("opendir").
  op_node_fs_opendir_sync(path, bufferSize);
  return new Dir(path, recursive);
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
    (data: Uint8Array) => markFileBackedBlob(new Blob([data], { type })),
  );
}

// -- write --

type WriteCallback = (
  err: ErrnoException | null,
  written?: number,
  strOrBuffer?: string | ArrayBufferView,
) => void;

type WriteOptions = {
  offset?: number;
  length?: number;
  position?: number | null;
};

// Direct op binding: the full overload resolution (buffer vs string,
// options-object form, offset/length/position defaults, validation, string
// encoding via `Buffer.from` + `validateEncoding`) lives in the native op, so
// `writeSync(fd, buffer, offsetOrOptions?, length?, position?)` is just the op.
const writeSync = op_node_fs_write_v_sync as (
  fd: number,
  buffer: ArrayBufferView | string,
  offsetOrOptions?: number | WriteOptions | null,
  length?: number | null,
  position?: number | null,
) => number;

/** Writes the buffer to the file of the given descriptor.
 * https://nodejs.org/api/fs.html#fswritefd-buffer-offset-length-position-callback
 * https://github.com/nodejs/node/blob/42ad4137aadda69c51e1df48eee9bc2e5cebca5c/lib/fs.js#L797
 */
// The overload resolution, validation and string encoding live in the native
// op (`op_node_fs_write_v`), which validates synchronously. callbackifyWrite
// forwards the whole argument list (the callback included, since the op uses
// the trailing slots to disambiguate the string overload), locates the
// callback, and re-attaches the original `buffer`/`string` to the completion.
const write = callbackifyWrite(op_node_fs_write_v) as (
  fd: number,
  buffer: ArrayBufferView | string,
  offsetOrOptions?: number | WriteOptions | WriteCallback | null,
  length?: number | WriteCallback | null,
  position?: number | WriteCallback | null,
  callback?: WriteCallback,
) => void;

ObjectDefineProperty(write, customPromisifyArgs, {
  __proto__: null,
  value: ["bytesWritten", "buffer"],
  enumerable: false,
});

// -- writev --

interface WriteVResult {
  bytesWritten: number;
  buffers: ReadonlyArray<ArrayBufferView>;
}

type writeVCallback = (
  err: ErrnoException | null,
  bytesWritten: number,
  buffers: ReadonlyArray<ArrayBufferView>,
) => void;

/**
 * Write an array of `ArrayBufferView`s to the file specified by `fd` using`writev()`.
 *
 * `position` is the offset from the beginning of the file where this data
 * should be written. If `typeof position !== 'number'`, the data will be written
 * at the current position.
 *
 * The callback will be given three arguments: `err`, `bytesWritten`, and`buffers`. `bytesWritten` is how many bytes were written from `buffers`.
 *
 * If this method is `util.promisify()` ed, it returns a promise for an`Object` with `bytesWritten` and `buffers` properties.
 *
 * It is unsafe to use `fs.writev()` multiple times on the same file without
 * waiting for the callback. For this scenario, use {@link createWriteStream}.
 *
 * On Linux, positional writes don't work when the file is opened in append mode.
 * The kernel ignores the position argument and always appends the data to
 * the end of the file.
 * @since v12.9.0
 */
// The op validates fd/buffers/position synchronously (async(eager_throw)),
// short-circuits empty buffer lists, then gathers the views and writes them
// at `position` (-1 = current). callbackifyWrite locates the callback and
// re-attaches the original `buffers` to the completion (node's `wrapper`).
const writev = callbackifyWrite(op_node_fs_writev) as (
  fd: number,
  buffers: ReadonlyArray<ArrayBufferView>,
  position?: number | null,
  callback?: writeVCallback,
) => void;

/**
 * For detailed information, see the documentation of the asynchronous version of
 * this API: {@link writev}.
 * @since v12.9.0
 * @return The number of bytes written.
 */
// Direct op binding: the op validates + gathers + writes, returning the bytes
// written.
const writevSync = op_node_fs_writev_sync as (
  fd: number,
  buffers: ArrayBufferView[],
  position?: number | null,
) => number;

// -- writeFile --

type WriteFileSyncData =
  | string
  | DataView
  | NodeJS.TypedArray
  | Iterable<NodeJS.TypedArray | string>;

type WriteFileData =
  | string
  | DataView
  | NodeJS.TypedArray
  | AsyncIterable<NodeJS.TypedArray | string>;

const {
  kWriteFileMaxChunkSize,
} = fsUtilConstants;

interface Writer {
  write(p: NodeJS.TypedArray): Promise<number>;
  writeSync(p: NodeJS.TypedArray): number;
}

async function _writeFileGetRid(
  pathOrRid: string | number,
  flag: string = "w",
): Promise<number> {
  if (typeof pathOrRid === "number") {
    return pathOrRid;
  }
  // The op parses the flag and emits the final node error (syscall "open",
  // path).
  return await op_node_open(pathOrRid, flag, 0o666);
}

function writeFile(
  pathOrRid: string | number | URL | FileHandle,
  data: WriteFileData,
  options: Encodings | CallbackWithError | WriteFileOptions | undefined,
  callback?: CallbackWithError,
) {
  let flag: string | undefined;
  let mode: number | undefined;
  let signal: AbortSignal | undefined;

  if (typeof options === "function") {
    callback = options;
    options = undefined;
  }

  validateFunction(callback, "callback");

  if (ObjectPrototypeIsPrototypeOf(URLPrototype, pathOrRid)) {
    pathOrRid = pathFromURL(pathOrRid as URL);
  } else if (
    ObjectPrototypeIsPrototypeOf(
      lazyInternalHandle().FileHandle.prototype,
      pathOrRid,
    )
  ) {
    pathOrRid = (pathOrRid as FileHandle).fd;
  }

  if (isFileOptions(options)) {
    flag = options.flag;
    mode = options.mode;
    signal = options.signal;
  }

  const isRid = typeof pathOrRid === "number";

  // Common case (non-iterable data, no AbortSignal), path or fd: the op
  // parses options + data (string -> encoded bytes), opens, optionally
  // chmods, writes all bytes, and closes natively, throwing the final node
  // error. The iterable / signal cases fall through to JS.
  if (!signal && !_isCustomIterable(data)) {
    PromisePrototypeThen(
      op_node_fs_write_file(pathOrRid, data, options),
      () => callback(null),
      callback,
    );
    return;
  }

  const encoding = getValidatedEncoding(options) || "utf8";

  if (!ArrayBufferIsView(data) && !_isCustomIterable(data)) {
    validateStringAfterArrayBufferView(data, "data");
    data = Buffer.from(data, encoding);
  }

  let file;

  let error: Error | null = null;
  (async () => {
    try {
      const fd = await _writeFileGetRid(pathOrRid as string | number, flag);
      file = {
        write(p: NodeJS.TypedArray) {
          // Use the deferred op to yield to the event loop between writes,
          // allowing abort signals scheduled via process.nextTick to fire.
          return op_node_fs_write_deferred(fd, p, -1);
        },
        writeSync(p: NodeJS.TypedArray) {
          return op_node_fs_write_sync(fd, p, -1);
        },
        close() {
          op_node_fs_close(fd);
        },
      };
      _checkAborted(signal);

      if (!isRid && mode) {
        await op_node_fs_chmod(pathOrRid as string, mode);
        _checkAborted(signal);
      }

      await _writeAll(
        file,
        data as (Exclude<WriteFileData, string>),
        encoding,
        signal,
      );
    } catch (e) {
      error = denoWriteFileErrorToNodeError(e as Error, { syscall: "write" });
    } finally {
      // Make sure to close resource
      if (!isRid && file) file.close();
      callback(error);
    }
  })();
}

// Direct op binding: the op parses options (encoding/flag/mode) and data
// (string -> encoded bytes; only string or ArrayBufferView are accepted,
// matching node's sync variant), opens (path case), optionally chmods, writes
// all bytes, and closes, throwing the final node error.
const writeFileSync = op_node_fs_write_file_sync as (
  pathOrRid: string | number | URL,
  data: WriteFileSyncData,
  options?: Encodings | WriteFileOptions,
) => void;

async function _writeAll(
  w: Writer,
  data: Exclude<WriteFileData, string>,
  encoding: BufferEncoding,
  signal?: AbortSignal,
) {
  if (!_isCustomIterable(data)) {
    // deno-lint-ignore prefer-primordials
    data = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    // deno-lint-ignore prefer-primordials
    let remaining = data.byteLength;
    while (remaining > 0) {
      const writeSize = MathMin(kWriteFileMaxChunkSize, remaining);
      // deno-lint-ignore prefer-primordials
      const offset = data.byteLength - remaining;
      const bytesWritten = await w.write(
        data.subarray(offset, offset + writeSize),
      );
      remaining -= bytesWritten;
      _checkAborted(signal);
    }
  } else {
    // deno-lint-ignore prefer-primordials
    for await (const buf of data) {
      _checkAborted(signal);
      let toWrite = ArrayBufferIsView(buf) ? buf : Buffer.from(buf, encoding);
      toWrite = new Uint8Array(
        // deno-lint-ignore prefer-primordials
        toWrite.buffer,
        // deno-lint-ignore prefer-primordials
        toWrite.byteOffset,
        // deno-lint-ignore prefer-primordials
        toWrite.byteLength,
      );
      // deno-lint-ignore prefer-primordials
      let remaining = toWrite.byteLength;
      while (remaining > 0) {
        const writeSize = MathMin(kWriteFileMaxChunkSize, remaining);
        // deno-lint-ignore prefer-primordials
        const offset = toWrite.byteLength - remaining;
        const bytesWritten = await w.write(
          toWrite.subarray(offset, offset + writeSize),
        );
        remaining -= bytesWritten;
        _checkAborted(signal);
      }
    }
  }

  _checkAborted(signal);
}

function _isCustomIterable(
  obj: unknown,
): obj is
  | Iterable<NodeJS.TypedArray | string>
  | AsyncIterable<NodeJS.TypedArray | string> {
  return isIterable(obj) && !ArrayBufferIsView(obj) &&
    typeof obj !== "string";
}

function _checkAborted(signal?: AbortSignal) {
  if (signal?.aborted) {
    throw new AbortError();
  }
}

// -- truncate --

// The op validates path + len (validateInteger, default 0), opens 'r+',
// ftruncates, and emits node errors; callbackifyOpt(.., 1) treats the optional
// 2nd arg as len-or-cb.
const truncate = callbackifyOpt(op_node_fs_truncate, 1);

const truncateSync = op_node_fs_truncate_sync;

// -- utimes --

const utimes = callbackify(op_node_fs_utime, 3);

const utimesSync = op_node_fs_utime_sync;

// -- symlink --

type SymlinkType = "file" | "dir" | "junction";

// The op validates target/path/type synchronously (async(eager_throw)) and,
// on Windows with no explicit type, auto-detects "dir" vs "file" by statting
// the target resolved relative to the new link's parent (matching node).
// callbackifyOpt(.., 2): target + path are fixed, the optional 3rd arg is the
// link type (or the callback). The op validates target/path/type and does the
// Windows dir/file autodetect.
const symlink = callbackifyOpt(op_node_fs_symlink, 2);

function symlinkSync(
  target: string | Buffer | URL,
  path: string | Buffer | URL,
  type?: SymlinkType,
) {
  op_node_fs_symlink_sync(target, path, type);
}

// -- watch --

const statPromisified = promisify(stat) as {
  (filename: string, options: { bigint: false }): Promise<Stats>;
  (filename: string, options: { bigint: true }): Promise<BigIntStats>;
};
const statAsync = async (
  filename: string,
  bigint: boolean,
): Promise<Stats | BigIntStats> => {
  try {
    return bigint
      ? await statPromisified(filename, { bigint: true })
      : await statPromisified(filename, { bigint: false });
  } catch {
    return bigint ? emptyBigIntStats() : emptyStats();
  }
};
let _emptyStats: Stats | undefined;
let _emptyBigIntStats: BigIntStats | undefined;
// Lazy: cppgc `Stats` cannot be constructed at snapshot-build time, and fs.ts
// is loaded during the snapshot build.
const emptyStats = (): Stats =>
  (_emptyStats ??= op_node_fs_empty_stats(false)) as unknown as Stats;
const emptyBigIntStats = (): BigIntStats =>
  (_emptyBigIntStats ??= op_node_fs_empty_stats(
    true,
  )) as unknown as BigIntStats;

// Mirrors libuv's `uv_fs_poll_t` field comparison so chmod/chown,
// file replacement, and sub-mtime-resolution changes all fire "change".
function asyncIterableToCallback<T>(
  iter: AsyncIterable<T>,
  callback: (val: T, done?: boolean) => void,
  errCallback: (e: unknown) => void,
) {
  const iterator = iter[SymbolAsyncIterator]();
  function next() {
    // deno-lint-ignore prefer-primordials
    PromisePrototypeThen(iterator.next(), (obj: IteratorResult<T>) => {
      if (obj.done) {
        callback(obj.value, true);
        return;
      }
      callback(obj.value);
      next();
    }, errCallback);
  }
  next();
}

// Mirrors Node's `validateIgnoreOption` /
// `createIgnoreMatcher` from `lib/internal/fs/watchers.js`.
// Accepts a string (minimatch glob), RegExp, function, or array of those.
// Returns a function `(filename) => boolean` (or `null` if `ignore` is nullish).
type IgnoreOption =
  | string
  | RegExp
  | ((filename: string) => boolean)
  | (string | RegExp | ((filename: string) => boolean))[]
  | undefined
  | null;

let _lazyMinimatch: any = null;
function getMinimatch() {
  _lazyMinimatch ??= core.createLazyLoader("ext:deno_node/deps/minimatch.js");
  return _lazyMinimatch();
}

function createIgnoreMatcher(
  ignore: IgnoreOption,
): ((filename: string) => boolean) | null {
  if (ignore == null) return null;
  const matchers = ArrayIsArray(ignore) ? ignore : [ignore];
  const compiled: Array<(filename: string) => boolean> = [];

  for (let i = 0; i < matchers.length; i++) {
    const matcher = matchers[i];
    if (typeof matcher === "string") {
      const { Minimatch } = getMinimatch().default;
      const mm = new Minimatch(matcher, {
        nocase: isMacOS || isWindows,
        windowsPathsNoEscape: true,
        nonegate: true,
        nocomment: true,
        optimizationLevel: 2,
        platform: isWindows ? "win32" : "posix",
        // Allow patterns without slashes to match the basename
        // e.g. '*.log' matches 'subdir/file.log'.
        matchBase: true,
      });
      ArrayPrototypePush(
        compiled,
        // deno-lint-ignore prefer-primordials
        (filename: string) => mm.match(filename),
      );
    } else if (ObjectPrototypeIsPrototypeOf(RegExpPrototype, matcher)) {
      ArrayPrototypePush(
        compiled,
        (filename: string) => RegExpPrototypeTest(matcher as RegExp, filename),
      );
    } else {
      // Function
      ArrayPrototypePush(compiled, matcher as (filename: string) => boolean);
    }
  }

  return (filename: string) => {
    for (let i = 0; i < compiled.length; i++) {
      if (compiled[i](filename)) return true;
    }
    return false;
  };
}

type watchOptions = {
  persistent?: boolean;
  recursive?: boolean;
  encoding?: string;
  signal?: AbortSignal;
  ignore?: IgnoreOption;
};

type watchListener = (
  eventType: string,
  filename: string | Buffer,
) => void;

function watch(
  filename: string | URL,
  options: watchOptions,
  listener: watchListener,
): FSWatcher;
function watch(
  filename: string | URL,
  listener: watchListener,
): FSWatcher;
function watch(
  filename: string | URL,
  options: watchOptions,
): FSWatcher;
function watch(filename: string | URL): FSWatcher;
function watch(
  filename: string | URL,
  optionsOrListener?: watchOptions | watchListener,
  optionsOrListener2?: watchOptions | watchListener,
) {
  const listener = typeof optionsOrListener === "function"
    ? optionsOrListener
    : typeof optionsOrListener2 === "function"
    ? optionsOrListener2
    : undefined;
  const options = typeof optionsOrListener === "object"
    ? optionsOrListener
    : typeof optionsOrListener2 === "object"
    ? optionsOrListener2
    : undefined;

  op_node_fs_validate_watch_ignore(options?.ignore, "options.ignore");

  // deno-lint-ignore prefer-primordials
  const watchPath = getValidatedPath(filename).toString();

  // Match Node: validate non-boolean `recursive`/`persistent` up front.
  // https://github.com/nodejs/node/blob/main/lib/internal/fs/recursive_watch.js
  if (options != null && options.recursive != null) {
    validateBoolean(options.recursive, "options.recursive");
  }
  if (options != null && options.persistent != null) {
    validateBoolean(options.persistent, "options.persistent");
  }
  const recursive = options?.recursive || false;
  const encoding = options?.encoding;
  op_node_fs_validate_watch_ignore(options?.ignore, "options.ignore");
  const ignoreMatcher = createIgnoreMatcher(options?.ignore);

  // Open the underlying Deno.FsWatcher, but defer any failure to an
  // 'error' event on the returned FSWatcher rather than throwing
  // synchronously with a raw `Deno.errors.NotFound`. Editors that
  // atomically save (write to <file>.tmp.<pid>.<ts> then rename over
  // the original) can race the inotify watch and produce a transient
  // ENOENT here; callers using EventEmitter-style error handling
  // (chokidar, vite) can't recover from a sync throw of a Deno error.
  // See denoland/deno#34396.
  const notFoundProto = Deno.errors.NotFound.prototype;
  const makeWatchNodeError = (e: unknown): Error => {
    // The notify crate's PathNotFound/WatchNotFound error messages don't
    // include the "(os error N)" suffix that `denoErrorToNodeError` parses,
    // so detect NotFound by class and build a Node-style ENOENT manually.
    if (ObjectPrototypeIsPrototypeOf(notFoundProto, e)) {
      return uvException({
        errno: codeMap.get("ENOENT")!,
        syscall: "watch",
        path: watchPath,
      });
    }
    return denoErrorToNodeError(e as Error, {
      syscall: "watch",
      path: watchPath,
    });
  };

  let iterator: Deno.FsWatcher | undefined;
  let openError: Error | undefined;
  let resolvedWatchPath = watchPath;
  try {
    // Pre-validate path existence so missing-path failures surface as a
    // typed `Deno.errors.NotFound` consistently across platforms. notify
    // 6.1.1's Windows backend (`add_watch` in src/windows.rs) returns a
    // Generic error rather than a typed NotFound when the path doesn't
    // exist, which would otherwise bypass the prototype check in
    // makeWatchNodeError above.
    Deno.lstatSync(watchPath);
    iterator = Deno.watchFs(watchPath, { recursive });
    // Resolve the watched path once so we can compute relative paths.
    // Use realPathSync to resolve symlinks (e.g. macOS /var -> /private/var)
    // since Deno.watchFs returns real (symlink-resolved) paths.
    resolvedWatchPath = realpathSync(watchPath) as string;
  } catch (e) {
    if (iterator) {
      try {
        iterator.close();
      } catch { /* ignore */ }
      iterator = undefined;
    }
    openError = makeWatchNodeError(e);
  }

  if (iterator) {
    asyncIterableToCallback<Deno.FsEvent>(iterator, (val, done) => {
      if (done) return;
      // Node.js returns the relative path from the watched directory for
      // recursive watches, but just the basename for non-recursive watches.
      const filename = recursive
        ? relative(resolvedWatchPath, val.paths[0])
        : basename(val.paths[0]);
      if (ignoreMatcher !== null && ignoreMatcher(filename)) {
        return;
      }
      fsWatcher.emit(
        "change",
        convertDenoFsEventToNodeFsEvent(val.kind),
        op_node_fs_encode_watch_filename(filename, encoding),
      );
    }, (e) => {
      fsWatcher.emit("error", makeWatchNodeError(e));
    });
  }

  const fsWatcher = new FSWatcher(() => {
    if (!iterator) return;
    try {
      iterator.close();
    } catch (e) {
      if (
        ObjectPrototypeIsPrototypeOf(Deno.errors.BadResource.prototype, e)
      ) {
        // already closed
        return;
      }
      throw e;
    }
  }, () => iterator);

  if (listener) {
    fsWatcher.on(
      "change",
      FunctionPrototypeBind(listener, { _handle: fsWatcher }),
    );
  }

  // Match Node's `fs.watch` AbortSignal handling:
  // https://github.com/nodejs/node/blob/main/lib/fs.js
  validateAbortSignal(options?.signal, "options.signal");
  if (options?.signal) {
    const signal = options.signal;
    if (signal.aborted) {
      process.nextTick(() => fsWatcher.close());
    } else {
      const onAbort = () => fsWatcher.close();
      signal.addEventListener("abort", onAbort, { once: true });
      fsWatcher.once("close", () => {
        signal.removeEventListener("abort", onAbort);
      });
    }
  }

  if (openError) {
    process.nextTick(() => {
      fsWatcher.emit("error", openError);
    });
  }

  return fsWatcher;
}

function watchPromise(
  filename: string | Buffer | URL,
  options?: {
    persistent?: boolean;
    recursive?: boolean;
    encoding?: string;
    signal?: AbortSignal;
    ignore?: IgnoreOption;
  },
): AsyncIterable<{ eventType: string; filename: string | Buffer | null }> {
  // deno-lint-ignore prefer-primordials
  const watchPath = getValidatedPath(filename).toString();

  const recursive = options?.recursive ?? false;
  const signal = options?.signal;
  validateAbortSignal(signal, "options.signal");
  op_node_fs_validate_watch_ignore(options?.ignore, "options.ignore");
  const ignoreMatcher = createIgnoreMatcher(options?.ignore);
  const watcher = Deno.watchFs(watchPath, {
    recursive,
  });
  const resolvedWatchPath = realpathSync(watchPath) as string;

  let onAbort: (() => void) | null = null;
  function cleanupAbort() {
    if (signal && onAbort) {
      signal.removeEventListener("abort", onAbort);
      onAbort = null;
    }
  }

  if (signal) {
    if (signal.aborted) {
      watcher.close();
    } else {
      onAbort = () => watcher.close();
      signal.addEventListener("abort", onAbort, { once: true });
    }
  }

  // Match Node: surface signal abort as a thrown AbortError carrying
  // `signal.reason` as `cause`.
  // https://github.com/nodejs/node/blob/main/lib/internal/fs/watchers.js
  function abortError(): AbortError {
    return new AbortError(undefined, { cause: signal?.reason });
  }

  const fsIterable = watcher[SymbolAsyncIterator]();
  const result = {
    async next(): Promise<
      IteratorResult<{ eventType: string; filename: string | Buffer | null }>
    > {
      if (signal?.aborted) {
        cleanupAbort();
        throw abortError();
      }
      while (true) {
        // deno-lint-ignore prefer-primordials
        const iterResult = await fsIterable.next();
        if (iterResult.done) {
          cleanupAbort();
          if (signal?.aborted) {
            throw abortError();
          }
          return iterResult;
        }

        const eventType = convertDenoFsEventToNodeFsEvent(
          iterResult.value.kind,
        );
        const fname = recursive
          ? relative(resolvedWatchPath, iterResult.value.paths[0])
          : basename(iterResult.value.paths[0]);
        if (ignoreMatcher !== null && ignoreMatcher(fname)) {
          continue;
        }
        return {
          value: { eventType, filename: fname },
          done: false,
        };
      }
    },
    return(value?: any): Promise<IteratorResult<any>> {
      cleanupAbort();
      watcher.close();
      return PromiseResolve({ value, done: true });
    },
    [SymbolAsyncIterator]() {
      return this;
    },
  };

  return result;
}

type WatchFileListener = (curr: Stats, prev: Stats) => void;
type WatchFileOptions = {
  bigint?: boolean;
  persistent?: boolean;
  interval?: number;
};

function watchFile(
  filename: string | Buffer | URL,
  listener: WatchFileListener,
): StatWatcher;
function watchFile(
  filename: string | Buffer | URL,
  options: WatchFileOptions,
  listener: WatchFileListener,
): StatWatcher;
function watchFile(
  filename: string | Buffer | URL,
  listenerOrOptions: WatchFileListener | WatchFileOptions,
  listener?: WatchFileListener,
): StatWatcher {
  // deno-lint-ignore prefer-primordials
  const watchPath = getValidatedPath(filename).toString();
  const handler = typeof listenerOrOptions === "function"
    ? listenerOrOptions
    : listener!;
  validateFunction(handler, "listener");
  const {
    bigint = false,
    persistent = true,
    interval = 5007,
  } = typeof listenerOrOptions === "object" ? listenerOrOptions : {};

  let watcher = MapPrototypeGet(statWatchers, watchPath);
  if (watcher === undefined) {
    watcher = new StatWatcher(bigint);
    watcher[kFSStatWatcherStart](watchPath, persistent, interval);
    MapPrototypeSet(statWatchers, watchPath, watcher);
  }

  watcher.addListener("change", handler);
  return watcher;
}

function unwatchFile(
  filename: string | Buffer | URL,
  listener?: WatchFileListener,
) {
  // deno-lint-ignore prefer-primordials
  const watchPath = getValidatedPath(filename).toString();
  const watcher = MapPrototypeGet(statWatchers, watchPath);

  if (!watcher) {
    return;
  }

  if (typeof listener === "function") {
    const beforeListenerCount = watcher.listenerCount("change");
    watcher.removeListener("change", listener);
    if (watcher.listenerCount("change") < beforeListenerCount) {
      watcher[kFSStatWatcherAddOrCleanRef]("clean");
    }
  } else {
    watcher.removeAllListeners("change");
    watcher[kFSStatWatcherAddOrCleanRef]("cleanAll");
  }

  if (watcher.listenerCount("change") === 0) {
    watcher.stop();
    MapPrototypeDelete(statWatchers, watchPath);
  }
}

const statWatchers = new SafeMap<string, StatWatcher>();

const kFSStatWatcherStart = SymbolFor("kFSStatWatcherStart");
const kFSStatWatcherAddOrCleanRef = SymbolFor("kFSStatWatcherAddOrCleanRef");

class StatWatcher extends EventEmitter {
  #bigint: boolean;
  #refCount = 0;
  #abortController = new AbortController();
  #refed = true;
  // The current in-flight interval timer, ref'd / unref'd when ref()/unref()
  // is called between polls so the process can exit while nothing is changing.
  #timer: Timeout | null = null;

  constructor(bigint: boolean) {
    super();
    this.#bigint = bigint;
  }
  [kFSStatWatcherStart](
    filename: string,
    persistent: boolean,
    interval: number,
  ) {
    if (persistent) {
      this.#refCount++;
    }

    const bigint = this.#bigint;
    (async () => {
      let prev = await statAsync(filename, bigint);

      // libuv emits an initial "change" only when the first stat fails.
      if (prev === emptyStats() || prev === emptyBigIntStats()) {
        this.emit("change", prev, prev);
      }

      try {
        while (true) {
          await this.#sleep(interval);
          const curr = await statAsync(filename, bigint);
          if (op_node_fs_stats_changed(prev, curr)) {
            this.emit("change", curr, prev);
            prev = curr;
          }
        }
      } catch (e) {
        if (
          ObjectPrototypeIsPrototypeOf(DOMException.prototype, e) &&
          e.name === "AbortError"
        ) {
          return;
        }
        this.emit("error", e);
      }
    })();
  }
  #sleep(ms: number): Promise<void> {
    return new Promise((resolve, reject) => {
      const signal = this.#abortController.signal;
      if (signal.aborted) {
        reject(signal.reason);
        return;
      }
      const abort = () => {
        clearTimeout(timer);
        this.#timer = null;
        reject(signal.reason);
      };
      const done = () => {
        signal.removeEventListener("abort", abort);
        this.#timer = null;
        resolve();
      };
      const timer = setTimeout(done, ms);
      if (!this.#refed) {
        timer.unref();
      }
      this.#timer = timer;
      signal.addEventListener("abort", abort, { once: true });
    });
  }
  [kFSStatWatcherAddOrCleanRef](addOrClean: "add" | "clean" | "cleanAll") {
    if (addOrClean === "add") {
      this.#refCount++;
    } else if (addOrClean === "clean") {
      this.#refCount--;
    } else {
      this.#refCount = 0;
    }
  }
  stop() {
    if (this.#abortController.signal.aborted) {
      return;
    }
    this.#abortController.abort();
    // Match Node: stop fires asynchronously so listeners removed
    // synchronously after stop() are not called (see
    // StatWatcher.prototype.stop in lib/internal/fs/watchers.js).
    process.nextTick(() => this.emit("stop"));
  }
  // Node's ref/unref toggle whether the StatWatcher's internal handle keeps
  // the event loop alive (see lib/internal/fs/watchers.js). In Deno the
  // handle is the interval Timeout used between poll iterations, so we
  // ref/unref that.
  ref() {
    this.#refed = true;
    this.#timer?.ref();
    return this;
  }
  unref() {
    this.#refed = false;
    this.#timer?.unref();
    return this;
  }
}

class FSWatcher extends EventEmitter {
  #closer: () => void;
  #closed = false;
  #watcher: () => Deno.FsWatcher | undefined;

  constructor(
    closer: () => void,
    getter: () => Deno.FsWatcher | undefined,
  ) {
    super();
    this.#closer = closer;
    this.#watcher = getter;
  }
  close() {
    if (this.#closed) {
      return;
    }
    this.#closed = true;
    this.emit("close");
    this.#closer();
  }
  ref() {
    this.#watcher()?.ref();
  }
  unref() {
    this.#watcher()?.unref();
  }
}

type NodeFsEventType = "rename" | "change";

function convertDenoFsEventToNodeFsEvent(
  kind: Deno.FsEvent["kind"],
): NodeFsEventType {
  if (kind === "create" || kind === "remove") {
    return "rename";
  } else if (kind === "rename") {
    return "rename";
  } else {
    return "change";
  }
}

// Match Node: the public `fs.Stats` export is deprecated (DEP0180).
// Internal call sites use the un-deprecated `Stats` directly (see
// emptyStats above). See lib/internal/fs/utils.js `Stats: deprecate(...)`.
const DeprecatedStats = deprecate(
  Stats,
  "fs.Stats constructor is deprecated.",
  "DEP0180",
);

return {
  // For tests
  _toUnixTimestamp,
  access,
  accessSync,
  appendFile,
  appendFileSync,
  BigIntStats,
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
  get createReadStream() {
    return _createReadStream ??
      (_createReadStream = lazyInternalStreams().createReadStream);
  },
  set createReadStream(v) {
    _createReadStream = v;
  },
  get createWriteStream() {
    return _createWriteStream ??
      (_createWriteStream = lazyInternalStreams().createWriteStream);
  },
  set createWriteStream(v) {
    _createWriteStream = v;
  },
  Dir,
  Dirent,
  exists,
  existsSync,
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
  mkdtempDisposableSync,
  mkdtempSync,
  open,
  openAsBlob,
  opendir,
  opendirSync,
  openSync,
  get promises() {
    return _promises ?? (_promises = lazyInternalPromises().default);
  },
  set promises(v) {
    _promises = v;
  },
  read,
  readdir,
  readdirSync,
  readFile,
  readFilePromise,
  readFileSync,
  readlink,
  readlinkPromise,
  readlinkSync,
  get ReadStream() {
    return _ReadStream ?? (_ReadStream = lazyInternalStreams().ReadStream);
  },
  set ReadStream(v) {
    _ReadStream = v;
  },
  readSync,
  readv,
  readvPromise,
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
  Stats: DeprecatedStats,
  statfs,
  statfsSync,
  statSync,
  symlink,
  symlinkSync,
  SyncWriteStream,
  truncate,
  truncateSync,
  unlink,
  unlinkSync,
  unwatchFile,
  get Utf8Stream() {
    return lazyUtf8Stream().default;
  },
  utimes,
  utimesSync,
  watch,
  watchFile,
  watchPromise,
  write,
  writeFile,
  writeFileSync,
  get WriteStream() {
    return _WriteStream ?? (_WriteStream = lazyInternalStreams().WriteStream);
  },
  set WriteStream(v) {
    _WriteStream = v;
  },
  writeSync,
  writev,
  writevSync,
};
})();
