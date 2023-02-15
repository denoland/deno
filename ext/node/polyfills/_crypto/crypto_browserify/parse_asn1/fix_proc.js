// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 crypto-browserify. All rights reserved. MIT license.
// from https://github.com/crypto-browserify/parse-asn1/blob/fbd70dca8670d17955893e083ca69118908570be/fixProc.js

import evp from "internal:deno_node/polyfills/_crypto/crypto_browserify/evp_bytes_to_key.ts";
import * as ciphers from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_aes/mod.js";
import { Buffer } from "internal:deno_node/polyfills/buffer.ts";

const findProc =
  /Proc-Type: 4,ENCRYPTED[\n\r]+DEK-Info: AES-((?:128)|(?:192)|(?:256))-CBC,([0-9A-H]+)[\n\r]+([0-9A-z\n\r+/=]+)[\n\r]+/m;
const startRegex = /^-----BEGIN ((?:.*? KEY)|CERTIFICATE)-----/m;
const fullRegex =
  /^-----BEGIN ((?:.*? KEY)|CERTIFICATE)-----([0-9A-z\n\r+/=]+)-----END \1-----$/m;
export default function (okey, password) {
  const key = okey.toString();
  const match = key.match(findProc);
  let decrypted;
  if (!match) {
    const match2 = key.match(fullRegex);
    decrypted = Buffer.from(match2[2].replace(/[\r\n]/g, ""), "base64");
  } else {
    const suite = "aes" + match[1];
    const iv = Buffer.from(match[2], "hex");
    const cipherText = Buffer.from(match[3].replace(/[\r\n]/g, ""), "base64");
    const cipherKey = evp(password, iv.slice(0, 8), parseInt(match[1], 10)).key;
    const out = [];
    const cipher = ciphers.createDecipheriv(suite, cipherKey, iv);
    out.push(cipher.update(cipherText));
    out.push(cipher.final());
    decrypted = Buffer.concat(out);
  }
  const tag = key.match(startRegex)[1];
  return {
    tag: tag,
    data: decrypted,
  };
}
