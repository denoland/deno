// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { lstat, lstatSync } from "./_fs_lstat.ts";
import { fail } from "../../testing/asserts.ts";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { assertStats, assertStatsBigInt } from "./_fs_stat_test.ts";
import type { BigIntStats, Stats } from "./_fs_stat.ts";

Deno.test({
  name: "ASYNC: get a file Stats (lstat)",
  async fn() {
    const file = Deno.makeTempFileSync();
    await new Promise<Stats>((resolve, reject) => {
      lstat(file, (err, stat) => {
        if (err) reject(err);
        resolve(stat);
      });
    })
      .then((stat) => {
        assertStats(stat, Deno.lstatSync(file));
      }, () => fail())
      .finally(() => {
        Deno.removeSync(file);
      });
  },
});

Deno.test({
  name: "SYNC: get a file Stats (lstat)",
  fn() {
    const file = Deno.makeTempFileSync();
    assertStats(lstatSync(file), Deno.lstatSync(file));
  },
});

Deno.test({
  name: "ASYNC: get a file BigInt Stats (lstat)",
  async fn() {
    const file = Deno.makeTempFileSync();
    await new Promise<BigIntStats>((resolve, reject) => {
      lstat(file, { bigint: true }, (err, stat) => {
        if (err) reject(err);
        resolve(stat);
      });
    })
      .then(
        (stat) => assertStatsBigInt(stat, Deno.lstatSync(file)),
        () => fail(),
      )
      .finally(() => Deno.removeSync(file));
  },
});

Deno.test({
  name: "SYNC: BigInt Stats (lstat)",
  fn() {
    const file = Deno.makeTempFileSync();
    assertStatsBigInt(lstatSync(file, { bigint: true }), Deno.lstatSync(file));
  },
});

Deno.test("[std/node/fs] lstat callback isn't called twice if error is thrown", async () => {
  const tempFile = await Deno.makeTempFile();
  const importUrl = new URL("./_fs_lstat.ts", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { lstat } from ${JSON.stringify(importUrl)}`,
    invocation: `lstat(${JSON.stringify(tempFile)}, `,
    async cleanup() {
      await Deno.remove(tempFile);
    },
  });
});
