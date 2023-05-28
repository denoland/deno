import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { open } from "node:fs/promises";
import * as path from "../../../../test_util/std/path/mod.ts";
import {
  assert,
  assertEquals,
} from "../../../../test_util/std/testing/asserts.ts";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testData = path.resolve(moduleDir, "testdata", "hello.txt");

Deno.test("readFileSuccess", async function () {
    const fileHandle = await fs.open("./main.ts");
    const data = await fileHandle.readFile()

  assert(data instanceof Uint8Array);
  assertEquals(new TextDecoder().decode(data as Uint8Array), "hello world");
});
