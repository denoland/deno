// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../../testing/asserts.ts";
import { exists, existsSync } from "./_fs_exists.ts";

const { test } = Deno;

test(async function existsFile() {
  const availableFile = await new Promise(async (resolve) => {
    const tmpFilePath = await Deno.makeTempFile();
    exists(tmpFilePath, (exists: boolean) => resolve(exists));
    Deno.remove(tmpFilePath);
  });
  const notAvailableFile = await new Promise((resolve) => {
    exists("./notAvailable.txt", (exists: boolean) => resolve(exists));
  });
  assertEquals(availableFile, true);
  assertEquals(notAvailableFile, false);
});

test(function existsSyncFile() {
  const tmpFilePath = Deno.makeTempFileSync();
  assertEquals(existsSync(tmpFilePath), true);
  Deno.removeSync(tmpFilePath);
  assertEquals(existsSync("./notAvailable.txt"), false);
});
