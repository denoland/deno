// Regression test: end-user code cannot construct `Crypto`,
// `SubtleCrypto` or `CryptoKey` directly. Each must throw a `TypeError`
// with `code: 'ERR_ILLEGAL_CONSTRUCTOR'`, matching Node and the upstream
// `parallel/test-webcrypto-constructors.js` shape.

import { strictEqual } from "node:assert";

function expectIllegal(thunk, label) {
  let caught;
  try {
    thunk();
  } catch (err) {
    caught = err;
  }
  strictEqual(
    caught instanceof TypeError,
    true,
    `${label}: expected TypeError`,
  );
  strictEqual(caught.message, "Illegal constructor", `${label}: message`);
  strictEqual(
    caught.code,
    "ERR_ILLEGAL_CONSTRUCTOR",
    `${label}: code (got ${JSON.stringify(caught.code)})`,
  );
}

// All three Web Crypto constructors are forbidden.
expectIllegal(() => new CryptoKey(), "new CryptoKey()");
expectIllegal(() => new SubtleCrypto(), "new SubtleCrypto()");
expectIllegal(() => new Crypto(), "new Crypto()");

console.log("ok");
