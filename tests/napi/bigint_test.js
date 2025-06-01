// Copyright 2018-2025 the Deno authors. MIT license.

import { assertEquals, assertThrows, loadTestLibrary } from "./common.js";

const bi = loadTestLibrary();

Deno.test("cases", function () {
  const cases = [
    0n,
    -0n,
    1n,
    -1n,
    100n,
    2121n,
    -1233n,
    986583n,
    -976675n,
    98765432213456789876546896323445679887645323232436587988766545658n,
    -4350987086545760976737453646576078997096876957864353245245769809n,
  ];

  for (const num of cases) {
    if (num > -(2n ** 63n) && num < 2n ** 63n) {
      assertEquals(bi.testInt64(num), num);
      assertEquals(bi.isLossless(num, true), true);
    } else {
      assertEquals(bi.isLossless(num, true), false);
    }

    if (num >= 0 && num < 2n ** 64n) {
      assertEquals(bi.testUint64(num), num);
      assertEquals(bi.isLossless(num, false), true);
    } else {
      assertEquals(bi.isLossless(num, false), false);
    }

    assertEquals(bi.testWords(num), num);
  }
});

Deno.test(
  // TODO(bartlomieju): fix this test
  { ignore: true },
  function tooBigBigInt() {
    assertThrows(
      () => bi.createTooBigBigInt(),
      Error,
      "Invalid argument",
    );
  },
);

Deno.test(
  // TODO(bartlomieju): fix this test
  { ignore: true },
  function exceptionForwarding() {
    assertThrows(
      () => bi.makeBigIntWordsThrow(),
      Error,
      "Maximum BigInt size exceeded",
    );
  },
);
