const { test } = Deno;
import { readFile, readFileSync, readlink, readlinkSync } from "./fs.ts";
import * as path from "../path/mod.ts";
import { assertEquals, assert } from "../testing/asserts.ts";

const testData = path.resolve(path.join("node", "testdata", "hello.txt"));
const testDir = Deno.makeTempDirSync();
const oldname = testDir + "/oldname";
const newname = testDir + "/newname";

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

// Just for now, until we implement symlink for Windows.
const skip = Deno.build.os == "win";

if (!skip) {
  Deno.symlinkSync(oldname, newname);
}

test({
  skip,
  name: "readlinkSuccess",
  async fn() {
    const data = await new Promise((res, rej) => {
      readlink(newname, (err, data) => {
        if (err) {
          rej(err);
        }
        res(data);
      });
    });

    assertEquals(typeof data, "string");
    assertEquals(data as string, oldname);
  }
});

test({
  skip,
  name: "readlinkEncodeBufferSuccess",
  async fn() {
    const data = await new Promise((res, rej) => {
      readlink(newname, { encoding: "buffer" }, (err, data) => {
        if (err) {
          rej(err);
        }
        res(data);
      });
    });

    assert(data instanceof Uint8Array);
    assertEquals(new TextDecoder().decode(data as Uint8Array), oldname);
  }
});

test({
  skip,
  name: "readlinkSyncSuccess",
  fn() {
    const data = readlinkSync(newname);
    assertEquals(typeof data, "string");
    assertEquals(data as string, oldname);
  }
});

test({
  skip,
  name: "readlinkEncodeBufferSuccess",
  fn() {
    const data = readlinkSync(newname, { encoding: "buffer" });
    assert(data instanceof Uint8Array);
    assertEquals(new TextDecoder().decode(data as Uint8Array), oldname);
  }
});
