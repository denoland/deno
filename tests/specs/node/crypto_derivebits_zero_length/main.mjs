// Regression test for w3c/webcrypto#380 ("Make 0 a valid `length`
// parameter for deriveBits"). Both HKDF and PBKDF2 must return an
// empty ArrayBuffer rather than throwing `OperationError: Invalid
// length`.

import { strictEqual } from "node:assert";

const { subtle } = globalThis.crypto;

// HKDF: length === 0 should yield an empty ArrayBuffer.
{
  const key = await subtle.importKey(
    "raw",
    new Uint8Array(0),
    "HKDF",
    false,
    ["deriveBits"],
  );
  const bits = await subtle.deriveBits(
    {
      name: "HKDF",
      hash: { name: "SHA-256" },
      info: new Uint8Array(0),
      salt: new Uint8Array(0),
    },
    key,
    0,
  );
  strictEqual(bits instanceof ArrayBuffer, true);
  strictEqual(bits.byteLength, 0);
}

// PBKDF2: same.
{
  const key = await subtle.importKey(
    "raw",
    new TextEncoder().encode("passphrase"),
    "PBKDF2",
    false,
    ["deriveBits"],
  );
  const bits = await subtle.deriveBits(
    {
      name: "PBKDF2",
      hash: "SHA-256",
      salt: new TextEncoder().encode("salt"),
      iterations: 1,
    },
    key,
    0,
  );
  strictEqual(bits instanceof ArrayBuffer, true);
  strictEqual(bits.byteLength, 0);
}

// Sanity: non-zero lengths still work.
{
  const key = await subtle.importKey(
    "raw",
    new Uint8Array(0),
    "HKDF",
    false,
    ["deriveBits"],
  );
  const bits = await subtle.deriveBits(
    {
      name: "HKDF",
      hash: "SHA-256",
      info: new Uint8Array(0),
      salt: new Uint8Array(0),
    },
    key,
    256,
  );
  strictEqual(bits.byteLength, 32);
}

// Sanity: non-byte-aligned length still throws.
{
  const key = await subtle.importKey(
    "raw",
    new Uint8Array(0),
    "HKDF",
    false,
    ["deriveBits"],
  );
  let threw = false;
  try {
    await subtle.deriveBits(
      {
        name: "HKDF",
        hash: "SHA-256",
        info: new Uint8Array(0),
        salt: new Uint8Array(0),
      },
      key,
      7,
    );
  } catch (err) {
    threw = true;
    strictEqual(err.name, "OperationError");
  }
  strictEqual(threw, true);
}

console.log("ok");
