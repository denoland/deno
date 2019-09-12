// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import {
  assertEquals,
  assertThrowsAsync,
  assertThrows
} from "../testing/asserts.ts";
import { readJson, readJsonSync } from "./read_json.ts";
import * as path from "./path/mod.ts";

const testdataDir = path.resolve("fs", "testdata");

test(async function readJsonFileNotExists(): Promise<void> {
  const emptyJsonFile = path.join(testdataDir, "json_not_exists.json");

  await assertThrowsAsync(
    async (): Promise<void> => {
      await readJson(emptyJsonFile);
    }
  );
});

test(async function readEmptyJsonFile(): Promise<void> {
  const emptyJsonFile = path.join(testdataDir, "json_empty.json");

  await assertThrowsAsync(
    async (): Promise<void> => {
      await readJson(emptyJsonFile);
    }
  );
});

test(async function readInvalidJsonFile(): Promise<void> {
  const invalidJsonFile = path.join(testdataDir, "json_invalid.json");

  await assertThrowsAsync(
    async (): Promise<void> => {
      await readJson(invalidJsonFile);
    }
  );
});

test(async function readValidArrayJsonFile(): Promise<void> {
  const invalidJsonFile = path.join(testdataDir, "json_valid_array.json");

  const json = await readJson(invalidJsonFile);

  assertEquals(json, ["1", "2", "3"]);
});

test(async function readValidObjJsonFile(): Promise<void> {
  const invalidJsonFile = path.join(testdataDir, "json_valid_obj.json");

  const json = await readJson(invalidJsonFile);

  assertEquals(json, { key1: "value1", key2: "value2" });
});

test(async function readValidObjJsonFileWithRelativePath(): Promise<void> {
  const json = await readJson("./fs/testdata/json_valid_obj.json");

  assertEquals(json, { key1: "value1", key2: "value2" });
});

test(function readJsonFileNotExistsSync(): void {
  const emptyJsonFile = path.join(testdataDir, "json_not_exists.json");

  assertThrows((): void => {
    readJsonSync(emptyJsonFile);
  });
});

test(function readEmptyJsonFileSync(): void {
  const emptyJsonFile = path.join(testdataDir, "json_empty.json");

  assertThrows((): void => {
    readJsonSync(emptyJsonFile);
  });
});

test(function readInvalidJsonFile(): void {
  const invalidJsonFile = path.join(testdataDir, "json_invalid.json");

  assertThrows((): void => {
    readJsonSync(invalidJsonFile);
  });
});

test(function readValidArrayJsonFileSync(): void {
  const invalidJsonFile = path.join(testdataDir, "json_valid_array.json");

  const json = readJsonSync(invalidJsonFile);

  assertEquals(json, ["1", "2", "3"]);
});

test(function readValidObjJsonFileSync(): void {
  const invalidJsonFile = path.join(testdataDir, "json_valid_obj.json");

  const json = readJsonSync(invalidJsonFile);

  assertEquals(json, { key1: "value1", key2: "value2" });
});

test(function readValidObjJsonFileSyncWithRelativePath(): void {
  const json = readJsonSync("./fs/testdata/json_valid_obj.json");

  assertEquals(json, { key1: "value1", key2: "value2" });
});
