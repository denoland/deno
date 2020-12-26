// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { readFile, readFileSync } from "./_fs_readFile.ts";
import * as path from "../../path/mod.ts";
import {
  assert,
  assertEquals,
  assertStringIncludes,
} from "../../testing/asserts.ts";

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
  // The correct behaviour is not to catch any errors thrown,
  // but that means there'll be an uncaught error and the test will fail.
  // So the only way to test this is to spawn a subprocess, and succeed if it has a non-zero exit code.
  // (assertThrowsAsync won't work because there's no way to catch the error.)
  const tempFile = await Deno.makeTempFile();
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "--no-check",
      `
      import { readFile } from "${
        new URL("./_fs_readFile.ts", import.meta.url).href
      }";

      readFile(${JSON.stringify(tempFile)}, (err) => {
        // If the bug is present and the callback is called again with an error,
        // don't throw another error, so if the subprocess fails we know it had the correct behaviour.
        if (!err) throw new Error("success");
      });`,
    ],
    stderr: "piped",
  });
  const status = await p.status();
  const stderr = new TextDecoder().decode(await Deno.readAll(p.stderr));
  p.close();
  p.stderr.close();
  await Deno.remove(tempFile);
  assert(!status.success);
  assertStringIncludes(stderr, "Error: success");
});
