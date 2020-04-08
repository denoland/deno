// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../../testing/asserts.ts";
import { exists, existsSync } from "./_fs_exists.ts";
import * as path from "../../path/mod.ts";

const { test } = Deno;

const testAvailableData = path.resolve("_fs", "testdata", "hello.txt");
const testNotAvailableData = path.resolve(
  "_fs",
  "testdata",
  "notAvailable.txt"
);

test(async function existsFile() {
  const availableFile = await new Promise((resolve) => {
    exists(testAvailableData, (exists: boolean) => resolve(exists));
  });
  const notAvailableFile = await new Promise((resolve) => {
    exists(testNotAvailableData, (exists: boolean) => resolve(exists));
  });
  assertEquals(availableFile, true);
  assertEquals(notAvailableFile, false);
});

test(function existsSyncFile() {
  assertEquals(existsSync(testAvailableData), true);
  assertEquals(existsSync(testNotAvailableData), false);
});
