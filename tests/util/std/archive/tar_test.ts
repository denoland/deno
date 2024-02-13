// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
/**
 * Tar test
 *
 * **test summary**
 * - create a tar archive in memory containing output.txt and dir/tar.ts.
 * - read and deflate a tar archive containing output.txt
 *
 * **to run this test**
 * deno run --allow-read archive/tar_test.ts
 */
import { assert, assertEquals } from "../assert/mod.ts";
import { resolve } from "../path/mod.ts";
import { Tar } from "./tar.ts";
import { Untar } from "./untar.ts";
import { Buffer } from "../io/buffer.ts";
import { copy } from "../streams/copy.ts";
import { readAll } from "../streams/read_all.ts";
import { filePath, testdataDir } from "./_test_common.ts";

Deno.test("createTarArchive", async function () {
  // initialize
  const tar = new Tar();

  // put data on memory
  const content = new TextEncoder().encode("hello tar world!");
  await tar.append("output.txt", {
    reader: new Buffer(content),
    contentSize: content.byteLength,
  });

  // put a file
  await tar.append("dir/tar.ts", { filePath });

  // write tar data to a buffer
  const writer = new Buffer();
  const wrote = await copy(tar.getReader(), writer);

  /**
   * 3072 = 512 (header) + 512 (content) + 512 (header) + 512 (content)
   *       + 1024 (footer)
   */
  assertEquals(wrote, 3072);
});

Deno.test("deflateTarArchive", async function () {
  const fileName = "output.txt";
  const text = "hello tar world!";

  // create a tar archive
  const tar = new Tar();
  const content = new TextEncoder().encode(text);
  await tar.append(fileName, {
    reader: new Buffer(content),
    contentSize: content.byteLength,
  });

  // read data from a tar archive
  const untar = new Untar(tar.getReader());
  const result = await untar.extract();
  assert(result !== null);
  const untarText = new TextDecoder("utf-8").decode(await readAll(result));

  assertEquals(await untar.extract(), null); // EOF
  // tests
  assertEquals(result.fileName, fileName);
  assertEquals(untarText, text);
});

Deno.test("appendFileWithLongNameToTarArchive", async function (): Promise<
  void
> {
  // 9 * 15 + 13 = 148 bytes
  const fileName = "long-file-name/".repeat(10) + "file-name.txt";
  const text = "hello tar world!";

  // create a tar archive
  const tar = new Tar();
  const content = new TextEncoder().encode(text);
  await tar.append(fileName, {
    reader: new Buffer(content),
    contentSize: content.byteLength,
  });

  // read data from a tar archive
  const untar = new Untar(tar.getReader());
  const result = await untar.extract();
  assert(result !== null);
  assert(!result.consumed);
  const untarText = new TextDecoder("utf-8").decode(await readAll(result));
  assert(result.consumed);

  // tests
  assertEquals(result.fileName, fileName);
  assertEquals(untarText, text);
});

Deno.test("directoryEntryType", async function () {
  const tar = new Tar();

  tar.append("directory/", {
    reader: new Buffer(),
    contentSize: 0,
    type: "directory",
  });

  const filePath = resolve(testdataDir);
  tar.append("archive/testdata/", {
    filePath,
  });

  const outputFile = resolve(testdataDir, "directory_type_test.tar");
  const file = await Deno.open(outputFile, { create: true, write: true });
  await copy(tar.getReader(), file);
  file.close();

  const reader = await Deno.open(outputFile, { read: true });
  const untar = new Untar(reader);
  await Array.fromAsync(
    untar,
    (entry) => assertEquals(entry.type, "directory"),
  );

  reader.close();
  await Deno.remove(outputFile);
});
