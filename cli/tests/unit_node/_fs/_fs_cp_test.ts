// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "../../../../test_util/std/assert/mod.ts";
import { cp, exists, readFile } from "node:fs/promise";
import { cpSync } from "node:fs";
import * as path from "../../../../test_util/std/path/mod.ts";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testData = path.resolve(moduleDir, "testdata", "hello.txt");
const destFile = "./destination.txt";

Deno.test("[std/node/fs] cp", async function () {
  try {
    await cp(testData, destFile);
    assert(await exists(destFile));
    assertEquals(
      await readFile(destFile, { encoding: "utf8" }),
      await readFile(testData, { encoding: "utf8" }),
    );
  } finally {
    Deno.removeSync(destFile);
  }
});

Deno.test("[std/node/fs] cpSync", async function () {
  cpSync(testData, destFile);
  assert(await exists(destFile));
  assertEquals(
    await readFile(destFile, { encoding: "utf8" }),
    await readFile(testData, { encoding: "utf8" }),
  );
  Deno.removeSync(destFile);
});
