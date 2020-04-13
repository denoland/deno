// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  assertEquals,
  assertThrowsAsync,
  assertThrows,
} from "../testing/asserts.ts";
import * as path from "../path/mod.ts";
import { writeJson, writeJsonSync } from "./write_json.ts";

const testdataDir = path.resolve("fs", "testdata");

Deno.test(async function writeJsonIfNotExists(): Promise<void> {
  const notExistsJsonFile = path.join(testdataDir, "file_not_exists.json");

  await assertThrowsAsync(
    async (): Promise<void> => {
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

Deno.test(async function writeJsonIfExists(): Promise<void> {
  const existsJsonFile = path.join(testdataDir, "file_write_exists.json");

  await Deno.writeFile(existsJsonFile, new Uint8Array());

  await assertThrowsAsync(
    async (): Promise<void> => {
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

Deno.test(async function writeJsonIfExistsAnInvalidJson(): Promise<void> {
  const existsInvalidJsonFile = path.join(
    testdataDir,
    "file_write_invalid.json"
  );

  const invalidJsonContent = new TextEncoder().encode("[123}");
  await Deno.writeFile(existsInvalidJsonFile, invalidJsonContent);

  await assertThrowsAsync(
    async (): Promise<void> => {
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

Deno.test(async function writeJsonWithSpaces(): Promise<void> {
  const existsJsonFile = path.join(testdataDir, "file_write_spaces.json");

  const invalidJsonContent = new TextEncoder().encode();
  await Deno.writeFile(existsJsonFile, invalidJsonContent);

  await assertThrowsAsync(
    async (): Promise<void> => {
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

Deno.test(async function writeJsonWithReplacer(): Promise<void> {
  const existsJsonFile = path.join(testdataDir, "file_write_replacer.json");

  const invalidJsonContent = new TextEncoder().encode();
  await Deno.writeFile(existsJsonFile, invalidJsonContent);

  await assertThrowsAsync(
    async (): Promise<void> => {
      await writeJson(
        existsJsonFile,
        { a: "1", b: "2", c: "3" },
        {
          replacer: ["a"],
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

Deno.test(function writeJsonSyncIfNotExists(): void {
  const notExistsJsonFile = path.join(testdataDir, "file_not_exists_sync.json");

  assertThrows(
    (): void => {
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

Deno.test(function writeJsonSyncIfExists(): void {
  const existsJsonFile = path.join(testdataDir, "file_write_exists_sync.json");

  Deno.writeFileSync(existsJsonFile, new Uint8Array());

  assertThrows(
    (): void => {
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

Deno.test(function writeJsonSyncIfExistsAnInvalidJson(): void {
  const existsInvalidJsonFile = path.join(
    testdataDir,
    "file_write_invalid_sync.json"
  );

  const invalidJsonContent = new TextEncoder().encode("[123}");
  Deno.writeFileSync(existsInvalidJsonFile, invalidJsonContent);

  assertThrows(
    (): void => {
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

Deno.test(function writeJsonWithSpaces(): void {
  const existsJsonFile = path.join(testdataDir, "file_write_spaces_sync.json");

  const invalidJsonContent = new TextEncoder().encode();
  Deno.writeFileSync(existsJsonFile, invalidJsonContent);

  assertThrows(
    (): void => {
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

Deno.test(function writeJsonWithReplacer(): void {
  const existsJsonFile = path.join(
    testdataDir,
    "file_write_replacer_sync.json"
  );

  const invalidJsonContent = new TextEncoder().encode();
  Deno.writeFileSync(existsJsonFile, invalidJsonContent);

  assertThrows(
    (): void => {
      writeJsonSync(
        existsJsonFile,
        { a: "1", b: "2", c: "3" },
        {
          replacer: ["a"],
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
