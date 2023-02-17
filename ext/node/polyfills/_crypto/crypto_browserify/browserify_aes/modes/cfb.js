// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2014-2017 browserify-aes contributors. All rights reserved. MIT license.
// Copyright 2013 Maxwell Krohn. All rights reserved. MIT license.
// Copyright 2009-2013 Jeff Mott. All rights reserved. MIT license.

import { xor } from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_aes/xor.ts";
import { Buffer } from "internal:deno_node/polyfills/buffer.ts";

function encryptStart(self, data, decrypt) {
  const len = data.length;
  const out = xor(data, self._cache);
  self._cache = self._cache.slice(len);
  self._prev = Buffer.concat([self._prev, decrypt ? data : out]);
  return out;
}

export const encrypt = function (self, data, decrypt) {
  let out = Buffer.allocUnsafe(0);
  let len;

  while (data.length) {
    if (self._cache.length === 0) {
      self._cache = self._cipher.encryptBlock(self._prev);
      self._prev = Buffer.allocUnsafe(0);
    }

    if (self._cache.length <= data.length) {
      len = self._cache.length;
      out = Buffer.concat([
        out,
        encryptStart(self, data.slice(0, len), decrypt),
      ]);
      data = data.slice(len);
    } else {
      out = Buffer.concat([out, encryptStart(self, data, decrypt)]);
      break;
    }
  }

  return out;
};
