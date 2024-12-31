// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals, assertRejects, assertThrows, fail } from "@std/assert";
import { rm, rmSync } from "node:fs";
import { existsSync } from "node:fs";
import { join } from "@std/path";

Deno.test({
  name: "ASYNC: removing empty folder",
  async fn() {
    const dir = Deno.makeTempDirSync();
    await new Promise<void>((resolve, reject) => {
      rm(dir, { recursive: true }, (err) => {
        if (err) reject(err);
        resolve();
      });
    })
      .then(() => assertEquals(existsSync(dir), false), () => fail())
      .finally(() => {
        if (existsSync(dir)) Deno.removeSync(dir);
      });
  },
});

Deno.test({
  name: "ASYNC: removing non-empty folder",
  async fn() {
    const dir = Deno.makeTempDirSync();
    using _file1 = Deno.createSync(join(dir, "file1.txt"));
    using _file2 = Deno.createSync(join(dir, "file2.txt"));
    Deno.mkdirSync(join(dir, "some_dir"));
    using _file = Deno.createSync(join(dir, "some_dir", "file.txt"));
    await new Promise<void>((resolve, reject) => {
      rm(dir, { recursive: true }, (err) => {
        if (err) reject(err);
        resolve();
      });
    })
      .then(() => assertEquals(existsSync(dir), false), () => fail())
      .finally(() => {
        if (existsSync(dir)) Deno.removeSync(dir, { recursive: true });
      });
  },
  ignore: Deno.build.os === "windows",
});

Deno.test({
  name: "ASYNC: removing a file",
  async fn() {
    const file = Deno.makeTempFileSync();
    await new Promise<void>((resolve, reject) => {
      rm(file, (err) => {
        if (err) reject(err);
        resolve();
      });
    });

    assertEquals(existsSync(file), false);
  },
});

Deno.test({
  name: "ASYNC: remove should fail if target does not exist",
  async fn() {
    const removePromise = new Promise<void>((resolve, reject) => {
      rm("/path/to/noexist.text", (err) => {
        if (err) reject(err);
        resolve();
      });
    });
    await assertRejects(() => removePromise, Error);
  },
});

Deno.test({
  name:
    "ASYNC: remove should not fail if target does not exist and force option is true",
  async fn() {
    await new Promise<void>((resolve, reject) => {
      rm("/path/to/noexist.text", { force: true }, (err) => {
        if (err) reject(err);
        resolve();
      });
    });
  },
});

Deno.test({
  name: "SYNC: removing empty folder",
  fn() {
    const dir = Deno.makeTempDirSync();
    rmSync(dir, { recursive: true });
    assertEquals(existsSync(dir), false);
  },
});

Deno.test({
  name: "SYNC: removing non-empty folder",
  fn() {
    const dir = Deno.makeTempDirSync();
    using _file1 = Deno.createSync(join(dir, "file1.txt"));
    using _file2 = Deno.createSync(join(dir, "file2.txt"));
    Deno.mkdirSync(join(dir, "some_dir"));
    using _file = Deno.createSync(join(dir, "some_dir", "file.txt"));
    rmSync(dir, { recursive: true });
    assertEquals(existsSync(dir), false);
  },
  ignore: Deno.build.os === "windows",
});

Deno.test({
  name: "SYNC: removing a file",
  fn() {
    const file = Deno.makeTempFileSync();

    rmSync(file);

    assertEquals(existsSync(file), false);
  },
});

Deno.test({
  name: "SYNC: remove should fail if target does not exist",
  fn() {
    assertThrows(() => rmSync("/path/to/noexist.text"), Error);
  },
});

Deno.test({
  name:
    "SYNC: remove should not fail if target does not exist and force option is true",
  fn() {
    rmSync("/path/to/noexist.text", { force: true });
  },
});
