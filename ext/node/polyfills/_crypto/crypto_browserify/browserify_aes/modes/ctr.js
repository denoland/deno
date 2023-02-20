// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2014-2017 browserify-aes contributors. All rights reserved. MIT license.
// Copyright 2013 Maxwell Krohn. All rights reserved. MIT license.
// Copyright 2009-2013 Jeff Mott. All rights reserved. MIT license.

import { xor } from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_aes/xor.ts";
import { Buffer } from "internal:deno_node/polyfills/buffer.ts";
import { incr32 } from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_aes/incr32.js";

function getBlock(self) {
  const out = self._cipher.encryptBlockRaw(self._prev);
  incr32(self._prev);
  return out;
}

const blockSize = 16;
export const encrypt = function (self, chunk) {
  const chunkNum = Math.ceil(chunk.length / blockSize);
  const start = self._cache.length;
  self._cache = Buffer.concat([
    self._cache,
    Buffer.allocUnsafe(chunkNum * blockSize),
  ]);
  for (let i = 0; i < chunkNum; i++) {
    const out = getBlock(self);
    const offset = start + i * blockSize;
    self._cache.writeUInt32BE(out[0], offset + 0);
    self._cache.writeUInt32BE(out[1], offset + 4);
    self._cache.writeUInt32BE(out[2], offset + 8);
    self._cache.writeUInt32BE(out[3], offset + 12);
  }
  const pad = self._cache.slice(0, chunk.length);
  self._cache = self._cache.slice(chunk.length);
  return xor(chunk, pad);
};
