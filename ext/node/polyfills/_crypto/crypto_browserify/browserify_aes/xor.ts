// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2014-2017 browserify-aes contributors. All rights reserved. MIT license.
// Copyright 2013 Maxwell Krohn. All rights reserved. MIT license.
// Copyright 2009-2013 Jeff Mott. All rights reserved. MIT license.

import { Buffer } from "internal:deno_node/polyfills/buffer.ts";

export function xor(a: Buffer, b: Buffer): Buffer {
  const length = Math.min(a.length, b.length);
  const buffer = Buffer.allocUnsafe(length);

  for (let i = 0; i < length; ++i) {
    buffer[i] = a[i] ^ b[i];
  }

  return buffer;
}
