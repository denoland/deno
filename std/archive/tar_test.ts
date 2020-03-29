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
import { assertEquals } from "../testing/asserts.ts";

import { resolve } from "../path/mod.ts";
import { Tar, Untar } from "./tar.ts";

const filePath = resolve("archive", "testdata", "example.txt");

Deno.test(async function createTarArchive(): Promise<void> {
  // initialize
  const tar = new Tar();

  // put data on memory
  const content = new TextEncoder().encode("hello tar world!");
  await tar.append("output.txt", {
    reader: new Deno.Buffer(content),
    contentSize: content.byteLength,
  });

  // put a file
  await tar.append("dir/tar.ts", { filePath });

  // write tar data to a buffer
  const writer = new Deno.Buffer(),
    wrote = await Deno.copy(writer, tar.getReader());

  /**
   * 3072 = 512 (header) + 512 (content) + 512 (header) + 512 (content)
   *       + 1024 (footer)
   */
  assertEquals(wrote, 3072);
});

Deno.test(async function deflateTarArchive(): Promise<void> {
  const fileName = "output.txt";
  const text = "hello tar world!";

  // create a tar archive
  const tar = new Tar();
  const content = new TextEncoder().encode(text);
  await tar.append(fileName, {
    reader: new Deno.Buffer(content),
    contentSize: content.byteLength,
  });

  // read data from a tar archive
  const untar = new Untar(tar.getReader());
  const buf = new Deno.Buffer();
  const result = await untar.extract(buf);
  const untarText = new TextDecoder("utf-8").decode(buf.bytes());

  // tests
  assertEquals(result.fileName, fileName);
  assertEquals(untarText, text);
});

Deno.test(async function appendFileWithLongNameToTarArchive(): Promise<void> {
  // 9 * 15 + 13 = 148 bytes
  const fileName = new Array(10).join("long-file-name/") + "file-name.txt";
  const text = "hello tar world!";

  // create a tar archive
  const tar = new Tar();
  const content = new TextEncoder().encode(text);
  await tar.append(fileName, {
    reader: new Deno.Buffer(content),
    contentSize: content.byteLength,
  });

  // read data from a tar archive
  const untar = new Untar(tar.getReader());
  const buf = new Deno.Buffer();
  const result = await untar.extract(buf);
  const untarText = new TextDecoder("utf-8").decode(buf.bytes());

  // tests
  assertEquals(result.fileName, fileName);
  assertEquals(untarText, text);
});
