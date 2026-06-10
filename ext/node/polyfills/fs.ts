// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-explicit-any

(function () {
const { core, primordials } = __bootstrap;
const { BadResourcePrototype } = core;
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
  getSignal,
  getValidatedEncoding,
  isFd,
  isFileOptions,
  makeCallback,
} = core.loadExtScript("ext:deno_node/_fs/_fs_common.ts");
type Encodings = any;
const {
  AbortError,
  denoErrorToNodeError,
  denoWriteFileErrorToNodeError,
  ERR_DIR_CLOSED,
  ERR_DIR_CONCURRENT_OPERATION,
  ERR_FS_FILE_TOO_LARGE,
  ERR_INVALID_THIS,
  ERR_MISSING_ARGS,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const constants = core.loadExtScript("ext:deno_node/_fs/_fs_constants.ts");
type statCallback = any;
type statCallbackBigInt = any;
type statOptions = any;
const { cp, cpSync } = core.loadExtScript("ext:deno_node/_fs/_fs_cp.ts");
const { read, readSync } = core.createLazyLoader(
  "ext:deno_node/_fs/_fs_read.ts",
)();
const { EventEmitter } = core.loadExtScript("ext:deno_node/_events.mjs");
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
  toUnixTimestamp,
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
  op_node_fs_copy_file,
  op_node_fs_copy_file_sync,
  op_node_fs_exists,
  op_node_fs_exists_sync,
  op_node_fs_fchmod,
  op_node_fs_fchmod_sync,
  op_node_fs_fchown,
  op_node_fs_fchown_sync,
  op_node_fs_fdatasync,
  op_node_fs_fdatasync_sync,
  op_node_fs_fstat_stats,
  op_node_fs_fstat_stats_sync,
  op_node_fs_fstat_sync,
  op_node_fs_fsync,
  op_node_fs_fsync_sync,
  op_node_fs_ftruncate,
  op_node_fs_ftruncate_sync,
  op_node_fs_futimes,
  op_node_fs_futimes_sync,
  op_node_fs_read_deferred,
  op_node_fs_readdir,
  op_node_fs_readdir_sync,
  op_node_fs_read_file,
  op_node_fs_readv,
  op_node_fs_readv_sync,
  op_node_fs_read_file_path_sync,
  op_node_fs_read_file_path,
  op_node_fs_write_deferred,
  op_node_fs_write_v_sync,
  op_node_fs_write_v,
  op_node_fs_writev_sync,
  op_node_fs_writev,
  op_node_fs_write_file_sync,
  op_node_fs_write_file,
  op_node_fs_append_file_sync,
  op_node_fs_append_file,
  op_node_fs_truncate_sync,
  op_node_fs_truncate,
  op_node_lchmod,
  op_node_lchmod_sync,
  op_node_lchown,
  op_node_lchown_sync,
  op_node_lutimes,
  op_node_lutimes_sync,
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
  op_node_fs_stat_watcher_open,
  op_node_fs_stat_watcher_poll,
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
  op_node_fs_lstat,
  op_node_fs_lstat_sync,
  op_node_fs_symlink,
  op_node_fs_symlink_sync,
  op_node_fs_utime,
  op_node_fs_utime_sync,
  op_node_fs_opendir_sync,
  op_node_fs_access,
  op_node_fs_access_sync,
  op_node_fs_validate_watch_ignore,
  op_node_fs_encode_bytes,
  op_node_fs_encode_watch_filename,
  op_node_fs_watch_open,
  op_node_fs_watch_poll,
} = core.ops;
const { isMacOS, isWindows } = core.loadExtScript(
  "ext:deno_node/_util/os.ts",
);
const {
  customPromisifyArgs,
  kCustomPromisifiedSymbol,
} = core.loadExtScript("ext:deno_node/internal/util.mjs");
const lazyPath = core.createLazyLoader("node:path");
const pathModule = lazyPath();
const { resolve } = pathModule;
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
  FunctionPrototypeBind,
  MapPrototypeDelete,
  MapPrototypeGet,
  MapPrototypeSet,
  MathMin,
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  Promise,
  PromisePrototypeThen,
  PromiseReject,
  PromiseResolve,
  RegExpPrototype,
  RegExpPrototypeTest,
  SafeMap,
  SymbolAsyncDispose,
  SymbolAsyncIterator,
  SymbolDispose,
  SymbolFor,
  Uint8ArrayPrototype,
  ArrayPrototypePush,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeSet,
  TypedArrayPrototypeSubarray,
  Uint8Array,
} = primordials;

const abortSignal = core.loadExtScript("ext:deno_web/03_abort_signal.js");
const { pathFromURL } = core.loadExtScript("ext:deno_web/00_infra.js");
const { URLPrototype } = core.loadExtScript("ext:deno_web/00_url.js");

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

// -- fstat / lstat --

// The op validates the fd (getValidatedFd) + extracts bigint from options and
// resolves the cppgc Stats (errors node-formatted with syscall "fstat").
const fstat = callbackifyOpt(op_node_fs_fstat_stats);

// Direct op binding: the op validates the fd, extracts bigint from options,
// reads the stats, and node-formats errors (syscall "fstat").
const fstatSync = op_node_fs_fstat_stats_sync;

// The op extracts bigint/throwIfNoEntry from options, validates the path
// (async(eager_throw)), and resolves the Stats (or undefined when
// throwIfNoEntry is false and the path is missing).
const lstat = callbackifyOpt(op_node_fs_lstat);

// Direct op binding (see lstat).
const lstatSync = op_node_fs_lstat_sync;

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

// The op validates fd/buffers/position synchronously (async(eager_throw)),
// short-circuits empty buffer lists, then seeks to `position` (-1 = current;
// a non-number, e.g. the callback in the 3-arg form, reads as -1) and fills
// each view in order. callbackifyWrite locates the callback and invokes it
// like node's wrapper: `(err, read || 0, buffers)`.
const readv = callbackifyWrite(op_node_fs_readv) as {
  (
    fd: number,
    buffers: readonly ArrayBufferView[],
    callback: ReadvCallback,
  ): void;
  (
    fd: number,
    buffers: readonly ArrayBufferView[],
    position: number | null,
    callback: ReadvCallback,
  ): void;
};

ObjectDefineProperty(readv, customPromisifyArgs, {
  __proto__: null,
  value: ["bytesRead", "buffers"],
  enumerable: false,
});

interface ReadVResult {
  bytesRead: number;
  buffers: readonly ArrayBufferView[];
}

// Direct op binding: a missing/non-number position reads from the current
// file position.
const readvSync = op_node_fs_readv_sync as (
  fd: number,
  buffers: readonly ArrayBufferView[],
  position?: number | null,
) => number;

// promisify picks up readv's customPromisifyArgs and resolves
// `{ bytesRead, buffers }`.
const readvPromise = promisify(readv) as (
  fd: number,
  buffers: readonly ArrayBufferView[],
  position?: number,
) => Promise<ReadVResult>;

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

// Runs `fn(cancelRid)` with node's AbortSignal semantics, shared by the
// readFile/writeFile signal paths: reject with node's AbortError when already
// aborted, close the cancel handle on abort (which interrupts the in-flight
// op), and always surface the abort (with `signal.reason` as cause) once the
// signal fired -- even if the op won the race. The op's own cancellation
// error is never observable: whenever the handle closes, `signal.aborted` is
// set and the AbortError replaces it.
function callWithSignal<T>(
  signal: AbortSignal,
  fn: (cancelRid: number) => Promise<T>,
): Promise<T> {
  if (signal.aborted) {
    return PromiseReject(new AbortError(undefined, { cause: signal.reason }));
  }
  const cancelRid = core.createCancelHandle();
  const abortHandler = () => core.tryClose(cancelRid as number);
  signal[abortSignal.add](abortHandler);
  const finish = () => {
    signal[abortSignal.remove](abortHandler);
    core.tryClose(cancelRid as number);
    if (signal.aborted) {
      throw new AbortError(undefined, { cause: signal.reason });
    }
  };
  return PromisePrototypeThen(
    fn(cancelRid as number),
    (value: T) => {
      finish();
      return value;
    },
    (e: Error) => {
      finish();
      throw e;
    },
  );
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
// "zero-byte liar" files). This stays a JS chunked loop -- NOT a one-shot
// native read with a cancel handle -- because node guarantees an abort
// scheduled via process.nextTick before a chunk read completes is observed
// (test-fs-promises-file-handle-readFile's tick-0 case), which needs
// JS-visible async hops between chunk reads. Returns raw bytes; the caller
// encodes via `op_node_fs_encode_bytes`.
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
  // (the ops parse options + decode). With a signal: the path case uses the
  // shared cancel-handle wrapper (the abort interrupts the native open/read);
  // the fd case stays a JS chunked loop for node's nextTick abort-timing
  // guarantee (see readFileFromFdWithSignal).
  let p: Promise<string | Buffer>;
  if (!options?.signal) {
    p = typeof pathOrRid === "number"
      ? op_node_fs_read_file(pathOrRid, options)
      : op_node_fs_read_file_path(pathOrRid, options);
  } else if (typeof pathOrRid === "number") {
    p = PromisePrototypeThen(
      readFileFromFdWithSignal(pathOrRid, options),
      (data: Uint8Array) => op_node_fs_encode_bytes(data, options),
    );
  } else {
    p = callWithSignal(
      options.signal,
      (cancelRid: number) =>
        op_node_fs_read_file_path(pathOrRid, options, cancelRid),
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

// The op validates path + mode synchronously (async(eager_throw));
// callbackifyOpt treats the optional 2nd arg as mode-or-callback.
const access = callbackifyOpt(op_node_fs_access, 1) as (
  path: string | Buffer | URL,
  mode?: number | CallbackWithError,
  callback?: CallbackWithError,
) => void;

const accessSync = op_node_fs_access_sync;

// Common case (no AbortSignal, no custom iterable): the op applies node's
// appendFile option handling (default flag "a") and runs the open/write/close
// natively, emitting the final node errors. The rare signal/iterable cases
// force the append flag like node and reuse writeFile's JS orchestration.
function appendFile(
  path: string | number | URL,
  data: string | Uint8Array,
  options: Encodings | WriteFileOptions | CallbackWithError,
  callback?: CallbackWithError,
) {
  callback ||= options as CallbackWithError;
  validateFunction(callback, "cb");
  if (typeof options === "function") {
    options = undefined;
  }

  if (getSignal(options) || _isCustomIterable(data)) {
    options = copyObject(
      getOptions(options, { encoding: "utf8", mode: 0o666, flag: "a" }),
    );
    // Force append behavior when using a supplied file descriptor
    if (!options.flag || isFd(path)) {
      options.flag = "a";
    }
    writeFile(path, data, options, callback);
    return;
  }

  PromisePrototypeThen(
    op_node_fs_append_file(path, data, options),
    () => callback(null),
    callback,
  );
}

// Direct op binding: writeFileSync with node's appendFile option handling
// (default flag "a") done natively, throwing the final node error.
const appendFileSync = op_node_fs_append_file_sync as (
  path: string | number | URL,
  data: string | Uint8Array,
  options?: Encodings | WriteFileOptions,
) => void;

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
  | undefined = !isMacOS ? undefined : op_node_lchmod_sync;

const lchown = callbackify(op_node_lchown, 3);

const fdatasyncSync = op_node_fs_fdatasync_sync;

const fsync = callbackify(op_node_fs_fsync, 1);

// Direct op binding: the op validates path + uid/gid (validateInteger) and
// node-formats errors (syscall "lchown").
const lchownSync = op_node_lchown_sync as (
  path: string | Buffer | URL,
  uid: number,
  gid: number,
) => void;

const fsyncSync = op_node_fs_fsync_sync;

const link = callbackify(op_node_fs_link, 2);

// Direct op binding: the op validates both paths (existingPath/newPath arg
// names like node) and node-formats errors (syscall "link").
const linkSync = op_node_fs_link_sync as (
  existingPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) => void;

const unlink = callbackify(op_node_fs_remove, 1);
const unlinkSync = op_node_fs_remove_sync;

const rename = callbackify(op_node_fs_rename, 2);

// Direct op binding: the op validates both paths (oldPath/newPath arg names
// like node) and node-formats errors (syscall "rename").
const renameSync = op_node_fs_rename_sync as (
  oldPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) => void;

type rmOptions = {
  force?: boolean;
  maxRetries?: number;
  recursive?: boolean;
  retryDelay?: number;
};

type rmCallback = (err: Error | null) => void;

// The op validates `options` (node's validateRmOptions: object + force/
// recursive/retryDelay/maxRetries types) and runs the lstat precheck
// (ERR_FS_EISDIR / force-ENOENT) natively, emitting the final node error;
// callbackifyOpt treats the optional 2nd arg as options-or-callback.
const rm = callbackifyOpt(op_node_fs_rm, 1) as {
  (path: string | URL, callback: rmCallback): void;
  (path: string | URL, options: rmOptions, callback: rmCallback): void;
};

const rmSync = op_node_fs_rm_sync;

type rmdirOptions = {
  maxRetries?: number;
  recursive?: boolean;
  retryDelay?: number;
};

type rmdirCallback = (err?: Error) => void;

// The op validates options (incl. throwing ERR_INVALID_ARG_VALUE for the
// removed `recursive` option) + the path synchronously (async(eager_throw))
// and throws the final node error (syscall "rmdir", path); callbackifyOpt
// treats the optional 2nd arg as options-or-callback.
const rmdir = callbackifyOpt(op_node_rmdir, 1) as {
  (path: string | Buffer | URL, callback: rmdirCallback): void;
  (
    path: string | Buffer | URL,
    options: rmdirOptions,
    callback: rmdirCallback,
  ): void;
};

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

// Node's `fs.Dir` (lib/internal/fs/dir.js), backed by a single native
// readdir: the whole listing is produced in Rust (`op_node_fs_readdir`) on
// the first read and handed out one entry at a time, so close() has nothing
// native to release (it only flips the flag, with node's ERR_DIR_CLOSED and
// callback-validation semantics).
class Dir {
  #dirPath: string | Uint8Array;
  #entries: Dirent[] | null = null;
  // The in-flight native readdir, shared by concurrent read()s so the listing
  // is fetched once and handed out in call order.
  #fetch: Promise<Dirent[]> | null = null;
  // Number of async reads currently awaiting the fetch: sync operations throw
  // ERR_DIR_CONCURRENT_OPERATION while nonzero, like node's operation queue.
  #pending = 0;
  #idx = 0;
  #closed = false;
  #recursive: boolean;

  constructor(path: string | Uint8Array, recursive = false) {
    if (!path) {
      throw new ERR_MISSING_ARGS("path");
    }
    this.#dirPath = path;
    this.#recursive = recursive;
  }

  get path(): string {
    // Match Node: invoking the getter on a non-Dir receiver (e.g. the
    // prototype) throws ERR_INVALID_THIS rather than a private-field error.
    // deno-lint-ignore prefer-primordials -- private-field brand check
    if (!(#dirPath in this)) {
      throw new ERR_INVALID_THIS("Dir");
    }
    if (ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, this.#dirPath)) {
      // deno-lint-ignore prefer-primordials
      return Buffer.from(this.#dirPath as Uint8Array).toString("utf8");
    }
    return this.#dirPath as string;
  }

  #next(): Dirent | null {
    if (this.#entries !== null && this.#idx < this.#entries.length) {
      return this.#entries[this.#idx++];
    }
    return null;
  }

  async #readPromisified(): Promise<Dirent | null> {
    if (this.#closed) {
      throw new ERR_DIR_CLOSED();
    }
    if (this.#entries === null) {
      // Async-function bodies run synchronously up to the first await, so
      // buffered reads complete without ever appearing pending.
      this.#pending++;
      try {
        this.#fetch ??= op_node_fs_readdir(this.path, this.#recursive, true);
        const entries = await this.#fetch;
        this.#entries ??= entries;
      } finally {
        this.#pending--;
      }
    }
    return this.#next();
  }

  // Match node's read(): no arguments -> a promise; with a callback the
  // closed check throws ERR_DIR_CLOSED synchronously, the callback is
  // validated (ERR_INVALID_ARG_TYPE), and the method returns undefined.
  read(
    callback?: (...args: any[]) => void,
  ): Promise<Dirent | null> | undefined {
    if (arguments.length === 0) {
      return this.#readPromisified();
    }
    if (this.#closed) {
      throw new ERR_DIR_CLOSED();
    }
    if (callback === undefined) {
      return this.#readPromisified();
    }
    validateFunction(callback, "callback");
    PromisePrototypeThen(
      this.#readPromisified(),
      (dirent) => callback(null, dirent),
      callback,
    );
  }

  readSync(): Dirent | null {
    if (this.#closed) {
      throw new ERR_DIR_CLOSED();
    }
    if (this.#pending > 0) {
      throw new ERR_DIR_CONCURRENT_OPERATION();
    }
    if (this.#entries === null) {
      this.#entries = op_node_fs_readdir_sync(
        this.path,
        this.#recursive,
        true,
      ) as unknown as Dirent[];
    }
    return this.#next();
  }

  close(callback?: (...args: any[]) => void): Promise<void> | undefined {
    if (callback === undefined) {
      if (this.#closed) {
        return PromiseReject(new ERR_DIR_CLOSED());
      }
      this.#closed = true;
      return PromiseResolve();
    }
    validateFunction(callback, "callback");
    if (this.#closed) {
      process.nextTick(callback, new ERR_DIR_CLOSED());
      return;
    }
    this.#closed = true;
    process.nextTick(callback, null);
  }

  closeSync() {
    if (this.#closed) {
      throw new ERR_DIR_CLOSED();
    }
    if (this.#pending > 0) {
      throw new ERR_DIR_CONCURRENT_OPERATION();
    }
    this.#closed = true;
  }

  async *entries(): AsyncIterableIterator<Dirent> {
    try {
      while (true) {
        const dirent = await this.#readPromisified();
        if (dirent === null) {
          break;
        }
        yield dirent;
      }
    } finally {
      if (!this.#closed) {
        this.#closed = true;
      }
    }
  }

  // Unlike explicit close()/closeSync(), the dispose protocol is idempotent:
  // repeated invocations must not throw (see node's file-handle-dispose test).
  [SymbolDispose]() {
    if (this.#closed) return;
    this.closeSync();
  }

  async [SymbolAsyncDispose]() {
    if (this.#closed) return;
    await this.close();
  }
}

// Match node: `Dir.prototype[Symbol.asyncIterator]` IS `entries` (the same
// function object; non-enumerable, writable, configurable).
ObjectDefineProperty(Dir.prototype, SymbolAsyncIterator, {
  __proto__: null,
  enumerable: false,
  writable: true,
  configurable: true,
  value: Dir.prototype.entries,
});

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

// -- readdir --

// Names come back utf8 from the native op; re-encode for the rare non-utf8
// encodings. Only `name` is re-encoded -- node's Dirent keeps `parentPath` a
// string even with `encoding: "buffer"`.
function applyReaddirEncoding(
  result: Array<string | Dirent>,
  options: { encoding?: string; withFileTypes?: boolean },
): Array<string | Dirent> {
  const enc = options.encoding;
  if (!enc || enc === "utf8" || enc === "utf-8") return result;
  if (options.withFileTypes) {
    for (let i = 0; i < result.length; i++) {
      const d = result[i] as Dirent;
      d.name = decodeDirentName(d.name as string, enc) as string;
    }
  } else {
    for (let i = 0; i < result.length; i++) {
      result[i] = decodeDirentName(result[i] as string, enc);
    }
  }
  return result;
}

function decodeDirentName(str: string, encoding: string): string | Buffer {
  // "buffer" returns Buffer instances; every other (node-supported) encoding
  // re-encodes the UTF-8 filename through Buffer to match node's
  // lib/internal/fs/utils.js getDirent / readdir output.
  const buf = Buffer.from(str, "utf8");
  if (encoding === "buffer") return buf;
  // No primordial exists for Buffer.prototype.toString with an encoding.
  // deno-lint-ignore prefer-primordials
  return buf.toString(encoding as BufferEncoding);
}

// Mirrors node's lib/fs.js readdir() validation order: callback, options
// (encoding via getOptions), path, recursive. The op walks the tree natively
// (recursive included) and produces the final node error (scandir + path).
function readdir(
  path: string | Buffer | URL,
  options?:
    | { encoding?: string; withFileTypes?: boolean; recursive?: boolean }
    | string
    | ((err: Error | null, files?: unknown[]) => void),
  callback?: (err: Error | null, files?: unknown[]) => void,
) {
  callback = makeCallback(typeof options === "function" ? options : callback);
  const opts = getOptions(options);
  path = getValidatedPathToString(path);
  if (opts.recursive != null) {
    validateBoolean(opts.recursive, "options.recursive");
  }
  PromisePrototypeThen(
    op_node_fs_readdir(path, opts.recursive ?? false, !!opts.withFileTypes),
    (result: Array<string | Dirent>) =>
      callback(null, applyReaddirEncoding(result, opts)),
    callback,
  );
}

function readdirSync(
  path: string | Buffer | URL,
  options?:
    | { encoding?: string; withFileTypes?: boolean; recursive?: boolean }
    | string,
): Array<string | Dirent> {
  const opts = getOptions(options);
  path = getValidatedPathToString(path);
  if (opts.recursive != null) {
    validateBoolean(opts.recursive, "options.recursive");
  }
  return applyReaddirEncoding(
    op_node_fs_readdir_sync(
      path,
      opts.recursive ?? false,
      !!opts.withFileTypes,
    ),
    opts,
  );
}

// -- copyFile / lutimes / exists --

// node validates src/dest before the callback and mode after it (in the C++
// binding); the op's eager validation covers all three, and COPYFILE_EXCL /
// the FICLONE hints are handled natively.
const copyFile = callbackifyOpt(op_node_fs_copy_file, 2);
const copyFileSync = op_node_fs_copy_file_sync;

// node's lutimes validates the callback (positionally, like symlink) before
// the path/times, which the op then validates eagerly.
const lutimes = callbackifyOpt(op_node_lutimes, 3, true);
const lutimesSync = op_node_lutimes_sync;

// Deprecated. node's `exists` swallows every error into a boolean and has a
// non-standard callback signature (no error argument), hence the custom
// promisify form below (fs.promises has no `exists`).
function exists(
  path: string | Buffer | URL,
  callback: (exists: boolean) => void,
) {
  callback = makeCallback(callback);
  try {
    path = getValidatedPathToString(path);
  } catch {
    callback(false);
    return;
  }
  PromisePrototypeThen(op_node_fs_exists(path), callback);
}

// fs.exists' callback has no error argument, so promisify needs a custom
// implementation (nodejs/node#13316), named `exists` to match node's.
const existsPromisified = (path: string | Buffer | URL) =>
  new Promise((resolve) => exists(path, resolve));
ObjectDefineProperty(existsPromisified, "name", {
  __proto__: null,
  value: "exists",
  configurable: true,
});
ObjectDefineProperty(exists, kCustomPromisifiedSymbol, {
  __proto__: null,
  value: existsPromisified,
  enumerable: false,
  writable: false,
  configurable: true,
});

let showExistsDeprecation = true;
function existsSync(path: string | Buffer | URL): boolean {
  try {
    path = getValidatedPathToString(path);
  } catch (err) {
    if (
      showExistsDeprecation && (err as any)?.code === "ERR_INVALID_ARG_TYPE"
    ) {
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
  kIoMaxLength,
  kReadFileUnknownBufferLength,
  kWriteFileMaxChunkSize,
} = fsUtilConstants;

interface Writer {
  write(p: NodeJS.TypedArray): Promise<number>;
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

  // Non-iterable data, path or fd: the op parses options + data (string ->
  // encoded bytes), opens, optionally chmods, writes all bytes, and closes
  // natively, throwing the final node error; with a signal the shared
  // cancel-handle wrapper interrupts the native open/write on abort. Only
  // the (async-)iterable data case falls through to JS.
  if (!_isCustomIterable(data)) {
    const promise = signal
      ? callWithSignal(
        signal,
        (cancelRid: number) =>
          op_node_fs_write_file(pathOrRid, data, options, cancelRid),
      )
      : op_node_fs_write_file(pathOrRid, data, options);
    PromisePrototypeThen(promise, () => callback(null), callback);
    return;
  }

  const encoding = getValidatedEncoding(options) || "utf8";

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

// Writes each chunk of an (async-)iterable `data` (the non-iterable cases go
// through `op_node_fs_write_file` natively), checking the AbortSignal between
// chunk writes.
async function _writeAll(
  w: Writer,
  data: Exclude<WriteFileData, string>,
  encoding: BufferEncoding,
  signal?: AbortSignal,
) {
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
// callbackifyOpt(.., 2, cbAtEnd): target + path are fixed; like node, the
// LAST argument past those is the callback by position (so
// `symlink(t, p, "dir")` with no callback throws "Received type string
// ('dir')"), validated before the op starts the I/O.
const symlink = callbackifyOpt(op_node_fs_symlink, 2, true);

// Direct op binding: the op validates target/path/type and does the Windows
// dir/file autodetect (see symlink above).
const symlinkSync = op_node_fs_symlink_sync as (
  target: string | Buffer | URL,
  path: string | Buffer | URL,
  type?: SymlinkType,
) => void;

// -- watch --

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
  // node's watch(filename, options, listener): when `options` is a function it
  // is the listener (a third argument is ignored), and a string is
  // `{ encoding }`. getOptions validates encoding + signal up front, before
  // the path (test-fs-assert-encoding-error relies on the sync throw).
  const options = getOptions(
    typeof optionsOrListener === "function" ? undefined : optionsOrListener,
  );

  op_node_fs_validate_watch_ignore(options.ignore, "options.ignore");

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
  const ignoreMatcher = createIgnoreMatcher(options?.ignore);

  // Open the watcher, but defer any failure to an 'error' event on the
  // returned FSWatcher rather than throwing synchronously. Editors that
  // atomically save (write to <file>.tmp.<pid>.<ts> then rename over
  // the original) can race the inotify watch and produce a transient
  // ENOENT here; callers using EventEmitter-style error handling
  // (chokidar, vite) can't recover from a sync throw. See
  // denoland/deno#34396. The op pre-validates path existence and returns
  // fully node-formatted errors (uv ENOENT with syscall "watch" etc).
  let rid: number | null = null;
  let openError: Error | undefined;
  try {
    rid = op_node_fs_watch_open(watchPath, recursive);
  } catch (e) {
    openError = e as Error;
  }

  const fsWatcher = new FSWatcher();
  if (rid !== null) {
    fsWatcher[kFSWatchStart](rid, ignoreMatcher, encoding);
  }

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
  const rid = op_node_fs_watch_open(watchPath, recursive);

  let closed = false;
  const close = () => {
    if (!closed) {
      closed = true;
      core.tryClose(rid);
    }
  };

  let onAbort: (() => void) | null = null;
  function cleanupAbort() {
    if (signal && onAbort) {
      signal.removeEventListener("abort", onAbort);
      onAbort = null;
    }
  }

  if (signal) {
    if (signal.aborted) {
      close();
    } else {
      onAbort = close;
      signal.addEventListener("abort", onAbort, { once: true });
    }
  }

  // Match Node: surface signal abort as a thrown AbortError carrying
  // `signal.reason` as `cause`.
  // https://github.com/nodejs/node/blob/main/lib/internal/fs/watchers.js
  function abortError(): AbortError {
    return new AbortError(undefined, { cause: signal?.reason });
  }

  const result = {
    async next(): Promise<
      IteratorResult<{ eventType: string; filename: string | Buffer | null }>
    > {
      if (signal?.aborted) {
        cleanupAbort();
        throw abortError();
      }
      while (true) {
        let event;
        try {
          event = await op_node_fs_watch_poll(rid);
        } catch (e) {
          cleanupAbort();
          if (ObjectPrototypeIsPrototypeOf(BadResourcePrototype, e)) {
            return { value: undefined, done: true };
          }
          throw e;
        }
        if (event === null) {
          cleanupAbort();
          if (signal?.aborted) {
            throw abortError();
          }
          return { value: undefined, done: true };
        }
        if (ignoreMatcher !== null && ignoreMatcher(event[1])) {
          continue;
        }
        return {
          value: { eventType: event[0], filename: event[1] },
          done: false,
        };
      }
    },
    return(value?: any): Promise<IteratorResult<any>> {
      cleanupAbort();
      close();
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
  #refed = true;
  #rid: number | null = null;
  #stopped = false;
  // The current in-flight poll op promise: the Rust op owns the interval
  // timer and the previous-stats snapshot, resolving once per change (or
  // null when the watcher is stopped). Ref/unref of the op promise toggles
  // whether the watcher keeps the event loop alive.
  #pollPromise: Promise<unknown> | null = null;

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

    const rid = op_node_fs_stat_watcher_open(filename, this.#bigint, interval);
    this.#rid = rid;
    (async () => {
      while (true) {
        let pair;
        try {
          const promise = op_node_fs_stat_watcher_poll(rid);
          this.#pollPromise = promise;
          if (!this.#refed) {
            core.unrefOpPromise(promise);
          }
          pair = await promise;
        } catch (e) {
          if (ObjectPrototypeIsPrototypeOf(BadResourcePrototype, e)) {
            return;
          }
          this.emit("error", e);
          return;
        } finally {
          this.#pollPromise = null;
        }
        if (pair === null) {
          return;
        }
        this.emit("change", pair[0], pair[1]);
      }
    })();
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
    if (this.#stopped) {
      return;
    }
    this.#stopped = true;
    if (this.#rid !== null) {
      core.tryClose(this.#rid);
    }
    // Match Node: stop fires asynchronously so listeners removed
    // synchronously after stop() are not called (see
    // StatWatcher.prototype.stop in lib/internal/fs/watchers.js).
    process.nextTick(() => this.emit("stop"));
  }
  // Node's ref/unref toggle whether the StatWatcher's internal handle keeps
  // the event loop alive (see lib/internal/fs/watchers.js). Here the handle
  // is the pending poll op promise, so we ref/unref that.
  ref() {
    this.#refed = true;
    if (this.#pollPromise !== null) {
      core.refOpPromise(this.#pollPromise);
    }
    return this;
  }
  unref() {
    this.#refed = false;
    if (this.#pollPromise !== null) {
      core.unrefOpPromise(this.#pollPromise);
    }
    return this;
  }
}

const kFSWatchStart = SymbolFor("kFSWatchStart");

class FSWatcher extends EventEmitter {
  #rid: number | null = null;
  #closed = false;
  #refed = true;
  // The current in-flight poll op promise. The Rust op converts each notify
  // event to node's (eventType, filename) pair and resolves null once the
  // watcher closes; ref/unref of the promise toggles event-loop liveness.
  #pollPromise: Promise<unknown> | null = null;

  [kFSWatchStart](
    rid: number,
    ignoreMatcher: ((filename: string) => boolean) | null,
    encoding: string | undefined,
  ) {
    this.#rid = rid;
    (async () => {
      while (true) {
        let event;
        try {
          const promise = op_node_fs_watch_poll(rid);
          this.#pollPromise = promise;
          if (!this.#refed) {
            core.unrefOpPromise(promise);
          }
          event = await promise;
        } catch (e) {
          if (ObjectPrototypeIsPrototypeOf(BadResourcePrototype, e)) {
            return;
          }
          // Already node-formatted by the op (uv-style, syscall "watch").
          this.emit("error", e);
          return;
        } finally {
          this.#pollPromise = null;
        }
        if (event === null) {
          return;
        }
        if (ignoreMatcher !== null && ignoreMatcher(event[1])) {
          continue;
        }
        this.emit(
          "change",
          event[0],
          op_node_fs_encode_watch_filename(event[1], encoding),
        );
      }
    })();
  }
  close() {
    if (this.#closed) {
      return;
    }
    this.#closed = true;
    this.emit("close");
    if (this.#rid !== null) {
      core.tryClose(this.#rid);
    }
  }
  ref() {
    this.#refed = true;
    if (this.#pollPromise !== null) {
      core.refOpPromise(this.#pollPromise);
    }
    return this;
  }
  unref() {
    this.#refed = false;
    if (this.#pollPromise !== null) {
      core.unrefOpPromise(this.#pollPromise);
    }
    return this;
  }
}

// Match Node: the public `fs.Stats` export is deprecated (DEP0180).
// Internal call sites use the un-deprecated `Stats` directly.
// See lib/internal/fs/utils.js `Stats: deprecate(...)`.
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
