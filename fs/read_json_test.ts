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

test(async function readJsonFileNotExists() {
  const emptyJsonFile = path.join(testdataDir, "json_not_exists.json");

  await assertThrowsAsync(async () => {
    await readJson(emptyJsonFile);
  });
});

test(async function readEmptyJsonFile() {
  const emptyJsonFile = path.join(testdataDir, "json_empty.json");

  await assertThrowsAsync(async () => {
    await readJson(emptyJsonFile);
  });
});

test(async function readInvalidJsonFile() {
  const invalidJsonFile = path.join(testdataDir, "json_invalid.json");

  await assertThrowsAsync(async () => {
    await readJson(invalidJsonFile);
  });
});

test(async function readValidArrayJsonFile() {
  const invalidJsonFile = path.join(testdataDir, "json_valid_array.json");

  const json = await readJson(invalidJsonFile);

  assertEquals(json, ["1", "2", "3"]);
});

test(async function readValidObjJsonFile() {
  const invalidJsonFile = path.join(testdataDir, "json_valid_obj.json");

  const json = await readJson(invalidJsonFile);

  assertEquals(json, { key1: "value1", key2: "value2" });
});

test(async function readValidObjJsonFileWithRelativePath() {
  const json = await readJson("./fs/testdata/json_valid_obj.json");

  assertEquals(json, { key1: "value1", key2: "value2" });
});

test(function readJsonFileNotExistsSync() {
  const emptyJsonFile = path.join(testdataDir, "json_not_exists.json");

  assertThrows(() => {
    readJsonSync(emptyJsonFile);
  });
});

test(function readEmptyJsonFileSync() {
  const emptyJsonFile = path.join(testdataDir, "json_empty.json");

  assertThrows(() => {
    readJsonSync(emptyJsonFile);
  });
});

test(function readInvalidJsonFile() {
  const invalidJsonFile = path.join(testdataDir, "json_invalid.json");

  assertThrows(() => {
    readJsonSync(invalidJsonFile);
  });
});

test(function readValidArrayJsonFileSync() {
  const invalidJsonFile = path.join(testdataDir, "json_valid_array.json");

  const json = readJsonSync(invalidJsonFile);

  assertEquals(json, ["1", "2", "3"]);
});

test(function readValidObjJsonFileSync() {
  const invalidJsonFile = path.join(testdataDir, "json_valid_obj.json");

  const json = readJsonSync(invalidJsonFile);

  assertEquals(json, { key1: "value1", key2: "value2" });
});

test(function readValidObjJsonFileSyncWithRelativePath() {
  const json = readJsonSync("./fs/testdata/json_valid_obj.json");

  assertEquals(json, { key1: "value1", key2: "value2" });
});
