// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "../testing/asserts.ts";
import * as path from "../path/mod.ts";
import { copyBytes, tempFile } from "./util.ts";

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

Deno.test({
  name: "[io/util] tempfile",
  fn: async function (): Promise<void> {
    const f = await tempFile(".", {
      prefix: "prefix-",
      postfix: "-postfix",
    });
    const base = path.basename(f.filepath);
    assert(!!base.match(/^prefix-.+?-postfix$/));
    f.file.close();
    await Deno.remove(f.filepath);
  },
});
