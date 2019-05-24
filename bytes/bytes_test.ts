// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import {
  bytesFindIndex,
  bytesFindLastIndex,
  bytesEqual,
  bytesHasPrefix,
  bytesRepeat
} from "./bytes.ts";
import { test } from "../testing/mod.ts";
import { assertEquals, assertThrows } from "../testing/asserts.ts";

test(function bytesBytesFindIndex1(): void {
  const i = bytesFindIndex(
    new Uint8Array([1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 3]),
    new Uint8Array([0, 1, 2])
  );
  assertEquals(i, 2);
});

test(function bytesBytesFindIndex2(): void {
  const i = bytesFindIndex(new Uint8Array([0, 0, 1]), new Uint8Array([0, 1]));
  assertEquals(i, 1);
});

test(function bytesBytesFindLastIndex1(): void {
  const i = bytesFindLastIndex(
    new Uint8Array([0, 1, 2, 0, 1, 2, 0, 1, 3]),
    new Uint8Array([0, 1, 2])
  );
  assertEquals(i, 3);
});

test(function bytesBytesFindLastIndex2(): void {
  const i = bytesFindLastIndex(
    new Uint8Array([0, 1, 1]),
    new Uint8Array([0, 1])
  );
  assertEquals(i, 0);
});

test(function bytesBytesBytesEqual(): void {
  const v = bytesEqual(
    new Uint8Array([0, 1, 2, 3]),
    new Uint8Array([0, 1, 2, 3])
  );
  assertEquals(v, true);
});

test(function bytesBytesHasPrefix(): void {
  const v = bytesHasPrefix(new Uint8Array([0, 1, 2]), new Uint8Array([0, 1]));
  assertEquals(v, true);
});

test(function bytesBytesRepeat(): void {
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
    ["abc ", "abc abc abc ", 3]
  ];
  for (const [input, output, count, errMsg] of repeatTestCase) {
    if (errMsg) {
      assertThrows(
        (): void => {
          bytesRepeat(
            new TextEncoder().encode(input as string),
            count as number
          );
        },
        Error,
        errMsg as string
      );
    } else {
      const newBytes = bytesRepeat(
        new TextEncoder().encode(input as string),
        count as number
      );

      assertEquals(new TextDecoder().decode(newBytes), output);
    }
  }
});
