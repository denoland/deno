// Copyright 2018-2025 the Deno authors. MIT license.

import { EventEmitter } from "node:events";
import { Buffer } from "node:buffer";
import { Mode, promises, read, ReadStream, write, WriteStream } from "node:fs";
import type { ReadAsyncOptions } from "node:fs";
import { core, primordials } from "ext:core/mod.js";
export type { BigIntStats, Stats } from "ext:deno_node/_fs/_fs_stat.ts";
import {
  BinaryOptionsArgument,
  FileOptionsArgument,
  TextOptionsArgument,
} from "ext:deno_node/_fs/_fs_common.ts";
import { ftruncatePromise } from "ext:deno_node/_fs/_fs_ftruncate.ts";
export type { BigIntStats, Stats } from "ext:deno_node/_fs/_fs_stat.ts";
import { writevPromise, WriteVResult } from "ext:deno_node/_fs/_fs_writev.ts";
import { fdatasyncPromise } from "ext:deno_node/_fs/_fs_fdatasync.ts";
import { fsyncPromise } from "ext:deno_node/_fs/_fs_fsync.ts";
import {
  CreateReadStreamOptions,
  CreateWriteStreamOptions,
} from "node:fs/promises";
const {
  Error,
  ObjectAssign,
  ObjectPrototypeIsPrototypeOf,
  Promise,
  PromiseResolve,
  SafeArrayIterator,
  SymbolAsyncDispose,
  Uint8ArrayPrototype,
} = primordials;

interface WriteResult {
  bytesWritten: number;
  buffer: Buffer | string;
}

interface ReadResult {
  bytesRead: number;
  buffer: Buffer;
}

type Path = string | Buffer | URL;
export class FileHandle extends EventEmitter {
  #rid: number;
  #path: Path;

  constructor(rid: number, path: Path) {
    super();
    this.#rid = rid;
    this.#path = path;
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
    if (ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, bufferOrOpt)) {
      if (arguments.length === 2) {
        return new Promise((resolve, reject) => {
          read(
            this.fd,
            bufferOrOpt,
            (err, bytesRead, buffer) => {
              if (err) reject(err);
              else resolve({ buffer, bytesRead });
            },
          );
        });
      }

      return new Promise((resolve, reject) => {
        read(
          this.fd,
          bufferOrOpt,
          offsetOrOpt,
          length,
          position,
          (err, bytesRead, buffer) => {
            if (err) reject(err);
            else resolve({ buffer, bytesRead });
          },
        );
      });
    } else {
      return new Promise((resolve, reject) => {
        read(this.fd, bufferOrOpt, (err, bytesRead, buffer) => {
          if (err) reject(err);
          else resolve({ buffer, bytesRead });
        });
      });
    }
  }

  truncate(len?: number): Promise<void> {
    return fsCall(ftruncatePromise, this, len);
  }

  readFile(
    opt?: TextOptionsArgument | BinaryOptionsArgument | FileOptionsArgument,
  ): Promise<string | Buffer> {
    return promises.readFile(this, opt);
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
    if (ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, bufferOrStr)) {
      const buffer = bufferOrStr;
      const offset = offsetOrPosition;
      const length = lengthOrEncoding;

      return new Promise((resolve, reject) => {
        write(
          this.fd,
          buffer,
          offset,
          length,
          position,
          (err, bytesWritten, buffer) => {
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
        write(this.fd, str, position, encoding, (err, bytesWritten, buffer) => {
          if (err) reject(err);
          else resolve({ buffer, bytesWritten });
        });
      });
    }
  }

  writeFile(data, options): Promise<void> {
    return fsCall(promises.writeFile, this, data, options);
  }

  writev(buffers: ArrayBufferView[], position?: number): Promise<WriteVResult> {
    return fsCall(writevPromise, this, buffers, position);
  }

  close(): Promise<void> {
    // Note that Deno.close is not async
    return PromiseResolve(core.close(this.fd));
  }

  stat(): Promise<Stats>;
  stat(options: { bigint: false }): Promise<Stats>;
  stat(options: { bigint: true }): Promise<BigIntStats>;
  stat(options?: { bigint: boolean }): Promise<Stats | BigIntStats> {
    return fsCall(promises.fstat, this, options);
  }
  chmod(mode: Mode): Promise<void> {
    assertNotClosed(this, promises.chmod.name);
    return promises.chmod(this.#path, mode);
  }

  datasync(): Promise<void> {
    return fsCall(fdatasyncPromise, this);
  }

  sync(): Promise<void> {
    return fsCall(fsyncPromise, this);
  }

  utimes(
    atime: number | string | Date,
    mtime: number | string | Date,
  ): Promise<void> {
    assertNotClosed(this, promises.utimes.name);
    return promises.utimes(this.#path, atime, mtime);
  }

  chown(uid: number, gid: number): Promise<void> {
    assertNotClosed(this, promises.chown.name);
    return promises.chown(this.#path, uid, gid);
  }

  createReadStream(options?: CreateReadStreamOptions): ReadStream {
    return new ReadStream(undefined, { ...options, fd: this.fd });
  }

  createWriteStream(options?: CreateWriteStreamOptions): WriteStream {
    return new WriteStream(undefined, { ...options, fd: this.fd });
  }

  [SymbolAsyncDispose]() {
    return this.close();
  }
}

function assertNotClosed(handle: FileHandle, syscall: string) {
  if (handle.fd === -1) {
    const err = new Error("file closed");
    throw ObjectAssign(err, {
      code: "EBADF",
      syscall,
    });
  }
}

function fsCall(fn, handle, ...args) {
  assertNotClosed(handle, fn.name);
  return fn(handle.fd, ...new SafeArrayIterator(args));
}

export default {
  FileHandle,
};
