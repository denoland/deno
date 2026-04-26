// Regression test: TypeErrors thrown by WebIDL dictionary converters
// when a `required: true` member is absent must carry Node's
// `code: 'ERR_MISSING_OPTION'`. Required by Node's
// `parallel/test-webcrypto-derivekey-cfrg.js` and
// `parallel/test-webcrypto-derivekey-ecdh.js`, both of which assert
// `code: 'ERR_MISSING_OPTION'` when `subtle.deriveKey({ name: 'X25519' })`
// (or 'ECDH', etc.) is called without the required `public` field.

import { rejects, strictEqual } from "node:assert";

const { subtle } = globalThis.crypto;

// 1. ECDH deriveKey without `public` -> rejects with code.
{
  const kp = await subtle.generateKey(
    { name: "ECDH", namedCurve: "P-256" },
    false,
    ["deriveKey"],
  );
  let caught;
  try {
    await subtle.deriveKey(
      { name: "ECDH" }, // <- missing `public`
      kp.privateKey,
      { name: "AES-CBC", length: 128 },
      true,
      ["encrypt", "decrypt"],
    );
  } catch (err) {
    caught = err;
  }
  strictEqual(caught instanceof TypeError, true, "ECDH: expected TypeError");
  strictEqual(
    caught.code,
    "ERR_MISSING_OPTION",
    `ECDH: code (got ${JSON.stringify(caught.code)})`,
  );
}

// 2. X25519 deriveKey without `public` -> same.
{
  const kp = await subtle.generateKey("X25519", false, ["deriveKey"]);
  let caught;
  try {
    await subtle.deriveKey(
      { name: "X25519" }, // <- missing `public`
      kp.privateKey,
      { name: "AES-CBC", length: 128 },
      true,
      ["encrypt", "decrypt"],
    );
  } catch (err) {
    caught = err;
  }
  strictEqual(caught instanceof TypeError, true, "X25519: expected TypeError");
  strictEqual(
    caught.code,
    "ERR_MISSING_OPTION",
    `X25519: code (got ${JSON.stringify(caught.code)})`,
  );
}

// 3. The error remains a plain TypeError (additive change).
{
  const kp = await subtle.generateKey(
    { name: "ECDH", namedCurve: "P-256" },
    false,
    ["deriveKey"],
  );
  await rejects(
    subtle.deriveKey(
      { name: "ECDH" },
      kp.privateKey,
      { name: "AES-CBC", length: 128 },
      true,
      ["encrypt", "decrypt"],
    ),
    {
      name: "TypeError",
      // The existing message is preserved.
      message:
        /can not be converted to 'EcdhKeyDeriveParams'.*'public' is required/,
    },
  );
}

console.log("ok");
