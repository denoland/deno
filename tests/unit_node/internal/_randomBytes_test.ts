// Copyright 2018-2026 the Deno authors. MIT license.
import { assert, assertEquals, assertRejects, assertThrows } from "@std/assert";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { pseudoRandomBytes, randomBytes } from "node:crypto";

const MAX_RANDOM_VALUES = 65536;
const MAX_SIZE = 4294967295;

Deno.test("randomBytes sync works correctly", function () {
  assertEquals(randomBytes(0).length, 0, "len: " + 0);
  assertEquals(randomBytes(3).length, 3, "len: " + 3);
  assertEquals(randomBytes(30).length, 30, "len: " + 30);
  assertEquals(randomBytes(300).length, 300, "len: " + 300);
  assertEquals(
    randomBytes(17 + MAX_RANDOM_VALUES).length,
    17 + MAX_RANDOM_VALUES,
    "len: " + 17 + MAX_RANDOM_VALUES,
  );
  assertEquals(
    randomBytes(MAX_RANDOM_VALUES * 100).length,
    MAX_RANDOM_VALUES * 100,
    "len: " + MAX_RANDOM_VALUES * 100,
  );
  assertThrows(() => randomBytes(MAX_SIZE + 1));
  assertThrows(() => randomBytes(-1));
});

Deno.test("randomBytes async works correctly", async function () {
  randomBytes(0, function (err, resp) {
    assert(!err);
    assertEquals(resp?.length, 0, "len: " + 0);
  });
  randomBytes(3, function (err, resp) {
    assert(!err);
    assertEquals(resp?.length, 3, "len: " + 3);
  });
  randomBytes(30, function (err, resp) {
    assert(!err);
    assertEquals(resp?.length, 30, "len: " + 30);
  });
  randomBytes(300, function (err, resp) {
    assert(!err);
    assertEquals(resp?.length, 300, "len: " + 300);
  });
  randomBytes(17 + MAX_RANDOM_VALUES, function (err, resp) {
    assert(!err);
    assertEquals(
      resp?.length,
      17 + MAX_RANDOM_VALUES,
      "len: " + 17 + MAX_RANDOM_VALUES,
    );
  });
  randomBytes(MAX_RANDOM_VALUES * 100, function (err, resp) {
    assert(!err);
    assertEquals(
      resp?.length,
      MAX_RANDOM_VALUES * 100,
      "len: " + MAX_RANDOM_VALUES * 100,
    );
  });
  assertThrows(() =>
    randomBytes(MAX_SIZE + 1, function (err) {
      //Shouldn't throw async
      assert(!err);
    })
  );
  await assertRejects(() =>
    new Promise((resolve, reject) => {
      randomBytes(-1, function (err, res) {
        //Shouldn't throw async
        if (err) {
          reject(err);
        } else {
          resolve(res);
        }
      });
    })
  );
});

Deno.test("[std/node/crypto] randomBytes callback isn't called twice if error is thrown", async () => {
  const importUrl = new URL("node:crypto", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { randomBytes } from ${JSON.stringify(importUrl)}`,
    invocation: "randomBytes(0, ",
  });
});

// https://github.com/denoland/deno/issues/28629
// randomBytes should return a buffer with its own ArrayBuffer, not a shared pool
Deno.test("randomBytes buffer has correct byteLength and unique values", function () {
  // Test that the underlying ArrayBuffer has the expected size
  const buf8 = randomBytes(8);
  assertEquals(buf8.buffer.byteLength, 8, "buffer.byteLength should match requested size");

  // Test that multiple calls return buffers with different underlying data
  // This was broken when using shared pool allocation
  const val1 = new BigUint64Array(randomBytes(8).buffer)[0];
  const val2 = new BigUint64Array(randomBytes(8).buffer)[0];
  const val3 = new BigUint64Array(randomBytes(8).buffer)[0];

  // While extremely unlikely to be identical by chance, this tests the fix
  // for the bug where all values were the same due to shared pool
  assert(
    !(val1 === val2 && val2 === val3),
    "random values should not all be identical (was caused by shared buffer pool)",
  );
});

// https://github.com/denoland/deno/issues/21632
Deno.test("pseudoRandomBytes works", function () {
  assertEquals(pseudoRandomBytes(0).length, 0, "len: " + 0);
  assertEquals(pseudoRandomBytes(3).length, 3, "len: " + 3);
  assertEquals(pseudoRandomBytes(30).length, 30, "len: " + 30);
  assertEquals(pseudoRandomBytes(300).length, 300, "len: " + 300);
  assertEquals(
    pseudoRandomBytes(17 + MAX_RANDOM_VALUES).length,
    17 + MAX_RANDOM_VALUES,
    "len: " + 17 + MAX_RANDOM_VALUES,
  );
  assertEquals(
    pseudoRandomBytes(MAX_RANDOM_VALUES * 100).length,
    MAX_RANDOM_VALUES * 100,
    "len: " + MAX_RANDOM_VALUES * 100,
  );
  assertThrows(() => pseudoRandomBytes(MAX_SIZE + 1));
  assertThrows(() => pseudoRandomBytes(-1));
});
