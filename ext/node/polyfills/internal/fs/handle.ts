// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { EventEmitter } from "node:events";
import { Buffer } from "node:buffer";
import {
  fdatasync,
  fstat,
  fsync,
  ftruncate,
  futimes,
  promises,
  read,
  write,
} from "node:fs";
import {
  BinaryOptionsArgument,
  FileOptionsArgument,
  ReadOptions,
  TextOptionsArgument,
  WriteFileOptions,
} from "ext:deno_node/_fs/_fs_common.ts";
import { core } from "ext:core/mod.js";
import { promisify } from "ext:deno_node/internal/util.mjs";
import { notImplemented } from "ext:deno_node/_utils.ts";

interface WriteResult {
  bytesWritten: number;
  buffer: Buffer | string;
}

interface ReadResult {
  bytesRead: number;
  buffer: Buffer;
}

const fdatasyncPromise = promisify(fdatasync);
const fstatPromise = promisify(fstat);
const fsyncPromise = promisify(fsync);
const ftruncatePromise = promisify(ftruncate);
const futimesPromise = promisify(futimes);

export class FileHandle extends EventEmitter {
  #rid: number;
  constructor(rid: number) {
    super();
    this.#rid = rid;
  }

  appendFile(
    data: string | Uint8Array,
    options?: WriteFileOptions,
  ): Promise<void> {
    return promises.appendFile(this.fd, data, options);
  }

  chmod(_mode: number) {
    notImplemented("FileHandle.chmod()");
  }

  chown(_uid: number, _gid: number) {
    notImplemented("FileHandle.chown()");
  }

  close(): Promise<void> {
    // TODO(lucacasonato): wait for ongoing operations to complete
    this.emit("close");
    return Promise.resolve(core.tryClose(this.fd));
  }

  createReadStream(_options?: unknown) {
    notImplemented("FileHandle.createReadStream()");
  }

  createWriteStream(_options?: unknown) {
    notImplemented("FileHandle.createWriteStream()");
  }

  datasync() {
    return fdatasyncPromise(this.fd);
  }

  get fd() {
    return this.#rid;
  }

  read(
    buffer: Uint8Array,
    offset?: number,
    length?: number,
    position?: number | null,
  ): Promise<ReadResult>;
  read(options?: ReadOptions): Promise<ReadResult>;
  read(
    bufferOrOpt: Uint8Array | ReadOptions,
    offset?: number,
    length?: number,
    position?: number | null,
  ): Promise<ReadResult> {
    if (bufferOrOpt instanceof Uint8Array) {
      return new Promise((resolve, reject) => {
        read(
          this.fd,
          bufferOrOpt,
          offset,
          length,
          position,
          (err, bytesRead, buffer) => {
            if (err) reject(err);
            else resolve({ buffer: buffer, bytesRead: bytesRead });
          },
        );
      });
    } else {
      return new Promise((resolve, reject) => {
        read(this.fd, bufferOrOpt, (err, bytesRead, buffer) => {
          if (err) reject(err);
          else resolve({ buffer: buffer, bytesRead: bytesRead });
        });
      });
    }
  }

  readableWebStream(_options?: { type?: "bytes" | undefined }) {
    notImplemented("FileHandle.readableWebStream()");
  }

  readFile(
    opt?: TextOptionsArgument | BinaryOptionsArgument | FileOptionsArgument,
  ): Promise<string | Buffer> {
    return promises.readFile(this.fd, opt);
  }

  readLines(_options?: unknown) {
    notImplemented("FileHandle.readLines()");
  }

  readv(_buffers: Buffer[], _position?: number): Promise<ReadResult> {
    notImplemented("FileHandle.readv()");
  }

  stat(options?: { bigint?: boolean }) {
    return fstatPromise(this.fd, options);
  }

  sync() {
    return fsyncPromise(this.fd);
  }

  truncate(len?: number) {
    return ftruncatePromise(this.fd, len);
  }

  utimes(atime: number, mtime: number) {
    return futimesPromise(this.fd, atime, mtime);
  }

  write(
    buffer: Buffer,
    offset: number,
    length: number,
    position: number,
  ): Promise<WriteResult>;
  write(
    str: string,
    position: number,
    encoding: string,
  ): Promise<WriteResult>;
  write(
    bufferOrStr: Uint8Array | string,
    offsetOrPosition: number,
    lengthOrEncoding: number | string,
    position?: number,
  ): Promise<WriteResult> {
    if (bufferOrStr instanceof Uint8Array) {
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
        write(
          this.fd,
          str,
          position,
          encoding,
          (err, bytesWritten, buffer) => {
            if (err) reject(err);
            else resolve({ buffer, bytesWritten });
          },
        );
      });
    }
  }

  writeFile(
    data: string | Uint8Array,
    options?: WriteFileOptions,
  ): Promise<void> {
    return promises.writeFile(this.fd, data, options);
  }

  writev(_buffers: Buffer[], _position?: number): Promise<WriteResult> {
    notImplemented("FileHandle.writev()");
  }

  [Symbol.asyncDispose] = this.close.bind(this);
}

export default {
  FileHandle,
};
