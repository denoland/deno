// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { equals } from "./equals.ts";
import { assert } from "../assert/mod.ts";

Deno.test("[bytes] equals", () => {
  const v = equals(new Uint8Array([0, 1, 2, 3]), new Uint8Array([0, 1, 2, 3]));
  const v2 = equals(new Uint8Array([0, 1, 2, 2]), new Uint8Array([0, 1, 2, 3]));
  const v3 = equals(new Uint8Array([0, 1, 2, 3]), new Uint8Array([0, 1, 2]));
  assert(v);
  assert(!v2);
  assert(!v3);
});

Deno.test("[bytes] equals randomized testing", () => {
  // run tests before and after cutoff
  for (let len = 995; len <= 1005; len++) {
    const arr1 = crypto.getRandomValues(new Uint8Array(len));
    const arr2 = crypto.getRandomValues(new Uint8Array(len));
    const arr3 = arr1.slice(0);
    // the chance of arr1 equaling arr2 is basically 0
    // but introduce an inequality at the end just in case
    arr2[arr2.length - 1] = arr1[arr1.length - 1] ^ 1;
    // arr3 is arr1 but with an inequality in the very last element
    // this is to test the equality check when length isn't a multiple of 4
    arr3[arr3.length - 1] ^= 1;
    // arrays with same underlying ArrayBuffer should be equal
    assert(equals(arr1, arr1));
    // equal arrays with different underlying ArrayBuffers should be equal
    assert(equals(arr1, arr1.slice(0)));
    // inequal arrays should be inequal
    assert(!equals(arr1, arr2));
    assert(!equals(arr1, arr3));
  }
});
