// Copyright 2018-2025 the Deno authors. MIT license.
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { existsSync, promises, readFile, readFileSync } from "node:fs";
import * as path from "@std/path";
import { assert, assertEquals, assertMatch } from "@std/assert";
import { Buffer } from "node:buffer";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testData = path.resolve(moduleDir, "testdata", "hello.txt");

Deno.test("readFileSuccess", async function () {
  const data = await new Promise((res, rej) => {
    readFile(testData, (err, data) => {
      if (err) {
        rej(err);
      }
      res(data);
    });
  });

  assert(data instanceof Uint8Array);
  assertEquals(new TextDecoder().decode(data as Uint8Array), "hello world");
});

Deno.test("readFileEncodeUtf8Success", async function () {
  const data = await new Promise((res, rej) => {
    readFile(testData, { encoding: "utf8" }, (err, data) => {
      if (err) {
        rej(err);
      }
      res(data);
    });
  });
  assertEquals(typeof data, "string");
  assertEquals(data as string, "hello world");
});

Deno.test("readFileEncodeHexSuccess", async function () {
  const data = await new Promise((res, rej) => {
    readFile(testData, { encoding: "hex" }, (err, data) => {
      if (err) {
        rej(err);
      }
      res(data);
    });
  });

  assertEquals(typeof data, "string");
  assertEquals(data as string, "68656c6c6f20776f726c64");
});

Deno.test("readFileEncodeBase64Success", async function () {
  const data = await new Promise((res, rej) => {
    readFile(testData, { encoding: "base64" }, (err, data) => {
      if (err) {
        rej(err);
      }
      res(data);
    });
  });
  assertEquals(typeof data, "string");
  assertEquals(data as string, "aGVsbG8gd29ybGQ=");
});

Deno.test("readFileEncodingAsString", async function () {
  const data = await new Promise((res, rej) => {
    readFile(testData, "utf8", (err, data) => {
      if (err) {
        rej(err);
      }
      res(data);
    });
  });

  assertEquals(typeof data, "string");
  assertEquals(data as string, "hello world");
});

Deno.test("readFileSyncSuccess", function () {
  const data = readFileSync(testData);
  assert(data instanceof Uint8Array);
  assertEquals(new TextDecoder().decode(data as Uint8Array), "hello world");
});

Deno.test("readFileEncodeUtf8Success", function () {
  const data = readFileSync(testData, { encoding: "utf8" });
  assertEquals(typeof data, "string");
  assertEquals(data as string, "hello world");
});

Deno.test("readFileEncodeHexSuccess", function () {
  const data = readFileSync(testData, { encoding: "hex" });
  assertEquals(typeof data, "string");
  assertEquals(data as string, "68656c6c6f20776f726c64");
});

Deno.test("readFileEncodeBase64Success", function () {
  const data = readFileSync(testData, { encoding: "base64" });
  assertEquals(typeof data, "string");
  assertEquals(data as string, "aGVsbG8gd29ybGQ=");
});

Deno.test("readFileEncodeAsString", function () {
  const data = readFileSync(testData, "utf8");
  assertEquals(typeof data, "string");
  assertEquals(data as string, "hello world");
});

Deno.test("[std/node/fs] readFile callback isn't called twice if error is thrown", async () => {
  const tempFile = await Deno.makeTempFile();
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { readFile } from ${JSON.stringify(importUrl)}`,
    invocation: `readFile(${JSON.stringify(tempFile)}, `,
    async cleanup() {
      await Deno.remove(tempFile);
    },
  });
});

Deno.test("fs.promises.readFile with no arg call rejects with error correctly", async () => {
  // @ts-ignore no arg call needs to be supported
  await promises.readFile().catch((_e) => {});
});

Deno.test("fs.readFile error message contains path + syscall", async () => {
  const path = "/does/not/exist";
  const err = await new Promise((resolve) => {
    readFile(path, "utf-8", (err) => resolve(err));
  });
  if (err instanceof Error) {
    assert(err.message.includes(path), "Path not found in error message");
    assertMatch(err.message, /[,\s]open\s/);
  }
});

Deno.test("fs.readFileSync error message contains path + syscall", () => {
  const path = "/does/not/exist";
  try {
    readFileSync(path, "utf-8");
  } catch (err) {
    if (err instanceof Error) {
      assert(err.message.includes(path), "Path not found in error message");
      assertMatch(err.message, /[,\s]open\s/);
    }
  }
});

Deno.test("fs.readFile returns Buffer when encoding is not provided", async () => {
  const data = await new Promise<Uint8Array>((res, rej) => {
    readFile(testData, (err, data) => {
      if (err) {
        rej(err);
      }
      res(data as Uint8Array);
    });
  });

  assert(data instanceof Uint8Array);
  assertEquals(Buffer.isBuffer(data), true);
  assertEquals(data.toString(), "hello world");
});

Deno.test("fs.readFile binary encoding returns string", async () => {
  const data = await new Promise<string>((res, rej) => {
    readFile(testData, { encoding: "binary" }, (err, data) => {
      if (err) {
        rej(err);
      }
      res(data as string);
    });
  });

  assertEquals(typeof data, "string");
  assertEquals(data, "hello world");
});

Deno.test("fs.readFileSync returns Buffer when encoding is not provided", () => {
  const data = readFileSync(testData);
  assert(data instanceof Uint8Array);
  assertEquals(Buffer.isBuffer(data), true);
  assertEquals(data.toString(), "hello world");
});

Deno.test("fs.readFileSync binary encoding returns string", () => {
  const data = readFileSync(testData, { encoding: "binary" });
  assertEquals(typeof data, "string");
  assertEquals(data, "hello world");
});

Deno.test("fs.readFile creates new file when passed 'w+' flag", async () => {
  const tmpDir = Deno.makeTempDirSync();
  const filePath = path.join(tmpDir, "newfile.txt");
  await new Promise((res, rej) => {
    readFile(filePath, { flag: "w+" }, (err, data) => {
      if (err) {
        rej(err);
      }
      res(data);
    });
  });

  assert(existsSync(filePath));
  Deno.removeSync(tmpDir, { recursive: true });
});

Deno.test("fs.readFileSync creates new file when passed 'w+' flag", () => {
  const tmpDir = Deno.makeTempDirSync();
  const filePath = path.join(tmpDir, "newfile.txt");
  readFileSync(filePath, { flag: "w+" });

  assert(existsSync(filePath));
  Deno.removeSync(tmpDir, { recursive: true });
});
