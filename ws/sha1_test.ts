// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { assert, test } from "../testing/mod.ts";
import { Sha1 } from "./sha1.ts";

test(function testSha1() {
  const sha1 = new Sha1();
  sha1.update("abcde");
  assert.equal(sha1.toString(), "03de6c570bfe24bfc328ccd7ca46b76eadaf4334");
});

test(function testSha1WithArray() {
  const data = Uint8Array.of(0x61, 0x62, 0x63, 0x64, 0x65);
  const sha1 = new Sha1();
  sha1.update(data);
  assert.equal(sha1.toString(), "03de6c570bfe24bfc328ccd7ca46b76eadaf4334");
});

test(function testSha1WithBuffer() {
  const data = Uint8Array.of(0x61, 0x62, 0x63, 0x64, 0x65);
  const sha1 = new Sha1();
  sha1.update(data.buffer);
  assert.equal(sha1.toString(), "03de6c570bfe24bfc328ccd7ca46b76eadaf4334");
});
