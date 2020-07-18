// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  assertEquals,
  assertThrowsAsync,
  assertThrows,
} from "../testing/asserts.ts";
import * as path from "../path/mod.ts";
import { readJson, readJsonSync } from "./read_json.ts";

const testdataDir = path.resolve("fs", "testdata");

Deno.test("readJsonFileNotExists", async function (): Promise<void> {
  const emptyJsonFile = path.join(testdataDir, "json_not_exists.json");

  await assertThrowsAsync(
    async (): Promise<void> => {
      await readJson(emptyJsonFile);
    },
  );
});

Deno.test("readEmptyJsonFile", async function (): Promise<void> {
  const emptyJsonFile = path.join(testdataDir, "json_empty.json");

  await assertThrowsAsync(
    async (): Promise<void> => {
      await readJson(emptyJsonFile);
    },
  );
});

Deno.test("readInvalidJsonFile", async function (): Promise<void> {
  const invalidJsonFile = path.join(testdataDir, "json_invalid.json");

  await assertThrowsAsync(
    async (): Promise<void> => {
      await readJson(invalidJsonFile);
    },
  );
});

Deno.test("readValidArrayJsonFile", async function (): Promise<void> {
  const invalidJsonFile = path.join(testdataDir, "json_valid_array.json");

  const json = await readJson(invalidJsonFile);

  assertEquals(json, ["1", "2", "3"]);
});

Deno.test("readValidObjJsonFile", async function (): Promise<void> {
  const invalidJsonFile = path.join(testdataDir, "json_valid_obj.json");

  const json = await readJson(invalidJsonFile);

  assertEquals(json, { key1: "value1", key2: "value2" });
});

Deno.test("readValidObjJsonFileWithRelativePath", async function (): Promise<
  void
> {
  const json = await readJson("./fs/testdata/json_valid_obj.json");

  assertEquals(json, { key1: "value1", key2: "value2" });
});

Deno.test("readJsonFileNotExistsSync", function (): void {
  const emptyJsonFile = path.join(testdataDir, "json_not_exists.json");

  assertThrows((): void => {
    readJsonSync(emptyJsonFile);
  });
});

Deno.test("readEmptyJsonFileSync", function (): void {
  const emptyJsonFile = path.join(testdataDir, "json_empty.json");

  assertThrows((): void => {
    readJsonSync(emptyJsonFile);
  });
});

Deno.test("readInvalidJsonFile", function (): void {
  const invalidJsonFile = path.join(testdataDir, "json_invalid.json");

  assertThrows((): void => {
    readJsonSync(invalidJsonFile);
  });
});

Deno.test("readValidArrayJsonFileSync", function (): void {
  const invalidJsonFile = path.join(testdataDir, "json_valid_array.json");

  const json = readJsonSync(invalidJsonFile);

  assertEquals(json, ["1", "2", "3"]);
});

Deno.test("readValidObjJsonFileSync", function (): void {
  const invalidJsonFile = path.join(testdataDir, "json_valid_obj.json");

  const json = readJsonSync(invalidJsonFile);

  assertEquals(json, { key1: "value1", key2: "value2" });
});

Deno.test("readValidObjJsonFileSyncWithRelativePath", function (): void {
  const json = readJsonSync("./fs/testdata/json_valid_obj.json");

  assertEquals(json, { key1: "value1", key2: "value2" });
});
