// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 Calvin Metcalf. All rights reserved. MIT license.

import parseKeys from "internal:deno_node/polyfills/_crypto/crypto_browserify/parse_asn1/mod.js";
import { randomBytes } from "internal:deno_node/polyfills/_crypto/crypto_browserify/randombytes.ts";
import { createHash } from "internal:deno_node/polyfills/internal/crypto/hash.ts";
import mgf from "internal:deno_node/polyfills/_crypto/crypto_browserify/public_encrypt/mgf.js";
import { xor } from "internal:deno_node/polyfills/_crypto/crypto_browserify/public_encrypt/xor.js";
import { BN } from "internal:deno_node/polyfills/_crypto/crypto_browserify/bn.js/bn.js";
import { withPublic } from "internal:deno_node/polyfills/_crypto/crypto_browserify/public_encrypt/with_public.js";
import crt from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_rsa.js";
import { Buffer } from "internal:deno_node/polyfills/buffer.ts";

export function publicEncrypt(publicKey, msg, reverse) {
  let padding;
  if (publicKey.padding) {
    padding = publicKey.padding;
  } else if (reverse) {
    padding = 1;
  } else {
    padding = 4;
  }
  const key = parseKeys(publicKey);
  let paddedMsg;
  if (padding === 4) {
    paddedMsg = oaep(key, msg);
  } else if (padding === 1) {
    paddedMsg = pkcs1(key, msg, reverse);
  } else if (padding === 3) {
    paddedMsg = new BN(msg);
    if (paddedMsg.cmp(key.modulus) >= 0) {
      throw new Error("data too long for modulus");
    }
  } else {
    throw new Error("unknown padding");
  }
  if (reverse) {
    return crt(paddedMsg, key);
  } else {
    return withPublic(paddedMsg, key);
  }
}

function oaep(key, msg) {
  const k = key.modulus.byteLength();
  const mLen = msg.length;
  const iHash = createHash("sha1").update(Buffer.alloc(0)).digest();
  const hLen = iHash.length;
  const hLen2 = 2 * hLen;
  if (mLen > k - hLen2 - 2) {
    throw new Error("message too long");
  }
  const ps = Buffer.alloc(k - mLen - hLen2 - 2);
  const dblen = k - hLen - 1;
  const seed = randomBytes(hLen);
  const maskedDb = xor(
    Buffer.concat([iHash, ps, Buffer.alloc(1, 1), msg], dblen),
    mgf(seed, dblen),
  );
  const maskedSeed = xor(seed, mgf(maskedDb, hLen));
  return new BN(Buffer.concat([Buffer.alloc(1), maskedSeed, maskedDb], k));
}
function pkcs1(key, msg, reverse) {
  const mLen = msg.length;
  const k = key.modulus.byteLength();
  if (mLen > k - 11) {
    throw new Error("message too long");
  }
  let ps;
  if (reverse) {
    ps = Buffer.alloc(k - mLen - 3, 0xff);
  } else {
    ps = nonZero(k - mLen - 3);
  }
  return new BN(
    Buffer.concat([
      Buffer.from([
        0,
        reverse ? 1 : 2,
      ]),
      ps,
      Buffer.alloc(1),
      msg,
    ], k),
  );
}
function nonZero(len) {
  const out = Buffer.allocUnsafe(len);
  let i = 0;
  let cache = randomBytes(len * 2);
  let cur = 0;
  let num;
  while (i < len) {
    if (cur === cache.length) {
      cache = randomBytes(len * 2);
      cur = 0;
    }
    num = cache[cur++];
    if (num) {
      out[i++] = num;
    }
  }
  return out;
}
