// Copyright 2018-2025 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { Buffer } from "node:buffer";
import { ERR_INVALID_ARG_VALUE } from "ext:deno_node/internal/errors.ts";
import * as io from "ext:deno_io/12_io.js";
import {
  arrayBufferViewToUint8Array,
  getValidatedFd,
  validateOffsetLengthRead,
  validatePosition,
} from "ext:deno_node/internal/fs/utils.mjs";
import {
  validateBuffer,
  validateFunction,
  validateInteger,
  validateObject,
} from "ext:deno_node/internal/validators.mjs";
import { isArrayBufferView } from "ext:deno_node/internal/util/types.ts";
import { op_fs_seek_async, op_fs_seek_sync } from "ext:core/ops";
import { primordials } from "ext:core/mod.js";
import {
  customPromisifyArgs,
  kEmptyObject,
} from "ext:deno_node/internal/util.mjs";
import * as process from "node:process";
import type { ReadAsyncOptions, ReadSyncOptions } from "node:fs";

const { ObjectDefineProperty } = primordials;

const validateOptionArgs = { __proto__: null, nullable: true };

type BinaryCallback = (
  err: Error | null,
  bytesRead: number | null,
  data?: ArrayBufferView,
) => void;
type Callback = BinaryCallback;

export function read(fd: number, callback: Callback): void;
export function read(
  fd: number,
  options: ReadAsyncOptions<NodeJS.ArrayBufferView>,
  callback: Callback,
): void;
export function read(
  fd: number,
  buffer: ArrayBufferView,
  options: ReadSyncOptions,
  callback: Callback,
): void;
export function read(
  fd: number,
  buffer: ArrayBufferView,
  offset: number,
  length: number,
  position: number | null,
  callback: Callback,
): void;
export function read(
  fd: number,
  optOrBufferOrCb?:
    | ArrayBufferView
    | ReadAsyncOptions<NodeJS.ArrayBufferView>
    | Callback,
  offsetOrOpt?:
    | number
    | ReadAsyncOptions<NodeJS.ArrayBufferView>
    | Callback,
  lengthOrCb?: number | Callback,
  position?: number | null,
  callback?: Callback,
) {
  fd = getValidatedFd(fd);

  let offset = offsetOrOpt;
  let buffer = optOrBufferOrCb;
  let length = lengthOrCb;
  let params = null;
  if (arguments.length <= 4) {
    if (arguments.length === 4) {
      // This is fs.read(fd, buffer, options, callback)
      validateObject(offsetOrOpt, "options", validateOptionArgs);
      callback = length as Callback;
      params = offsetOrOpt;
    } else if (arguments.length === 3) {
      // This is fs.read(fd, bufferOrParams, callback)
      if (!isArrayBufferView(buffer)) {
        // This is fs.read(fd, params, callback)
        params = buffer;
        ({ buffer = Buffer.alloc(16384) } = params ?? kEmptyObject);
      }
      callback = offsetOrOpt as Callback;
    } else {
      // This is fs.read(fd, callback)
      callback = buffer as Callback;
      buffer = Buffer.alloc(16384);
    }

    if (params !== undefined) {
      validateObject(params, "options", validateOptionArgs);
    }
    ({
      offset = 0,
      length = buffer?.byteLength - (offset as number),
      position = null,
    } = params ?? kEmptyObject);
  }

  validateBuffer(buffer);
  validateFunction(callback, "cb");

  if (offset == null) {
    offset = 0;
  } else {
    validateInteger(offset, "offset", 0);
  }

  (length as number) |= 0;

  if (length === 0) {
    return process.nextTick(function tick() {
      callback!(null, 0, buffer);
    });
  }

  if (buffer.byteLength === 0) {
    throw new ERR_INVALID_ARG_VALUE(
      "buffer",
      buffer,
      "is empty and cannot be written",
    );
  }

  validateOffsetLengthRead(offset, length, buffer.byteLength);

  if (position == null) {
    position = -1;
  } else {
    validatePosition(position, "position", length as number);
  }

  (async () => {
    try {
      let nread: number | null;
      if (typeof position === "number" && position >= 0) {
        const currentPosition = await op_fs_seek_async(
          fd,
          0,
          io.SeekMode.Current,
        );
        // We use sync calls below to avoid being affected by others during
        // these calls.
        op_fs_seek_sync(fd, position, io.SeekMode.Start);
        nread = io.readSync(
          fd,
          arrayBufferViewToUint8Array(buffer).subarray(
            offset,
            offset + (length as number),
          ),
        );
        op_fs_seek_sync(fd, currentPosition, io.SeekMode.Start);
      } else {
        nread = await io.read(
          fd,
          arrayBufferViewToUint8Array(buffer).subarray(
            offset,
            offset + (length as number),
          ),
        );
      }
      callback!(null, nread ?? 0, buffer);
    } catch (error) {
      callback!(error as Error, null);
    }
  })();
}

ObjectDefineProperty(read, customPromisifyArgs, {
  __proto__: null,
  value: ["bytesRead", "buffer"],
  enumerable: false,
});

export function readSync(
  fd: number,
  buffer: ArrayBufferView,
  offset: number,
  length: number,
  position: number | null,
): number;
export function readSync(
  fd: number,
  buffer: ArrayBufferView,
  opt: ReadSyncOptions,
): number;
export function readSync(
  fd: number,
  buffer: ArrayBufferView,
  offsetOrOpt?: number | ReadSyncOptions,
  length?: number,
  position?: number | null,
): number {
  fd = getValidatedFd(fd);

  validateBuffer(buffer);

  let offset = offsetOrOpt;
  if (arguments.length <= 3 || typeof offsetOrOpt === "object") {
    if (offsetOrOpt !== undefined) {
      validateObject(offsetOrOpt, "options", validateOptionArgs);
    }

    ({
      offset = 0,
      length = buffer.byteLength - (offset as number),
      position = null,
    } = offsetOrOpt ?? kEmptyObject);
  }

  if (offset === undefined) {
    offset = 0;
  } else {
    validateInteger(offset, "offset", 0);
  }

  length! |= 0;

  if (length === 0) {
    return 0;
  }

  if (buffer.byteLength === 0) {
    throw new ERR_INVALID_ARG_VALUE(
      "buffer",
      buffer,
      "is empty and cannot be written",
    );
  }

  validateOffsetLengthRead(offset, length, buffer.byteLength);

  if (position == null) {
    position = -1;
  } else {
    validatePosition(position, "position", length);
  }

  let currentPosition = 0;
  if (typeof position === "number" && position >= 0) {
    currentPosition = op_fs_seek_sync(fd, 0, io.SeekMode.Current);
    op_fs_seek_sync(fd, position, io.SeekMode.Start);
  }

  const numberOfBytesRead = io.readSync(
    fd,
    arrayBufferViewToUint8Array(buffer).subarray(offset, offset + length!),
  );

  if (typeof position === "number" && position >= 0) {
    op_fs_seek_sync(fd, currentPosition, io.SeekMode.Start);
  }

  return numberOfBytesRead ?? 0;
}
