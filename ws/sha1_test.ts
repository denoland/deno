// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertEq } from "../testing/asserts.ts";
import { Sha1 } from "./sha1.ts";

test(function testSha1() {
  const sha1 = new Sha1();
  sha1.update("abcde");
  assertEq(sha1.toString(), "03de6c570bfe24bfc328ccd7ca46b76eadaf4334");
});

test(function testSha1WithArray() {
  const data = Uint8Array.of(0x61, 0x62, 0x63, 0x64, 0x65);
  const sha1 = new Sha1();
  sha1.update(data);
  assertEq(sha1.toString(), "03de6c570bfe24bfc328ccd7ca46b76eadaf4334");
});

test(function testSha1WithBuffer() {
  const data = Uint8Array.of(0x61, 0x62, 0x63, 0x64, 0x65);
  const sha1 = new Sha1();
  sha1.update(data.buffer);
  assertEq(sha1.toString(), "03de6c570bfe24bfc328ccd7ca46b76eadaf4334");
});
