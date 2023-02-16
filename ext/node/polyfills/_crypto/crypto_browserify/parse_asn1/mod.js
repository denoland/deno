// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 crypto-browserify. All rights reserved. MIT license.
// from https://github.com/crypto-browserify/parse-asn1/blob/fbd70dca8670d17955893e083ca69118908570be/index.js

import * as asn1 from "internal:deno_node/polyfills/_crypto/crypto_browserify/parse_asn1/asn1.js";
import fixProc from "internal:deno_node/polyfills/_crypto/crypto_browserify/parse_asn1/fix_proc.js";
import * as ciphers from "internal:deno_node/polyfills/_crypto/crypto_browserify/browserify_aes/mod.js";
import { Buffer } from "internal:deno_node/polyfills/buffer.ts";
import { pbkdf2Sync } from "internal:deno_node/polyfills/internal/crypto/pbkdf2.ts";

const aesid = {
  "2.16.840.1.101.3.4.1.1": "aes-128-ecb",
  "2.16.840.1.101.3.4.1.2": "aes-128-cbc",
  "2.16.840.1.101.3.4.1.3": "aes-128-ofb",
  "2.16.840.1.101.3.4.1.4": "aes-128-cfb",
  "2.16.840.1.101.3.4.1.21": "aes-192-ecb",
  "2.16.840.1.101.3.4.1.22": "aes-192-cbc",
  "2.16.840.1.101.3.4.1.23": "aes-192-ofb",
  "2.16.840.1.101.3.4.1.24": "aes-192-cfb",
  "2.16.840.1.101.3.4.1.41": "aes-256-ecb",
  "2.16.840.1.101.3.4.1.42": "aes-256-cbc",
  "2.16.840.1.101.3.4.1.43": "aes-256-ofb",
  "2.16.840.1.101.3.4.1.44": "aes-256-cfb",
};
export function parseKeys(buffer) {
  let password;
  if (typeof buffer === "object" && !Buffer.isBuffer(buffer)) {
    password = buffer.passphrase;
    buffer = buffer.key;
  }
  if (typeof buffer === "string") {
    buffer = Buffer.from(buffer);
  }

  const stripped = fixProc(buffer, password);

  const type = stripped.tag;
  let data = stripped.data;
  let subtype, ndata;
  switch (type) {
    case "CERTIFICATE":
      ndata = asn1.certificate.decode(data, "der").tbsCertificate
        .subjectPublicKeyInfo;
      // falls through
    case "PUBLIC KEY":
      if (!ndata) {
        ndata = asn1.PublicKey.decode(data, "der");
      }
      subtype = ndata.algorithm.algorithm.join(".");
      switch (subtype) {
        case "1.2.840.113549.1.1.1":
          return asn1.RSAPublicKey.decode(ndata.subjectPublicKey.data, "der");
        case "1.2.840.10045.2.1":
          ndata.subjectPrivateKey = ndata.subjectPublicKey;
          return {
            type: "ec",
            data: ndata,
          };
        case "1.2.840.10040.4.1":
          ndata.algorithm.params.pub_key = asn1.DSAparam.decode(
            ndata.subjectPublicKey.data,
            "der",
          );
          return {
            type: "dsa",
            data: ndata.algorithm.params,
          };
        default:
          throw new Error("unknown key id " + subtype);
      }
      // throw new Error('unknown key type ' + type)
    case "ENCRYPTED PRIVATE KEY":
      data = asn1.EncryptedPrivateKey.decode(data, "der");
      data = decrypt(data, password);
      // falls through
    case "PRIVATE KEY":
      ndata = asn1.PrivateKey.decode(data, "der");
      subtype = ndata.algorithm.algorithm.join(".");
      switch (subtype) {
        case "1.2.840.113549.1.1.1":
          return asn1.RSAPrivateKey.decode(ndata.subjectPrivateKey, "der");
        case "1.2.840.10045.2.1":
          return {
            curve: ndata.algorithm.curve,
            privateKey: asn1.ECPrivateKey.decode(ndata.subjectPrivateKey, "der")
              .privateKey,
          };
        case "1.2.840.10040.4.1":
          ndata.algorithm.params.priv_key = asn1.DSAparam.decode(
            ndata.subjectPrivateKey,
            "der",
          );
          return {
            type: "dsa",
            params: ndata.algorithm.params,
          };
        default:
          throw new Error("unknown key id " + subtype);
      }
      // throw new Error('unknown key type ' + type)
    case "RSA PUBLIC KEY":
      return asn1.RSAPublicKey.decode(data, "der");
    case "RSA PRIVATE KEY":
      return asn1.RSAPrivateKey.decode(data, "der");
    case "DSA PRIVATE KEY":
      return {
        type: "dsa",
        params: asn1.DSAPrivateKey.decode(data, "der"),
      };
    case "EC PRIVATE KEY":
      data = asn1.ECPrivateKey.decode(data, "der");
      return {
        curve: data.parameters.value,
        privateKey: data.privateKey,
      };
    default:
      throw new Error("unknown key type " + type);
  }
}
export default parseKeys;
parseKeys.signature = asn1.signature;
function decrypt(data, password) {
  const salt = data.algorithm.decrypt.kde.kdeparams.salt;
  const iters = parseInt(
    data.algorithm.decrypt.kde.kdeparams.iters.toString(),
    10,
  );
  const algo = aesid[data.algorithm.decrypt.cipher.algo.join(".")];
  const iv = data.algorithm.decrypt.cipher.iv;
  const cipherText = data.subjectPrivateKey;
  const keylen = parseInt(algo.split("-")[1], 10) / 8;
  const key = pbkdf2Sync(password, salt, iters, keylen, "sha1");
  const cipher = ciphers.createDecipheriv(algo, key, iv);
  const out = [];
  out.push(cipher.update(cipherText));
  out.push(cipher.final());
  return Buffer.concat(out);
}
