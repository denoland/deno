import { readFile, readFileSync, readlink, readlinkSync } from "./fs.ts";
import { test } from "../testing/mod.ts";
import * as path from "../path/mod.ts";
import { assertEquals, assert } from "../testing/asserts.ts";
const { run } = Deno;

const testData = path.resolve(path.join("node", "testdata", "hello.txt"));
const testLinkPath = "./testdata/hello.txt";
const testLink = "hello.txt";
run({
  args: ["ln", "-s", testLinkPath, testLink]
});

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

test(async function readlinkSuccess() {
  const data = await new Promise((res, rej) => {
    readlink(testLink, (err, data) => {
      if (err) {
        rej(err);
      }
      res(data);
    });
  });

  assertEquals(typeof data, "string");
  assertEquals(data as string, testLinkPath);
});

test(async function readlinkEncodeBufferSuccess() {
  const data = await new Promise((res, rej) => {
    readlink(testLink, { encoding: "buffer" }, (err, data) => {
      if (err) {
        rej(err);
      }
      res(data);
    });
  });

  assert(data instanceof Uint8Array);
  assertEquals(new TextDecoder().decode(data as Uint8Array), testLinkPath);
});

test(function readlinkSyncSuccess() {
  const data = readlinkSync(testLink);
  assertEquals(typeof data, "string");
  assertEquals(data as string, testLinkPath);
});

test(function readlinkEncodeBufferSuccess() {
  const data = readlinkSync(testLink, { encoding: "buffer" });
  assert(data instanceof Uint8Array);
  assertEquals(new TextDecoder().decode(data as Uint8Array), testLinkPath);
  run({
    args: ["rm", "-rf", testLink]
  });
});
