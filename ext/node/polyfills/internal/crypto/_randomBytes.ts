// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { core } from "ext:core/mod.js";
import { Buffer, kMaxLength } from "node:buffer";
import {
  emitAfter,
  emitBefore,
  emitDestroy,
  emitInit,
  executionAsyncId,
  newAsyncId,
} from "ext:deno_node/internal/async_hooks.ts";
const {
  validateFunction,
  validateNumber,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
import { ERR_OUT_OF_RANGE } from "ext:deno_node/internal/errors.ts";
import process from "node:process";

export const MAX_RANDOM_VALUES = 65536;
const kMaxInt32 = 2 ** 31 - 1;
const kMaxPossibleLength = Math.min(kMaxLength, kMaxInt32);
export const MAX_SIZE = kMaxPossibleLength;

// Mirrors Node's lib/internal/crypto/random.js assertSize() with
// elementSize = 1, offset = 0, length = Infinity.
function assertSize(size: number): number {
  validateNumber(size, "size");

  if (Number.isNaN(size) || size > kMaxPossibleLength || size < 0) {
    throw new ERR_OUT_OF_RANGE(
      "size",
      `>= 0 && <= ${kMaxPossibleLength}`,
      size,
    );
  }

  return size >>> 0;
}

function generateRandomBytes(size: number) {
  const bytes = Buffer.allocUnsafeSlow(size);

  //Work around for getRandomValues max generation
  if (size > MAX_RANDOM_VALUES) {
    for (let generated = 0; generated < size; generated += MAX_RANDOM_VALUES) {
      globalThis.crypto.getRandomValues(
        bytes.slice(generated, generated + MAX_RANDOM_VALUES),
      );
    }
  } else {
    globalThis.crypto.getRandomValues(bytes);
  }

  return bytes;
}

/**
 * @param size Buffer length, must be equal or greater than zero
 */
export default function randomBytes(size: number): Buffer;
export default function randomBytes(
  size: number,
  cb?: (err: Error | null, buf?: Buffer) => void,
): void;
export default function randomBytes(
  size: number,
  cb?: (err: Error | null, buf?: Buffer) => void,
): Buffer | void {
  size = assertSize(size);
  if (cb !== undefined) {
    validateFunction(cb, "callback");
  }
  if (typeof cb === "function") {
    let err: Error | null = null, bytes: Buffer;
    try {
      bytes = generateRandomBytes(size);
    } catch (e) {
      if (e instanceof Error) {
        err = e;
      } else {
        err = new Error("[non-error thrown]");
      }
    }

    // Set up async_hooks tracking
    const asyncId = newAsyncId();
    const triggerAsyncId = executionAsyncId();
    const resource = {};
    emitInit(asyncId, "RANDOMBYTESREQUEST", triggerAsyncId, resource);

    process.nextTick(() => {
      emitBefore(asyncId);
      try {
        if (err) {
          cb(err);
        } else {
          cb(null, bytes);
        }
      } catch (callbackErr) {
        // If there's an active domain, emit error to it
        if (process.domain && process.domain.listenerCount("error") > 0) {
          process.domain.emit("error", callbackErr);
        } else {
          throw callbackErr;
        }
      } finally {
        emitAfter(asyncId);
        emitDestroy(asyncId);
      }
    });
  } else {
    return generateRandomBytes(size);
  }
}
