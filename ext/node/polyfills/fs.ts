// Copyright 2018-2026 the Deno authors. MIT license.
import { fs as fsConstants } from "ext:deno_node/internal_binding/constants.ts";
import { codeMap } from "ext:deno_node/internal_binding/uv.ts";
import {
  type BinaryOptionsArgument,
  type CallbackWithError,
  type FileOptions,
  type FileOptionsArgument,
  getValidatedEncoding,
  isFd,
  isFileOptions,
  makeCallback,
  maybeCallback,
  type TextOptionsArgument,
  type WriteFileOptions,
} from "ext:deno_node/_fs/_fs_common.ts";
import type { Encodings } from "ext:deno_node/_utils.ts";
import {
  AbortError,
  denoErrorToNodeError,
  denoWriteFileErrorToNodeError,
  ERR_FS_FILE_TOO_LARGE,
} from "ext:deno_node/internal/errors.ts";
import * as constants from "ext:deno_node/_fs/_fs_constants.ts";
import {
  CFISBIS,
  convertFileInfoToBigIntStats,
  convertFileInfoToStats,
  type statCallback,
  type statCallbackBigInt,
  type statOptions,
} from "ext:deno_node/internal/fs/stat_utils.ts";
import { copyFile, copyFileSync } from "ext:deno_node/_fs/_fs_copy.ts";
import { cp, cpSync } from "ext:deno_node/_fs/_fs_cp.ts";
import Dir from "ext:deno_node/_fs/_fs_dir.ts";
import { exists, existsSync } from "ext:deno_node/_fs/_fs_exists.ts";
import { fstat, fstatSync } from "ext:deno_node/_fs/_fs_fstat.ts";
import { lstat, lstatSync } from "ext:deno_node/_fs/_fs_lstat.ts";
import { lutimes, lutimesSync } from "ext:deno_node/_fs/_fs_lutimes.ts";
import { read, readSync } from "ext:deno_node/_fs/_fs_read.ts";
import { readdir, readdirSync } from "ext:deno_node/_fs/_fs_readdir.ts";
import { EventEmitter } from "node:events";
import { type MaybeEmpty, notImplemented } from "ext:deno_node/_utils.ts";
import { promisify } from "node:util";
import { delay } from "ext:deno_node/_util/async.ts";
import promises from "ext:deno_node/internal/fs/promises.ts";
// @deno-types="./internal/fs/streams.d.ts"
import {
  createReadStream,
  createWriteStream,
  ReadStream,
  WriteStream,
} from "ext:deno_node/internal/fs/streams.mjs";
import {
  arrayBufferViewToUint8Array,
  BigIntStats,
  constants as fsUtilConstants,
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
  Stats,
  stringToFlags,
  toUnixTimestamp as _toUnixTimestamp,
  validateBufferArray,
  validateOffsetLengthWrite,
  validateRmdirOptions,
  validateRmOptions,
  validateRmOptionsSync,
  validateStringAfterArrayBufferView,
  warnOnNonPortableTemplate,
} from "ext:deno_node/internal/fs/utils.mjs";
import { glob, globSync } from "ext:deno_node/_fs/_fs_glob.ts";
import {
  parseFileMode,
  validateBoolean,
  validateEncoding,
  validateFunction,
  validateInt32,
  validateInteger,
  validateObject,
  validateOneOf,
  validateString,
} from "ext:deno_node/internal/validators.mjs";
import { Buffer } from "node:buffer";
import process from "node:process";
import { isArrayBufferView } from "ext:deno_node/internal/util/types.ts";
import { TextEncoder } from "ext:deno_web/08_text_encoding.js";
import * as abortSignal from "ext:deno_web/03_abort_signal.js";
import { pathFromURL } from "ext:deno_web/00_infra.js";
import { URLPrototype } from "ext:deno_web/00_url.js";
import { FileHandle } from "ext:deno_node/internal/fs/handle.ts";
import { isIterable } from "ext:deno_node/internal/streams/utils.js";
import type { ErrnoException } from "ext:deno_node/_global.d.ts";
import type { BufferEncoding } from "ext:deno_node/_global.d.ts";
import {
  op_fs_read_file_async,
  op_fs_read_file_sync,
  op_node_fs_close,
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
  op_node_fs_read_file_sync,
  op_node_fs_read_sync,
  op_node_fs_seek,
  op_node_fs_seek_sync,
  op_node_fs_write_deferred,
  op_node_fs_write_sync,
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
} from "ext:core/ops";
import {
  ERR_FS_RMDIR_ENOTDIR,
  ERR_INVALID_ARG_TYPE,
  uvException,
} from "ext:deno_node/internal/errors.ts";
import { toUnixTimestamp } from "ext:deno_node/internal/fs/utils.mjs";
import { isMacOS, isWindows } from "ext:deno_node/_util/os.ts";
import {
  customPromisifyArgs,
  kEmptyObject,
  normalizeEncoding,
} from "ext:deno_node/internal/util.mjs";
import { basename, resolve, toNamespacedPath } from "node:path";
import * as pathModule from "node:path";
import type { Encoding } from "node:crypto";
import { core, primordials } from "ext:core/mod.js";

const {
  ArrayBufferIsView,
  BigInt,
  DatePrototypeGetTime,
  DateUTC,
  Error,
  FunctionPrototypeBind,
  ErrorPrototype,
  MapPrototypeDelete,
  MapPrototypeGet,
  MapPrototypeSet,
  MathMin,
  MathTrunc,
  Number,
  NumberIsFinite,
  NumberIsNaN,
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  Promise,
  PromisePrototypeThen,
  PromiseResolve,
  SafeMap,
  StringPrototypeToString,
  SymbolAsyncIterator,
  SymbolFor,
  ArrayPrototypePush,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeSet,
  TypedArrayPrototypeSubarray,
  Uint8Array,
} = primordials;

const {
  kIoMaxLength,
  kReadFileBufferLength,
  kReadFileUnknownBufferLength,
} = fsUtilConstants;

const defaultStatOptions = { __proto__: null, bigint: false };
const defaultStatSyncOptions = {
  __proto__: null,
  bigint: false,
  throwIfNoEntry: true,
};

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
function stat(
  path: string | Buffer | URL,
  options:
    | statCallback
    | statCallbackBigInt
    | statOptions = defaultStatOptions,
  callback?: statCallback | statCallbackBigInt,
) {
  if (typeof options === "function") {
    callback = options;
    options = defaultStatOptions;
  }
  callback = makeCallback(callback);
  path = getValidatedPathToString(path);

  PromisePrototypeThen(
    Deno.stat(path),
    (stat) => callback(null, CFISBIS(stat, options.bigint)),
    (err) =>
      callback(
        denoErrorToNodeError(err, { syscall: "stat", path }),
      ),
  );
}

function statSync(path: string | Buffer | URL): Stats;
function statSync(
  path: string | Buffer | URL,
  options: { bigint: false; throwIfNoEntry: true },
): Stats;
function statSync(
  path: string | Buffer | URL,
  options: { bigint: false; throwIfNoEntry: false },
): Stats | undefined;
function statSync(
  path: string | Buffer | URL,
  options: { bigint: true; throwIfNoEntry: true },
): BigIntStats;
function statSync(
  path: string | Buffer | URL,
  options: { bigint: true; throwIfNoEntry: false },
): BigIntStats | undefined;
function statSync(
  path: string | Buffer | URL,
  options: statOptions = defaultStatSyncOptions,
): Stats | BigIntStats | undefined {
  path = getValidatedPathToString(path);

  try {
    const origin = Deno.statSync(path);
    return CFISBIS(origin, options.bigint);
  } catch (err) {
    if (
      options?.throwIfNoEntry === false &&
      ObjectPrototypeIsPrototypeOf(Deno.errors.NotFound.prototype, err)
    ) {
      return;
    }
    if (ObjectPrototypeIsPrototypeOf(ErrorPrototype, err)) {
      throw denoErrorToNodeError(err as Error, {
        syscall: "stat",
        path,
      });
    } else {
      throw err;
    }
  }
}

// -- realpath --

type RealpathEncoding = BufferEncoding | "buffer";
type RealpathEncodingObj = { encoding?: RealpathEncoding };
type RealpathOptions = RealpathEncoding | RealpathEncodingObj;
type RealpathCallback = (
  err: Error | null,
  path?: string | Buffer,
) => void;

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
function readv(
  fd: number,
  buffers: readonly ArrayBufferView[],
  position: number | ReadvCallback,
  callback?: ReadvCallback,
): void {
  if (typeof fd !== "number") {
    throw new ERR_INVALID_ARG_TYPE("fd", "number", fd);
  }
  fd = getValidatedFd(fd);
  validateBufferArray(buffers);
  const cb = maybeCallback(callback || position) as ReadvCallback;
  let pos: number | null = null;
  if (typeof position === "number") {
    validateInteger(position, "position", 0);
    pos = position;
  }

  if (buffers.length === 0) {
    process.nextTick(cb, null, 0, buffers);
    return;
  }

  const innerReadv = async (
    fd: number,
    buffers: readonly ArrayBufferView[],
    position: number | null,
  ) => {
    if (typeof position === "number") {
      await op_node_fs_seek(fd, position, 0);
    }

    let readTotal = 0;
    let readInBuf = 0;
    let bufIdx = 0;
    let buf = buffers[bufIdx];
    while (bufIdx < buffers.length) {
      const nread = op_node_fs_read_sync(fd, buf, -1);
      if (nread === null) {
        break;
      }
      readInBuf += nread;
      if (readInBuf === TypedArrayPrototypeGetByteLength(buf)) {
        readTotal += readInBuf;
        readInBuf = 0;
        bufIdx += 1;
        buf = buffers[bufIdx];
      }
    }
    readTotal += readInBuf;

    return readTotal;
  };

  PromisePrototypeThen(innerReadv(fd, buffers, pos), (numRead) => {
    cb(null, numRead, buffers);
  }, (err) => cb(err, -1, buffers));
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
  if (typeof fd !== "number") {
    throw new ERR_INVALID_ARG_TYPE("fd", "number", fd);
  }
  fd = getValidatedFd(fd);
  validateBufferArray(buffers);
  if (buffers.length === 0) {
    return 0;
  }
  if (typeof position === "number") {
    validateInteger(position, "position", 0);
    op_node_fs_seek_sync(fd, position, 0);
  }

  let readTotal = 0;
  let readInBuf = 0;
  let bufIdx = 0;
  let buf = buffers[bufIdx];
  while (bufIdx < buffers.length) {
    const nread = op_node_fs_read_sync(fd, buf, -1);
    if (nread === null) {
      break;
    }
    readInBuf += nread;
    if (readInBuf === TypedArrayPrototypeGetByteLength(buf)) {
      readTotal += readInBuf;
      readInBuf = 0;
      bufIdx += 1;
      buf = buffers[bufIdx];
    }
  }
  readTotal += readInBuf;

  return readTotal;
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

function readFileMaybeDecode(data: Uint8Array, encoding: Encodings): string;
function readFileMaybeDecode(
  data: Uint8Array,
  encoding: null | undefined,
): Buffer;
function readFileMaybeDecode(
  data: Uint8Array,
  encoding: Encodings | null | undefined,
): string | Buffer {
  // deno-lint-ignore prefer-primordials
  const buffer = Buffer.from(data.buffer, data.byteOffset, data.byteLength);
  // deno-lint-ignore prefer-primordials
  if (encoding) return buffer.toString(encoding);
  return buffer;
}

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

async function readFileAsync(
  path: string,
  options: FileOptions | undefined,
): Promise<Uint8Array> {
  let cancelRid: number | undefined;
  let abortHandler: (rid: number) => void;
  const flagsNumber = stringToFlags(options!.flag, "options.flag");
  if (options?.signal) {
    options.signal.throwIfAborted();
    cancelRid = core.createCancelHandle();
    abortHandler = () => core.tryClose(cancelRid as number);
    options.signal[abortSignal.add](abortHandler);
  }

  try {
    const data = await op_fs_read_file_async(
      path,
      cancelRid,
      flagsNumber,
    );
    return data;
  } finally {
    if (options?.signal) {
      options.signal[abortSignal.remove](abortHandler);

      // always throw the abort error when aborted
      options.signal.throwIfAborted();
    }
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

async function readFileFromFd(fd: number, options?: FileOptions) {
  const signal = options?.signal;
  const encoding = options?.encoding;
  readFileCheckAborted(signal);

  const statFields = op_node_fs_fstat_sync(fd);
  readFileCheckAborted(signal);

  let size = 0;
  let length = 0;
  if (statFields.isFile) {
    size = statFields.size;
    length = encoding ? MathMin(size, kReadFileBufferLength) : size;
  }
  if (length === 0) {
    length = kReadFileUnknownBufferLength;
  }

  if (size > kIoMaxLength) {
    throw new ERR_FS_FILE_TOO_LARGE(size);
  }

  const buffer = new Uint8Array(length);
  const buffers: Uint8Array[] = [];

  while (true) {
    readFileCheckAborted(signal);
    // Use the deferred op so we yield to the event loop between reads,
    // allowing abort signals scheduled via process.nextTick to fire.
    const nread = await op_node_fs_read_deferred(fd, buffer, -1);
    if (nread === 0) {
      break;
    }
    ArrayPrototypePush(buffers, TypedArrayPrototypeSubarray(buffer, 0, nread));
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
  if (ObjectPrototypeIsPrototypeOf(FileHandle.prototype, pathOrRid)) {
    pathOrRid = (pathOrRid as FileHandle).fd;
  } else if (typeof pathOrRid !== "number") {
    pathOrRid = getValidatedPathToString(pathOrRid as string);
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

  let p: Promise<Uint8Array>;
  if (typeof pathOrRid === "string") {
    p = readFileAsync(pathOrRid, options);
  } else {
    p = readFileFromFd(pathOrRid as number, options);
  }

  if (cb) {
    PromisePrototypeThen(
      p,
      (data: Uint8Array) => {
        const textOrBuffer = readFileMaybeDecode(data, options?.encoding);
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
  // deno-lint-ignore no-explicit-any
): Promise<any> {
  return new Promise((resolve, reject) => {
    readFile(path, options, (err, data) => {
      if (err) reject(err);
      else resolve(data);
    });
  });
}

function readFileSync(
  path: string | URL | number,
  opt: TextOptionsArgument,
): string;
function readFileSync(
  path: string | URL | number,
  opt?: BinaryOptionsArgument,
): Buffer;
function readFileSync(
  path: string | URL | number,
  opt?: FileOptionsArgument,
): string | Buffer {
  const options = getOptions<FileOptions>(opt, readFileDefaultOptions);

  let data;
  if (typeof path === "number") {
    data = op_node_fs_read_file_sync(path);
  } else {
    // Validate/convert path to string (throws on invalid types)
    path = getValidatedPathToString(path as unknown as string);

    const flagsNumber = stringToFlags(options?.flag, "options.flag");
    try {
      data = op_fs_read_file_sync(path, flagsNumber);
    } catch (err) {
      throw denoErrorToNodeError(err, { path, syscall: "open" });
    }
  }
  const textOrBuffer = readFileMaybeDecode(data, options?.encoding);
  return textOrBuffer;
}

// -- readlink --

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

  PromisePrototypeThen(
    Deno.readLink(path),
    (data: string) => {
      const res = readlinkMaybeEncode(data, encoding);
      if (cb) cb(null, res);
    },
    (err: Error) => {
      if (cb) {
        (cb as (e: Error) => void)(denoErrorToNodeError(err, {
          syscall: "readlink",
          path,
        }));
      }
    },
  );
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

// -- statfs --

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

function opResultToStatFs(
  result: StatFsOpResult,
  bigint: true,
): StatFs<bigint>;
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
      op_node_fs_close(fd);
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
  op_node_fs_close(fd);
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
    op_node_fs_fchown(fd, uid, gid),
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
    op_node_fs_fchmod(fd, mode),
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

  op_node_fs_fchown_sync(fd, uid, gid);
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
    op_node_fs_ftruncate(fd, len ?? 0),
    () => callback(null),
    callback,
  );
}

function ftruncateSync(fd: number, len?: number) {
  op_node_fs_ftruncate_sync(fd, len ?? 0);
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

  const atimeSecs = MathTrunc(atime as number);
  const atimeNanos = MathTrunc(
    ((atime as number) - atimeSecs) * 1e9,
  );
  const mtimeSecs = MathTrunc(mtime as number);
  const mtimeNanos = MathTrunc(
    ((mtime as number) - mtimeSecs) * 1e9,
  );
  PromisePrototypeThen(
    op_node_fs_futimes(fd, atimeSecs, atimeNanos, mtimeSecs, mtimeNanos),
    () => callback(null),
    callback,
  );
}

function fchmodSync(fd: number, mode: string | number) {
  validateInteger(fd, "fd", 0, 2147483647);

  op_node_fs_fchmod_sync(fd, parseFileMode(mode, "mode"));
}

function fdatasync(
  fd: number,
  callback: CallbackWithError,
) {
  validateInt32(fd, "fd", 0);
  PromisePrototypeThen(
    op_node_fs_fdatasync(fd),
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

  const atimeSecs = MathTrunc(atime as number);
  const atimeNanos = MathTrunc(
    ((atime as number) - atimeSecs) * 1e9,
  );
  const mtimeSecs = MathTrunc(mtime as number);
  const mtimeNanos = MathTrunc(
    ((mtime as number) - mtimeSecs) * 1e9,
  );
  op_node_fs_futimes_sync(fd, atimeSecs, atimeNanos, mtimeSecs, mtimeNanos);
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
  op_node_fs_fdatasync_sync(fd);
}

function fsync(
  fd: number,
  callback: CallbackWithError,
) {
  validateInt32(fd, "fd", 0);
  PromisePrototypeThen(
    op_node_fs_fsync(fd),
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
  op_node_fs_fsync_sync(fd);
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

// -- mkdir --

type MkdirCallback =
  | ((err: Error | null, path?: string) => void)
  | CallbackWithError;

/**
 * On Windows, recursive mkdir through a file returns EEXIST instead of
 * ENOTDIR. Check if any component of the path is a file and fix the error.
 */
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

/** Find the first component of `path` that does not exist. */
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

function open(path: string | Buffer | URL, callback: OpenCallback): void;
function open(
  path: string | Buffer | URL,
  flags: OpenFlags,
  callback: OpenCallback,
): void;
function open(
  path: string | Buffer | URL,
  flags: OpenFlags,
  mode: number,
  callback: OpenCallback,
): void;
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

function openSync(path: string | Buffer | URL): number;
function openSync(
  path: string | Buffer | URL,
  flags?: OpenFlags,
): number;
function openSync(path: string | Buffer | URL, mode?: number): number;
function openSync(
  path: string | Buffer | URL,
  flags?: OpenFlags,
  mode?: number,
): number;
function openSync(
  path: string | Buffer | URL,
  flags: OpenFlags = "r",
  maybeMode?: number,
) {
  path = getValidatedPathToString(path);
  flags = stringToFlags(flags);
  const mode = parseFileMode(maybeMode, "mode", 0o666);

  try {
    return op_node_open_sync(path, flags, mode);
  } catch (err) {
    throw denoErrorToNodeError(err as Error, { syscall: "open", path });
  }
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

    /** Throws if path is invalid */
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

function opendirSync(
  path: string | Buffer | URL,
  options?: OpendirOptions,
): Dir {
  path = _opendirGetPathString(getValidatedPath(path));

  const { bufferSize } = getOptions(options, {
    encoding: "utf8",
    bufferSize: 32,
  });

  validateInteger(bufferSize, "options.bufferSize", 1, 4294967295);

  try {
    /** Throws if path is invalid */
    Deno.readDirSync(path);

    return new Dir(path);
  } catch (err) {
    throw denoErrorToNodeError(err as Error, { syscall: "opendir" });
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

function writeSync(
  fd: number,
  buffer: ArrayBufferView | string,
  offsetOrOptions?: number | WriteOptions | null,
  length?: number | null,
  position?: number | null,
): number {
  fd = getValidatedFd(fd);

  const innerWriteSync = (
    fd: number,
    buffer: ArrayBufferView | Uint8Array,
    offset: number,
    length: number,
    position: number | null | undefined,
  ) => {
    buffer = arrayBufferViewToUint8Array(buffer);
    const pos = typeof position === "number" && position >= 0 ? position : -1;
    return op_node_fs_write_sync(
      fd,
      (buffer as Uint8Array).subarray(offset, offset + length),
      pos,
    );
  };

  let offset = offsetOrOptions;
  if (isArrayBufferView(buffer)) {
    if (typeof offset === "object") {
      ({
        offset = 0,
        // deno-lint-ignore prefer-primordials
        length = buffer.byteLength - (offset as number),
        position = null,
      } = offsetOrOptions ?? kEmptyObject);
    }
    if (position === undefined) {
      position = null;
    }
    if (offset == null) {
      offset = 0;
    } else {
      validateInteger(offset, "offset", 0);
    }
    if (typeof length !== "number") {
      // deno-lint-ignore prefer-primordials
      length = buffer.byteLength - offset;
    }
    // deno-lint-ignore prefer-primordials
    validateOffsetLengthWrite(offset, length, buffer.byteLength);
    return innerWriteSync(fd, buffer, offset, length, position);
  }
  validateStringAfterArrayBufferView(buffer, "buffer");
  validateEncoding(buffer, length);
  buffer = Buffer.from(buffer, length);
  return innerWriteSync(fd, buffer, 0, buffer.length, position);
}

/** Writes the buffer to the file of the given descriptor.
 * https://nodejs.org/api/fs.html#fswritefd-buffer-offset-length-position-callback
 * https://github.com/nodejs/node/blob/42ad4137aadda69c51e1df48eee9bc2e5cebca5c/lib/fs.js#L797
 */
function write(
  fd: number,
  buffer: ArrayBufferView | string,
  offsetOrOptions?: number | WriteOptions | WriteCallback | null,
  length?: number | WriteCallback | null,
  position?: number | WriteCallback | null,
  callback?: WriteCallback,
) {
  fd = getValidatedFd(fd);

  const innerWrite = async (
    fd: number,
    buffer: ArrayBufferView | Uint8Array,
    offset: number,
    length: number,
    position: number | null | undefined,
  ) => {
    buffer = arrayBufferViewToUint8Array(buffer);
    const pos = typeof position === "number" && position >= 0 ? position : -1;
    return await op_node_fs_write_deferred(
      fd,
      (buffer as Uint8Array).subarray(offset, offset + length),
      pos,
    );
  };

  let offset = offsetOrOptions;
  if (isArrayBufferView(buffer)) {
    callback = maybeCallback(callback || position || length || offset);

    if (typeof offset === "object") {
      ({
        offset = 0,
        // deno-lint-ignore prefer-primordials
        length = buffer.byteLength - (offset as number),
        position = null,
      } = offsetOrOptions ?? kEmptyObject);
    }
    if (offset == null || typeof offset === "function") {
      offset = 0;
    } else {
      validateInteger(offset, "offset", 0);
    }
    if (typeof length !== "number") {
      // deno-lint-ignore prefer-primordials
      length = buffer.byteLength - offset;
    }
    if (typeof position !== "number") {
      position = null;
    }
    // deno-lint-ignore prefer-primordials
    validateOffsetLengthWrite(offset, length, buffer.byteLength);
    // deno-lint-ignore prefer-primordials
    innerWrite(fd, buffer, offset, length, position).then(
      (nwritten) => {
        callback!(null, nwritten, buffer);
      },
      (err) => callback!(err),
    );
    return;
  }

  // Here the call signature is
  // `fs.write(fd, string[, position[, encoding]], callback)`

  validateStringAfterArrayBufferView(buffer, "buffer");

  if (typeof position !== "function") {
    if (typeof offset === "function") {
      position = offset;
      offset = null;
    } else {
      position = length;
    }
    length = "utf-8";
  }

  const str = buffer;
  validateEncoding(str, length);
  callback = maybeCallback(position);
  buffer = Buffer.from(str, length);

  // deno-lint-ignore prefer-primordials
  innerWrite(fd, buffer, 0, buffer.length, offset).then(
    (nwritten) => {
      callback(null, nwritten, buffer);
    },
    (err) => callback(err),
  );
}

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
function writev(
  fd: number,
  buffers: ReadonlyArray<ArrayBufferView>,
  position?: number | null,
  callback?: writeVCallback,
): void {
  const innerWritev = async (fd, buffers, position) => {
    const chunks: Buffer[] = [];
    for (let i = 0; i < buffers.length; i++) {
      if (Buffer.isBuffer(buffers[i])) {
        // deno-lint-ignore prefer-primordials
        chunks.push(buffers[i]);
      } else {
        // deno-lint-ignore prefer-primordials
        chunks.push(Buffer.from(buffers[i]));
      }
    }
    const pos = typeof position === "number" ? position : -1;
    // deno-lint-ignore prefer-primordials
    const buffer = Buffer.concat(chunks);
    return await op_node_fs_write_deferred(fd, buffer, pos);
  };

  fd = getValidatedFd(fd);
  validateBufferArray(buffers);
  callback = maybeCallback(callback || position);

  if (buffers.length === 0) {
    process.nextTick(callback, null, 0, buffers);
    return;
  }

  if (typeof position !== "number") position = null;

  // deno-lint-ignore prefer-primordials
  innerWritev(fd, buffers, position).then(
    (nwritten) => callback(null, nwritten, buffers),
    (err) => callback(err),
  );
}

/**
 * For detailed information, see the documentation of the asynchronous version of
 * this API: {@link writev}.
 * @since v12.9.0
 * @return The number of bytes written.
 */
function writevSync(
  fd: number,
  buffers: ArrayBufferView[],
  position?: number | null,
): number {
  const innerWritev = (fd, buffers, position) => {
    const chunks: Buffer[] = [];
    for (let i = 0; i < buffers.length; i++) {
      if (Buffer.isBuffer(buffers[i])) {
        // deno-lint-ignore prefer-primordials
        chunks.push(buffers[i]);
      } else {
        // deno-lint-ignore prefer-primordials
        chunks.push(Buffer.from(buffers[i]));
      }
    }
    const pos = typeof position === "number" ? position : -1;
    // deno-lint-ignore prefer-primordials
    const buffer = Buffer.concat(chunks);
    return op_node_fs_write_sync(fd, buffer, pos);
  };

  fd = getValidatedFd(fd);
  validateBufferArray(buffers);

  if (buffers.length === 0) {
    return 0;
  }

  if (typeof position !== "number") position = null;

  return innerWritev(fd, buffers, position);
}

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
  try {
    return await op_node_open(pathOrRid, stringToFlags(flag), 0o666);
  } catch (err) {
    throw denoErrorToNodeError(err as Error, {
      syscall: "open",
      path: pathOrRid,
    });
  }
}

function _writeFileGetRidSync(
  pathOrRid: string | number,
  flag: string = "w",
): number {
  if (typeof pathOrRid === "number") {
    return pathOrRid;
  }
  try {
    return op_node_open_sync(pathOrRid, stringToFlags(flag), 0o666);
  } catch (err) {
    throw denoErrorToNodeError(err as Error, {
      syscall: "open",
      path: pathOrRid,
    });
  }
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
  } else if (ObjectPrototypeIsPrototypeOf(FileHandle.prototype, pathOrRid)) {
    pathOrRid = (pathOrRid as FileHandle).fd;
  }

  if (isFileOptions(options)) {
    flag = options.flag;
    mode = options.mode;
    signal = options.signal;
  }

  const encoding = getValidatedEncoding(options) || "utf8";

  if (!ArrayBufferIsView(data) && !_isCustomIterable(data)) {
    validateStringAfterArrayBufferView(data, "data");
    data = Buffer.from(data, encoding);
  }

  const isRid = typeof pathOrRid === "number";
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
        await Deno.chmod(pathOrRid as string, mode);
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

function writeFileSync(
  pathOrRid: string | number | URL,
  data: WriteFileSyncData,
  options?: Encodings | WriteFileOptions,
) {
  let flag: string | undefined;
  let mode: number | undefined;

  pathOrRid = ObjectPrototypeIsPrototypeOf(URLPrototype, pathOrRid)
    ? pathFromURL(pathOrRid as URL)
    : pathOrRid as string | number;

  if (isFileOptions(options)) {
    flag = options.flag;
    mode = options.mode;
  }

  const encoding = getValidatedEncoding(options) || "utf8";

  if (!ArrayBufferIsView(data) && !_isCustomIterable(data)) {
    validateStringAfterArrayBufferView(data, "data");
    data = Buffer.from(data, encoding);
  }

  const isRid = typeof pathOrRid === "number";
  let file;

  let error: Error | null = null;
  try {
    const fd = _writeFileGetRidSync(pathOrRid, flag);
    file = {
      write(p: NodeJS.TypedArray) {
        return PromiseResolve(op_node_fs_write_sync(fd, p, -1));
      },
      writeSync(p: NodeJS.TypedArray) {
        return op_node_fs_write_sync(fd, p, -1);
      },
      close() {
        op_node_fs_close(fd);
      },
    };

    if (!isRid && mode) {
      Deno.chmodSync(pathOrRid as string, mode);
    }

    _writeAllSync(
      file,
      data as (Exclude<WriteFileSyncData, string>),
      encoding,
    );
  } catch (e) {
    error = denoWriteFileErrorToNodeError(e as Error, { syscall: "write" });
  } finally {
    // Make sure to close resource
    if (!isRid && file) file.close();
  }

  if (error) throw error;
}

function _writeAllSync(
  w: Writer,
  data: Exclude<WriteFileSyncData, string>,
  encoding: BufferEncoding,
) {
  if (!_isCustomIterable(data)) {
    // deno-lint-ignore prefer-primordials
    data = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    // deno-lint-ignore prefer-primordials
    let remaining = data.byteLength;
    while (remaining > 0) {
      const bytesWritten = w.writeSync(
        // deno-lint-ignore prefer-primordials
        data.subarray(data.byteLength - remaining),
      );
      remaining -= bytesWritten;
    }
  } else {
    // deno-lint-ignore prefer-primordials
    for (const buf of data) {
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
        const bytesWritten = w.writeSync(
          // deno-lint-ignore prefer-primordials
          toWrite.subarray(toWrite.byteLength - remaining),
        );
        remaining -= bytesWritten;
      }
    }
  }
}

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
  return isIterable(obj) && !ArrayBufferIsView(obj) && typeof obj !== "string";
}

function _checkAborted(signal?: AbortSignal) {
  if (signal?.aborted) {
    throw new AbortError();
  }
}

// -- truncate --

function truncate(
  path: string | URL,
  lenOrCallback: number | CallbackWithError,
  maybeCallback?: CallbackWithError,
) {
  path = ObjectPrototypeIsPrototypeOf(URLPrototype, path)
    ? pathFromURL(path)
    : path;
  const len: number | undefined = typeof lenOrCallback === "number"
    ? lenOrCallback
    : undefined;
  const callback: CallbackWithError = typeof lenOrCallback === "function"
    ? lenOrCallback
    : maybeCallback as CallbackWithError;

  if (!callback) throw new Error("No callback function supplied");

  PromisePrototypeThen(
    Deno.truncate(path, len),
    () => callback(null),
    callback,
  );
}

function truncateSync(path: string | URL, len?: number) {
  path = ObjectPrototypeIsPrototypeOf(URLPrototype, path)
    ? pathFromURL(path)
    : path;

  Deno.truncateSync(path, len);
}

// -- utimes --

function getValidTime(
  time: number | string | Date,
  name: string,
): number {
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

  PromisePrototypeThen(
    Deno.utime(path, atime, mtime),
    () => callback(null),
    callback,
  );
}

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

// -- symlink --

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
      // Symlinks targets can be relative to the newly created path.
      // Calculate absolute file name of the symlink target, and check
      // if it is a directory. Ignore resolve error to keep symlink
      // errors consistent between platforms if invalid path is
      // provided.
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

// -- watch --

const statPromisified = promisify(stat);
const statAsync = async (filename: string): Promise<Stats | null> => {
  try {
    return await statPromisified(filename);
  } catch {
    return emptyStats;
  }
};
const emptyStats = new Stats(
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  DateUTC(1970, 0, 1, 0, 0, 0),
  DateUTC(1970, 0, 1, 0, 0, 0),
  DateUTC(1970, 0, 1, 0, 0, 0),
  DateUTC(1970, 0, 1, 0, 0, 0),
) as unknown as Stats;

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

type watchOptions = {
  persistent?: boolean;
  recursive?: boolean;
  encoding?: string;
};

type watchListener = (eventType: string, filename: string) => void;

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

  // deno-lint-ignore prefer-primordials
  const watchPath = getValidatedPath(filename).toString();

  const iterator: Deno.FsWatcher = Deno.watchFs(watchPath, {
    recursive: options?.recursive || false,
  });

  asyncIterableToCallback<Deno.FsEvent>(iterator, (val, done) => {
    if (done) return;
    fsWatcher.emit(
      "change",
      convertDenoFsEventToNodeFsEvent(val.kind),
      basename(val.paths[0]),
    );
  }, (e) => {
    fsWatcher.emit("error", e);
  });

  const fsWatcher = new FSWatcher(() => {
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

  return fsWatcher;
}

function watchPromise(
  filename: string | Buffer | URL,
  options?: {
    persistent?: boolean;
    recursive?: boolean;
    encoding?: string;
    signal?: AbortSignal;
  },
): AsyncIterable<{ eventType: string; filename: string | Buffer | null }> {
  // deno-lint-ignore prefer-primordials
  const watchPath = getValidatedPath(filename).toString();

  const watcher = Deno.watchFs(watchPath, {
    recursive: options?.recursive ?? false,
  });

  if (options?.signal) {
    if (options.signal.aborted) {
      watcher.close();
    } else {
      options.signal.addEventListener(
        "abort",
        () => watcher.close(),
        { once: true },
      );
    }
  }

  const fsIterable = watcher[SymbolAsyncIterator]();
  const result = {
    async next(): Promise<
      IteratorResult<{ eventType: string; filename: string | Buffer | null }>
    > {
      // deno-lint-ignore prefer-primordials
      const iterResult = await fsIterable.next();
      if (iterResult.done) return iterResult;

      const eventType = convertDenoFsEventToNodeFsEvent(
        iterResult.value.kind,
      );
      return {
        value: { eventType, filename: basename(iterResult.value.paths[0]) },
        done: false,
      };
    },
    // deno-lint-ignore no-explicit-any
    return(value?: any): Promise<IteratorResult<any>> {
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

    (async () => {
      let prev = await statAsync(filename);

      if (prev === emptyStats) {
        this.emit("change", prev, prev);
      }

      try {
        while (true) {
          await delay(interval, { signal: this.#abortController.signal });
          const curr = await statAsync(filename);
          if (
            DatePrototypeGetTime(curr?.mtime) !==
              DatePrototypeGetTime(prev?.mtime)
          ) {
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
    this.emit("stop");
  }
  ref() {
    notImplemented("StatWatcher.ref() is not implemented");
  }
  unref() {
    notImplemented("StatWatcher.unref() is not implemented");
  }
}

class FSWatcher extends EventEmitter {
  #closer: () => void;
  #closed = false;
  #watcher: () => Deno.FsWatcher;

  constructor(closer: () => void, getter: () => Deno.FsWatcher) {
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
    this.#watcher().ref();
  }
  unref() {
    this.#watcher().unref();
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
  open,
  openAsBlob,
  openSync,
  opendir,
  opendirSync,
  read,
  readSync,
  promises,
  readdir,
  readdirSync,
  readFile,
  readFilePromise,
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
  symlinkSync,
  truncate,
  truncateSync,
  unlink,
  unlinkSync,
  unwatchFile,
  utimes,
  utimesSync,
  watch,
  watchFile,
  write,
  writeFile,
  writev,
  writevSync,
  writeFileSync,
  WriteStream,
  writeSync,
  // For tests
  _toUnixTimestamp,
};

export type { ReadVResult, statCallback, statCallbackBigInt, statOptions };

export {
  // For tests
  _toUnixTimestamp,
  access,
  accessSync,
  appendFile,
  appendFileSync,
  BigIntStats,
  CFISBIS,
  chmod,
  chmodSync,
  chown,
  chownSync,
  close,
  closeSync,
  constants,
  convertFileInfoToBigIntStats,
  convertFileInfoToStats,
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
  open,
  openAsBlob,
  opendir,
  opendirSync,
  openSync,
  promises,
  read,
  readdir,
  readdirSync,
  readFile,
  readFilePromise,
  readFileSync,
  readlink,
  readlinkPromise,
  readlinkSync,
  ReadStream,
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
  watch,
  watchFile,
  watchPromise,
  write,
  writeFile,
  writeFileSync,
  WriteStream,
  writeSync,
  writev,
  writevSync,
};
