// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  assertEquals,
  assertThrowsAsync,
  assertThrows,
} from "../testing/asserts.ts";
import { EOL, format } from "./eol.ts";
import * as path from "../path/mod.ts";
import { writeJson, writeJsonSync } from "./write_json.ts";

const testdataDir = path.resolve("fs", "testdata");
const platformEol = Deno.build.os === "windows" ? EOL.CRLF : EOL.LF;

Deno.test("writeJsonIfNotExists", async function (): Promise<void> {
  const notExistsJsonFile = path.join(testdataDir, "file_not_exists.json");

  await assertThrowsAsync(
    async (): Promise<void> => {
      await writeJson(notExistsJsonFile, { a: "1" });
      throw new Error("should write success");
    },
    Error,
    "should write success",
  );

  const content = await Deno.readFile(notExistsJsonFile);

  await Deno.remove(notExistsJsonFile);

  const expected = format(`{"a":"1"}\n`, platformEol);
  assertEquals(new TextDecoder().decode(content), expected);
});

Deno.test("writeJsonIfExists", async function (): Promise<void> {
  const existsJsonFile = path.join(testdataDir, "file_write_exists.json");

  await Deno.writeFile(existsJsonFile, new Uint8Array());

  await assertThrowsAsync(
    async (): Promise<void> => {
      await writeJson(existsJsonFile, { a: "1" });
      throw new Error("should write success");
    },
    Error,
    "should write success",
  );

  const content = await Deno.readFile(existsJsonFile);

  await Deno.remove(existsJsonFile);

  const expected = format(`{"a":"1"}\n`, platformEol);
  assertEquals(new TextDecoder().decode(content), expected);
});

Deno.test("writeJsonIfExistsAnInvalidJson", async function (): Promise<void> {
  const existsInvalidJsonFile = path.join(
    testdataDir,
    "file_write_invalid.json",
  );

  const invalidJsonContent = new TextEncoder().encode("[123}");
  await Deno.writeFile(existsInvalidJsonFile, invalidJsonContent);

  await assertThrowsAsync(
    async (): Promise<void> => {
      await writeJson(existsInvalidJsonFile, { a: "1" });
      throw new Error("should write success");
    },
    Error,
    "should write success",
  );

  const content = await Deno.readFile(existsInvalidJsonFile);

  await Deno.remove(existsInvalidJsonFile);

  const expected = format(`{"a":"1"}\n`, platformEol);
  assertEquals(new TextDecoder().decode(content), expected);
});

Deno.test("writeJsonWithSpaces", async function (): Promise<void> {
  const existsJsonFile = path.join(testdataDir, "file_write_spaces.json");

  const invalidJsonContent = new TextEncoder().encode();
  await Deno.writeFile(existsJsonFile, invalidJsonContent);

  await assertThrowsAsync(
    async (): Promise<void> => {
      await writeJson(existsJsonFile, { a: "1" }, { spaces: 2 });
      throw new Error("should write success");
    },
    Error,
    "should write success",
  );

  const content = await Deno.readFile(existsJsonFile);

  await Deno.remove(existsJsonFile);

  const expected = format(`{\n  "a": "1"\n}\n`, platformEol);
  assertEquals(new TextDecoder().decode(content), expected);
});

Deno.test("writeJsonWithReplacer", async function (): Promise<void> {
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
        },
      );
      throw new Error("should write success");
    },
    Error,
    "should write success",
  );

  const content = await Deno.readFile(existsJsonFile);

  await Deno.remove(existsJsonFile);

  const expected = format(`{"a":"1"}\n`, platformEol);
  assertEquals(new TextDecoder().decode(content), expected);
});

Deno.test("writeJsonSyncIfNotExists", function (): void {
  const notExistsJsonFile = path.join(testdataDir, "file_not_exists_sync.json");

  assertThrows(
    (): void => {
      writeJsonSync(notExistsJsonFile, { a: "1" });
      throw new Error("should write success");
    },
    Error,
    "should write success",
  );

  const content = Deno.readFileSync(notExistsJsonFile);

  Deno.removeSync(notExistsJsonFile);

  const expected = format(`{"a":"1"}\n`, platformEol);
  assertEquals(new TextDecoder().decode(content), expected);
});

Deno.test("writeJsonSyncIfExists", function (): void {
  const existsJsonFile = path.join(testdataDir, "file_write_exists_sync.json");

  Deno.writeFileSync(existsJsonFile, new Uint8Array());

  assertThrows(
    (): void => {
      writeJsonSync(existsJsonFile, { a: "1" });
      throw new Error("should write success");
    },
    Error,
    "should write success",
  );

  const content = Deno.readFileSync(existsJsonFile);

  Deno.removeSync(existsJsonFile);

  const expected = format(`{"a":"1"}\n`, platformEol);
  assertEquals(new TextDecoder().decode(content), expected);
});

Deno.test("writeJsonSyncIfExistsAnInvalidJson", function (): void {
  const existsInvalidJsonFile = path.join(
    testdataDir,
    "file_write_invalid_sync.json",
  );

  const invalidJsonContent = new TextEncoder().encode("[123}");
  Deno.writeFileSync(existsInvalidJsonFile, invalidJsonContent);

  assertThrows(
    (): void => {
      writeJsonSync(existsInvalidJsonFile, { a: "1" });
      throw new Error("should write success");
    },
    Error,
    "should write success",
  );

  const content = Deno.readFileSync(existsInvalidJsonFile);

  Deno.removeSync(existsInvalidJsonFile);

  const expected = format(`{"a":"1"}\n`, platformEol);
  assertEquals(new TextDecoder().decode(content), expected);
});

Deno.test("writeJsonWithSpaces", function (): void {
  const existsJsonFile = path.join(testdataDir, "file_write_spaces_sync.json");

  const invalidJsonContent = new TextEncoder().encode();
  Deno.writeFileSync(existsJsonFile, invalidJsonContent);

  assertThrows(
    (): void => {
      writeJsonSync(existsJsonFile, { a: "1" }, { spaces: 2 });
      throw new Error("should write success");
    },
    Error,
    "should write success",
  );

  const content = Deno.readFileSync(existsJsonFile);

  Deno.removeSync(existsJsonFile);

  const expected = format(`{\n  "a": "1"\n}\n`, platformEol);
  assertEquals(new TextDecoder().decode(content), expected);
});

Deno.test("writeJsonWithReplacer", function (): void {
  const existsJsonFile = path.join(
    testdataDir,
    "file_write_replacer_sync.json",
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
        },
      );
      throw new Error("should write success");
    },
    Error,
    "should write success",
  );

  const content = Deno.readFileSync(existsJsonFile);

  Deno.removeSync(existsJsonFile);

  const expected = format(`{"a":"1"}\n`, platformEol);
  assertEquals(new TextDecoder().decode(content), expected);
});
