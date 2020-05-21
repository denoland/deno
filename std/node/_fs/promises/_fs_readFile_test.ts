const { test } = Deno;
import { readFile } from "./_fs_readFile.ts";
import * as path from "../../../path/mod.ts";
import { assertEquals, assert } from "../../../testing/asserts.ts";

const testData = path.resolve(
  path.join("node", "_fs", "testdata", "hello.txt")
);

test("readFileSuccess", async function () {
  const data = await readFile(testData);

  assert(data instanceof Uint8Array);
  assertEquals(new TextDecoder().decode(data as Uint8Array), "hello world");
});

test("readFileEncodeUtf8Success", async function () {
  const data = await readFile(testData, { encoding: "utf8" });

  assertEquals(typeof data, "string");
  assertEquals(data as string, "hello world");
});

test("readFileEncodingAsString", async function () {
  const data = await readFile(testData, "utf8");

  assertEquals(typeof data, "string");
  assertEquals(data as string, "hello world");
});

test("readFileError", async function () {
  try {
    await readFile("invalid-file", "utf8");
  } catch (e) {
    assert(e instanceof Deno.errors.NotFound);
  }
});
