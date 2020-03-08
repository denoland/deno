// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const { test } = Deno;
import {
  assertEquals,
  assert,
  assertThrows,
  assertThrowsAsync
} from "../../testing/asserts.ts";
import { appendFile, appendFileSync } from "./_fs_appendFile.ts";

const decoder = new TextDecoder("utf-8");

test({
  name: "No callback Fn results in Error",
  async fn() {
    await assertThrowsAsync(
      async () => {
        await appendFile("some/path", "some data", "utf8");
      },
      Error,
      "No callback function supplied"
    );
  }
});

test({
  name: "Unsupported encoding results in error()",
  async fn() {
    await assertThrowsAsync(
      async () => {
        await appendFile(
          "some/path",
          "some data",
          "made-up-encoding",
          () => {}
        );
      },
      Error,
      "Only 'utf8' encoding is currently supported"
    );
    await assertThrowsAsync(
      async () => {
        await appendFile(
          "some/path",
          "some data",
          { encoding: "made-up-encoding" },
          () => {}
        );
      },
      Error,
      "Only 'utf8' encoding is currently supported"
    );
    assertThrows(
      () => appendFileSync("some/path", "some data", "made-up-encoding"),
      Error,
      "Only 'utf8' encoding is currently supported"
    );
    assertThrows(
      () =>
        appendFileSync("some/path", "some data", {
          encoding: "made-up-encoding"
        }),
      Error,
      "Only 'utf8' encoding is currently supported"
    );
  }
});

test({
  name: "Async: Data is written to passed in rid",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    const file: Deno.File = await Deno.open(tempFile, {
      create: true,
      write: true,
      read: true
    });
    let calledBack = false;
    await appendFile(file.rid, "hello world", () => {
      calledBack = true;
    });
    assert(calledBack);
    Deno.close(file.rid);
    const data = await Deno.readFile(tempFile);
    assertEquals(decoder.decode(data), "hello world");
    await Deno.remove(tempFile);
  }
});

test({
  name: "Async: Data is written to passed in file path",
  async fn() {
    let calledBack = false;
    const openResourcesBeforeAppend: Deno.ResourceMap = Deno.resources();
    await appendFile("_fs_appendFile_test_file.txt", "hello world", () => {
      calledBack = true;
    });
    assert(calledBack);
    assertEquals(Deno.resources(), openResourcesBeforeAppend);
    const data = await Deno.readFile("_fs_appendFile_test_file.txt");
    assertEquals(decoder.decode(data), "hello world");
    await Deno.remove("_fs_appendFile_test_file.txt");
  }
});

test({
  name:
    "Async: Callback is made with error if attempting to append data to an existing file with 'ax' flag",
  async fn() {
    let calledBack = false;
    const openResourcesBeforeAppend: Deno.ResourceMap = Deno.resources();
    const tempFile: string = await Deno.makeTempFile();
    await appendFile(tempFile, "hello world", { flag: "ax" }, (err: Error) => {
      calledBack = true;
      assert(err);
    });
    assert(calledBack);
    assertEquals(Deno.resources(), openResourcesBeforeAppend);
    await Deno.remove(tempFile);
  }
});

test({
  name: "Sync: Data is written to passed in rid",
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const file: Deno.File = Deno.openSync(tempFile, {
      create: true,
      write: true,
      read: true
    });
    appendFileSync(file.rid, "hello world");
    Deno.close(file.rid);
    const data = Deno.readFileSync(tempFile);
    assertEquals(decoder.decode(data), "hello world");
    Deno.removeSync(tempFile);
  }
});

test({
  name: "Sync: Data is written to passed in file path",
  fn() {
    const openResourcesBeforeAppend: Deno.ResourceMap = Deno.resources();
    appendFileSync("_fs_appendFile_test_file_sync.txt", "hello world");
    assertEquals(Deno.resources(), openResourcesBeforeAppend);
    const data = Deno.readFileSync("_fs_appendFile_test_file_sync.txt");
    assertEquals(decoder.decode(data), "hello world");
    Deno.removeSync("_fs_appendFile_test_file_sync.txt");
  }
});

test({
  name:
    "Sync: error thrown if attempting to append data to an existing file with 'ax' flag",
  fn() {
    const openResourcesBeforeAppend: Deno.ResourceMap = Deno.resources();
    const tempFile: string = Deno.makeTempFileSync();
    assertThrows(
      () => appendFileSync(tempFile, "hello world", { flag: "ax" }),
      Deno.errors.AlreadyExists,
      ""
    );
    assertEquals(Deno.resources(), openResourcesBeforeAppend);
    Deno.removeSync(tempFile);
  }
});
