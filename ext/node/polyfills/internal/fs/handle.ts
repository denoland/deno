// Copyright 2018-2025 the Deno authors. MIT license.

import { EventEmitter } from "node:events";
import { Buffer } from "node:buffer";
import {
  type BigIntStats,
  Mode,
  promises,
  read as readAsync,
  type ReadAsyncOptions,
  ReadStream,
  type Stats,
  write as writeAsync,
  WriteStream,
} from "node:fs";
import { createInterface } from "node:readline";
import type { Interface as ReadlineInterface } from "node:readline";
import { core, primordials } from "ext:core/mod.js";
import {
  BinaryOptionsArgument,
  FileOptionsArgument,
  TextOptionsArgument,
} from "ext:deno_node/_fs/_fs_common.ts";
import { ftruncatePromise } from "ext:deno_node/_fs/_fs_ftruncate.ts";
import { writevPromise, WriteVResult } from "ext:deno_node/_fs/_fs_writev.ts";
import { fchmodPromise } from "ext:deno_node/_fs/_fs_fchmod.ts";
import { fchownPromise } from "ext:deno_node/_fs/_fs_fchown.ts";
import { fdatasyncPromise } from "ext:deno_node/_fs/_fs_fdatasync.ts";
import { fstatPromise } from "ext:deno_node/_fs/_fs_fstat.ts";
import { fsyncPromise } from "ext:deno_node/_fs/_fs_fsync.ts";
import { futimesPromise } from "ext:deno_node/_fs/_fs_futimes.ts";
import {
  CreateReadStreamOptions,
  CreateWriteStreamOptions,
} from "node:fs/promises";
import assert from "node:assert";
import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";

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
const kRef = Symbol("kRef");
const kUnref = Symbol("kUnref");

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

  constructor(rid: number) {
    super();
    this.#rid = rid;

    this[kRefs] = 1;
    this[kClosePromise] = null;
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
    options?: ReadAsyncOptions<NodeJS.ArrayBufferView>,
  ): Promise<ReadResult>;
  read(options?: ReadAsyncOptions<NodeJS.ArrayBufferView>): Promise<ReadResult>;
  read(
    bufferOrOpt?: ArrayBufferView | ReadAsyncOptions<NodeJS.ArrayBufferView>,
    offsetOrOpt?: number | ReadAsyncOptions<NodeJS.ArrayBufferView>,
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
    return fsCall(promises.readFile, "readFile", this, opt);
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
    return fsCall(promises.writeFile, "writeFile", this, data, options);
  }

  writev(buffers: ArrayBufferView[], position?: number): Promise<WriteVResult> {
    return fsCall(writevPromise, "writev", this, buffers, position);
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
        core.close(this.fd);
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

  stat(): Promise<Stats>;
  stat(options: { bigint: false }): Promise<Stats>;
  stat(options: { bigint: true }): Promise<BigIntStats>;
  stat(options?: { bigint: boolean }): Promise<Stats | BigIntStats> {
    return fsCall(fstatPromise, "fstat", this, options);
  }

  chmod(mode: Mode): Promise<void> {
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

  createReadStream(options?: CreateReadStreamOptions): ReadStream {
    return new ReadStream(undefined, { ...options, fd: this.fd });
  }

  createWriteStream(options?: CreateWriteStreamOptions): WriteStream {
    return new WriteStream(undefined, { ...options, fd: this.fd });
  }

  readLines(options?: CreateReadStreamOptions): ReadlineInterface {
    return createInterface({
      input: this.createReadStream({ ...options, autoClose: false }),
      crlfDelay: Infinity,
    });
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

    return fsCall(promises.writeFile, "writeFile", this, data, optsWithAppend);
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
  options?: ReadAsyncOptions<NodeJS.ArrayBufferView>,
): Promise<ReadResult>;
function readPromise(
  rid: number,
  options?: ReadAsyncOptions<NodeJS.ArrayBufferView>,
): Promise<ReadResult>;
function readPromise(
  rid: number,
  bufferOrOpt?: ArrayBufferView | ReadAsyncOptions<NodeJS.ArrayBufferView>,
  offsetOrOpt?: number | ReadAsyncOptions<NodeJS.ArrayBufferView>,
  length?: number,
  position?: number | null,
): Promise<ReadResult> {
  if (ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, bufferOrOpt)) {
    if (typeof length !== "number" && typeof position !== "number") {
      return new Promise((resolve, reject) => {
        readAsync(
          rid,
          bufferOrOpt,
          offsetOrOpt,
          (err: Error, bytesRead: number, buffer: Buffer) => {
            if (err) reject(err);
            else resolve({ buffer, bytesRead });
          },
        );
      });
    }

    return new Promise((resolve, reject) => {
      readAsync(
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
      readAsync(
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
      writeAsync(
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
      writeAsync(
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
