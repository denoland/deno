// Copyright 2018-2025 the Deno authors. MIT license.

import * as fs from "node:fs";
import { assertEquals, assertRejects, assertThrows } from "@std/assert";
import * as path from "@std/path";
import { Buffer } from "node:buffer";

function assertStatFs(
  statFs: fs.StatsFsBase<unknown>,
  { bigint = false } = {},
) {
  assertEquals(statFs.constructor.name, "StatFs");
  const expectedType = bigint ? "bigint" : "number";
  assertEquals(typeof statFs.type, expectedType);
  assertEquals(typeof statFs.bsize, expectedType);
  assertEquals(typeof statFs.blocks, expectedType);
  assertEquals(typeof statFs.bfree, expectedType);
  assertEquals(typeof statFs.bavail, expectedType);
  assertEquals(typeof statFs.files, expectedType);
  assertEquals(typeof statFs.ffree, expectedType);
  if (Deno.build.os == "windows") {
    assertEquals(statFs.type, bigint ? 0n : 0);
    assertEquals(statFs.files, bigint ? 0n : 0);
    assertEquals(statFs.ffree, bigint ? 0n : 0);
  }
}

const filePath = path.fromFileUrl(import.meta.url);

Deno.test({
  name: "fs.statfs()",
  async fn() {
    await new Promise<fs.StatsFsBase<unknown>>((resolve, reject) => {
      fs.statfs(filePath, (err, statFs) => {
        if (err) reject(err);
        resolve(statFs);
      });
    }).then((statFs) => assertStatFs(statFs));
  },
});

Deno.test({
  name: "fs.statfs() with Buffer",
  async fn() {
    await new Promise<fs.StatsFsBase<unknown>>((resolve, reject) => {
      fs.statfs(Buffer.from(filePath), (err, statFs) => {
        if (err) reject(err);
        resolve(statFs);
      });
    }).then((statFs) => assertStatFs(statFs));
  },
});

Deno.test({
  name: "fs.statfs() bigint",
  async fn() {
    await new Promise<fs.StatsFsBase<unknown>>((resolve, reject) => {
      fs.statfs(filePath, { bigint: true }, (err, statFs) => {
        if (err) reject(err);
        resolve(statFs);
      });
    }).then((statFs) => assertStatFs(statFs, { bigint: true }));
  },
});

Deno.test({
  name: "fs.statfsSync()",
  fn() {
    const statFs = fs.statfsSync(filePath);
    assertStatFs(statFs);
  },
});

Deno.test({
  name: "fs.statfsSync() with Buffer",
  fn() {
    const statFs = fs.statfsSync(Buffer.from(filePath));
    assertStatFs(statFs);
  },
});

Deno.test({
  name: "fs.statfsSync() bigint",
  fn() {
    const statFs = fs.statfsSync(filePath, { bigint: true });
    assertStatFs(statFs, { bigint: true });
  },
});

Deno.test({
  name: "fs.statfs() non-existent path",
  async fn() {
    const nonExistentPath = path.join(filePath, "../non-existent");
    await assertRejects(
      async () => await fs.promises.statfs(nonExistentPath),
      `ENOENT: no such file or directory, statfs '${nonExistentPath}'`,
    );
  },
});

Deno.test({
  name: "fs.statfsSync() non-existent path",
  fn() {
    const nonExistentPath = path.join(filePath, "../non-existent");
    assertThrows(
      () => fs.statfsSync(nonExistentPath),
      `ENOENT: no such file or directory, statfs '${nonExistentPath}'`,
    );
  },
});
