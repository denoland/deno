// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows, fail } from "@std/assert";
import { appendFile, appendFileSync } from "node:fs";
import { fromFileUrl } from "@std/path";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";

const decoder = new TextDecoder("utf-8");

Deno.test({
  name: "No callback Fn results in Error",
  fn() {
    assertThrows(
      () => {
        // @ts-expect-error Argument of type 'string' is not assignable to parameter of type 'NoParamCallback'
        appendFile("some/path", "some data", "utf8");
      },
      Error,
      "The \"cb\" argument must be of type function. Received type string ('utf8')",
    );
  },
});

Deno.test({
  name: "Unsupported encoding results in error()",
  fn() {
    assertThrows(
      () => {
        // @ts-expect-error Type '"made-up-encoding"' is not assignable to type
        appendFile("some/path", "some data", "made-up-encoding", () => {});
      },
      Error,
      "The argument 'made-up-encoding' is invalid encoding. Received 'encoding'",
    );
    assertThrows(
      () => {
        appendFile(
          "some/path",
          "some data",
          // @ts-expect-error Type '"made-up-encoding"' is not assignable to type
          { encoding: "made-up-encoding" },
          () => {},
        );
      },
      Error,
      "The argument 'made-up-encoding' is invalid encoding. Received 'encoding'",
    );
    assertThrows(
      // @ts-expect-error Type '"made-up-encoding"' is not assignable to type
      () => appendFileSync("some/path", "some data", "made-up-encoding"),
      Error,
      "The argument 'made-up-encoding' is invalid encoding. Received 'encoding'",
    );
    assertThrows(
      () =>
        appendFileSync("some/path", "some data", {
          // @ts-expect-error Type '"made-up-encoding"' is not assignable to type
          encoding: "made-up-encoding",
        }),
      Error,
      "The argument 'made-up-encoding' is invalid encoding. Received 'encoding'",
    );
  },
});

Deno.test({
  name: "Async: Data is written to passed in rid",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    using file = await Deno.open(tempFile, {
      create: true,
      write: true,
      read: true,
    });
    await new Promise<void>((resolve, reject) => {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      appendFile(file.rid, "hello world", (err) => {
        if (err) reject();
        else resolve();
      });
    })
      .then(async () => {
        const data = await Deno.readFile(tempFile);
        assertEquals(decoder.decode(data), "hello world");
      }, () => {
        fail("No error expected");
      })
      .finally(async () => {
        await Deno.remove(tempFile);
      });
  },
});

Deno.test({
  name: "Async: Data is written to passed in file path",
  async fn() {
    await new Promise<void>((resolve, reject) => {
      appendFile("_fs_appendFile_test_file.txt", "hello world", (err) => {
        if (err) reject(err);
        else resolve();
      });
    })
      .then(async () => {
        const data = await Deno.readFile("_fs_appendFile_test_file.txt");
        assertEquals(decoder.decode(data), "hello world");
      }, (err) => {
        fail("No error was expected: " + err);
      })
      .finally(async () => {
        await Deno.remove("_fs_appendFile_test_file.txt");
      });
  },
});

Deno.test({
  name: "Async: Data is written to passed in URL",
  async fn() {
    const fileURL = new URL("_fs_appendFile_test_file.txt", import.meta.url);
    await new Promise<void>((resolve, reject) => {
      appendFile(fileURL, "hello world", (err) => {
        if (err) reject(err);
        else resolve();
      });
    })
      .then(async () => {
        const data = await Deno.readFile(fromFileUrl(fileURL));
        assertEquals(decoder.decode(data), "hello world");
      }, (err) => {
        fail("No error was expected: " + err);
      })
      .finally(async () => {
        await Deno.remove(fromFileUrl(fileURL));
      });
  },
});

Deno.test({
  name:
    "Async: Callback is made with error if attempting to append data to an existing file with 'ax' flag",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    await new Promise<void>((resolve, reject) => {
      appendFile(tempFile, "hello world", { flag: "ax" }, (err) => {
        if (err) reject(err);
        else resolve();
      });
    })
      .then(() => fail("Expected error to be thrown"))
      .catch(() => {})
      .finally(async () => {
        await Deno.remove(tempFile);
      });
  },
});

Deno.test({
  name: "Sync: Data is written to passed in rid",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    using file = Deno.openSync(tempFile, {
      create: true,
      write: true,
      read: true,
    });
    // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
    appendFileSync(file.rid, "hello world");
    const data = Deno.readFileSync(tempFile);
    assertEquals(decoder.decode(data), "hello world");
    Deno.removeSync(tempFile);
  },
});

Deno.test({
  name: "Sync: Data is written to passed in file path",
  fn() {
    appendFileSync("_fs_appendFile_test_file_sync.txt", "hello world");
    const data = Deno.readFileSync("_fs_appendFile_test_file_sync.txt");
    assertEquals(decoder.decode(data), "hello world");
    Deno.removeSync("_fs_appendFile_test_file_sync.txt");
  },
});

Deno.test({
  name:
    "Sync: error thrown if attempting to append data to an existing file with 'ax' flag",
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    assertThrows(
      () => appendFileSync(tempFile, "hello world", { flag: "ax" }),
      Error,
      "",
    );
    Deno.removeSync(tempFile);
  },
});

Deno.test({
  name: "Sync: Data is written in Uint8Array to passed in file path",
  fn() {
    const testData = new TextEncoder().encode("hello world");
    appendFileSync("_fs_appendFile_test_file_sync.txt", testData);
    const data = Deno.readFileSync("_fs_appendFile_test_file_sync.txt");
    assertEquals(data, testData);
    Deno.removeSync("_fs_appendFile_test_file_sync.txt");
  },
});

Deno.test({
  name: "Async: Data is written in Uint8Array to passed in file path",
  async fn() {
    const testData = new TextEncoder().encode("hello world");
    await new Promise<void>((resolve, reject) => {
      appendFile("_fs_appendFile_test_file.txt", testData, (err) => {
        if (err) reject(err);
        else resolve();
      });
    })
      .then(async () => {
        const data = await Deno.readFile("_fs_appendFile_test_file.txt");
        assertEquals(data, testData);
      }, (err) => {
        fail("No error was expected: " + err);
      })
      .finally(async () => {
        await Deno.remove("_fs_appendFile_test_file.txt");
      });
  },
});

Deno.test("[std/node/fs] appendFile callback isn't called twice if error is thrown", async () => {
  const tempFile = await Deno.makeTempFile();
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { appendFile } from ${JSON.stringify(importUrl)}`,
    invocation: `appendFile(${JSON.stringify(tempFile)}, "hello world", `,
    async cleanup() {
      await Deno.remove(tempFile);
    },
  });
});
