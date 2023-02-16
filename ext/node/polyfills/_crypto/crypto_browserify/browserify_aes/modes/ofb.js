// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2014-2017 browserify-aes contributors. All rights reserved. MIT license.
// Copyright 2013 Maxwell Krohn. All rights reserved. MIT license.
// Copyright 2009-2013 Jeff Mott. All rights reserved. MIT license.

import { xor } from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_aes/xor.ts";
import { Buffer } from "internal:deno_node/polyfills/buffer.ts";

function getBlock(self) {
  self._prev = self._cipher.encryptBlock(self._prev);
  return self._prev;
}

export const encrypt = function (self, chunk) {
  while (self._cache.length < chunk.length) {
    self._cache = Buffer.concat([self._cache, getBlock(self)]);
  }

  const pad = self._cache.slice(0, chunk.length);
  self._cache = self._cache.slice(chunk.length);
  return xor(chunk, pad);
};
