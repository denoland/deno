// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 crypto-browserify. All rights reserved. MIT license.

import { BN } from "internal:deno_node/polyfills/_crypto/crypto_browserify/bn.js/bn.js";
import { randomBytes } from "internal:deno_node/polyfills/_crypto/crypto_browserify/randombytes.ts";
import { Buffer } from "internal:deno_node/polyfills/buffer.ts";

function blind(priv) {
  const r = getr(priv);
  const blinder = r.toRed(BN.mont(priv.modulus)).redPow(
    new BN(priv.publicExponent),
  ).fromRed();
  return { blinder: blinder, unblinder: r.invm(priv.modulus) };
}

function getr(priv) {
  const len = priv.modulus.byteLength();
  let r;
  do {
    r = new BN(randomBytes(len));
  } while (
    r.cmp(priv.modulus) >= 0 || !r.umod(priv.prime1) || !r.umod(priv.prime2)
  );
  return r;
}

function crt(msg, priv) {
  const blinds = blind(priv);
  const len = priv.modulus.byteLength();
  const blinded = new BN(msg).mul(blinds.blinder).umod(priv.modulus);
  const c1 = blinded.toRed(BN.mont(priv.prime1));
  const c2 = blinded.toRed(BN.mont(priv.prime2));
  const qinv = priv.coefficient;
  const p = priv.prime1;
  const q = priv.prime2;
  const m1 = c1.redPow(priv.exponent1).fromRed();
  const m2 = c2.redPow(priv.exponent2).fromRed();
  const h = m1.isub(m2).imul(qinv).umod(p).imul(q);
  return m2.iadd(h).imul(blinds.unblinder).umod(priv.modulus).toArrayLike(
    Buffer,
    "be",
    len,
  );
}
crt.getr = getr;

export default crt;
