const { test } = Deno;
import { readFile, readFileSync } from "./_fs_readFile.ts";
import * as path from "../../path/mod.ts";
import { assertEquals, assert } from "../../testing/asserts.ts";

const testData = path.resolve(
  path.join("node", "_fs", "testdata", "hello.txt")
);

test("readFileSuccess", async function () {
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

test("readFileEncodeUtf8Success", async function () {
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

test("readFileEncodingAsString", async function () {
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

test("readFileSyncSuccess", function () {
  const data = readFileSync(testData);
  assert(data instanceof Uint8Array);
  assertEquals(new TextDecoder().decode(data as Uint8Array), "hello world");
});

test("readFileEncodeUtf8Success", function () {
  const data = readFileSync(testData, { encoding: "utf8" });
  assertEquals(typeof data, "string");
  assertEquals(data as string, "hello world");
});

test("readFileEncodeAsString", function () {
  const data = readFileSync(testData, "utf8");
  assertEquals(typeof data, "string");
  assertEquals(data as string, "hello world");
});
