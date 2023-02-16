// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2014-2017 browserify-aes contributors. All rights reserved. MIT license.
// Copyright 2013 Maxwell Krohn. All rights reserved. MIT license.
// Copyright 2009-2013 Jeff Mott. All rights reserved. MIT license.

import { Buffer } from "internal:deno_node/polyfills/buffer.ts";

function encryptByte(self, byteParam, decrypt) {
  let pad;
  let i = -1;
  const len = 8;
  let out = 0;
  let bit, value;
  while (++i < len) {
    pad = self._cipher.encryptBlock(self._prev);
    bit = (byteParam & (1 << (7 - i))) ? 0x80 : 0;
    value = pad[0] ^ bit;
    out += (value & 0x80) >> (i % 8);
    self._prev = shiftIn(self._prev, decrypt ? bit : value);
  }
  return out;
}

function shiftIn(buffer, value) {
  const len = buffer.length;
  let i = -1;
  const out = Buffer.allocUnsafe(buffer.length);
  buffer = Buffer.concat([buffer, Buffer.from([value])]);

  while (++i < len) {
    out[i] = buffer[i] << 1 | buffer[i + 1] >> (7);
  }

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
