// Copyright 2018-2025 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "https://deno.land/std@0.200.0/assert/mod.ts";
import { join, dirname, fromFileUrl } from "https://deno.land/std@0.200.0/path/mod.ts";
import fsPromises from "node:fs/promises";

const moduleDir = dirname(fromFileUrl(import.meta.url));

Deno.test("FileHandle.appendFile - append text", async () => {
  const testFile = join(moduleDir, "append_text_test.txt");
  await Deno.writeTextFile(testFile, "Start");

  const fh = await fsPromises.open(testFile, "a+");
  try {
    await fh.appendFile(" End");
    await fh.close();

    const content = await Deno.readTextFile(testFile);
    assertEquals(content, "Start End");
  } finally {
    try { await Deno.remove(testFile); } catch {}
  }
});

Deno.test("FileHandle.appendFile - append binary", async () => {
  const testFile = join(moduleDir, "append_bin_test.bin");
  await Deno.writeFile(testFile, new Uint8Array([1,2,3]));

  const fh = await fsPromises.open(testFile, "a+");
  try {
    await fh.appendFile(new Uint8Array([4,5]));
    await fh.close();

    const content = await Deno.readFile(testFile);
    assertEquals(Array.from(content), [1,2,3,4,5]);
  } finally {
    try { await Deno.remove(testFile); } catch {}
  }
});
