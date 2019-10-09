// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { remove } = Deno;
import { test } from "../testing/mod.ts";
import { assert, assertEquals } from "../testing/asserts.ts";
import { copyBytes, tempFile } from "./util.ts";
import * as path from "../fs/path.ts";

test(function testCopyBytes(): void {
  const dst = new Uint8Array(4);

  dst.fill(0);
  let src = Uint8Array.of(1, 2);
  let len = copyBytes(dst, src, 0);
  assert(len === 2);
  assertEquals(dst, Uint8Array.of(1, 2, 0, 0));

  dst.fill(0);
  src = Uint8Array.of(1, 2);
  len = copyBytes(dst, src, 1);
  assert(len === 2);
  assertEquals(dst, Uint8Array.of(0, 1, 2, 0));

  dst.fill(0);
  src = Uint8Array.of(1, 2, 3, 4, 5);
  len = copyBytes(dst, src);
  assert(len === 4);
  assertEquals(dst, Uint8Array.of(1, 2, 3, 4));

  dst.fill(0);
  src = Uint8Array.of(1, 2);
  len = copyBytes(dst, src, 100);
  assert(len === 0);
  assertEquals(dst, Uint8Array.of(0, 0, 0, 0));

  dst.fill(0);
  src = Uint8Array.of(3, 4);
  len = copyBytes(dst, src, -2);
  assert(len === 2);
  assertEquals(dst, Uint8Array.of(3, 4, 0, 0));
});

test(async function ioTempfile(): Promise<void> {
  const f = await tempFile(".", {
    prefix: "prefix-",
    postfix: "-postfix"
  });
  console.log(f.file, f.filepath);
  const base = path.basename(f.filepath);
  assert(!!base.match(/^prefix-.+?-postfix$/));
  await remove(f.filepath);
});
