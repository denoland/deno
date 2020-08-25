// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as path from "../path/mod.ts";
import {
  assertEquals,
  assertThrows,
  assertThrowsAsync,
} from "../testing/asserts.ts";
import {
  exists,
  existsSync,
} from "./exists.ts";
import { writeJson, writeJsonSync } from "./write_json.ts";

const testdataDir = path.resolve("fs", "testdata");

Deno.test("writeJson not exists", async function (): Promise<void> {
  const notExistsJsonFile = path.join(testdataDir, "writeJson_not_exists.json");

  await writeJson(notExistsJsonFile, { a: "1" });

  const content = await Deno.readTextFile(notExistsJsonFile);

  await Deno.remove(notExistsJsonFile);

  assertEquals(content, `{"a":"1"}\n`);
});

Deno.test("writeJson if not exists", async function (): Promise<void> {
  const notExistsJsonFile = path.join(
    testdataDir,
    "writeJson_file_not_exists.json",
  );

  try {
    assertThrowsAsync(
      async function (): Promise<void> {
        await writeJson(notExistsJsonFile, { a: "1" }, { create: false });
      },
      Deno.errors.NotFound,
    );
  } finally {
    if (await exists(notExistsJsonFile)) await Deno.remove(notExistsJsonFile);
  }
});

Deno.test("writeJson exists", async function (): Promise<void> {
  const existsJsonFile = path.join(testdataDir, "writeJson_exists.json");
  await Deno.writeFile(existsJsonFile, new Uint8Array());

  try {
    await writeJson(existsJsonFile, { a: "1" });
    const content = await Deno.readTextFile(existsJsonFile);
    assertEquals(content, `{"a":"1"}\n`);
  } finally {
    await Deno.remove(existsJsonFile);
  }
});

Deno.test("writeJson spaces", async function (): Promise<void> {
  const existsJsonFile = path.join(testdataDir, "writeJson_spaces.json");
  await Deno.writeFile(existsJsonFile, new Uint8Array());

  try {
    await writeJson(existsJsonFile, { a: "1" }, { spaces: 2 });
    const content = await Deno.readTextFile(existsJsonFile);
    assertEquals(content, `{\n  "a": "1"\n}\n`);
  } finally {
    await Deno.remove(existsJsonFile);
  }
});

Deno.test("writeJson replacer", async function (): Promise<void> {
  const existsJsonFile = path.join(testdataDir, "writeJson_replacer.json");
  await Deno.writeFile(existsJsonFile, new Uint8Array());

  try {
    await writeJson(
      existsJsonFile,
      { a: "1", b: "2", c: "3" },
      { replacer: ["a"] },
    );

    const content = await Deno.readTextFile(existsJsonFile);
    assertEquals(content, `{"a":"1"}\n`);
  } finally {
    await Deno.remove(existsJsonFile);
  }
});

Deno.test("writeJson append", async function (): Promise<void> {
  const existsJsonFile = path.join(testdataDir, "writeJson_append.json");
  await Deno.writeFile(existsJsonFile, new Uint8Array());

  try {
    await writeJson(existsJsonFile, { a: "1" }, { append: true });
    await writeJson(existsJsonFile, { b: "2" }, { append: true });

    const content = await Deno.readTextFile(existsJsonFile);
    assertEquals(content, `{"a":"1"}\n{"b":"2"}\n`);
  } finally {
    await Deno.remove(existsJsonFile);
  }
});

Deno.test("writeJsonSync not exists", function (): void {
  const notExistsJsonFile = path.join(
    testdataDir,
    "writeJsonSync_not_exists.json",
  );

  writeJsonSync(notExistsJsonFile, { a: "1" });

  const content = Deno.readTextFileSync(notExistsJsonFile);

  Deno.removeSync(notExistsJsonFile);

  assertEquals(content, `{"a":"1"}\n`);
});

Deno.test("writeJsonSync if not exists", function (): void {
  const notExistsJsonFile = path.join(
    testdataDir,
    "writeJsonSync_file_not_exists.json",
  );

  try {
    assertThrows(
      function (): void {
        writeJsonSync(notExistsJsonFile, { a: "1" }, { create: false });
      },
      Deno.errors.NotFound,
    );
  } finally {
    if (existsSync(notExistsJsonFile)) Deno.removeSync(notExistsJsonFile);
  }
});

Deno.test("writeJsonSync exists", function (): void {
  const existsJsonFile = path.join(testdataDir, "writeJsonSync_exists.json");
  Deno.writeFileSync(existsJsonFile, new Uint8Array());

  try {
    writeJsonSync(existsJsonFile, { a: "1" });
    const content = Deno.readTextFileSync(existsJsonFile);
    assertEquals(content, `{"a":"1"}\n`);
  } finally {
    Deno.removeSync(existsJsonFile);
  }
});

Deno.test("writeJsonSync spaces", function (): void {
  const existsJsonFile = path.join(testdataDir, "writeJsonSync_spaces.json");

  Deno.writeFileSync(existsJsonFile, new Uint8Array());

  try {
    writeJsonSync(existsJsonFile, { a: "1" }, { spaces: 2 });
    const content = Deno.readTextFileSync(existsJsonFile);
    assertEquals(content, `{\n  "a": "1"\n}\n`);
  } finally {
    Deno.removeSync(existsJsonFile);
  }
});

Deno.test("writeJsonSync replacer", function (): void {
  const existsJsonFile = path.join(
    testdataDir,
    "writeJsonSync_replacer.json",
  );

  Deno.writeFileSync(existsJsonFile, new Uint8Array());

  try {
    writeJsonSync(
      existsJsonFile,
      { a: "1", b: "2", c: "3" },
      { replacer: ["a"] },
    );

    const content = Deno.readTextFileSync(existsJsonFile);
    assertEquals(content, `{"a":"1"}\n`);
  } finally {
    Deno.removeSync(existsJsonFile);
  }
});

Deno.test("writeJsonSync append", function (): void {
  const existsJsonFile = path.join(testdataDir, "writeJsonSync_append.json");

  Deno.writeFileSync(existsJsonFile, new Uint8Array());

  try {
    writeJsonSync(existsJsonFile, { a: "1" }, { append: true });
    writeJsonSync(existsJsonFile, { b: "2" }, { append: true });

    const content = Deno.readTextFileSync(existsJsonFile);
    assertEquals(content, `{"a":"1"}\n{"b":"2"}\n`);
  } finally {
    Deno.removeSync(existsJsonFile);
  }
});
