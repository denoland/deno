// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { Buffer } from "node:buffer";
import { ERR_INVALID_ARG_TYPE } from "ext:deno_node/internal/errors.ts";
import * as io from "ext:deno_io/12_io.js";
import * as fs from "ext:deno_fs/30_fs.js";
import { ReadOptions } from "ext:deno_node/_fs/_fs_common.ts";
import {
  validateOffsetLengthRead,
  validatePosition,
} from "ext:deno_node/internal/fs/utils.mjs";
import {
  validateBuffer,
  validateInteger,
} from "ext:deno_node/internal/validators.mjs";

type readSyncOptions = {
  offset: number;
  length: number;
  position: number | null;
};

type BinaryCallback = (
  err: Error | null,
  bytesRead: number | null,
  data?: Buffer | Uint8Array,
) => void;
type Callback = BinaryCallback;

export function read(fd: number, callback: Callback): void;
export function read(
  fd: number,
  options: ReadOptions,
  callback: Callback,
): void;
export function read(
  fd: number,
  buffer: Buffer | Uint8Array,
  offset: number,
  length: number,
  position: number | null,
  callback: Callback,
): void;
export function read(
  fd: number,
  optOrBufferOrCb?: Buffer | Uint8Array | ReadOptions | Callback,
  offsetOrCallback?: number | Callback,
  length?: number,
  position?: number | null,
  callback?: Callback,
) {
  let cb: Callback | undefined;
  let offset = 0,
    buffer: Buffer | Uint8Array;

  if (typeof fd !== "number") {
    throw new ERR_INVALID_ARG_TYPE("fd", "number", fd);
  }

  if (length == null) {
    length = 0;
  }

  if (typeof offsetOrCallback === "function") {
    cb = offsetOrCallback;
  } else if (typeof optOrBufferOrCb === "function") {
    cb = optOrBufferOrCb;
  } else {
    offset = offsetOrCallback as number;
    validateInteger(offset, "offset", 0);
    cb = callback;
  }

  if (
    optOrBufferOrCb instanceof Buffer || optOrBufferOrCb instanceof Uint8Array
  ) {
    buffer = optOrBufferOrCb;
  } else if (typeof optOrBufferOrCb === "function") {
    offset = 0;
    buffer = Buffer.alloc(16384);
    length = buffer.byteLength;
    position = null;
  } else {
    const opt = optOrBufferOrCb as ReadOptions;
    if (
      opt?.buffer !== undefined &&
      !(opt.buffer instanceof Buffer) && !(opt.buffer instanceof Uint8Array)
    ) {
      throw new ERR_INVALID_ARG_TYPE("buffer", [
        "Buffer",
        "TypedArray",
        "DataView",
      ], optOrBufferOrCb);
    }
    offset = opt?.offset ?? 0;
    buffer = opt?.buffer ?? Buffer.alloc(16384);
    length = opt?.length ?? (buffer.byteLength - offset);
    position = opt?.position ?? null;
  }

  if (position == null) {
    position = -1;
  }

  validatePosition(position);
  validateOffsetLengthRead(offset, length, buffer.byteLength);

  if (!cb) throw new ERR_INVALID_ARG_TYPE("cb", "Callback", cb);

  (async () => {
    try {
      const readBuf = new Uint8Array(buffer.buffer, offset, length);
      let nread: number | null;
      if (typeof position === "number" && position >= 0) {
        const currentPosition = await fs.seek(fd, 0, io.SeekMode.Current);
        // We use sync calls below to avoid being affected by others during
        // these calls.
        fs.seekSync(fd, position, io.SeekMode.Start);
        nread = io.readSync(fd, readBuf);
        fs.seekSync(fd, currentPosition, io.SeekMode.Start);
      } else {
        nread = await io.read(fd, readBuf);
      }
      cb(null, nread ?? 0, buffer);
    } catch (error) {
      cb(error as Error, null);
    }
  })();
}

export function readSync(
  fd: number,
  buffer: Buffer | Uint8Array,
  offset: number,
  length: number,
  position: number | null,
): number;
export function readSync(
  fd: number,
  buffer: Buffer | Uint8Array,
  opt: readSyncOptions,
): number;
export function readSync(
  fd: number,
  buffer: Buffer | Uint8Array,
  offsetOrOpt?: number | readSyncOptions,
  length?: number,
  position?: number | null,
): number {
  let offset = 0;

  if (typeof fd !== "number") {
    throw new ERR_INVALID_ARG_TYPE("fd", "number", fd);
  }

  validateBuffer(buffer);

  if (length == null) {
    length = 0;
  }

  if (typeof offsetOrOpt === "number") {
    offset = offsetOrOpt;
    validateInteger(offset, "offset", 0);
  } else if (offsetOrOpt !== undefined) {
    const opt = offsetOrOpt as readSyncOptions;
    offset = opt.offset ?? 0;
    length = opt.length ?? (buffer.byteLength - offset);
    position = opt.position ?? null;
  }

  if (position == null) {
    position = -1;
  }

  validatePosition(position);
  validateOffsetLengthRead(offset, length, buffer.byteLength);

  let currentPosition = 0;
  if (typeof position === "number" && position >= 0) {
    currentPosition = fs.seekSync(fd, 0, io.SeekMode.Current);
    fs.seekSync(fd, position, io.SeekMode.Start);
  }

  const readBuf = new Uint8Array(buffer.buffer, offset, length);
  const numberOfBytesRead = io.readSync(fd, readBuf);

  if (typeof position === "number" && position >= 0) {
    fs.seekSync(fd, currentPosition, io.SeekMode.Start);
  }

  return numberOfBytesRead ?? 0;
}
