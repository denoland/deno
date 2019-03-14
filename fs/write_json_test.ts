// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import {
  assertEquals,
  assertThrowsAsync,
  assertThrows
} from "../testing/asserts.ts";
import { writeJson, writeJsonSync } from "./write_json.ts";
import * as path from "./path/mod.ts";

const testdataDir = path.resolve("fs", "testdata");

test(async function writeJsonIfNotExists() {
  const notExistsJsonFile = path.join(testdataDir, "file_not_exists.json");

  await assertThrowsAsync(
    async () => {
      await writeJson(notExistsJsonFile, { a: "1" });
      throw new Error("should write success");
    },
    Error,
    "should write success"
  );

  const content = await Deno.readFile(notExistsJsonFile);

  await Deno.remove(notExistsJsonFile);

  assertEquals(new TextDecoder().decode(content), `{"a":"1"}`);
});

test(async function writeJsonIfExists() {
  const existsJsonFile = path.join(testdataDir, "file_write_exists.json");

  await Deno.writeFile(existsJsonFile, new Uint8Array());

  await assertThrowsAsync(
    async () => {
      await writeJson(existsJsonFile, { a: "1" });
      throw new Error("should write success");
    },
    Error,
    "should write success"
  );

  const content = await Deno.readFile(existsJsonFile);

  await Deno.remove(existsJsonFile);

  assertEquals(new TextDecoder().decode(content), `{"a":"1"}`);
});

test(async function writeJsonIfExistsAnInvalidJson() {
  const existsInvalidJsonFile = path.join(
    testdataDir,
    "file_write_invalid.json"
  );

  const invalidJsonContent = new TextEncoder().encode("[123}");
  await Deno.writeFile(existsInvalidJsonFile, invalidJsonContent);

  await assertThrowsAsync(
    async () => {
      await writeJson(existsInvalidJsonFile, { a: "1" });
      throw new Error("should write success");
    },
    Error,
    "should write success"
  );

  const content = await Deno.readFile(existsInvalidJsonFile);

  await Deno.remove(existsInvalidJsonFile);

  assertEquals(new TextDecoder().decode(content), `{"a":"1"}`);
});

test(async function writeJsonWithSpaces() {
  const existsJsonFile = path.join(testdataDir, "file_write_spaces.json");

  const invalidJsonContent = new TextEncoder().encode();
  await Deno.writeFile(existsJsonFile, invalidJsonContent);

  await assertThrowsAsync(
    async () => {
      await writeJson(existsJsonFile, { a: "1" }, { spaces: 2 });
      throw new Error("should write success");
    },
    Error,
    "should write success"
  );

  const content = await Deno.readFile(existsJsonFile);

  await Deno.remove(existsJsonFile);

  assertEquals(new TextDecoder().decode(content), `{\n  "a": "1"\n}`);
});

test(async function writeJsonWithReplacer() {
  const existsJsonFile = path.join(testdataDir, "file_write_replacer.json");

  const invalidJsonContent = new TextEncoder().encode();
  await Deno.writeFile(existsJsonFile, invalidJsonContent);

  await assertThrowsAsync(
    async () => {
      await writeJson(
        existsJsonFile,
        { a: "1", b: "2", c: "3" },
        {
          replacer: ["a"]
        }
      );
      throw new Error("should write success");
    },
    Error,
    "should write success"
  );

  const content = await Deno.readFile(existsJsonFile);

  await Deno.remove(existsJsonFile);

  assertEquals(new TextDecoder().decode(content), `{"a":"1"}`);
});

test(function writeJsonSyncIfNotExists() {
  const notExistsJsonFile = path.join(testdataDir, "file_not_exists_sync.json");

  assertThrows(
    () => {
      writeJsonSync(notExistsJsonFile, { a: "1" });
      throw new Error("should write success");
    },
    Error,
    "should write success"
  );

  const content = Deno.readFileSync(notExistsJsonFile);

  Deno.removeSync(notExistsJsonFile);

  assertEquals(new TextDecoder().decode(content), `{"a":"1"}`);
});

test(function writeJsonSyncIfExists() {
  const existsJsonFile = path.join(testdataDir, "file_write_exists_sync.json");

  Deno.writeFileSync(existsJsonFile, new Uint8Array());

  assertThrows(
    () => {
      writeJsonSync(existsJsonFile, { a: "1" });
      throw new Error("should write success");
    },
    Error,
    "should write success"
  );

  const content = Deno.readFileSync(existsJsonFile);

  Deno.removeSync(existsJsonFile);

  assertEquals(new TextDecoder().decode(content), `{"a":"1"}`);
});

test(function writeJsonSyncIfExistsAnInvalidJson() {
  const existsInvalidJsonFile = path.join(
    testdataDir,
    "file_write_invalid_sync.json"
  );

  const invalidJsonContent = new TextEncoder().encode("[123}");
  Deno.writeFileSync(existsInvalidJsonFile, invalidJsonContent);

  assertThrows(
    () => {
      writeJsonSync(existsInvalidJsonFile, { a: "1" });
      throw new Error("should write success");
    },
    Error,
    "should write success"
  );

  const content = Deno.readFileSync(existsInvalidJsonFile);

  Deno.removeSync(existsInvalidJsonFile);

  assertEquals(new TextDecoder().decode(content), `{"a":"1"}`);
});

test(function writeJsonWithSpaces() {
  const existsJsonFile = path.join(testdataDir, "file_write_spaces_sync.json");

  const invalidJsonContent = new TextEncoder().encode();
  Deno.writeFileSync(existsJsonFile, invalidJsonContent);

  assertThrows(
    () => {
      writeJsonSync(existsJsonFile, { a: "1" }, { spaces: 2 });
      throw new Error("should write success");
    },
    Error,
    "should write success"
  );

  const content = Deno.readFileSync(existsJsonFile);

  Deno.removeSync(existsJsonFile);

  assertEquals(new TextDecoder().decode(content), `{\n  "a": "1"\n}`);
});

test(function writeJsonWithReplacer() {
  const existsJsonFile = path.join(
    testdataDir,
    "file_write_replacer_sync.json"
  );

  const invalidJsonContent = new TextEncoder().encode();
  Deno.writeFileSync(existsJsonFile, invalidJsonContent);

  assertThrows(
    () => {
      writeJsonSync(
        existsJsonFile,
        { a: "1", b: "2", c: "3" },
        {
          replacer: ["a"]
        }
      );
      throw new Error("should write success");
    },
    Error,
    "should write success"
  );

  const content = Deno.readFileSync(existsJsonFile);

  Deno.removeSync(existsJsonFile);

  assertEquals(new TextDecoder().decode(content), `{"a":"1"}`);
});
