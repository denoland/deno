// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 crypto-browserify. All rights reserved. MIT license.
import { Buffer } from "internal:deno_node/polyfills/buffer.ts";
import { nextTick } from "internal:deno_node/polyfills/_next_tick.ts";

// limit of Crypto.getRandomValues()
// https://developer.mozilla.org/en-US/docs/Web/API/Crypto/getRandomValues
const MAX_BYTES = 65536;

// Node supports requesting up to this number of bytes
// https://github.com/nodejs/node/blob/master/lib/internal/crypto/random.js#L48
const MAX_UINT32 = 4294967295;

export function randomBytes(
  size: number,
  cb?: (err: Error | null, b: Buffer) => void,
) {
  // phantomjs needs to throw
  if (size > MAX_UINT32) {
    throw new RangeError("requested too many random bytes");
  }

  const bytes = Buffer.allocUnsafe(size);

  if (size > 0) { // getRandomValues fails on IE if size == 0
    if (size > MAX_BYTES) { // this is the max bytes crypto.getRandomValues
      // can do at once see https://developer.mozilla.org/en-US/docs/Web/API/window.crypto.getRandomValues
      for (let generated = 0; generated < size; generated += MAX_BYTES) {
        // buffer.slice automatically checks if the end is past the end of
        // the buffer so we don't have to here
        globalThis.crypto.getRandomValues(
          bytes.slice(generated, generated + MAX_BYTES),
        );
      }
    } else {
      globalThis.crypto.getRandomValues(bytes);
    }
  }

  if (typeof cb === "function") {
    return nextTick(function () {
      cb(null, bytes);
    });
  }

  return bytes;
}
