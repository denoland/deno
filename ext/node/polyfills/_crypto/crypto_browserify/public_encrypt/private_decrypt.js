// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 Calvin Metcalf. All rights reserved. MIT license.

import parseKeys from "internal:deno_node/polyfills/_crypto/crypto_browserify/parse_asn1/mod.js";
import { createHash } from "internal:deno_node/polyfills/internal/crypto/hash.ts";
import mgf from "internal:deno_node/polyfills/_crypto/crypto_browserify/public_encrypt/mgf.js";
import { xor } from "internal:deno_node/polyfills/_crypto/crypto_browserify/public_encrypt/xor.js";
import { BN } from "internal:deno_node/polyfills/_crypto/crypto_browserify/bn.js/bn.js";
import { withPublic } from "internal:deno_node/polyfills/_crypto/crypto_browserify/public_encrypt/with_public.js";
import crt from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_rsa.js";
import { Buffer } from "internal:deno_node/polyfills/buffer.ts";

export function privateDecrypt(privateKey, enc, reverse) {
  let padding;
  if (privateKey.padding) {
    padding = privateKey.padding;
  } else if (reverse) {
    padding = 1;
  } else {
    padding = 4;
  }

  const key = parseKeys(privateKey);
  const k = key.modulus.byteLength();
  if (enc.length > k || new BN(enc).cmp(key.modulus) >= 0) {
    throw new Error("decryption error");
  }
  let msg;
  if (reverse) {
    msg = withPublic(new BN(enc), key);
  } else {
    msg = crt(enc, key);
  }
  const zBuffer = Buffer.alloc(k - msg.length);
  msg = Buffer.concat([zBuffer, msg], k);
  if (padding === 4) {
    return oaep(key, msg);
  } else if (padding === 1) {
    return pkcs1(key, msg, reverse);
  } else if (padding === 3) {
    return msg;
  } else {
    throw new Error("unknown padding");
  }
}

function oaep(key, msg) {
  const k = key.modulus.byteLength();
  const iHash = createHash("sha1").update(Buffer.alloc(0)).digest();
  const hLen = iHash.length;
  if (msg[0] !== 0) {
    throw new Error("decryption error");
  }
  const maskedSeed = msg.slice(1, hLen + 1);
  const maskedDb = msg.slice(hLen + 1);
  const seed = xor(maskedSeed, mgf(maskedDb, hLen));
  const db = xor(maskedDb, mgf(seed, k - hLen - 1));
  if (compare(iHash, db.slice(0, hLen))) {
    throw new Error("decryption error");
  }
  let i = hLen;
  while (db[i] === 0) {
    i++;
  }
  if (db[i++] !== 1) {
    throw new Error("decryption error");
  }
  return db.slice(i);
}

function pkcs1(_key, msg, reverse) {
  const p1 = msg.slice(0, 2);
  let i = 2;
  let status = 0;
  while (msg[i++] !== 0) {
    if (i >= msg.length) {
      status++;
      break;
    }
  }
  const ps = msg.slice(2, i - 1);

  if (
    (p1.toString("hex") !== "0002" && !reverse) ||
    (p1.toString("hex") !== "0001" && reverse)
  ) {
    status++;
  }
  if (ps.length < 8) {
    status++;
  }
  if (status) {
    throw new Error("decryption error");
  }
  return msg.slice(i);
}
function compare(a, b) {
  a = Buffer.from(a);
  b = Buffer.from(b);
  let dif = 0;
  let len = a.length;
  if (a.length !== b.length) {
    dif++;
    len = Math.min(a.length, b.length);
  }
  let i = -1;
  while (++i < len) {
    dif += a[i] ^ b[i];
  }
  return dif;
}
