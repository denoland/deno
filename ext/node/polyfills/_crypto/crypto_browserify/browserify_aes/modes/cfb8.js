// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2014-2017 browserify-aes contributors. All rights reserved. MIT license.
// Copyright 2013 Maxwell Krohn. All rights reserved. MIT license.
// Copyright 2009-2013 Jeff Mott. All rights reserved. MIT license.

import { Buffer } from "internal:deno_node/polyfills/buffer.ts";

function encryptByte(self, byteParam, decrypt) {
  const pad = self._cipher.encryptBlock(self._prev);
  const out = pad[0] ^ byteParam;

  self._prev = Buffer.concat([
    self._prev.slice(1),
    Buffer.from([decrypt ? byteParam : out]),
  ]);

  return out;
}

export const encrypt = function (self, chunk, decrypt) {
  const len = chunk.length;
  const out = Buffer.allocUnsafe(len);
  let i = -1;

  while (++i < len) {
    out[i] = encryptByte(self, chunk[i], decrypt);
  }

  return out;
};
