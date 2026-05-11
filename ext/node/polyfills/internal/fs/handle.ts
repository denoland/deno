// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials no-explicit-any no-node-globals

import { core, primordials } from "ext:core/mod.js";
const { EventEmitter } = core.loadExtScript("ext:deno_node/_events.mjs");
const lazyFs = core.createLazyLoader("node:fs");
const lazyReadline = core.createLazyLoader("node:readline");
import { op_node_fs_close } from "ext:core/ops";
import type {
  BinaryOptionsArgument,
  FileOptionsArgument,
  TextOptionsArgument,
} from "ext:deno_node/_fs/_fs_common.ts";
// writev loaded lazily from node:fs via lazyFs

export interface WriteVResult {
  bytesWritten: number;
  buffers: ReadonlyArray<ArrayBufferView>;
}

function writevPromise(
  fd: number,
  buffers: ArrayBufferView[],
  position?: number,
): Promise<WriteVResult> {
  return new Promise((resolve, reject) => {
    lazyFs().writev(fd, buffers, position, (err, bytesWritten, buffers) => {
      if (err) reject(err);
      else resolve({ bytesWritten, buffers });
    });
  });
}
// readvPromise loaded lazily from node:fs via lazyFs
const { fstatPromise } = core.loadExtScript("ext:deno_node/_fs/_fs_fstat.ts");
// fchown, ftruncate, futimes loaded lazily from node:fs via lazyFs
const { kEmptyObject, promisify } = core.loadExtScript(
  "ext:deno_node/internal/util.mjs",
);

// CreateReadStreamOptions, CreateWriteStreamOptions types from node:fs/promises
const { default: assert } = core.loadExtScript("ext:deno_node/assert.ts");
const {
  denoErrorToNodeError,
  ERR_INVALID_STATE,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const { readableStreamCancel } = core.loadExtScript(
  "ext:deno_web/06_streams.js",
);
const {
  validateBoolean,
  validateObject,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const lazyProcess = core.createLazyLoader("node:process");

const fchmodPromise = promisify(lazyFs().fchmod) as (
  fd: number,
  mode: string | number,
) => Promise<void>;
const fdatasyncPromise = promisify(lazyFs().fdatasync) as (
  fd: number,
) => Promise<void>;
const fsyncPromise = promisify(lazyFs().fsync) as (fd: number) => Promise<void>;

const {
  Error,
  ObjectAssign,
  ObjectPrototypeIsPrototypeOf,
  Promise,
  PromisePrototypeThen,
  SafePromisePrototypeFinally,
  PromiseResolve,
  SafeArrayIterator,
  Symbol,
  SymbolAsyncDispose,
  Uint8ArrayPrototype,
} = primordials;

const kRefs = Symbol("kRefs");
const kClosePromise = Symbol("kClosePromise");
const kCloseResolve = Symbol("kCloseResolve");
const kCloseReject = Symbol("kCloseReject");
export const kRef = Symbol("kRef");
export const kUnref = Symbol("kUnref");
const kLocked = Symbol("kLocked");

const ftruncatePromise = promisify(lazyFs().ftruncate);
const fchownPromise = promisify(lazyFs().fchown);
const futimesPromise = promisify(lazyFs().futimes);

interface WriteResult {
  bytesWritten: number;
  buffer: Buffer | string;
}

interface ReadResult {
  bytesRead: number;
  buffer: Buffer;
}

export class FileHandle extends EventEmitter {
  #rid: number;
  [kRefs]: number;
  [kClosePromise]?: Promise<void> | null;
  [kCloseResolve]?: () => void;
  [kCloseReject]?: (err: Error) => void;
  [kLocked]: boolean;

  constructor(rid: number) {
    super();
    this.#rid = rid;

    this[kRefs] = 1;
    this[kClosePromise] = null;
    this[kLocked] = false;
  }

  get fd() {
    return this.#rid;
  }

  read(
    buffer: ArrayBufferView,
    offset?: number,
    length?: number,
    position?: number | null,
  ): Promise<ReadResult>;
  read(
    buffer: ArrayBufferView,
    options?: any,
  ): Promise<ReadResult>;
  read(options?: any): Promise<ReadResult>;
  read(
    bufferOrOpt?: ArrayBufferView | any,
    offsetOrOpt?: number | any,
    length?: number,
    position?: number | null,
  ): Promise<ReadResult> {
    return fsCall(
      readPromise,
      "read",
      this,
      bufferOrOpt,
      offsetOrOpt,
      length,
      position,
    );
  }

  truncate(len?: number): Promise<void> {
    return fsCall(ftruncatePromise, "ftruncate", this, len);
  }

  readFile(
    opt?: TextOptionsArgument | BinaryOptionsArgument | FileOptionsArgument,
  ): Promise<string | Buffer> {
    return fsCall(lazyFs().promises.readFile, "readFile", this, opt);
  }

  write(
    buffer: Buffer,
    offset: number,
    length: number,
    position: number,
  ): Promise<WriteResult>;
  write(str: string, position: number, encoding: string): Promise<WriteResult>;
  write(
    bufferOrStr: Uint8Array | string,
    offsetOrPosition: number,
    lengthOrEncoding: number | string,
    position?: number,
  ): Promise<WriteResult> {
    return fsCall(
      writePromise,
      "write",
      this,
      bufferOrStr,
      offsetOrPosition,
      lengthOrEncoding,
      position,
    );
  }

  writeFile(data, options): Promise<void> {
    return fsCall(
      lazyFs().promises.writeFile,
      "writeFile",
      this,
      data,
      options,
    );
  }

  writev(buffers: ArrayBufferView[], position?: number): Promise<WriteVResult> {
    return fsCall(writevPromise, "writev", this, buffers, position);
  }

  readv(
    buffers: readonly ArrayBufferView[],
    position?: number,
  ): Promise<any> {
    return fsCall(lazyFs().readvPromise, "readv", this, buffers, position);
  }

  [kRef]() {
    this[kRefs]++;
  }

  [kUnref]() {
    this[kRefs]--;
    if (this[kRefs] > 0 || this.fd === -1) {
      return;
    }

    PromisePrototypeThen(
      this.#close(),
      this[kCloseResolve],
      this[kCloseReject],
    );
  }

  #close(): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        op_node_fs_close(this.fd);
        this.#rid = -1;
        resolve();
      } catch (err) {
        reject(denoErrorToNodeError(err as Error, { syscall: "close" }));
      }
    });
  }

  close(): Promise<void> {
    if (this.fd === -1) {
      return PromiseResolve();
    }

    if (this[kClosePromise]) {
      return this[kClosePromise];
    }

    this[kRefs]--;
    if (this[kRefs] === 0) {
      this[kClosePromise] = SafePromisePrototypeFinally(
        this.#close(),
        () => {
          this[kClosePromise] = null;
        },
      );
    } else {
      this[kClosePromise] = SafePromisePrototypeFinally(
        new Promise((resolve, reject) => {
          this[kCloseResolve] = resolve as () => void;
          this[kCloseReject] = reject;
        }),
        () => {
          this[kClosePromise] = null;
          this[kCloseReject] = undefined;
          this[kCloseResolve] = undefined;
        },
      );
    }

    this.emit("close");
    return this[kClosePromise];
  }

  stat(): Promise<any>;
  stat(options: { bigint: false }): Promise<any>;
  stat(options: { bigint: true }): Promise<any>;
  stat(options?: { bigint: boolean }): Promise<any> {
    return fsCall(fstatPromise, "fstat", this, options);
  }

  chmod(mode: any): Promise<void> {
    return fsCall(fchmodPromise, "fchmod", this, mode);
  }

  datasync(): Promise<void> {
    return fsCall(fdatasyncPromise, "fdatasync", this);
  }

  sync(): Promise<void> {
    return fsCall(fsyncPromise, "fsync", this);
  }

  utimes(
    atime: number | string | Date,
    mtime: number | string | Date,
  ): Promise<void> {
    return fsCall(futimesPromise, "futimes", this, atime, mtime);
  }

  chown(uid: number, gid: number): Promise<void> {
    return fsCall(fchownPromise, "fchown", this, uid, gid);
  }

  createReadStream(options?: any): any {
    return new (lazyFs().ReadStream)(undefined, { ...options, fd: this });
  }

  createWriteStream(options?: any): any {
    return new (lazyFs().WriteStream)(undefined, { ...options, fd: this });
  }

  readLines(options?: any): any {
    return lazyReadline().createInterface({
      input: this.createReadStream(options),
      crlfDelay: Infinity,
    });
  }

  readableWebStream(
    options: { autoClose?: boolean; type?: string } = kEmptyObject,
  ): ReadableStream<Uint8Array> {
    if (this.fd === -1) {
      throw new ERR_INVALID_STATE("The FileHandle is closed");
    }
    if (this[kClosePromise]) {
      throw new ERR_INVALID_STATE("The FileHandle is closing");
    }
    if (this[kLocked]) {
      throw new ERR_INVALID_STATE("The FileHandle is locked");
    }
    this[kLocked] = true;

    validateObject(options, "options");
    const autoClose = options?.autoClose ?? false;
    const type = options?.type ?? "bytes";
    validateBoolean(autoClose, "options.autoClose");

    if (type !== "bytes") {
      lazyProcess().default.emitWarning(
        'A non-"bytes" options.type has no effect. A byte-oriented steam is ' +
          "always created.",
        "ExperimentalWarning",
      );
    }

    const ondone = async () => {
      this[kUnref]();
      if (autoClose) {
        await this.close().catch(() => {});
      }
    };

    const readable = new ReadableStream({
      type: "bytes",
      autoAllocateChunkSize: 16384,

      pull: async (controller) => {
        const view = controller.byobRequest!.view! as Uint8Array;
        const { bytesRead } = await this.read(
          view,
          view.byteOffset,
          view.byteLength,
        );

        if (bytesRead === 0) {
          controller.close();
          await ondone();
        }

        controller.byobRequest!.respond(bytesRead);
      },

      cancel: async () => {
        await ondone();
      },
    });

    this[kRef]();
    this.once("close", () => {
      readableStreamCancel(readable);
    });

    return readable;
  }

  [SymbolAsyncDispose]() {
    return this.close();
  }

  appendFile(
    data: string | ArrayBufferView | ArrayBuffer | DataView,
    options?: string | { encoding?: string; mode?: number; flag?: string },
  ): Promise<void> {
    const resolvedOptions = typeof options === "string"
      ? { encoding: options }
      : (options ?? {});

    const optsWithAppend = {
      ...resolvedOptions,
      flag: resolvedOptions.flag ?? "a",
    };

    return fsCall(
      lazyFs().promises.writeFile,
      "writeFile",
      this,
      data,
      optsWithAppend,
    );
  }
}

function readPromise(
  rid: number,
  buffer: ArrayBufferView,
  offset?: number,
  length?: number,
  position?: number | null,
): Promise<ReadResult>;
function readPromise(
  rid: number,
  buffer: ArrayBufferView,
  options?: any,
): Promise<ReadResult>;
function readPromise(
  rid: number,
  options?: any,
): Promise<ReadResult>;
function readPromise(
  rid: number,
  bufferOrOpt?: ArrayBufferView | any,
  offsetOrOpt?: number | any,
  length?: number,
  position?: number | null,
): Promise<ReadResult> {
  if (ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, bufferOrOpt)) {
    if (
      typeof offsetOrOpt !== "number" && typeof length !== "number" &&
      typeof position !== "number"
    ) {
      // fileHandle.read(buffer) or fileHandle.read(buffer, options)
      const opts = (offsetOrOpt ?? {}) as any;
      return new Promise((resolve, reject) => {
        lazyFs().read(
          rid,
          bufferOrOpt,
          opts,
          (err: Error, bytesRead: number, buffer: Buffer) => {
            if (err) reject(err);
            else resolve({ buffer, bytesRead });
          },
        );
      });
    }

    return new Promise((resolve, reject) => {
      lazyFs().read(
        rid,
        bufferOrOpt,
        offsetOrOpt,
        length,
        position,
        (err: Error, bytesRead: number, buffer: Buffer) => {
          if (err) reject(err);
          else resolve({ buffer, bytesRead });
        },
      );
    });
  } else {
    return new Promise((resolve, reject) => {
      lazyFs().read(
        rid,
        bufferOrOpt,
        (err: Error, bytesRead: number, buffer: Buffer) => {
          if (err) reject(err);
          else resolve({ buffer, bytesRead });
        },
      );
    });
  }
}

function writePromise(
  rid: number,
  buffer: Buffer,
  offset: number,
  length: number,
  position: number,
): Promise<WriteResult>;
function writePromise(
  rid: number,
  str: string,
  position: number,
  encoding: string,
): Promise<WriteResult>;
function writePromise(
  rid: number,
  bufferOrStr: Uint8Array | string,
  offsetOrPosition: number,
  lengthOrEncoding: number | string,
  position?: number,
): Promise<WriteResult> {
  if (ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, bufferOrStr)) {
    const buffer = bufferOrStr;
    const offset = offsetOrPosition;
    const length = lengthOrEncoding;

    return new Promise((resolve, reject) => {
      lazyFs().write(
        rid,
        buffer,
        offset,
        length,
        position,
        (err: Error, bytesWritten: number, buffer: Buffer) => {
          if (err) reject(err);
          else resolve({ buffer, bytesWritten });
        },
      );
    });
  } else {
    const str = bufferOrStr;
    const position = offsetOrPosition;
    const encoding = lengthOrEncoding;

    return new Promise((resolve, reject) => {
      lazyFs().write(
        rid,
        str,
        position,
        encoding,
        (err: Error, bytesWritten: number, buffer: Buffer) => {
          if (err) reject(err);
          else resolve({ buffer, bytesWritten });
        },
      );
    });
  }
}

function assertNotClosed(rid: number, syscall: string) {
  if (rid === -1) {
    const err = new Error("file closed");
    throw ObjectAssign(err, {
      code: "EBADF",
      syscall,
    });
  }
}

type FileHandleFn<P, R> = (...args: [number, ...P[]]) => Promise<R>;

async function fsCall<P, R, T extends FileHandleFn<P, R>>(
  fn: T,
  fnName: string,
  handle: FileHandle,
  ...args: P[]
): Promise<R> {
  assert(
    handle[kRefs] !== undefined,
    "handle must be an instance of FileHandle",
  );
  assertNotClosed(handle.fd, fnName);
  try {
    handle[kRef]();
    return await fn(handle.fd, ...new SafeArrayIterator(args));
  } finally {
    handle[kUnref]();
  }
}

export default {
  FileHandle,
};
