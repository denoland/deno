import {
  bytesFindIndex,
  bytesFindLastIndex,
  bytesEqual,
  bytesHasPrefix
} from "./bytes.ts";
import { test } from "../testing/mod.ts";
import { assertEq } from "../testing/asserts.ts";

test(function bytesBytesFindIndex() {
  const i = bytesFindIndex(
    new Uint8Array([1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 3]),
    new Uint8Array([0, 1, 2])
  );
  assertEq(i, 2);
});

test(function bytesBytesFindLastIndex1() {
  const i = bytesFindLastIndex(
    new Uint8Array([0, 1, 2, 0, 1, 2, 0, 1, 3]),
    new Uint8Array([0, 1, 2])
  );
  assertEq(i, 3);
});

test(function bytesBytesBytesEqual() {
  const v = bytesEqual(
    new Uint8Array([0, 1, 2, 3]),
    new Uint8Array([0, 1, 2, 3])
  );
  assertEq(v, true);
});

test(function bytesBytesHasPrefix() {
  const v = bytesHasPrefix(new Uint8Array([0, 1, 2]), new Uint8Array([0, 1]));
  assertEq(v, true);
});
