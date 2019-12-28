import { readFile, readFileSync } from "./fs.ts";
import { test } from "../testing/mod.ts";
import * as path from "../path/mod.ts";
import { assertEquals, assert } from "../testing/asserts.ts";

const testData = path.resolve(path.join("node", "testdata", "hello.txt"));

// Need to convert to promises, otherwise test() won't report error correctly.
test(async function readFileSuccess() {
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

test(async function readFileEncodeUtf8Success() {
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

test(function readFileSyncSuccess() {
  const data = readFileSync(testData);
  assert(data instanceof Uint8Array);
  assertEquals(new TextDecoder().decode(data as Uint8Array), "hello world");
});

test(function readFileEncodeUtf8Success() {
  const data = readFileSync(testData, { encoding: "utf8" });
  assertEquals(typeof data, "string");
  assertEquals(data as string, "hello world");
});
