// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { statfs, statfsSync, StatsFsBase } from "node:fs";
import { assertEquals } from "jsr:@std/assert";

function assertStatFs(statFs: StatsFsBase<unknown>, { bigint = false } = {}) {
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

Deno.test({
  name: "fs.statfs()",
  async fn() {
    await new Promise<StatsFsBase<unknown>>((resolve, reject) => {
      statfs("/", (err, statFs) => {
        if (err) reject(err);
        resolve(statFs);
      });
    }).then((statFs) => assertStatFs(statFs));
  },
});

Deno.test({
  name: "fs.statfs() bigint",
  async fn() {
    await new Promise<StatsFsBase<unknown>>((resolve, reject) => {
      statfs("/", { bigint: true }, (err, statFs) => {
        if (err) reject(err);
        resolve(statFs);
      });
    }).then((statFs) => assertStatFs(statFs, { bigint: true }));
  },
});

Deno.test({
  name: "fs.statfsSync()",
  fn() {
    const statFs = statfsSync("/");
    assertStatFs(statFs);
  },
});

Deno.test({
  name: "fs.statfsSync() bigint",
  fn() {
    const statFs = statfsSync("/", { bigint: true });
    assertStatFs(statFs, { bigint: true });
  },
});
