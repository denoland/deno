// Regression test for the `encoding` option on `crypto.publicEncrypt` /
// `privateDecrypt` / `privateEncrypt` / `publicDecrypt`. When the key is
// passed as `{ key, encoding }`, Node applies that encoding to the buffer
// argument too. Prior to this fix, Deno applied the encoding only to the
// key string, so a hex-encoded plaintext was treated as UTF-8 bytes (the
// hex string itself), which round-tripped to the hex form rather than the
// original payload.
//
// Mirrors the shape exercised by Node's `parallel/test-crypto-rsa-dsa.js`.

import { strictEqual } from "node:assert";
import { Buffer } from "node:buffer";
import {
  generateKeyPairSync,
  privateDecrypt,
  privateEncrypt,
  publicDecrypt,
  publicEncrypt,
} from "node:crypto";

const { publicKey: publicPem, privateKey: privatePem } = generateKeyPairSync(
  "rsa",
  {
    modulusLength: 2048,
    publicKeyEncoding: { type: "spki", format: "pem" },
    privateKeyEncoding: { type: "pkcs8", format: "pem" },
  },
);
const publicPemBytes = Buffer.from(publicPem);
const privatePemBytes = Buffer.from(privatePem);
const plaintext = "I AM THE WALRUS";

// 1. Hex-encoded buffer; key as bytes so the `encoding` only affects the
// buffer argument.
{
  const hex = Buffer.from(plaintext).toString("hex");
  const ct = publicEncrypt({ key: publicPemBytes, encoding: "hex" }, hex);
  const pt = privateDecrypt(privatePem, ct);
  strictEqual(pt.toString(), plaintext);
}

// 2. Base64-encoded buffer.
{
  const b64 = Buffer.from(plaintext).toString("base64");
  const ct = publicEncrypt({ key: publicPemBytes, encoding: "base64" }, b64);
  const pt = privateDecrypt(privatePem, ct);
  strictEqual(pt.toString(), plaintext);
}

// 3. Hex on both the key string and the buffer (matches the upstream
// `test-crypto-rsa-dsa.js` snippet that uncovered the bug).
{
  const keyHex = publicPemBytes.toString("hex");
  const dataHex = Buffer.from(plaintext).toString("hex");
  const ct = publicEncrypt({ key: keyHex, encoding: "hex" }, dataHex);
  const pt = privateDecrypt(privatePem, ct);
  strictEqual(pt.toString(), plaintext);
}

// 4. No encoding option -> buffer string is utf8 (the historical default).
{
  const ct = publicEncrypt(publicPem, plaintext);
  const pt = privateDecrypt(privatePem, ct);
  strictEqual(pt.toString(), plaintext);
}

// 5. Private-side encrypt / public-side decrypt round trip.
{
  const hex = Buffer.from(plaintext).toString("hex");
  const ct = privateEncrypt({ key: privatePemBytes, encoding: "hex" }, hex);
  const pt = publicDecrypt({ key: publicPemBytes, encoding: "hex" }, ct);
  strictEqual(pt.toString(), plaintext);
}

console.log("ok");
