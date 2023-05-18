// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { Buffer } from "ext:deno_node/buffer.ts";
import { ERR_INVALID_ARG_TYPE } from "ext:deno_node/internal/errors.ts";
import {
  validateOffsetLengthRead,
  validatePosition,
} from "ext:deno_node/internal/fs/utils.mjs";
import {
  validateBuffer,
  validateInteger,
} from "ext:deno_node/internal/validators.mjs";

type readOptions = {
  buffer: Buffer | Uint8Array;
  offset: number;
  length: number;
  position: number | null;
};

type readSyncOptions = {
  offset: number;
  length: number;
  position: number | null;
};

type BinaryCallback = (
  err: Error | null,
  bytesRead: number | null,
  data?: Buffer,
) => void;
type Callback = BinaryCallback;

export function read(fd: number, callback: Callback): void;
export function read(
  fd: number,
  options: readOptions,
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
  optOrBufferOrCb?: Buffer | Uint8Array | readOptions | Callback,
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
    const opt = optOrBufferOrCb as readOptions;
    if (
      !(opt.buffer instanceof Buffer) && !(opt.buffer instanceof Uint8Array)
    ) {
      if (opt.buffer === null) {
        // @ts-ignore: Intentionally create TypeError for passing test-fs-read.js#L87
        length = opt.buffer.byteLength;
      }
      throw new ERR_INVALID_ARG_TYPE("buffer", [
        "Buffer",
        "TypedArray",
        "DataView",
      ], optOrBufferOrCb);
    }
    offset = opt.offset ?? 0;
    buffer = opt.buffer ?? Buffer.alloc(16384);
    length = opt.length ?? buffer.byteLength;
    position = opt.position ?? null;
  }

  if (position == null) {
    position = -1;
  }

  validatePosition(position);
  validateOffsetLengthRead(offset, length, buffer.byteLength);

  if (!cb) throw new ERR_INVALID_ARG_TYPE("cb", "Callback", cb);

  (async () => {
    try {
      let nread: number | null;
      if (typeof position === "number" && position >= 0) {
        const currentPosition = await Deno.seek(fd, 0, Deno.SeekMode.Current);
        // We use sync calls below to avoid being affected by others during
        // these calls.
        Deno.seekSync(fd, position, Deno.SeekMode.Start);
        nread = Deno.readSync(fd, buffer);
        Deno.seekSync(fd, currentPosition, Deno.SeekMode.Start);
      } else {
        nread = await Deno.read(fd, buffer);
      }
      cb(null, nread ?? 0, Buffer.from(buffer.buffer, offset, length));
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
  } else {
    const opt = offsetOrOpt as readSyncOptions;
    offset = opt.offset ?? 0;
    length = opt.length ?? buffer.byteLength;
    position = opt.position ?? null;
  }

  if (position == null) {
    position = -1;
  }

  validatePosition(position);
  validateOffsetLengthRead(offset, length, buffer.byteLength);

  let currentPosition = 0;
  if (typeof position === "number" && position >= 0) {
    currentPosition = Deno.seekSync(fd, 0, Deno.SeekMode.Current);
    Deno.seekSync(fd, position, Deno.SeekMode.Start);
  }

  const numberOfBytesRead = Deno.readSync(fd, buffer);

  if (typeof position === "number" && position >= 0) {
    Deno.seekSync(fd, currentPosition, Deno.SeekMode.Start);
  }

  return numberOfBytesRead ?? 0;
}
