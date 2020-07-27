// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as path from "../path/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { writeJson, writeJsonSync } from "./write_json.ts";

const testdataDir = path.resolve("fs", "testdata");

Deno.test("writeJsonIfNotExists", async function (): Promise<void> {
  const notExistsJsonFile = path.join(testdataDir, "file_not_exists.json");

  await writeJson(notExistsJsonFile, { a: "1" });

  const content = await Deno.readFile(notExistsJsonFile);

  await Deno.remove(notExistsJsonFile);

  assertEquals(new TextDecoder().decode(content), `{"a":"1"}\n`);
});

Deno.test("writeJsonIfExists", async function (): Promise<void> {
  const existsJsonFile = path.join(testdataDir, "file_write_exists.json");

  await Deno.writeFile(existsJsonFile, new Uint8Array());

  try {
    await writeJson(existsJsonFile, { a: "1" });
  } catch {
    // empty
  }

  const content = await Deno.readFile(existsJsonFile);

  await Deno.remove(existsJsonFile);

  assertEquals(new TextDecoder().decode(content), `{"a":"1"}\n`);
});

Deno.test("writeJsonIfExistsAnInvalidJson", async function (): Promise<void> {
  const existsInvalidJsonFile = path.join(
    testdataDir,
    "file_write_invalid.json",
  );

  const invalidJsonContent = new TextEncoder().encode("[123}");
  await Deno.writeFile(existsInvalidJsonFile, invalidJsonContent);

  try {
    await writeJson(existsInvalidJsonFile, { a: "1" });
  } catch {
    // empty
  }

  const content = await Deno.readFile(existsInvalidJsonFile);

  await Deno.remove(existsInvalidJsonFile);

  assertEquals(new TextDecoder().decode(content), `{"a":"1"}\n`);
});

Deno.test("writeJsonWithSpaces", async function (): Promise<void> {
  const existsJsonFile = path.join(testdataDir, "file_write_spaces.json");

  const invalidJsonContent = new TextEncoder().encode();
  await Deno.writeFile(existsJsonFile, invalidJsonContent);

  try {
    await writeJson(existsJsonFile, { a: "1" }, { spaces: 2 });
  } catch {
    // empty
  }

  const content = await Deno.readFile(existsJsonFile);

  await Deno.remove(existsJsonFile);

  assertEquals(new TextDecoder().decode(content), `{\n  "a": "1"\n}\n`);
});

Deno.test("writeJsonWithReplacer", async function (): Promise<void> {
  const existsJsonFile = path.join(testdataDir, "file_write_replacer.json");

  const invalidJsonContent = new TextEncoder().encode();
  await Deno.writeFile(existsJsonFile, invalidJsonContent);

  try {
    await writeJson(
      existsJsonFile,
      { a: "1", b: "2", c: "3" },
      { replacer: ["a"] },
    );
  } catch {
    // empty
  }

  const content = await Deno.readFile(existsJsonFile);

  await Deno.remove(existsJsonFile);

  assertEquals(new TextDecoder().decode(content), `{"a":"1"}\n`);
});

Deno.test("writeJsonAppend", async function (): Promise<void> {
  const existsJsonFile = path.join(testdataDir, "file_write_append.json");

  await Deno.writeFile(existsJsonFile, new Uint8Array());

  try {
    await writeJson(existsJsonFile, { a: "1" }, { append: true });
    await writeJson(existsJsonFile, { b: "2" }, { append: true });
  } catch {
    // empty
  }

  const content = await Deno.readFile(existsJsonFile);

  await Deno.remove(existsJsonFile);

  assertEquals(new TextDecoder().decode(content), `{"a":"1"}\n{"b":"2"}\n`);
});

Deno.test("writeJsonSyncIfNotExists", function (): void {
  const notExistsJsonFile = path.join(testdataDir, "file_not_exists_sync.json");

  try {
    writeJsonSync(notExistsJsonFile, { a: "1" });
  } catch {
    // empty
  }

  const content = Deno.readFileSync(notExistsJsonFile);

  Deno.removeSync(notExistsJsonFile);

  assertEquals(new TextDecoder().decode(content), `{"a":"1"}\n`);
});

Deno.test("writeJsonSyncIfExists", function (): void {
  const existsJsonFile = path.join(testdataDir, "file_write_exists_sync.json");

  Deno.writeFileSync(existsJsonFile, new Uint8Array());

  try {
    writeJsonSync(existsJsonFile, { a: "1" });
  } catch {
    // empty
  }

  const content = Deno.readFileSync(existsJsonFile);

  Deno.removeSync(existsJsonFile);

  assertEquals(new TextDecoder().decode(content), `{"a":"1"}\n`);
});

Deno.test("writeJsonSyncIfExistsAnInvalidJson", function (): void {
  const existsInvalidJsonFile = path.join(
    testdataDir,
    "file_write_invalid_sync.json",
  );

  const invalidJsonContent = new TextEncoder().encode("[123}");
  Deno.writeFileSync(existsInvalidJsonFile, invalidJsonContent);

  try {
    writeJsonSync(existsInvalidJsonFile, { a: "1" });
  } catch {
    // empty
  }

  const content = Deno.readFileSync(existsInvalidJsonFile);

  Deno.removeSync(existsInvalidJsonFile);

  assertEquals(new TextDecoder().decode(content), `{"a":"1"}\n`);
});

Deno.test("writeJsonSyncWithSpaces", function (): void {
  const existsJsonFile = path.join(testdataDir, "file_write_spaces_sync.json");

  const invalidJsonContent = new TextEncoder().encode();
  Deno.writeFileSync(existsJsonFile, invalidJsonContent);

  try {
    writeJsonSync(existsJsonFile, { a: "1" }, { spaces: 2 });
  } catch {
    // empty
  }

  const content = Deno.readFileSync(existsJsonFile);

  Deno.removeSync(existsJsonFile);

  assertEquals(new TextDecoder().decode(content), `{\n  "a": "1"\n}\n`);
});

Deno.test("writeJsonSyncWithReplacer", function (): void {
  const existsJsonFile = path.join(
    testdataDir,
    "file_write_replacer_sync.json",
  );

  const invalidJsonContent = new TextEncoder().encode();
  Deno.writeFileSync(existsJsonFile, invalidJsonContent);

  try {
    writeJsonSync(
      existsJsonFile,
      { a: "1", b: "2", c: "3" },
      { replacer: ["a"] },
    );
  } catch {
    // empty
  }

  const content = Deno.readFileSync(existsJsonFile);

  Deno.removeSync(existsJsonFile);

  assertEquals(new TextDecoder().decode(content), `{"a":"1"}\n`);
});

Deno.test("writeJsonSyncAppend", function (): void {
  const existsJsonFile = path.join(testdataDir, "file_write_append_sync.json");

  Deno.writeFileSync(existsJsonFile, new Uint8Array());

  try {
    writeJsonSync(existsJsonFile, { a: "1" }, { append: true });
    writeJsonSync(existsJsonFile, { b: "2" }, { append: true });
  } catch {
    // empty
  }

  const content = Deno.readFileSync(existsJsonFile);

  Deno.removeSync(existsJsonFile);

  assertEquals(new TextDecoder().decode(content), `{"a":"1"}\n{"b":"2"}\n`);
});
