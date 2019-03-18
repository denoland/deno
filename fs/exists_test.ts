// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { exists, existsSync } from "./exists.ts";
import * as path from "./path/mod.ts";

const testdataDir = path.resolve("fs", "testdata");

test(async function existsFile() {
  assertEquals(
    await exists(path.join(testdataDir, "not_exist_file.ts")),
    false
  );
  assertEquals(await existsSync(path.join(testdataDir, "0.ts")), true);
});

test(function existsFileSync() {
  assertEquals(existsSync(path.join(testdataDir, "not_exist_file.ts")), false);
  assertEquals(existsSync(path.join(testdataDir, "0.ts")), true);
});

test(async function existsDirectory() {
  assertEquals(
    await exists(path.join(testdataDir, "not_exist_directory")),
    false
  );
  assertEquals(existsSync(testdataDir), true);
});

test(function existsDirectorySync() {
  assertEquals(
    existsSync(path.join(testdataDir, "not_exist_directory")),
    false
  );
  assertEquals(existsSync(testdataDir), true);
});

test(function existsLinkSync() {
  // TODO(axetroy): generate link file use Deno api instead of set a link file in repository
  assertEquals(existsSync(path.join(testdataDir, "0-link.ts")), true);
});

test(async function existsLink() {
  // TODO(axetroy): generate link file use Deno api instead of set a link file in repository
  assertEquals(await exists(path.join(testdataDir, "0-link.ts")), true);
});
