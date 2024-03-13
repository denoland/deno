// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import * as fs from "node:fs";
import { assertEquals, assertRejects } from "@std/assert/mod.ts";
import * as path from "@std/path/mod.ts";

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
  // https://github.com/denoland/deno/issues/22897
  ignore: Deno.build.os == "windows",
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
  name: "fs.statfs() bigint",
  // https://github.com/denoland/deno/issues/22897
  ignore: Deno.build.os == "windows",
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
  // https://github.com/denoland/deno/issues/22897
  ignore: Deno.build.os == "windows",
  fn() {
    const statFs = fs.statfsSync(filePath);
    assertStatFs(statFs);
  },
});

Deno.test({
  name: "fs.statfsSync() bigint",
  // https://github.com/denoland/deno/issues/22897
  ignore: Deno.build.os == "windows",
  fn() {
    const statFs = fs.statfsSync(filePath, { bigint: true });
    assertStatFs(statFs, { bigint: true });
  },
});

Deno.test({
  name: "fs.statfs() non-existent path",
  // https://github.com/denoland/deno/issues/22897
  ignore: Deno.build.os == "windows",
  async fn() {
    const nonExistentPath = path.join(filePath, "../non-existent");
    await assertRejects(async () => {
      await fs.promises.statfs(nonExistentPath);
    }, "NotFound");
  },
});
