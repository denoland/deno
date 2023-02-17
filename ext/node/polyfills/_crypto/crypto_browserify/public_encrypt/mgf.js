// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 Calvin Metcalf. All rights reserved. MIT license.

import { createHash } from "internal:deno_node/polyfills/internal/crypto/hash.ts";
import { Buffer } from "internal:deno_node/polyfills/buffer.ts";

export default function (seed, len) {
  let t = Buffer.alloc(0);
  let i = 0;
  let c;
  while (t.length < len) {
    c = i2ops(i++);
    t = Buffer.concat([t, createHash("sha1").update(seed).update(c).digest()]);
  }
  return t.slice(0, len);
}

function i2ops(c) {
  const out = Buffer.allocUnsafe(4);
  out.writeUInt32BE(c, 0);
  return out;
}
