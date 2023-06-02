// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import * as path from "../../../../test_util/std/path/mod.ts";
import {
  assert,
  assertEquals,
} from "../../../../test_util/std/testing/asserts.ts";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testData = path.resolve(moduleDir, "testdata", "hello.txt");

Deno.test("readFileSuccess", async function () {
  const fs = await import("node:fs/promises");
  const fileHandle = await fs.open(testData);
  const data = await fileHandle.readFile();

  assert(data instanceof Uint8Array);
  assertEquals(new TextDecoder().decode(data as Uint8Array), "hello world");

  Deno.close(fileHandle.fd);
});
