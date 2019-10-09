// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { Sha1 } from "./sha1.ts";

test(function testSha1(): void {
  const sha1 = new Sha1();
  sha1.update("abcde");
  assertEquals(sha1.toString(), "03de6c570bfe24bfc328ccd7ca46b76eadaf4334");
});

test(function testSha1WithArray(): void {
  const data = Uint8Array.of(0x61, 0x62, 0x63, 0x64, 0x65);
  const sha1 = new Sha1();
  sha1.update(data);
  assertEquals(sha1.toString(), "03de6c570bfe24bfc328ccd7ca46b76eadaf4334");
});

test(function testSha1WithBuffer(): void {
  const data = Uint8Array.of(0x61, 0x62, 0x63, 0x64, 0x65);
  const sha1 = new Sha1();
  sha1.update(data.buffer);
  assertEquals(sha1.toString(), "03de6c570bfe24bfc328ccd7ca46b76eadaf4334");
});
