// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 crypto-browserify. All rights reserved. MIT license.

import { Buffer } from "internal:deno_node/polyfills/buffer.ts";
import { createHash } from "internal:deno_node/polyfills/internal/crypto/hash.ts";

// deno-lint-ignore camelcase
export function EVP_BytesToKey(
  password: string | Buffer,
  salt: string | Buffer,
  keyBits: number,
  ivLen: number,
) {
  if (!Buffer.isBuffer(password)) password = Buffer.from(password, "binary");
  if (salt) {
    if (!Buffer.isBuffer(salt)) salt = Buffer.from(salt, "binary");
    if (salt.length !== 8) {
      throw new RangeError("salt should be Buffer with 8 byte length");
    }
  }

  let keyLen = keyBits / 8;
  const key = Buffer.alloc(keyLen);
  const iv = Buffer.alloc(ivLen || 0);
  let tmp = Buffer.alloc(0);

  while (keyLen > 0 || ivLen > 0) {
    const hash = createHash("md5");
    hash.update(tmp);
    hash.update(password);
    if (salt) hash.update(salt);
    tmp = hash.digest() as Buffer;

    let used = 0;

    if (keyLen > 0) {
      const keyStart = key.length - keyLen;
      used = Math.min(keyLen, tmp.length);
      tmp.copy(key, keyStart, 0, used);
      keyLen -= used;
    }

    if (used < tmp.length && ivLen > 0) {
      const ivStart = iv.length - ivLen;
      const length = Math.min(ivLen, tmp.length - used);
      tmp.copy(iv, ivStart, used, used + length);
      ivLen -= length;
    }
  }

  tmp.fill(0);
  return { key, iv };
}

export default EVP_BytesToKey;
