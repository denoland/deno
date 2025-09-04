// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { Buffer } from "node:buffer";
import process from "node:process";
import { ErrnoException } from "ext:deno_node/_global.d.ts";
import { validateBufferArray } from "ext:deno_node/internal/fs/utils.mjs";
import { getValidatedFd } from "ext:deno_node/internal/fs/utils.mjs";
import { WriteVResult } from "ext:deno_node/internal/fs/handle.ts";
import { maybeCallback } from "ext:deno_node/_fs/_fs_common.ts";
import * as io from "ext:deno_io/12_io.js";
import { op_fs_seek_async, op_fs_seek_sync } from "ext:core/ops";

export interface WriteVResult {
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
export function writev(
  fd: number,
  buffers: ReadonlyArray<ArrayBufferView>,
  position?: number | null,
  callback?: writeVCallback,
): void {
  const innerWritev = async (fd, buffers, position) => {
    const chunks: Buffer[] = [];
    const offset = 0;
    for (let i = 0; i < buffers.length; i++) {
      if (Buffer.isBuffer(buffers[i])) {
        chunks.push(buffers[i]);
      } else {
        chunks.push(Buffer.from(buffers[i]));
      }
    }
    if (typeof position === "number") {
      await op_fs_seek_async(fd, position, io.SeekMode.Start);
    }
    const buffer = Buffer.concat(chunks);
    let currentOffset = 0;
    while (currentOffset < buffer.byteLength) {
      currentOffset += await io.writeSync(fd, buffer.subarray(currentOffset));
    }
    return currentOffset - offset;
  };

  fd = getValidatedFd(fd);
  validateBufferArray(buffers);
  callback = maybeCallback(callback || position);

  if (buffers.length === 0) {
    process.nextTick(callback, null, 0, buffers);
    return;
  }

  if (typeof position !== "number") position = null;

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
export function writevSync(
  fd: number,
  buffers: ArrayBufferView[],
  position?: number | null,
): number {
  const innerWritev = (fd, buffers, position) => {
    const chunks: Buffer[] = [];
    const offset = 0;
    for (let i = 0; i < buffers.length; i++) {
      if (Buffer.isBuffer(buffers[i])) {
        chunks.push(buffers[i]);
      } else {
        chunks.push(Buffer.from(buffers[i]));
      }
    }
    if (typeof position === "number") {
      op_fs_seek_sync(fd, position, io.SeekMode.Start);
    }
    const buffer = Buffer.concat(chunks);
    let currentOffset = 0;
    while (currentOffset < buffer.byteLength) {
      currentOffset += io.writeSync(fd, buffer.subarray(currentOffset));
    }
    return currentOffset - offset;
  };

  fd = getValidatedFd(fd);
  validateBufferArray(buffers);

  if (buffers.length === 0) {
    return 0;
  }

  if (typeof position !== "number") position = null;

  return innerWritev(fd, buffers, position);
}

export function writevPromise(
  fd: number,
  buffers: ArrayBufferView[],
  position?: number,
): Promise<WriteVResult> {
  return new Promise((resolve, reject) => {
    writev(fd, buffers, position, (err, bytesWritten, buffers) => {
      if (err) reject(err);
      else resolve({ bytesWritten, buffers });
    });
  });
}
