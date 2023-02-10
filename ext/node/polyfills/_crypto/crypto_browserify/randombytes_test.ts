// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 crypto-browserify. All rights reserved. MIT license.
import { randomBytes } from "./randombytes.ts";
import {
  assert,
  assertEquals,
  assertThrows,
} from "../../../testing/asserts.ts";

const MAX_BYTES = 65536;
const MAX_UINT32 = 4294967295;

Deno.test("sync", () => {
  assertEquals(randomBytes(0)!.length, 0, "len: " + 0);
  assertEquals(randomBytes(3)!.length, 3, "len: " + 3);
  assertEquals(randomBytes(30)!.length, 30, "len: " + 30);
  assertEquals(randomBytes(300)!.length, 300, "len: " + 300);
  assertEquals(
    randomBytes(17 + MAX_BYTES)!.length,
    17 + MAX_BYTES,
    "len: " + 17 + MAX_BYTES,
  );
  assertEquals(
    randomBytes(MAX_BYTES * 100)!.length,
    MAX_BYTES * 100,
    "len: " + MAX_BYTES * 100,
  );
  assertThrows(function () {
    randomBytes(MAX_UINT32 + 1);
  });
  assertThrows(function () {
    assert(randomBytes(-1));
  });
  assertThrows(function () {
    assert(randomBytes("hello" as unknown as number));
  });
});

Deno.test("async", async () => {
  await new Promise<void>((resolve) => {
    randomBytes(3, function (err, resp) {
      if (err) throw err;

      assertEquals(resp.length, 3, "len: " + 3);
      resolve();
    });
  });
});
