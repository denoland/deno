// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 Calvin Metcalf. All rights reserved. MIT license.

import { publicEncrypt } from "internal:deno_node/polyfills/_crypto/crypto_browserify/public_encrypt/public_encrypt.js";
import { privateDecrypt } from "internal:deno_node/polyfills/_crypto/crypto_browserify/public_encrypt/private_decrypt.js";

export { privateDecrypt, publicEncrypt };

export function privateEncrypt(key, buf) {
  return publicEncrypt(key, buf, true);
}

export function publicDecrypt(key, buf) {
  return privateDecrypt(key, buf, true);
}
