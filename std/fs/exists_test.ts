// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { exists, existsSync } from "./exists.ts";
import * as path from "./path/mod.ts";

const testdataDir = path.resolve("fs", "testdata");

test(async function existsFile(): Promise<void> {
  assertEquals(
    await exists(path.join(testdataDir, "not_exist_file.ts")),
    false
  );
  assertEquals(await existsSync(path.join(testdataDir, "0.ts")), true);
});

test(function existsFileSync(): void {
  assertEquals(existsSync(path.join(testdataDir, "not_exist_file.ts")), false);
  assertEquals(existsSync(path.join(testdataDir, "0.ts")), true);
});

test(async function existsDirectory(): Promise<void> {
  assertEquals(
    await exists(path.join(testdataDir, "not_exist_directory")),
    false
  );
  assertEquals(existsSync(testdataDir), true);
});

test(function existsDirectorySync(): void {
  assertEquals(
    existsSync(path.join(testdataDir, "not_exist_directory")),
    false
  );
  assertEquals(existsSync(testdataDir), true);
});

test(function existsLinkSync(): void {
  // TODO(axetroy): generate link file use Deno api instead of set a link file
  // in repository
  assertEquals(existsSync(path.join(testdataDir, "0-link.ts")), true);
});

test(async function existsLink(): Promise<void> {
  // TODO(axetroy): generate link file use Deno api instead of set a link file
  // in repository
  assertEquals(await exists(path.join(testdataDir, "0-link.ts")), true);
});
