// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// TODO(axetroy): Add test for Windows once symlink is implemented for Windows.
import { test } from "../testing/mod.ts";
import {
  assertEquals,
  assertThrows,
  assertThrowsAsync
} from "../testing/asserts.ts";
import { ensureLink, ensureLinkSync } from "./ensure_link.ts";
import * as path from "./path/mod.ts";

const testdataDir = path.resolve("fs", "testdata");

test(async function ensureLinkIfItNotExist(): Promise<void> {
  const srcDir = path.join(testdataDir, "ensure_link_1");
  const destDir = path.join(testdataDir, "ensure_link_1_2");
  const testFile = path.join(srcDir, "test.txt");
  const linkFile = path.join(destDir, "link.txt");

  await assertThrowsAsync(
    async (): Promise<void> => {
      await ensureLink(testFile, linkFile);
    }
  );

  await Deno.remove(destDir, { recursive: true });
});

test(function ensureLinkSyncIfItNotExist(): void {
  const testDir = path.join(testdataDir, "ensure_link_2");
  const testFile = path.join(testDir, "test.txt");
  const linkFile = path.join(testDir, "link.txt");

  assertThrows((): void => {
    ensureLinkSync(testFile, linkFile);
  });

  Deno.removeSync(testDir, { recursive: true });
});

test(async function ensureLinkIfItExist(): Promise<void> {
  const testDir = path.join(testdataDir, "ensure_link_3");
  const testFile = path.join(testDir, "test.txt");
  const linkFile = path.join(testDir, "link.txt");

  await Deno.mkdir(testDir, true);
  await Deno.writeFile(testFile, new Uint8Array());

  await ensureLink(testFile, linkFile);

  const srcStat = await Deno.lstat(testFile);
  const linkStat = await Deno.lstat(linkFile);

  assertEquals(srcStat.isFile(), true);
  assertEquals(linkStat.isFile(), true);

  // har link success. try to change one of them. they should be change both.

  // let's change origin file.
  await Deno.writeFile(testFile, new TextEncoder().encode("123"));

  const testFileContent1 = new TextDecoder().decode(
    await Deno.readFile(testFile)
  );
  const linkFileContent1 = new TextDecoder().decode(
    await Deno.readFile(testFile)
  );

  assertEquals(testFileContent1, "123");
  assertEquals(testFileContent1, linkFileContent1);

  // let's change link file.
  await Deno.writeFile(testFile, new TextEncoder().encode("abc"));

  const testFileContent2 = new TextDecoder().decode(
    await Deno.readFile(testFile)
  );
  const linkFileContent2 = new TextDecoder().decode(
    await Deno.readFile(testFile)
  );

  assertEquals(testFileContent2, "abc");
  assertEquals(testFileContent2, linkFileContent2);

  await Deno.remove(testDir, { recursive: true });
});

test(function ensureLinkSyncIfItExist(): void {
  const testDir = path.join(testdataDir, "ensure_link_4");
  const testFile = path.join(testDir, "test.txt");
  const linkFile = path.join(testDir, "link.txt");

  Deno.mkdirSync(testDir, true);
  Deno.writeFileSync(testFile, new Uint8Array());

  ensureLinkSync(testFile, linkFile);

  const srcStat = Deno.lstatSync(testFile);

  const linkStat = Deno.lstatSync(linkFile);

  assertEquals(srcStat.isFile(), true);
  assertEquals(linkStat.isFile(), true);

  // har link success. try to change one of them. they should be change both.

  // let's change origin file.
  Deno.writeFileSync(testFile, new TextEncoder().encode("123"));

  const testFileContent1 = new TextDecoder().decode(
    Deno.readFileSync(testFile)
  );
  const linkFileContent1 = new TextDecoder().decode(
    Deno.readFileSync(testFile)
  );

  assertEquals(testFileContent1, "123");
  assertEquals(testFileContent1, linkFileContent1);

  // let's change link file.
  Deno.writeFileSync(testFile, new TextEncoder().encode("abc"));

  const testFileContent2 = new TextDecoder().decode(
    Deno.readFileSync(testFile)
  );
  const linkFileContent2 = new TextDecoder().decode(
    Deno.readFileSync(testFile)
  );

  assertEquals(testFileContent2, "abc");
  assertEquals(testFileContent2, linkFileContent2);

  Deno.removeSync(testDir, { recursive: true });
});

test(async function ensureLinkDirectoryIfItExist(): Promise<void> {
  const testDir = path.join(testdataDir, "ensure_link_origin_3");
  const linkDir = path.join(testdataDir, "ensure_link_link_3");
  const testFile = path.join(testDir, "test.txt");

  await Deno.mkdir(testDir, true);
  await Deno.writeFile(testFile, new Uint8Array());

  await assertThrowsAsync(
    async (): Promise<void> => {
      await ensureLink(testDir, linkDir);
    },
    Deno.DenoError
    // "Operation not permitted (os error 1)" // throw an local matching test
    // "Access is denied. (os error 5)" // throw in CI
  );

  Deno.removeSync(testDir, { recursive: true });
});

test(function ensureLinkSyncDirectoryIfItExist(): void {
  const testDir = path.join(testdataDir, "ensure_link_origin_3");
  const linkDir = path.join(testdataDir, "ensure_link_link_3");
  const testFile = path.join(testDir, "test.txt");

  Deno.mkdirSync(testDir, true);
  Deno.writeFileSync(testFile, new Uint8Array());

  assertThrows(
    (): void => {
      ensureLinkSync(testDir, linkDir);
    },
    Deno.DenoError
    // "Operation not permitted (os error 1)" // throw an local matching test
    // "Access is denied. (os error 5)" // throw in CI
  );

  Deno.removeSync(testDir, { recursive: true });
});
