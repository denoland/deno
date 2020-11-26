// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import {
  concat,
  contains,
  copyBytes,
  equal,
  findIndex,
  findLastIndex,
  hasPrefix,
  hasSuffix,
  repeat,
} from "./mod.ts";
import { assert, assertEquals, assertThrows } from "../testing/asserts.ts";
import { decode, encode } from "../encoding/utf8.ts";

Deno.test("[bytes] findIndex1", () => {
  const i = findIndex(
    new Uint8Array([1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 3]),
    new Uint8Array([0, 1, 2]),
  );
  assertEquals(i, 2);
});

Deno.test("[bytes] findIndex2", () => {
  const i = findIndex(new Uint8Array([0, 0, 1]), new Uint8Array([0, 1]));
  assertEquals(i, 1);
});

Deno.test("[bytes] findIndex3", () => {
  const i = findIndex(encode("Deno"), encode("D"));
  assertEquals(i, 0);
});

Deno.test("[bytes] findLastIndex1", () => {
  const i = findLastIndex(
    new Uint8Array([0, 1, 2, 0, 1, 2, 0, 1, 3]),
    new Uint8Array([0, 1, 2]),
  );
  assertEquals(i, 3);
});

Deno.test("[bytes] findLastIndex2", () => {
  const i = findLastIndex(new Uint8Array([0, 1, 1]), new Uint8Array([0, 1]));
  assertEquals(i, 0);
});

Deno.test("[bytes] equal", () => {
  const v = equal(new Uint8Array([0, 1, 2, 3]), new Uint8Array([0, 1, 2, 3]));
  assertEquals(v, true);
});

Deno.test("[bytes] hasPrefix", () => {
  const v = hasPrefix(new Uint8Array([0, 1, 2]), new Uint8Array([0, 1]));
  assertEquals(v, true);
});

Deno.test("[bytes] hasSuffix", () => {
  const v = hasSuffix(new Uint8Array([0, 1, 2]), new Uint8Array([1, 2]));
  assertEquals(v, true);
});

Deno.test("[bytes] repeat", () => {
  // input / output / count / error message
  const repeatTestCase = [
    ["", "", 0],
    ["", "", 1],
    ["", "", 1.1, "bytes: repeat count must be an integer"],
    ["", "", 2],
    ["", "", 0],
    ["-", "", 0],
    ["-", "-", -1, "bytes: negative repeat count"],
    ["-", "----------", 10],
    ["abc ", "abc abc abc ", 3],
  ];
  for (const [input, output, count, errMsg] of repeatTestCase) {
    if (errMsg) {
      assertThrows(
        (): void => {
          repeat(new TextEncoder().encode(input as string), count as number);
        },
        Error,
        errMsg as string,
      );
    } else {
      const newBytes = repeat(
        new TextEncoder().encode(input as string),
        count as number,
      );

      assertEquals(new TextDecoder().decode(newBytes), output);
    }
  }
});

Deno.test("[bytes] concat", () => {
  const u1 = encode("Hello ");
  const u2 = encode("World");
  const joined = concat(u1, u2);
  assertEquals(decode(joined), "Hello World");
  assert(u1 !== joined);
  assert(u2 !== joined);
});

Deno.test("[bytes] concat empty arrays", () => {
  const u1 = new Uint8Array();
  const u2 = new Uint8Array();
  const joined = concat(u1, u2);
  assertEquals(joined.byteLength, 0);
  assert(u1 !== joined);
  assert(u2 !== joined);
});

Deno.test("[bytes] contain", () => {
  const source = encode("deno.land");
  const pattern = encode("deno");
  assert(contains(source, pattern));

  assert(contains(new Uint8Array([0, 1, 2, 3]), new Uint8Array([2, 3])));
});

Deno.test("[io/tuil] copyBytes", function (): void {
  const dst = new Uint8Array(4);

  dst.fill(0);
  let src = Uint8Array.of(1, 2);
  let len = copyBytes(src, dst, 0);
  assert(len === 2);
  assertEquals(dst, Uint8Array.of(1, 2, 0, 0));

  dst.fill(0);
  src = Uint8Array.of(1, 2);
  len = copyBytes(src, dst, 1);
  assert(len === 2);
  assertEquals(dst, Uint8Array.of(0, 1, 2, 0));

  dst.fill(0);
  src = Uint8Array.of(1, 2, 3, 4, 5);
  len = copyBytes(src, dst);
  assert(len === 4);
  assertEquals(dst, Uint8Array.of(1, 2, 3, 4));

  dst.fill(0);
  src = Uint8Array.of(1, 2);
  len = copyBytes(src, dst, 100);
  assert(len === 0);
  assertEquals(dst, Uint8Array.of(0, 0, 0, 0));

  dst.fill(0);
  src = Uint8Array.of(3, 4);
  len = copyBytes(src, dst, -2);
  assert(len === 2);
  assertEquals(dst, Uint8Array.of(3, 4, 0, 0));
});
